//! Bitmap subtitles: PGS, VobSub and DVB are pictures, not text. The server will not convert
//! them to SRT, so the player extracts the stream from the same direct-play URL with ffmpeg and
//! renders it in-process with `libbitsub-core`. The result is a list of RGBA rectangles that
//! `in_app.rs` places over the video; the video path stays zero-copy.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use jellyfin_api::Client;
use libbitsub_core::{PgsParser, VobSubParser};
use rune::Pixels;

/// One image track's decoder. DVB is transcoded by ffmpeg into DVD subtitle packets at
/// extraction time, so it joins the VobSub parser rather than growing a second bitmap stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageCodec {
    Pgs,
    VobSub,
    Dvb,
}

impl ImageCodec {
    /// The codecs Jellyfin reports for bitmap subtitle streams.
    pub fn for_codec(codec: Option<&str>) -> Option<ImageCodec> {
        match codec?.to_ascii_lowercase().as_str() {
            "pgssub" | "hdmv_pgs_subtitle" | "pgs" => Some(ImageCodec::Pgs),
            "dvdsub" | "vobsub" | "dvd_subtitle" => Some(ImageCodec::VobSub),
            "dvbsub" | "dvb_subtitle" | "dvb" => Some(ImageCodec::Dvb),
            _ => None,
        }
    }

    fn extension(self) -> &'static str {
        match self {
            ImageCodec::Pgs => "sup",
            ImageCodec::VobSub => "mks",
            // Not `.mks`: a DVB track and a DVD track on the same index must not share a cache
            // file if the server's codec report changes.
            ImageCodec::Dvb => "dvb",
        }
    }

    fn ffmpeg_format(self) -> &'static str {
        match self {
            ImageCodec::Pgs => "sup",
            // A subtitle-only Matroska keeps the original SPU packets; the crate reads MKS.
            ImageCodec::VobSub => "matroska",
            // ffmpeg cannot write DVB subtitles into a file we parse, but it can decode them
            // and encode the same pictures as DVD subtitle packets, which the parser reads.
            ImageCodec::Dvb => "matroska",
        }
    }

    fn ffmpeg_codec(self) -> &'static str {
        match self {
            ImageCodec::Pgs | ImageCodec::VobSub => "copy",
            ImageCodec::Dvb => "dvdsub",
        }
    }
}

/// One RGBA rectangle in the subtitle canvas. `pixels` is thread-safe and uploaded by Rune.
pub struct ImageBitmap {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub pixels: Pixels,
}

enum Parser {
    Pgs(PgsParser),
    VobSub(VobSubParser),
    /// A snapshot/fixture frame that is already rendered.
    #[cfg(debug_assertions)]
    Static,
}

/// A decoded image track. Only the active display set is held: the parser keeps the compressed
/// stream and renders the cue under the playhead on demand.
pub struct ImageSubtitle {
    parser: Parser,
    width: u16,
    height: u16,
    active_index: i32,
    pub bitmaps: Vec<ImageBitmap>,
}

impl ImageSubtitle {
    pub fn parse(codec: ImageCodec, bytes: &[u8]) -> Result<ImageSubtitle, String> {
        let parser = match codec {
            ImageCodec::Pgs => {
                let mut parser = PgsParser::new();
                let count = parser.parse(bytes);
                if count == 0 {
                    return Err("This PGS track has no display sets.".into());
                }
                Parser::Pgs(parser)
            }
            ImageCodec::VobSub | ImageCodec::Dvb => {
                let mut parser = VobSubParser::new();
                parser
                    .load_from_mks(bytes)
                    .map_err(|error| format!("This bitmap track could not be read: {error}"))?;
                if parser.count() == 0 {
                    return Err("This bitmap track has no cues.".into());
                }
                Parser::VobSub(parser)
            }
        };
        let (width, height) = match &parser {
            Parser::Pgs(parser) => (parser.screen_width(), parser.screen_height()),
            Parser::VobSub(parser) => (parser.screen_width(), parser.screen_height()),
            #[cfg(debug_assertions)]
            Parser::Static => (0, 0),
        };
        Ok(ImageSubtitle {
            parser,
            width,
            height,
            active_index: -1,
            bitmaps: Vec::new(),
        })
    }

    /// A fixed frame for snapshots and tests; it has no timeline.
    #[cfg(debug_assertions)]
    pub fn from_bitmaps(width: u16, height: u16, bitmaps: Vec<ImageBitmap>) -> ImageSubtitle {
        ImageSubtitle {
            parser: Parser::Static,
            width,
            height,
            active_index: 0,
            bitmaps,
        }
    }

