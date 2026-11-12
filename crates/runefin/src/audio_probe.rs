//! Decoding a window of a film's audio for [`crate::subtitle_sync`], so the aligner can compare
//! the cues against what is actually said.
//!
//! A short-lived playbin of its own, on the file's direct-play URL — not the playing pipeline's,
//! and not the server's transcoder: the subtitle's timeline is the file's, so the file's audio is
//! the right reference. Video is left out of the pipeline entirely, and the sink is an appsink
//! with `sync=false`, so the window decodes as fast as the machine and the network allow rather
//! than in real time.
//!
//! Runs on a worker. It reports seconds decoded through a shared counter and stops when the
//! request's cancel flag is set, which is what a closed panel or a new episode does.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app::AppSink;

/// The aligner's rate: mono, 8 kHz, which is what a voice-activity envelope needs and nothing more.
pub const RATE: u32 = 8_000;
/// A window this long is 4.8 MB of samples and plenty of dialogue to correlate against.
pub const MAX_SECONDS: u32 = 600;
/// A stalled source must not hold the request open forever.
const DEADLINE: Duration = Duration::from_secs(60);

/// A decoded window of audio, in item time.
pub struct Window {
    pub samples: Vec<i16>,
    /// Where the first sample sits in the item, in milliseconds.
    pub start_ms: i64,
}

