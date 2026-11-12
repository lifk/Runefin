//! The in-app GStreamer session: starts the player on a worker, polls its clock, and reports
//! the same start/progress/stop lifecycle as the mpv fallback.

use std::time::{Duration, Instant};

use jellyfin_api::{Client, PlaybackInfoRequest, PlaybackInfoResponse, PlaybackReport, SortBy};

use super::*;
use crate::player::Player;

/// The speeds the player offers. 1× is in the middle so either direction is one press away.
const SPEEDS: [f64; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];

/// What the worker found on `fields=MediaSources`: the file to ask for subtitles and the
/// text tracks the app can draw.
pub(super) struct PlaybackStreams {
    pub(super) source: Option<String>,
    pub(super) container: Option<String>,
    pub(super) video: Option<(String, i32, i32, Option<String>)>,
    pub(super) subtitles: Vec<jellyfin_api::SubtitleTrack>,
    pub(super) audio: Vec<jellyfin_api::AudioTrack>,
    /// Present even when playback started from a card, whose `/Items` row did not ask for it.
    pub(super) trickplay: Option<
        std::collections::HashMap<
            String,
            std::collections::HashMap<String, jellyfin_api::TrickplayInfo>,
        >,
    >,
    pub(super) chapters: Vec<jellyfin_api::ChapterInfo>,
}

/// One downloaded track, parsed into cues on the worker.
pub(super) struct SubtitleCues {
    pub(super) track: usize,
    pub(super) cues: Vec<crate::subtitles::Cue>,
}

pub(super) struct PendingInApp {
    pub(super) item: Item,
    pub(super) session_id: String,
    pub(super) start_ms: i64,
    /// How many seconds the resume point was moved back, for the OSD confirmation.
    pub(super) rewound: i64,
    /// This device and the server disagree about where to start: (local ms, server ms).
    pub(super) conflict: Option<(i64, i64)>,
    /// A run resumed from a paused player should come back paused.
    pub(super) paused: bool,
    pub(super) live: bool,
    pub(super) live_stream_id: Option<String>,
    /// A trailer reports nothing to Jellyfin, so it never touches Continue Watching.
    pub(super) report: bool,
    /// A local file has no server streams, subtitles or neighbours, and no mpv fallback.
    pub(super) local: bool,
}

pub struct InAppSession {
    pub player: Player,
    pub item: Item,
    pub item_id: String,
    pub session_id: String,
    pub position_ms: i64,
    pub duration_ms: Option<i64>,
    pub paused: bool,
    /// While the pointer is dragging the timeline, the position the scrub preview shows.
    pub scrub_ms: Option<i64>,
    /// After a flushing seek the player's clock can keep reporting the pre-seek second for a
    /// frame or two. Hold the requested position until the clock reaches it, so the bar does
    /// not snap back and then jump forward again.
    pub seek_hold: Option<(i64, Instant)>,
    /// Segments already auto-skipped this session, so rewinding into one does not skip again.
    pub skipped_segments: std::collections::HashSet<String>,
    pub last_report: Instant,
    /// A tuner stream has no duration and no next episode; the OSD shows LIVE instead of a
    /// timeline, and stopping must close the server-side stream.
    pub live: bool,
    pub live_stream_id: Option<String>,
    /// Below 100 while the source stalls; the OSD shows it instead of pretending to play.
    pub buffering: Option<u32>,
    /// False for a trailer: start/progress/stop are not sent to Jellyfin.
    pub report: bool,
    /// What the server agreed to on start.
    pub play_method: &'static str,
    /// The plain-language reason for that method; the quality panel shows it.
    pub play_reason: Option<String>,
    pub play_session_id: Option<String>,
    /// Position the play started at, so history records watched time rather than the resume point.
    pub history_start_ms: i64,
    /// Added to the pipeline's position when the server's transcoded stream starts at zero
    /// rather than carrying the file's timestamps. Subtitle cues are always in file time.
    pub position_base_ms: i64,
    /// Whether `position_base_ms` has been decided for this session.
    pub position_calibrated: bool,
}

/// A Dolby Vision source, by the server's own range label.
fn item_is_dolby_vision(item: &Item) -> bool {
    item.media_sources
        .iter()
        .flat_map(|source| source.media_streams.iter())
        .filter(|stream| stream.kind == "Video")
        .filter_map(|stream| stream.video_range.as_deref())
        .any(|range| {
            let range = range.to_ascii_lowercase();
            range.contains("dovi") || range.contains("dolby")
        })
}

/// Why the server is converting: the chosen cap is the most useful fact, then the codec and
/// container the device profile refused. The quality panel shows this verbatim.
fn transcode_reason(
    source: &jellyfin_api::PlaybackMediaSource,
    max_bitrate: Option<i64>,
) -> String {
    if let Some(cap) = max_bitrate
        && let Some(bitrate) = source.bitrate
        && bitrate > cap
    {
        return format!(
            "The {} Mbps cap is below this file's {} Mbps.",
            cap / 1_000_000,
            bitrate / 1_000_000
        );
    }
    let codec = source
        .media_streams
        .iter()
        .find(|stream| stream.kind == "Video")
        .and_then(|stream| stream.codec.as_deref());
    let container = source.container.as_deref().unwrap_or("file");
    match codec {
        Some(codec) => format!("The device profile cannot direct-play {codec} in {container}."),
        None => format!("The device profile cannot direct-play this {container} file."),
    }
}

/// The offset to add to a transcoded stream's clock so it reads in file time. A stream that
/// starts at the resume point but reports zero needs the resume point added; one that carries
/// the file's own timestamps needs nothing.
fn transcode_position_base(raw_ms: i64, expected_ms: i64) -> i64 {
    if raw_ms > 0 && raw_ms + 2_000 < expected_ms {
        expected_ms
    } else {
        0
    }
}

impl InAppSession {
    /// Seek in file time. A stream whose timeline starts at the resume point needs the base
    /// removed before the pipeline sees it.
    pub(super) fn seek_to(&self, target_ms: i64) {
        self.player.seek_ms(target_ms - self.position_base_ms);
    }
}

/// Whether the server's transcoding URL already carries the requested start. Jellyfin writes
/// `StartTimeTicks`; a URL without it transcodes from the file's beginning, so the picture
/// must be seeked after the first frame or the resume clock would be a lie.
fn url_starts_at(url: &str, start_ms: i64) -> bool {
    if start_ms <= 0 {
        return true;
    }
    let lower = url.to_ascii_lowercase();
    let Some(at) = lower
        .find("starttimeticks=")
        .or_else(|| lower.find("startticks="))
    else {
        return false;
    };
    let value = url[at..]
        .split_once('=')
        .map(|(_, value)| value)
        .unwrap_or_default()
        .split('&')
        .next()
        .unwrap_or_default();
    value.parse::<i64>().is_ok_and(|ticks| {
        (ticks - start_ms.saturating_mul(jellyfin_api::TICKS_PER_MS)).abs()
            <= 2_000 * jellyfin_api::TICKS_PER_MS
    })
}

/// The session's length, for clamping a second sync window to the file.
fn session_duration_ms(state: &State) -> i64 {
    state
        .in_app
        .as_ref()
        .and_then(|session| session.duration_ms)
        .unwrap_or(0)
}

/// Where the pipeline should start: the resume point, unless the negotiated source already
/// begins there (a transcode URL that carries `StartTimeTicks`).
fn resume_target(method: &'static str, url: &str, start_ms: i64) -> Option<i64> {
    let needs_seek = start_ms > 0 && (method != "Transcode" || !url_starts_at(url, start_ms));
    needs_seek.then_some(start_ms)
}

/// The start worker's answer as the session wants it: the position the first picture shows, and
/// the reason it is not the resume point when it could not be reached.
fn resume_notice(
    started_ms: Option<i64>,
    resume_at: Option<i64>,
    notice: Option<String>,
) -> (Option<i64>, Option<String>) {
    match (started_ms, resume_at) {
        (Some(started), _) => (Some(started), notice),
        (None, Some(target)) => (
            Some(0),
            Some(format!(
                "Could not resume at {}; starting from the beginning.",
                crate::detail::format_time(target)
            )),
        ),
        (None, None) => (None, notice),
    }
}

/// Pick the URL and the method the server offered. Direct play wins unless the reader capped
/// the bitrate below the file's own or the server says the profile cannot play it.
fn choose_playback(
    client: &Client,
    response: Option<&PlaybackInfoResponse>,
    source_id: Option<&str>,
    max_bitrate: Option<i64>,
    fallback: &str,
) -> (String, &'static str, Option<String>, Option<String>) {
    let Some(response) = response else {
        return (
            fallback.to_string(),
            "DirectPlay",
            None,
            Some("PlaybackInfo failed; playing the file directly.".into()),
        );
    };
    let source = source_id
        .and_then(|id| response.media_sources.iter().find(|source| source.id == id))
        .or_else(|| response.media_sources.first());
    let Some(source) = source else {
        return (
            fallback.to_string(),
            "DirectPlay",
            None,
            Some("PlaybackInfo returned no source; playing the file directly.".into()),
        );
    };
    let play_session = response
        .play_session_id
        .clone()
        .or_else(|| source.play_session_id.clone());
    let capped = max_bitrate.is_some_and(|cap| source.bitrate.is_some_and(|bitrate| bitrate > cap));
    if source.supports_direct_play && !capped {
        return (fallback.to_string(), "DirectPlay", play_session, None);
    }
    if !source.supports_direct_play
        && source.supports_direct_stream
        && let Some(url) = &source.direct_stream_url
    {
        return (
            client.authed_url(url),
            "DirectStream",
            play_session,
            Some("The server is copying the video and converting its audio or subtitles.".into()),
        );
    }
    if source.supports_transcoding
        && let Some(url) = &source.transcoding_url
    {
        return (
            client.authed_url(url),
            "Transcode",
            play_session,
            Some(transcode_reason(source, max_bitrate)),
        );
    }
    (
        fallback.to_string(),
        "DirectPlay",
        None,
        Some("The server offered no playable URL; playing the file directly.".into()),
    )
}

impl State {
    pub(super) fn play_in_app(&mut self, item: Item) {
        self.stop_theme_media();
        // A single item is not a run: clicking a card or a detail page ends whatever
        // "Play all" started.
        self.queue.clear();
        self.queue_at = 0;
        self.in_app_run_episodes = 0;
        self.play_in_app_at(item, None, false);
    }

    /// Start a run in order (`Play all`) or shuffled (`Shuffle`). The queue is what EOS and the
    /// OSD's next/previous buttons follow, so a library or a season plays through without
    /// going back to a grid between items.
    pub fn play_queue(&mut self, items: Vec<Item>, at: usize, shuffle: bool) {
        let mut items = items;
        if items.is_empty() {
            return;
        }
        if shuffle {
            shuffle_items(&mut items);
        }
        let at = at.min(items.len() - 1);
        let first = items[at].clone();
        self.queue = items;
        self.queue_at = at;
        self.in_app_run_episodes = 0;
        self.play_in_app_at(first, None, false);
    }

    /// The queue's neighbour in that direction, if there is one.
    pub(crate) fn queue_step(&mut self, forward: bool) -> Option<Item> {
        if self.queue.is_empty() {
            return None;
        }
        let at = if forward {
            self.queue_at + 1
        } else {
            self.queue_at.checked_sub(1)?
        };
        let item = self.queue.get(at)?.clone();
        self.queue_at = at;
        Some(item)
    }

    /// The one place that opens or closes a player panel. Opening one closes the rest, the
    /// statistics overlay and any half-read subtitle search, so the `if/else if` render chain
    /// always shows exactly the panel the reader asked for. A panel opened from the settings
    /// sheet leaves `gear_returns` set, so closing it comes back to the sheet.
    pub fn open_player_panel(&mut self, panel: PlayerPanel) {
        if panel != PlayerPanel::None && panel != PlayerPanel::Gear && self.in_app_gear_open {
            self.in_app_gear_returns = true;
        }
        self.in_app_gear_open = panel == PlayerPanel::Gear;
        if self.in_app_gear_open {
            self.in_app_gear_returns = false;
        }
        self.in_app_audio_open = panel == PlayerPanel::Audio;
        self.in_app_subtitles.open = panel == PlayerPanel::Subtitles;
        self.in_app_quality_open = panel == PlayerPanel::Quality;
        self.in_app_chapters_open = panel == PlayerPanel::Chapters;
        self.in_app_queue_open = panel == PlayerPanel::Queue;
        self.in_app_sleep_open = panel == PlayerPanel::Sleep;
        self.in_app_goto_open = panel == PlayerPanel::Goto;
        if panel != PlayerPanel::Subtitles {
            self.in_app_subtitle_search.open = false;
        }
        self.in_app_stats = false;
        self.in_app_video = None;
        self.in_app_container = None;
        if panel == PlayerPanel::Goto {
            self.in_app_goto = rune::TextEdit::new("");
            self.in_app_goto.focused = true;
        }
        if panel != PlayerPanel::None {
            // Reading a panel must not race the idle timer.
            self.osd_hide_at = None;
        }
    }

    /// A panel is on screen, so `Esc` should close it before it stops the player.
    pub fn player_panel_open(&self) -> bool {
        self.in_app_audio_open
            || self.in_app_subtitles.open
            || self.in_app_quality_open
            || self.in_app_chapters_open
            || self.in_app_queue_open
            || self.in_app_sleep_open
            || self.in_app_goto_open
    }

    /// The settings sheet is the last thing in the panel slot. After a panel opened from it
    /// closes, it is still the thing to show.
    pub fn player_gear_visible(&self) -> bool {
        self.in_app_gear_open || (self.in_app_gear_returns && !self.player_panel_open())
    }

    pub fn toggle_in_app_gear(&mut self) {
        self.show_in_app_osd();
        if self.player_gear_visible() {
            self.close_player_gear();
        } else {
            self.open_player_panel(PlayerPanel::Gear);
        }
    }

    pub fn close_player_gear(&mut self) {
        self.in_app_gear_open = false;
        self.in_app_gear_returns = false;
    }

    pub fn close_player_panel(&mut self) {
        self.open_player_panel(PlayerPanel::None);
    }

    pub fn toggle_in_app_queue(&mut self) {
        self.show_in_app_osd();
        let panel = if self.in_app_queue_open {
            PlayerPanel::None
        } else {
            PlayerPanel::Queue
        };
        self.open_player_panel(panel);
    }

    pub fn close_in_app_queue(&mut self) {
        self.in_app_queue_open = false;
    }

    /// Jump to a place in the run. The item starts from its own resume point, exactly as if it
    /// had been reached by EOS.
    pub fn open_in_app_transfer(&mut self) {
        let Some(session) = &self.in_app else {
            return;
        };
        let item = session.item.clone();
        self.cast_item = Some(item);
        self.cast_picker_open = true;
        self.cast_loading = true;
        self.transfer_after_cast = true;
        let Some(client) = self.client() else {
            self.show_toast("Your session expired. Sign in again.");
            return;
        };
        self.cast_sessions_load.request((), move |()| {
            client.sessions().map_err(|error| error.user_message())
        });
    }

    /// Take this device's newer checkpoint. The card disappears; the film jumps.
    pub fn resume_from_local(&mut self) {
        let Some((local, _)) = self.resume_conflict.take() else {
            return;
        };
        if let Some(session) = &mut self.in_app {
            session.seek_to(local);
            session.position_ms = local;
            session.seek_hold = Some((local, Instant::now()));
        }
        self.show_toast("Resumed from this device.");
    }

    pub fn resume_from_server(&mut self) {
        self.resume_conflict = None;
        if let Some(item_id) = self.in_app.as_ref().map(|session| session.item_id.clone()) {
            // Otherwise the same stale local position would prompt on every launch.
            self.forget_checkpoint(&item_id);
        }
        self.show_in_app_osd();
    }

    /// A special feature plays in the player without reporting and outside the run.
    pub fn play_extra(&mut self, item: Item) {
        self.queue.clear();
        self.queue_at = 0;
        self.play_in_app_media(item, None, None, false, false);
    }

    pub fn play_queue_at(&mut self, at: usize) {
        let Some(item) = self.queue.get(at).cloned() else {
            return;
        };
        self.queue_at = at;
        self.in_app_queue_open = false;
        self.play_in_app_at(item, None, false);
    }

