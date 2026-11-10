//! App state. Screens are pure functions of this struct, so everything a screen shows is
//! assertable in a headless test without a server.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use jellyfin_api::{AuthResult, Client, ImageKind, Item, MediaSegment, QuickConnectState};
use rune::{Image, ImageCache, Latest, Pixels, TextEdit};
use seerr_api::{Details, RequestCounts, SearchResults};

use crate::config::{
    Config, LiveReminder, SubtitleBackground, SubtitleColor, SubtitleFont, SubtitleMode,
    SubtitleOutline, SubtitlePosition, SubtitleStyle,
};
use crate::detail::{DetailPage, DetailState, EpisodesPage};
use crate::handoff::PlaybackHandle;
use crate::home::{HomeData, HomeLoad};
use crate::library::{LibraryPage, LibraryRequest, LibraryState, SortMode, ViewMode};
use crate::net::{Failure, Msg, ReportKind, Session, Stage, Worker};
use crate::search::SearchLoad;
use crate::shortcuts::{Action as ShortcutAction, KeyBindings};

/// The width posters are fetched at: a 180 pt card at 2×.
const POSTER_WIDTH: u32 = 360;
/// A detail page's poster is wider than a card's; the backdrop fills the window.
const DETAIL_POSTER_WIDTH: u32 = 520;
const BACKDROP_WIDTH: u32 = 1600;
/// The home ambient is one image, so it stays small enough to decode quickly and evict.
const HOME_BACKDROP_WIDTH: u32 = 1280;
/// Decoded posters held at once; evicting one drops its handles and frees its pixels.
const IMAGE_CACHE_LIMIT: usize = 256;

mod card_menu_state;
mod detail_state;
mod idle;
mod image_state;
mod in_app_state;
pub use in_app_state::is_playable;
#[cfg(test)]
pub use live_state::live_zap_index;
mod live_state;
mod mpris_state;
mod person_state;
mod playback_state;
mod quick_state;
mod remote_state;
mod search_state;
mod seerr_state;

pub use card_menu_state::CardMenu;
pub use in_app_state::InAppSession;
use in_app_state::{PendingInApp, PlaybackStreams, SubtitleCues};
pub use live_state::LiveState;
pub use person_state::PersonPage;
use person_state::{PersonBundle, PersonLoad};
use playback_state::PendingPlay;
pub use playback_state::PlaybackSession;
pub use quick_state::QuickLogin;
use quick_state::QuickStart;
use remote_state::RemoteState;
use search_state::SearchState;
pub use seerr_state::{
    DiscoverFilters, DiscoverKind, DiscoverMode, DiscoverSort, NETWORKS, RequestsFilter, STUDIOS,
    SeerrDetailState, SeerrDiscover, SeerrProbe, SeerrState, SeerrTab, missing_seasons,
};

/// The screen the shell is showing. The full `(route, scroll, focus)` stack lands with M4's
/// shared-element detail; M3 only needs Home and one open library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Home,
    Library {
        id: String,
    },
    Detail {
        id: String,
    },
    Person {
        id: String,
    },
    Search {
        scope: Option<String>,
    },
    Remote,
    LiveTv,
    Settings,
    Seerr,
    SeerrDetail {
        media_type: String,
        tmdb_id: u64,
    },
    /// A person opened from a Seerr cast rail. The server's person is resolved by name.
    SeerrPerson {
        tmdb_id: u64,
    },
}

/// Parse `mm:ss`, `h:mm:ss` or plain seconds. No locale-dependent input.
fn parse_timecode(text: &str) -> Option<i64> {
    let parts: Vec<&str> = text.split(':').collect();
    match parts.as_slice() {
        [seconds] => seconds.parse::<i64>().ok().map(|seconds| seconds * 1_000),
        [minutes, seconds] => {
            let minutes = minutes.parse::<i64>().ok()?;
            let seconds = seconds.parse::<i64>().ok()?;
            (0..60)
                .contains(&seconds)
                .then_some((minutes * 60 + seconds) * 1_000)
        }
        [hours, minutes, seconds] => {
            let hours = hours.parse::<i64>().ok()?;
            let minutes = minutes.parse::<i64>().ok()?;
            let seconds = seconds.parse::<i64>().ok()?;
            ((0..60).contains(&minutes) && (0..60).contains(&seconds))
                .then_some(((hours * 60 + minutes) * 60 + seconds) * 1_000)
        }
        _ => None,
    }
}

/// The result of the player worker: a negotiated method and the session id to report.
pub struct InAppStart {
    pub player: crate::player::Player,
    /// `DirectPlay`, `DirectStream` or `Transcode`.
    pub method: &'static str,
    pub play_session_id: Option<String>,
    /// A short explanation when PlaybackInfo failed and the static stream was used.
    pub notice: Option<String>,
}

/// A "See all" grid over one of Home's dynamic rails. It is a local view, not a server library,
/// so it never changes the route: closing it returns to the same Home scroll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeShelf {
    ContinueWatching,
    NextUp,
    RecentlyPlayed,
}

impl HomeShelf {
    pub fn title(self) -> &'static str {
        match self {
            HomeShelf::ContinueWatching => "Continue Watching",
            HomeShelf::NextUp => "Next Up",
            HomeShelf::RecentlyPlayed => "Recently Played",
        }
    }
}

/// One reversible change behind a toast. The action is data, not a closure, so it survives a
/// frame without borrowing `State`.
#[derive(Debug, Clone)]
pub enum UndoAction {
    Favorite {
        item_id: String,
        value: bool,
    },
    Played {
        item_id: String,
        value: bool,
        /// The Continue Watching entry the action removed, if it removed one.
        resume: Option<Item>,
    },
    Resume {
        item: Item,
        position_ticks: i64,
        percentage: Option<f64>,
    },
    Seek {
        position_ms: i64,
    },
    /// A playlist entry removed from the page. The item and its old index are enough to put
    /// the card back; the server gets the same item re-added.
    PlaylistRemove {
        playlist_id: String,
        item: Item,
        index: usize,
    },
    /// A move already applied optimistically; Undo sends the opposite move.
    PlaylistMove {
        playlist_id: String,
        entry_id: String,
        from: usize,
        to: usize,
    },
    /// An item added to a playlist. Undo finds the entry by item id and removes it.
    PlaylistAdd {
        playlist_id: String,
        item_id: String,
    },
    NotInterested {
        item_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Waiting for input.
    Idle,
    /// A probe + sign-in is in flight.
    SigningIn,
    /// Signed in; `server_name` is what the connect screen thanked us with.
    Ready {
        server_name: String,
    },
    Failed(Failure),
}

/// The online subtitle lookup shown inside the player's subtitle panel. Results come from the
/// server's configured providers (Open Subtitles on the live box); a download attaches the file
/// to the item, so the track list is re-read and the new track selected.
pub struct SubtitleSearch {
    pub open: bool,
    /// Three-letter code the provider expects (`eng`, `spa`, …).
    pub language: String,
    pub loading: bool,
    pub results: Vec<jellyfin_api::RemoteSubtitle>,
    pub error: Option<String>,
    /// The provider id currently downloading, so its row can say so.
    pub downloading: Option<String>,
}

impl Default for SubtitleSearch {
    fn default() -> Self {
        Self {
            open: false,
            language: "eng".into(),
            loading: false,
            results: Vec::new(),
            error: None,
            downloading: None,
        }
    }
}

/// The next language in the preference cycle, wrapping to "Any".
fn next_language(current: &str) -> String {
    let index = State::PREFERRED_LANGUAGES
        .iter()
        .position(|(code, _)| *code == current)
        .unwrap_or(0);
    let next = (index + 1) % State::PREFERRED_LANGUAGES.len();
    State::PREFERRED_LANGUAGES[next].0.to_string()
}

/// The player's own subtitle overlay: which text track, its cues, and the reader's delay and
/// size. Session-local on purpose; nothing here is written to disk or the server.
pub struct Subtitles {
    pub tracks: Vec<jellyfin_api::SubtitleTrack>,
    /// The media source the tracks came from, for the subtitle URL.
    pub source: Option<String>,
    /// Index into `tracks`, not the server's stream index.
    pub selected: Option<usize>,
    pub cues: Vec<crate::subtitles::Cue>,
    pub delay_ms: i64,
    pub size: f32,
    pub loading: bool,
    pub error: Option<String>,
    /// A selected PGS/VobSub track and its current RGBA rectangles.
    pub image: Option<crate::image_subtitles::ImageSubtitle>,
    pub image_loading: bool,
    pub image_error: Option<String>,
    /// The track list / delay / size panel above the transport controls.
    pub open: bool,
}

impl Default for Subtitles {
    fn default() -> Self {
        Self {
            tracks: Vec::new(),
            source: None,
            selected: None,
            cues: Vec::new(),
            delay_ms: 0,
            size: 1.0,
            loading: false,
            error: None,
            image: None,
            image_loading: false,
            image_error: None,
            open: false,
        }
    }
}

/// The episodes before and after the one playing, when the series has them.
pub type EpisodeNeighbours = (Option<Item>, Option<Item>);

/// One player panel at a time. The screen renders an `if/else if` chain, so two flags left set
/// would hide one panel and reveal the other when the visible one closed. Every open and close
/// goes through `State::open_player_panel` so that cannot happen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayerPanel {
    #[default]
    None,
    Audio,
    Subtitles,
    Quality,
    Chapters,
    Queue,
    Sleep,
    Goto,
    Gear,
}

/// Which layers were open at the last focus check, so a newly opened menu is focused once.
#[derive(Default)]
struct FocusLayers {
    /// Whether the connect screen has handed over to the signed-in app.
    ready: bool,
    /// Whether the Quick Connect card replaced the sign-in form.
    quick: bool,
    /// Whether the controls sheet opened.
    shortcuts: bool,
    shelf: bool,
    filters: bool,
    genres: bool,
    playlist_picker: bool,
    collection_picker: bool,
    playlist_create: bool,
    cast_picker: bool,
    preplay: bool,
    still_watching: bool,
    quality: bool,
    sleep: bool,
    goto: bool,
    card: bool,
    gear: bool,
    subtitles: bool,
    audio: bool,
    chapters: bool,
    queue: bool,
}

pub struct State {
    pub config: Config,
    pub server: TextEdit,
    pub user: TextEdit,
    pub password: TextEdit,
    pub step: Step,
    pub worker: Worker,
    pub home: Latest<HomeLoad>,
    pub home_data: HomeData,
    home_next_up: Latest<Vec<Item>>,
    home_on_now: Latest<Vec<Item>>,
    home_recently_played: Latest<Vec<Item>>,
    home_collections: Latest<Vec<Item>>,
    pub home_loaded: bool,
    pub home_error: Option<String>,
    home_started: bool,
    pub image_cache: Option<ImageCache>,
    pub images: HashMap<String, Image>,
    /// 27×17 RGBA washes painted behind a poster while the full image fades in.
    pub placeholders: HashMap<String, Pixels>,
    pub requested_images: HashSet<String>,
    /// Keys the current screen asked for. The decoded-image cache evicts around them, so a
    /// screen does not blank while it is open just because another menu filled the cache.
    pub current_images: HashSet<String>,
    last_route: Route,
    pub image_notice: Option<String>,
    image_order: VecDeque<String>,
    /// Keys whose full image is still held at zero opacity for the placeholder beat.
    image_fades: HashMap<String, Instant>,
    pub route: Route,
    /// A Home rail's full grid, shown over Home without pushing a route.
    pub shelf: Option<HomeShelf>,
    pub library: Option<LibraryState>,
    /// The library is in multi-select mode; clicking a card toggles it instead of opening.
    pub library_selecting: bool,
    pub library_selected: std::collections::HashSet<String>,
    /// The playlist page is showing its remove controls. Never on for a normal library.
    pub playlist_editing: bool,
    playlist_remove_load: Latest<Result<(), String>>,
    pub library_page: Latest<LibraryPage>,
    library_genres: Latest<Result<Vec<Item>, String>>,
    pub detail: Option<DetailState>,
    pub person: Option<PersonPage>,
    person_return: Route,
    person_detail: Option<DetailState>,
    person_latest: Latest<Result<PersonLoad, String>>,
    seerr_person_load: Latest<Result<PersonBundle, String>>,
    pub detail_page: Latest<DetailPage>,
    /// "More like this" loads beside the page: recommendations must never delay the hero.
    detail_similar_load: Latest<Result<Vec<Item>, String>>,
    /// The item the pending recommendations belong to, so a late answer for a page the reader
    /// already left never lands on the new one.
    detail_similar_for: Option<String>,
    pub detail_episodes: Latest<EpisodesPage>,
    pub detail_return: Route,
    /// Earlier detail pages when one detail opens another (a collection member), newest last.
    detail_stack: Vec<(DetailState, Route)>,
    pub detail_notice: Option<String>,
    /// IMDb is not in Jellyfin's schema; it comes from OMDb when a key is configured.
    pub detail_imdb: Option<f64>,
    detail_imdb_for: Option<String>,
    /// Special features for the open detail page.
    pub detail_extras: Vec<Item>,
    /// Collections this item belongs to.
    pub detail_collections: Vec<Item>,
    detail_collections_for: Option<String>,
    detail_collections_load: Latest<Result<Vec<Item>, String>>,
    /// Theme media carried by (or inherited from) the open detail page.
    pub detail_theme_songs: Vec<Item>,
    pub detail_theme_videos: Vec<Item>,
    detail_theme_load: Latest<Result<jellyfin_api::ThemeMedia, String>>,
    detail_theme_for: Option<String>,
    /// A theme song is decoration, not playback: it is separate from the in-app session.
    pub theme_audio: Option<crate::player::Player>,
    pub theme_audio_title: Option<String>,
    theme_audio_load: Latest<Result<crate::player::Player, String>>,
    theme_audio_starting: bool,
    /// A muted theme video behind the hero. Off by default; the toggle starts it.
    pub theme_video: Option<crate::player::Player>,
    pub theme_video_on: bool,
    theme_video_load: Latest<Result<crate::player::Player, String>>,
    theme_video_starting: bool,
    /// (local ms, server ms) when this device and the server disagree after a crash.
    pub resume_conflict: Option<(i64, i64)>,
    detail_extras_for: Option<String>,
    detail_extras_load: Latest<Result<Vec<Item>, String>>,
    detail_imdb_load: Latest<Result<(String, Option<f64>), String>>,
    imdb_ratings: Option<crate::imdb::ImdbRatings>,
    imdb_ratings_loading: bool,
    imdb_ratings_error: Option<String>,
    imdb_ratings_load: Latest<Result<crate::imdb::ImdbRatings, String>>,
    /// OMDb API key, typed in Settings and stored in the 0600 config.
    pub omdb_key: TextEdit,
    /// The optional Seerr companion's Settings buffers and last probe.
    pub seerr: SeerrState,
    seerr_probe: Latest<Result<SeerrProbe, String>>,
    seerr_requests: Latest<Result<seerr_state::SeerrRequestsLoad, String>>,
    seerr_counts: Latest<Result<RequestCounts, String>>,
    seerr_summaries: Latest<Result<seerr_state::SummaryBatch, String>>,
    seerr_discover: Latest<Result<SeerrDiscover, String>>,
    seerr_search: Latest<Result<SearchResults, String>>,
    seerr_detail_load: Latest<Result<Details, String>>,
    seerr_detail_for: Option<(String, u64)>,
    seerr_series_load: Latest<Result<seerr_api::TvDetails, String>>,
    seerr_library_load: Latest<Result<seerr_state::LibraryRows, String>>,
    seerr_genres: Latest<Result<seerr_state::GenrePair, String>>,
    seerr_browse: Latest<Result<seerr_api::SearchResults, String>>,
    seerr_search_global: Latest<Result<seerr_api::SearchResults, String>>,
    seerr_browse_for: Option<(DiscoverFilters, u32)>,
    seerr_series_for: Option<u64>,
    /// The right-click menu on a poster, drawn in-app.
    pub card_menu: Option<CardMenu>,
    /// LAN servers the discovery handshake found, for the connect screen.
    pub discovered: Vec<crate::discovery::DiscoveredServer>,
    pub discovering: bool,
    discovery_load: Latest<Vec<crate::discovery::DiscoveredServer>>,
    /// Holds the idle/sleep inhibitor while a video plays.
    pub idle: idle::IdleGuard,
    /// The "Add to playlist" picker, opened from a detail page or a card's item menu.
    pub playlist_picker_open: bool,
    pub playlist_picker_item: Option<Item>,
    /// The ids the picker will add; a bulk selection uses this instead of one detail item.
    pub playlist_picker_ids: Option<Vec<String>>,
    playlist_add_load: Latest<Result<(), String>>,
    /// The add waiting for its answer, so a successful one can offer Undo.
    pub(crate) playlist_add_pending: Option<(String, String, String)>,
    /// The "Add to collection" picker, opened from a detail page or a card's item menu.
    pub collection_picker_open: bool,
    pub collection_picker_item: Option<Item>,
    /// The ids the picker will add; a bulk selection uses this instead of one detail item.
    pub collection_picker_ids: Option<Vec<String>>,
    /// Box sets on the server, loaded when the picker opens.
    pub collection_choices: Vec<Item>,
    pub collection_choices_loading: bool,
    collection_choices_load: Latest<Result<Vec<Item>, String>>,
    collection_add_load: Latest<Result<(), String>>,
    collection_add_pending: Option<(String, String)>,
    /// The "New playlist" prompt: naming a playlist that starts with the current item.
    pub playlist_create_open: bool,
    pub playlist_new_name: TextEdit,
    playlist_create_load: Latest<Result<Item, String>>,
    /// "Play on…": the item waiting for a target session, and the sessions on offer.
    pub cast_picker_open: bool,
    pub cast_item: Option<Item>,
    pub cast_sessions: Vec<jellyfin_api::SessionInfo>,
    pub cast_loading: bool,
    cast_sessions_load: Latest<Result<Vec<jellyfin_api::SessionInfo>, String>>,
    cast_send_load: Latest<Result<(), String>>,
    /// An in-player "Continue on…": stop this player only after the server accepted the send.
    pub transfer_after_cast: bool,
    /// A short message over the page, e.g. "Added to the playlist". Cleared by its own timer.
    pub toast: Option<String>,
    pub toast_undo: Option<UndoAction>,
    toast_until: Option<Instant>,
    pub detail_overview_open: bool,
    pub detail_tech_open: bool,
    pub playback_start: Latest<Result<PlaybackHandle, Failure>>,
    pub playback: Option<PlaybackSession>,
    pub launching: bool,
    pub playback_error: Option<String>,
    pending_play: Option<PendingPlay>,
    next_playback: u64,
    pub search: SearchState,
    pub search_latest: Latest<SearchLoad>,
    pub search_hints: Latest<Result<Vec<jellyfin_api::SearchHint>, String>>,
    search_return: Route,
    pub remote: RemoteState,
    pub remote_latest: Latest<Result<Vec<jellyfin_api::SessionInfo>, String>>,
    pub live: LiveState,
    /// Opt-in desktop notifications; the in-app toast is always shown.
    pub notifications: bool,
    /// Player shortcuts the reader has remapped.
    pub key_bindings: KeyBindings,
    /// The action waiting for its next key, or `None` when not rebinding.
    pub rebinding: Option<ShortcutAction>,
    /// Live programs the reader asked to be reminded about, persisted locally.
    pub live_reminders: Vec<LiveReminder>,
    /// A reminder that already fired this run, so one minute produces one toast.
    reminders_fired: HashSet<String>,
    last_reminder_check: Instant,
    pub quick: Option<QuickLogin>,
    quick_start: Latest<Result<QuickStart, Failure>>,
    quick_poll: Latest<Result<QuickConnectState, Failure>>,
    quick_auth: Latest<Result<AuthResult, Failure>>,
    live_latest: Latest<Result<crate::state::live_state::LiveLoad, String>>,
    live_open: Latest<Result<jellyfin_api::LiveStream, Failure>>,
    live_close: Latest<Result<(), String>>,
    /// A channel chosen while the old tuner is still closing. The server refuses a second
    /// open before the first close lands, so the next channel waits for that answer.
    pending_zap: Option<Item>,
    pending_live: Option<Item>,
    pub in_app: Option<InAppSession>,
    pub in_app_fill: bool,
    /// 0.5–2.0; GStreamer holds the rate, this is what the OSD labels.
    pub in_app_speed: f64,
    /// What "Play all" or "Shuffle" started: the run in playing order. Empty for a single
    /// item started from a card or a detail page.
    pub queue: Vec<Item>,
    /// Index into `queue` of the item playing now.
    pub queue_at: usize,
    /// The run's item list over the video, opened from the OSD's `2 of 4` chip.
    pub in_app_queue_open: bool,
    /// Playback preferences: the reader's defaults, not the current film's state.
    pub autoplay_next: bool,
    pub subtitle_mode: SubtitleMode,
    pub skip_mode: crate::config::SkipMode,
    pub still_watching: bool,
    pub still_watching_episodes: i64,
    pub still_watching_minutes: i64,
    pub resume_rewind_seconds: i64,
    pub downmix_stereo: bool,
    pub audio_delay_ms: i64,
    pub audio_mode: crate::config::AudioMode,
    pub reduce_motion: bool,
    /// The Settings search field; it filters the rows already on the page.
    pub settings_query: rune::TextEdit,
    pub library_search: rune::TextEdit,
    library_search_due: Option<Instant>,
    /// A run's episode count, reset when a new run starts.
    pub in_app_run_episodes: u32,
    pub in_app_last_input: Instant,
    pub in_app_play_since: Instant,
    /// When the player's clock was last queried. `query_position` and `query_duration` take the
    /// pipeline lock, which the streaming thread holds during a flush or renegotiation; asking
    /// every frame turns that into UI stalls. Ten times a second is plenty for the bar.
    pub in_app_query_at: Instant,
    /// Why the integrated player refused the last item. Kept so a failed mpv fallback can say
    /// both reasons instead of only "mpv isn't installed".
    pub last_in_app_error: Option<String>,
    pub still_watching_open: bool,
    pub still_watching_next: Option<Item>,
    /// When the card opened, so its 60 s countdown can stop the run quietly.
    pub still_watching_since: Option<Instant>,
    pub still_watching_countdown: i64,
    /// Last percentage applied by the sleep timer's 20 s fade.
    pub sleep_fade_percent: i64,
    pub sleep_deadline: Option<Instant>,
    pub sleep_end_of_episode: bool,
    pub in_app_sleep_open: bool,
    pub in_app_quality_open: bool,
    pub in_app_goto_open: bool,
    /// The settings sheet. `returns` keeps it on screen after a row opened a panel, so closing
    /// that panel comes back here instead of dropping the reader on the picture.
    pub in_app_gear_open: bool,
    pub in_app_gear_returns: bool,
    pub in_app_goto: rune::TextEdit,
    /// A–B loop points, in milliseconds. Both set means the loop is active.
    pub loop_a: Option<i64>,
    pub loop_b: Option<i64>,
    /// Last known local position per item, loaded once at startup and written off-thread.
    checkpoints: std::collections::HashMap<String, i64>,
    checkpoint_at: Instant,
    pub subtitle_background: SubtitleBackground,
    /// Typography for the player's own text overlay. ASS inline styling still wins where the
    /// script sets it; these are the reader's defaults for plain SRT/VTT cues.
    pub subtitle_font: SubtitleFont,
    pub subtitle_color: SubtitleColor,
    pub subtitle_outline: SubtitleOutline,
    pub subtitle_position: SubtitlePosition,
    pub subtitle_style: SubtitleStyle,
    pub subtitle_opacity: f32,
    pub subtitle_background_opacity: f32,
    /// Three-letter language codes; empty means no preference.
    pub subtitle_language: String,
    pub audio_language: String,
    pub seek_step_ms: i64,
    /// 0–2; mirrored from GStreamer for the volume slider's knob.
    pub in_app_volume: f64,
    /// The root view's measured size, saved on close so the next launch opens at the same size.
    pub window_size: (f32, f32),
    /// Whether the window is borderless-fullscreen. The shell applies `pending_fullscreen`
    /// once, in `poll`, so handlers never need a `Tx`.
    pub fullscreen: bool,
    pub pending_fullscreen: Option<bool>,
    /// The session-bus media service, absent in headless renders and without a bus.
    mpris: Option<crate::mpris::Mpris>,
    /// The root view's screen origin, for the next launch. Compositor-owned position is not
    /// guaranteed on Wayland; a rejected request simply leaves the default placement.
    pub window_position: (f32, f32),
    /// A library-wide run being fetched from the server: `(items, shuffle)`.
    queue_load: Latest<(Vec<Item>, bool)>,
    /// The user's playlists, for the sidebar. Loaded beside the home rails; a failure is an
    /// empty section, not a broken home.
    pub playlists: Vec<Item>,
    playlists_load: Latest<Result<Vec<Item>, String>>,
    pub in_app_minimized: bool,
    pub sidebar_collapsed: bool,
    pub shortcuts_open: bool,
    pub osd_visible: bool,
    osd_hide_at: Option<Instant>,
    pub in_app_start: Latest<Result<InAppStart, Failure>>,
    /// A YouTube trailer download in flight; the detail button shows its progress.
    pub trailer_loading: bool,
    trailer_load: Latest<Result<(String, std::path::PathBuf), String>>,
    pub in_app_segments: Vec<MediaSegment>,
    pub in_app_segments_load: Latest<Result<Vec<MediaSegment>, String>>,
    pub in_app_subtitles: Subtitles,
    pub in_app_subtitle_search: SubtitleSearch,
    in_app_subtitle_search_load: Latest<Result<Vec<jellyfin_api::RemoteSubtitle>, String>>,
    in_app_subtitle_download_load: Latest<Result<(), String>>,
    /// Stream indexes that existed before an online subtitle download, so the new track can be
    /// picked out of the refreshed list and selected for the viewer.
    pending_subtitle_before: Option<Vec<i32>>,
    in_app_subtitle_load: Latest<Result<SubtitleCues, String>>,
    in_app_image_load: Latest<Result<(usize, crate::image_subtitles::ImageSubtitle), String>>,
    in_app_streams_load: Latest<Result<PlaybackStreams, String>>,
    pub in_app_audio_tracks: Vec<jellyfin_api::AudioTrack>,
    pub in_app_audio_open: bool,
    /// A language preference or the viewer has already picked an audio track this session.
    in_app_audio_chosen: bool,
    /// The playing file's technical summary for the stats overlay.
    pub in_app_video: Option<(String, i32, i32, Option<String>)>,
    pub in_app_container: Option<String>,
    pub in_app_stats: bool,
    /// The episode before the one playing, so a rewatch can step back without leaving.
    pub in_app_prev: Option<Item>,
    in_app_neighbours_load: Latest<Result<EpisodeNeighbours, String>>,
    /// The chapter list over the video. Chapter marks stay in the seek bar either way.
    pub in_app_chapters_open: bool,
    /// The episode after the one playing, so the last 30 s can offer it and EOS can roll on.
    pub in_app_next: Option<Item>,
    pub in_app_next_dismissed: bool,

