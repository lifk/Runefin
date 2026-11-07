//! Quick Connect sign-in: initiate a code, poll until the server says it was approved, then
//! exchange the secret for a token. The code is typed into Jellyfin's own UI, so no password
//! ever passes through the app.

use super::*;
use jellyfin_api::{AuthResult, QuickConnectState};

/// What the connect screen shows while a code is waiting.
pub struct QuickLogin {
    pub server_url: String,
    pub server_name: String,
    pub version: String,
    pub code: String,
    pub secret: String,
    pub authenticated: bool,
    pub exchanging: bool,
    pub polling: bool,
    pub poll_at: Instant,
    pub error: Option<String>,
}

pub(super) struct QuickStart {
    pub server_name: String,
    pub version: String,
    pub state: QuickConnectState,
}

impl State {
    /// Validate the address, then probe and initiate on a worker. The card appears immediately
    /// with a "pending" state; the code arrives when the server answers.
    pub fn start_quick_connect(&mut self) {
        let url = self.server.text.trim().to_string();
        if url.is_empty() {
            self.step = Step::Failed(Failure {
                stage: Stage::Server,
                message: "Enter your server's address first.".into(),
            });
            return;
        }
        if self.config.device_id.is_empty() {
            self.config.device_id = Config::fresh().device_id;
        }
        let url = jellyfin_api::normalise_base_url(&url);
        self.config.server_url = url.clone();
        self.step = Step::Idle;
        self.quick = Some(QuickLogin {
            server_url: url.clone(),
            server_name: String::new(),
            version: String::new(),
            code: String::new(),
            secret: String::new(),
            authenticated: false,
            exchanging: false,
            polling: false,
            poll_at: Instant::now(),
            error: None,
        });
        let device = self.config.device_id.clone();
        self.quick_start.request(url, move |url| {
            let client = Client::new(&url, device);
            let info = client.public_system_info().map_err(|error| Failure {
                stage: Stage::Server,
                message: error.user_message(),
            })?;
            let state = client.quick_connect_initiate().map_err(|error| Failure {
                stage: Stage::Credentials,
                message: error.user_message(),
            })?;
            Ok(QuickStart {
                server_name: info.server_name,
                version: info.version,
                state,
            })
        });
    }

    pub fn cancel_quick_connect(&mut self) {
        self.quick_start.cancel();
        self.quick_poll.cancel();
        self.quick_auth.cancel();
        self.quick = None;
        self.step = Step::Idle;
    }

    pub(super) fn apply_quick_start(&mut self, result: Result<QuickStart, Failure>) {
        let Some(quick) = &mut self.quick else {
            return;
        };
        match result {
            Ok(start) => {
                quick.server_name = start.server_name;
                quick.version = start.version;
                quick.code = start.state.code;
                quick.secret = start.state.secret;
                quick.error = None;
                quick.poll_at = Instant::now() + Duration::from_secs(1);
            }
            Err(failure) => quick.error = Some(failure.message),
        }
    }

    /// Called from `poll`: one in-flight poll at a time, every two seconds.
    pub(super) fn poll_quick(&mut self) -> bool {
        let now = Instant::now();
        let job = self.quick.as_ref().and_then(|quick| {
            (!quick.authenticated
                && !quick.polling
                && !quick.exchanging
                && !quick.secret.is_empty()
                && now >= quick.poll_at)
                .then(|| (quick.secret.clone(), quick.server_url.clone()))
        });
        let Some((secret, url)) = job else {
            return false;
        };
        if let Some(quick) = &mut self.quick {
            quick.polling = true;
            quick.poll_at = now + Duration::from_secs(2);
        }
        let device = self.config.device_id.clone();
        self.quick_poll.request(secret, move |secret| {
            let client = Client::new(&url, device);
            client.quick_connect_poll(&secret).map_err(|error| Failure {
                stage: Stage::Credentials,
                message: error.user_message(),
            })
        });
        true
    }

    pub(super) fn apply_quick_poll(&mut self, result: Result<QuickConnectState, Failure>) {
        let exchange = {
            let Some(quick) = &mut self.quick else {
                return;
            };
            quick.polling = false;
            match result {
                Ok(state) => {
                    quick.error = None;
                    if state.authenticated && !quick.exchanging {
                        quick.authenticated = true;
                        quick.exchanging = true;
                        Some((quick.secret.clone(), quick.server_url.clone()))
                    } else {
                        None
                    }
                }
                Err(failure) => {
                    quick.error = Some(failure.message);
                    quick.poll_at = Instant::now() + Duration::from_secs(3);
                    None
                }
            }
        };
        let Some((secret, url)) = exchange else {
            return;
        };
        let device = self.config.device_id.clone();
        self.quick_auth.request(secret, move |secret| {
            let client = Client::new(&url, device);
            client
                .authenticate_with_quick_connect(&secret)
                .map_err(|error| Failure {
                    stage: Stage::Credentials,
                    message: error.user_message(),
                })
        });
    }

    pub(super) fn apply_quick_auth(&mut self, result: Result<AuthResult, Failure>) {
        let session = {
            let Some(quick) = &mut self.quick else {
                return;
            };
            quick.exchanging = false;
            match result {
                Ok(auth) => Some(Session {
                    server_url: quick.server_url.clone(),
                    server_name: quick.server_name.clone(),
                    version: quick.version.clone(),
                    user_id: auth.user.id,
                    user_name: auth.user.name,
                    token: auth.access_token,
                }),
                Err(failure) => {
                    quick.error = Some(failure.message);
                    quick.authenticated = false;
                    quick.poll_at = Instant::now() + Duration::from_secs(3);
                    None
                }
            }
        };
        if let Some(session) = session {
            self.apply_session(session);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "needs a live Jellyfin server with Quick Connect enabled"]
    fn a_quick_connect_login_completes_against_the_live_server() {
        let live = Config::load();
        assert!(live.has_session(), "sign in once before this test");
        let approver = Client::new(&live.server_url, live.device_id.clone())
            .with_session(live.token.clone(), live.user_id.clone());
        if !approver.quick_connect_enabled().unwrap_or(false) {
            eprintln!("quick connect is disabled on this server");
            return;
        }
        let mut state = State::from_config(Config::fresh());
        state.server = TextEdit::new(live.server_url.clone());
        // A fresh device id on purpose: Quick Connect with the signed-in device's id can rotate
        // that device's token, which would sign the real app out.
        state.start_quick_connect();

        let deadline = Instant::now() + Duration::from_secs(20);
        let mut code = None;
        while Instant::now() < deadline {
            state.poll();
            if let Some(quick) = &state.quick
                && !quick.code.is_empty()
            {
                code = Some(quick.code.clone());
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let code = code.expect("the server sent no Quick Connect code");
        approver
            .quick_connect_authorize(&code)
            .expect("approve the code");

        while Instant::now() < deadline {
            state.poll();
            if state.is_signed_in() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        eprintln!(
            "quick connect app: signed in as {}, error={:?}",
            state.config.user_name,
            state.quick.as_ref().and_then(|quick| quick.error.clone())
        );
        assert!(state.is_signed_in(), "the app did not complete sign-in");
    }
}