    pub fn queue_next_available(&self) -> bool {
        !self.queue.is_empty() && self.queue_at + 1 < self.queue.len()
    }

    pub fn queue_previous_available(&self) -> bool {
        !self.queue.is_empty() && self.queue_at > 0
    }

    /// Add an item to the end of the run without interrupting the current one.
    pub fn queue_add(&mut self, item: Item) {
        if !is_playable(&item) {
            return;
        }
        if self.queue.is_empty()
            && let Some(session) = &self.in_app
        {
            self.queue.push(session.item.clone());
            self.queue_at = 0;
        }
        self.queue.push(item);
        self.request_images();
        self.show_toast("Added to the queue.");
    }

    /// Turn the current run into a playlist. The name says where it came from; the ids are the
    /// queue in order, including the item playing now.
    /// Change quality and renegotiate at the current position.
    pub fn apply_in_app_quality(&mut self, quality: crate::config::PlaybackQuality) {
        self.set_playback_quality(quality);
        self.in_app_quality_open = false;
        let Some(session) = &self.in_app else {
            return;
        };
        if session.live || !session.report {
            return;
        }
        let item = session.item.clone();
        let position = session.position_ms;
        let paused = session.paused;
        let report = session.report;
        self.quality_choices.insert(item.id.clone(), quality);
        self.play_in_app_media(item, None, Some(position), paused, report);
    }

    pub fn save_queue_as_playlist(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let Some(client) = self.client() else {
            self.show_toast("Your session expired. Sign in again.");
            return;
        };
        let ids: Vec<String> = self.queue.iter().map(|item| item.id.clone()).collect();
        let name = self
            .queue
            .get(self.queue_at)
            .map(|item| {
                let title = item.series_name.as_deref().unwrap_or(&item.name);
                format!("Queue · {title}")
            })
            .unwrap_or_else(|| "Queue".to_string());
        self.playlist_create_load
            .request(name.clone(), move |name| {
                let id = client
                    .create_playlist(&name, &ids)
                    .map_err(|error| error.user_message())?;
                Ok(Item {
                    id,
                    name,
                    kind: "Playlist".into(),
                    ..Item::default()
                })
            });
        self.show_toast("Saving the queue as a playlist…");
    }

    pub fn queue_remove(&mut self, at: usize) {
        if at >= self.queue.len() || self.queue.len() <= 1 {
            return;
        }
        self.queue.remove(at);
        if at < self.queue_at {
            self.queue_at -= 1;
        } else if at == self.queue_at {
            self.queue_at = self.queue_at.min(self.queue.len() - 1);
        }
        self.show_toast("Removed from the queue.");
    }

    pub fn queue_move(&mut self, at: usize, delta: isize) {
        let len = self.queue.len();
        if at >= len {
            return;
        }
        let target = (at as isize + delta).clamp(0, len as isize - 1) as usize;
        if target == at {
            return;
        }
        self.queue.swap(at, target);
        if self.queue_at == at {
            self.queue_at = target;
        } else if self.queue_at == target {
            self.queue_at = at;
        }
    }