    pub fn screen(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    /// The first display set's start, for the live probe.
    #[cfg(test)]
    pub fn first_cue_ms(&self) -> Option<i64> {
        match &self.parser {
            Parser::Pgs(parser) => parser.get_timestamps().first().map(|time| *time as i64),
            Parser::VobSub(parser) => parser.get_timestamps().first().map(|time| *time as i64),
            #[cfg(debug_assertions)]
            Parser::Static => None,
        }
    }

    /// Render the cue for `position_ms` if the active display set changed. Returns true when
    /// the bitmaps were replaced, so the caller can request one frame.
    pub fn refresh(&mut self, position_ms: i64) -> bool {
        let index = match &mut self.parser {
            Parser::Pgs(parser) => parser.find_index_at_timestamp(position_ms as f64),
            Parser::VobSub(parser) => parser.find_index_at_timestamp(position_ms as f64),
            #[cfg(debug_assertions)]
            Parser::Static => return false,
        };
        if index == self.active_index {
            return false;
        }
        self.active_index = index;
        self.bitmaps.clear();
        if index < 0 {
            return true;
        }
        match &mut self.parser {
            Parser::Pgs(parser) => {
                if let Some(frame) = parser.render_at_index(index as usize) {
                    for at in 0..frame.composition_count() {
                        let Some(composition) = frame.get_composition(at) else {
                            continue;
                        };
                        self.bitmaps.push(bitmap(
                            composition.x().into(),
                            composition.y().into(),
                            composition.width(),
                            composition.height(),
                            composition.get_rgba().to_vec(),
                        ));
                    }
                }
            }
            Parser::VobSub(parser) => {
                if let Some(frame) = parser.render_at_index(index as usize) {
                    self.bitmaps.push(bitmap(
                        frame.x().into(),
                        frame.y().into(),
                        frame.width(),
                        frame.height(),
                        frame.get_rgba().to_vec(),
                    ));
                }
            }
            #[cfg(debug_assertions)]
            Parser::Static => {}
        }
        true
    }
}

pub fn bitmap(x: f32, y: f32, width: u16, height: u16, rgba: Vec<u8>) -> ImageBitmap {
    let pixels = Pixels::new();
    pixels.set(width as u32, height as u32, rgba);
    ImageBitmap {
        x,
        y,
        width: width as f32,
        height: height as f32,
        pixels,
    }
}

/// `$XDG_CACHE_HOME/runefin/subtitles`, or `~/.cache/runefin/subtitles`.
pub fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(base.join("runefin").join("subtitles"))
}

pub fn cache_path(item_id: &str, index: i32, codec: ImageCodec) -> Option<PathBuf> {
    Some(cache_dir()?.join(format!("{item_id}-{index}.{}", codec.extension())))
}

/// One track's bytes: the disk cache first, then ffmpeg on the direct-play URL. The caller runs
/// this on a worker; the UI never waits for the extraction.
pub fn load(
    client: &Client,
    item_id: &str,
    stream_index: i32,
    codec: ImageCodec,
) -> Result<ImageSubtitle, String> {
    let path = cache_path(item_id, stream_index, codec)
        .ok_or("No cache directory for image subtitles.")?;
    let bytes = if let Ok(bytes) = std::fs::read(&path)
        && !bytes.is_empty()
    {
        bytes
    } else {
        let bytes = extract(client, item_id, stream_index, codec, &path)?;
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let temporary = path.with_extension(format!("{}.part", codec.extension()));
        std::fs::write(&temporary, &bytes)
            .map_err(|error| format!("Could not cache the image subtitle: {error}"))?;
        std::fs::rename(&temporary, &path)
            .map_err(|error| format!("Could not finish the image subtitle cache: {error}"))?;
        bytes
    };
    ImageSubtitle::parse(codec, &bytes)
}

