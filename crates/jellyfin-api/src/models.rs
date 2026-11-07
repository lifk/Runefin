//! Jellyfin's wire types, as few as possible.
//!
//! The API answers PascalCase JSON (`"Name"`, `"Id"`, `"UserData"`), so every struct renames
//! accordingly. Unknown fields are ignored: a server upgrade must never break the app, and the
//! fields we do not read (People, MediaStreams, Trickplay) are the ones most likely to change.
//!
//! Deliberately absent: transcoding profiles and playback negotiation. The in-app player reads
//! media sources only for the subtitle streams it renders itself; GStreamer owns the rest.

use serde::{Deserialize, Serialize};

/// Jellyfin counts 10,000 ticks per millisecond.
pub const TICKS_PER_MS: i64 = 10_000;

/// One item's server-side watch state. The resume position lives here.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct UserData {
    pub playback_position_ticks: i64,
    pub play_count: i32,
    pub is_favorite: bool,
    /// `true` liked, `false` disliked, `None` no rating.
    #[serde(default)]
    pub likes: Option<bool>,
    pub played: bool,
    pub last_played_date: Option<String>,
    /// Series only: whether a next-up episode exists.
    pub unplayed_item_count: Option<i32>,
    pub played_percentage: Option<f64>,
}

impl UserData {
    /// Position in milliseconds, or `None` when the item has never started.
    pub fn position_ms(&self) -> Option<i64> {
        (self.playback_position_ticks > 0).then_some(self.playback_position_ticks / TICKS_PER_MS)
    }

    /// True when the item is started and not finished: what Continue Watching filters on.
    pub fn is_resumable(&self) -> bool {
        self.position_ms().is_some() && !self.played
    }
}

/// A person credit on an item. The server sends these only when the item query asks for
/// `People`, which the detail request now does.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub role: Option<String>,
    #[serde(rename = "Type")]
    pub kind: Option<String>,
    pub primary_image_tag: Option<String>,
}

/// One file behind an item, with the streams the player can choose from.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MediaSource {
    pub id: String,
    /// Server-provided display name; often a filename or a version label.
    pub name: Option<String>,
    pub container: Option<String>,
    /// File size in bytes.
    pub size: Option<i64>,
    /// Stream bitrate in bits per second.
    pub bitrate: Option<i64>,
    pub media_streams: Vec<MediaStream>,
}

/// One audio, video or subtitle stream. Subtitle indexes are what the subtitle endpoint takes.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MediaStream {
    pub index: i32,
    #[serde(rename = "Type")]
    pub kind: String,
    pub codec: Option<String>,
    pub language: Option<String>,
    pub display_title: Option<String>,
    pub is_external: bool,
    pub is_default: bool,
    pub is_forced: bool,
    /// The server can convert this stream to SRT/VTT; image subtitles cannot be rendered as text.
    pub is_text_subtitle_stream: bool,
    pub channels: Option<i32>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    /// `SDR`, `HDR10`, `HLG`, `DolbyVision`, …
    pub video_range: Option<String>,
    pub profile: Option<String>,
}

/// One playable audio stream. `ordinal` is the index GStreamer's `current-audio` expects.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AudioTrack {
    pub ordinal: i32,
    pub display_title: String,
    pub language: Option<String>,
    pub codec: Option<String>,
    pub channels: Option<i32>,
}

/// A subtitle the app can fetch and draw in Rune. `index` is the server's stream index.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SubtitleTrack {
    pub index: i32,
    pub language: Option<String>,
    pub display_title: String,
    /// `srt`, `ass`, `subrip`, … The player asks the server for ASS tracks as ASS so it can
    /// keep their alignment; everything else is read as SRT.
    pub codec: Option<String>,
    /// False for bitmap subtitles (PGS/VobSub/DVB): the server can serve them, but the overlay
    /// draws text, so they are listed and explained rather than selected.
    pub is_text: bool,
    pub is_external: bool,
    pub is_forced: bool,
}

/// A named server object: a studio, genre or tag with an id when one exists.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct NameGuid {
    pub id: String,
    pub name: String,
}

