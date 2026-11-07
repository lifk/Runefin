//! Remote-control state: the other Jellyfin sessions and the commands sent to them.

use jellyfin_api::SessionInfo;

use super::*;

#[derive(Default)]
pub struct RemoteState {
    pub sessions: Vec<SessionInfo>,
    pub loading: bool,
    pub error: Option<String>,
}

impl State {
    pub fn open_remote(&mut self) {
        self.stop_theme_media();
        self.route = Route::Remote;
        self.load_remote();
    }

    pub fn load_remote(&mut self) {
        if !self.fetch_home {
            return;
        }
        let Some(client) = self.client() else {
            return;
        };
        self.remote.loading = true;
        self.remote.error = None;
        self.remote_latest.request((), move |()| {
            client.sessions().map_err(|error| error.user_message())
        });
    }

    pub(super) fn apply_remote(&mut self, result: Result<Vec<SessionInfo>, String>) {
        self.remote.loading = false;
        match result {
            Ok(sessions) => {
                self.remote.sessions = sessions;
                self.remote.error = None;
            }
            Err(error) => self.remote.error = Some(error),
        }
    }

    /// `Pause`, `Unpause` or `Stop` on another session.
    pub fn remote_command(&mut self, session_id: String, command: &'static str) {
        let Some(client) = self.client() else {
            return;
        };
        self.remote.error = None;
        self.worker.remote_command(client, session_id, command);
    }

    pub fn remote_seek(&mut self, session_id: String, position_ticks: i64) {
        let Some(client) = self.client() else {
            return;
        };
        self.remote.error = None;
        self.worker.remote_seek(client, session_id, position_ticks);
    }

    pub(super) fn remote_command_finished(&mut self, result: Result<(), String>) {
        match result {
            Ok(()) => self.load_remote(),
            Err(error) => self.remote.error = Some(error),
        }
    }
}
