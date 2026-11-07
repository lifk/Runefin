//! Publishing the in-app player to MPRIS and answering its commands. The service lives in
//! `crate::mpris`; this module is the bridge to `State`, kept out of the already large state
//! module.

use super::*;

impl State {
    /// Called from `poll`: apply what the desktop asked for, then publish the current picture.
    /// Returns true when a command changed something the view draws.
    pub(super) fn sync_mpris(&mut self) -> bool {
        let Some(mpris) = self.mpris.clone() else {
            return false;
        };
        let mut changed = false;
        for command in mpris.take_commands() {
            changed |= self.apply_mpris(command);
        }
        mpris.publish(self.mpris_snapshot());
        changed
    }

    fn apply_mpris(&mut self, command: crate::mpris::Command) -> bool {
        use crate::mpris::Command;
        match command {
            Command::PlayPause => {
                if self.in_app.is_some() {
                    self.toggle_in_app_pause();
                    true
                } else if self.playback.is_some() {
                    self.toggle_pause();
                    true
                } else {
                    false
                }
            }
            Command::Play => {
                if self.in_app.as_ref().is_some_and(|session| session.paused) {
                    self.toggle_in_app_pause();
                    true
                } else if self.playback.as_ref().is_some_and(|session| session.paused) {
                    self.toggle_pause();
                    true
                } else {
                    false
                }
            }
            Command::Pause => {
                if self.in_app.as_ref().is_some_and(|session| !session.paused) {
                    self.toggle_in_app_pause();
                    true
                } else if self
                    .playback
                    .as_ref()
                    .is_some_and(|session| !session.paused)
                {
                    self.toggle_pause();
                    true
                } else {
                    false
                }
            }
            Command::Stop => {
                if self.in_app.is_some() {
                    self.stop_in_app();
                    true
                } else if self.playback.is_some() {
                    self.stop_playback();
                    true
                } else {
                    false
                }
            }
            Command::Next => {
                if self.in_app.is_some() && self.queue_next_available() {
                    self.play_in_app_next_episode();
                    true
                } else {
                    false
                }
            }
            Command::Previous => {
                if self.in_app.is_some() && self.queue_previous_available() {
                    self.play_in_app_previous();
                    true
                } else {
                    false
                }
            }
            Command::Seek(offset_us) => {
                if self.in_app.as_ref().is_some_and(|session| !session.live) {
                    self.seek_in_app(offset_us / 1_000);
                    true
                } else {
                    false
                }
            }
            Command::SetPosition(position_us) => {
                if let Some(session) = &self.in_app
                    && !session.live
                {
                    let target = position_us / 1_000;
                    self.seek_in_app(target.saturating_sub(session.position_ms));
                    true
                } else {
                    false
                }
            }
            Command::SetVolume(volume) => {
                if self.in_app.is_some() {
                    self.set_in_app_volume(volume);
                    true
                } else {
                    false
                }
            }
            Command::SetRate(rate) => {
                if self.in_app.is_some() {
                    self.set_in_app_speed(rate);
                    true
                } else {
                    false
                }
            }
            Command::SetFullscreen(fullscreen) => {
                self.fullscreen = fullscreen;
                self.pending_fullscreen = Some(fullscreen);
                true
            }
            Command::SetShuffle(shuffle) => {
                self.mpris_shuffle = shuffle;
                true
            }
            Command::SetLoopStatus(status) => match status.as_str() {
                "Track" => {
                    if let Some(session) = &self.in_app {
                        self.loop_a = Some(0);
                        self.loop_b = session.duration_ms;
                        true
                    } else {
                        false
                    }
                }
                _ => {
                    self.loop_a = None;
                    self.loop_b = None;
                    true
                }
            },
        }
    }