    /// Play all of a library's loaded items, or the same set shuffled. Only items the player can
    /// open on their own are queued: a series expands to episodes on its own page, not here.
    pub fn play_library_queue(&mut self, shuffle: bool) {
        let Some(library) = &self.library else {
            return;
        };
        let loaded: Vec<Item> = library
            .items
            .iter()
            .filter(|item| is_playable(item))
            .cloned()
            .collect();
        if loaded.is_empty() {
            self.detail_notice = Some("This library has nothing the player can open.".into());
            return;
        }
        // "Play all" means the library, not the first page. When the grid has not reached the
        // end yet, the run is built from one wide request with the same filters the grid shows.
        // A TV library lists series, which are containers, not streams: its "play" is an
        // episode somewhere below. The server randomises the order for a shuffle; doing it
        // client-side would only shuffle the page we happened to load.
        if library.collection_type.as_deref() == Some("tvshows") {
            let Some(client) = self.client() else {
                self.detail_notice = Some("Your session expired. Sign in again.".into());
                return;
            };
            let id = library.id.clone();
            let sort = if shuffle {
                SortBy::Random
            } else {
                SortBy::SortName
            };
            self.queue_load.request(shuffle, move |shuffle| {
                let items = client
                    .episodes_in(&id, sort, 100)
                    .map(|items| items.into_iter().filter(is_playable).collect::<Vec<_>>())
                    .unwrap_or_default();
                (items, shuffle)
            });
            return;
        }
        if library.total as usize > library.items.len() {
            let Some(client) = self.client() else {
                self.detail_notice = Some("Your session expired. Sign in again.".into());
                return;
            };
            let query = library.queue_query();
            self.queue_load.request(shuffle, move |shuffle| {
                let items = client
                    .items(&query)
                    .map(|page| {
                        page.items
                            .into_iter()
                            .filter(is_playable)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                (items, shuffle)
            });
            return;
        }
        let at = if shuffle { 0 } else { first_unplayed(&loaded) };
        self.play_queue(loaded, at, shuffle);
    }

    /// Play or shuffle the season on screen. The episodes are already loaded, so this starts
    /// without a request; the run begins at the first unfinished episode, like the primary Play.
    pub fn play_season_queue(&mut self, shuffle: bool) {
        let Some(detail) = &self.detail else {
            return;
        };
        let items: Vec<Item> = detail
            .episodes
            .iter()
            .filter(|episode| is_playable(episode))
            .cloned()
            .collect();
        if items.is_empty() {
            self.detail_notice = Some("No episodes to play yet.".into());
            return;
        }
        let at = if shuffle { 0 } else { first_unplayed(&items) };
        self.play_queue(items, at, shuffle);
    }

    /// The same for a collection page: its members, shuffled or in the server's order.
    pub fn play_collection_queue(&mut self, shuffle: bool) {
        let Some(detail) = &self.detail else {
            return;
        };
        let items: Vec<Item> = detail
            .collection
            .iter()
            .filter(|item| is_playable(item))
            .cloned()
            .collect();
        if items.is_empty() {
            self.detail_notice = Some("This collection has nothing playable yet.".into());
            return;
        }
        let at = if shuffle {
            0
        } else {
            items
                .iter()
                .position(|item| item.user_data.as_ref().is_none_or(|user| !user.played))
                .unwrap_or(0)
        };
        self.play_queue(items, at, shuffle);
    }

    /// The one playback entry point that leaves the queue alone: up-next, a queue step, and a
    /// resumed run all arrive here.
    pub(super) fn play_in_app_at(&mut self, item: Item, start_override: Option<i64>, paused: bool) {
        if !self.queue.is_empty() {
            self.in_app_run_episodes = self.in_app_run_episodes.saturating_add(1);
        }
        self.still_watching_open = false;
        self.still_watching_next = None;
        self.still_watching_since = None;
        self.in_app_play_since = Instant::now();
        self.play_in_app_media(item, None, start_override, paused, true);
    }

    /// A downloaded trailer: same in-app player, no Jellyfin reporting, no server stream work.
    pub(super) fn play_in_app_local(&mut self, item: Item, path: std::path::PathBuf) {
        self.play_in_app_media(item, Some(path), None, false, false);
    }

    /// A server-hosted trailer: the normal stream path, but no reporting.
    pub(super) fn play_in_app_trailer(&mut self, item: Item) {
        self.play_in_app_media(item, None, None, false, false);
    }

    fn play_in_app_media(
        &mut self,
        item: Item,
        local: Option<std::path::PathBuf>,
        start_override: Option<i64>,
        paused: bool,
        report: bool,
    ) {
        if self.playback.is_some() {
            self.stop_playback();
        }
        if self.in_app.is_some() {
            self.finish_in_app();
        }
        let Some(client) = self.client() else {
            self.detail_notice = Some("Your session expired. Sign in again.".into());
            return;
        };
        // A source chosen for one item must never be sent for another.
        let chosen = self
            .in_app_source
            .take()
            .filter(|_| self.in_app_source_for.as_deref() == Some(item.id.as_str()));
        self.in_app_source_for = None;
        let source = chosen.or_else(|| self.version_choices.get(&item.id).cloned());
        // A quality chosen for this item wins over the last global choice.
        let quality = self
            .quality_choices
            .get(&item.id)
            .copied()
            .unwrap_or(self.playback_quality);
        self.playback_quality = quality;
        let url = match &local {
            Some(path) => format!("file://{}", path.display()),
            None => {
                let Some(url) = client.direct_stream_url_with_source(&item.id, source.as_deref())
                else {
                    self.detail_notice = Some("This item has no playable stream.".into());
                    return;
                };
                url
            }
        };
        self.in_app_speed = 1.0;
        // The volume the reader left is the volume a new film starts at.
        self.in_app_volume = self.config.volume.unwrap_or(1.0).clamp(0.0, 2.0);
        let session_id = format!(
            "runefin-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let server_start = start_override.unwrap_or_else(|| {
            item.user_data
                .as_ref()
                .and_then(|user| user.position_ms())
                .unwrap_or(0)
        });
        // A local checkpoint newer than the server's last report wins after a crash. The
        // conflict is only a few seconds in normal use; do not resurrect a stale one, and never
        // override an explicit Start over.
        let reported = server_start;
        let local = start_override
            .is_none()
            .then(|| {
                (!item.user_data.as_ref().is_some_and(|user| user.played))
                    .then(|| self.local_checkpoint(&item.id))
                    .flatten()
            })
            .flatten()
            .filter(|local| *local > reported + 5_000);
        // Start where the server says, then let the conflict card offer the local position.
        // Choosing there is one seek, and the film already has a picture meanwhile.
        // Resume a little earlier than the server's second. This is deliberately not applied to
        // Start over or a trailer (both pass an explicit override).
        let rewound = if start_override.is_none() && reported > 0 {
            (self.resume_rewind_seconds * 1_000).min(reported)
        } else {
            0
        };
        let start_ms = reported - rewound;
        self.pending_in_app = Some(PendingInApp {
            item: item.clone(),
            session_id: session_id.clone(),
            start_ms,
            rewound,
            conflict: local.map(|local| (local, reported)),
            paused,
            live: false,
            live_stream_id: None,
            report,
            local: local.is_some(),
        });
        self.launching = true;
        self.in_app_launching = true;
        self.playback_error = None;
        let surface = rune::NativeSurface::new();
        let waker = self.waker.clone();
        self.in_app_segments.clear();
        if local.is_none() {
            let segment_client = client.clone();
            let segment_item = item.id.clone();
            self.in_app_segments_load
                .request(segment_item, move |item_id| {
                    segment_client
                        .media_segments(&item_id)
                        .map_err(|error| error.user_message())
                });
        } else {
            self.in_app_segments_load.cancel();
        }
        // Subtitle preferences survive an episode change; the tracks and cues do not.
        let size = self.in_app_subtitles.size;
        let delay_ms = self.in_app_subtitles.delay_ms;
        self.in_app_subtitles = Subtitles {
            size,
            delay_ms,
            ..Subtitles::default()
        };
        self.in_app_subtitle_load.cancel();
        self.in_app_image_load.cancel();
        self.in_app_audio_tracks.clear();
        self.in_app_audio_chosen = false;
        self.in_app_audio_open = false;
        self.in_app_video = None;
        self.in_app_container = None;
        self.in_app_stats = false;
        self.in_app_next = None;
        self.in_app_prev = None;
        self.in_app_next_dismissed = local.is_some();
        self.in_app_neighbours_load.cancel();
        if local.is_none() {
            self.request_in_app_streams(&client, &item.id);
            self.request_chapter_images(&item);
            // The neighbours are only useful near the end, so one request loads them alongside
            // playback instead of delaying the first frame. It is scoped to the whole series,
            // not the season: stepping back from S02E01 has to reach S01's finale, and one
            // request now answers both directions.
            if let Some(scope) = item.series_id.clone().or_else(|| item.season_id.clone()) {
                let current_id = item.id.clone();
                let neighbour_client = client.clone();
                self.in_app_neighbours_load
                    .request(current_id.clone(), move |current_id| {
                        let episodes = neighbour_client
                            .episodes_under(&scope)
                            .map_err(|error| error.user_message())?;
                        Ok(episode_neighbours(&episodes, &current_id))
                    });
            }
        }
        let playback_request = client.user_id().map(|user_id| PlaybackInfoRequest {
            user_id: user_id.to_string(),
            start_time_ticks: start_ms.saturating_mul(jellyfin_api::TICKS_PER_MS),
            max_streaming_bitrate: quality.max_bitrate(),
            media_source_id: source.clone(),
            audio_stream_index: None,
            subtitle_stream_index: None,
            max_audio_channels: self.downmix_stereo.then_some(2),
            enable_direct_play: true,
            enable_direct_stream: true,
            enable_transcoding: true,
            allow_video_stream_copy: true,
            allow_audio_stream_copy: true,
            device_profile: Client::device_profile(),
        });
        let fallback = url.clone();
        let item_id = item.id.clone();
        let cap = quality.max_bitrate();
        self.in_app_start.request(
            (
                client,
                item_id,
                source,
                playback_request,
                cap,
                fallback,
                surface,
                waker,
                start_ms,
            ),
            |(client, item_id, source, request, cap, fallback, surface, waker, start_ms)| {
                let negotiated = request
                    .as_ref()
                    .and_then(|request| client.playback_info(&item_id, request).ok());
                let (url, method, play_session_id, notice) = choose_playback(
                    &client,
                    negotiated.as_ref(),
                    source.as_deref(),
                    cap,
                    &fallback,
                );
                let resume_at = resume_target(method, &url, start_ms);
                match Player::start_at(&url, surface.clone(), waker.clone(), resume_at) {
                    Ok(started) => {
                        let (started_ms, notice) =
                            resume_notice(started.started_ms, resume_at, notice);
                        Ok(InAppStart {
                            player: started.player,
                            method,
                            play_session_id,
                            notice,
                            started_ms,
                        })
                    }
                    Err(first) => {
                        // A direct play that produced no frame is worth one more try through the
                        // server's transcoder: it can tone-map or re-encode whatever the local
                        // pipeline could not open (a Dolby Vision mp4, for example) instead of
                        // sending the item to the mpv fallback.
                        let direct = method == "DirectPlay" || method == "DirectStream";
                        let Some(retry) = request.as_ref().filter(|_| direct).map(|request| {
                            let mut retry = request.clone();
                            retry.enable_direct_play = false;
                            retry.enable_direct_stream = false;
                            retry.enable_transcoding = true;
                            retry.max_streaming_bitrate = Some(8_000_000);
                            retry
                        }) else {
                            return Err(Failure {
                                stage: Stage::Player,
                                message: format!("{method}: {first}"),
                            });
                        };
                        let Some(response) = client.playback_info(&item_id, &retry).ok() else {
                            return Err(Failure {
                                stage: Stage::Player,
                                message: format!(
                                    "{method}: {first}; the server offered no transcoded copy"
                                ),
                            });
                        };
                        let (url, method, play_session_id, _) = choose_playback(
                            &client,
                            Some(&response),
                            source.as_deref(),
                            None,
                            &fallback,
                        );
                        let resume_at = resume_target(method, &url, start_ms);
                        match Player::start_at(&url, surface, waker, resume_at) {
                            Ok(started) => {
                                let (started_ms, notice) = resume_notice(
                                    started.started_ms,
                                    resume_at,
                                    Some(
                                        "Direct play produced no frames; the server is \
                                         transcoding this item."
                                            .into(),
                                    ),
                                );
                                Ok(InAppStart {
                                    player: started.player,
                                    method,
                                    play_session_id,
                                    notice,
                                    started_ms,
                                })
                            }
                            Err(second) => Err(Failure {
                                stage: Stage::Player,
                                message: format!("{method}: {first}; transcode retry: {second}"),
                            }),
                        }
                    }
                }
            },
        );
    }

    /// The track the reader's defaults pick, or `None` to leave subtitles off. "Forced only"
    /// takes a forced track (the dub leaves foreign dialogue there) and never a full one;
    /// "Always" takes the preferred language, falling back to the first track; "Off" takes none.
    fn preferred_subtitle_track(&self) -> Option<usize> {
        let tracks = &self.in_app_subtitles.tracks;
        let wanted = self.subtitle_language.to_ascii_lowercase();
        let matches_language = |track: &jellyfin_api::SubtitleTrack| {
            wanted.is_empty()
                || track
                    .language
                    .as_deref()
                    .is_some_and(|language| language.eq_ignore_ascii_case(&wanted))
        };
        match self.subtitle_mode {
            SubtitleMode::Off => None,
            SubtitleMode::Forced => tracks
                .iter()
                .position(|track| track.is_text && track.is_forced && matches_language(track))
                .or_else(|| {
                    tracks
                        .iter()
                        .position(|track| track.is_text && track.is_forced)
                }),
            SubtitleMode::Always if tracks.iter().all(|track| !track.is_text) => None,
            SubtitleMode::Always => Some(
                tracks
                    .iter()
                    .position(|track| track.is_text && matches_language(track))
                    .unwrap_or_else(|| tracks.iter().position(|track| track.is_text).unwrap_or(0)),
            ),
        }
    }

    /// Select the preferred audio language once, when the tracks first arrive. Manual choices
    /// later are never overridden: this only runs while nothing has been picked this session.
    fn apply_audio_preference(&mut self) {
        if self.audio_language.is_empty() || self.in_app_audio_chosen {
            return;
        }
        let wanted = self.audio_language.to_ascii_lowercase();
        let chosen = self
            .in_app_audio_tracks
            .iter()
            .find(|track| {
                track
                    .language
                    .as_deref()
                    .is_some_and(|language| language.eq_ignore_ascii_case(&wanted))
            })
            .map(|track| track.ordinal);
        if let Some(ordinal) = chosen
            && let Some(session) = &self.in_app
        {
            session.player.select_audio(ordinal);
            self.in_app_audio_chosen = true;
        }
    }

    /// Download and parse one selected track. The server converts embedded ASS to SRT, so a
    /// single parser covers both external and in-container text subtitles.
    pub fn select_in_app_subtitle(&mut self, at: Option<usize>) {
        self.cancel_subtitle_sync();
        self.in_app_subtitles.sync_note = None;
        self.show_in_app_osd();
        self.in_app_subtitles.open = false;
        self.in_app_subtitles.selected = at;
        self.in_app_subtitles.cues.clear();
        self.in_app_subtitles.error = None;
        self.in_app_subtitle_load.cancel();
        self.in_app_image_load.cancel();
        self.in_app_subtitles.image = None;
        self.in_app_subtitles.image_loading = false;
        self.in_app_subtitles.image_error = None;
        self.in_app_subtitles.loading = false;
        let Some(at) = at else {
            return;
        };
        let Some(track) = self.in_app_subtitles.tracks.get(at).cloned() else {
            self.in_app_subtitles.selected = None;
            return;
        };
        if !track.is_text {
            self.select_in_app_image_subtitle_style(at, &track);
            return;
        }
        let Some(source) = self.in_app_subtitles.source.clone() else {
            self.in_app_subtitles.selected = None;
            self.in_app_subtitles.error = Some("No media source for subtitles.".into());
            return;
        };
        let Some(client) = self.client() else {
            return;
        };
        let Some(item_id) = self.in_app.as_ref().map(|session| session.item_id.clone()) else {
            return;
        };
        // ASS keeps its own alignment and signs when the server serves it as ASS; everything
        // else is converted to SRT by the server and parsed there.
        let format = subtitle_format(&track);
        let is_ass = format == "ass";
        let Some(url) = client.subtitle_url_with(&item_id, &source, track.index, format) else {
            return;
        };
        self.in_app_subtitles.loading = true;
        self.in_app_subtitle_load
            .request((at, url), move |(at, url)| {
                let bytes = client
                    .fetch_bytes(&url)
                    .map_err(|error| error.user_message())?;
                Ok(SubtitleCues {
                    track: at,
                    cues: if is_ass {
                        crate::subtitles::parse_ass(&bytes)
                    } else {
                        crate::subtitles::parse(&bytes)
                    },
                })
            });
    }

    /// Start a PGS/VobSub track. The server will not serve image subtitles as a file, so the
    /// worker extracts the stream from the same direct-play URL with ffmpeg and decodes it in
    /// process. The result is a list of RGBA rectangles placed over the video.
    fn select_in_app_image_subtitle_style(
        &mut self,
        at: usize,
        track: &jellyfin_api::SubtitleTrack,
    ) {
        let Some(codec) = crate::image_subtitles::ImageCodec::for_codec(track.codec.as_deref())
        else {
            self.in_app_subtitles.selected = None;
            self.in_app_subtitles.image_error = Some(
                "This image subtitle format is not supported yet. Search online for a text track."
                    .into(),
            );
            return;
        };
        let Some(client) = self.client() else {
            return;
        };
        let Some(item_id) = self.in_app.as_ref().map(|session| session.item_id.clone()) else {
            return;
        };
        let stream_index = track.index;
        self.in_app_subtitles.image_loading = true;
        self.in_app_subtitles.image_error = None;
        self.show_toast("Preparing image subtitles…".to_string());
        self.in_app_image_load.request(at, move |at| {
            crate::image_subtitles::load(&client, &item_id, stream_index, codec)
                .map(|image| (at, image))
        });
    }

    pub(super) fn apply_in_app_image_subtitle_style(
        &mut self,
        load: Result<(usize, crate::image_subtitles::ImageSubtitle), String>,
    ) {
        self.in_app_subtitles.image_loading = false;
        match load {
            Ok((at, image)) if self.in_app_subtitles.selected == Some(at) => {
                self.in_app_subtitles.image = Some(image);
                self.in_app_subtitles.image_error = None;
                self.sync_image_subtitles();
                self.show_toast("Image subtitles ready".to_string());
            }
            Ok(_) => {}
            Err(error) => {
                self.in_app_subtitles.image = None;
                self.in_app_subtitles.image_error = Some(error);
            }
        }
    }

    /// Move the image track to the display set under the playhead. Decoding one PGS display set
    /// is sub-millisecond; the parser keeps the stream and only the active rectangles live.
    pub fn sync_image_subtitles(&mut self) -> bool {
        let Some(image) = &mut self.in_app_subtitles.image else {
            return false;
        };
        let Some(session) = &self.in_app else {
            return false;
        };
        let position = session
            .scrub_ms
            .unwrap_or(session.position_ms)
            .saturating_sub(self.in_app_subtitles.delay_ms)
            .max(0);
        image.refresh(position)
    }

    pub(super) fn apply_in_app_neighbours(&mut self, load: Result<EpisodeNeighbours, String>) {
        // A missing neighbour is the normal first/last-episode answer, not an error to surface.
        let Ok((previous, next)) = load else {
            return;
        };
        self.in_app_prev = previous;
        self.in_app_next = next;
        if !self.in_app_next_dismissed
            && let Some(next) = self.in_app_next.clone()
        {
            self.request_next_episode_image(&next);
        }
    }

    /// Roll to the next episode at EOS unless the reader dismissed the card. Stops and reports
    /// the finished episode first, so Continue Watching is never left on a phantom session.
    pub fn play_in_app_next(&mut self) {
        let Some(next) = self.in_app_next.take() else {
            return;
        };
        self.in_app_next_dismissed = true;
        self.play_in_app_at(next, None, false);
    }

    pub fn dismiss_in_app_next(&mut self) {
        self.in_app_next = None;
        self.in_app_next_dismissed = true;
    }

    /// The last 30 seconds of an episode with a known next one. This deliberately ignores the
    /// OSD and pointer: it also keeps the chrome awake so the offer is visible without a move.
    pub fn next_up_eligible(&self) -> bool {
        let Some(session) = &self.in_app else {
            return false;
        };
        let Some(duration) = session.duration_ms else {
            return false;
        };
        self.in_app_next.is_some()
            && !self.queue_next_available()
            && !self.in_app_next_dismissed
            && !self.in_app_subtitles.open
            && !self.in_app_chapters_open
            && !self.in_app_queue_open
            && !self.in_app_audio_open
            && session.position_ms < duration
            && duration - session.position_ms <= 30_000
    }

    /// Re-read the item's media streams. Called when playback starts and after an online
    /// subtitle download, when the server has just attached a new external track.
    fn request_in_app_streams(&mut self, client: &jellyfin_api::Client, item_id: &str) {
        let subtitle_client = client.clone();
        self.in_app_streams_load
            .request(item_id.to_string(), move |item_id| {
                let item = subtitle_client
                    .playback_item(&item_id)
                    .map_err(|error| error.user_message())?;
                Ok(PlaybackStreams {
                    source: item.primary_media_source().map(|source| source.id.clone()),
                    container: item
                        .primary_media_source()
                        .and_then(|source| source.container.clone()),
                    video: item.video_stream().map(|video| {
                        (
                            video.codec.clone().unwrap_or_default(),
                            video.width.unwrap_or(0),
                            video.height.unwrap_or(0),
                            video.video_range.clone(),
                        )
                    }),
                    subtitles: item.subtitle_tracks(),
                    audio: item.audio_tracks(),
                    trickplay: item.trickplay.clone(),
                    chapters: item.chapters.clone(),
                })
            });
    }

    /// Open the online lookup inside the subtitle panel, searching once for the current
    /// language. Re-opening keeps the previous results so a mis-click does not refetch.
    pub fn open_in_app_subtitle_search(&mut self) {
        self.show_in_app_osd();
        self.in_app_subtitle_search.open = true;
        self.in_app_audio_open = false;
        // Start from the language in Settings; "Any" falls back to English.
        if !self.subtitle_language.is_empty() {
            self.in_app_subtitle_search.language = self.subtitle_language.clone();
        }
        if self.in_app_subtitle_search.results.is_empty()
            && self.in_app_subtitle_search.error.is_none()
        {
            let language = self.in_app_subtitle_search.language.clone();
            self.search_in_app_subtitles(language);
        }
    }

    pub fn close_in_app_subtitle_search(&mut self) {
        self.in_app_subtitle_search.open = false;
    }

    /// Ask every configured provider for subtitles in `language`. A search for another language
    /// replaces the list; the same language reuses it.
    pub fn search_in_app_subtitles(&mut self, language: String) {
        let Some(client) = self.client() else {
            self.in_app_subtitle_search.error = Some("Your session expired. Sign in again.".into());
            return;
        };
        let Some(item_id) = self.in_app.as_ref().map(|session| session.item_id.clone()) else {
            return;
        };
        if self.in_app_subtitle_search.language != language {
            self.in_app_subtitle_search.results.clear();
        }
        self.in_app_subtitle_search.language = language.clone();
        self.in_app_subtitle_search.loading = true;
        self.in_app_subtitle_search.error = None;
        self.in_app_subtitle_search_load
            .request(language.clone(), move |language| {
                client
                    .search_subtitles(&item_id, &language)
                    .map_err(|error| error.user_message())
            });
    }

    /// Download one provider result. On success the server has a new external track, so the
    /// stream list is re-read and the new track selected once it arrives.
    pub fn download_in_app_subtitle(&mut self, subtitle_id: String) {
        let Some(client) = self.client() else {
            self.in_app_subtitle_search.error = Some("Your session expired. Sign in again.".into());
            return;
        };
        let Some(item_id) = self.in_app.as_ref().map(|session| session.item_id.clone()) else {
            return;
        };
        self.in_app_subtitle_search.downloading = Some(subtitle_id.clone());
        self.in_app_subtitle_search.error = None;
        self.in_app_subtitle_download_load
            .request(subtitle_id.clone(), move |subtitle_id| {
                client
                    .download_subtitle(&item_id, &subtitle_id)
                    .map_err(|error| error.user_message())
            });
    }

    pub(super) fn apply_in_app_subtitle_search(
        &mut self,
        load: Result<Vec<jellyfin_api::RemoteSubtitle>, String>,
    ) {
        self.in_app_subtitle_search.loading = false;
        match load {
            Ok(results) => {
                self.in_app_subtitle_search.results = results;
                self.in_app_subtitle_search.error = None;
            }
            Err(message) => {
                self.in_app_subtitle_search.results.clear();
                self.in_app_subtitle_search.error = Some(message);
            }
        }
    }

    pub(super) fn apply_in_app_subtitle_download(&mut self, load: Result<(), String>) {
        self.in_app_subtitle_search.downloading = None;
        match load {
            Ok(()) => {
                self.in_app_subtitle_search.open = false;
                self.in_app_subtitle_search.results.clear();
                let Some(client) = self.client() else {
                    return;
                };
                let Some(item_id) = self.in_app.as_ref().map(|session| session.item_id.clone())
                else {
                    return;
                };
                self.pending_subtitle_before = Some(
                    self.in_app_subtitles
                        .tracks
                        .iter()
                        .map(|track| track.index)
                        .collect(),
                );
                self.request_in_app_streams(&client, &item_id);
            }
            Err(message) => self.in_app_subtitle_search.error = Some(message),
        }
    }

    pub(super) fn apply_in_app_playback_streams(&mut self, load: Result<PlaybackStreams, String>) {
        match load {
            Ok(load) => {
                // The list can change under a selection (an online download adds a track), so
                // remember the chosen server stream index, not its old position.
                let chosen = self
                    .in_app_subtitles
                    .selected
                    .and_then(|at| self.in_app_subtitles.tracks.get(at))
                    .map(|track| track.index);
                self.in_app_subtitles.source = load.source;
                self.in_app_container = load.container;
                self.in_app_video = load.video;
                self.in_app_subtitles.tracks = load.subtitles;
                self.in_app_audio_tracks = load.audio;
                self.in_app_subtitles.error = None;
                // A card-started film carries no timeline manifest or chapter list. Fill them
                // from the same request, before the reader can hover the bar.
                if let Some(pending) = &mut self.pending_in_app {
                    if pending.item.trickplay.is_none() {
                        pending.item.trickplay = load.trickplay.clone();
                    }
                    if pending.item.chapters.is_empty() {
                        pending.item.chapters = load.chapters.clone();
                    }
                }
                if let Some(session) = &mut self.in_app {
                    if session.item.trickplay.is_none() {
                        session.item.trickplay = load.trickplay;
                    }
                    if session.item.chapters.is_empty() {
                        session.item.chapters = load.chapters;
                    }
                }
                if let Some(before) = self.pending_subtitle_before.take() {
                    // Select the track that was not there a moment ago: the one just downloaded.
                    if let Some(at) = self
                        .in_app_subtitles
                        .tracks
                        .iter()
                        .position(|track| !before.contains(&track.index))
                    {
                        self.select_in_app_subtitle(Some(at));
                    }
                    return;
                }
                self.in_app_subtitles.selected = chosen.and_then(|index| {
                    self.in_app_subtitles
                        .tracks
                        .iter()
                        .position(|track| track.index == index)
                });
                if self.in_app_subtitles.selected.is_none()
                    && let Some(at) = self.preferred_subtitle_track()
                {
                    self.select_in_app_subtitle(Some(at));
                }
                self.apply_audio_preference();
            }
            Err(message) => self.in_app_subtitles.error = Some(message),
        }
    }

    pub(super) fn apply_in_app_subtitle_cues(&mut self, load: Result<SubtitleCues, String>) {
        self.in_app_subtitles.loading = false;
        match load {
            Ok(load) if self.in_app_subtitles.selected == Some(load.track) => {
                self.in_app_subtitles.cues = load.cues;
                self.in_app_subtitles.error = None;
            }
            Ok(_) => {}
            Err(message) => {
                self.in_app_subtitles.error = Some(message);
                self.in_app_subtitles.selected = None;
                self.in_app_subtitles.cues.clear();
            }
        }
    }

    /// Off → each track in turn → off again. The menu offers the same list directly.
    pub fn cycle_in_app_text(&mut self) {
        let count = self.in_app_subtitles.tracks.len();
        if count == 0 {
            return;
        }
        let next = match self.in_app_subtitles.selected {
            None => Some(0),
            Some(at) if at + 1 < count => Some(at + 1),
            Some(_) => None,
        };
        self.select_in_app_subtitle(next);
    }

    pub fn toggle_in_app_subtitles_menu(&mut self) {
        self.show_in_app_osd();
        let panel = if self.in_app_subtitles.open {
            PlayerPanel::None
        } else {
            PlayerPanel::Subtitles
        };
        self.open_player_panel(panel);
    }

    pub fn close_in_app_subtitles_menu(&mut self) {
        self.in_app_subtitles.open = false;
        self.cancel_subtitle_sync();
    }

    /// Align the selected track to the film's own audio: decode a few minutes of the file's
    /// sound, build the speech mask from both sides and set the delay to the shift that matches.
    ///
    /// The decode is the file's direct stream, not the playing pipeline: subtitle cues are in
    /// file time, and a server transcode would only add CPU on both ends. Live TV and bitmap
    /// tracks have no cues to move and are left alone.
    pub fn sync_in_app_subtitles(&mut self) {
        let Some(session) = &self.in_app else {
            return;
        };
        if session.live || self.in_app_subtitles.cues.is_empty() {
            return;
        }
        let (position, item_id) = (session.position_ms, session.item_id.clone());
        let Some(client) = self.client() else {
            self.in_app_subtitles.sync_note = Some("Sign in again to sync.".into());
            return;
        };
        let Some(url) = client.direct_stream_url(&item_id) else {
            self.in_app_subtitles.sync_note = Some("This stream has no file to listen to.".into());
            return;
        };
        self.show_in_app_osd();
        let start = (position - 5_000).max(0);
        let cues: Vec<(i64, i64)> = self
            .in_app_subtitles
            .cues
            .iter()
            .map(|cue| (cue.start_ms, cue.end_ms))
            .collect();
        self.in_app_subtitles.syncing = true;
        self.in_app_subtitles.sync_note = None;
        self.subtitle_sync_seen = 0;
        self.subtitle_sync_progress
            .store(0, std::sync::atomic::Ordering::Relaxed);
        self.subtitle_sync_cancel
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let progress = Arc::clone(&self.subtitle_sync_progress);
        let cancel = Arc::clone(&self.subtitle_sync_cancel);
        let duration = session_duration_ms(self);
        self.subtitle_sync.request((), move |()| {
            // Two windows: a scene of just music or a stretch of silence decides nothing, and the
            // next few minutes usually do. Two confident answers that disagree are not an answer.
            let mut found: Vec<crate::subtitle_sync::Alignment> = Vec::new();
            for extra in [0i64, 240_000] {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return Err("cancelled".into());
                }
                if duration > 0 && start + extra >= duration {
                    break;
                }
                let window =
                    crate::audio_probe::sample(&url, start + extra, 240, &progress, &cancel)?;
                if let Some(alignment) = crate::subtitle_sync::align(
                    &window.samples,
                    crate::audio_probe::RATE,
                    window.start_ms,
                    &cues,
                ) {
                    found.push(alignment);
                    // A strong answer needs no second opinion.
                    if alignment.confidence >= 0.35 {
                        break;
                    }
                }
            }
            Ok(crate::subtitle_sync::settle(found))
        });
    }