    /// Pointer position and time under the timeline, for trickplay preview.
    pub in_app_hover: Option<(f32, i64)>,
    pub in_app_seek_width: f32,
    /// True once a real pointer move has rested on the timeline; the first hover-change of a
    /// fresh frame is not a leave, so it must not clear a preview the scene seeded.
    in_app_hover_armed: bool,
    in_app_trickplay_key: Option<String>,
    pub in_app_launching: bool,
    pending_in_app: Option<PendingInApp>,
    waker: Option<rune::Waker>,
    /// Last viewport position per library id, restored when the library is reopened.
    library_scrolls: HashMap<String, f32>,
    /// Applied by `main::poll` with `Tx::scroll_to_y`.
    pending_scroll: Option<f32>,
    /// The media source chosen on the detail Version row for the next play.
    pub in_app_source: Option<String>,
    /// The item the pending source belongs to; a stale choice never leaks to another film.
    pub in_app_source_for: Option<String>,
    /// The last version chosen per item, so reopening a film keeps the edition.
    pub version_choices: std::collections::HashMap<String, String>,
    /// The last quality chosen; Auto keeps direct play first.
    pub playback_quality: crate::config::PlaybackQuality,
    /// Quality chosen per item; a re-open keeps the edition's cap.
    pub quality_choices: std::collections::HashMap<String, crate::config::PlaybackQuality>,
    /// Home ids hidden locally. No server write: this is this window's opinion.
    pub hidden_home: std::collections::HashSet<String>,
    pub spoiler_guard: bool,
    pub saved_views: Vec<crate::config::SavedView>,
    pub home_hero: bool,
    pub light_theme: bool,
    /// The running interface mode. The saved choice is `config.ui_mode`; a change needs a
    /// restart because the layout scale is fixed at window creation.
    pub big_screen: bool,
    /// Settings changed the saved mode this session, so the card shows that pending choice and
    /// the restart note instead of the mode the window is running.
    pub ui_mode_restart: bool,
    /// The big-screen launch or a route change asks for focus on the screen's first item.
    pub focus_first: bool,
    /// The route the last focus request was for, so a route change arms `focus_first` once.
    focus_route: Option<Route>,
    /// Layer-open edges, so a menu that appears takes focus once instead of every poll.
    focus_layers: FocusLayers,
    /// The card that opened the item menu, so closing it returns focus there.
    card_menu_focus: Option<String>,
    pub high_contrast: bool,
    /// Reader text size applied to every type token.
    pub text_scale: f32,
    pub mpris_shuffle: bool,
    tray: Option<rune::Tray>,
    tray_events: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<rune::TrayEvent>>>,
    tray_tooltip: String,
    /// Local watch history, newest last.
    pub history: Vec<crate::history::HistoryEntry>,
    pub history_ms: i64,
    /// A place to reopen after the first Home load; set from the config.
    pending_place: Option<crate::config::LastPlace>,
    /// The pre-playback summary sheet on the detail page.
    pub preplay_open: bool,
    /// Set by the image cache's waker; a decoded poster is a reason to draw.
    image_dirty: Arc<AtomicBool>,
    /// False until `setup`, so headless tests never start a network request.
    fetch_home: bool,
    /// False for states built from `from_config` in tests: a test must not rewrite the user's
    /// real session file.
    persist: bool,
}

impl State {
    pub fn new() -> State {
        let mut state = State::from_config(Config::load());
        state.persist = true;
        state
    }

