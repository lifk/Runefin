//! The playback session: start mpv on a worker, feed its events into progress reports, and end
//! with the stop payload that makes Continue Watching truthful.

use std::time::{Duration, Instant};

use jellyfin_api::{PlaybackReport, PlaybackStop};

use super::*;
use crate::handoff::{self, PlayRequest, PlaybackEvent};

pub(super) struct PendingPlay {
    pub(super) item: Item,
    pub(super) session_id: String,
    pub(super) start_ms: i64,
    pub(super) generation: u64,
}

pub struct PlaybackSession {
    pub item: Item,
    pub item_id: String,
    pub session_id: String,
    pub position_ms: i64,
    pub duration_ms: Option<i64>,
    pub paused: bool,
    pub generation: u64,
    pub last_report: Instant,
    pub handle: Option<PlaybackHandle>,
}

impl State {
    /// The primary action on a detail page. A series starts its first unplayed episode; a movie
    /// or episode starts itself.
    pub fn play_detail(&mut self) {
        let target = self.detail_target();
        match target {
            Some(item) => self.play_in_app(item),
            None => self.detail_notice = Some("No episodes to play yet.".into()),
        }
    }

    /// Play the same target from the beginning. The primary button resumes, so this is the only
    /// way back to zero without seeking.
    /// The detail page's Trailer action: a server-hosted trailer plays in the app, a remote
    /// YouTube trailer opens in the browser (there is no bundled extractor, and a web view is
    /// outside the product).
    /// Play the detail page with a chosen media source (a version/edition). The next
    /// `play_in_app_media` consumes the choice and clears it.
    pub fn play_detail_source(&mut self, source_id: String) {
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
        self.preplay_open = false;
        self.play_detail();
    }

    pub fn play_detail_trailer(&mut self) {
        let local = self
            .detail
            .as_ref()
            .and_then(|detail| detail.local_trailers.first().cloned());
        if let Some(trailer) = local {
            self.play_in_app_trailer(trailer);
            return;
        }
        let remote = self.detail.as_ref().and_then(|detail| {
            detail
                .item
                .remote_trailers
                .iter()
                .find_map(|trailer| trailer.url.clone())
        });
        let Some(url) = remote else {
            self.detail_notice = Some("No trailer for this item.".into());
            return;
        };
        if self.trailer_loading {
            return;
        }
        if crate::trailer::is_youtube(&url) {
            self.trailer_loading = true;
            self.show_toast("Preparing trailer…".to_string());
            self.trailer_load.request(url.clone(), move |url| {
                crate::trailer::download(&url).map(|path| (url, path))
            });
        } else {
            self.open_trailer_in_browser(&url);
        }
    }

    pub(super) fn apply_trailer_download(
        &mut self,
        load: Result<(String, std::path::PathBuf), String>,
    ) {
        self.trailer_loading = false;
        match load {
            Ok((url, path)) => {
                // The reader may have left the page while the download ran; a trailer for a
                // page that is no longer open must not start over whatever is there now.
                let Some(title) = self.detail.as_ref().and_then(|detail| {
                    detail
                        .item
                        .remote_trailers
                        .iter()
                        .find(|trailer| trailer.url.as_deref() == Some(url.as_str()))
                        .map(|trailer| trailer.name.clone().unwrap_or_else(|| "Trailer".into()))
                }) else {
                    return;
                };
                let item = Item {
                    id: "trailer".into(),
                    name: title,
                    kind: "Trailer".into(),
                    ..Item::default()
                };
                self.play_in_app_local(item, path);
            }
            Err(_) => {
                let url = self.detail.as_ref().and_then(|detail| {
                    detail
                        .item
                        .remote_trailers
                        .iter()
                        .find_map(|trailer| trailer.url.clone())
                });
                if let Some(url) = url {
                    self.open_trailer_in_browser(&url);
                } else {
                    self.detail_notice = Some("Could not download the trailer.".into());
                }
            }
        }
    }

    fn open_trailer_in_browser(&mut self, url: &str) {
        let message = url.to_string();
        std::thread::spawn(move || {
            let _ = State::open_url(&message);
        });
        self.show_toast("Opening the trailer in your browser".to_string());
    }

    pub fn play_detail_from_start(&mut self) {
        let target = self.detail_target();
        match target {
            Some(item) => self.play_in_app_at(item, Some(0), false),
            None => self.detail_notice = Some("No episodes to play yet.".into()),
        }
    }

    fn detail_target(&self) -> Option<Item> {
        self.detail
            .as_ref()
            .and_then(|detail| detail.play_target())
            .cloned()
    }