    /// Stop a running sync without waiting for it; its answer, if one still lands, is dropped
    /// with the cancelled request.
    fn cancel_subtitle_sync(&mut self) {
        self.subtitle_sync.cancel();
        self.subtitle_sync_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.in_app_subtitles.syncing = false;
    }

    /// The worker's verdict: set the delay, or say why there is nothing to set.
    pub fn apply_subtitle_sync(
        &mut self,
        result: Result<Option<crate::subtitle_sync::Alignment>, String>,
    ) {
        self.in_app_subtitles.syncing = false;
        match result {
            Ok(Some(found)) => {
                let applied = found.lag_ms.clamp(-30_000, 30_000);
                self.in_app_subtitles.delay_ms = applied;
                let label = format!("Synced {:+.1} s", applied as f32 / 1000.0);
                self.in_app_subtitles.sync_note = Some(label.clone());
                self.show_toast(format!("Subtitles {label}"));
            }
            Ok(None) => {
                self.in_app_subtitles.sync_note = Some("No clear match — adjust by hand.".into());
            }
            Err(error) if error == "cancelled" => {}
            Err(error) => {
                self.in_app_subtitles.sync_note = Some(format!("Could not sync: {error}"));
            }
        }
    }

    /// The chapter list. It shares the panel slot with the audio, subtitle and stats panels,
    /// so opening one closes the others.
    pub fn toggle_in_app_chapters(&mut self) {
        self.show_in_app_osd();
        let panel = if self.in_app_chapters_open {
            PlayerPanel::None
        } else {
            PlayerPanel::Chapters
        };
        self.open_player_panel(panel);
    }

    pub fn close_in_app_chapters(&mut self) {
        self.in_app_chapters_open = false;
    }

    /// Jump to a chapter start. The list stays open, so stepping through chapters is one click
    /// each and the current chapter highlight follows the playhead.
    pub fn seek_in_app_chapter(&mut self, at: usize) {
        self.show_in_app_osd();
        let Some(start) = self
            .in_app
            .as_ref()
            .and_then(|session| session.item.chapters.get(at))
            .map(|chapter| chapter.start_ms())
        else {
            return;
        };
        if let Some(session) = &mut self.in_app {
            let target = start.clamp(0, session.duration_ms.unwrap_or(start));
            session.seek_to(target);
            session.position_ms = target;
            session.seek_hold = Some((target, Instant::now()));
            session.scrub_ms = None;
        }
        self.send_in_app_progress(true);
        self.osd_hide_at = None;
    }

    /// Step back one episode without leaving the player. The neighbour list is loaded beside
    /// playback; the first episode of a season simply has nothing to offer.
    pub fn play_in_app_previous(&mut self) {
        if let Some(previous) = self.queue_step(false) {
            self.play_in_app_at(previous, None, false);
            return;
        }
        let Some(previous) = self.in_app_prev.clone() else {
            return;
        };
        self.play_in_app_at(previous, None, false);
    }

    /// Step forward now, rather than waiting for EOS. Uses the same neighbour the up-next strip
    /// offers, so the two can never disagree.
    pub fn play_in_app_next_episode(&mut self) {
        if let Some(next) = self.queue_step(true) {
            self.play_in_app_at(next, None, false);
            return;
        }
        self.play_in_app_next();
    }

    pub fn adjust_in_app_subtitle_delay(&mut self, delta_ms: i64) {
        self.show_in_app_osd();
        if self.in_app_subtitles.open {
            self.osd_hide_at = None;
        }
        self.in_app_subtitles.delay_ms =
            (self.in_app_subtitles.delay_ms + delta_ms).clamp(-30_000, 30_000);
    }

    pub fn adjust_in_app_subtitle_size(&mut self, delta: f32) {
        self.show_in_app_osd();
        if self.in_app_subtitles.open {
            self.osd_hide_at = None;
        }
        self.in_app_subtitles.size = (self.in_app_subtitles.size + delta).clamp(0.7, 1.8);
    }

    /// The cue under the (delayed) playhead. The OSD and the overlay both read this.
    #[cfg(test)]
    pub fn current_in_app_subtitle(&self) -> Option<&crate::subtitles::Cue> {
        self.current_in_app_cues().into_iter().next_back()
    }

    /// Every cue covering the (delayed) playhead, so a sign and a line can show together.
    pub fn current_in_app_cues(&self) -> Vec<&crate::subtitles::Cue> {
        if self.in_app_subtitles.selected.is_none() {
            return Vec::new();
        }
        let Some(session) = self.in_app.as_ref() else {
            return Vec::new();
        };
        let position =
            session.scrub_ms.unwrap_or(session.position_ms) - self.in_app_subtitles.delay_ms;
        crate::subtitles::cues_at(&self.in_app_subtitles.cues, position)
    }

    pub(super) fn apply_in_app_segments(&mut self, result: Result<Vec<MediaSegment>, String>) {
        // The segment request can beat GStreamer's first frame, so do not require an active
        // session here. Starting another item clears the list before its own request.
        self.in_app_segments = result.unwrap_or_default();
    }

    /// The segment the playhead is currently inside, if the server provided one. Used by the
    /// contextual skip action in the OSD.
    /// The segment starting within `lead_ms`, for the Ask mode's early button. Auto-skip
    /// still uses the strict active check: a lead is an affordance, not a reason to jump.
    pub fn upcoming_in_app_segment(&self, lead_ms: i64) -> Option<&MediaSegment> {
        let session = self.in_app.as_ref()?;
        let position = session.scrub_ms.unwrap_or(session.position_ms);
        self.in_app_segments.iter().find(|segment| {
            position >= segment.start_ms() - lead_ms
                && position < segment.end_ms()
                && matches!(
                    segment.kind.as_str(),
                    "Intro" | "Outro" | "Recap" | "Preview"
                )
        })
    }

    pub fn active_in_app_segment(&self) -> Option<&MediaSegment> {
        let session = self.in_app.as_ref()?;
        let position = session.scrub_ms.unwrap_or(session.position_ms);
        self.in_app_segments.iter().find(|segment| {
            position >= segment.start_ms()
                && position < segment.end_ms()
                && matches!(
                    segment.kind.as_str(),
                    "Intro" | "Outro" | "Recap" | "Preview"
                )
        })
    }

    /// The segment the skip action acts on, honouring the reader's mode. `Ask` offers it five
    /// seconds early, which is also when the pill appears; the key and the pill both read this,
    /// so pressing `S` while the button is visible can never do nothing.
    pub fn offered_in_app_segment(&self) -> Option<&MediaSegment> {
        match self.skip_mode {
            crate::config::SkipMode::Ask => self.upcoming_in_app_segment(5_000),
            crate::config::SkipMode::Always => self.active_in_app_segment(),
            crate::config::SkipMode::Never => None,
        }
    }

    pub(super) fn apply_in_app_start(&mut self, result: Result<InAppStart, Failure>) {
        self.launching = false;
        self.in_app_launching = false;
        match result {
            Ok(started) => {
                let method = started.method;
                let play_session_id = started.play_session_id.clone();
                let play_reason = started.notice.clone();
                if let Some(notice) = &started.notice {
                    self.show_toast(notice.clone());
                }
                let player = started.player;
                let Some(pending) = self.pending_in_app.take() else {
                    return;
                };
                if method == "Transcode" && item_is_dolby_vision(&pending.item) {
                    self.show_toast(
                        "Dolby Vision: the server is tone-mapping this to SDR.".to_string(),
                    );
                }
                // The worker applies and confirms the resume (unless the source already
                // begins there), so the session starts from where the picture really is.
                let start_ms = started.started_ms.unwrap_or(pending.start_ms);
                if pending.paused {
                    player.set_paused(true);
                }
                player.set_volume(self.in_app_volume);
                player.set_downmix(self.downmix_stereo);
                player.set_audio_delay(self.audio_delay_ms);
                player.set_audio_dynamics(match self.audio_mode {
                    crate::config::AudioMode::Off => "off",
                    crate::config::AudioMode::Dialogue => "dialogue",
                    crate::config::AudioMode::Night => "night",
                });
                self.idle.hold();
                self.osd_visible = true;
                // The remote has no pointer to find the bar: a fresh player starts with the
                // timeline focused, not with the page behind the picture.
                self.osd_focus_pending = true;
                self.osd_hide_at = Some(Instant::now() + Duration::from_secs(3));
                self.in_app_minimized = false;
                if pending.rewound > 0 {
                    self.show_toast(format!("Rewound {} s.", pending.rewound / 1_000));
                }
                self.resume_conflict = pending.conflict;
                self.in_app = Some(InAppSession {
                    player,
                    item: pending.item.clone(),
                    item_id: pending.item.id.clone(),
                    session_id: pending.session_id.clone(),
                    position_ms: start_ms,
                    duration_ms: pending.item.runtime_ms(),
                    paused: pending.paused,
                    scrub_ms: None,
                    seek_hold: None,
                    skipped_segments: std::collections::HashSet::new(),
                    last_report: Instant::now(),
                    live: pending.live,
                    live_stream_id: pending.live_stream_id.clone(),
                    buffering: None,
                    report: pending.report,
                    play_method: method,
                    play_reason,
                    play_session_id: play_session_id.clone(),
                    history_start_ms: start_ms,
                    position_base_ms: 0,
                    position_calibrated: method != "Transcode"
                        || pending.start_ms == 0
                        || started.started_ms == Some(0),
                });
                // The preference may have arrived before the player did; apply it now that
                // there is a pipeline to switch.
                self.apply_audio_preference();
                // The stream list can beat the first frame. A forced selection made then had no
                // session to download its cues against, so finish that request now.
                if self.in_app_subtitles.selected.is_some()
                    && self.in_app_subtitles.cues.is_empty()
                    && !self.in_app_subtitles.loading
                {
                    let at = self.in_app_subtitles.selected;
                    self.select_in_app_subtitle(at);
                }
                if pending.report
                    && let Some(client) = self.client()
                {
                    let mut report = PlaybackReport::direct_play(
                        self.in_app
                            .as_ref()
                            .map_or_else(String::new, |session| session.item_id.clone()),
                        self.in_app
                            .as_ref()
                            .map_or(0, |session| session.position_ms),
                        false,
                    );
                    report.play_method = method;
                    report.play_session_id = play_session_id.or(Some(pending.session_id.clone()));
                    self.worker.report_playing(client, report);
                }
            }
            Err(failure) => {
                if let Some(pending) = self.pending_in_app.take() {
                    if pending.local {
                        self.detail_notice = Some(format!("Trailer failed: {}", failure.message));
                    } else {
                        eprintln!("in-app: {}", failure.message);
                        self.last_in_app_error = Some(failure.message.clone());
                        self.detail_notice = Some(format!(
                            "In-app player failed: {}. Trying mpv.",
                            failure.message
                        ));
                        self.start_stream(pending.item);
                    }
                } else {
                    self.playback_error = Some(failure.message);
                }
            }
        }
    }