/// A library item: movie, series, season, episode, collection folder.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Item {
    pub id: String,
    pub name: String,
    /// `Movie`, `Series`, `Season`, `Episode`, `BoxSet`, `CollectionFolder`, `Person`, …
    #[serde(rename = "Type")]
    pub kind: String,
    pub collection_type: Option<String>,
    pub overview: Option<String>,
    pub original_title: Option<String>,
    pub tagline: Option<String>,
    /// ISO 8601 release date, when the server has one.
    pub premiere_date: Option<String>,
    /// `Continuing`, `Ended`, … for a series.
    pub status: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub studios: Vec<NameGuid>,
    pub production_year: Option<i32>,
    /// ISO 8601 timestamp; the library's "NEW" badge reads its date part.
    pub date_created: Option<String>,
    pub run_time_ticks: Option<i64>,
    pub user_data: Option<UserData>,
    /// `{"Primary": "<hash>"}`: the hash is the image tag, part of every image URL.
    pub image_tags: Option<std::collections::HashMap<String, String>>,
    /// Only on a playlist entry: the id to remove that entry with. It is not the item id when
    /// the same film appears twice, so removals must use this field.
    pub playlist_item_id: Option<String>,
    pub backdrop_image_tags: Option<Vec<String>>,
    pub series_name: Option<String>,
    pub series_id: Option<String>,
    /// The series poster's tag, so an episode card can show the series poster rather than a
    /// still. Comes from `/Items` and `/UserItems/Resume` with `fields=UserData`.
    pub series_primary_image_tag: Option<String>,
    pub season_id: Option<String>,
    pub index_number: Option<i32>,
    pub parent_index_number: Option<i32>,
    pub community_rating: Option<f64>,
    pub critic_rating: Option<f64>,
    /// `BehindTheScenes`, `DeletedScene`, … on a special feature.
    pub extra_type: Option<String>,
    /// `{"Imdb": "tt…", "Tmdb": "…", "Tvdb": "…"}`.
    #[serde(default)]
    pub provider_ids: std::collections::HashMap<String, String>,
    pub official_rating: Option<String>,
    pub genres: Option<Vec<String>>,
    pub primary_image_aspect_ratio: Option<f64>,
    /// Episodes: when the server thinks this is the one to watch next.
    pub is_virtual_unaired: Option<bool>,
    /// Cast and crew, when the request includes `fields=People`.
    pub people: Vec<Person>,
    /// Chapter starts, when the request includes `fields=Chapters`.
    pub chapters: Vec<ChapterInfo>,
    /// Trickplay manifests when `fields=Trickplay` was asked. Jellyfin keys the outer map by
    /// item id and the inner one by thumbnail width, even for a single item.
    pub trickplay:
        Option<std::collections::HashMap<String, std::collections::HashMap<String, TrickplayInfo>>>,
    /// Files and streams, when the request includes `fields=MediaSources`.
    pub media_sources: Vec<MediaSource>,
    /// `TvChannel`: the dial number the server or tuner assigned.
    pub channel_number: Option<String>,
    /// Trailers hosted by a provider (Jellyfin stores YouTube URLs here).
    #[serde(default)]
    pub remote_trailers: Vec<RemoteTrailer>,
    /// `TvChannel`: what is on now, when the guide has it.
    pub current_program: Option<CurrentProgram>,
    /// A guide program's channel, start and end. Timestamps are ISO-8601 UTC strings.
    pub channel_id: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

/// A trailer URL and the provider's label (`Official Trailer`, `Teaser`, …).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct RemoteTrailer {
    pub url: Option<String>,
    pub name: Option<String>,
}

/// The guide entry a live channel carries as `CurrentProgram`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct CurrentProgram {
    pub name: String,
    pub overview: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct TrickplayInfo {
    pub width: u32,
    pub height: u32,
    pub tile_width: u32,
    pub tile_height: u32,
    pub thumbnail_count: u32,
    /// Milliseconds between thumbnails.
    pub interval: u32,
    pub bandwidth: u32,
}