    /// The testable half of `new`: no disk. A stored session opens straight onto the home screen;
    /// a revoked token is found on the first real request (M2) and bounces back here.
    pub fn from_config(config: Config) -> State {
        let step = if config.has_session() {
            Step::Ready {
                server_name: if config.server_name.is_empty() {
                    config.server_url.clone()
                } else {
                    config.server_name.clone()
                },
            }
        } else {
            Step::Idle
        };
        let mut server = TextEdit::new(config.server_url.clone());
        server.focused = config.server_url.is_empty();
        let mut user = TextEdit::new(config.user_name.clone());
        user.focused = !config.server_url.is_empty() && config.user_name.is_empty();
        let big_screen = config.big_screen();
        let mut state = State {
            server,
            user,
            password: TextEdit::new("").secure(),
            step,
            worker: Worker::new(),
            home: Latest::default(),
            home_data: HomeData::default(),
            home_next_up: Latest::default(),
            home_on_now: Latest::default(),
            home_recently_played: Latest::default(),
            home_collections: Latest::default(),
            home_loaded: false,
            home_error: None,
            home_started: false,
            image_cache: None,
            images: HashMap::new(),
            placeholders: HashMap::new(),
            requested_images: HashSet::new(),
            current_images: HashSet::new(),
            last_route: Route::Home,
            image_notice: None,
            image_order: VecDeque::new(),
            image_fades: HashMap::new(),
            route: Route::Home,
            shelf: None,
            library: None,
            library_selecting: false,
            library_selected: std::collections::HashSet::new(),
            playlist_editing: false,
            playlist_remove_load: Latest::default(),
            library_page: Latest::default(),
            library_genres: Latest::default(),
            detail: None,
            person: None,
            person_return: Route::Home,
            person_detail: None,
            person_latest: Latest::default(),
            seerr_person_load: Latest::default(),
            detail_page: Latest::default(),
            detail_similar_load: Latest::default(),
            detail_similar_for: None,
            detail_episodes: Latest::default(),
            detail_return: Route::Home,
            detail_stack: Vec::new(),
            detail_notice: None,
            detail_imdb: None,
            detail_imdb_for: None,
            detail_extras: Vec::new(),
            detail_collections: Vec::new(),
            detail_collections_for: None,
            detail_collections_load: Latest::default(),
            detail_theme_songs: Vec::new(),
            detail_theme_videos: Vec::new(),
            detail_theme_load: Latest::default(),
            detail_theme_for: None,
            theme_audio: None,
            theme_audio_title: None,
            theme_audio_load: Latest::default(),
            theme_audio_starting: false,
            theme_video: None,
            theme_video_on: false,
            theme_video_load: Latest::default(),
            theme_video_starting: false,
            resume_conflict: None,
            detail_extras_for: None,
            detail_extras_load: Latest::default(),
            detail_imdb_load: Latest::default(),
            imdb_ratings: None,
            imdb_ratings_loading: false,
            imdb_ratings_error: None,
            imdb_ratings_load: Latest::default(),
            omdb_key: TextEdit::new(config.omdb_api_key.clone().unwrap_or_default()).secure(),
            seerr: SeerrState::new(&config),
            seerr_probe: Latest::default(),
            seerr_requests: Latest::default(),
            seerr_counts: Latest::default(),
            seerr_summaries: Latest::default(),
            seerr_discover: Latest::default(),
            seerr_search: Latest::default(),
            seerr_detail_load: Latest::default(),
            seerr_detail_for: None,
            seerr_series_load: Latest::default(),
            seerr_library_load: Latest::default(),
            seerr_genres: Latest::default(),
            seerr_browse: Latest::default(),
            seerr_search_global: Latest::default(),
            seerr_browse_for: None,
            seerr_series_for: None,
            card_menu: None,
            discovered: Vec::new(),
            discovering: false,
            discovery_load: Latest::default(),
            idle: idle::IdleGuard::default(),
            playlist_picker_open: false,
            playlist_picker_item: None,
            playlist_picker_ids: None,
            playlist_add_load: Latest::default(),
            playlist_add_pending: None,
            collection_picker_open: false,
            collection_picker_item: None,
            collection_picker_ids: None,
            collection_choices: Vec::new(),
            collection_choices_loading: false,
            collection_choices_load: Latest::default(),
            collection_add_load: Latest::default(),
            collection_add_pending: None,
            playlist_create_open: false,
            playlist_new_name: TextEdit::new(""),
            cast_picker_open: false,
            cast_item: None,
            cast_sessions: Vec::new(),
            cast_loading: false,
            cast_sessions_load: Latest::default(),
            cast_send_load: Latest::default(),
            transfer_after_cast: false,
            toast: None,
            toast_undo: None,
            toast_until: None,
            playlist_create_load: Latest::default(),
            detail_overview_open: false,
            detail_tech_open: false,
            playback_start: Latest::default(),
            playback: None,
            launching: false,
            playback_error: None,
            pending_play: None,
            next_playback: 0,
            search: SearchState::new(),
            search_latest: Latest::default(),
            search_hints: Latest::default(),
            search_return: Route::Home,
            remote: RemoteState::default(),
            remote_latest: Latest::default(),
            live: LiveState {
                favorites: config
                    .live_favorites
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .collect(),
                ..LiveState::default()
            },
            notifications: config.notifications.unwrap_or(false),
            key_bindings: config.shortcuts.clone().unwrap_or_default(),
            rebinding: None,
            live_reminders: config.live_reminders.clone().unwrap_or_default(),
            reminders_fired: HashSet::new(),
            last_reminder_check: Instant::now(),
            quick: None,
            quick_start: Latest::default(),
            quick_poll: Latest::default(),
            quick_auth: Latest::default(),
            live_latest: Latest::default(),
            live_open: Latest::default(),
            live_close: Latest::default(),
            pending_zap: None,
            pending_live: None,
            in_app: None,
            in_app_fill: false,
            in_app_speed: 1.0,
            in_app_volume: 1.0,
            queue: Vec::new(),
            queue_at: 0,
            in_app_queue_open: false,
            autoplay_next: true,
            subtitle_mode: SubtitleMode::Forced,
            skip_mode: crate::config::SkipMode::Ask,
            still_watching: true,
            still_watching_episodes: 3,
            still_watching_minutes: 90,
            resume_rewind_seconds: 10,
            downmix_stereo: false,
            audio_delay_ms: 0,
            audio_mode: crate::config::AudioMode::Off,
            reduce_motion: false,
            settings_query: rune::TextEdit::new(""),
            library_search: rune::TextEdit::new(""),
            library_search_due: None,
            in_app_run_episodes: 0,
            in_app_last_input: Instant::now(),
            in_app_play_since: Instant::now(),
            in_app_query_at: Instant::now(),
            last_in_app_error: None,
            still_watching_open: false,
            still_watching_next: None,
            still_watching_since: None,
            still_watching_countdown: 60,
            sleep_fade_percent: 100,
            sleep_deadline: None,
            sleep_end_of_episode: false,
            in_app_sleep_open: false,
            in_app_quality_open: false,
            in_app_goto_open: false,
            in_app_gear_open: false,
            in_app_gear_returns: false,
            in_app_goto: rune::TextEdit::new(""),
            loop_a: None,
            loop_b: None,
            checkpoints: std::collections::HashMap::new(),
            checkpoint_at: Instant::now(),
            subtitle_background: SubtitleBackground::Dim,
            subtitle_font: config.subtitle_font.unwrap_or_default(),
            subtitle_color: config.subtitle_color.unwrap_or_default(),
            subtitle_outline: config.subtitle_outline.unwrap_or_default(),
            subtitle_position: config.subtitle_position.unwrap_or_default(),
            subtitle_style: config.subtitle_style.unwrap_or_default(),
            subtitle_opacity: config.subtitle_opacity.unwrap_or(1.0).clamp(0.2, 1.0),
            subtitle_background_opacity: config
                .subtitle_background_opacity
                .unwrap_or(1.0)
                .clamp(0.0, 1.0),
            subtitle_language: String::new(),
            audio_language: String::new(),
            seek_step_ms: 10_000,
            window_size: (0.0, 0.0),
            fullscreen: false,
            pending_fullscreen: None,
            mpris: None,
            window_position: (0.0, 0.0),
            queue_load: Latest::default(),
            playlists: Vec::new(),
            playlists_load: Latest::default(),
            in_app_minimized: false,
            sidebar_collapsed: false,
            shortcuts_open: false,
            osd_visible: true,
            osd_hide_at: None,
            in_app_start: Latest::default(),
            trailer_loading: false,
            trailer_load: Latest::default(),
            in_app_segments: Vec::new(),
            in_app_segments_load: Latest::default(),
            in_app_subtitles: Subtitles::default(),
            in_app_subtitle_search: SubtitleSearch::default(),
            in_app_subtitle_search_load: Latest::default(),
            in_app_subtitle_download_load: Latest::default(),
            pending_subtitle_before: None,
            in_app_subtitle_load: Latest::default(),
            in_app_image_load: Latest::default(),
            in_app_streams_load: Latest::default(),
            in_app_audio_tracks: Vec::new(),
            in_app_audio_open: false,
            in_app_audio_chosen: false,
            in_app_video: None,
            in_app_container: None,
            in_app_stats: false,
            in_app_prev: None,
            in_app_neighbours_load: Latest::default(),
            in_app_chapters_open: false,
            in_app_next: None,
            in_app_next_dismissed: false,

            in_app_hover: None,
            in_app_seek_width: 0.0,
            in_app_hover_armed: false,
            in_app_trickplay_key: None,
            in_app_launching: false,
            pending_in_app: None,
            waker: None,
            library_scrolls: HashMap::new(),
            pending_scroll: None,
            in_app_source: None,
            in_app_source_for: None,
            version_choices: std::collections::HashMap::new(),
            playback_quality: crate::config::PlaybackQuality::Auto,
            quality_choices: std::collections::HashMap::new(),
            hidden_home: std::collections::HashSet::new(),
            spoiler_guard: true,
            saved_views: Vec::new(),
            home_hero: false,
            light_theme: false,
            big_screen,
            ui_mode_restart: false,
            focus_first: big_screen,
            focus_route: None,
            focus_layers: FocusLayers::default(),
            card_menu_focus: None,
            high_contrast: false,
            text_scale: 1.0,
            mpris_shuffle: false,
            tray: None,
            tray_events: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::VecDeque::new(),
            )),
            tray_tooltip: String::new(),
            history: Vec::new(),
            history_ms: 0,
            pending_place: config.last_place.clone(),
            preplay_open: false,
            image_dirty: Arc::new(AtomicBool::new(false)),
            fetch_home: false,
            persist: false,
            config,
        };
        state.in_app_subtitles.size = if state.config.subtitle_size > 0.0 {
            state.config.subtitle_size.clamp(0.7, 1.8)
        } else {
            1.0
        };
        state.in_app_subtitles.delay_ms = state.config.subtitle_delay_ms.clamp(-30_000, 30_000);
        state.autoplay_next = state.config.autoplay_next.unwrap_or(true);
        state.subtitle_mode = state.config.subtitle_mode.unwrap_or_default();
        state.skip_mode = state.config.skip_segments.unwrap_or_default();
        state.still_watching = state.config.still_watching.unwrap_or(true);
        state.still_watching_episodes = state
            .config
            .still_watching_episodes
            .unwrap_or(3)
            .clamp(2, 10);
        state.still_watching_minutes = state
            .config
            .still_watching_minutes
            .unwrap_or(90)
            .clamp(15, 240);
        state.resume_rewind_seconds = match state.config.resume_rewind_seconds.unwrap_or(10) {
            0 | 5 | 10 | 15 => state.config.resume_rewind_seconds.unwrap_or(10),
            _ => 10,
        };
        state.downmix_stereo = state.config.downmix_stereo.unwrap_or(false);
        state.audio_delay_ms = state.config.audio_delay_ms.unwrap_or(0).clamp(-100, 100);
        state.audio_mode = state.config.audio_mode.unwrap_or_default();
        state.reduce_motion = state.config.reduce_motion.unwrap_or(false);
        state.version_choices = state.config.version_choices.clone().unwrap_or_default();
        state.playback_quality = state.config.playback_quality.unwrap_or_default();
        state.quality_choices = state.config.quality_choices.clone().unwrap_or_default();
        state.spoiler_guard = state.config.spoiler_guard.unwrap_or(true);
        state.saved_views = state.config.saved_views.clone().unwrap_or_default();
        state.home_hero = state.config.home_hero.unwrap_or(false);
        state.light_theme = state.config.light_theme.unwrap_or(false);
        crate::theme::set_light(state.light_theme);
        state.text_scale = match state.config.text_scale.unwrap_or(1.0) {
            scale if (0.85..=1.35).contains(&scale) => scale,
            _ => 1.0,
        };
        crate::theme::set_text_scale(state.text_scale);
        state.high_contrast = state.config.high_contrast.unwrap_or(false);
        crate::theme::set_high_contrast(state.high_contrast);
        state.hidden_home = state
            .config
            .hidden_home
            .clone()
            .unwrap_or_default()
            .into_iter()
            .collect();
        state.subtitle_background = state.config.subtitle_background.unwrap_or_default();
        state.subtitle_language = state.config.subtitle_language.clone().unwrap_or_default();
        state.audio_language = state.config.audio_language.clone().unwrap_or_default();
        state.seek_step_ms = state.config.seek_seconds.unwrap_or(10).clamp(5, 60) * 1000;
        state.in_app_volume = state.config.volume.unwrap_or(1.0).clamp(0.0, 2.0);
        state
    }

    /// The languages the playback preferences cycle through. Empty is "Any": the server's
    /// first track, which is what the player did before this was a preference.
    pub const PREFERRED_LANGUAGES: [(&'static str, &'static str); 7] = [
        ("", "Any language"),
        ("eng", "English"),
        ("spa", "Español"),
        ("fre", "Français"),
        ("ger", "Deutsch"),
        ("jpn", "日本語"),
        ("por", "Português"),
    ];

    pub fn cycle_autoplay_next(&mut self) {
        self.autoplay_next = !self.autoplay_next;
    }

    /// 5 → 10 → 30 → 60 → 5 seconds. Ten is the long-standing default.
    pub fn cycle_seek_step(&mut self) {
        let seconds = self.seek_step_ms / 1000;
        self.seek_step_ms = match seconds {
            5 => 10_000,
            10 => 30_000,
            30 => 60_000,
            _ => 5_000,
        };
    }

    pub fn cycle_subtitle_mode(&mut self) {
        self.subtitle_mode = self.subtitle_mode.next();
    }

    pub fn cycle_skip_mode(&mut self) {
        self.skip_mode = self.skip_mode.next();
    }

    /// Start or stop the theme song. A second press stops it. The player lives beside the
    /// detail page and is dropped on navigation.
    pub fn toggle_theme_audio(&mut self) {
        if self.theme_audio.is_some() {
            self.stop_theme_audio();
            return;
        }
        let Some(item) = self.detail_theme_songs.first() else {
            return;
        };
        let Some(client) = self.client() else {
            return;
        };
        let Some(url) = client.direct_stream_url(&item.id) else {
            return;
        };
        if self.theme_audio_starting {
            return;
        }
        self.theme_audio_starting = true;
        self.theme_audio_load
            .request(url, |url| crate::player::Player::start_audio(&url));
    }

    /// Leaving the page or starting a film stops both theme players; decoration must never
    /// outlive the screen that started it.
    pub fn stop_theme_media(&mut self) {
        self.stop_theme_audio();
        self.stop_theme_video();
    }

    pub fn stop_theme_audio(&mut self) {
        self.theme_audio_starting = false;
        self.theme_audio_load.cancel();
        if let Some(player) = self.theme_audio.take() {
            player.stop();
        }
        self.theme_audio_title = None;
    }

    /// A muted theme video behind the hero. Reduced motion and the off default keep it opt-in.
    pub fn toggle_theme_video(&mut self) {
        if self.reduce_motion {
            self.show_toast("Theme video is off while reduced motion is on.");
            return;
        }
        if self.theme_video_on {
            self.stop_theme_video();
            return;
        }
        if self.theme_video_starting {
            return;
        }
        let Some(item) = self.detail_theme_videos.first() else {
            return;
        };
        let Some(client) = self.client() else {
            return;
        };
        let Some(url) = client.direct_stream_url(&item.id) else {
            return;
        };
        // Starting a pipeline can take seconds; it belongs on the latest worker, not a handler.
        self.theme_video_starting = true;
        let waker = self.waker.clone();
        self.theme_video_load.request((url, waker), |(url, waker)| {
            crate::player::Player::start(&url, rune::NativeSurface::new(), waker)
        });
    }

    pub fn stop_theme_video(&mut self) {
        self.theme_video_on = false;
        self.theme_video_starting = false;
        self.theme_video_load.cancel();
        if let Some(player) = self.theme_video.take() {
            player.stop();
        }
    }

    /// Choose a source in the pre-playback sheet without starting playback yet.
    pub fn toggle_spoiler_guard(&mut self) {
        self.spoiler_guard = !self.spoiler_guard;
    }

    pub fn is_home_hidden(&self, item_id: &str) -> bool {
        self.hidden_home.contains(item_id)
    }

    pub fn hide_from_home(&mut self, item_id: String) {
        if !self.hidden_home.insert(item_id.clone()) {
            return;
        }
        self.show_undo_toast(
            "Hidden from Home.",
            crate::state::UndoAction::NotInterested { item_id },
        );
    }

    /// A one-line honest answer to "Why is this on Home?".
    pub fn explain_home_item(&mut self, item_id: &str) {
        let reason = if self.home_data.resume.iter().any(|item| item.id == item_id) {
            "Because you started it and have not finished it."
        } else if self.home_data.next_up.iter().any(|item| item.id == item_id) {
            "The next unwatched episode of a series you are watching."
        } else if self
            .home_data
            .recently_played
            .iter()
            .any(|item| item.id == item_id)
        {
            "Because you played it recently."
        } else if self
            .home_data
            .latest
            .iter()
            .flat_map(|rail| rail.items.iter())
            .any(|item| item.id == item_id)
        {
            "Recently added to one of your libraries."
        } else if self
            .home_data
            .collections
            .iter()
            .any(|item| item.id == item_id)
        {
            "A box set the server has in your collection library."
        } else {
            "The server returned it for this rail."
        };
        self.show_toast(reason);
    }

    pub fn cycle_playback_quality(&mut self) {
        self.playback_quality = self.playback_quality.next();
        if let Some(item) = self.detail.as_ref().and_then(|detail| detail.play_target()) {
            self.quality_choices
                .insert(item.id.clone(), self.playback_quality);
        }
        self.show_toast(format!("Quality: {}", self.playback_quality.label()));
    }

    pub fn set_playback_quality(&mut self, quality: crate::config::PlaybackQuality) {
        if self.playback_quality == quality {
            return;
        }
        self.playback_quality = quality;
        self.show_toast(format!("Quality: {}", quality.label()));
    }

    pub fn set_preplay_source(&mut self, source_id: String) {
        if let Some(target) = self.detail.as_ref().and_then(|detail| detail.play_target()) {
            self.version_choices
                .insert(target.id.clone(), source_id.clone());
        }
        self.in_app_source = Some(source_id);
        self.in_app_source_for = self
            .detail
            .as_ref()
            .and_then(|detail| detail.play_target())
            .map(|item| item.id.clone());
    }

    pub fn open_preplay(&mut self) {
        self.preplay_open = true;
    }

    pub fn close_preplay(&mut self) {
        self.preplay_open = false;
        self.in_app_source = None;
        self.in_app_source_for = None;
    }

    /// Put every preference back to the shipped default. Server, user and device id stay.
    pub fn reset_preferences(&mut self) {
        self.autoplay_next = true;
        self.seek_step_ms = 10_000;
        self.skip_mode = crate::config::SkipMode::Ask;
        self.still_watching = true;
        self.still_watching_episodes = 3;
        self.still_watching_minutes = 90;
        self.resume_rewind_seconds = 10;
        self.downmix_stereo = false;
        self.audio_delay_ms = 0;
        self.audio_mode = crate::config::AudioMode::Off;
        self.reduce_motion = false;
        self.subtitle_mode = crate::config::SubtitleMode::Forced;
        self.subtitle_background = crate::config::SubtitleBackground::Dim;
        self.subtitle_font = crate::config::SubtitleFont::Sans;
        self.subtitle_color = crate::config::SubtitleColor::White;
        self.subtitle_outline = crate::config::SubtitleOutline::default();
        self.subtitle_position = crate::config::SubtitlePosition::Bottom;
        self.subtitle_style = crate::config::SubtitleStyle::default();
        self.subtitle_opacity = 1.0;
        self.subtitle_background_opacity = 0.55;
        self.in_app_subtitles.size = 1.0;
        self.in_app_subtitles.delay_ms = 0;
        self.subtitle_language = String::new();
        self.audio_language = String::new();
        self.in_app_volume = 1.0;
        self.save_preferences();
        self.show_toast("Preferences reset.");
    }

    /// Write a readable copy of the current preferences next to the config. The write is on a
    /// worker thread; the toast gives the path the reader needs.
    pub fn export_preferences(&mut self) {
        self.save_preferences();
        let Ok(text) = toml::to_string_pretty(&self.config) else {
            self.show_toast("Could not serialise the preferences.");
            return;
        };
        let Some(directory) = crate::config::Config::path()
            .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
        else {
            self.show_toast("No config directory to export to.");
            return;
        };
        let path = directory.join("runefin-preferences.toml");
        let target = path.clone();
        std::thread::spawn(move || {
            let _ = std::fs::write(target, text);
        });
        self.show_toast(format!("Exported to {}", path.display()));
    }

    pub fn toggle_reduce_motion(&mut self) {
        self.reduce_motion = !self.reduce_motion;
        if self.reduce_motion {
            self.stop_theme_video();
        }
    }

    pub fn toggle_downmix(&mut self) {
        self.downmix_stereo = !self.downmix_stereo;
        if let Some(session) = &self.in_app {
            session.player.set_downmix(self.downmix_stereo);
        }
    }

    pub fn adjust_audio_delay(&mut self, delta: i64) {
        self.audio_delay_ms = (self.audio_delay_ms + delta).clamp(-100, 100);
        if let Some(session) = &self.in_app {
            session.player.set_audio_delay(self.audio_delay_ms);
        }
    }

    pub fn cycle_audio_mode(&mut self) {
        self.audio_mode = self.audio_mode.next();
        if let Some(session) = &self.in_app {
            session.player.set_audio_dynamics(match self.audio_mode {
                crate::config::AudioMode::Off => "off",
                crate::config::AudioMode::Dialogue => "dialogue",
                crate::config::AudioMode::Night => "night",
            });
        }
    }

    pub fn cycle_resume_rewind(&mut self) {
        self.resume_rewind_seconds = match self.resume_rewind_seconds {
            0 => 5,
            5 => 10,
            10 => 15,
            _ => 0,
        };
    }

    /// A tray icon is optional: no host simply means no icon, never a failed launch.
    fn start_tray(&mut self, waker: &rune::Waker) {
        let events = std::sync::Arc::clone(&self.tray_events);
        let wake = waker.clone();
        let icon = rune::TrayIcon::rgba(16, 16, [40u8, 130, 255, 255].repeat(16 * 16));
        let spec = rune::TraySpec::new("runefin", "Runefin", icon)
            .tooltip("Runefin")
            .menu(vec![
                rune::TrayItem::action("playpause", "Play / Pause"),
                rune::TrayItem::action("next", "Next"),
                rune::TrayItem::action("previous", "Previous"),
                rune::TrayItem::Separator,
                rune::TrayItem::action("stop", "Stop"),
            ]);
        self.tray = rune::Tray::new(spec, move |event| {
            if let Ok(mut queue) = events.lock() {
                queue.push_back(event);
            }
            wake();
        })
        .ok();
    }

    /// The newest known position for an item from this device, if any.
    pub fn local_checkpoint(&self, item_id: &str) -> Option<i64> {
        self.checkpoints.get(item_id).copied()
    }

    /// Update and persist the local checkpoint. The write is off-thread.
    /// Drop a checkpoint that the server's answer has superseded.
    pub fn forget_checkpoint(&mut self, item_id: &str) {
        if self.checkpoints.remove(item_id).is_some() && self.persist {
            crate::checkpoint::save(self.checkpoints.clone());
        }
    }

    pub fn remember_checkpoint(&mut self, item_id: &str, position_ms: i64) {
        if position_ms <= 0 {
            return;
        }
        self.checkpoints.insert(item_id.to_string(), position_ms);
        self.checkpoint_at = Instant::now();
        if self.persist {
            crate::checkpoint::save(self.checkpoints.clone());
        }
    }

    pub fn toggle_still_watching(&mut self) {
        self.still_watching = !self.still_watching;
    }

    pub fn set_sleep_timer(&mut self, minutes: Option<u32>) {
        self.sleep_fade_percent = 100;
        if let Some(session) = &self.in_app {
            session.player.set_volume(self.in_app_volume);
        }
        self.sleep_end_of_episode = minutes.is_none();
        self.sleep_deadline =
            minutes.map(|minutes| Instant::now() + Duration::from_secs(u64::from(minutes) * 60));
        self.in_app_sleep_open = false;
        self.show_toast(match minutes {
            Some(minutes) => format!("Sleep timer: {minutes} min."),
            None => "Sleep at the end of this episode.".to_string(),
        });
    }

    pub fn cancel_sleep_timer(&mut self) {
        self.sleep_fade_percent = 100;
        if let Some(session) = &self.in_app {
            session.player.set_volume(self.in_app_volume);
        }
        self.sleep_deadline = None;
        self.sleep_end_of_episode = false;
        self.in_app_sleep_open = false;
        self.show_toast("Sleep timer off.");
    }

    /// Mark A, then B, then clear. A second press at or before A moves A instead.
    pub fn cycle_loop(&mut self) {
        let position = self
            .in_app
            .as_ref()
            .map_or(0, |session| session.position_ms);
        match (self.loop_a, self.loop_b) {
            (None, _) => {
                self.loop_a = Some(position);
                self.loop_b = None;
                self.show_toast("Loop point A set.");
            }
            (Some(a), None) if position > a => {
                self.loop_b = Some(position);
                self.show_toast("Loop A–B is on.");
            }
            (Some(_), None) => {
                self.loop_a = Some(position);
                self.show_toast("Loop point A moved.");
            }
            (Some(_), Some(_)) => {
                self.loop_a = None;
                self.loop_b = None;
                self.show_toast("Loop cleared.");
            }
        }
    }

    pub fn toggle_in_app_goto(&mut self) {
        let panel = if self.in_app_goto_open {
            PlayerPanel::None
        } else {
            PlayerPanel::Goto
        };
        self.open_player_panel(panel);
        self.show_in_app_osd();
    }

    pub fn submit_in_app_goto(&mut self) {
        let text = self.in_app_goto.text.trim().to_string();
        let Some(target) = parse_timecode(&text) else {
            self.show_toast("Use mm:ss or h:mm:ss.");
            return;
        };
        self.seek_in_app_to(target);
        self.in_app_goto_open = false;
    }

    pub fn toggle_quality_menu(&mut self) {
        let panel = if self.in_app_quality_open {
            PlayerPanel::None
        } else {
            PlayerPanel::Quality
        };
        self.open_player_panel(panel);
        self.show_in_app_osd();
    }

    pub fn toggle_sleep_menu(&mut self) {
        let panel = if self.in_app_sleep_open {
            PlayerPanel::None
        } else {
            PlayerPanel::Sleep
        };
        self.open_player_panel(panel);
        self.show_in_app_osd();
    }

    /// Adjust the starting volume from Settings, in 10 % steps, without a player to talk to.
    pub fn adjust_preferred_volume(&mut self, delta: f64) {
        self.in_app_volume = (self.in_app_volume + delta).clamp(0.0, 2.0);
    }

    /// Ask the LAN who is out there. The handshake blocks for under a second, so it runs on a
    /// worker like every other request.
    /// Show a short confirmation over the page. It disappears on its own; no click needed.
    pub fn show_toast(&mut self, message: impl Into<String>) {
        self.toast = Some(message.into());
        self.toast_undo = None;
        self.toast_until = Some(Instant::now() + Duration::from_millis(2_500));
    }

    /// A toast with an Undo action stays up long enough to use it.
    pub fn show_undo_toast(&mut self, message: impl Into<String>, undo: UndoAction) {
        self.toast = Some(message.into());
        self.toast_undo = Some(undo);
        self.toast_until = Some(Instant::now() + Duration::from_secs(8));
    }

    pub fn discover_servers(&mut self) {
        if self.discovering {
            return;
        }
        self.discovering = true;
        self.discovered.clear();
        self.discovery_load.request((), |()| {
            crate::discovery::discover(std::time::Duration::from_millis(900))
        });
    }

    /// Fill the Server field from a discovered server. The list stays, so a second answer is
    /// one click away.
    pub fn use_discovered(&mut self, address: String) {
        self.server = TextEdit::new(address);
        self.server.focused = false;
        self.step = Step::Idle;
    }

    pub fn cycle_subtitle_background(&mut self) {
        self.subtitle_background = self.subtitle_background.next();
    }

    /// Open a provider page in the user's browser. Only http(s) is accepted; a server string is
    /// untrusted enough that a `file:` or shell scheme must never reach the opener.
    pub fn open_url(url: &str) -> Result<(), String> {
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err("Refusing to open a non-web trailer URL.".into());
        }
        let mut command = if cfg!(target_os = "macos") {
            std::process::Command::new("open")
        } else if cfg!(target_os = "windows") {
            let mut command = std::process::Command::new("cmd");
            command.args(["/C", "start", ""]);
            command
        } else {
            std::process::Command::new("xdg-open")
        };
        command
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("Could not open the browser: {error}"))
    }

    pub fn cycle_subtitle_font(&mut self) {
        self.subtitle_font = self.subtitle_font.next();
    }

    pub fn cycle_subtitle_color(&mut self) {
        self.subtitle_color = self.subtitle_color.next();
    }

    pub fn cycle_subtitle_outline(&mut self) {
        self.subtitle_outline = self.subtitle_outline.next();
    }

    pub fn cycle_subtitle_position(&mut self) {
        self.subtitle_position = self.subtitle_position.next();
    }

    pub fn cycle_subtitle_style(&mut self) {
        self.subtitle_style = self.subtitle_style.next();
    }

    pub fn adjust_subtitle_opacity(&mut self, delta: f32) {
        self.subtitle_opacity = (self.subtitle_opacity + delta).clamp(0.2, 1.0);
    }

    pub fn adjust_subtitle_background_opacity(&mut self, delta: f32) {
        self.subtitle_background_opacity =
            (self.subtitle_background_opacity + delta).clamp(0.0, 1.0);
    }

    pub fn cycle_subtitle_language(&mut self) {
        self.subtitle_language = next_language(&self.subtitle_language);
    }

    pub fn cycle_audio_language(&mut self) {
        self.audio_language = next_language(&self.audio_language);
    }

    /// The label the settings row shows for a language code.
    pub fn language_label(code: &str) -> &'static str {
        Self::PREFERRED_LANGUAGES
            .iter()
            .find(|(candidate, _)| *candidate == code)
            .map(|(_, label)| *label)
            .unwrap_or("Any language")
    }

    /// Copy the session-local subtitle preferences into the config when the window closes.
    /// Called from `on_close`, not from a control handler: saving on every stepper click would
    /// put a file write on the UI thread.
    pub fn save_preferences(&mut self) {
        let omdb = self.omdb_key.text.trim().to_string();
        self.config.omdb_api_key = if omdb.is_empty() { None } else { Some(omdb) };
        self.apply_seerr_buffers();
        if let Some(probe) = &self.seerr.probe {
            self.config.seerr_user_name = Some(probe.user.clone());
        }
        self.config.subtitle_size = self.in_app_subtitles.size;
        self.config.subtitle_delay_ms = self.in_app_subtitles.delay_ms;
        self.config.autoplay_next = Some(self.autoplay_next);
        self.config.subtitle_mode = Some(self.subtitle_mode);
        self.config.skip_segments = Some(self.skip_mode);
        self.config.still_watching = Some(self.still_watching);
        self.config.still_watching_episodes = Some(self.still_watching_episodes);
        self.config.still_watching_minutes = Some(self.still_watching_minutes);
        self.config.resume_rewind_seconds = Some(self.resume_rewind_seconds);
        self.config.downmix_stereo = Some(self.downmix_stereo);
        self.config.audio_delay_ms = Some(self.audio_delay_ms);
        self.config.audio_mode = Some(self.audio_mode);
        self.config.reduce_motion = Some(self.reduce_motion);
        self.config.version_choices = Some(self.version_choices.clone());
        self.config.playback_quality = Some(self.playback_quality);
        self.config.quality_choices = Some(self.quality_choices.clone());
        self.config.hidden_home = Some(self.hidden_home.iter().cloned().collect());
        self.config.spoiler_guard = Some(self.spoiler_guard);
        self.config.saved_views = Some(self.saved_views.clone());
        self.config.home_hero = Some(self.home_hero);
        self.config.light_theme = Some(self.light_theme);
        self.config.high_contrast = Some(self.high_contrast);
        self.config.text_scale = Some(self.text_scale);
        self.config.live_favorites = Some(self.live.favorites.iter().cloned().collect());
        self.config.live_reminders = Some(self.live_reminders.clone());
        self.config.notifications = Some(self.notifications);
        self.config.shortcuts = Some(self.key_bindings.clone());
        self.config.subtitle_background = Some(self.subtitle_background);
        self.config.subtitle_font = Some(self.subtitle_font);
        self.config.subtitle_color = Some(self.subtitle_color);
        self.config.subtitle_outline = Some(self.subtitle_outline);
        self.config.subtitle_position = Some(self.subtitle_position);
        self.config.subtitle_style = Some(self.subtitle_style);
        self.config.subtitle_opacity = Some(self.subtitle_opacity);
        self.config.subtitle_background_opacity = Some(self.subtitle_background_opacity);
        self.config.subtitle_language = Some(self.subtitle_language.clone());
        self.config.audio_language = Some(self.audio_language.clone());
        self.config.seek_seconds = Some(self.seek_step_ms / 1000);
        self.config.volume = Some(self.in_app_volume);
        if self.window_size.0 >= 1.0 && self.window_size.1 >= 1.0 {
            self.config.window_width = self.window_size.0;
            self.config.window_height = self.window_size.1;
            // Negative origins are a left-hand monitor; restoring them blindly can put the
            // window off-screen after that monitor is unplugged. Positive origins only.
            if self.window_position.0 >= 0.0 && self.window_position.1 >= 0.0 {
                self.config.window_x = Some(self.window_position.0.round() as i32);
                self.config.window_y = Some(self.window_position.1.round() as i32);
            }
        }
        if self.persist {
            let _ = self.config.save();
        }
    }

    pub fn setup(&mut self, waker: rune::Waker) {
        // Disk work belongs before the first frame, not in a handler. Headless tests never call
        // setup, so fixtures stay hermetic.
        self.checkpoints = crate::checkpoint::load();
        self.history = crate::history::load(50);
        self.start_tray(&waker);
        self.history_ms = self.history.iter().map(|entry| entry.played_ms).sum();
        self.waker = Some(waker.clone());
        self.worker.set_waker(waker.clone());
        self.home.set_waker(waker.clone());
        self.queue_load.set_waker(waker.clone());
        self.playlist_remove_load.set_waker(waker.clone());
        self.playlists_load.set_waker(waker.clone());
        self.playlist_add_load.set_waker(waker.clone());
        self.collection_choices_load.set_waker(waker.clone());
        self.collection_add_load.set_waker(waker.clone());
        self.playlist_create_load.set_waker(waker.clone());
        self.home_next_up.set_waker(waker.clone());
        self.home_on_now.set_waker(waker.clone());
        self.home_recently_played.set_waker(waker.clone());
        self.home_collections.set_waker(waker.clone());
        self.library_page.set_waker(waker.clone());
        self.library_genres.set_waker(waker.clone());
        self.person_latest.set_waker(waker.clone());
        self.seerr_person_load.set_waker(waker.clone());
        self.cast_sessions_load.set_waker(waker.clone());
        self.cast_send_load.set_waker(waker.clone());
        self.discovery_load.set_waker(waker.clone());
        self.detail_page.set_waker(waker.clone());
        self.detail_similar_load.set_waker(waker.clone());
        self.detail_imdb_load.set_waker(waker.clone());
        self.detail_extras_load.set_waker(waker.clone());
        self.detail_collections_load.set_waker(waker.clone());
        self.detail_theme_load.set_waker(waker.clone());
        self.theme_video_load.set_waker(waker.clone());
        self.theme_audio_load.set_waker(waker.clone());
        self.imdb_ratings_load.set_waker(waker.clone());
        self.detail_episodes.set_waker(waker.clone());
        self.playback_start.set_waker(waker.clone());
        self.search_latest.set_waker(waker.clone());
        self.search_hints.set_waker(waker.clone());
        self.remote_latest.set_waker(waker.clone());
        self.seerr_probe.set_waker(waker.clone());
        self.seerr_requests.set_waker(waker.clone());
        self.seerr_counts.set_waker(waker.clone());
        self.seerr_summaries.set_waker(waker.clone());
        self.seerr_discover.set_waker(waker.clone());
        self.seerr_search.set_waker(waker.clone());
        self.seerr_detail_load.set_waker(waker.clone());
        self.seerr_series_load.set_waker(waker.clone());
        self.seerr_library_load.set_waker(waker.clone());
        self.seerr_genres.set_waker(waker.clone());
        self.seerr_browse.set_waker(waker.clone());
        self.seerr_search_global.set_waker(waker.clone());
        self.quick_start.set_waker(waker.clone());
        self.quick_poll.set_waker(waker.clone());
        self.quick_auth.set_waker(waker.clone());
        self.live_latest.set_waker(waker.clone());
        self.live_open.set_waker(waker.clone());
        self.live_close.set_waker(waker.clone());
        self.in_app_start.set_waker(waker.clone());
        self.trailer_load.set_waker(waker.clone());
        self.in_app_segments_load.set_waker(waker.clone());
        self.in_app_subtitle_search_load.set_waker(waker.clone());
        self.in_app_subtitle_download_load.set_waker(waker.clone());
        self.in_app_subtitle_load.set_waker(waker.clone());
        self.in_app_image_load.set_waker(waker.clone());
        self.in_app_streams_load.set_waker(waker.clone());
        self.in_app_neighbours_load.set_waker(waker.clone());
        self.fetch_home = true;
        if !std::env::args().any(|arg| arg == "--snap" || arg == "--tree") {
            self.mpris = crate::mpris::Mpris::start();
        }
        let dirty = Arc::clone(&self.image_dirty);
        let image_waker: rune::Waker = {
            let waker = waker.clone();
            Arc::new(move || {
                dirty.store(true, Ordering::Relaxed);
                waker();
            })
        };
        self.image_cache = Some(ImageCache::new(image_waker));
        if self.is_signed_in() {
            self.home_started = true;
            self.load_home();
            if self.seerr_configured() {
                self.refresh_seerr_badge();
                self.test_seerr();
            }
            // Start the restored page beside Home instead of waiting for Home to paint.
            self.restore_pending_place();
            // `setup` runs before the window's wake lane is connected. A fast answer would
            // otherwise land with nowhere to go; ring once after the first frame is up.
            let waker = waker.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(300));
                waker();
            });
        }
    }

    /// Drop a restored place before a developer live capture picks its own route.
    pub fn forget_last_place(&mut self) {
        self.pending_place = None;
        self.config.last_place = None;
    }

    /// Reopen the last library/detail. Called from setup once the session is known and from
    /// poll as a fallback for states built without setup.
    fn restore_pending_place(&mut self) -> bool {
        let Some(place) = self.pending_place.take() else {
            return false;
        };
        match place.kind.as_str() {
            "library" => {
                if let (Some(id), Some(name)) = (place.id, place.name) {
                    self.open_library(id, name, place.collection_type);
                    self.show_toast("Returned to where you left off.");
                    return true;
                }
            }
            "detail" => {
                if let Some(id) = place.id {
                    self.open_detail(id);
                    self.show_toast("Returned to where you left off.");
                    return true;
                }
            }
            "livetv" => {
                self.open_live_tv();
                self.show_toast("Returned to where you left off.");
                return true;
            }
            _ => {}
        }
        false
    }

    /// `poll` asks this every frame; it returns true only when something visible changed.
    pub fn poll(&mut self) -> bool {
        // A route change makes the previous screen's images evictable; the new screen's
        // request pass repopulates the protected set.
        if self.route != self.last_route {
            self.current_images.clear();
            self.last_route = self.route.clone();
        }
        let mut booted = false;
        if self.fetch_home && !self.home_started && self.is_signed_in() {
            self.home_started = true;
            self.load_home();
            booted = true;
        }
        let mut revealed = false;
        self.image_fades.retain(|_, started| {
            if started.elapsed() >= Duration::from_millis(120) {
                revealed = true;
                false
            } else {
                true
            }
        });
        let messages = self.worker.drain();
        let mut changed = booted || revealed || !messages.is_empty();
        let tray_events: Vec<rune::TrayEvent> = self
            .tray_events
            .lock()
            .map(|mut queue| queue.drain(..).collect())
            .unwrap_or_default();
        for event in tray_events {
            match event {
                rune::TrayEvent::Menu(id) => {
                    changed |= match id.as_str() {
                        "playpause" if self.in_app.is_some() => {
                            self.toggle_in_app_pause();
                            true
                        }
                        "stop" if self.in_app.is_some() => {
                            self.stop_in_app();
                            true
                        }
                        "next" if self.in_app.is_some() => {
                            self.play_in_app_next_episode();
                            true
                        }
                        "previous" if self.in_app.is_some() => {
                            self.play_in_app_previous();
                            true
                        }
                        _ => false,
                    };
                }
                rune::TrayEvent::Activate { .. } => {}
                _ => {}
            }
        }
        for message in messages {
            self.apply(message);
        }
        if let Some(load) = self.home.take() {
            self.apply_home(load);
            changed = true;
        }
        if let Some((items, shuffle)) = self.queue_load.take() {
            if !items.is_empty() {
                let at = if shuffle {
                    0
                } else {
                    crate::state::in_app_state::first_unplayed(&items)
                };
                self.play_queue(items, at, shuffle);
            }
            changed = true;
        }
        if let Some(added) = self.playlist_add_load.take() {
            self.apply_playlist_add(added);
            changed = true;
        }
        if let Some(choices) = self.collection_choices_load.take() {
            self.collection_choices_loading = false;
            self.collection_choices = choices.unwrap_or_default();
            changed = true;
        }
        if let Some(added) = self.collection_add_load.take() {
            self.apply_collection_add(added);
            changed = true;
        }
        if let Some(created) = self.playlist_create_load.take() {
            self.apply_playlist_create(created);
            changed = true;
        }
        if let Some(removed) = self.playlist_remove_load.take() {
            self.apply_playlist_remove(removed);
            changed = true;
        }
        if let Some(playlists) = self.playlists_load.take()
            && self.is_signed_in()
        {
            self.playlists = playlists.unwrap_or_default();
            changed = true;
        }
        if let Some(next_up) = self.home_next_up.take() {
            if self.is_signed_in() {
                self.home_data.next_up = next_up;
                self.request_images();
            }
            changed = true;
        }
        if let Some(on_now) = self.home_on_now.take() {
            if self.is_signed_in() {
                self.home_data.on_now = on_now;
                self.request_images();
            }
            changed = true;
        }
        if let Some(played) = self.home_recently_played.take() {
            if self.is_signed_in() {
                self.home_data.recently_played =
                    crate::home::merge_recently_played(played, &self.home_data.resume);
                self.request_images();
            }
            changed = true;
        }
        if let Some(collections) = self.home_collections.take() {
            if self.is_signed_in() {
                self.home_data.collections = collections;
                self.request_images();
            }
            changed = true;
        }
        if let Some(page) = self.library_page.take() {
            self.apply_library_page(page);
            changed = true;
        }
        if let Some(genres) = self.library_genres.take() {
            self.apply_library_genres(genres);
            changed = true;
        }
        if self.last_reminder_check.elapsed() >= Duration::from_secs(1) {
            self.last_reminder_check = Instant::now();
            if self.poll_live_reminders() {
                changed = true;
            }
        }
        if self
            .toast_until
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.toast = None;
            self.toast_undo = None;
            self.toast_until = None;
            changed = true;
        }
        if let Some(sessions) = self.cast_sessions_load.take() {
            self.cast_loading = false;
            match sessions {
                Ok(sessions) => self.cast_sessions = sessions,
                Err(message) => {
                    self.cast_sessions.clear();
                    self.show_toast(message);
                }
            }
            changed = true;
        }
        if let Some(started) = self.theme_audio_load.take() {
            let wanted = self.theme_audio_starting;
            self.theme_audio_starting = false;
            match started {
                Ok(player) if !wanted => player.stop(),
                Ok(player) => {
                    if let Some(old) = self.theme_audio.replace(player) {
                        old.stop();
                    }
                    self.theme_audio_title = self
                        .detail_theme_songs
                        .first()
                        .map(|item| item.name.clone());
                }
                Err(message) => self.show_toast(format!("Theme song: {message}")),
            }
            changed = true;
        }
        if let Some(started) = self.theme_video_load.take() {
            let wanted = self.theme_video_starting;
            self.theme_video_starting = false;
            match started {
                Ok(player) if wanted && !self.reduce_motion => {
                    if let Some(old) = self.theme_video.replace(player) {
                        old.stop();
                    }
                    if let Some(player) = &self.theme_video {
                        player.toggle_mute();
                    }
                    self.theme_video_on = true;
                }
                Ok(player) => player.stop(),
                Err(message) => self.show_toast(format!("Theme video: {message}")),
            }
            changed = true;
        }
        if let Some(sent) = self.cast_send_load.take() {
            match sent {
                Ok(()) => {
                    if self.transfer_after_cast {
                        self.transfer_after_cast = false;
                        self.stop_in_app();
                        self.show_toast("Continuing on the other device.");
                    } else {
                        self.show_toast("Sent to the other device.");
                    }
                }
                Err(message) => {
                    self.transfer_after_cast = false;
                    self.show_toast(format!("Could not send it: {message}"));
                }
            }
            changed = true;
        }
        if let Some(servers) = self.discovery_load.take() {
            self.discovered = servers;
            self.discovering = false;
            changed = true;
        }
        if let Some(hints) = self.search_hints.take() {
            self.search.hints_loading = false;
            self.search.hints = hints.unwrap_or_default();
            changed = true;
        }
        if let Some(page) = self.detail_page.take() {
            self.apply_detail_page(page);
            changed = true;
        }
        if let Some(theme) = self.detail_theme_load.take() {
            let keep = self.detail.as_ref().is_some_and(|detail| {
                self.detail_theme_for.as_deref() == Some(detail.item.id.as_str())
            });
            if keep {
                match theme {
                    Ok(theme) => {
                        self.detail_theme_songs = theme
                            .theme_songs_result
                            .map(|result| result.items)
                            .unwrap_or_default();
                        self.detail_theme_videos = theme
                            .theme_videos_result
                            .map(|result| result.items)
                            .unwrap_or_default();
                    }
                    Err(_) => {
                        self.detail_theme_songs.clear();
                        self.detail_theme_videos.clear();
                    }
                }
            }
            changed = true;
        }
        if let Some(collections) = self.detail_collections_load.take() {
            let keep = self.detail.as_ref().is_some_and(|detail| {
                self.detail_collections_for.as_deref() == Some(detail.item.id.as_str())
            });
            if keep {
                self.detail_collections = collections.unwrap_or_default();
            }
            changed = true;
        }
        if let Some(extras) = self.detail_extras_load.take() {
            let keep = self.detail.as_ref().is_some_and(|detail| {
                self.detail_extras_for.as_deref() == Some(detail.item.id.as_str())
            });
            if keep {
                self.detail_extras = extras.unwrap_or_default();
            }
            changed = true;
        }
        if let Some(rating) = self.detail_imdb_load.take() {
            self.apply_imdb_rating(rating);
            changed = true;
        }
        if let Some(trailer) = self.trailer_load.take() {
            self.apply_trailer_download(trailer);
            changed = true;
        }
        if let Some(ratings) = self.imdb_ratings_load.take() {
            self.apply_imdb_ratings(ratings);
            changed = true;
        }
        if let Some(similar) = self.detail_similar_load.take() {
            let belongs = self.detail_similar_for.take().is_some_and(|id| {
                self.detail
                    .as_ref()
                    .is_some_and(|detail| detail.item.id == id)
            });
            if belongs
                && self.is_signed_in()
                && let Some(detail) = &mut self.detail
            {
                detail.similar = similar.unwrap_or_default();
                self.request_detail_images();
            }
            changed = true;
        }
        if let Some(load) = self.person_latest.take() {
            self.apply_person(load);
            changed = true;
        }
        if let Some(load) = self.seerr_person_load.take() {
            self.apply_seerr_person(load);
            changed = true;
        }
        if let Some(page) = self.detail_episodes.take() {
            self.apply_episodes_page(page);
            changed = true;
        }
        if let Some(started) = self.playback_start.take() {
            self.apply_playback_start(started);
            changed = true;
        }
        if let Some(result) = self.seerr_search_global.take() {
            self.apply_search_seerr(result);
            changed = true;
        }
        if let Some(search) = self.search_latest.take() {
            self.apply_search(search);
            changed = true;
        }
        if let Some(probe) = self.seerr_probe.take() {
            self.apply_seerr_probe(probe);
            changed = true;
        }
        if self.poll_seerr() {
            changed = true;
        }
        if let Some(remote) = self.remote_latest.take() {
            self.apply_remote(remote);
            changed = true;
        }
        if let Some(channels) = self.live_latest.take() {
            self.apply_live_channels(channels);
            changed = true;
        }
        if let Some(started) = self.quick_start.take() {
            self.apply_quick_start(started);
            changed = true;
        }
        if let Some(polled) = self.quick_poll.take() {
            self.apply_quick_poll(polled);
            changed = true;
        }
        if let Some(auth) = self.quick_auth.take() {
            self.apply_quick_auth(auth);
            changed = true;
        }
        if self.poll_quick() {
            changed = true;
        }
        if let Some(opened) = self.live_open.take() {
            self.apply_live_open(opened);
            changed = true;
        }
        // Closing a tuner is fire-and-forget for a stop, but a zap is waiting for the close to
        // land before it opens the next stream: the server counts tuners, not channels.
        if self.live_close.take().is_some()
            && let Some(channel) = self.pending_zap.take()
        {
            self.play_live_channel(channel);
            changed = true;
        }
        if let Some(started) = self.in_app_start.take() {
            self.apply_in_app_start(started);
            changed = true;
        }
        if let Some(segments) = self.in_app_segments_load.take() {
            self.apply_in_app_segments(segments);
            changed = true;
        }
        if let Some(streams) = self.in_app_streams_load.take() {
            self.apply_in_app_playback_streams(streams);
            changed = true;
        }
        if let Some(found) = self.in_app_subtitle_search_load.take() {
            self.apply_in_app_subtitle_search(found);
            changed = true;
        }
        if let Some(downloaded) = self.in_app_subtitle_download_load.take() {
            self.apply_in_app_subtitle_download(downloaded);
            changed = true;
        }
        if let Some(cues) = self.in_app_subtitle_load.take() {
            self.apply_in_app_subtitle_cues(cues);
            changed = true;
        }
        if let Some(image) = self.in_app_image_load.take() {
            self.apply_in_app_image_subtitle_style(image);
            changed = true;
        }
        if let Some(neighbours) = self.in_app_neighbours_load.take() {
            self.apply_in_app_neighbours(neighbours);
            changed = true;
        }
        if self.poll_in_app() {
            changed = true;
        }
        if self.sync_image_subtitles() {
            changed = true;
        }
        if self.home_loaded && self.in_app.is_none() && self.restore_pending_place() {
            changed = true;
        }
        if self.image_dirty.swap(false, Ordering::Relaxed) {
            changed = true;
        }
        let tooltip = self
            .in_app
            .as_ref()
            .map(|session| session.item.name.clone())
            .unwrap_or_else(|| "Runefin".into());
        if tooltip != self.tray_tooltip
            && let Some(tray) = &self.tray
        {
            tray.set_tooltip(tooltip.clone());
            self.tray_tooltip = tooltip;
        }
        if self.sync_mpris() {
            changed = true;
        }
        changed
    }

    pub fn apply(&mut self, msg: Msg) {
        match msg {
            Msg::Connected(Ok(session)) => self.apply_session(session),
            Msg::Connected(Err(failure)) => self.step = Step::Failed(failure),
            Msg::Image {
                key,
                bytes,
                placeholder,
            } => match (bytes, &self.image_cache) {
                (Ok(bytes), Some(cache)) => {
                    let image = cache.load(bytes);
                    let first = self.images.insert(key.clone(), image).is_none();
                    if let Some(rgba) = placeholder {
                        let pixels = Pixels::new();
                        pixels.set(
                            crate::image::PLACEHOLDER_WIDTH,
                            crate::image::PLACEHOLDER_HEIGHT,
                            rgba,
                        );
                        self.placeholders.insert(key.clone(), pixels);
                    }
                    if first {
                        self.image_order.push_back(key.clone());
                        if self.placeholders.contains_key(&key) {
                            self.image_fades.insert(key, Instant::now());
                        }
                    }
                    // Evict around the current screen: skip a key the screen is showing and
                    // take the next oldest. If everything left is current, the oldest goes.
                    let mut scanned = 0usize;
                    while self.images.len() > IMAGE_CACHE_LIMIT {
                        let Some(old) = self.image_order.pop_front() else {
                            break;
                        };
                        if self.current_images.contains(&old) && scanned < self.image_order.len() {
                            self.image_order.push_back(old);
                            scanned += 1;
                            continue;
                        }
                        scanned = 0;
                        if self.images.remove(&old).is_some() {
                            // A later visible-row pass may ask for it again.
                            self.requested_images.remove(&old);
                            self.current_images.remove(&old);
                            self.placeholders.remove(&old);
                            self.image_fades.remove(&old);
                        }
                    }
                }
                // Let the next visible/page pass retry a failed fetch.
                (Err(_), _) => {
                    self.requested_images.remove(&key);
                }
                _ => {}
            },
            Msg::Playback { generation, event } => self.on_playback_event(generation, event),
            Msg::Reported { kind, result } => self.on_reported(kind, result),
            Msg::Remote(result) => self.remote_command_finished(result),
            Msg::SeerrRequested(result) => {
                self.apply_seerr_requested(result.map(|request| *request))
            }
            Msg::Favorite(Ok(())) => {}
            Msg::Favorite(Err(message)) => {
                self.detail_notice = Some(format!("Could not update favourite: {message}"));
            }
            Msg::Played(Ok(())) => {}
            Msg::Played(Err(message)) => {
                self.detail_notice = Some(format!("Could not update watched state: {message}"));
            }
            Msg::Rated(Ok(())) => {}
            Msg::Rated(Err(message)) => {
                self.show_toast(format!("Could not save the rating: {message}"));
            }
            Msg::PlaylistOrder(Ok(())) => {}
            Msg::PlaylistAdded(Err(message)) => {
                self.show_toast(format!("Could not restore the entry: {message}"));
            }
            Msg::PlaylistAdded(Ok(())) => {}
            Msg::Collection(Ok(())) => {}
            Msg::Collection(Err(message)) => {
                self.show_toast(format!("Collection: {message}"));
            }
            Msg::ResumeCleared(Ok(())) => {}
            Msg::ResumeCleared(Err(message)) => {
                self.show_toast(format!(
                    "Could not remove it from Continue Watching: {message}"
                ));
            }
            Msg::PlaylistOrder(Err(message)) => {
                // The optimistic order is now wrong: re-read the server's.
                if let Some(library) = &mut self.library {
                    library.notice = Some(format!("Could not reorder the playlist: {message}"));
                }
                self.load_library_page(true);
            }
            Msg::ImagesCleared(result) => {
                self.images.clear();
                self.placeholders.clear();
                self.image_order.clear();
                self.image_fades.clear();
                self.requested_images.clear();
                self.image_notice = Some(match result {
                    Ok(count) => format!("Removed {count} cached poster(s)."),
                    Err(error) => format!("Could not clear posters: {error}"),
                });
            }
        }
    }

    /// Ask the server for the home screen. `Latest` cancels an older request, so a retry after a
    /// failure cannot be overtaken by the failure it replaces.
    pub fn load_home(&mut self) {
        if !self.fetch_home || !self.is_signed_in() {
            return;
        }
        let Some(client) = self.client() else {
            return;
        };
        if self.home_data.is_empty() {
            self.home_loaded = false;
        }
        self.home_error = None;
        let now = crate::home::now_days();
        let next_client = client.clone();
        let on_now_client = client.clone();
        let played_client = client.clone();
        let collections_client = client.clone();
        self.home
            .request((), move |()| match crate::home::load(&client, now) {
                Ok(data) => HomeLoad::Ready(data),
                Err(message) => HomeLoad::Failed(message),
            });
        // In parallel with the rest: this query is the slowest one on a large library, and it
        // only adds a rail, so it must not hold the first paint.
        self.home_next_up
            .request((), move |()| crate::home::load_next_up(&next_client));
        self.home_on_now
            .request((), move |()| crate::home::load_on_now(&on_now_client));
        self.home_recently_played.request((), move |()| {
            crate::home::load_recently_played(&played_client)
        });
        self.home_collections.request((), move |()| {
            crate::home::load_collections(&collections_client)
        });
    }

    fn apply_home(&mut self, load: HomeLoad) {
        match load {
            HomeLoad::Ready(data) => {
                let played = std::mem::take(&mut self.home_data.recently_played);
                self.home_data = data;
                // If Recently Played arrived first, re-apply the Continue Watching dedupe now
                // that the fresh resume list is known.
                self.home_data.recently_played =
                    crate::home::merge_recently_played(played, &self.home_data.resume);
                self.home_loaded = true;
                self.home_error = None;
                self.request_images();
            }
            HomeLoad::Failed(message) => {
                // A revoked token must not leave a dead Home behind a Retry button. The API's
                // 401 message is the one place this distinction survives the worker boundary.
                if message.contains("session expired") {
                    self.sign_out();
                    self.step = Step::Failed(Failure {
                        stage: Stage::Credentials,
                        message,
                    });
                    return;
                }
                self.home_loaded = true;
                self.home_error = Some(message);
            }
        }
    }

    /// Open a library from the sidebar. The first page starts immediately; the viewport returns
    /// to the position it had the last time this library was open.
    pub fn open_library(&mut self, id: String, name: String, collection_type: Option<String>) {
        self.capture_library_scroll();
        self.stop_theme_media();
        self.detail_stack.clear();
        self.library_genres.cancel();
        self.library_search = rune::TextEdit::new("");
        self.library_search_due = None;
        self.library = Some(LibraryState::new(id.clone(), name, collection_type));
        self.route = Route::Library { id: id.clone() };
        self.remember_place();
        if let Some(y) = self.library_scrolls.get(&id).copied()
            && let Some(library) = &mut self.library
        {
            library.set_scroll_y(y);
            self.pending_scroll = Some(y);
        }
        self.load_library_page(true);
    }

    /// Back to the home screen, remembering where the library was scrolled to.
    pub fn open_shelf(&mut self, shelf: HomeShelf) {
        self.shelf = Some(shelf);
    }

    pub fn close_shelf(&mut self) {
        self.shelf = None;
    }

    pub fn go_home(&mut self) {
        if self.in_app.is_some() && !self.in_app_minimized {
            self.stop_in_app();
        }
        self.capture_library_scroll();
        self.stop_theme_media();
        self.route = Route::Home;
        self.remember_place();
        self.shelf = None;
        self.library = None;
        self.library_selecting = false;
        self.library_selected.clear();
        self.library_page.cancel();
        self.library_genres.cancel();
        self.detail_page.cancel();
        self.detail_episodes.cancel();
        self.detail = None;
        self.person = None;
        self.person_detail = None;
        self.person_latest.cancel();
        self.detail_return = Route::Home;
        self.detail_stack.clear();
        self.detail_notice = None;
        self.detail_overview_open = false;
        self.search.query.focused = false;
        self.search_latest.cancel();
        self.pending_scroll = None;
        self.request_images();
    }

    fn capture_library_scroll(&mut self) {
        if let Some(library) = &self.library {
            self.library_scrolls
                .insert(library.id.clone(), library.scroll_y());
        }
    }

    /// The offset `main::poll` should apply once the library screen has laid out. While the
    /// first page is still loading the viewport does not exist, so the request waits rather than
    /// scrolling an empty screen and being forgotten.
    /// F11 and the OSD's fullscreen chip. The window itself is asked in `poll`, which holds
    /// the `Tx`.
    pub fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
        self.pending_fullscreen = Some(self.fullscreen);
    }

    pub fn take_pending_scroll(&mut self) -> Option<f32> {
        if self
            .library
            .as_ref()
            .is_some_and(|library| library.items.is_empty())
        {
            return None;
        }
        self.pending_scroll.take()
    }

    pub fn toggle_library_selecting(&mut self) {
        self.library_selecting = !self.library_selecting;
        if !self.library_selecting {
            self.library_selected.clear();
        }
    }

    pub fn toggle_library_selected(&mut self, id: &str) {
        if !self.library_selected.remove(id) {
            self.library_selected.insert(id.to_string());
        }
    }

    pub fn clear_library_selection(&mut self) {
        self.library_selected.clear();
    }

    /// Bulk watched. One server write per item, off-thread; the visible cards change at once.
    pub fn mark_selected_watched(&mut self) {
        let ids: Vec<String> = self.library_selected.iter().cloned().collect();
        if ids.is_empty() {
            return;
        }
        if let Some(client) = self.client() {
            for id in &ids {
                self.worker.set_played(client.clone(), id.clone(), true);
            }
        }
        for id in &ids {
            self.update_item(id, |item| {
                item.user_data
                    .get_or_insert_with(jellyfin_api::UserData::default)
                    .played = true;
            });
        }
        self.library_selected.clear();
        self.show_toast(format!("Marked {} item(s) watched.", ids.len()));
    }

    pub fn set_sort(&mut self, sort: SortMode) {
        let changed = match &mut self.library {
            Some(library) if library.sort != sort => {
                library.sort = sort;
                library.ascending = !sort.descending();
                library.letter = None;
                library.set_scroll_y(0.0);
                true
            }
            _ => false,
        };
        if changed {
            self.load_library_page(true);
            self.pending_scroll = Some(0.0);
        }
    }

    /// Flip the sort direction in place. Re-reading from the first page is required: the
    /// server's order is the only order, so a client-side reverse would be wrong past page one.
    pub fn toggle_sort_direction(&mut self) {
        let changed = match &mut self.library {
            Some(library) => {
                library.ascending = !library.ascending;
                library.letter = None;
                library.set_scroll_y(0.0);
                true
            }
            None => false,
        };
        if changed {
            self.load_library_page(true);
            self.pending_scroll = Some(0.0);
        }
    }

    pub fn set_view(&mut self, view: ViewMode) {
        if let Some(library) = &mut self.library {
            library.view = view;
        }
    }

    pub fn toggle_unwatched(&mut self) {
        let changed = match &mut self.library {
            Some(library) => {
                library.unwatched = !library.unwatched;
                library.set_scroll_y(0.0);
                true
            }
            None => false,
        };
        if changed {
            self.load_library_page(true);
            self.pending_scroll = Some(0.0);
        }
    }

    /// The filter/sort sheet: everything the toolbar does not fit, in one place.
    pub fn adjust_library_year(&mut self, delta: i32) {
        let changed = match &mut self.library {
            Some(library) => {
                library.year = match (library.year, delta) {
                    (None, d) if d < 0 => Some(2026),
                    (None, _) => None,
                    (Some(year), d) => {
                        let next = year + d;
                        (1990..=2026).contains(&next).then_some(next)
                    }
                };
                library.set_scroll_y(0.0);
                true
            }
            None => false,
        };
        if changed {
            self.load_library_page(true);
            self.pending_scroll = Some(0.0);
        }
    }

    pub fn cycle_library_rating(&mut self) {
        let changed = match &mut self.library {
            Some(library) => {
                library.min_rating = match library.min_rating {
                    None => Some(7),
                    Some(7) => Some(8),
                    Some(8) => Some(9),
                    _ => None,
                };
                library.set_scroll_y(0.0);
                true
            }
            None => false,
        };
        if changed {
            self.load_library_page(true);
            self.pending_scroll = Some(0.0);
        }
    }

    pub fn cycle_library_audio_language(&mut self) {
        self.cycle_language(true);
    }

    pub fn cycle_library_subtitle_language(&mut self) {
        self.cycle_language(false);
    }

    fn cycle_language(&mut self, audio: bool) {
        const LANGUAGES: [&str; 5] = ["eng", "spa", "fre", "ger", "jpn"];
        let changed = match &mut self.library {
            Some(library) => {
                let slot = if audio {
                    &mut library.audio_language
                } else {
                    &mut library.subtitle_language
                };
                *slot = match slot.as_deref() {
                    None => Some(LANGUAGES[0].to_string()),
                    Some(current) => {
                        let at = LANGUAGES.iter().position(|code| *code == current);
                        at.and_then(|at| LANGUAGES.get(at + 1))
                            .map(|code| (*code).to_string())
                    }
                };
                library.set_scroll_y(0.0);
                true
            }
            None => false,
        };
        if changed {
            self.load_library_page(true);
            self.pending_scroll = Some(0.0);
        }
    }

    /// Type-ahead is debounced: one request after the reader stops, not one per key.
    /// Create one server-owned collection from the current multi-select. The name is a
    /// placeholder the reader can rename in Jellyfin; there is no native rename here.
    pub fn create_collection_from_selection(&mut self) {
        let ids: Vec<String> = self.library_selected.iter().cloned().collect();
        if ids.is_empty() {
            return;
        }
        let Some(client) = self.client() else {
            return;
        };
        let name = format!("Runefin collection {}", self.saved_views.len() + 1);
        self.worker
            .create_collection(client, name.clone(), ids.clone());
        self.library_selected.clear();
        self.show_toast(format!("Creating {name}."));
    }

    /// Remove an item from the collection page currently open.
    pub fn remove_from_current_collection(&mut self, item_id: String) {
        let Some(collection_id) = self
            .detail
            .as_ref()
            .filter(|detail| detail.item.kind == "BoxSet")
            .map(|detail| detail.item.id.clone())
        else {
            return;
        };
        if let Some(detail) = &mut self.detail {
            detail.collection.retain(|item| item.id != item_id);
        }
        if let Some(client) = self.client() {
            self.worker
                .remove_from_collection(client, collection_id, vec![item_id]);
        }
        self.show_toast("Removed from the collection.");
    }

    /// Keep live reminders across a restart. One small write per change, off the UI thread.
    pub(crate) fn persist_reminders(&mut self) {
        self.config.live_reminders = Some(self.live_reminders.clone());
        if self.persist {
            let config = self.config.clone();
            std::thread::spawn(move || {
                let _ = config.save();
            });
        }
    }

    pub fn toggle_notifications(&mut self) {
        self.notifications = !self.notifications;
        self.show_toast(if self.notifications {
            "Desktop notifications on."
        } else {
            "Desktop notifications off."
        });
    }

    /// Persist the current place for the next launch. The write is off-thread: navigation must
    /// never block on the config file.
    fn remember_place(&mut self) {
        let place = match &self.route {
            Route::Library { id } => crate::config::LastPlace {
                kind: "library".into(),
                id: Some(id.clone()),
                name: self.library.as_ref().map(|library| library.name.clone()),
                collection_type: self
                    .library
                    .as_ref()
                    .and_then(|library| library.collection_type.clone()),
            },
            Route::Detail { id } => crate::config::LastPlace {
                kind: "detail".into(),
                id: Some(id.clone()),
                name: None,
                collection_type: None,
            },
            Route::LiveTv => crate::config::LastPlace {
                kind: "livetv".into(),
                id: None,
                name: None,
                collection_type: None,
            },
            _ => crate::config::LastPlace {
                kind: "home".into(),
                id: None,
                name: None,
                collection_type: None,
            },
        };
        self.config.last_place = Some(place);
        self.config.live_favorites = Some(self.live.favorites.iter().cloned().collect());
        self.config.live_reminders = Some(self.live_reminders.clone());
        if self.persist {
            let config = self.config.clone();
            std::thread::spawn(move || {
                let _ = config.save();
            });
        }
    }

    pub fn toggle_high_contrast(&mut self) {
        self.high_contrast = !self.high_contrast;
        crate::theme::set_high_contrast(self.high_contrast);
    }

    pub fn toggle_light_theme(&mut self) {
        self.light_theme = !self.light_theme;
        crate::theme::set_light(self.light_theme);
    }

    /// The key to focus after a big-screen launch or a route change, once that screen has data.
    /// Returns `None` while a screen is still loading; the request stays armed.
    pub fn take_focus_key(&mut self) -> Option<String> {
        if !self.big_screen {
            return None;
        }
        if self.focus_route.as_ref() != Some(&self.route) {
            self.focus_route = Some(self.route.clone());
            self.focus_first = true;
        }
        let ready = matches!(self.step, Step::Ready { .. });
        if ready != self.focus_layers.ready {
            self.focus_layers.ready = ready;
            self.focus_first = true;
        }
        let quick = self.quick.is_some();
        if quick != self.focus_layers.quick {
            self.focus_layers.quick = quick;
            if quick {
                self.focus_first = false;
                return Some("quick-connect-cancel".into());
            }
            self.focus_first = true;
        }
        if let Some(key) = self.layer_focus_key() {
            return Some(key);
        }
        if !self.focus_first {
            return None;
        }
        if !ready {
            // The connect screen is a form, not a route: start on its first field, or on the
            // only control of the Quick Connect card.
            self.focus_first = false;
            return Some(if self.quick.is_some() {
                "quick-connect-cancel".into()
            } else {
                "server".into()
            });
        }
        let key = match &self.route {
            Route::Home => self
                .home_data
                .resume
                .iter()
                .find(|item| !self.is_home_hidden(&item.id))
                .map(|item| {
                    format!(
                        "rail-continue-watching-{}",
                        item.series_id.as_deref().unwrap_or(&item.id)
                    )
                })
                .or_else(|| {
                    self.home_data
                        .next_up
                        .iter()
                        .find(|item| !self.is_home_hidden(&item.id))
                        .map(|item| {
                            format!(
                                "rail-next-up-{}",
                                item.series_id.as_deref().unwrap_or(&item.id)
                            )
                        })
                }),
            Route::Library { .. } => self.library.as_ref().and_then(|library| {
                library.items.first().map(|item| {
                    // The list and the grid name the same item differently.
                    if library.view == ViewMode::Grid {
                        format!("library-{}", item.id)
                    } else {
                        format!("library-item-{}", item.id)
                    }
                })
            }),
            Route::Detail { .. } => self
                .detail
                .as_ref()
                .filter(|detail| !detail.loading && !detail.item.id.is_empty())
                .map(|_| "detail-play".into()),
            Route::Person { .. } | Route::SeerrPerson { .. } => {
                self.person.as_ref().map(|_| "person-back".into())
            }
            Route::Search { .. } => Some("search-input".into()),
            Route::Remote => Some("remote-refresh".into()),
            Route::Settings => Some("setting-ui-mode".into()),
            Route::Seerr => Some("seerr-tab-discover".into()),
            Route::SeerrDetail { .. } => self
                .seerr
                .detail
                .as_ref()
                .map(|_| "seerr-detail-back".into()),
            Route::LiveTv => self
                .live
                .channels
                .first()
                .map(|channel| format!("live-channel-{}", channel.id)),
        };
        if key.is_some() {
            self.focus_first = false;
        }
        key
    }

    /// The first control of a layer that just opened, so Start on a card lands inside the menu
    /// and A on a gear row lands inside the panel. Only edges return a key.
    fn layer_focus_key(&mut self) -> Option<String> {
        let card = self.card_menu.is_some();
        if card != self.focus_layers.card {
            self.focus_layers.card = card;
            if card {
                return Some("card-menu-open".into());
            }
            if let Some(key) = self.card_menu_focus.take() {
                return Some(key);
            }
            self.focus_first = true;
        }
        let gear = self.in_app_gear_open;
        if gear != self.focus_layers.gear {
            self.focus_layers.gear = gear;
            if gear {
                return Some("in-app-subtitles".into());
            }
            self.focus_first = true;
        }
        let subtitles = self.in_app_subtitles.open;
        if subtitles != self.focus_layers.subtitles {
            self.focus_layers.subtitles = subtitles;
            if subtitles {
                return Some("subtitle-track-off".into());
            }
            self.focus_first = true;
        }
        let audio = self.in_app_audio_open;
        if audio != self.focus_layers.audio {
            self.focus_layers.audio = audio;
            if audio {
                return self
                    .in_app_audio_tracks
                    .first()
                    .map(|track| format!("audio-track-{}", track.ordinal));
            }
            self.focus_first = true;
        }
        let chapters = self.in_app_chapters_open;
        if chapters != self.focus_layers.chapters {
            self.focus_layers.chapters = chapters;
            if chapters {
                return Some("in-app-chapter-0".into());
            }
            self.focus_first = true;
        }
        let shortcuts = self.shortcuts_open;
        if shortcuts != self.focus_layers.shortcuts {
            self.focus_layers.shortcuts = shortcuts;
            if shortcuts {
                return Some("shortcut-close".into());
            }
            return Some("nav-controls".into());
        }
        // Every overlay a controller can open takes focus once, so arrows do not start from a
        // node that the overlay covered.
        let shelf = self.shelf.is_some();
        if shelf != self.focus_layers.shelf {
            self.focus_layers.shelf = shelf;
            if shelf {
                return Some("shelf-back".into());
            }
            self.focus_first = true;
        }
        let filters = self
            .library
            .as_ref()
            .is_some_and(|library| library.filters_open);
        if filters != self.focus_layers.filters {
            self.focus_layers.filters = filters;
            if filters {
                return Some("filters-unwatched".into());
            }
            self.focus_first = true;
        }
        let genres = self
            .library
            .as_ref()
            .is_some_and(|library| library.genres_open);
        if genres != self.focus_layers.genres {
            self.focus_layers.genres = genres;
            if genres {
                return Some("genre-all".into());
            }
            self.focus_first = true;
        }
        let playlist_picker = self.playlist_picker_open;
        if playlist_picker != self.focus_layers.playlist_picker {
            self.focus_layers.playlist_picker = playlist_picker;
            if playlist_picker {
                return Some("playlist-picker-close".into());
            }
            self.focus_first = true;
        }
        let collection_picker = self.collection_picker_open;
        if collection_picker != self.focus_layers.collection_picker {
            self.focus_layers.collection_picker = collection_picker;
            if collection_picker {
                return Some("collection-picker-close".into());
            }
            self.focus_first = true;
        }
        let playlist_create = self.playlist_create_open;
        if playlist_create != self.focus_layers.playlist_create {
            self.focus_layers.playlist_create = playlist_create;
            if playlist_create {
                return Some("playlist-name-input".into());
            }
            self.focus_first = true;
        }
        let cast_picker = self.cast_picker_open;
        if cast_picker != self.focus_layers.cast_picker {
            self.focus_layers.cast_picker = cast_picker;
            if cast_picker {
                return Some("cast-picker-close".into());
            }
            self.focus_first = true;
        }
        let preplay = self.preplay_open;
        if preplay != self.focus_layers.preplay {
            self.focus_layers.preplay = preplay;
            if preplay {
                return Some("preplay-play".into());
            }
            self.focus_first = true;
        }
        let still_watching = self.still_watching_open;
        if still_watching != self.focus_layers.still_watching {
            self.focus_layers.still_watching = still_watching;
            if still_watching {
                return Some("still-watching-continue".into());
            }
            self.focus_first = true;
        }
        let quality = self.in_app_quality_open;
        if quality != self.focus_layers.quality {
            self.focus_layers.quality = quality;
            if quality {
                return Some("quality-auto".into());
            }
            self.focus_first = true;
        }
        let sleep = self.in_app_sleep_open;
        if sleep != self.focus_layers.sleep {
            self.focus_layers.sleep = sleep;
            if sleep {
                return Some("sleep-off".into());
            }
            self.focus_first = true;
        }
        let goto = self.in_app_goto_open;
        if goto != self.focus_layers.goto {
            self.focus_layers.goto = goto;
            if goto {
                return Some("in-app-goto-input".into());
            }
            self.focus_first = true;
        }
        let queue = self.in_app_queue_open;
        if queue != self.focus_layers.queue {
            self.focus_layers.queue = queue;
            if queue {
                return Some("in-app-queue-0".into());
            }
            self.focus_first = true;
        }
        None
    }

    /// Save the other interface mode. The running layout keeps its scale; the Settings card
    /// says a restart applies it.
    pub fn toggle_ui_mode(&mut self) {
        self.config.ui_mode = Some(if self.shown_big_screen() {
            "desktop".into()
        } else {
            "big-screen".into()
        });
        self.ui_mode_restart = true;
        if self.persist {
            let _ = self.config.save();
        }
    }

    /// The mode the Interface card shows: the pending saved choice after a change, else the mode
    /// this window launched in (a launch flag may override the file).
    pub fn shown_big_screen(&self) -> bool {
        if self.ui_mode_restart {
            self.config.big_screen()
        } else {
            self.big_screen
        }
    }

    /// 100% → 115% → 130% → 85% → 100%. Scaling every type token keeps cards readable
    /// without changing the poster geometry.
    pub fn cycle_text_scale(&mut self) {
        let next = match (self.text_scale * 100.0).round() as i32 {
            100 => 1.15,
            115 => 1.30,
            130 => 0.85,
            _ => 1.0,
        };
        self.text_scale = next;
        crate::theme::set_text_scale(next);
        self.show_toast(format!("Text size: {}%", (next * 100.0).round() as i32));
    }

    pub fn toggle_home_hero(&mut self) {
        self.home_hero = !self.home_hero;
    }

    pub fn export_history(&mut self) {
        if self.history.is_empty() {
            self.show_toast("There is no history to export.");
            return;
        }
        crate::history::export_csv(&self.history);
        self.show_toast("Exported letterboxd.csv to the cache.");
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
        self.history_ms = 0;
        crate::history::clear();
        self.show_toast("Local watch history cleared.");
    }

    pub fn save_library_view(&mut self) {
        let Some(library) = &self.library else {
            return;
        };
        let name = format!("View {}", self.saved_views.len() + 1);
        self.saved_views.push(crate::config::SavedView {
            name: name.clone(),
            sort: library.sort,
            ascending: library.ascending,
            unwatched: library.unwatched,
            favorites: library.favorites,
            genre: library.genre.clone(),
            year: library.year,
            min_rating: library.min_rating,
            audio_language: library.audio_language.clone(),
            subtitle_language: library.subtitle_language.clone(),
            has_subtitles: library.has_subtitles,
            only_4k: library.only_4k,
            missing_only: library.missing_only,
            unaired_only: library.unaired_only,
            letter: library.letter.clone(),
            search: library.search.clone(),
        });
        self.show_toast(format!("Saved {name}."));
    }

    pub fn apply_library_view(&mut self, index: usize) {
        let Some(view) = self.saved_views.get(index).cloned() else {
            return;
        };
        if let Some(library) = &mut self.library {
            library.sort = view.sort;
            library.ascending = view.ascending;
            library.unwatched = view.unwatched;
            library.favorites = view.favorites;
            library.genre = view.genre;
            library.year = view.year;
            library.min_rating = view.min_rating;
            library.audio_language = view.audio_language;
            library.subtitle_language = view.subtitle_language;
            library.has_subtitles = view.has_subtitles;
            library.only_4k = view.only_4k;
            library.missing_only = view.missing_only;
            library.unaired_only = view.unaired_only;
            library.letter = view.letter;
            library.search = view.search;
            library.set_scroll_y(0.0);
        }
        self.library_search = rune::TextEdit::new(
            self.library
                .as_ref()
                .and_then(|library| library.search.clone())
                .unwrap_or_default(),
        );
        self.load_library_page(true);
        self.pending_scroll = Some(0.0);
    }

    pub fn delete_library_view(&mut self, index: usize) {
        if index < self.saved_views.len() {
            self.saved_views.remove(index);
        }
    }

    pub fn library_search_changed(&mut self) {
        let term = self.library_search.text.trim().to_string();
        if let Some(library) = &mut self.library {
            library.search = (!term.is_empty()).then_some(term);
            library.set_scroll_y(0.0);
        }
        self.library_search_due = Some(Instant::now() + Duration::from_millis(200));
    }

    pub fn clear_library_search(&mut self) {
        self.library_search = rune::TextEdit::new("");
        self.library_search_due = None;
        if let Some(library) = &mut self.library {
            library.search = None;
        }
        self.load_library_page(true);
        self.pending_scroll = Some(0.0);
    }

    pub fn toggle_library_missing(&mut self) {
        let changed = match &mut self.library {
            Some(library) => {
                library.missing_only = !library.missing_only;
                library.set_scroll_y(0.0);
                true
            }
            None => false,
        };
        if changed {
            self.load_library_page(true);
            self.pending_scroll = Some(0.0);
        }
    }

    pub fn toggle_library_unaired(&mut self) {
        let changed = match &mut self.library {
            Some(library) => {
                library.unaired_only = !library.unaired_only;
                library.set_scroll_y(0.0);
                true
            }
            None => false,
        };
        if changed {
            self.load_library_page(true);
            self.pending_scroll = Some(0.0);
        }
    }

    pub fn toggle_library_4k(&mut self) {
        let changed = match &mut self.library {
            Some(library) => {
                library.only_4k = !library.only_4k;
                library.set_scroll_y(0.0);
                true
            }
            None => false,
        };
        if changed {
            self.load_library_page(true);
            self.pending_scroll = Some(0.0);
        }
    }

    pub fn toggle_library_subtitles(&mut self) {
        let changed = match &mut self.library {
            Some(library) => {
                library.has_subtitles = !library.has_subtitles;
                library.set_scroll_y(0.0);
                true
            }
            None => false,
        };
        if changed {
            self.load_library_page(true);
            self.pending_scroll = Some(0.0);
        }
    }

    pub fn toggle_library_filters(&mut self) {
        let mut fetch = None;
        if let Some(library) = &mut self.library {
            library.filters_open = !library.filters_open;
            if library.filters_open && library.genres.is_empty() && !library.genres_loading {
                library.genres_loading = true;
                fetch = Some(library.id.clone());
            }
        }
        if let Some(parent) = fetch
            && parent != "__watchlist__"
            && let Some(client) = self.client()
        {
            self.library_genres.request(parent.clone(), move |parent| {
                client
                    .genres(Some(&parent))
                    .map_err(|error| error.user_message())
            });
        }
    }

    /// Show or hide the genre chips, fetching the library's genre list the first time.
    pub fn toggle_library_genres(&mut self) {
        let mut fetch = None;
        if let Some(library) = &mut self.library {
            library.genres_open = !library.genres_open;
            if library.genres_open && library.genres.is_empty() && !library.genres_loading {
                library.genres_loading = true;
                fetch = Some(library.id.clone());
            }
        }
        if let Some(parent) = fetch
            && let Some(client) = self.client()
        {
            self.library_genres.request(parent.clone(), move |parent| {
                client
                    .genres(Some(&parent))
                    .map_err(|error| error.user_message())
            });
        }
    }

    pub(super) fn apply_library_genres(&mut self, load: Result<Vec<Item>, String>) {
        if let Some(library) = &mut self.library {
            library.genres_loading = false;
            match load {
                Ok(genres) => {
                    library.genres = genres;
                    library.error = None;
                }
                Err(message) => library.error = Some(message),
            }
        }
    }

    pub fn select_library_genre(&mut self, genre: Option<(String, String)>) {
        let changed = match &mut self.library {
            Some(library) => {
                library.genre = genre;
                library.genres_open = false;
                library.set_scroll_y(0.0);
                true
            }
            None => false,
        };
        if changed {
            self.load_library_page(true);
            self.pending_scroll = Some(0.0);
        }
    }

    pub fn toggle_favorites(&mut self) {
        let changed = match &mut self.library {
            Some(library) => {
                library.favorites = !library.favorites;
                library.set_scroll_y(0.0);
                true
            }
            None => false,
        };
        if changed {
            self.load_library_page(true);
            self.pending_scroll = Some(0.0);
        }
    }

    /// Clear every filter the library screen exposes, and reload from the top.
    pub fn clear_library_filters(&mut self) {
        if let Some(library) = &mut self.library {
            library.unwatched = false;
            library.favorites = false;
            library.genre = None;
            library.letter = None;
            library.year = None;
            library.min_rating = None;
            library.audio_language = None;
            library.subtitle_language = None;
            library.has_subtitles = false;
            library.only_4k = false;
            library.missing_only = false;
            library.unaired_only = false;
            library.search = None;
            library.set_scroll_y(0.0);
        }
        self.library_search = rune::TextEdit::new("");
        self.load_library_page(true);
        self.pending_scroll = Some(0.0);
    }

    /// The letter rail: pressing the selected letter clears it.
    pub fn toggle_letter(&mut self, letter: char) {
        let value = letter.to_string();
        let changed = match &mut self.library {
            Some(library) => {
                library.letter = if library.letter.as_deref() == Some(value.as_str()) {
                    None
                } else {
                    Some(value)
                };
                library.set_scroll_y(0.0);
                true
            }
            None => false,
        };
        if changed {
            self.load_library_page(true);
            self.pending_scroll = Some(0.0);
        }
    }

    /// Fetch the first page (`reset`) or the next one. A page in flight stops another request;
    /// the first page immediately queues one page ahead.
    /// Open a playlist in the library screen. A playlist is an ordered list with no sort, no
    /// filters and no paging: the server returns it whole on one endpoint.
    /// The watchlist is the server's favourite flag shown as one cross-library page. It is not
    /// a second store, so adding from a card and opening here always agree.
    pub fn open_watchlist(&mut self) {
        self.capture_library_scroll();
        self.stop_theme_media();
        self.detail_stack.clear();
        self.library_genres.cancel();
        self.library = Some(LibraryState::new(
            "__watchlist__".into(),
            "Watchlist".into(),
            Some("watchlist".into()),
        ));
        self.route = Route::Library {
            id: "__watchlist__".into(),
        };
        self.pending_scroll = Some(0.0);
        self.remember_place();
        self.load_library_page(true);
    }

    pub fn open_playlist(&mut self, id: String, name: String) {
        self.route = Route::Library { id: id.clone() };
        self.playlist_editing = false;
        self.library = Some(LibraryState::new(id, name, Some("playlists".into())));
        self.pending_scroll = Some(0.0);
        self.load_library_page(true);
        self.request_images();
    }

    /// Show or hide the remove controls on a playlist page. They never appear on a library.
    pub fn toggle_playlist_editing(&mut self) {
        let is_playlist = self
            .library
            .as_ref()
            .is_some_and(|library| library.collection_type.as_deref() == Some("playlists"));
        if is_playlist {
            self.playlist_editing = !self.playlist_editing;
        }
    }

    /// Move one entry by one place. The list reorders immediately and the server is asked in
    /// the background; a refusal reloads the page so the visible order is never a lie for long.
    pub fn move_playlist_entry(&mut self, entry_id: String, delta: isize) {
        if !self.playlist_editing {
            return;
        }
        let (playlist_id, from, to) = {
            let Some(library) = &mut self.library else {
                return;
            };
            if library.collection_type.as_deref() != Some("playlists") {
                return;
            }
            let Some(at) = library
                .items
                .iter()
                .position(|item| item.playlist_item_id.as_deref() == Some(entry_id.as_str()))
            else {
                return;
            };
            let last = library.items.len().saturating_sub(1) as isize;
            let target = (at as isize + delta).clamp(0, last) as usize;
            if target == at {
                return;
            }
            // Rotate rather than swap: a drag can move several places, and the entries between
            // the old and new position should shift by one, not jump into each other's slots.
            let moved = library.items.remove(at);
            library.items.insert(target, moved);
            library.notice = None;
            (library.id.clone(), at, target)
        };
        if let Some(client) = self.client() {
            self.worker
                .move_playlist_entry(client, playlist_id.clone(), entry_id.clone(), to);
        }
        self.show_undo_toast(
            "Moved the entry.",
            crate::state::UndoAction::PlaylistMove {
                playlist_id,
                entry_id,
                from,
                to,
            },
        );
    }

    pub fn remove_playlist_entry(&mut self, entry_id: String) {
        let Some(library) = &self.library else {
            return;
        };
        let client = self.client();
        if library.collection_type.as_deref() != Some("playlists") {
            return;
        }
        let playlist_id = library.id.clone();
        let Some((index, item)) = library
            .items
            .iter()
            .position(|item| item.playlist_item_id.as_deref() == Some(entry_id.as_str()))
            .and_then(|at| library.items.get(at).cloned().map(|item| (at, item)))
        else {
            return;
        };
        if let Some(library) = &mut self.library {
            library.items.remove(index);
            library.total = (library.total - 1).max(0);
        }
        let remove_playlist = playlist_id.clone();
        if let Some(client) = client {
            self.playlist_remove_load
                .request(entry_id.clone(), move |entry_id| {
                    client
                        .remove_from_playlist(&remove_playlist, &[entry_id])
                        .map_err(|error| error.user_message())
                });
        }
        self.show_undo_toast(
            "Removed from the playlist.",
            crate::state::UndoAction::PlaylistRemove {
                playlist_id,
                item,
                index,
            },
        );
    }

    pub(super) fn apply_playlist_remove(&mut self, load: Result<(), String>) {
        match load {
            // The page already removed the row; a reload here would race the Undo toast.
            Ok(()) => {}
            Err(message) => {
                if let Some(library) = &mut self.library {
                    library.error = Some(format!("Could not remove the entry: {message}"));
                }
            }
        }
    }

    pub fn load_library_page(&mut self, reset: bool) {
        if !self.fetch_home {
            return;
        }
        let Some(client) = self.client() else {
            return;
        };
        let Some(library) = &mut self.library else {
            return;
        };
        if library.loading && !reset {
            return;
        }
        if !reset && !library.has_more() {
            return;
        }
        let start = if reset {
            library.items.clear();
            library.total = 0;
            library.prefetched = false;
            library.error = None;
            0
        } else {
            library.items.len() as u32
        };
        library.loading = true;
        let query = library.query();
        eprintln!(
            "LIVE library load id={} type={:?} start={start} total_before={} fetch_home={}",
            library.id, library.collection_type, library.total, self.fetch_home
        );
        if library.collection_type.as_deref() == Some("playlists") {
            let playlist_id = library.id.clone();
            self.library_page
                .request(
                    LibraryRequest { query, start: 0 },
                    move |_request| match client.playlist_items(&playlist_id) {
                        Ok(items) => LibraryPage {
                            start_index: 0,
                            total: items.len() as i32,
                            items,
                            error: None,
                        },
                        Err(error) => LibraryPage {
                            start_index: 0,
                            items: Vec::new(),
                            total: 0,
                            error: Some(error.user_message()),
                        },
                    },
                );
            return;
        }
        self.library_page
            .request(LibraryRequest { query, start }, move |request| match client
                .items(&request.query)
            {
                Ok(page) => LibraryPage {
                    start_index: request.start,
                    items: page.items,
                    total: page.total_record_count,
                    error: None,
                },
                Err(error) => LibraryPage {
                    start_index: request.start,
                    items: Vec::new(),
                    total: 0,
                    error: Some(error.user_message()),
                },
            });
    }

    fn apply_library_page(&mut self, page: LibraryPage) {
        eprintln!(
            "LIVE library page start={} total={} items={} error={:?}",
            page.start_index,
            page.total,
            page.items.len(),
            page.error
        );
        let prefetch = match &mut self.library {
            Some(library) => {
                library.loading = false;
                if let Some(error) = page.error {
                    library.error = Some(error);
                    false
                } else {
                    if page.start_index == 0 {
                        library.items = page.items;
                    } else if page.start_index as usize == library.items.len() {
                        library.items.extend(page.items);
                    }
                    library.total = page.total;
                    library.error = None;
                    let prefetch =
                        page.start_index == 0 && !library.prefetched && library.has_more();
                    if prefetch {
                        library.prefetched = true;
                    }
                    prefetch
                }
            }
            None => false,
        };
        if prefetch {
            self.load_library_page(false);
        }
    }

    /// Zero while the placeholder beat holds, then one; the full image opacity springs between.
    pub fn image_alpha(&self, key: &str) -> f32 {
        if self.image_fades.contains_key(key) {
            0.0
        } else {
            1.0
        }
    }

    pub fn toggle_sidebar(&mut self) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
    }

    pub fn toggle_shortcuts(&mut self) {
        self.shortcuts_open = !self.shortcuts_open;
    }

    /// Wait for the next key and bind it to `action`. The sheet stays open behind the settings
    /// page so the reader can see the row change.
    pub fn begin_rebind(&mut self, action: ShortcutAction) {
        self.rebinding = Some(action);
    }

    pub fn cancel_rebind(&mut self) {
        self.rebinding = None;
    }

    /// Bind the captured key unless another action already owns it. Returns false on a clash,
    /// which leaves the old binding in place.
    pub fn capture_shortcut(&mut self, key: &str) -> bool {
        let Some(action) = self.rebinding.take() else {
            return false;
        };
        if self.key_bindings.used_by_other(action, key) {
            self.show_toast(format!(
                "{} is already used by another shortcut.",
                crate::shortcuts::display(key)
            ));
            return false;
        }
        self.key_bindings.set(action, key);
        self.show_toast(format!(
            "{} is now {}.",
            action.label(),
            crate::shortcuts::display(key)
        ));
        true
    }

    pub fn reset_shortcuts(&mut self) {
        self.key_bindings = KeyBindings::default();
        self.show_toast("Shortcuts restored to their defaults.");
    }

    pub fn shortcut_matches(&self, action: ShortcutAction, key: &rune::KeyEvent) -> bool {
        crate::shortcuts::matches(self.key_bindings.get(action), key)
    }

    pub fn close_shortcuts(&mut self) {
        self.shortcuts_open = false;
    }

    /// The backdrop behind Home: the newest item that actually has one. It is deliberately one
    /// image, not a rotating hero, so the page stays quiet and cheap.
    pub fn home_backdrop_key(&self) -> Option<String> {
        let item = self
            .home_data
            .resume
            .iter()
            .chain(
                self.home_data
                    .latest
                    .iter()
                    .flat_map(|rail| rail.items.iter()),
            )
            .find(|item| item.image_tag(ImageKind::Backdrop).is_some())?;
        let tag = item.image_tag(ImageKind::Backdrop)?;
        Some(crate::image::key(
            &self.config.server_url,
            &item.id,
            "Backdrop",
            Some(tag),
            HOME_BACKDROP_WIDTH,
        ))
    }

    /// The small portrait used by the cast rail.
    pub fn person_image_key(&self, person: &jellyfin_api::Person) -> Option<String> {
        let tag = person.primary_image_tag.as_deref()?;
        Some(crate::image::key(
            &self.config.server_url,
            &person.id,
            "Primary",
            Some(tag),
            160,
        ))
    }

    /// The 16:9 still used by an episode row.
    pub fn episode_image_key(&self, item: &Item) -> Option<String> {
        let tag = item.image_tag(ImageKind::Primary)?;
        Some(crate::image::key(
            &self.config.server_url,
            &item.id,
            "Primary",
            Some(tag),
            320,
        ))
    }

    /// The still for one chapter of the playing item, at the size the chapter list draws.
    pub fn chapter_image_key(&self, item: &Item, at: usize) -> Option<String> {
        let tag = item.chapters.get(at)?.image_tag.as_deref()?;
        Some(crate::image::key(
            &self.config.server_url,
            &item.id,
            &format!("Chapter{at}"),
            Some(tag),
            160,
        ))
    }

    /// The landscape image for Continue Watching: the item's backdrop, or its poster when the
    /// server has no backdrop. The card crops it to 16:9.
    pub fn resume_image_key(&self, item: &Item) -> Option<String> {
        let backdrop = item.image_tag(ImageKind::Backdrop).map(|tag| {
            crate::image::key(
                &self.config.server_url,
                &item.id,
                "Backdrop",
                Some(tag),
                640,
            )
        });
        backdrop.or_else(|| self.image_key(item))
    }

    /// The cache key for an item's poster, as the view looks it up.
    pub fn image_key(&self, item: &Item) -> Option<String> {
        let (id, tag) = crate::home::poster(item)?;
        Some(crate::image::key(
            &self.config.server_url,
            id,
            "Primary",
            Some(tag),
            POSTER_WIDTH,
        ))
    }

    fn client(&self) -> Option<Client> {
        if !self.config.has_session() {
            return None;
        }
        Some(
            Client::new(&self.config.server_url, self.config.device_id.clone())
                .with_session(self.config.token.clone(), self.config.user_id.clone()),
        )
    }

    /// Complete any sign-in path: password or Quick Connect. Stores the session, persists it if
    /// this is a real app (not a fixture), and moves to the home screen.
    pub(super) fn apply_session(&mut self, session: Session) {
        // Wipe the previous session (and its token buffer) before storing the new one.
        self.config.sign_out();
        self.config.server_url = session.server_url;
        self.config.user_id = session.user_id;
        self.config.user_name = session.user_name.clone();
        self.config.server_name = session.server_name.clone();
        self.config.token = session.token;
        // A failed write is not worth blocking the UI for; the session still works for
        // this run, and the next launch simply asks to sign in again.
        if self.persist {
            let _ = self.config.save();
        }
        self.server = TextEdit::new(self.config.server_url.clone());
        self.user = TextEdit::new(self.config.user_name.clone());
        self.password = TextEdit::new("").secure();
        self.step = Step::Ready {
            server_name: session.server_name,
        };
        self.quick = None;
        self.route = Route::Home;
        self.remember_place();
        self.shelf = None;
        self.library = None;
        self.library_selecting = false;
        self.library_selected.clear();
        self.detail = None;
        self.detail_return = Route::Home;
        self.search.query = TextEdit::new("");
        self.search_latest.cancel();
        self.library_scrolls.clear();
        self.home_started = true;
        self.load_home();
        if self.seerr_configured() {
            self.refresh_seerr_badge();
            self.test_seerr();
        }
    }

    /// The connect button. Returns false when the form cannot be submitted yet (and sets the
    /// failure the screen shows), so the view stays a pure read.
    pub fn connect(&mut self) -> bool {
        let url = self.server.text.trim().to_string();
        if url.is_empty() {
            self.step = Step::Failed(Failure {
                stage: Stage::Server,
                message: "Enter your server's address.".into(),
            });
            return false;
        }
        let user = self.user.text.trim().to_string();
        if user.is_empty() {
            self.step = Step::Failed(Failure {
                stage: Stage::Credentials,
                message: "Enter your username.".into(),
            });
            return false;
        }
        if self.config.device_id.is_empty() {
            self.config.device_id = Config::fresh().device_id;
        }
        self.config.server_url = jellyfin_api::normalise_base_url(&url);
        self.step = Step::SigningIn;
        self.worker.connect(
            self.config.server_url.clone(),
            self.config.device_id.clone(),
            user,
            self.password.text.clone(),
        );
        true
    }

    pub fn clear_image_cache(&mut self) {
        self.image_notice = Some("Clearing poster cache…".into());
        self.worker.clear_image_cache();
    }

    pub fn open_settings(&mut self) {
        self.stop_theme_media();
        self.route = Route::Settings;
        self.search.query.focused = false;
    }

    pub fn sign_out(&mut self) {
        self.stop_in_app();
        self.config.sign_out();
        if self.persist {
            let _ = self.config.save();
        }
        self.home.cancel();
        self.home_next_up.cancel();
        self.home_collections.cancel();
        self.playlists_load.cancel();
        self.library_page.cancel();
        self.library_genres.cancel();
        self.detail_page.cancel();
        self.detail_episodes.cancel();
        self.home_data = HomeData::default();
        self.home_loaded = false;
        self.home_error = None;
        self.home_started = false;
        self.route = Route::Home;
        self.remember_place();
        self.shelf = None;
        self.library = None;
        self.library_selecting = false;
        self.library_selected.clear();
        self.detail = None;
        self.person = None;
        self.person_detail = None;
        self.person_latest.cancel();
        self.detail_return = Route::Home;
        self.detail_stack.clear();
        self.detail_notice = None;
        self.detail_overview_open = false;
        self.search.query = TextEdit::new("");
        self.search.query.focused = false;
        self.search_latest.cancel();
        self.remote = RemoteState::default();
        self.seerr.testing = false;
        self.seerr.probe = None;
        self.seerr.error = None;
        self.seerr_probe.cancel();
        self.seerr_requests.cancel();
        self.seerr_counts.cancel();
        self.seerr_summaries.cancel();
        self.seerr_discover.cancel();
        self.seerr_search.cancel();
        self.seerr_detail_load.cancel();
        self.seerr_series_load.cancel();
        self.seerr_library_load.cancel();
        self.seerr_genres.cancel();
        self.seerr_browse.cancel();
        self.seerr_search_global.cancel();
        self.seerr.series = None;
        self.seerr.series_error = None;
        self.seerr_series_for = None;
        self.seerr.requests.clear();
        self.seerr.counts = None;
        self.seerr.summaries.clear();
        self.seerr.discover = SeerrDiscover::default();
        self.seerr.search = SearchResults::default();
        self.seerr.detail = None;
        self.seerr.in_flight = false;
        self.seerr.loading = false;
        self.seerr.summaries_loading = false;
        self.seerr.discover_loading = false;
        self.seerr.searching = false;
        self.seerr.last_loaded = None;
        self.remote_latest.cancel();
        self.library_scrolls.clear();
        self.pending_scroll = None;
        self.images.clear();
        self.requested_images.clear();
        self.image_order.clear();
        self.image_notice = None;
        self.image_dirty.store(false, Ordering::Relaxed);
        self.step = Step::Idle;
        self.password = TextEdit::new("").secure();
        self.server.focused = true;
    }

    pub fn is_signed_in(&self) -> bool {
        matches!(self.step, Step::Ready { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handoff::PlaybackEvent;
    use std::time::Instant;

    fn state() -> State {
        State {
            server: TextEdit::new(""),
            user: TextEdit::new(""),
            password: TextEdit::new("").secure(),
            step: Step::Idle,
            worker: Worker::new(),
            home: Latest::default(),
            home_data: HomeData::default(),
            home_next_up: Latest::default(),
            home_on_now: Latest::default(),
            home_recently_played: Latest::default(),
            home_collections: Latest::default(),
            home_loaded: false,
            home_error: None,
            home_started: false,
            image_cache: None,
            images: HashMap::new(),
            placeholders: HashMap::new(),
            requested_images: HashSet::new(),
            current_images: HashSet::new(),
            last_route: Route::Home,
            image_notice: None,
            image_order: VecDeque::new(),
            image_fades: HashMap::new(),
            route: Route::Home,
            shelf: None,
            library: None,
            library_selecting: false,
            library_selected: std::collections::HashSet::new(),
            playlist_editing: false,
            playlist_remove_load: Latest::default(),
            library_page: Latest::default(),
            library_genres: Latest::default(),
            detail: None,
            person: None,
            person_return: Route::Home,
            person_detail: None,
            person_latest: Latest::default(),
            seerr_person_load: Latest::default(),
            detail_page: Latest::default(),
            detail_similar_load: Latest::default(),
            detail_similar_for: None,
            detail_episodes: Latest::default(),
            detail_return: Route::Home,
            detail_stack: Vec::new(),
            detail_notice: None,
            detail_imdb: None,
            detail_imdb_for: None,
            detail_extras: Vec::new(),
            detail_collections: Vec::new(),
            detail_collections_for: None,
            detail_collections_load: Latest::default(),
            detail_theme_songs: Vec::new(),
            detail_theme_videos: Vec::new(),
            detail_theme_load: Latest::default(),
            detail_theme_for: None,
            theme_audio: None,
            theme_audio_title: None,
            theme_audio_load: Latest::default(),
            theme_audio_starting: false,
            theme_video: None,
            theme_video_on: false,
            theme_video_load: Latest::default(),
            theme_video_starting: false,
            resume_conflict: None,
            detail_extras_for: None,
            detail_extras_load: Latest::default(),
            detail_imdb_load: Latest::default(),
            imdb_ratings: None,
            imdb_ratings_loading: false,
            imdb_ratings_error: None,
            imdb_ratings_load: Latest::default(),
            omdb_key: TextEdit::new("").secure(),
            seerr: SeerrState::new(&Config::default()),
            seerr_probe: Latest::default(),
            seerr_requests: Latest::default(),
            seerr_counts: Latest::default(),
            seerr_summaries: Latest::default(),
            seerr_discover: Latest::default(),
            seerr_search: Latest::default(),
            seerr_detail_load: Latest::default(),
            seerr_detail_for: None,
            seerr_series_load: Latest::default(),
            seerr_library_load: Latest::default(),
            seerr_genres: Latest::default(),
            seerr_browse: Latest::default(),
            seerr_search_global: Latest::default(),
            seerr_browse_for: None,
            seerr_series_for: None,
            card_menu: None,
            discovered: Vec::new(),
            discovering: false,
            discovery_load: Latest::default(),
            idle: idle::IdleGuard::default(),
            playlist_picker_open: false,
            playlist_picker_item: None,
            playlist_picker_ids: None,
            playlist_add_load: Latest::default(),
            playlist_add_pending: None,
            collection_picker_open: false,
            collection_picker_item: None,
            collection_picker_ids: None,
            collection_choices: Vec::new(),
            collection_choices_loading: false,
            collection_choices_load: Latest::default(),
            collection_add_load: Latest::default(),
            collection_add_pending: None,
            playlist_create_open: false,
            playlist_new_name: TextEdit::new(""),
            cast_picker_open: false,
            cast_item: None,
            cast_sessions: Vec::new(),
            cast_loading: false,
            cast_sessions_load: Latest::default(),
            cast_send_load: Latest::default(),
            transfer_after_cast: false,
            toast: None,
            toast_undo: None,
            toast_until: None,
            playlist_create_load: Latest::default(),
            detail_overview_open: false,
            detail_tech_open: false,
            playback_start: Latest::default(),
            playback: None,
            launching: false,
            playback_error: None,
            pending_play: None,
            next_playback: 0,
            search: SearchState::new(),
            search_latest: Latest::default(),
            search_hints: Latest::default(),
            search_return: Route::Home,
            remote: RemoteState::default(),
            remote_latest: Latest::default(),
            live: LiveState::default(),
            notifications: false,
            key_bindings: KeyBindings::default(),
            rebinding: None,
            live_reminders: Vec::new(),
            reminders_fired: HashSet::new(),
            last_reminder_check: Instant::now(),
            quick: None,
            quick_start: Latest::default(),
            quick_poll: Latest::default(),
            quick_auth: Latest::default(),
            live_latest: Latest::default(),
            live_open: Latest::default(),
            live_close: Latest::default(),
            pending_zap: None,
            pending_live: None,
            in_app: None,
            in_app_fill: false,
            in_app_speed: 1.0,
            in_app_volume: 1.0,
            queue: Vec::new(),
            queue_at: 0,
            in_app_queue_open: false,
            autoplay_next: true,
            subtitle_mode: SubtitleMode::Forced,
            skip_mode: crate::config::SkipMode::Ask,
            still_watching: true,
            still_watching_episodes: 3,
            still_watching_minutes: 90,
            resume_rewind_seconds: 10,
            downmix_stereo: false,
            audio_delay_ms: 0,
            audio_mode: crate::config::AudioMode::Off,
            reduce_motion: false,
            settings_query: rune::TextEdit::new(""),
            library_search: rune::TextEdit::new(""),
            library_search_due: None,
            in_app_run_episodes: 0,
            in_app_last_input: Instant::now(),
            in_app_play_since: Instant::now(),
            in_app_query_at: Instant::now(),
            last_in_app_error: None,
            still_watching_open: false,
            still_watching_next: None,
            still_watching_since: None,
            still_watching_countdown: 60,
            sleep_fade_percent: 100,
            sleep_deadline: None,
            sleep_end_of_episode: false,
            in_app_sleep_open: false,
            in_app_quality_open: false,
            in_app_goto_open: false,
            in_app_gear_open: false,
            in_app_gear_returns: false,
            in_app_goto: rune::TextEdit::new(""),
            loop_a: None,
            loop_b: None,
            checkpoints: std::collections::HashMap::new(),
            checkpoint_at: Instant::now(),
            subtitle_background: SubtitleBackground::Dim,
            subtitle_font: SubtitleFont::Sans,
            subtitle_color: SubtitleColor::White,
            subtitle_outline: SubtitleOutline::Off,
            subtitle_position: SubtitlePosition::Bottom,
            subtitle_style: SubtitleStyle::Normal,
            subtitle_opacity: 1.0,
            subtitle_background_opacity: 1.0,
            subtitle_language: String::new(),
            audio_language: String::new(),
            seek_step_ms: 10_000,
            window_size: (0.0, 0.0),
            fullscreen: false,
            pending_fullscreen: None,
            mpris: None,
            window_position: (0.0, 0.0),
            queue_load: Latest::default(),
            playlists: Vec::new(),
            playlists_load: Latest::default(),
            in_app_minimized: false,
            sidebar_collapsed: false,
            shortcuts_open: false,
            osd_visible: true,
            osd_hide_at: None,
            in_app_start: Latest::default(),
            trailer_loading: false,
            trailer_load: Latest::default(),
            in_app_segments: Vec::new(),
            in_app_segments_load: Latest::default(),
            in_app_subtitles: Subtitles::default(),
            in_app_subtitle_search: SubtitleSearch::default(),
            in_app_subtitle_search_load: Latest::default(),
            in_app_subtitle_download_load: Latest::default(),
            pending_subtitle_before: None,
            in_app_subtitle_load: Latest::default(),
            in_app_image_load: Latest::default(),
            in_app_streams_load: Latest::default(),
            in_app_audio_tracks: Vec::new(),
            in_app_audio_open: false,
            in_app_audio_chosen: false,
            in_app_video: None,
            in_app_container: None,
            in_app_stats: false,
            in_app_prev: None,
            in_app_neighbours_load: Latest::default(),
            in_app_chapters_open: false,
            in_app_next: None,
            in_app_next_dismissed: false,

            in_app_hover: None,
            in_app_seek_width: 0.0,
            in_app_hover_armed: false,
            in_app_trickplay_key: None,
            in_app_launching: false,
            pending_in_app: None,
            waker: None,
            library_scrolls: HashMap::new(),
            pending_scroll: None,
            in_app_source: None,
            in_app_source_for: None,
            version_choices: std::collections::HashMap::new(),
            playback_quality: crate::config::PlaybackQuality::Auto,
            quality_choices: std::collections::HashMap::new(),
            hidden_home: std::collections::HashSet::new(),
            spoiler_guard: true,
            saved_views: Vec::new(),
            home_hero: false,
            light_theme: false,
            big_screen: false,
            ui_mode_restart: false,
            focus_first: false,
            focus_route: None,
            focus_layers: FocusLayers::default(),
            card_menu_focus: None,
            high_contrast: false,
            text_scale: 1.0,
            mpris_shuffle: false,
            tray: None,
            tray_events: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::VecDeque::new(),
            )),
            tray_tooltip: String::new(),
            history: Vec::new(),
            pending_place: None,
            history_ms: 0,
            preplay_open: false,
            image_dirty: Arc::new(AtomicBool::new(false)),
            fetch_home: false,
            persist: false,
            config: Config::fresh(),
        }
    }

    fn item(id: &str) -> Item {
        Item {
            id: id.into(),
            name: id.into(),
            ..Item::default()
        }
    }

    #[test]
    fn reopening_a_library_restores_the_scroll_it_was_left_at() {
        let mut s = state();
        s.step = Step::Ready {
            server_name: "example".into(),
        };
        s.open_library("lib".into(), "Movies".into(), Some("movies".into()));
        if let Some(library) = &s.library {
            library.set_scroll_y(420.0);
        }
        s.go_home();
        assert_eq!(s.library_scrolls.get("lib"), Some(&420.0));
        s.open_library("lib".into(), "Movies".into(), Some("movies".into()));
        if let Some(library) = &mut s.library {
            library.items.push(item("x"));
        }
        assert_eq!(s.take_pending_scroll(), Some(420.0));
    }

    #[test]
    fn a_launch_override_shows_the_running_mode_until_settings_changes_it() {
        let mut s = state();
        s.config.ui_mode = None;
        s.big_screen = true; // `--big-screen` with a desktop file
        assert!(s.shown_big_screen(), "the card shows the mode in use");
        assert!(!s.ui_mode_restart, "a launch override is already applied");

        s.toggle_ui_mode();
        assert_eq!(s.config.ui_mode.as_deref(), Some("desktop"));
        assert!(!s.shown_big_screen(), "the pending choice shows at once");
        assert!(s.ui_mode_restart, "and asks for a restart");

        s.toggle_ui_mode();
        assert_eq!(s.config.ui_mode.as_deref(), Some("big-screen"));
        assert!(s.shown_big_screen());
        assert!(s.ui_mode_restart);
    }

    #[test]
    fn the_first_library_page_queues_one_page_ahead_and_later_pages_append() {
        let mut s = state();
        s.step = Step::Ready {
            server_name: "example".into(),
        };
        s.library = Some(LibraryState::new(
            "lib".into(),
            "Movies".into(),
            Some("movies".into()),
        ));
        s.apply_library_page(LibraryPage {
            start_index: 0,
            items: vec![item("a"), item("b")],
            total: 4,
            error: None,
        });
        let library = s.library.as_ref().expect("the library is open");
        assert_eq!(library.items.len(), 2);
        assert!(library.prefetched);
        s.apply_library_page(LibraryPage {
            start_index: 2,
            items: vec![item("c"), item("d")],
            total: 4,
            error: None,
        });
        let library = s.library.as_ref().expect("the library is open");
        assert_eq!(library.items.len(), 4);
        assert!(!library.has_more());
    }

    #[test]
    fn a_playback_event_from_an_old_session_is_ignored() {
        let mut s = state();
        s.playback = Some(PlaybackSession {
            item: item("episode"),
            item_id: "episode".into(),
            session_id: "session".into(),
            position_ms: 0,
            duration_ms: Some(60_000),
            paused: false,
            generation: 7,
            last_report: Instant::now(),
            handle: None,
        });
        s.on_playback_event(7, PlaybackEvent::Position(500));
        assert_eq!(
            s.playback.as_ref().map(|session| session.position_ms),
            Some(500)
        );
        s.on_playback_event(8, PlaybackEvent::Position(900));
        assert_eq!(
            s.playback.as_ref().map(|session| session.position_ms),
            Some(500)
        );
        s.stop_playback();
        assert!(s.playback.is_none());
    }

    #[test]
    fn decoded_images_are_bounded_and_failed_fetches_can_retry() {
        let mut s = state();
        s.image_cache = Some(ImageCache::new(Arc::new(|| {})));
        for index in 0..(IMAGE_CACHE_LIMIT + 10) {
            s.apply(Msg::Image {
                key: format!("image-{index}"),
                bytes: Ok(vec![0]),
                placeholder: None,
            });
        }
        assert!(s.images.len() <= IMAGE_CACHE_LIMIT);
        s.requested_images.insert("retry-me".into());
        s.apply(Msg::Image {
            key: "retry-me".into(),
            bytes: Err("offline".into()),
            placeholder: None,
        });
        assert!(!s.requested_images.contains("retry-me"));
    }

    #[test]
    fn a_visible_image_survives_the_cache_limit() {
        let mut s = state();
        s.image_cache = Some(ImageCache::new(Arc::new(|| {})));
        s.current_images.insert("keep".into());
        s.requested_images.insert("keep".into());
        s.apply(Msg::Image {
            key: "keep".into(),
            bytes: Ok(vec![0]),
            placeholder: None,
        });
        for index in 0..(IMAGE_CACHE_LIMIT + 10) {
            s.apply(Msg::Image {
                key: format!("other-{index}"),
                bytes: Ok(vec![0]),
                placeholder: None,
            });
        }
        assert!(
            s.images.contains_key("keep"),
            "the screen's image is not evicted by another menu's load"
        );
        assert!(s.images.len() <= IMAGE_CACHE_LIMIT);
    }

    #[test]
    fn a_placeholder_lifts_after_the_short_blur_beat() {
        let mut s = state();
        s.image_cache = Some(ImageCache::new(Arc::new(|| {})));
        s.apply(Msg::Image {
            key: "poster".into(),
            bytes: Ok(vec![0]),
            placeholder: Some(vec![
                0;
                (crate::image::PLACEHOLDER_WIDTH * crate::image::PLACEHOLDER_HEIGHT * 4)
                    as usize
            ]),
        });
        assert!(s.placeholders.contains_key("poster"));
        assert_eq!(s.image_alpha("poster"), 0.0);
        std::thread::sleep(Duration::from_millis(130));
        assert!(s.poll());
        assert_eq!(s.image_alpha("poster"), 1.0);
    }

    #[test]
    fn a_revoked_session_returns_to_the_connect_screen_instead_of_a_dead_home() {
        let config = Config {
            server_url: "http://198.51.100.2:8096".into(),
            user_id: "u1".into(),
            user_name: "example-user".into(),
            token: "stale".into(),
            ..Config::default()
        };
        let mut state = State::from_config(config);
        state.apply_home(HomeLoad::Failed(
            "Your session expired. Sign in again.".into(),
        ));
        assert!(!state.is_signed_in());
        assert!(state.config.token.is_empty());
        assert!(matches!(state.step, Step::Failed(_)));
        assert_eq!(state.config.server_url, "http://198.51.100.2:8096");
    }

    #[test]
    fn subtitle_preferences_survive_a_restart_and_can_be_saved_on_close() {
        let config = Config {
            subtitle_size: 1.4,
            subtitle_delay_ms: 250,
            subtitle_font: Some(SubtitleFont::Serif),
            subtitle_color: Some(SubtitleColor::Cyan),
            subtitle_outline: Some(SubtitleOutline::Thick),
            subtitle_position: Some(SubtitlePosition::Top),
            subtitle_style: Some(SubtitleStyle::Bold),
            subtitle_opacity: Some(0.9),
            subtitle_background_opacity: Some(0.3),
            ..Config::fresh()
        };
        let mut state = State::from_config(config);
        assert!((state.in_app_subtitles.size - 1.4).abs() < 0.001);
        assert_eq!(state.in_app_subtitles.delay_ms, 250);
        assert_eq!(state.subtitle_font, SubtitleFont::Serif);
        assert_eq!(state.subtitle_color, SubtitleColor::Cyan);
        assert_eq!(state.subtitle_outline, SubtitleOutline::Thick);
        assert_eq!(state.subtitle_position, SubtitlePosition::Top);
        assert_eq!(state.subtitle_style, SubtitleStyle::Bold);
        assert!((state.subtitle_opacity - 0.9).abs() < 0.001);
        assert!((state.subtitle_background_opacity - 0.3).abs() < 0.001);

        state.in_app_subtitles.size = 1.2;
        state.in_app_subtitles.delay_ms = -300;
        state.subtitle_font = SubtitleFont::Mono;
        state.subtitle_opacity = 0.6;
        state.omdb_key = TextEdit::new("abcd1234").secure();
        state.save_preferences();
        assert!((state.config.subtitle_size - 1.2).abs() < 0.001);
        assert_eq!(state.config.subtitle_delay_ms, -300);
        assert_eq!(state.config.subtitle_font, Some(SubtitleFont::Mono));
        assert_eq!(state.config.subtitle_opacity, Some(0.6));
        assert_eq!(state.config.omdb_api_key.as_deref(), Some("abcd1234"));

        // A file written before the preference existed has 0; it must mean the default size.
        let old = State::from_config(Config::fresh());
        assert!((old.in_app_subtitles.size - 1.0).abs() < 0.001);
    }

    #[test]
    fn a_stored_session_opens_signed_in() {
        let config = Config {
            server_url: "http://198.51.100.2:8096".into(),
            user_id: "u1".into(),
            user_name: "example-user".into(),
            server_name: "example".into(),
            token: "tok".into(),
            device_id: "dev".into(),
            ..Config::default()
        };
        let s = State::from_config(config);
        assert!(s.is_signed_in());
    }

    #[test]
    fn an_empty_server_stops_before_any_request() {
        let mut s = state();
        assert!(!s.connect());
        match &s.step {
            Step::Failed(f) => {
                assert_eq!(f.stage, Stage::Server);
                assert_eq!(f.message, "Enter your server's address.");
            }
            other => panic!("expected a failure, got {other:?}"),
        }
    }

    #[test]
    fn a_server_without_a_user_stops_at_the_credentials_step() {
        let mut s = state();
        s.server = TextEdit::new("192.0.2.10:8096");
        assert!(!s.connect());
        match &s.step {
            Step::Failed(f) => assert_eq!(f.stage, Stage::Credentials),
            other => panic!("expected a failure, got {other:?}"),
        }
    }

    #[test]
    fn a_valid_form_normalises_the_url_and_goes_to_signing_in() {
        let mut s = state();
        s.server = TextEdit::new("  192.0.2.10:8096/  ");
        s.user = TextEdit::new("luis");
        assert!(s.connect());
        assert_eq!(s.step, Step::SigningIn);
        assert_eq!(s.config.server_url, "http://192.0.2.10:8096");
    }

    #[test]
    fn a_successful_sign_in_is_stored_and_lands_on_ready() {
        let mut s = state();
        s.apply(Msg::Connected(Ok(crate::net::Session {
            server_url: "http://198.51.100.2:8096".into(),
            server_name: "example".into(),
            version: "12.4.0".into(),
            user_id: "u1".into(),
            user_name: "example-user".into(),
            token: "tok".into(),
        })));
        assert!(s.is_signed_in());
        assert_eq!(s.config.token, "tok");
        assert_eq!(s.config.user_name, "example-user");
    }

    #[test]
    fn a_failed_sign_in_keeps_the_typed_server_so_the_user_can_fix_it() {
        let mut s = state();
        s.server = TextEdit::new("198.51.100.9:8096");
        s.user = TextEdit::new("luis");
        s.apply(Msg::Connected(Err(Failure {
            stage: Stage::Server,
            message: "Can't reach the server.".into(),
        })));
        assert!(!s.is_signed_in());
        assert_eq!(s.server.text, "198.51.100.9:8096");
        assert!(matches!(s.step, Step::Failed(_)));
    }

    #[test]
    fn the_home_ambient_uses_the_first_item_that_has_a_backdrop() {
        let mut s = state();
        let mut home = crate::demo::home();
        home.resume[0].backdrop_image_tags = None;
        home.latest[0].items[0].backdrop_image_tags = Some(vec!["ambient".into()]);
        s.home_data = home;
        let key = s.home_backdrop_key().expect("a backdrop is available");
        assert!(key.contains("his-girl-friday"));
        assert!(key.contains("1280"));
    }

    #[test]
    fn signing_out_clears_the_session_but_not_the_server() {
        let mut s = state();
        s.apply(Msg::Connected(Ok(crate::net::Session {
            server_url: "http://198.51.100.2:8096".into(),
            server_name: "example".into(),
            version: "12.4.0".into(),
            user_id: "u1".into(),
            user_name: "example-user".into(),
            token: "tok".into(),
        })));
        s.sign_out();
        assert!(!s.is_signed_in());
        assert!(s.config.token.is_empty());
        assert_eq!(s.config.server_url, "http://198.51.100.2:8096");
    }

    #[test]
    fn shortcuts_can_be_remapped_but_not_duplicated() {
        let mut s = state();
        assert_eq!(s.key_bindings.mute, "m");
        s.begin_rebind(ShortcutAction::Mute);
        assert!(s.capture_shortcut("b"));
        assert_eq!(s.key_bindings.mute, "b");
        s.begin_rebind(ShortcutAction::Fit);
        assert!(!s.capture_shortcut("b"), "fit cannot steal mute's key");
        assert_eq!(s.key_bindings.fit, "f");
        s.reset_shortcuts();
        assert_eq!(s.key_bindings.mute, "m");
        assert_eq!(s.key_bindings.fit, "f");
    }

    #[test]
    fn live_tv_is_remembered_as_a_place_to_restore() {
        let mut s = state();
        s.route = Route::LiveTv;
        s.remember_place();
        assert_eq!(
            s.config
                .last_place
                .as_ref()
                .map(|place| place.kind.as_str()),
            Some("livetv")
        );
    }

    #[test]
    fn text_size_cycles_and_updates_the_global_token_scale() {
        let mut s = state();
        assert_eq!(s.text_scale, 1.0);
        s.cycle_text_scale();
        assert_eq!(s.text_scale, 1.15);
        assert!((crate::theme::text_scale() - 1.15).abs() < f32::EPSILON);
        s.cycle_text_scale();
        s.cycle_text_scale();
        assert_eq!(s.text_scale, 0.85);
        s.cycle_text_scale();
        assert_eq!(s.text_scale, 1.0);
        crate::theme::set_text_scale(1.0);
    }

    #[test]
    fn the_collection_picker_targets_the_card_or_the_detail_item() {
        let mut s = state();
        let item = Item {
            id: "movie-1".into(),
            name: "A Film".into(),
            kind: "Movie".into(),
            ..Item::default()
        };
        s.open_collection_picker_for(item.clone());
        assert!(s.collection_picker_open);
        assert_eq!(s.collection_picker_ids, Some(vec!["movie-1".into()]));
        assert_eq!(
            s.collection_picker_item
                .as_ref()
                .map(|item| item.id.as_str()),
            Some("movie-1")
        );
        s.close_collection_picker();
        assert!(!s.collection_picker_open);
        assert!(s.collection_picker_ids.is_none());
    }
}