fn extract(
    client: &Client,
    item_id: &str,
    stream_index: i32,
    codec: ImageCodec,
    path: &Path,
) -> Result<Vec<u8>, String> {
    let url = client
        .direct_stream_url(item_id)
        .ok_or("This item has no direct stream URL.")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("Could not create the subtitle cache: {error}"))?;
    }
    let temporary = path.with_extension(format!("{}.part", codec.extension()));
    let output = Command::new("ffmpeg")
        .args(["-nostdin", "-v", "error", "-y", "-i", &url])
        .args(["-map", &format!("0:{stream_index}")])
        .args(["-c:s", codec.ffmpeg_codec(), "-f", codec.ffmpeg_format()])
        .arg(&temporary)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                "ffmpeg is not installed. Install it to render image subtitles.".to_string()
            } else {
                format!("ffmpeg could not start: {error}")
            }
        })?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&temporary);
        let message = String::from_utf8_lossy(&output.stderr);
        let line = message.lines().last().unwrap_or("no detail").trim();
        return Err(format!("ffmpeg could not extract this image track: {line}"));
    }
    let bytes = std::fs::read(&temporary)
        .map_err(|error| format!("Could not read the extracted image subtitle: {error}"))?;
    let _ = std::fs::remove_file(&temporary);
    if bytes.is_empty() {
        return Err("ffmpeg produced an empty image subtitle.".into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jellyfin_codecs_map_to_the_supported_decoders() {
        assert_eq!(ImageCodec::for_codec(Some("PGSSUB")), Some(ImageCodec::Pgs));
        assert_eq!(
            ImageCodec::for_codec(Some("hdmv_pgs_subtitle")),
            Some(ImageCodec::Pgs)
        );
        assert_eq!(
            ImageCodec::for_codec(Some("dvdsub")),
            Some(ImageCodec::VobSub)
        );
        assert_eq!(
            ImageCodec::for_codec(Some("vobsub")),
            Some(ImageCodec::VobSub)
        );
        assert_eq!(ImageCodec::for_codec(Some("dvbsub")), Some(ImageCodec::Dvb));
        assert_eq!(
            ImageCodec::for_codec(Some("dvb_subtitle")),
            Some(ImageCodec::Dvb)
        );
        assert_eq!(ImageCodec::for_codec(None), None);
    }

    #[test]
    #[ignore = "needs a signed-in config and a PGS item on the server"]
    fn a_live_pgs_track_extracts_and_renders() {
        use jellyfin_api::ItemQuery;

        let config = crate::config::Config::load();
        assert!(config.has_session(), "sign in once before this test");
        let client = Client::new(&config.server_url, config.device_id.clone())
            .with_session(config.token.clone(), config.user_id.clone());
        let page = client
            .items(&ItemQuery {
                search: Some("Darker than BLACK Gaiden".into()),
                include_types: vec!["Episode"],
                recursive: true,
                limit: 10,
                extra_fields: Some("MediaSources".into()),
                ..ItemQuery::default()
            })
            .expect("search");
        let found = page.items.iter().find_map(|item| {
            item.subtitle_tracks().into_iter().find_map(|track| {
                ImageCodec::for_codec(track.codec.as_deref())
                    .filter(|codec| *codec == ImageCodec::Pgs)
                    .map(|codec| (item, track, codec))
            })
        });
        let Some((item, track, codec)) = found else {
            eprintln!("pgs: no item with a PGS track");
            return;
        };
        let mut image =
            load(&client, &item.id, track.index, codec).expect("extract and parse the PGS track");
        let first = image.first_cue_ms().expect("a first cue");
        assert!(image.refresh(first + 50), "the first cue becomes active");
        assert!(!image.bitmaps.is_empty(), "the first cue has a bitmap");
        eprintln!(
            "pgs: {:?} track {} at {} ms, {} bitmap(s), canvas {}x{}",
            item.name,
            track.index,
            first,
            image.bitmaps.len(),
            image.screen().0,
            image.screen().1
        );
    }

    #[test]
    #[ignore = "needs RUNEFIN_DVB_SAMPLE pointing at an MPEG-TS with DVB subtitles"]
    fn a_dvb_track_transcodes_and_renders() {
        let sample = std::env::var("RUNEFIN_DVB_SAMPLE").expect("the sample path");
        let dir = std::env::temp_dir().join(format!("runefin-dvb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a temp directory");
        let output = dir.join("track.mks");
        let status = Command::new("ffmpeg")
            .args(["-nostdin", "-v", "error", "-y", "-i", &sample])
            .args(["-c:s", ImageCodec::Dvb.ffmpeg_codec()])
            .args(["-f", ImageCodec::Dvb.ffmpeg_format()])
            .arg(&output)
            .status()
            .expect("ffmpeg");
        assert!(
            status.success(),
            "the DVB track transcodes to DVD subtitles"
        );
        let bytes = std::fs::read(&output).expect("the transcoded track");
        let mut image = ImageSubtitle::parse(ImageCodec::Dvb, &bytes).expect("a parsed track");
        let first = image.first_cue_ms().expect("a first DVB cue");
        assert!(image.refresh(first + 50), "the first cue becomes active");
        assert!(!image.bitmaps.is_empty(), "the first cue has a bitmap");
        eprintln!(
            "dvb: first cue {first} ms, {} bitmap(s), canvas {}x{}",
            image.bitmaps.len(),
            image.screen().0,
            image.screen().1
        );
    }

    #[test]
    #[ignore = "needs RUNEFIN_VOBSUB_SAMPLE pointing at an MKS extracted from a DVD track"]
    fn a_vobsub_track_renders() {
        let sample = std::env::var("RUNEFIN_VOBSUB_SAMPLE").expect("the sample path");
        let bytes = std::fs::read(sample).expect("the MKS file");
        let mut image = ImageSubtitle::parse(ImageCodec::VobSub, &bytes).expect("a parsed track");
        let first = image.first_cue_ms().expect("a first VobSub cue");
        assert!(image.refresh(first + 50), "the first cue becomes active");
        assert!(!image.bitmaps.is_empty(), "the first cue has a bitmap");
        eprintln!(
            "vobsub: first cue {first} ms, {} bitmap(s), canvas {}x{}",
            image.bitmaps.len(),
            image.screen().0,
            image.screen().1
        );
    }

    #[test]
    fn the_cache_name_is_stable_and_separates_codecs() {
        let pgs = cache_path("abc", 3, ImageCodec::Pgs).expect("cache");
        let vob = cache_path("abc", 3, ImageCodec::VobSub).expect("cache");
        assert!(pgs.ends_with("abc-3.sup"));
        assert!(vob.ends_with("abc-3.mks"));
        let dvb = cache_path("abc", 3, ImageCodec::Dvb).expect("cache");
        assert!(dvb.ends_with("abc-3.dvb"));
        assert_ne!(
            vob, dvb,
            "a DVB track and a DVD track on the same index differ"
        );
        assert_ne!(pgs, vob);
    }
}