    pub(super) fn start_stream(&mut self, item: Item) {
        if self.playback.is_some() {
            self.stop_playback();
        }
        let Some(client) = self.client() else {
            self.detail_notice = Some("Your session expired. Sign in again.".into());
            return;
        };
        let Some(url) = client.direct_stream_url(&item.id) else {
            self.detail_notice = Some("This item has no playable stream.".into());
            return;
        };
        let start_ms = item
            .user_data
            .as_ref()
            .and_then(|user| user.position_ms())
            .unwrap_or(0);
        let session_id = format!(
            "runefin-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let generation = self.next_playback;
        self.next_playback = self.next_playback.wrapping_add(1);
        self.pending_play = Some(PendingPlay {
            item: item.clone(),
            session_id: session_id.clone(),
            start_ms,
            generation,
        });
        self.launching = true;
        self.playback_error = None;
        self.playback_start.request(
            PlayRequest {
                binary: "mpv".into(),
                url,
                start_ms,
            },
            |request| handoff::start(&request),
        );
    }

    pub(super) fn apply_playback_start(&mut self, result: Result<PlaybackHandle, Failure>) {
        self.launching = false;
        match result {
            Ok(mut handle) => {
                let Some(pending) = self.pending_play.take() else {
                    handle.quit();
                    return;
                };
                if let Some(reader) = handle.reader.take() {
                    self.worker.watch_playback(reader, pending.generation);
                }
                let duration = pending.item.runtime_ms();
                self.playback = Some(PlaybackSession {
                    item_id: pending.item.id.clone(),
                    item: pending.item,
                    session_id: pending.session_id.clone(),
                    position_ms: pending.start_ms,
                    duration_ms: duration,
                    paused: false,
                    generation: pending.generation,
                    last_report: Instant::now(),
                    handle: Some(handle),
                });
                if let Some(client) = self.client() {
                    let mut report = PlaybackReport::direct_play(
                        self.playback
                            .as_ref()
                            .map_or_else(String::new, |session| session.item_id.clone()),
                        pending.start_ms,
                        false,
                    );
                    report.play_session_id = Some(pending.session_id);
                    self.worker.report_playing(client, report);
                }
            }
            Err(failure) => {
                self.pending_play = None;
                self.playback_error = Some(failure.message.clone());
                self.detail_notice = Some(failure.message);
            }
        }
    }

    pub(super) fn on_playback_event(&mut self, generation: u64, event: PlaybackEvent) {
        if self
            .playback
            .as_ref()
            .is_none_or(|session| session.generation != generation)
        {
            return;
        }
        match event {
            PlaybackEvent::Position(position) => {
                if let Some(session) = &mut self.playback {
                    session.position_ms = position.max(0);
                }
                self.send_progress(false);
            }
            PlaybackEvent::Duration(duration) => {
                if let Some(session) = &mut self.playback {
                    session.duration_ms = Some(duration.max(0));
                }
            }
            PlaybackEvent::Paused(paused) => {
                if let Some(session) = &mut self.playback {
                    session.paused = paused;
                }
                self.send_progress(true);
            }
            PlaybackEvent::Ended => self.finish_playback(false),
            PlaybackEvent::Exited => self.finish_playback(true),
        }
    }

    pub(super) fn on_reported(&mut self, kind: ReportKind, result: Result<(), String>) {
        match (kind, result) {
            (ReportKind::Stopped, Ok(())) => {
                // The server now has the final position; a fresh Continue Watching is truthful.
                self.load_home();
                if matches!(&self.route, Route::Detail { .. }) {
                    self.retry_detail();
                }
            }
            (_, Ok(())) => {}
            (_, Err(error)) => self.playback_error = Some(error),
        }
    }

    fn send_progress(&mut self, force: bool) {
        let Some(client) = self.client() else {
            return;
        };
        let Some(session) = &mut self.playback else {
            return;
        };
        let now = Instant::now();
        if !force && now.duration_since(session.last_report) < Duration::from_secs(10) {
            return;
        }
        session.last_report = now;
        let mut report = PlaybackReport::direct_play(
            session.item_id.clone(),
            session.position_ms,
            session.paused,
        );
        report.play_session_id = Some(session.session_id.clone());
        self.worker.report_progress(client, report);
    }

    pub fn toggle_pause(&mut self) {
        let changed = match &mut self.playback {
            Some(session) => {
                let paused = !session.paused;
                let Some(handle) = &mut session.handle else {
                    return;
                };
                if !handle.send(serde_json::json!({ "command": ["set", "pause", paused] })) {
                    return;
                }
                session.paused = paused;
                true
            }
            None => false,
        };
        if changed {
            self.send_progress(true);
        }
    }

    /// Stop mpv now and report the position we last saw, then let `on_reported` refresh home.
    pub fn stop_playback(&mut self) {
        self.playback_start.cancel();
        self.pending_play = None;
        self.launching = false;
        self.finish_playback(false);
    }

    fn finish_playback(&mut self, failed: bool) {
        let Some(mut session) = self.playback.take() else {
            return;
        };
        if let Some(handle) = &mut session.handle {
            handle.quit();
        }
        if let Some(client) = self.client() {
            let mut stop = PlaybackStop::new(session.item_id.clone(), session.position_ms);
            stop.play_session_id = Some(session.session_id.clone());
            stop.failed = failed;
            self.worker.report_stopped(client, stop);
        }
    }
}