/// Decode up to `seconds` of `url` from `start_ms`.
pub fn sample(
    url: &str,
    start_ms: i64,
    seconds: u32,
    track: Option<i32>,
    progress: &AtomicU64,
    cancelled: &AtomicBool,
) -> Result<Window, String> {
    let seconds = seconds.clamp(1, MAX_SECONDS);
    gst::init().map_err(|error| format!("GStreamer could not start: {error}"))?;
    let started = Instant::now();

    // Stereo, then averaged here: `audioconvert`'s mono downmix divides by the channel count, so
    // a 7.1 mix arrived 18 dB quieter than the same film through ffmpeg. The stereo downmix is
    // level-preserving, and taking the mean of two channels is a no-op for loudness.
    let caps = gst::Caps::builder("audio/x-raw")
        .field("format", "S16LE")
        .field("channels", 2i32)
        .field("rate", RATE as i32)
        .field("layout", "interleaved")
        .build();
    let convert = gst::ElementFactory::make("audioconvert")
        .build()
        .map_err(|error| format!("GStreamer has no audioconvert: {error}"))?;
    let resample = gst::ElementFactory::make("audioresample")
        .build()
        .map_err(|error| format!("GStreamer has no audioresample: {error}"))?;
    let filter = gst::ElementFactory::make("capsfilter")
        .property("caps", &caps)
        .build()
        .map_err(|error| format!("GStreamer has no capsfilter: {error}"))?;
    let sink = AppSink::builder().sync(false).max_buffers(4).build();
    let bin = gst::Bin::new();
    bin.add_many([
        &convert,
        &resample,
        &filter,
        sink.upcast_ref::<gst::Element>(),
    ])
    .map_err(|error| format!("GStreamer probe sink: {error}"))?;
    gst::Element::link_many([
        &convert,
        &resample,
        &filter,
        sink.upcast_ref::<gst::Element>(),
    ])
    .map_err(|error| format!("GStreamer probe sink: {error}"))?;
    // playbin links its own audio pad to this bin, so the bin needs a sink pad of its own.
    let pad = convert
        .static_pad("sink")
        .ok_or_else(|| "the probe converter has no sink pad".to_string())?;
    let ghost =
        gst::GhostPad::with_target(&pad).map_err(|error| format!("the probe sink pad: {error}"))?;
    bin.add_pad(&ghost)
        .map_err(|error| format!("the probe sink pad: {error}"))?;

    let playbin = gst::ElementFactory::make("playbin")
        .build()
        .map_err(|error| format!("GStreamer has no playbin: {error}"))?;
    playbin.set_property("uri", url);
    playbin.set_property("audio-sink", &bin);
    // Audio only: no video decode, no text, no visualisation.
    if playbin.find_property("flags").is_some() {
        playbin.set_property_from_str("flags", "soft-volume+audio");
    }
    if let Some(track) = track
        && playbin.find_property("current-audio").is_some()
    {
        playbin.set_property("current-audio", track);
    }
    let pipeline = playbin
        .downcast::<gst::Pipeline>()
        .map_err(|_| "GStreamer playbin is not a pipeline".to_string())?;
    let bus = pipeline
        .bus()
        .ok_or_else(|| "GStreamer probe pipeline has no bus".to_string())?;

    let stop = |pipeline: &gst::Pipeline| {
        let _ = pipeline.set_state(gst::State::Null);
    };
    pipeline
        .set_state(gst::State::Paused)
        .map_err(|error| format!("the probe could not open the stream: {error}"))?;
    if let Err(error) = wait_for(&bus, &pipeline, cancelled) {
        stop(&pipeline);
        return Err(error);
    }
    if start_ms > 0 {
        pipeline
            .seek_simple(
                gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                gst::ClockTime::from_mseconds(start_ms.max(0) as u64),
            )
            .map_err(|error| format!("the probe could not seek: {error}"))?;
        if let Err(error) = wait_for(&bus, &pipeline, cancelled) {
            stop(&pipeline);
            return Err(error);
        }
    }
    pipeline
        .set_state(gst::State::Playing)
        .map_err(|error| format!("the probe could not play the stream: {error}"))?;

    let want = seconds as usize * RATE as usize;
    let mut samples: Vec<i16> = Vec::with_capacity(want);
    // The window's real start is the first buffer's timestamp, not the seek we asked for: an
    // accurate seek still begins at the demuxer's keyframe when the file's GOP is long, and a
    // start that is 20 s out would shift every cue with it.
    let mut first_pts_ms: Option<i64> = None;
    while samples.len() < want {
        if cancelled.load(Ordering::Relaxed) {
            stop(&pipeline);
            return Err("cancelled".into());
        }
        if started.elapsed() > DEADLINE {
            stop(&pipeline);
            return Err("the stream stopped sending audio".into());
        }
        if sink.is_eos() {
            break;
        }
        match sink.try_pull_sample(gst::ClockTime::from_mseconds(200)) {
            Some(sample) => {
                if first_pts_ms.is_none()
                    && let Some(pts) = sample.buffer().and_then(|buffer| buffer.pts())
                {
                    first_pts_ms = Some(pts.mseconds() as i64);
                }
                if let Some(bytes) = sample
                    .buffer()
                    .and_then(|buffer| buffer.map_readable().ok())
                {
                    let take = (want - samples.len()) * 4;
                    let bytes = &bytes.as_slice()[..bytes.len().min(take)];
                    let (frames, _) = bytes.as_chunks::<4>();
                    samples.extend(frames.iter().map(|frame| {
                        let left = i16::from_le_bytes([frame[0], frame[1]]) as i32;
                        let right = i16::from_le_bytes([frame[2], frame[3]]) as i32;
                        ((left + right) / 2) as i16
                    }));
                }
                progress.store((samples.len() / RATE as usize) as u64, Ordering::Relaxed);
            }
            None => {
                // A timeout with no buffer is normal while the source reconnects; the deadline above
                // is what turns a dead source into an error.
                if let Some(message) = error_on(&bus) {
                    stop(&pipeline);
                    return Err(message);
                }
            }
        }
    }
    progress.store((samples.len() / RATE as usize) as u64, Ordering::Relaxed);
    stop(&pipeline);
    if samples.is_empty() {
        return Err("the probe decoded no audio".into());
    }
    Ok(Window {
        samples: samples[..samples.len().min(want)].to_vec(),
        start_ms: first_pts_ms.unwrap_or(start_ms),
    })
}

