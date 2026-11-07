//! The HTTP side: one blocking [`Client`], no async runtime.
//!
//! The app runs every call on a worker (`rune::Latest` or `Worker::spawn`), so this module stays
//! plain functions returning `Result`. The agent keeps non-2xx responses as values, because the
//! Seerr error body is the useful part on a wrong key or a quota refusal.

use std::fmt;
use std::sync::OnceLock;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::models::*;

#[derive(Debug, Clone)]
pub struct Client {
    base: String,
    api_key: String,
    user_id: Option<i64>,
    agent: ureq::Agent,
}

#[derive(Debug)]
pub enum Error {
    /// The server answered with a non-2xx status. The body is kept: Seerr's `message` or `error`
    /// is what the UI should show.
    Status { code: u16, body: String },
    /// No connection, DNS, TLS or timeout.
    Transport(String),
    /// The body parsed as JSON but not as the expected type.
    Decode(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Status { code, body } => write!(f, "Seerr returned {code}: {body}"),
            Error::Transport(e) => write!(f, "could not reach Seerr: {e}"),
            Error::Decode(e) => write!(f, "unexpected answer from Seerr: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl Error {
    /// Worth one retry: the request never reached the server, or the server said it was unwell.
    pub fn is_transient(&self) -> bool {
        match self {
            Error::Transport(_) => true,
            Error::Status { code, .. } => *code >= 500,
            Error::Decode(_) => false,
        }
    }

    /// A missing credential (401). A wrong API key is 403 on this API and is handled by the probe.
    pub fn is_auth(&self) -> bool {
        matches!(self, Error::Status { code: 401, .. })
    }

    /// A short line safe to put on screen.
    pub fn user_message(&self) -> String {
        match self {
            Error::Status { code: 401, .. } => "Seerr needs an API key.".into(),
            Error::Status { code: 403, body } => {
                server_message(body).unwrap_or_else(|| "This Seerr user may not do that.".into())
            }
            Error::Status { code: 404, .. } => "Seerr does not know that request.".into(),
            Error::Status { code: 409, .. } => "Already requested.".into(),
            Error::Status { code: 429, .. } => "Seerr is busy; try again shortly.".into(),
            Error::Status { code, body } if *code >= 500 => {
                server_message(body).unwrap_or_else(|| "Seerr had a problem.".into())
            }
            Error::Status { code, .. } => format!("Seerr refused the request ({code})."),
            Error::Transport(_) => "Can't reach Seerr.".into(),
            Error::Decode(_) => "Seerr sent something unexpected.".into(),
        }
    }
}

type Result<T> = std::result::Result<T, Error>;

/// Normalises what people type: bare host, trailing slash, a stray path. A Seerr behind a reverse
/// proxy may live under a path, so the path is kept.
pub fn normalise_base_url(input: &str) -> String {
    let trimmed = input.trim();
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };
    with_scheme.trim_end_matches('/').to_string()
}

fn agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build(),
    )
}

impl Client {
    pub fn new(base_url: &str, api_key: impl Into<String>) -> Self {
        Self {
            base: normalise_base_url(base_url),
            api_key: api_key.into(),
            user_id: None,
            agent: agent(),
        }
    }

    /// `X-API-User` acts as that Seerr user. Optional; the key alone acts as the administrator.
    pub fn with_user(mut self, user_id: i64) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    fn user_agent(&self) -> String {
        format!("Runefin/{}", env!("CARGO_PKG_VERSION"))
    }

    fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = self.url(path);
        let mut request = self
            .agent
            .get(&url)
            .header("X-Api-Key", &self.api_key)
            .header("User-Agent", self.user_agent())
            .header("Accept", "application/json");
        if let Some(user_id) = self.user_id {
            request = request.header("X-API-User", &user_id.to_string());
        }
        let mut response = request.call().map_err(transport)?;
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|error| Error::Transport(error.to_string()))?;
        if !(200..300).contains(&status) {
            return Err(Error::Status { code: status, body });
        }
        serde_json::from_str(&body).map_err(|error| Error::Decode(format!("{error} in {url}")))
    }

    fn post_json<B: Serialize, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
        let url = self.url(path);
        let json = serde_json::to_string(body).map_err(|error| Error::Decode(error.to_string()))?;
        let mut request = self
            .agent
            .post(&url)
            .header("X-Api-Key", &self.api_key)
            .header("User-Agent", self.user_agent())
            .header("Accept", "application/json")
            .header("Content-Type", "application/json");
        if let Some(user_id) = self.user_id {
            request = request.header("X-API-User", &user_id.to_string());
        }
        let mut response = request.send(json.as_str()).map_err(transport)?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|error| Error::Transport(error.to_string()))?;
        if !(200..300).contains(&status) {
            return Err(Error::Status {
                code: status,
                body: text,
            });
        }
        serde_json::from_str(&text).map_err(|error| Error::Decode(format!("{error} in {url}")))
    }

    fn delete(&self, path: &str) -> Result<()> {
        let url = self.url(path);
        let mut request = self
            .agent
            .delete(&url)
            .header("X-Api-Key", &self.api_key)
            .header("User-Agent", self.user_agent())
            .header("Accept", "application/json");
        if let Some(user_id) = self.user_id {
            request = request.header("X-API-User", &user_id.to_string());
        }
        let mut response = request.call().map_err(transport)?;
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|error| Error::Transport(error.to_string()))?;
        if !(200..300).contains(&status) {
            return Err(Error::Status { code: status, body });
        }
        Ok(())
    }

    // ---- probe ---------------------------------------------------------------------------

    /// `GET /status`. Public; `checkUpdateAvailable=false` avoids a needless upstream call.
    pub fn status(&self) -> Result<Status> {
        self.get_json("/api/v1/status?checkUpdateAvailable=false")
    }

    /// `GET /settings/public`. Public; tells the client whether setup is finished and which
    /// request features the server allows.
    pub fn public_settings(&self) -> Result<PublicSettings> {
        self.get_json("/api/v1/settings/public")
    }

    /// `GET /auth/me`. A wrong key is 403 on this API; the Settings probe maps that to a key error.
    pub fn auth_me(&self) -> Result<User> {
        self.get_json("/api/v1/auth/me")
    }

    // ---- catalogue -----------------------------------------------------------------------

    pub fn search(&self, query: &str, page: u32) -> Result<SearchResults> {
        self.get_json(&format!(
            "/api/v1/search?query={}&page={page}",
            urlencode(query)
        ))
    }

    pub fn discover_trending(&self, page: u32) -> Result<SearchResults> {
        self.get_json(&format!(
            "/api/v1/discover/trending?mediaType=all&timeWindow=week&page={page}"
        ))
    }

    pub fn discover_movies(&self, page: u32) -> Result<SearchResults> {
        self.get_json(&format!("/api/v1/discover/movies?page={page}"))
    }

    pub fn discover_tv(&self, page: u32) -> Result<SearchResults> {
        self.get_json(&format!("/api/v1/discover/tv?page={page}"))
    }

    /// A filtered catalogue page. The accepted filters are the ones the live instance takes:
    /// `genre`, `studio` (movies), `network` (series), `sortBy`, a release-date window and
    /// `language`; unknown filters are a 400, so this builds only the ones we know.
    pub fn discover_movies_filtered(&self, query: &DiscoverQuery) -> Result<SearchResults> {
        self.get_json(&format!(
            "/api/v1/discover/movies?{}",
            query.query_string("primaryReleaseDate")
        ))
    }

    pub fn discover_tv_filtered(&self, query: &DiscoverQuery) -> Result<SearchResults> {
        self.get_json(&format!(
            "/api/v1/discover/tv?{}",
            query.query_string("firstAirDate")
        ))
    }

    /// TMDB's genre list for a media type (`movie` or `tv`).
    pub fn genres(&self, media_type: &str) -> Result<Vec<Genre>> {
        self.get_json(&format!("/api/v1/genres/{media_type}"))
    }

    pub fn movie(&self, tmdb_id: u64) -> Result<MovieDetails> {
        self.get_json(&format!("/api/v1/movie/{tmdb_id}"))
    }

    pub fn tv(&self, tmdb_id: u64) -> Result<TvDetails> {
        self.get_json(&format!("/api/v1/tv/{tmdb_id}"))
    }

    /// The person's biography and facts. The Seerr route is a TMDB person fetch; it does not
    /// include credits, which come from [`Client::person_credits`].
    pub fn person(&self, tmdb_id: u64) -> Result<PersonDetails> {
        self.get_json(&format!("/api/v1/person/{tmdb_id}"))
    }

    /// Every movie and series the person is credited in, cast and crew.
    pub fn person_credits(&self, tmdb_id: u64) -> Result<PersonCredits> {
        self.get_json(&format!("/api/v1/person/{tmdb_id}/combined_credits"))
    }

    // ---- requests ------------------------------------------------------------------------

    /// `GET /request`. `filter` is `all`, `pending`, `processing`, `available`, …
    pub fn requests(&self, filter: &str, skip: u32, take: u32) -> Result<Paginated<MediaRequest>> {
        self.get_json(&format!(
            "/api/v1/request?take={take}&skip={skip}&filter={}&sort=modified&sortDirection=desc",
            urlencode(filter)
        ))
    }

    pub fn request_count(&self) -> Result<RequestCounts> {
        self.get_json("/api/v1/request/count")
    }

    pub fn create_request(&self, request: &CreateRequest) -> Result<MediaRequest> {
        self.post_json("/api/v1/request", request)
    }

    pub fn delete_request(&self, request_id: u64) -> Result<()> {
        self.delete(&format!("/api/v1/request/{request_id}"))
    }
}

fn transport(error: ureq::Error) -> Error {
    Error::Transport(error.to_string())
}

fn server_message(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    value
        .get("message")
        .or_else(|| value.get("error"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

/// Percent-encodes everything that is not unreserved, so titles and queries survive the URL.
fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

static IMAGE_AGENT: OnceLock<ureq::Agent> = OnceLock::new();

/// Fetch a TMDB image. Seerr does not proxy artwork, so this is a plain CDN request with no
/// credential. The app calls it on its image worker.
pub fn fetch_image(url: &str) -> std::result::Result<Vec<u8>, String> {
    let agent = IMAGE_AGENT.get_or_init(agent);
    let mut response = agent
        .get(url)
        .header(
            "User-Agent",
            format!("Runefin/{}", env!("CARGO_PKG_VERSION")),
        )
        .header("Accept", "image/*,*/*")
        .call()
        .map_err(|error| error.to_string())?;
    let status = response.status().as_u16();
    let bytes = response
        .body_mut()
        .read_to_vec()
        .map_err(|error| error.to_string())?;
    if !(200..300).contains(&status) {
        return Err(format!("image returned {status}"));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