impl TrickplayInfo {
    /// The thumbnail covering `ms`, clamped to the sprites the server says exist.
    pub fn thumbnail_at(&self, ms: i64) -> u32 {
        let index = (ms.max(0) as u64 / u64::from(self.interval.max(1))) as u32;
        index.min(self.thumbnail_count.saturating_sub(1))
    }

    /// How many thumbnails one sprite tile holds.
    pub fn thumbnails_per_tile(&self) -> u32 {
        self.tile_width.max(1) * self.tile_height.max(1)
    }

    /// The sprite index that holds `thumbnail`.
    pub fn tile_of(&self, thumbnail: u32) -> u32 {
        thumbnail / self.thumbnails_per_tile()
    }

    /// The column and row of `thumbnail` inside its own sprite. Row is relative to the tile:
    /// using the absolute thumbnail number here would point past the sprite after tile 0.
    pub fn cell_of(&self, thumbnail: u32) -> (u32, u32) {
        let columns = self.tile_width.max(1);
        let inside = thumbnail % self.thumbnails_per_tile();
        (inside % columns, inside / columns)
    }
}

impl Item {
    /// The trickplay manifest to sample: the 640 px set when the server generated it,
    /// otherwise the widest one it has, so the choice never depends on map order.
    pub fn trickplay_manifest(&self) -> Option<(u32, &TrickplayInfo)> {
        let sets = self.trickplay.as_ref()?;
        let mut sets: Vec<(u32, &TrickplayInfo)> = sets
            .values()
            .flat_map(|widths| widths.iter())
            .filter_map(|(key, info)| Some((key.parse().ok()?, info)))
            .collect();
        sets.sort_by_key(|(width, _)| *width);
        sets.iter()
            .find(|(width, _)| *width == 640)
            .or_else(|| sets.last())
            .map(|(width, info)| (*width, *info))
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ChapterInfo {
    pub start_position_ticks: i64,
    pub name: String,
    pub image_tag: Option<String>,
}

impl ChapterInfo {
    pub fn start_ms(&self) -> i64 {
        self.start_position_ticks / TICKS_PER_MS
    }
}

impl Item {
    /// The chapter the playhead is inside: the last one that started at or before `ms`.
    pub fn chapter_at(&self, ms: i64) -> Option<&ChapterInfo> {
        self.chapters
            .iter()
            .rev()
            .find(|chapter| chapter.start_ms() <= ms)
    }

    pub fn runtime_ms(&self) -> Option<i64> {
        self.run_time_ticks.map(|t| t / TICKS_PER_MS)
    }

    /// The file the player opens: the first media source, which is the one Jellyfin marks default.
    pub fn primary_media_source(&self) -> Option<&MediaSource> {
        self.media_sources.first()
    }

    /// Audio streams in the order playbin indexes them.
    pub fn audio_tracks(&self) -> Vec<AudioTrack> {
        let Some(source) = self.primary_media_source() else {
            return Vec::new();
        };
        source
            .media_streams
            .iter()
            .filter(|stream| stream.kind == "Audio")
            .enumerate()
            .map(|(ordinal, stream)| AudioTrack {
                ordinal: ordinal as i32,
                display_title: stream
                    .display_title
                    .clone()
                    .filter(|title| !title.trim().is_empty())
                    .or_else(|| stream.language.as_ref().map(|lang| lang.to_uppercase()))
                    .unwrap_or_else(|| format!("Audio {}", ordinal + 1)),
                language: stream.language.clone(),
                codec: stream.codec.clone(),
                channels: stream.channels,
            })
            .collect()
    }

    /// Text subtitles only: image-based streams (PGS, VobSub) cannot be drawn by the app.
    pub fn subtitle_tracks(&self) -> Vec<SubtitleTrack> {
        let Some(source) = self.primary_media_source() else {
            return Vec::new();
        };
        source
            .media_streams
            .iter()
            .filter(|stream| stream.kind == "Subtitle")
            .map(|stream| SubtitleTrack {
                index: stream.index,
                codec: stream.codec.clone(),
                is_text: stream.is_text_subtitle_stream,
                language: stream.language.clone(),
                display_title: stream
                    .display_title
                    .clone()
                    .filter(|title| !title.trim().is_empty())
                    .or_else(|| stream.language.as_ref().map(|lang| lang.to_uppercase()))
                    .unwrap_or_else(|| format!("Subtitle {}", stream.index + 1)),
                is_external: stream.is_external,
                is_forced: stream.is_forced,
            })
            .collect()
    }

    /// The image tag for `kind` (`Primary`, `Backdrop`, `Thumb`, `Logo`), when the item has one.
    /// Passed to [`crate::Client::image_url`] so the server can mark the URL cacheable forever.
    pub fn image_tag(&self, kind: ImageKind) -> Option<&str> {
        match kind {
            ImageKind::Primary => self.image_tags.as_ref()?.get("Primary").map(String::as_str),
            ImageKind::Backdrop => self
                .backdrop_image_tags
                .as_ref()?
                .first()
                .map(String::as_str),
            ImageKind::Thumb => self.image_tags.as_ref()?.get("Thumb").map(String::as_str),
            ImageKind::Logo => self.image_tags.as_ref()?.get("Logo").map(String::as_str),
        }
    }

    pub fn is_live_channel(&self) -> bool {
        self.kind == "TvChannel"
    }

    pub fn video_stream(&self) -> Option<&MediaStream> {
        self.primary_media_source()?
            .media_streams
            .iter()
            .find(|stream| stream.kind == "Video")
    }

    pub fn audio_stream(&self) -> Option<&MediaStream> {
        self.primary_media_source()?
            .media_streams
            .iter()
            .find(|stream| stream.kind == "Audio")
    }

    pub fn is_folder(&self) -> bool {
        matches!(
            self.kind.as_str(),
            "Series" | "Season" | "BoxSet" | "CollectionFolder"
        )
    }

    /// `S2E07 · 48 min`, the line a card shows under a series episode.
    pub fn episode_label(&self) -> Option<String> {
        let s = self.parent_index_number?;
        let e = self.index_number?;
        Some(format!("S{s}E{e:02}"))
    }
}

/// One intro, outro, recap or preview span reported by `/MediaSegments/{itemId}`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MediaSegment {
    pub id: String,
    pub item_id: String,
    #[serde(rename = "Type")]
    pub kind: String,
    pub start_ticks: i64,
    pub end_ticks: i64,
}

impl MediaSegment {
    pub fn start_ms(&self) -> i64 {
        self.start_ticks / TICKS_PER_MS
    }