    /// Poll the GStreamer clock; returns true when the drawn state changed.
    pub(super) fn poll_in_app(&mut self) -> bool {
        let mut changed = false;
        // The up-next offer is useless if the chrome has faded by the time it is due, so it
        // wakes the OSD and holds it until the offer is taken, dismissed or gone.
        if !self.osd_visible && self.next_up_eligible() {
            self.osd_visible = true;
            self.osd_hide_at = None;
            changed = true;
        }
        if !self.osd_visible {
            // The precomputed preview is only meaningful with the chrome it belongs to.
            self.in_app_hover = None;
        }
        // The end-of-episode card replaces the transport. Focus its Play now once when it
        // appears, and let go when it is dismissed or a panel takes over.
        let end_card =
            self.next_up_eligible() && !self.player_panel_open() && !self.player_gear_visible();
        if end_card != self.in_app_next_offered {
            self.in_app_next_offered = end_card;
            self.in_app_next_focus = if end_card { 2 } else { 0 };
        }
        if !end_card {
            self.in_app_next_focus = 0;
        }
        if self.in_app.is_some()
            && self.osd_visible
            && !self.next_up_eligible()
            && !self
                .in_app
                .as_ref()
                .is_some_and(|session| session.paused || session.scrub_ms.is_some())
            && self
                .osd_hide_at
                .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.osd_visible = false;
            self.osd_hide_at = None;
            changed = true;
        }
        let Some(session) = self.in_app.as_ref() else {
            return changed;
        };
        // Ten clock queries a second, not sixty: the pipeline lock is held by the streaming
        // thread during a flush or a renegotiation, and a blocked query freezes the whole
        // window. A held seek target keeps winning until a fresh query settles it.
        let hold_expired = session
            .seek_hold
            .is_some_and(|(_, at)| at.elapsed() >= Duration::from_secs(4));
        let query_due =
            hold_expired || self.in_app_query_at.elapsed() >= Duration::from_millis(100);
        if query_due {
            self.in_app_query_at = Instant::now();
        }
        let raw = if query_due {
            session.player.position_ms()
        } else {
            0
        };
        let mut base = session.position_base_ms;
        let expected = session.history_start_ms;
        let calibrate = query_due
            && !session.position_calibrated
            && session.play_method == "Transcode"
            && raw > 0;
        let stored = session.position_ms;
        if calibrate {
            // A transcoded stream may carry its own timeline or start at zero. If the clock is
            // nowhere near the resume point the server was asked for, the stream is zero-based;
            // subtitle cues are always in file time, so the base puts the two back together.
            base = transcode_position_base(raw, expected);
            if let Some(session) = &mut self.in_app {
                session.position_base_ms = base;
                session.position_calibrated = true;
            }
        }
        let reported = if query_due { raw + base } else { stored };
        let Some(session) = self.in_app.as_ref() else {
            return changed;
        };
        let (position, seek_settled) = match session.seek_hold {
            Some((target, at)) => {
                let expired = at.elapsed() >= Duration::from_secs(4);
                if !expired && (!query_due || (reported - target).abs() > 1_000) {
                    (target, false)
                } else {
                    (reported, true)
                }
            }
            None => (reported, false),
        };
        let duration = if query_due {
            session.player.duration_ms()
        } else {
            session.duration_ms
        };
        let paused = session.player.is_paused();
        let paused_changed = paused != session.paused;
        let buffering = session.player.buffering();
        let buffering_changed = buffering != session.buffering;
        let mut changed = changed
            || buffering_changed
            || position != session.position_ms
            || duration != session.duration_ms
            || paused_changed;
        let error = session.player.error();
        let ended = session.player.has_ended();
        if let Some(session) = &mut self.in_app {
            if seek_settled {
                session.seek_hold = None;
            }
            session.position_ms = position;
            // A flushing seek can report a zero duration for an instant; keeping the last real
            // one stops the bar from jumping to the end and back during the flush.
            if let Some(duration) = duration.filter(|duration| *duration > 0) {
                session.duration_ms = Some(duration);
            }
            session.paused = paused;
            session.buffering = buffering;
        }
        if let Some(deadline) = self.sleep_deadline {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                self.sleep_deadline = None;
                self.sleep_fade_percent = 100;
                if let Some(session) = &mut self.in_app {
                    session.player.set_volume(self.in_app_volume);
                    session.player.set_paused(true);
                    session.paused = true;
                }
                self.show_toast("Sleep timer paused playback.");
                changed = true;
            } else if left <= Duration::from_secs(20) {
                // The last twenty seconds fade to silence instead of cutting the mix.
                let percent = ((left.as_millis() * 100) / 20_000).clamp(0, 100) as i64;
                if percent != self.sleep_fade_percent {
                    self.sleep_fade_percent = percent;
                    if let Some(session) = &self.in_app {
                        session
                            .player
                            .set_volume(self.in_app_volume * percent as f64 / 100.0);
                    }
                    changed = true;
                }
            }
        }
        if self.still_watching
            && !self.still_watching_open
            && !paused
            && self.in_app_play_since.elapsed()
                >= Duration::from_secs((self.still_watching_minutes.max(1) as u64) * 60)
        {
            if let Some(session) = &mut self.in_app {
                session.player.set_paused(true);
                session.paused = true;
            }
            self.still_watching_open = true;
            self.still_watching_since = Some(Instant::now());
            self.still_watching_countdown = 60;
            self.osd_visible = false;
            self.osd_hide_at = None;
            changed = true;
        }
        if self.still_watching_open
            && let Some(since) = self.still_watching_since
        {
            let elapsed = since.elapsed().as_secs() as i64;
            let left = 60 - elapsed;
            if left <= 0 {
                self.still_watching_stop();
                changed = true;
            } else if left != self.still_watching_countdown {
                self.still_watching_countdown = left;
                changed = true;
            }
        }
        if !paused
            && let (Some(a), Some(b)) = (self.loop_a, self.loop_b)
            && position >= b
        {
            if let Some(session) = &mut self.in_app {
                session.seek_to(a);
                session.position_ms = a;
                session.seek_hold = Some((a, Instant::now()));
                session.scrub_ms = None;
            }
            changed = true;
        }
        // Direct-play a marked intro/recap/credits when the reader chose Always. The segment id
        // is remembered so rewinding into it does not fight the reader a second time.
        if self.skip_mode == crate::config::SkipMode::Always
            && !paused
            && let Some(segment) = self.active_in_app_segment().cloned()
            && !self
                .in_app
                .as_ref()
                .is_some_and(|session| session.skipped_segments.contains(&segment.id))
        {
            if let Some(session) = &mut self.in_app {
                session.skipped_segments.insert(segment.id.clone());
            }
            self.skip_in_app_segment(segment.end_ms());
            changed = true;
        }
        if let Some(error) = error {
            self.playback_error = Some(error);
            self.finish_in_app();
            return true;
        }
        if ended {
            // The item is finished; a checkpoint from its middle must not resurrect later.
            if let Some(item_id) = self.in_app.as_ref().map(|session| session.item_id.clone()) {
                self.forget_checkpoint(&item_id);
            }
            if self.sleep_end_of_episode {
                self.sleep_end_of_episode = false;
                self.show_toast("Sleep timer: end of episode.");
                self.finish_in_app();
                return true;
            }
            if self.still_watching
                && self.in_app_run_episodes >= self.still_watching_episodes.max(1) as u32
            {
                let next = self.queue_step(true).or_else(|| self.in_app_next.clone());
                if next.is_some() {
                    self.still_watching_next = next;
                    self.still_watching_open = true;
                    self.still_watching_since = Some(Instant::now());
                    self.still_watching_countdown = 60;
                    self.osd_visible = false;
                    self.osd_hide_at = None;
                    return true;
                }
            }
            // A run continues into its next item first: "Play all" is a promise about order.
            // Then the next episode, which dismissing the card opts out of.
            if self.autoplay_next
                && let Some(next) = self.queue_step(true)
            {
                self.play_in_app_at(next, None, false);
                return true;
            }
            if self.autoplay_next
                && !self.in_app_next_dismissed
                && let Some(next) = self.in_app_next.take()
            {
                self.in_app_run_episodes = self.in_app_run_episodes.saturating_add(1);
                self.play_in_app_at(next, None, false);
                return true;
            }
            self.finish_in_app();
            return true;
        }
        if self.checkpoint_due()
            && let Some((id, position)) = self
                .in_app
                .as_ref()
                .filter(|session| session.position_ms > 0)
                .map(|session| (session.item_id.clone(), session.position_ms))
        {
            self.remember_checkpoint(&id, position);
        }
        self.send_in_app_progress(paused_changed);
        changed
    }

    /// Reveal the player controls and restart the idle timer.
    pub fn show_in_app_osd(&mut self) {
        if self.in_app.is_some() {
            if !self.osd_visible {
                // The remote has no pointer to put on the bar: opening the chrome puts focus
                // there, so Left/Right scrub at once and OK commits.
                self.osd_focus_pending = true;
            }
            self.in_app_last_input = Instant::now();
            self.osd_visible = true;
            self.osd_hide_at = Some(Instant::now() + Duration::from_secs(3));
        }
    }

    /// Step the rate up or down. The first press from 1× is 1.25× (or 0.75× going back).
    pub fn shift_in_app_speed(&mut self, forward: bool) {
        if self.in_app.as_ref().is_some_and(|session| session.live) {
            // A tuner stream is already at the live edge; a rate seek would leave it.
            return;
        }
        self.show_in_app_osd();
        let index = SPEEDS
            .iter()
            .position(|rate| (rate - self.in_app_speed).abs() < 0.001)
            .unwrap_or(2);
        let next = if forward {
            (index + 1).min(SPEEDS.len() - 1)
        } else {
            index.saturating_sub(1)
        };
        let rate = SPEEDS[next];
        if (rate - self.in_app_speed).abs() < 0.001 {
            return;
        }
        self.in_app_speed = rate;
        if let Some(session) = &self.in_app {
            session.player.set_rate(rate);
        }
    }

    /// Set an arbitrary rate, snapping to the nearest speed the player offers. MPRIS clients
    /// send a rate rather than stepping through the OSD's list.
    pub fn set_in_app_speed(&mut self, rate: f64) {
        if !rate.is_finite() || self.in_app.as_ref().is_some_and(|session| session.live) {
            return;
        }
        let Some(rate) = SPEEDS
            .iter()
            .copied()
            .min_by(|a, b| (*a - rate).abs().total_cmp(&(*b - rate).abs()))
        else {
            return;
        };
        if (rate - self.in_app_speed).abs() < 0.001 {
            return;
        }
        self.in_app_speed = rate;
        self.show_in_app_osd();
        if let Some(session) = &self.in_app {
            session.player.set_rate(rate);
        }
    }

    pub fn toggle_in_app_stats(&mut self) {
        self.show_in_app_osd();
        self.in_app_stats = !self.in_app_stats;
    }

    pub fn close_in_app_stats(&mut self) {
        self.in_app_stats = false;
    }

    pub fn toggle_in_app_fit(&mut self) {
        self.show_in_app_osd();
        self.in_app_fill = !self.in_app_fill;
    }

    /// Keep the session alive while the browser takes the screen. The surface is deliberately
    /// not rebuilt: the same player moves into the thumb-sized bar.
    pub fn minimize_in_app(&mut self) {
        if self.in_app.is_some() {
            self.in_app_minimized = true;
            self.osd_visible = false;
            self.osd_hide_at = None;
        }
    }

    pub fn restore_in_app(&mut self) {
        if self.in_app.is_some() {
            self.in_app_minimized = false;
            self.show_in_app_osd();
        }
    }

    pub fn toggle_in_app_mute(&mut self) {
        self.show_in_app_osd();
        if let Some(session) = &self.in_app {
            session.player.toggle_mute();
        }
    }

    pub fn adjust_in_app_volume(&mut self, delta: f64) {
        self.set_in_app_volume(self.in_app_volume + delta);
    }

    pub fn set_in_app_volume(&mut self, volume: f64) {
        self.show_in_app_osd();
        let volume = volume.clamp(0.0, 2.0);
        if let Some(session) = &self.in_app {
            session.player.set_volume(volume);
        }
        self.in_app_volume = volume;
    }

    pub fn toggle_in_app_audio_menu(&mut self) {
        self.show_in_app_osd();
        let panel = if self.in_app_audio_open {
            PlayerPanel::None
        } else {
            PlayerPanel::Audio
        };
        self.open_player_panel(panel);
    }

    pub fn close_in_app_audio_menu(&mut self) {
        self.in_app_audio_open = false;
        self.in_app_video = None;
        self.in_app_container = None;
        self.in_app_stats = false;
    }

    /// Playbin addresses audio tracks by their order in the file. The menu uses the same
    /// ordinal, so a language picked there is the one GStreamer switches to.
    pub fn select_in_app_audio(&mut self, ordinal: i32) {
        self.show_in_app_osd();
        self.in_app_audio_open = false;
        self.in_app_video = None;
        self.in_app_container = None;
        self.in_app_stats = false;
        self.in_app_audio_chosen = true;
        if let Some(session) = &self.in_app {
            session.player.select_audio(ordinal);
        }
    }

    fn checkpoint_due(&self) -> bool {
        self.checkpoint_at.elapsed() >= Duration::from_secs(5)
    }

    pub fn still_watching_continue(&mut self) {
        self.still_watching_open = false;
        self.still_watching_since = None;
        if let Some(next) = self.still_watching_next.take() {
            self.play_in_app_at(next, None, false);
        } else if let Some(session) = &mut self.in_app {
            session.player.set_paused(false);
            session.paused = false;
            self.in_app_play_since = Instant::now();
            self.osd_visible = true;
            self.osd_hide_at = Some(Instant::now() + Duration::from_secs(3));
        }
        self.in_app_run_episodes = 0;
    }

    pub fn still_watching_stop(&mut self) {
        self.still_watching_open = false;
        self.still_watching_since = None;
        self.still_watching_next = None;
        self.stop_in_app();
    }

    pub fn toggle_in_app_pause(&mut self) {
        self.show_in_app_osd();
        let changed = if let Some(session) = &mut self.in_app {
            let paused = !session.paused;
            session.player.set_paused(paused);
            session.paused = paused;
            true
        } else {
            false
        };
        if changed {
            self.send_in_app_progress(true);
        }
    }

    /// Jump to an absolute position. Used by a segment band, which names a start rather than
    /// a distance.
    pub fn seek_in_app_to(&mut self, position_ms: i64) {
        self.show_in_app_osd();
        if let Some(session) = &mut self.in_app {
            let target = position_ms.clamp(0, session.duration_ms.unwrap_or(position_ms));
            session.seek_to(target);
            session.position_ms = target;
            session.seek_hold = Some((target, Instant::now()));
            session.scrub_ms = None;
        }
        self.send_in_app_progress(true);
        self.osd_hide_at = None;
    }

    pub fn seek_in_app(&mut self, delta_ms: i64) {
        self.show_in_app_osd();
        if let Some(session) = &mut self.in_app {
            let target = (session.position_ms + delta_ms).max(0);
            session.seek_to(target);
            session.position_ms = target;
            session.seek_hold = Some((target, Instant::now()));
            session.scrub_ms = None;
        }
        self.send_in_app_progress(true);
    }

    /// Seek by the configured step, so the OSD buttons and the arrow keys always agree and a
    /// reader who wants 30-second jumps gets them on both.
    /// Pause and move one frame-ish step. 24 fps is the common case; an exact frame clock would
    /// need the source's frame rate, which is not worth a second seek for this control.
    pub fn step_in_app_frame(&mut self, forward: bool) {
        self.show_in_app_osd();
        if let Some(session) = &mut self.in_app {
            let target = (session.position_ms + if forward { 42 } else { -42 }).max(0);
            session.player.set_paused(true);
            session.seek_to(target);
            session.position_ms = target;
            session.paused = true;
            session.seek_hold = Some((target, Instant::now()));
            session.scrub_ms = None;
        }
        self.send_in_app_progress(true);
    }

    pub fn seek_in_app_step(&mut self, forward: bool) {
        let step = self.seek_step_ms;
        self.seek_in_app(if forward { step } else { -step });
        // A remote has no pointer to rest on the bar, so show the frame at the new position
        // while the chrome is up: the same trickplay preview a mouse drag gets.
        if let Some(session) = &self.in_app {
            let duration = session.duration_ms.unwrap_or(0).max(1) as f32;
            let x =
                (session.position_ms as f32 / duration).clamp(0.0, 1.0) * self.in_app_seek_width;
            self.hover_in_app_seek(x);
        }
    }

    pub fn skip_in_app_segment(&mut self, end_ms: i64) {
        self.show_in_app_osd();
        let before = self
            .in_app
            .as_ref()
            .map_or(0, |session| session.position_ms);
        if let Some(session) = &mut self.in_app {
            session.seek_to(end_ms);
            session.position_ms = end_ms;
            session.seek_hold = Some((end_ms, Instant::now()));
            session.scrub_ms = None;
        }
        self.send_in_app_progress(true);
        self.show_undo_toast(
            "Skipped a segment.",
            crate::state::UndoAction::Seek {
                position_ms: before,
            },
        );
    }

    /// Track the pointer over the timeline and ask the image worker for the tile that covers it.
    pub fn hover_in_app_seek(&mut self, x: f32) {
        let width = self.in_app_seek_width.max(1.0);
        let Some(session) = &self.in_app else {
            return;
        };
        let duration = session.duration_ms.unwrap_or(0).max(1) as f32;
        let ms = (duration * (x / width).clamp(0.0, 1.0)) as i64;
        let item = session.item.clone();
        self.in_app_hover = Some((x, ms));
        self.in_app_hover_armed = true;
        // A reader resting on the bar must not lose the preview to the idle timer.
        self.osd_visible = true;
        self.osd_hide_at = None;
        self.ensure_trickplay_tile(&item, ms);
    }

    pub fn clear_in_app_hover(&mut self) {
        self.in_app_hover = None;
        self.in_app_hover_armed = false;
    }

    /// A leave event only clears a preview that a real pointer move put there.
    pub fn leave_in_app_hover(&mut self) {
        if self.in_app_hover_armed {
            self.clear_in_app_hover();
            if self.in_app.is_some() {
                self.osd_hide_at = Some(Instant::now() + Duration::from_secs(3));
            }
        }
    }

    /// The tile key for the thumbnail at `ms`, if the item has a trickplay manifest.
    pub fn trickplay_key(&self, item: &Item, ms: i64) -> Option<String> {
        let (width, info) = item.trickplay_manifest()?;
        let tile = info.tile_of(info.thumbnail_at(ms));
        Some(crate::image::key(
            &self.config.server_url,
            &item.id,
            "Trickplay",
            Some(&format!("{width}-{tile}")),
            width,
        ))
    }

    /// Show the position under the pointer while the timeline is being dragged. This changes
    /// only the OSD; the player keeps its own clock until the gesture ends.
    pub fn preview_in_app_scrub(&mut self, fraction: f32) {
        let Some(session) = &self.in_app else {
            return;
        };
        let duration = session.duration_ms.unwrap_or(0).max(1) as f32;
        self.preview_in_app_at((duration * fraction.clamp(0.0, 1.0)) as i64);
    }

    /// Show `position_ms` on the timeline with its trickplay frame, without moving the player.
    /// A remote's scrub and a pointer drag both land here; OK or releasing the pointer commits.
    pub fn preview_in_app_at(&mut self, position_ms: i64) {
        let Some(session) = &self.in_app else {
            return;
        };
        let duration = session.duration_ms.unwrap_or(0).max(1);
        let target = position_ms.clamp(0, duration);
        let width = self.in_app_seek_width.max(1.0);
        let x = (target as f32 / duration as f32).clamp(0.0, 1.0) * width;
        let item = session.item.clone();
        if let Some(session) = &mut self.in_app {
            session.scrub_ms = Some(target);
        }
        self.in_app_hover = Some((x, target));
        self.in_app_hover_armed = true;
        // A reader resting on the bar must not lose the preview to the idle timer.
        self.osd_visible = true;
        self.osd_hide_at = None;
        self.ensure_trickplay_tile(&item, target);
    }

    /// A TV remote's timeline scrub: move the preview one step without seeking. OK commits it,
    /// so the picture jumps once instead of once per press.
    pub fn scrub_in_app_step(&mut self, forward: bool) {
        let step = self.seek_step_ms;
        let Some(session) = &self.in_app else {
            return;
        };
        let duration = session.duration_ms.unwrap_or(0).max(1);
        let from = session.scrub_ms.unwrap_or(session.position_ms);
        let target = (from + if forward { step } else { -step }).clamp(0, duration);
        self.preview_in_app_at(target);
    }

    /// Commit the timeline's preview; with no preview up, OK is play/pause.
    pub fn commit_in_app_scrub(&mut self) {
        let Some(target) = self.in_app.as_ref().and_then(|session| session.scrub_ms) else {
            self.toggle_in_app_pause();
            return;
        };
        self.clear_in_app_hover();
        self.seek_in_app_to(target);
        self.osd_hide_at = Some(Instant::now() + Duration::from_secs(3));
    }

    /// Commit a timeline drag. A cancelled drag only clears the preview and leaves the player
    /// exactly where it was.
    pub fn finish_in_app_scrub(&mut self, fraction: f32, cancelled: bool) {
        if let Some(session) = &mut self.in_app {
            session.scrub_ms = None;
            if !cancelled {
                let duration = session.duration_ms.unwrap_or(0).max(1) as f32;
                let target = (duration * fraction.clamp(0.0, 1.0)) as i64;
                session.seek_to(target);
                session.position_ms = target;
                session.seek_hold = Some((target, Instant::now()));
            }
        }
        self.osd_hide_at = Some(Instant::now() + Duration::from_secs(3));
        self.send_in_app_progress(true);
    }

    pub fn stop_in_app(&mut self) {
        self.cancel_subtitle_sync();
        if let Some(session) = &self.in_app
            && session.position_ms > 0
        {
            let (id, position) = (session.item_id.clone(), session.position_ms);
            self.remember_checkpoint(&id, position);
        }
        // A channel may have been opened while the player was still starting; that tuner has
        // to be released even though there is no session to stop yet.
        self.live_open.cancel();
        if self.in_app.is_none()
            && let Some(id) = self
                .pending_in_app
                .as_ref()
                .and_then(|pending| pending.live_stream_id.clone())
        {
            self.close_live_id(Some(id));
        }
        self.pending_live = None;
        // Stopping is a decision to leave the run, unlike an item-to-item hand-off. It also
        // cancels a channel zap that has not opened yet.
        self.queue.clear();
        self.queue_at = 0;
        self.pending_zap = None;
        self.in_app_start.cancel();
        self.in_app_segments_load.cancel();
        self.in_app_segments.clear();
        self.in_app_subtitle_search_load.cancel();
        self.in_app_subtitle_download_load.cancel();
        self.in_app_subtitle_search = SubtitleSearch::default();
        self.pending_subtitle_before = None;
        self.in_app_hover = None;
        self.in_app_hover_armed = false;
        self.in_app_trickplay_key = None;
        self.in_app_subtitle_load.cancel();
        self.in_app_streams_load.cancel();
        self.in_app_neighbours_load.cancel();
        self.in_app_next = None;
        self.in_app_prev = None;
        self.in_app_chapters_open = false;
        self.in_app_queue_open = false;
        self.in_app_gear_open = false;
        self.in_app_gear_returns = false;
        self.in_app_next_dismissed = false;
        self.in_app_audio_tracks.clear();
        self.in_app_audio_chosen = false;
        self.in_app_audio_open = false;
        self.in_app_video = None;
        self.in_app_container = None;
        self.in_app_stats = false;
        let size = self.in_app_subtitles.size;
        let delay_ms = self.in_app_subtitles.delay_ms;
        self.in_app_subtitles = Subtitles {
            size,
            delay_ms,
            ..Subtitles::default()
        };
        self.pending_in_app = None;
        self.launching = false;
        self.in_app_launching = false;
        self.in_app_minimized = false;
        self.osd_visible = true;
        self.osd_hide_at = None;
        self.finish_in_app();
    }

    pub(super) fn finish_in_app(&mut self) {
        self.idle.release();
        // Release the tuner before the session disappears, on every path: stop, error, EOS.
        if let Some(id) = self
            .in_app
            .as_ref()
            .and_then(|session| session.live_stream_id.clone())
        {
            self.close_live_id(Some(id));
        }
        self.in_app_minimized = false;
        self.in_app_segments_load.cancel();
        self.in_app_segments.clear();
        self.in_app_subtitle_search_load.cancel();
        self.in_app_subtitle_download_load.cancel();
        self.in_app_subtitle_search = SubtitleSearch::default();
        self.pending_subtitle_before = None;
        self.in_app_subtitle_load.cancel();
        self.in_app_streams_load.cancel();
        self.in_app_neighbours_load.cancel();
        self.in_app_next = None;
        self.in_app_prev = None;
        self.in_app_chapters_open = false;
        self.in_app_queue_open = false;
        self.in_app_gear_open = false;
        self.in_app_gear_returns = false;
        self.in_app_next_dismissed = false;
        self.in_app_audio_tracks.clear();
        self.in_app_audio_chosen = false;
        self.in_app_audio_open = false;
        self.in_app_video = None;
        self.in_app_container = None;
        self.in_app_stats = false;
        let size = self.in_app_subtitles.size;
        let delay_ms = self.in_app_subtitles.delay_ms;
        self.in_app_subtitles = Subtitles {
            size,
            delay_ms,
            ..Subtitles::default()
        };
        if let Some(entry) = self.in_app.as_ref().and_then(history_entry) {
            self.history.push(entry.clone());
            if self.history.len() > 100 {
                self.history.drain(..self.history.len() - 100);
            }
            self.history_ms += entry.played_ms;
            crate::history::append(entry);
        }
        let Some(session) = self.in_app.take() else {
            return;
        };
        session.player.stop();
        if session.report
            && let Some(client) = self.client()
        {
            let mut stop =
                jellyfin_api::PlaybackStop::new(session.item_id.clone(), session.position_ms);
            stop.play_session_id = session
                .play_session_id
                .clone()
                .or_else(|| Some(session.session_id.clone()));
            self.worker.report_stopped(client, stop);
        }
    }

    fn send_in_app_progress(&mut self, force: bool) {
        let Some(client) = self.client() else {
            return;
        };
        let Some(session) = &mut self.in_app else {
            return;
        };
        if !session.report {
            return;
        }
        let now = Instant::now();
        if !force && now.duration_since(session.last_report) < Duration::from_secs(10) {
            return;
        }
        session.last_report = now;
        let report = playback_report(session);
        self.worker.report_progress(client, report);
    }
}

