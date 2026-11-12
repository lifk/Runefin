//! Live TV: the channel list and the tuner handshake. A live channel is opened on the server
//! in two steps (`PlaybackInfo` for an open token, then `/LiveStreams/Open`), played from the
//! tuner's own URL, and closed on every exit path.

use super::*;
use crate::player::Player;
use jellyfin_api::LiveStream;

/// The channel rail: what the tuner exposes, and whether the last fetch is still running.
pub struct LiveState {
    pub channels: Vec<Item>,
    /// The next guide entry per channel id.
    pub next: std::collections::HashMap<String, Item>,
    /// Programs per channel over the guide window.
    pub guide: std::collections::HashMap<String, Vec<Item>>,
    /// The guide grid instead of the channel list.
    pub guide_open: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub favorites: std::collections::HashSet<String>,
    pub favorites_only: bool,
    /// Guide window in hours: 3 or 24.
    pub guide_hours: i64,
}

impl Default for LiveState {
    fn default() -> Self {
        Self {
            channels: Vec::new(),
            next: std::collections::HashMap::new(),
            guide: std::collections::HashMap::new(),
            guide_open: false,
            loading: false,
            error: None,
            favorites: std::collections::HashSet::new(),
            favorites_only: false,
            guide_hours: 3,
        }
    }
}

/// Channels plus the guide lookahead, produced by one worker pass.
pub struct LiveLoad {
    pub channels: Vec<Item>,
    pub next: std::collections::HashMap<String, Item>,
    pub guide: std::collections::HashMap<String, Vec<Item>>,
}