/// Wait for the pipeline to preroll after a state change or a seek, surfacing errors.
fn wait_for(
    bus: &gst::Bus,
    pipeline: &gst::Pipeline,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    let started = Instant::now();
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        match bus.timed_pop_filtered(
            gst::ClockTime::from_mseconds(100),
            &[gst::MessageType::AsyncDone, gst::MessageType::Error],
        ) {
            Some(message) => match message.view() {
                gst::MessageView::AsyncDone(_) => return Ok(()),
                gst::MessageView::Error(error) => {
                    return Err(format!(
                        "the probe could not decode the audio: {}",
                        error.error()
                    ));
                }
                _ => {}
            },
            None => {
                let _ = pipeline;
                if started.elapsed() > Duration::from_secs(20) {
                    return Err("the stream did not answer".into());
                }
            }
        }
    }
}

/// An error the bus raised while pulling, if any.
fn error_on(bus: &gst::Bus) -> Option<String> {
    let message = bus.timed_pop_filtered(gst::ClockTime::ZERO, &[gst::MessageType::Error])?;
    match message.view() {
        gst::MessageView::Error(error) => Some(format!(
            "the probe could not decode the audio: {}",
            error.error()
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtitle_sync;

    /// Two episodes of one series, through the real probe and the real detector:
    /// `RUNEFIN_PROBE_SERIES=<series id>`, optionally `RUNEFIN_PROBE_PAIR=a,b` (1-based).
    /// Prints what the pair shares, so the detector can be judged on a title sequence rather than
    /// on synthetic notes.
    #[test]
    #[ignore = "needs RUNEFIN_PROBE_SERIES=<series id>"]
    fn two_real_episodes_of_a_series_share_an_intro() {
        let series = std::env::var("RUNEFIN_PROBE_SERIES").expect("RUNEFIN_PROBE_SERIES");
        let config = crate::config::Config::load();
        let client = jellyfin_api::Client::new(&config.server_url, config.device_id.clone())
            .with_session(config.token.clone(), config.user_id.clone());
        let episodes = client
            .episodes_in(&series, jellyfin_api::SortBy::DatePlayed, 4)
            .expect("the series' episodes");
        let episodes = {
            let mut sorted = episodes;
            sorted.sort_by_key(|item| (item.parent_index_number, item.index_number));
            sorted
        };
        let pair: Vec<usize> = std::env::var("RUNEFIN_PROBE_PAIR")
            .ok()
            .and_then(|value| {
                let mut parts = value
                    .split(',')
                    .filter_map(|p| p.trim().parse::<usize>().ok());
                Some(vec![parts.next()? - 1, parts.next()? - 1])
            })
            .unwrap_or_else(|| vec![0, 1]);
        let (a, b) = (&episodes[pair[0]], &episodes[pair[1]]);
        let progress = AtomicU64::new(0);
        let cancelled = AtomicBool::new(false);
        let mut windows = Vec::new();
        for item in [a, b] {
            let url = client.direct_stream_url(&item.id).expect("a stream");
            let window = sample(&url, 0, 360, None, &progress, &cancelled).expect("decoded");
            windows.push(window);
        }
        let (wa, wb) = (&windows[0], &windows[1]);
        let found =
            crate::intro_detect::detect(&wa.samples, wa.start_ms, &wb.samples, wb.start_ms, RATE);
        let best = crate::intro_detect::best_match(
            &crate::intro_detect::features(&wa.samples, RATE),
            &crate::intro_detect::features(&wb.samples, RATE),
        );
        eprintln!(
            "{} vs {}: candidate {:?} (shift s, span start-end s, corr), then {:?}",
            a.name,
            b.name,
            best.map(|(shift, start, length, mean)| (
                shift as f32 * 0.2,
                start as f32 * 0.2,
                (start + length) as f32 * 0.2,
                (mean * 100.0).round() / 100.0
            )),
            found.map(|intro| (
                intro.start_ms / 1000,
                intro.end_ms / 1000,
                (intro.confidence * 100.0).round() / 100.0
            ))
        );
    }

    /// What a real item measures, for tuning the target: `RUNEFIN_PROBE_ITEM=<id>` and optionally
    /// `RUNEFIN_PROBE_START_MS`. Prints gated RMS and the gain it would take.
    #[test]
    #[ignore = "needs RUNEFIN_PROBE_ITEM=<id> [RUNEFIN_PROBE_START_MS]"]
    fn a_real_item_measures_its_level() {
        let config = crate::config::Config::load();
        let client = jellyfin_api::Client::new(&config.server_url, config.device_id.clone())
            .with_session(config.token.clone(), config.user_id.clone());
        // A file URL tests the probe's own level against an ffmpeg measurement of the same WAV.
        let (url, name, duration) = match std::env::var("RUNEFIN_PROBE_URL") {
            Ok(url) => (url, "url".to_string(), 0),
            Err(_) => {
                let item_id = std::env::var("RUNEFIN_PROBE_ITEM").expect("RUNEFIN_PROBE_ITEM");
                let item = client.playback_item(&item_id).expect("the item");
                let url = client
                    .direct_stream_url(&item_id)
                    .expect("the item has a stream");
                (url, item.name.clone(), item.runtime_ms().unwrap_or(0))
            }
        };
        let starts: Vec<i64> = std::env::var("RUNEFIN_PROBE_START_MS")
            .ok()
            .and_then(|value| value.parse::<i64>().ok())
            .map(|one| vec![one])
            .unwrap_or_else(|| {
                [2, 5, 8]
                    .iter()
                    .map(|tenths| duration * tenths / 10)
                    .collect()
            });
        let progress = AtomicU64::new(0);
        let cancelled = AtomicBool::new(false);
        let mut levels = Vec::new();
        for start in &starts {
            let track = std::env::var("RUNEFIN_PROBE_AUDIO")
                .ok()
                .and_then(|value| value.parse::<i32>().ok());
            let window =
                sample(&url, *start, 60, track, &progress, &cancelled).expect("the probe decodes");
            if let Ok(dump) = std::env::var("RUNEFIN_PROBE_DUMP") {
                let data_len = window.samples.len() as u32 * 2;
                let mut out = Vec::new();
                out.extend_from_slice(b"RIFF");
                out.extend_from_slice(&(36 + data_len).to_le_bytes());
                out.extend_from_slice(b"WAVEfmt ");
                out.extend_from_slice(&16u32.to_le_bytes());
                out.extend_from_slice(&1u16.to_le_bytes());
                out.extend_from_slice(&1u16.to_le_bytes());
                out.extend_from_slice(&RATE.to_le_bytes());
                out.extend_from_slice(&(RATE * 2).to_le_bytes());
                out.extend_from_slice(&2u16.to_le_bytes());
                out.extend_from_slice(&16u16.to_le_bytes());
                out.extend_from_slice(b"data");
                out.extend_from_slice(&data_len.to_le_bytes());
                for sample in &window.samples {
                    out.extend_from_slice(&sample.to_le_bytes());
                }
                std::fs::write(dump, out).expect("dump written");
            }
            let measured = crate::loudness::measure(&window.samples, RATE).expect("a level");
            levels.push(measured.dbfs);
        }
        let mut sorted = levels.clone();
        sorted.sort_by(f32::total_cmp);
        let measured = crate::loudness::at_level(sorted[sorted.len() / 2]);
        let gain_db = 20.0 * measured.gain.log10();
        eprintln!(
            "{}: windows {:?} -> median {:.1} dBFS, gain {gain_db:+.1} dB -> {:.1} dBFS",
            name,
            levels
                .iter()
                .map(|l| (l * 10.0).round() / 10.0)
                .collect::<Vec<_>>(),
            measured.dbfs,
            measured.dbfs + gain_db
        );
        assert!(
            measured.dbfs < -5.0 && measured.dbfs > -45.0,
            "{}",
            measured.dbfs
        );
    }

    /// The real path, against the real server: `RUNEFIN_PROBE_ITEM` a signed-in item and
    /// `RUNEFIN_PROBE_SRT` its text track, with `RUNEFIN_PROBE_START_MS` the window to listen to.
    /// It decodes through the app's own probe (not ffmpeg, whose `-ss` on a long-GOP file seeks
    /// to a cluster and shifts the window by tens of seconds), then prints the alignment so a
    /// track the server considers in sync can be read as a number rather than "it looked right".
    #[test]
    #[ignore = "needs RUNEFIN_PROBE_ITEM=<id> RUNEFIN_PROBE_SRT=<srt> [RUNEFIN_PROBE_START_MS]"]
    fn a_real_track_lines_up_with_real_audio() {
        let item_id = std::env::var("RUNEFIN_PROBE_ITEM").expect("RUNEFIN_PROBE_ITEM");
        let srt_path = std::env::var("RUNEFIN_PROBE_SRT").expect("RUNEFIN_PROBE_SRT");
        let config = crate::config::Config::load();
        assert!(config.has_session(), "sign in once before this test");
        let client = jellyfin_api::Client::new(&config.server_url, config.device_id.clone())
            .with_session(config.token.clone(), config.user_id.clone());
        let url = client
            .direct_stream_url(&item_id)
            .expect("the item has a stream");
        let start = std::env::var("RUNEFIN_PROBE_START_MS")
            .ok()
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0);
        let progress = AtomicU64::new(0);
        let cancelled = AtomicBool::new(false);
        let window = sample(&url, start, 240, None, &progress, &cancelled)
            .expect("the probe decodes the window");
        let cues: Vec<(i64, i64)> = crate::subtitles::parse(&std::fs::read(srt_path).expect("srt"))
            .iter()
            .map(|cue| (cue.start_ms, cue.end_ms))
            .collect();
        if let Ok(dump) = std::env::var("RUNEFIN_PROBE_DUMP") {
            let data_len = window.samples.len() as u32 * 2;
            let mut out = Vec::new();
            out.extend_from_slice(b"RIFF");
            out.extend_from_slice(&(36 + data_len).to_le_bytes());
            out.extend_from_slice(b"WAVEfmt ");
            out.extend_from_slice(&16u32.to_le_bytes());
            out.extend_from_slice(&1u16.to_le_bytes());
            out.extend_from_slice(&1u16.to_le_bytes());
            out.extend_from_slice(&RATE.to_le_bytes());
            out.extend_from_slice(&(RATE * 2).to_le_bytes());
            out.extend_from_slice(&2u16.to_le_bytes());
            out.extend_from_slice(&16u16.to_le_bytes());
            out.extend_from_slice(b"data");
            out.extend_from_slice(&data_len.to_le_bytes());
            for sample in &window.samples {
                out.extend_from_slice(&sample.to_le_bytes());
            }
            std::fs::write(dump, out).expect("dump written");
        }
        let searched = subtitle_sync::search(&window.samples, RATE, start, &cues);
        eprintln!(
            "real track from {start} ms: {:?} ({} cues, {} s decoded)",
            searched.map(|(a, mean)| (a.lag_ms, a.confidence, a.contrast, mean)),
            cues.len(),
            window.samples.len() / RATE as usize
        );
        let mut confident = Vec::new();
        for extra in [0i64, 240_000] {
            if extra > 0 {
                continue;
            }
            if let Some(alignment) =
                subtitle_sync::align(&window.samples, RATE, window.start_ms, &cues)
            {
                confident.push(alignment);
            }
        }
        let found =
            subtitle_sync::settle(confident).expect("a real track should align with its own audio");
        assert!(
            found.lag_ms.abs() < 1_000,
            "the server's own track drifted {} ms",
            found.lag_ms
        );
    }

    const RATE: u32 = 8_000;

    /// A 16-bit mono WAV of speech-like bursts at the given item-time intervals.
    fn wav(seconds: i64, bursts: &[(i64, i64)]) -> Vec<u8> {
        let mut samples = vec![0i16; (seconds * RATE as i64) as usize];
        for &(from, to) in bursts {
            let first = ((from * RATE as i64 / 1000).max(0) as usize).min(samples.len());
            let last = ((to * RATE as i64 / 1000) as usize).min(samples.len());
            if first >= last {
                continue;
            }
            for (offset, sample) in samples[first..last].iter_mut().enumerate() {
                let t = (first + offset) as f64 / f64::from(RATE);
                let voice = (t * 330.0 * std::f64::consts::TAU).sin()
                    + 0.6 * (t * 990.0 * std::f64::consts::TAU).sin()
                    + 0.3 * (t * 1_980.0 * std::f64::consts::TAU).sin();
                let wobble = 1.0 + 0.35 * (t * 30.0).sin();
                *sample = (voice * wobble * 3_000.0) as i16;
            }
        }
        let data_len = samples.len() as u32 * 2;
        let mut out = Vec::with_capacity(44 + data_len as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&1u16.to_le_bytes()); // mono
        out.extend_from_slice(&RATE.to_le_bytes());
        out.extend_from_slice(&(RATE * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for sample in &samples {
            out.extend_from_slice(&sample.to_le_bytes());
        }
        out
    }

    fn dialogue() -> Vec<(i64, i64)> {
        let gaps = [900, 1_400, 700, 2_100, 1_100, 1_800, 600, 1_300, 2_400, 800];
        let lengths = [520, 780, 430, 1_100, 610, 900, 380, 700, 1_200, 460];
        let mut at = 3_000;
        let mut out = Vec::new();
        for i in 0..40 {
            let length = lengths[i % lengths.len()];
            out.push((at, at + length));
            at += length + gaps[i % gaps.len()];
            if at > 45_000 {
                break;
            }
        }
        out
    }

    #[test]
    fn a_window_decodes_and_aligns_against_a_shifted_track() {
        // End to end without a server: a WAV, the real probe pipeline, the real aligner.
        let bursts = dialogue();
        let path =
            std::env::temp_dir().join(format!("runefin-audio-probe-{}.wav", std::process::id()));
        std::fs::write(&path, wav(48, &bursts)).expect("wav written");
        let url = format!("file://{}", path.display());
        let progress = AtomicU64::new(0);
        let cancelled = AtomicBool::new(false);
        let window = match sample(&url, 0, 40, None, &progress, &cancelled) {
            Ok(window) => window,
            Err(error) => {
                let _ = std::fs::remove_file(&path);
                panic!("the probe failed: {error}");
            }
        };
        let _ = std::fs::remove_file(&path);
        assert!(
            window.samples.len() > 30 * RATE as usize,
            "decoded {} samples",
            window.samples.len()
        );
        assert!(progress.load(Ordering::Relaxed) >= 30);
        // Cues that run 1.5 s early: the aligner should ask for +1.5 s.
        let cues: Vec<(i64, i64)> = bursts.iter().map(|(a, b)| (a - 1_500, b - 1_500)).collect();
        let found = subtitle_sync::align(&window.samples, RATE, window.start_ms, &cues)
            .expect("an alignment from the decoded window");
        assert!(
            (found.lag_ms - 1_500).abs() <= 100,
            "expected about +1500 ms, got {}",
            found.lag_ms
        );
    }

    #[test]
    fn a_window_from_the_middle_keeps_item_time() {
        let bursts = dialogue();
        let path = std::env::temp_dir().join(format!(
            "runefin-audio-probe-mid-{}.wav",
            std::process::id()
        ));
        std::fs::write(&path, wav(48, &bursts)).expect("wav written");
        let url = format!("file://{}", path.display());
        let progress = AtomicU64::new(0);
        let cancelled = AtomicBool::new(false);
        // Start at 20 s: the cues are in item time, so the window's own start must be added back.
        let window =
            sample(&url, 20_000, 20, None, &progress, &cancelled).expect("the probe decodes");
        let _ = std::fs::remove_file(&path);
        let cues: Vec<(i64, i64)> = bursts.iter().map(|(a, b)| (a + 800, b + 800)).collect();
        let found = subtitle_sync::align(&window.samples, RATE, window.start_ms, &cues)
            .expect("an alignment");
        assert!(
            (found.lag_ms + 800).abs() <= 100,
            "expected about -800 ms, got {}",
            found.lag_ms
        );
    }
}