/// One line per completed play. Short trailers and previews are not history.
fn history_entry(session: &InAppSession) -> Option<crate::history::HistoryEntry> {
    if !session.report {
        return None;
    }
    let played = (session.position_ms - session.history_start_ms).max(0);
    if played < 30_000 {
        return None;
    }
    Some(crate::history::HistoryEntry {
        item_id: session.item_id.clone(),
        name: session.item.name.clone(),
        kind: session.item.kind.clone(),
        series_name: session.item.series_name.clone(),
        year: session.item.production_year,
        played_ms: played,
        at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs() as i64),
    })
}

/// The progress/start payload for one session. Kept pure so the method and session id are
/// testable without a server.
fn playback_report(session: &InAppSession) -> PlaybackReport {
    let mut report =
        PlaybackReport::direct_play(session.item_id.clone(), session.position_ms, session.paused);
    report.play_method = session.play_method;
    report.play_session_id = session
        .play_session_id
        .clone()
        .or_else(|| Some(session.session_id.clone()));
    report
}

/// The file format to ask the server for. ASS keeps its own layout; everything else is
/// converted to SRT, which is the one text format the overlay has parsed since the start.
pub fn subtitle_format(track: &jellyfin_api::SubtitleTrack) -> &'static str {
    if track
        .codec
        .as_deref()
        .is_some_and(|codec| codec.eq_ignore_ascii_case("ass") || codec.eq_ignore_ascii_case("ssa"))
    {
        "ass"
    } else {
        "srt"
    }
}

/// The episodes on either side of `current_id` in broadcast order. Kept pure so the first and
/// last episodes — and a season boundary — are tested without a server.
pub fn episode_neighbours(episodes: &[Item], current_id: &str) -> EpisodeNeighbours {
    let Some(at) = episodes.iter().position(|episode| episode.id == current_id) else {
        return (None, None);
    };
    (
        at.checked_sub(1)
            .and_then(|before| episodes.get(before).cloned()),
        episodes.get(at + 1).cloned(),
    )
}

/// Where an ordered run should start: the first thing not finished, else the top.
pub fn first_unplayed(items: &[Item]) -> usize {
    items
        .iter()
        .position(|item| item.user_data.as_ref().is_none_or(|user| !user.played))
        .unwrap_or(0)
}

/// Kinds the player can open directly. A Series or a Folder is a container, not a stream.
pub fn is_playable(item: &Item) -> bool {
    matches!(
        item.kind.as_str(),
        "Movie" | "Episode" | "Video" | "MusicVideo"
    )
}