/// Channel up/down with wraparound, kept pure so the ends are tested without a tuner.
pub fn live_zap_index(at: usize, len: usize, delta: isize) -> usize {
    (at as isize + delta).rem_euclid(len.max(1) as isize) as usize
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// Reminders starting in the last two minutes. A reminder that was missed by more than that is
/// stale and should not pop up much later.
pub fn due_live_reminders(
    reminders: &[crate::config::LiveReminder],
    now: i64,
) -> Vec<crate::config::LiveReminder> {
    reminders
        .iter()
        .filter(|reminder| reminder.start_seconds <= now && now - reminder.start_seconds <= 120)
        .cloned()
        .collect()
}

/// A desktop notification through the session helper. Spawned so a slow or missing
/// `notify-send` can never hold a frame; the in-app toast is the guaranteed path.
fn notify_desktop(summary: &str, body: &str) {
    let summary = summary.to_string();
    let body = body.to_string();
    std::thread::spawn(move || {
        let _ = std::process::Command::new("notify-send")
            .arg("--app-name=Runefin")
            .arg(summary)
            .arg(body)
            .status();
    });
}

impl State {
    pub fn open_live_tv(&mut self) {
        self.stop_theme_media();
        self.route = Route::LiveTv;
        if self.live.channels.is_empty() && !self.live.loading {
            self.load_live_channels();
        }
    }

    /// Whether the next program on this channel already has a reminder.
    pub fn has_live_reminder(&self, program_id: &str) -> bool {
        self.live_reminders
            .iter()
            .any(|reminder| reminder.program_id == program_id)
    }

    /// Set or clear a reminder for the next program on a channel. The guide already loaded the
    /// start time, so no request is needed.
    pub fn toggle_next_live_reminder(&mut self, channel_id: &str) {
        let Some(program) = self.live.next.get(channel_id).cloned() else {
            self.show_toast("No next program to remind you about.");
            return;
        };
        if self
            .live_reminders
            .iter()
            .any(|reminder| reminder.program_id == program.id)
        {
            self.live_reminders
                .retain(|reminder| reminder.program_id != program.id);
            self.show_toast("Reminder removed.");
            self.persist_reminders();
            return;
        }
        let Some(start_seconds) = program
            .start_date
            .as_deref()
            .and_then(jellyfin_api::iso_seconds)
        else {
            self.show_toast("This program has no start time.");
            return;
        };
        let channel_name = self
            .live
            .channels
            .iter()
            .find(|channel| channel.id == channel_id)
            .map(|channel| channel.name.clone())
            .unwrap_or_else(|| "Live TV".into());
        self.live_reminders.push(crate::config::LiveReminder {
            program_id: program.id.clone(),
            channel_id: channel_id.to_string(),
            channel_name,
            program_name: program.name.clone(),
            start_seconds,
        });
        self.show_toast(format!("Reminder set for {}.", program.name));
        self.persist_reminders();
    }

    /// Fire reminders whose start time has arrived. Stale entries (missed by more than two
    /// minutes, or from a previous run) are dropped silently. Returns true when the list or a
    /// toast changed.
    pub fn poll_live_reminders(&mut self) -> bool {
        let now = now_seconds();
        let due = due_live_reminders(&self.live_reminders, now);
        let before = self.live_reminders.len();
        self.live_reminders
            .retain(|reminder| now - reminder.start_seconds < 120);
        let mut changed = self.live_reminders.len() != before;
        for reminder in due {
            if !self.reminders_fired.insert(reminder.program_id.clone()) {
                continue;
            }
            self.show_toast(format!(
                "{} starts now on {}.",
                reminder.program_name, reminder.channel_name
            ));
            if self.notifications {
                notify_desktop(
                    &format!("{} starts now", reminder.program_name),
                    &reminder.channel_name,
                );
            }
            changed = true;
        }
        if changed {
            self.persist_reminders();
        }
        changed
    }

    pub fn toggle_live_favorite(&mut self, channel_id: &str) {
        if !self.live.favorites.remove(channel_id) {
            self.live.favorites.insert(channel_id.to_string());
        }
    }

    pub fn toggle_live_favorites_only(&mut self) {
        self.live.favorites_only = !self.live.favorites_only;
    }

    pub fn toggle_live_guide(&mut self) {
        self.live.guide_open = !self.live.guide_open;
    }

    pub fn toggle_guide_window(&mut self) {
        self.live.guide_hours = if self.live.guide_hours >= 24 { 3 } else { 24 };
        self.load_live_channels();
    }

    pub fn load_live_channels(&mut self) {
        let Some(client) = self.client() else {
            return;
        };
        self.live.loading = true;
        self.live.error = None;
        let hours = self.live.guide_hours;
        self.live_latest.request((), move |_| {
            let channels = client.live_channels(120).map_err(|e| e.user_message())?;
            let now = now_seconds();
            let ids: Vec<String> = channels.iter().map(|channel| channel.id.clone()).collect();
            // One request for the whole guide window. A guide failure leaves Now/Next working
            // from the channel's own CurrentProgram and simply hides future blocks.
            let programs = client
                .live_guide(&ids, now - 3_600, now + hours * 3_600)
                .unwrap_or_default();
            let mut guide: std::collections::HashMap<String, Vec<Item>> =
                std::collections::HashMap::new();
            for program in programs {
                if let Some(channel) = &program.channel_id {
                    guide.entry(channel.clone()).or_default().push(program);
                }
            }
            for programs in guide.values_mut() {
                programs.sort_by_key(|program| program.start_date.clone());
            }
            let mut next = std::collections::HashMap::new();
            for channel in &channels {
                let end = channel
                    .current_program
                    .as_ref()
                    .and_then(|program| program.end_date.clone());
                if let Some(programs) = guide.get(&channel.id)
                    && let Some(program) = programs.iter().find(|program| {
                        end.as_deref().is_none_or(|end| {
                            program
                                .start_date
                                .as_deref()
                                .is_some_and(|start| start > end)
                        })
                    })
                {
                    next.insert(channel.id.clone(), program.clone());
                }
            }
            Ok(LiveLoad {
                channels,
                next,
                guide,
            })
        });
    }

    pub(super) fn apply_live_channels(&mut self, load: Result<LiveLoad, String>) {
        self.live.loading = false;
        match load {
            Ok(load) => {
                self.live.channels = load.channels;
                self.live.next = load.next;
                self.live.guide = load.guide;
                self.live.error = None;
                let requests =
                    self.image_requests(self.live.channels.iter(), crate::net::ImagePriority::High);
                self.send_images(requests);
            }
            Err(message) => self.live.error = Some(message),
        }
    }

    /// Clicking a channel starts the tuner in the background; the player is handed the URL once
    /// `/LiveStreams/Open` answers, so nothing blocks the click.
    /// The index of the channel on the tuner right now, if this is a live session.
    pub fn live_channel_index(&self) -> Option<usize> {
        let id = &self.in_app.as_ref()?.item_id;
        self.live
            .channels
            .iter()
            .position(|channel| &channel.id == id)
    }

    /// Channel up/down while a tuner stream plays. The old stream is closed before the next
    /// one opens, and the list wraps, which is what a remote's CH+ does at the end.
    pub fn zap_live(&mut self, delta: isize) {
        if !self.in_app.as_ref().is_some_and(|session| session.live) {
            return;
        }
        let channels = self.live.channels.len();
        if channels < 2 {
            return;
        }
        let at = self.live_channel_index().unwrap_or(0);
        let next = live_zap_index(at, channels, delta);
        let channel = self.live.channels[next].clone();
        let open = self
            .in_app
            .as_ref()
            .is_some_and(|session| session.live_stream_id.is_some());
        if !open {
            self.play_live_channel(channel);
            return;
        }
        // Stop first, then remember the target: a manual stop clears the pending zap, and this
        // one must survive it.
        self.stop_in_app();
        self.pending_zap = Some(channel);
    }

    pub fn play_live_channel(&mut self, item: Item) {
        if self.in_app.is_some() {
            self.finish_in_app();
        }
        if self.playback.is_some() {
            self.stop_playback();
        }
        self.pending_live = Some(item.clone());
        self.in_app_launching = true;
        self.launching = true;
        self.playback_error = None;
        self.live.error = None;
        let Some(client) = self.client() else {
            self.fail_live("Your session expired. Sign in again.".into());
            return;
        };
        self.live_open.request(item.id.clone(), move |id| {
            client.open_live_stream(&id).map_err(|error| Failure {
                stage: Stage::Player,
                message: error.user_message(),
            })
        });
    }

    pub(super) fn apply_live_open(&mut self, result: Result<LiveStream, Failure>) {
        let stream = match result {
            Ok(stream) => stream,
            Err(failure) => {
                self.fail_live(format!("Could not start live TV: {}", failure.message));
                return;
            }
        };
        let Some(item) = self.pending_live.take() else {
            // The viewer left while the tuner was opening; do not leak the stream.
            self.close_live_id(stream.live_stream_id);
            return;
        };
        let session_id = stream.play_session_id.clone().unwrap_or_else(|| {
            format!(
                "runefin-live-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            )
        });
        self.pending_in_app = Some(PendingInApp {
            item: item.clone(),
            session_id,
            start_ms: 0,
            rewound: 0,
            conflict: None,
            paused: false,
            live: true,
            live_stream_id: stream.live_stream_id.clone(),
            report: true,
            local: false,
        });
        self.in_app_segments.clear();
        let size = self.in_app_subtitles.size;
        let delay_ms = self.in_app_subtitles.delay_ms;
        self.in_app_subtitles = Subtitles {
            size,
            delay_ms,
            ..Subtitles::default()
        };
        self.in_app_audio_tracks.clear();
        self.in_app_next = None;
        self.in_app_volume = 1.0;
        let surface = rune::NativeSurface::new();
        let waker = self.waker.clone();
        self.in_app_start
            .request((stream.url, surface, waker), |(url, surface, waker)| {
                Player::start(&url, surface, waker)
                    .map(|player| crate::state::InAppStart {
                        started_ms: None,
                        player,
                        method: "DirectPlay",
                        play_session_id: None,
                        notice: None,
                    })
                    .map_err(|message| Failure {
                        stage: Stage::Player,
                        message,
                    })
            });
    }

    fn fail_live(&mut self, message: String) {
        self.pending_live = None;
        self.launching = false;
        self.in_app_launching = false;
        self.live.error = Some(message);
    }

    pub(super) fn close_live_id(&mut self, id: Option<String>) {
        let Some(id) = id else {
            return;
        };
        let Some(client) = self.client() else {
            return;
        };
        self.live_close.request(id, move |id| {
            client
                .close_live_stream(&id)
                .map_err(|error| error.user_message())
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reminder(id: &str, start: i64) -> crate::config::LiveReminder {
        crate::config::LiveReminder {
            program_id: id.into(),
            channel_id: "c1".into(),
            channel_name: "News".into(),
            program_name: "Nine O'Clock".into(),
            start_seconds: start,
        }
    }

    #[test]
    fn a_reminder_fires_at_its_start_and_not_hours_later() {
        let reminders = vec![
            reminder("past", 1_000),
            reminder("due", 1_100),
            reminder("future", 1_300),
        ];
        let due = due_live_reminders(&reminders, 1_200);
        assert_eq!(
            due.iter()
                .map(|reminder| reminder.program_id.as_str())
                .collect::<Vec<_>>(),
            ["due"],
            "a reminder from the past is stale; one in the future waits"
        );
    }
}