    pub fn end_ms(&self) -> i64 {
        self.end_ticks / TICKS_PER_MS
    }
}

/// One online subtitle a provider offers for an item. `/RemoteSearch/Subtitles` returns these
/// from every configured provider (Open Subtitles here); the id is the provider's handle for
/// the download call, not something the server stores.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct RemoteSubtitle {
    pub id: String,
    pub name: String,
    pub provider_name: Option<String>,
    pub format: Option<String>,
    pub author: Option<String>,
    pub comment: Option<String>,
    pub download_count: Option<i64>,
    pub community_rating: Option<f64>,
    #[serde(rename = "ThreeLetterISOLanguageName")]
    pub language: Option<String>,
    pub is_hash_match: bool,
    pub forced: bool,
    pub hearing_impaired: bool,
    pub ai_translated: bool,
    pub machine_translated: bool,
    pub frame_rate: Option<f64>,
}

impl RemoteSubtitle {
    /// `Open Subtitles · 64k downloads · ★ 3.2 · hash match`: the facts that decide which
    /// result to take. Only what the provider actually sent, so no empty separators.
    pub fn detail_line(&self) -> String {
        let mut parts = Vec::new();
        if let Some(provider) = self.provider_name.as_deref().filter(|p| !p.is_empty()) {
            parts.push(provider.to_string());
        }
        if let Some(count) = self.download_count.filter(|count| *count > 0) {
            parts.push(if count >= 1_000 {
                format!("{}k downloads", count / 1_000)
            } else {
                format!("{count} downloads")
            });
        }
        if let Some(rating) = self.community_rating.filter(|rating| *rating > 0.0) {
            parts.push(format!("★ {rating:.1}"));
        }
        if self.is_hash_match {
            parts.push("exact file".into());
        }
        if self.forced {
            parts.push("forced".into());
        }
        if self.hearing_impaired {
            parts.push("SDH".into());
        }
        if let Some(format) = self.format.as_deref().filter(|format| !format.is_empty()) {
            parts.push(format.to_uppercase());
        }
        parts.join(" · ")
    }