/// Shuffle without a dependency: a xorshift seeded from the clock. The order only has to be
/// unpredictable, not cryptographically random, and a repeat run should differ.
fn shuffle_items(items: &mut [Item]) {
    let mut seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0x9e37_79b9_7f4a_7c15)
        | 1;
    for index in (1..items.len()).rev() {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let swap = (seed % (index as u64 + 1)) as usize;
        items.swap(index, swap);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn source() -> jellyfin_api::PlaybackMediaSource {
        jellyfin_api::PlaybackMediaSource {
            id: "4k".into(),
            container: Some("mkv".into()),
            bitrate: Some(31_000_000),
            media_streams: vec![jellyfin_api::MediaStream {
                index: 0,
                kind: "Video".into(),
                codec: Some("hevc".into()),
                ..jellyfin_api::MediaStream::default()
            }],
            supports_direct_play: true,
            supports_direct_stream: true,
            supports_transcoding: true,
            direct_stream_url: Some("/Videos/item/stream?static=false".into()),
            transcoding_url: Some("/Videos/item/master.m3u8".into()),
            ..jellyfin_api::PlaybackMediaSource::default()
        }
    }

    fn client() -> Client {
        Client::new("https://media.example", "device").with_session("token", "user")
    }

    #[test]
    fn history_records_only_plays_long_enough_to_matter() {
        let mut session = crate::demo::in_app();
        session.position_ms = 20 * 60_000;
        session.history_start_ms = 12 * 60_000;
        let entry = history_entry(&session).expect("a history entry");
        assert_eq!(entry.played_ms, 8 * 60_000);
        session.position_ms = 12 * 60_000 + 10_000;
        assert!(history_entry(&session).is_none());
    }

    #[test]
    fn the_progress_report_carries_the_negotiated_method_and_id() {
        let mut session = crate::demo::in_app();
        session.play_method = "Transcode";
        session.play_session_id = Some("server-session".into());
        let report = playback_report(&session);
        assert_eq!(report.play_method, "Transcode");
        assert_eq!(report.play_session_id.as_deref(), Some("server-session"));
        assert_eq!(
            report.position_ticks,
            session.position_ms * jellyfin_api::TICKS_PER_MS
        );
    }

    #[test]
    fn a_transcode_url_only_starts_at_the_resume_when_it_says_so() {
        let start = 3_790_000;
        let ticks = start * jellyfin_api::TICKS_PER_MS;
        assert!(url_starts_at(
            &format!("/Videos/x/master.m3u8?StartTimeTicks={ticks}&api_key=t"),
            start
        ));
        assert!(url_starts_at(
            &format!("/Videos/x/master.m3u8?startticks={}", ticks - 5_000_000),
            start
        ));
        assert!(!url_starts_at("/Videos/x/master.m3u8", start));
        assert!(!url_starts_at(
            "/Videos/x/master.m3u8?StartTimeTicks=0",
            start
        ));
        assert!(url_starts_at("/Videos/x/master.m3u8", 0));
    }

    #[test]
    fn a_negotiated_method_and_session_land_on_the_session() {
        let mut state = State::from_config(Config::fresh());
        state.pending_in_app = Some(PendingInApp {
            item: Item {
                id: "item".into(),
                name: "Item".into(),
                kind: "Movie".into(),
                ..Item::default()
            },
            session_id: "local".into(),
            start_ms: 0,
            rewound: 0,
            conflict: None,
            paused: false,
            live: false,
            live_stream_id: None,
            report: false,
            local: false,
        });
        state.apply_in_app_start(Ok(InAppStart {
            player: Player::preview(),
            method: "Transcode",
            play_session_id: Some("server-session".into()),
            notice: Some("The 2 Mbps cap is below this file's 31 Mbps.".into()),
            started_ms: None,
        }));
        let session = state.in_app.as_ref().expect("a session");
        assert_eq!(session.play_method, "Transcode");
        assert_eq!(session.play_session_id.as_deref(), Some("server-session"));
        assert_eq!(
            session.play_reason.as_deref(),
            Some("The 2 Mbps cap is below this file's 31 Mbps.")
        );
    }

    #[test]
    fn direct_play_wins_when_the_file_is_supported() {
        let response = PlaybackInfoResponse {
            media_sources: vec![source()],
            play_session_id: Some("session".into()),
        };
        let (url, method, session, notice) =
            choose_playback(&client(), Some(&response), Some("4k"), None, "fallback");
        assert_eq!(url, "fallback");
        assert_eq!(method, "DirectPlay");
        assert_eq!(session.as_deref(), Some("session"));
        assert!(notice.is_none());
    }

    #[test]
    fn a_bitrate_cap_prefers_the_transcoding_url() {
        let response = PlaybackInfoResponse {
            media_sources: vec![source()],
            play_session_id: Some("session".into()),
        };
        let (url, method, _, notice) = choose_playback(
            &client(),
            Some(&response),
            Some("4k"),
            Some(2_000_000),
            "fallback",
        );
        assert_eq!(method, "Transcode");
        assert!(url.contains("master.m3u8"), "{url}");
        assert!(url.contains("api_key=token"), "{url}");
        let notice = notice.expect("the cap is explained");
        assert!(notice.contains("2 Mbps cap"), "{notice}");
        assert!(notice.contains("31 Mbps"), "{notice}");
    }

    #[test]
    fn a_profile_transcode_names_the_codec_and_container() {
        let mut source = source();
        source.supports_direct_play = false;
        source.supports_direct_stream = false;
        let response = PlaybackInfoResponse {
            media_sources: vec![source],
            play_session_id: None,
        };
        let (_, method, _, notice) =
            choose_playback(&client(), Some(&response), None, None, "fallback");
        assert_eq!(method, "Transcode");
        let notice = notice.expect("the profile is explained");
        assert!(notice.contains("hevc in mkv"), "{notice}");
    }

    #[test]
    fn direct_stream_is_used_when_direct_play_is_unsupported() {
        let mut source = source();
        source.supports_direct_play = false;
        let response = PlaybackInfoResponse {
            media_sources: vec![source],
            play_session_id: None,
        };
        let (url, method, _, notice) =
            choose_playback(&client(), Some(&response), None, None, "fallback");
        assert_eq!(method, "DirectStream");
        assert!(url.contains("static=false"), "{url}");
        let notice = notice.expect("the copy is explained");
        assert!(notice.contains("audio or subtitles"), "{notice}");
    }

    #[test]
    fn a_failed_negotiation_falls_back_to_the_static_stream() {
        let (url, method, session, notice) =
            choose_playback(&client(), None, None, None, "fallback");
        assert_eq!(url, "fallback");
        assert_eq!(method, "DirectPlay");
        assert!(session.is_none());
        assert!(notice.is_some());
    }

    #[test]
    fn finishing_an_online_subtitle_download_selects_the_new_track() {
        let mut state = State::from_config(Config::fresh());
        state.in_app = Some(crate::demo::in_app());
        state.in_app_subtitles.tracks = crate::demo::subtitle_tracks();
        state.in_app_subtitles.selected = Some(0);
        state.in_app_subtitle_search.open = true;
        state.in_app_subtitle_search.downloading = Some("demo-sub-1".into());
        state.pending_subtitle_before = Some(vec![0, 3]);
        state.apply_in_app_subtitle_download(Ok(()));
        assert!(!state.in_app_subtitle_search.open, "the lookup closes");
        // The refreshed stream list keeps the old tracks and appends the downloaded one.
        let mut tracks = crate::demo::subtitle_tracks();
        tracks.push(jellyfin_api::SubtitleTrack {
            index: 7,
            language: Some("eng".into()),
            display_title: "English - SUBRIP - External".into(),
            codec: Some("subrip".into()),
            is_text: true,
            is_external: true,
            is_forced: false,
        });
        state.apply_in_app_playback_streams(Ok(PlaybackStreams {
            source: Some("source-1".into()),
            container: None,
            video: None,
            subtitles: tracks,
            audio: Vec::new(),
            trickplay: None,
            chapters: Vec::new(),
        }));
        assert_eq!(
            state.in_app_subtitles.selected,
            Some(2),
            "the newly downloaded track becomes the active one"
        );
    }

    #[test]
    fn the_subtitle_preference_picks_the_language_then_falls_back() {
        let mut state = State::from_config(Config::fresh());
        state.in_app = Some(crate::demo::in_app());
        state.in_app_subtitles.source = Some("source-1".into());
        state.subtitle_mode = SubtitleMode::Always;
        state.subtitle_language = "jpn".into();
        let mut tracks = crate::demo::subtitle_tracks();
        tracks.push(jellyfin_api::SubtitleTrack {
            index: 5,
            language: Some("jpn".into()),
            display_title: "Japanese".into(),
            codec: Some("subrip".into()),
            is_text: true,
            is_external: true,
            is_forced: false,
        });
        state.apply_in_app_playback_streams(Ok(PlaybackStreams {
            source: Some("source-1".into()),
            container: None,
            video: None,
            subtitles: tracks,
            audio: Vec::new(),
            trickplay: None,
            chapters: Vec::new(),
        }));
        assert_eq!(
            state.in_app_subtitles.selected,
            Some(2),
            "the preferred language wins"
        );

        // No Japanese track: "Always" still shows something rather than nothing.
        let mut state = State::from_config(Config::fresh());
        state.in_app = Some(crate::demo::in_app());
        state.in_app_subtitles.source = Some("source-1".into());
        state.subtitle_mode = SubtitleMode::Always;
        state.subtitle_language = "jpn".into();
        state.apply_in_app_playback_streams(Ok(PlaybackStreams {
            source: Some("source-1".into()),
            container: None,
            video: None,
            subtitles: crate::demo::subtitle_tracks(),
            audio: Vec::new(),
            trickplay: None,
            chapters: Vec::new(),
        }));
        assert_eq!(state.in_app_subtitles.selected, Some(0));
    }

    #[test]
    fn subtitle_mode_off_leaves_the_track_alone() {
        let mut state = State::from_config(Config::fresh());
        state.in_app = Some(crate::demo::in_app());
        state.in_app_subtitles.source = Some("source-1".into());
        state.subtitle_mode = SubtitleMode::Off;
        let mut tracks = crate::demo::subtitle_tracks();
        tracks[0].is_forced = true;
        state.apply_in_app_playback_streams(Ok(PlaybackStreams {
            source: Some("source-1".into()),
            container: None,
            video: None,
            subtitles: tracks,
            audio: Vec::new(),
            trickplay: None,
            chapters: Vec::new(),
        }));
        assert_eq!(state.in_app_subtitles.selected, None);
    }

    #[test]
    fn the_seek_step_is_the_configured_distance() {
        let mut state = State::from_config(Config::fresh());
        let mut session = crate::demo::in_app();
        session.position_ms = 0;
        state.in_app = Some(session);
        state.seek_step_ms = 30_000;
        state.seek_in_app_step(true);
        assert_eq!(state.in_app.as_ref().expect("playing").position_ms, 30_000);
        state.seek_in_app_step(false);
        assert_eq!(state.in_app.as_ref().expect("playing").position_ms, 0);
    }

    #[test]
    fn a_timeline_scrub_previews_without_moving_the_player() {
        let mut state = State::from_config(Config::fresh());
        state.in_app = Some(crate::demo::in_app());
        state.in_app_seek_width = 1232.0;
        state.seek_step_ms = 10_000;
        let start = state.in_app.as_ref().expect("playing").position_ms;
        state.scrub_in_app_step(true);
        state.scrub_in_app_step(true);
        let session = state.in_app.as_ref().expect("playing");
        assert_eq!(session.scrub_ms, Some(start + 20_000));
        assert_eq!(session.position_ms, start, "the scrub has not sought yet");
        assert_eq!(
            state.in_app_hover.map(|(_, ms)| ms),
            Some(start + 20_000),
            "the trickplay preview follows the scrub"
        );

        state.commit_in_app_scrub();
        let session = state.in_app.as_ref().expect("playing");
        assert_eq!(session.position_ms, start + 20_000);
        assert_eq!(session.scrub_ms, None);
        assert!(state.in_app_hover.is_none());

        // With nothing previewed, OK is the play/pause key.
        assert!(!session.paused);
        state.commit_in_app_scrub();
        assert!(state.in_app.as_ref().expect("playing").paused);
    }

    #[test]
    fn playback_streams_fill_a_card_started_timeline_manifest() {
        let mut state = State::from_config(Config::fresh());
        let mut session = crate::demo::in_app();
        // A library card carries no Trickplay or Chapters field; the playback item request is
        // where the player learns them.
        session.item.trickplay = None;
        session.item.chapters.clear();
        state.in_app = Some(session);
        state.apply_in_app_playback_streams(Ok(PlaybackStreams {
            source: Some("source-1".into()),
            container: None,
            video: None,
            subtitles: Vec::new(),
            audio: Vec::new(),
            trickplay: Some(crate::demo::trickplay_manifest()),
            chapters: vec![jellyfin_api::ChapterInfo {
                start_position_ticks: 0,
                name: "Cold open".into(),
                image_tag: None,
            }],
        }));
        let session = state.in_app.as_ref().expect("playing");
        assert!(
            session.item.trickplay.is_some(),
            "the preview manifest arrived"
        );
        assert_eq!(session.item.chapters.len(), 1, "the chapter rail arrived");
    }

    #[test]
    fn a_seek_holds_its_target_until_the_player_clock_arrives() {
        let mut state = State::from_config(Config::fresh());
        state.seek_step_ms = 10_000;
        let mut session = crate::demo::in_app();
        // The demo player's clock sits at 42:00 and does not move with a seek, exactly like a
        // flushing GStreamer seek that reports the old second for a frame.
        session.position_ms = 42 * 60_000;
        state.in_app = Some(session);
        let target = 42 * 60_000 + 10_000;
        state.seek_in_app_step(true);
        state.poll_in_app();
        assert_eq!(
            state.in_app.as_ref().expect("playing").position_ms,
            target,
            "the stale clock does not pull the bar back"
        );
        // The hold expires rather than lying forever: a player that never reaches the target
        // hands the bar back to the clock it actually has.
        if let Some(session) = &mut state.in_app {
            session.seek_hold = Some((target, Instant::now() - Duration::from_secs(5)));
        }
        state.poll_in_app();
        assert_eq!(
            state.in_app.as_ref().expect("playing").position_ms,
            42 * 60_000,
            "after the hold the reported clock wins"
        );
    }

    #[test]
    fn a_queue_steps_forward_and_back_and_stops_at_its_ends() {
        let mut state = State::from_config(Config::fresh());
        let items: Vec<Item> = (0..3)
            .map(|index| Item {
                id: format!("film-{index}"),
                name: format!("Film {index}"),
                kind: "Movie".into(),
                ..Item::default()
            })
            .collect();
        state.play_queue(items, 0, false);
        assert_eq!(state.queue.len(), 3);
        assert_eq!(state.queue_at, 0);
        assert!(state.queue_next_available());
        assert!(!state.queue_previous_available());

        state.play_in_app_next_episode();
        assert_eq!(state.queue_at, 1);
        state.play_in_app_next_episode();
        assert_eq!(state.queue_at, 2);
        assert!(!state.queue_next_available(), "the last item has no next");
        assert!(state.queue_previous_available());

        state.play_in_app_previous();
        assert_eq!(state.queue_at, 1);

        // A single item clicked from a card is not a run.
        state.play_in_app(Item {
            id: "single".into(),
            name: "Single".into(),
            kind: "Movie".into(),
            ..Item::default()
        });
        assert!(state.queue.is_empty());
    }

    #[test]
    fn episode_neighbours_cross_a_season_boundary_and_stop_at_the_ends() {
        let episodes: Vec<Item> = [(1, 1), (1, 2), (2, 1), (2, 2)]
            .into_iter()
            .map(|(season, index)| Item {
                id: format!("s{season}e{index}"),
                name: format!("S{season}E{index}"),
                kind: "Episode".into(),
                parent_index_number: Some(season),
                index_number: Some(index),
                ..Item::default()
            })
            .collect();
        let (previous, next) = episode_neighbours(&episodes, "s1e2");
        assert_eq!(previous.map(|item| item.id), Some("s1e1".into()));
        assert_eq!(
            next.map(|item| item.id),
            Some("s2e1".into()),
            "the season finale's next is the next season's opener"
        );
        let (previous, next) = episode_neighbours(&episodes, "s2e1");
        assert_eq!(
            previous.map(|item| item.id),
            Some("s1e2".into()),
            "and stepping back crosses it too"
        );
        assert_eq!(next.map(|item| item.id), Some("s2e2".into()));
        let (previous, _) = episode_neighbours(&episodes, "s1e1");
        assert!(previous.is_none(), "the first episode has no previous");
        let (_, next) = episode_neighbours(&episodes, "s2e2");
        assert!(next.is_none(), "the last episode has no next");
        let (previous, next) = episode_neighbours(&episodes, "missing");
        assert!(previous.is_none() && next.is_none());
    }

    #[test]
    fn ass_tracks_are_requested_as_ass_and_others_as_srt() {
        let track = |codec: &str| jellyfin_api::SubtitleTrack {
            codec: Some(codec.into()),
            display_title: codec.into(),
            ..jellyfin_api::SubtitleTrack::default()
        };
        assert_eq!(subtitle_format(&track("ass")), "ass");
        assert_eq!(subtitle_format(&track("SSA")), "ass");
        assert_eq!(subtitle_format(&track("subrip")), "srt");
        assert_eq!(subtitle_format(&track("")), "srt");
    }

    #[test]
    fn the_online_lookup_starts_in_the_preferred_language() {
        let mut state = State::from_config(Config::fresh());
        state.in_app = Some(crate::demo::in_app());
        state.subtitle_language = "spa".into();
        state.open_in_app_subtitle_search();
        assert_eq!(state.in_app_subtitle_search.language, "spa");
        assert!(state.in_app_subtitle_search.open);
    }

    #[test]
    fn a_run_starts_at_the_first_unfinished_item() {
        let items: Vec<Item> = (0..4)
            .map(|index| Item {
                id: format!("film-{index}"),
                kind: "Movie".into(),
                user_data: Some(jellyfin_api::UserData {
                    played: index < 2,
                    ..jellyfin_api::UserData::default()
                }),
                ..Item::default()
            })
            .collect();
        assert_eq!(first_unplayed(&items), 2);
        let all_watched: Vec<Item> = items
            .iter()
            .cloned()
            .map(|mut item| {
                if let Some(user) = &mut item.user_data {
                    user.played = true;
                }
                item
            })
            .collect();
        assert_eq!(
            first_unplayed(&all_watched),
            0,
            "a finished library starts over"
        );
    }

    #[test]
    fn a_shuffled_queue_keeps_every_item_once() {
        let items: Vec<Item> = (0..8)
            .map(|index| Item {
                id: format!("film-{index}"),
                name: format!("Film {index}"),
                kind: "Movie".into(),
                ..Item::default()
            })
            .collect();
        let mut shuffled = items.clone();
        shuffle_items(&mut shuffled);
        let mut ids: Vec<&str> = shuffled.iter().map(|item| item.id.as_str()).collect();
        ids.sort_unstable();
        let mut expected: Vec<&str> = items.iter().map(|item| item.id.as_str()).collect();
        expected.sort_unstable();
        assert_eq!(ids, expected, "shuffle is a permutation, never a loss");
    }

    fn wait(state: &mut State, seconds: u64, mut ready: impl FnMut(&State) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        while Instant::now() < deadline {
            state.poll();
            if ready(state) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        false
    }

    #[test]
    fn a_timeline_hover_maps_to_the_trickplay_tile_and_clears_on_leave() {
        let mut state = State::from_config(Config::fresh());
        let mut session = crate::demo::in_app();
        // The live shape: 640×360 thumbnails, 10×10 per sprite, 156 thumbnails.
        session.item.trickplay = Some(std::collections::HashMap::from([(
            "quiet-floor-s2e2".to_string(),
            std::collections::HashMap::from([(
                "640".to_string(),
                jellyfin_api::TrickplayInfo {
                    width: 640,
                    height: 360,
                    tile_width: 10,
                    tile_height: 10,
                    thumbnail_count: 156,
                    interval: 10_000,
                    bandwidth: 8_000,
                },
            )]),
        )]));
        state.in_app = Some(session);
        state.in_app_seek_width = 1232.0;

        // Half way through a 48-minute episode is 24:00, thumbnail 144, tile 1.
        state.hover_in_app_seek(616.0);
        let (x, ms) = state.in_app_hover.expect("the hover is tracked");
        assert!((x - 616.0).abs() < 0.01);
        assert_eq!(ms, 1_440_000);

        let item = state
            .in_app
            .as_ref()
            .map(|session| session.item.clone())
            .expect("a session");
        let key = state.trickplay_key(&item, ms).expect("a tile key");
        assert!(key.contains("-t640-1-"), "tile 1 is addressable: {key}");

        // Tile 0 ends at thumbnail 99; thumbnail 100 opens tile 1. Past the last generated
        // thumbnail the key clamps instead of asking for a sprite the server does not have.
        assert!(
            state
                .trickplay_key(&item, 999_000)
                .is_some_and(|k| k.contains("-t640-0-"))
        );
        assert!(
            state
                .trickplay_key(&item, 1_000_000)
                .is_some_and(|k| k.contains("-t640-1-"))
        );
        assert!(
            state
                .trickplay_key(&item, 9_999_999)
                .is_some_and(|k| k.contains("-t640-1-"))
        );

        state.clear_in_app_hover();
        assert!(state.in_app_hover.is_none());
    }

    #[test]
    fn a_forced_subtitle_track_turns_itself_on() {
        let mut state = State::from_config(Config::fresh());
        state.in_app = Some(crate::demo::in_app());
        let mut tracks = crate::demo::subtitle_tracks();
        tracks[0].is_forced = true;
        tracks[0].display_title = "English - Forced".into();
        state.apply_in_app_playback_streams(Ok(PlaybackStreams {
            source: Some("source-1".into()),
            container: None,
            video: None,
            subtitles: tracks,
            audio: Vec::new(),
            trickplay: None,
            chapters: Vec::new(),
        }));
        assert_eq!(state.in_app_subtitles.selected, Some(0));
        assert_eq!(state.in_app_subtitles.tracks.len(), 2);
    }

    #[test]
    fn a_dolby_vision_item_is_recognised_from_its_range_label() {
        let item = |range: &str| Item {
            media_sources: vec![jellyfin_api::MediaSource {
                media_streams: vec![jellyfin_api::MediaStream {
                    kind: "Video".into(),
                    video_range: Some(range.into()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(item_is_dolby_vision(&item("DOVIWithHDR10")));
        assert!(item_is_dolby_vision(&item("Dolby Vision")));
        assert!(!item_is_dolby_vision(&item("HDR10")));
    }

    #[test]
    fn a_zero_based_transcode_clock_gets_the_resume_point_added() {
        assert_eq!(transcode_position_base(1_000, 705_000), 705_000);
        assert_eq!(transcode_position_base(706_000, 705_000), 0);
        assert_eq!(transcode_position_base(0, 705_000), 0, "no sample yet");
        assert_eq!(transcode_position_base(1_000, 0), 0, "nothing to add");
    }

    #[test]
    fn a_selected_track_shows_the_cue_under_the_delayed_playhead() {
        let mut state = State::from_config(Config::fresh());
        let mut session = crate::demo::in_app();
        session.position_ms = 42 * 60_000;
        state.in_app = Some(session);
        state.in_app_subtitles.tracks = crate::demo::subtitle_tracks();
        state.in_app_subtitles.selected = Some(0);
        state.in_app_subtitles.cues = crate::demo::subtitle_cues();

        assert_eq!(
            state.current_in_app_subtitle().map(|cue| cue.text.as_str()),
            Some("The board has reviewed your request.\nYour request has been denied.")
        );
        // A positive delay means the words arrive later: the playhead reads five seconds back.
        state.adjust_in_app_subtitle_delay(5_000);
        assert_eq!(
            state.current_in_app_subtitle().map(|cue| cue.text.as_str()),
            Some("Previously on The Quiet Floor…")
        );
        state.adjust_in_app_subtitle_size(0.1);
        assert!((state.in_app_subtitles.size - 1.1).abs() < 0.001);

        state.select_in_app_subtitle(None);
        assert!(state.current_in_app_subtitle().is_none());
    }

    #[test]
    #[ignore = "needs RUNEFIN_PROBE_ITEM (a file the live server can serve) and GStreamer"]
    fn a_live_file_decodes_in_the_app_pipeline_without_reporting() {
        // A URL lets the same probe cover a live tuner stream, which has no Jellyfin item
        // URL of the usual `/Videos/{id}/stream` shape.
        let (url, label) = if let Ok(url) = std::env::var("RUNEFIN_PROBE_URL") {
            (url, "url".to_string())
        } else {
            let item_id = std::env::var("RUNEFIN_PROBE_ITEM").expect("set RUNEFIN_PROBE_ITEM");
            let config = Config::load();
            assert!(config.has_session(), "sign in once before this test");
            let client = Client::new(&config.server_url, config.device_id.clone())
                .with_session(config.token.clone(), config.user_id.clone());
            let url = client
                .direct_stream_url(&item_id)
                .expect("the item has a direct stream URL");
            (url, item_id)
        };
        let started_at = Instant::now();
        let player = crate::player::Player::start(&url, rune::NativeSurface::new(), None)
            .expect("the app pipeline starts");
        let startup_ms = started_at.elapsed().as_secs_f64() * 1000.0;
        let rate_set = if let Ok(rate) = std::env::var("RUNEFIN_PROBE_RATE")
            && let Ok(rate) = rate.parse::<f64>()
        {
            player.set_rate(rate);
            eprintln!("probe rate {rate}");
            true
        } else {
            false
        };
        let path = player.path_label();
        let started = Instant::now();
        let mut position = 0;
        while started.elapsed() < Duration::from_secs(8) {
            position = player.position_ms();
            if position > 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if std::env::var("RUNEFIN_PROBE_SEEK").is_ok() {
            if let Ok(delay) = std::env::var("RUNEFIN_PROBE_SEEK_DELAY_MS")
                && let Ok(delay) = delay.parse::<u64>()
            {
                std::thread::sleep(Duration::from_millis(delay));
            }
            let from = player.position_ms();
            let target = from + 60_000;
            let started = Instant::now();
            player.seek_ms(target);
            let mut landed = from;
            while started.elapsed() < Duration::from_secs(8) {
                landed = player.position_ms();
                if landed >= target - 2_000 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            eprintln!(
                "probe seek {from} -> {landed} ms in {:.0} ms",
                started.elapsed().as_secs_f64() * 1000.0
            );
        }
        if rate_set {
            let before = player.position_ms();
            let frames_before = player.frames();
            let started = Instant::now();
            std::thread::sleep(Duration::from_secs(3));
            let after = player.position_ms();
            let frames_after = player.frames();
            let seconds = started.elapsed().as_secs_f64();
            eprintln!(
                "probe advance {} ms in {:.2} s, frames {} ({:.1} fps)",
                after - before,
                seconds,
                frames_after - frames_before,
                (frames_after - frames_before) as f64 / seconds
            );
        }
        let buffering = player.buffering();
        eprintln!("probe buffering after playback: {buffering:?}");
        assert!(
            buffering.is_none(),
            "a direct stream should not report a stall it is not in"
        );
        let duration = player.duration_ms();
        player.stop();
        eprintln!("probe {label}: {path}, start {startup_ms:.0} ms, {position} ms of {duration:?}");
        assert!(position > 0, "no clock movement from the app pipeline");
    }

    #[test]
    #[ignore = "needs a signed-in config"]
    fn a_live_queue_steps_to_the_next_item() {
        let item_id = std::env::var("RUNEFIN_PLAY_ITEM").expect("set RUNEFIN_PLAY_ITEM");
        let config = Config::load();
        assert!(config.has_session(), "sign in once before this test");
        let client = Client::new(&config.server_url, config.device_id.clone())
            .with_session(config.token.clone(), config.user_id.clone());
        let item = client.playback_item(&item_id).expect("the item");
        let season = item.season_id.clone().expect("an episode with a season");
        let episodes = client.episodes(&season).expect("the season's episodes");
        assert!(episodes.len() >= 2, "need two episodes to step between");
        let mut state = State::from_config(config);
        let waker: rune::Waker = std::sync::Arc::new(|| {});
        state.setup(waker);
        state.play_queue(episodes.clone(), 0, false);
        assert!(
            wait(&mut state, 20, |state| state.in_app.is_some()),
            "GStreamer did not start: {:?}",
            state.playback_error
        );
        for _ in 0..100 {
            state.poll();
            if state
                .in_app
                .as_ref()
                .is_some_and(|session| session.position_ms > 0)
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let first = state
            .in_app
            .as_ref()
            .map(|session| session.item_id.clone())
            .unwrap_or_default();
        eprintln!(
            "before step: queue {} at {}",
            state.queue.len(),
            state.queue_at
        );
        state.play_in_app_next_episode();
        eprintln!(
            "after step: queue {} at {}",
            state.queue.len(),
            state.queue_at
        );
        assert_eq!(state.queue_at, 1, "the queue moved to its second item");
        assert!(
            wait(&mut state, 20, |state| state.in_app.as_ref().is_some_and(
                |session| session.item_id != first && session.position_ms > 0
            )),
            "the next item did not start playing"
        );
        eprintln!(
            "queue: {} -> {}, {} total",
            first,
            state
                .in_app
                .as_ref()
                .map(|session| session.item_id.clone())
                .unwrap_or_default(),
            state.queue.len()
        );
        state.stop_in_app();
        assert!(state.queue.is_empty(), "a manual stop ends the run");
    }

    #[test]
    #[ignore = "needs a signed-in live server with an ASS subtitle track"]
    fn a_live_ass_track_keeps_its_alignment_and_positions() {
        let config = Config::load();
        assert!(config.has_session(), "sign in once before this test");
        let client = jellyfin_api::Client::new(&config.server_url, config.device_id.clone())
            .with_session(config.token.clone(), config.user_id.clone());
        let mut query = jellyfin_api::ItemQuery {
            recursive: true,
            include_types: vec!["Episode"],
            limit: 100,
            ..jellyfin_api::ItemQuery::default()
        };
        query.extra_fields = Some("MediaSources".into());
        let episodes = client.items(&query).expect("episodes").items;
        let found = episodes.into_iter().find_map(|item| {
            let source = item.primary_media_source()?.id.clone();
            let track = item
                .subtitle_tracks()
                .into_iter()
                .find(|track| track.codec.as_deref() == Some("ass"))?;
            Some((item.id, item.name, source, track))
        });
        let Some((id, name, source, track)) = found else {
            eprintln!("ass: no ASS track in the first hundred episodes");
            return;
        };
        let url = client
            .subtitle_url_with(&id, &source, track.index, "ass")
            .expect("a signed-in client");
        let bytes = client.fetch_bytes(&url).expect("the ASS file");
        let cues = crate::subtitles::parse_ass(&bytes);
        let positioned = cues
            .iter()
            .filter(|cue| cue.pos.is_some() || cue.align != crate::subtitles::Align::BottomCenter)
            .count();
        let coloured = cues
            .iter()
            .filter(|cue| cue.color != [255, 255, 255])
            .count();
        eprintln!(
            "ass: {:?} -> {} cues, {} positioned, {} coloured, first {:?}",
            name,
            cues.len(),
            positioned,
            coloured,
            cues.first().map(|cue| cue.text.clone())
        );
        assert!(!cues.is_empty(), "the ASS track parsed into cues");
        assert!(positioned > 0, "at least one event carried a place");
        assert!(coloured > 0, "at least one event carried its own colour");
    }

    #[test]
    #[ignore = "needs a live tuner and a signed-in config"]
    fn a_live_channel_opens_plays_and_closes() {
        let config = Config::load();
        assert!(config.has_session(), "sign in once before this test");
        let mut state = State::from_config(config);
        let waker: rune::Waker = std::sync::Arc::new(|| {});
        state.setup(waker);
        state.open_live_tv();
        assert!(
            wait(&mut state, 20, |state| !state.live.channels.is_empty()),
            "no channels arrived: {:?}",
            state.live.error
        );
        eprintln!(
            "guide: {} channels, {} with next",
            state.live.guide.len(),
            state.live.next.len()
        );
        assert!(
            !state.live.guide.is_empty(),
            "the guide window returned no programs"
        );
        let channel = state.live.channels[0].clone();
        let launch = Instant::now();
        state.play_live_channel(channel.clone());
        assert!(
            wait(&mut state, 25, |state| state.in_app.is_some()),
            "the tuner never produced a player: {:?} {:?}",
            state.live.error,
            state.playback_error
        );
        let launch_ms = launch.elapsed().as_secs_f64() * 1000.0;
        eprintln!("idle inhibitor: {}", state.idle.active());
        let frames_before = state
            .in_app
            .as_ref()
            .expect("a live session")
            .player
            .frames();
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(3) {
            state.poll();
            std::thread::sleep(Duration::from_millis(50));
        }
        let seconds = started.elapsed().as_secs_f64();
        let session = state.in_app.as_ref().expect("a live session");
        assert!(session.live);
        let position = session.position_ms;
        let frames = session.player.frames() - frames_before;

        // Channel up, like a remote: the old tuner closes and the next one opens. Most of this
        // server's Xtream channels answer 500 from the provider, so a refused open is reported
        // and skipped rather than failing the whole run.
        let second = state.live.channels.get(1).map(|channel| channel.id.clone());
        if let Some(second) = second {
            state.zap_live(1);
            let opened = wait(&mut state, 25, |state| {
                state
                    .in_app
                    .as_ref()
                    .is_some_and(|session| session.item_id == second && session.position_ms > 0)
            });
            if opened {
                eprintln!(
                    "zapped to {} ({} ms)",
                    state
                        .in_app
                        .as_ref()
                        .map(|session| session.item.name.clone())
                        .unwrap_or_default(),
                    state
                        .in_app
                        .as_ref()
                        .map(|session| session.position_ms)
                        .unwrap_or(0)
                );
            } else {
                eprintln!("zapping unavailable on this server: {:?}", state.live.error);
                assert!(
                    state.in_app.is_none(),
                    "a refused channel must not leave the old stream playing"
                );
            }
        }
        state.stop_in_app();
        assert!(wait(&mut state, 5, |state| state.in_app.is_none()));
        assert!(
            !state.idle.active(),
            "stopping playback releases the idle inhibitor"
        );
        eprintln!(
            "live channel {} opened in {launch_ms:.0} ms, {frames} frames ({:.1} fps) in {seconds:.1} s, stopped at {position} ms",
            channel.name,
            frames as f64 / seconds
        );
        assert!(position > 0, "the live clock did not move");
        assert!(state.playback_error.is_none(), "{:?}", state.playback_error);
    }

    #[test]
    #[ignore = "needs RUNEFIN_PLAY_ITEM and gst-plugins-good (Matroska/MP4 demuxers)"]
    fn a_live_item_plays_in_app_and_advances_its_clock() {
        let item_id = std::env::var("RUNEFIN_PLAY_ITEM").expect("set RUNEFIN_PLAY_ITEM");
        let config = Config::load();
        assert!(config.has_session(), "sign in once before this test");
        let mut state = State::from_config(config.clone());
        let waker: rune::Waker = std::sync::Arc::new(|| {});
        state.setup(waker);
        state.open_detail(item_id.clone());
        assert!(
            wait(&mut state, 20, |state| state
                .detail
                .as_ref()
                .is_some_and(|detail| !detail.loading)),
            "detail did not load"
        );
        state.play_detail();
        assert!(
            wait(&mut state, 20, |state| state.in_app.is_some()),
            "GStreamer did not start: {:?}",
            state.playback_error
        );
        // Subtitles are fetched separately from playback: prove the live item's track list
        // and its converted SRT arrive before the clock check.
        for _ in 0..100 {
            state.poll();
            if !state.in_app_subtitles.tracks.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if !state.in_app_subtitles.tracks.is_empty() {
            state.select_in_app_subtitle(Some(0));
            for _ in 0..200 {
                state.poll();
                if !state.in_app_subtitles.cues.is_empty() || state.in_app_subtitles.error.is_some()
                {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            eprintln!(
                "subtitles: {} tracks, {} audio, {} cues, error={:?}",
                state.in_app_subtitles.tracks.len(),
                state.in_app_audio_tracks.len(),
                state.in_app_subtitles.cues.len(),
                state.in_app_subtitles.error
            );
            assert!(
                !state.in_app_subtitles.cues.is_empty(),
                "the selected subtitle track produced no cues"
            );
        } else {
            eprintln!("subtitles: this item has no text tracks");
        }
        // The up-next lookup runs alongside playback; an episode with a following one should
        // answer within a few seconds, and the last episode should stay empty without error.
        for _ in 0..50 {
            state.poll();
            if state.in_app_next.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        eprintln!(
            "next episode: {:?}",
            state.in_app_next.as_ref().map(|item| item.name.clone())
        );
        eprintln!(
            "previous episode: {:?}",
            state.in_app_prev.as_ref().map(|item| item.name.clone())
        );
        let start_position = state
            .in_app
            .as_ref()
            .map_or(0, |session| session.position_ms);
        // Jellyfin intentionally ignores a fresh item under its five-minute resume threshold.
        // Move past it so the test verifies the stop report rather than that policy.
        if start_position < 300_000 {
            state.seek_in_app(300_000 - start_position);
        }
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(30) {
            state.poll();
            std::thread::sleep(Duration::from_millis(100));
        }
        let position = state
            .in_app
            .as_ref()
            .map_or(0, |session| session.position_ms);
        eprintln!(
            "in-app={} position={position} error={:?}",
            state.in_app.is_some(),
            state.playback_error
        );
        assert!(
            position > 0,
            "the in-app clock did not advance: {start_position} -> {position}"
        );
        state.stop_in_app();
        for _ in 0..20 {
            state.poll();
            std::thread::sleep(Duration::from_millis(100));
        }
        let client = Client::new(&config.server_url, config.device_id.clone())
            .with_session(config.token.clone(), config.user_id.clone());
        let mut stored = None;
        for _ in 0..20 {
            let resume = client.resume(24).expect("resume after in-app playback");
            if let Some(item) = resume.items.iter().find(|item| item.id == item_id) {
                let stored_position = item
                    .user_data
                    .as_ref()
                    .and_then(|user| user.position_ms())
                    .unwrap_or(0);
                if stored_position >= 10_000 {
                    stored = Some(stored_position);
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        let stored = stored.unwrap_or_else(|| {
            panic!("item {item_id} did not return to Continue Watching after {position} ms")
        });
        assert!(stored >= 10_000, "server kept only {stored} ms");
        eprintln!(
            "in-app playback: {start_position} ms -> {position} ms, server stored {stored} ms"
        );
    }
}