    fn mpris_snapshot(&self) -> crate::mpris::Snapshot {
        use crate::mpris::PlaybackStatus;
        let mut snapshot = crate::mpris::Snapshot {
            fullscreen: self.fullscreen,
            shuffle: self.mpris_shuffle,
            loop_status: if self.loop_a.is_some() && self.loop_b.is_some() {
                "Track".into()
            } else {
                "None".into()
            },
            ..Default::default()
        };
        if let Some(session) = &self.in_app {
            snapshot.status = if session.paused {
                PlaybackStatus::Paused
            } else {
                PlaybackStatus::Playing
            };
            snapshot.title = session.item.name.clone();
            snapshot.artist = session.item.series_name.clone().unwrap_or_default();
            snapshot.album = session.item.episode_label().unwrap_or_default();
            snapshot.track_id = session.item.id.clone();
            snapshot.length_us = session.duration_ms.unwrap_or(0).max(0) * 1_000;
            snapshot.position_us = session.position_ms.max(0) * 1_000;
            snapshot.volume = self.in_app_volume.clamp(0.0, 1.0);
            snapshot.rate = self.in_app_speed;
            snapshot.can_next = self.queue_next_available();
            snapshot.can_previous = self.queue_previous_available();
            snapshot.can_seek = !session.live;
            snapshot.art_url = self.mpris_art(&session.item);
            return snapshot;
        }
        if let Some(session) = &self.playback {
            snapshot.status = if session.paused {
                PlaybackStatus::Paused
            } else {
                PlaybackStatus::Playing
            };
            snapshot.title = session.item.name.clone();
            snapshot.artist = session.item.series_name.clone().unwrap_or_default();
            snapshot.album = session.item.episode_label().unwrap_or_default();
            snapshot.track_id = session.item.id.clone();
            snapshot.length_us = session.duration_ms.unwrap_or(0).max(0) * 1_000;
            snapshot.position_us = session.position_ms.max(0) * 1_000;
            snapshot.volume = self.in_app_volume.clamp(0.0, 1.0);
            snapshot.rate = 1.0;
            snapshot.can_seek = false;
            snapshot.art_url = self.mpris_art(&session.item);
        }
        snapshot
    }

    /// A `file://` URL for the poster already in the image cache. The server URL would carry
    /// the access token, which an MPRIS reader must never see.
    fn mpris_art(&self, item: &Item) -> Option<String> {
        let key = self.image_key(item)?;
        let path = crate::image::cache_path(&key)?;
        path.exists().then(|| format!("file://{}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_snapshot_names_the_playing_episode() {
        let mut state = State::from_config(Config::fresh());
        let mut session = crate::demo::in_app();
        session.item.series_name = Some("Severance".into());
        state.in_app = Some(session);
        state.in_app_volume = 0.6;
        state.in_app_speed = 1.25;
        let snapshot = state.mpris_snapshot();
        assert_eq!(snapshot.status, crate::mpris::PlaybackStatus::Playing);
        assert_eq!(snapshot.title, "Goodbye, Mrs. Selvig");
        assert_eq!(snapshot.artist, "Severance");
        assert_eq!(snapshot.album, "S2E02");
        assert!((snapshot.volume - 0.6).abs() < 0.001);
        assert!((snapshot.rate - 1.25).abs() < 0.001);
        assert!(snapshot.can_seek);
        assert!(snapshot.length_us > 0);
    }

    #[test]
    fn a_pause_command_pauses_and_a_seek_moves_by_microseconds() {
        let mut state = State::from_config(Config::fresh());
        state.in_app = Some(crate::demo::in_app());
        assert!(state.apply_mpris(crate::mpris::Command::Pause));
        assert!(state.in_app.as_ref().expect("playing").paused);
        assert!(state.apply_mpris(crate::mpris::Command::Seek(10_000_000)));
        assert_eq!(
            state.in_app.as_ref().expect("playing").position_ms,
            42 * 60_000 + 10_000
        );
        assert!(state.apply_mpris(crate::mpris::Command::SetFullscreen(true)));
        assert!(state.fullscreen);
        assert_eq!(state.pending_fullscreen, Some(true));
    }

    #[test]
    fn a_stopped_snapshot_has_no_metadata_but_keeps_fullscreen() {
        let mut state = State::from_config(Config::fresh());
        state.fullscreen = true;
        let snapshot = state.mpris_snapshot();
        assert_eq!(snapshot.status, crate::mpris::PlaybackStatus::Stopped);
        assert_eq!(snapshot.title, "");
        assert!(snapshot.fullscreen);
    }
    #[test]
    fn mpris_shuffle_and_loop_reach_the_snapshot() {
        let mut state = crate::state::State::from_config(crate::config::Config::fresh());
        assert!(state.apply_mpris(crate::mpris::Command::SetShuffle(true)));
        assert!(state.mpris_snapshot().shuffle);
        state.in_app = Some(crate::demo::in_app());
        assert!(state.apply_mpris(crate::mpris::Command::SetLoopStatus("Track".into())));
        assert_eq!(state.mpris_snapshot().loop_status, "Track");
        assert!(state.apply_mpris(crate::mpris::Command::SetLoopStatus("None".into())));
        assert_eq!(state.mpris_snapshot().loop_status, "None");
    }
}