    /// The line under the title: `tedi · Retail NF`, when the provider says who made it.
    pub fn credit_line(&self) -> Option<String> {
        let author = self.author.as_deref().filter(|author| !author.is_empty());
        let comment = self
            .comment
            .as_deref()
            .filter(|comment| !comment.is_empty());
        match (author, comment) {
            (Some(author), Some(comment)) => Some(format!("{author} · {comment}")),
            (Some(author), None) => Some(author.to_string()),
            (None, Some(comment)) => Some(comment.to_string()),
            (None, None) => None,
        }
    }
}

/// One `/Search/Hints` row: a name the query matched, with its kind and id.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct SearchHint {
    pub id: String,
    pub name: String,
    /// `Movie`, `Series`, `Episode`, `Person`, `Genre`, `Studio`, …
    #[serde(rename = "Type")]
    pub kind: String,
}

/// `/Items/{id}/SpecialFeatures` and `/Items/{id}/ThemeMedia` containers.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ItemsContainer {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ThemeMedia {
    pub theme_videos_result: Option<ItemsContainer>,
    pub theme_songs_result: Option<ItemsContainer>,
    pub soundtrack_songs_result: Option<ItemsContainer>,
}

/// One page of items.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ItemsResult {
    pub items: Vec<Item>,
    pub total_record_count: i32,
    pub start_index: i32,
}

/// A top-level library, as the server advertises it (`/Users/{id}/Views`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct LibraryView {
    pub id: String,
    pub name: String,
    /// `movies`, `tvshows`, `music`, `boxsets`, `playlists`; `None` for a bare folder.
    pub collection_type: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct User {
    pub id: String,
    pub name: String,
    pub primary_image_tag: Option<String>,
}

/// What `POST /Users/AuthenticateByName` returns.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct AuthResult {
    pub user: User,
    pub access_token: String,
    pub server_id: String,
}

/// One session from `GET /Sessions`, used by the M6 remote-control screen.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct SessionInfo {
    pub id: String,
    pub user_id: String,
    pub user_name: String,
    pub client: String,
    pub device_name: String,
    pub device_id: String,
    pub application_version: String,
    pub is_active: bool,
    pub now_playing_item: Option<Item>,
    pub play_state: Option<PlayerState>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct PlayerState {
    pub position_ticks: i64,
    pub is_paused: bool,
    pub can_seek: bool,
}

/// `GET /System/Info/Public`: the pre-login handshake (server name for the connect screen).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct PublicSystemInfo {
    pub server_name: String,
    pub version: String,
    pub id: String,
}

/// `POST /QuickConnect/Initiate` and `GET /QuickConnect/Connect`. The user types `code` into the
/// server's own web UI; `secret` is what this client polls with, so no password enters the app.
///
/// Deliberately no `AccessToken` here: `QuickConnectResult` has none. Once `authenticated` flips,
/// the token comes from [`crate::Client::authenticate_with_quick_connect`].
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct QuickConnectState {
    pub secret: String,
    pub code: String,
    pub authenticated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Primary,
    Backdrop,
    Thumb,
    Logo,
}

impl ImageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ImageKind::Primary => "Primary",
            ImageKind::Backdrop => "Backdrop",
            ImageKind::Thumb => "Thumb",
            ImageKind::Logo => "Logo",
        }
    }
}

/// Sort keys the server understands; the strings are what `/Items` expects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    SortName,
    DateCreated,
    CommunityRating,
    DatePlayed,
    PremiereDate,
    Runtime,
    PlayCount,
    Random,
}

impl SortBy {
    pub fn as_str(self) -> &'static str {
        match self {
            SortBy::SortName => "SortName",
            SortBy::DateCreated => "DateCreated",
            SortBy::CommunityRating => "CommunityRating",
            SortBy::DatePlayed => "DatePlayed",
            SortBy::PremiereDate => "PremiereDate",
            SortBy::Runtime => "Runtime",
            SortBy::PlayCount => "PlayCount",
            SortBy::Random => "Random",
        }
    }
}

/// `POST /Items/{id}/PlaybackInfo`: the server's answer, not a catalogue item.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct PlaybackInfoResponse {
    pub media_sources: Vec<PlaybackMediaSource>,
    /// The id every playback report must echo for this negotiation.
    pub play_session_id: Option<String>,
}

/// One stream the server is willing to serve for a PlaybackInfo request.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct PlaybackMediaSource {
    pub id: String,
    pub name: Option<String>,
    pub container: Option<String>,
    pub size: Option<i64>,
    pub bitrate: Option<i64>,
    pub media_streams: Vec<MediaStream>,
    pub supports_direct_play: bool,
    pub supports_direct_stream: bool,
    pub supports_transcoding: bool,
    /// Relative to the server base; `None` for a pure direct play.
    pub direct_stream_url: Option<String>,
    /// Relative to the server base; present when the server chose to transcode.
    pub transcoding_url: Option<String>,
    pub transcoding_sub_protocol: Option<String>,
    pub transcoding_container: Option<String>,
    pub play_session_id: Option<String>,
}

/// Body of `POST /Sessions/Playing` and `/Sessions/Playing/Progress`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlaybackReport {
    pub item_id: String,
    pub position_ticks: i64,
    pub is_paused: bool,
    pub can_seek: bool,
    /// `DirectPlay`, `DirectStream` or `Transcode`. mpv plays the original file, so `DirectPlay`.
    pub play_method: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub play_session_id: Option<String>,
}

/// Body of `POST /Sessions/Playing/Stopped`. A different payload on purpose: the server's
/// `PlaybackStopInfo` has no `IsPaused`/`CanSeek`, and the final position is the whole point.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlaybackStop {
    pub item_id: String,
    pub position_ticks: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub play_session_id: Option<String>,
    /// True when the player died rather than finishing; keeps a crash out of the resume state.
    pub failed: bool,
}

impl PlaybackStop {
    pub fn new(item_id: impl Into<String>, position_ms: i64) -> Self {
        Self {
            item_id: item_id.into(),
            position_ticks: position_ms.saturating_mul(TICKS_PER_MS),
            play_session_id: None,
            failed: false,
        }
    }
}

impl PlaybackReport {
    pub fn direct_play(item_id: impl Into<String>, position_ms: i64, paused: bool) -> Self {
        Self {
            item_id: item_id.into(),
            position_ticks: position_ms.saturating_mul(TICKS_PER_MS),
            is_paused: paused,
            can_seek: true,
            play_method: "DirectPlay",
            play_session_id: None,
        }
    }
}

#[cfg(test)]
mod tests;

/// Seconds since the Unix epoch from Jellyfin's ISO-8601 UTC timestamps
/// (`2026-11-03T22:00:00.0000000Z`). Fractional seconds are ignored.
pub fn iso_seconds(value: &str) -> Option<i64> {
    let bytes = value.as_bytes();
    if bytes.len() < 19 || bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'T' {
        return None;
    }
    let number = |range: std::ops::Range<usize>| -> Option<i64> {
        std::str::from_utf8(&bytes[range]).ok()?.parse().ok()
    };
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// The inverse of [`iso_seconds`], for the live guide's date-range query.
pub fn iso_from_seconds(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        of_day / 3_600,
        (of_day % 3_600) / 60,
        of_day % 60
    )
}

/// Howard Hinnant's days-from-civil, valid for the whole proleptic Gregorian range.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (year + i64::from(month <= 2), month, day)
}
