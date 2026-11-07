//! The HTTP side: one [`Client`], blocking, no async runtime.
//!
//! Every call is a plain function that returns `Result<T>`; the app runs them on a worker
//! thread (`rune::Latest`) so the UI thread never blocks. That is the whole concurrency story —
//! there is no `tokio` in this crate and there does not need to be one.

use std::fmt;

use crate::models::*;

/// Everything a request needs: where the server is, and who is asking.
#[derive(Debug, Clone)]
pub struct Client {
    base: String,
    token: Option<String>,
    user_id: Option<String>,
    device_id: String,
    app_version: &'static str,
    /// One pool for every request this client makes, including its clones. Reusing the TLS
    /// connection is what makes a screenful of posters cost one handshake, not thirty.
    agent: ureq::Agent,
}

#[derive(Debug)]
pub enum Error {
    /// The server answered, with a status that is not 2xx.
    Status { code: u16, body: String },
    /// No connection, DNS, TLS or timeout.
    Transport(String),
    /// The body parsed as JSON but not as the expected type.
    Decode(String),
}

impl Error {
    /// Worth one more try: the request never reached the server, or the server said it was
    /// temporarily unwell. Configuration and auth errors are not.
    pub fn is_transient(&self) -> bool {
        match self {
            Error::Transport(_) => true,
            Error::Status { code, .. } => *code >= 500,
            Error::Decode(_) => false,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Status { code, body } => {
                write!(f, "server returned {code}: {}", redact(body))
            }
            Error::Transport(e) => write!(f, "could not reach the server: {}", redact(e)),
            Error::Decode(e) => {
                write!(f, "unexpected answer from the server: {}", redact(e))
            }
        }
    }
}

/// Remove credentials from a message before it reaches a screen, a log or a bug report.
///
/// Transport errors quote the request URL, and Jellyfin's image and stream URLs carry the
/// session token as `api_key`. The token also appears as `token`, `ApiKey` and `X-Emby-Token`
/// in a few shapes, so every query value whose name mentions a key or a token is replaced.
#[must_use]
pub fn redact(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    while let Some(index) = find_secret_param(rest) {
        let (before, after) = rest.split_at(index);
        out.push_str(before);
        let name_end = after.find('=').map_or(after.len(), |at| at + 1);
        let (name, tail) = after.split_at(name_end);
        out.push_str(name);
        let stop = tail
            .find(|c: char| c == '&' || c.is_whitespace() || c == '"' || c == '\'')
            .unwrap_or(tail.len());
        out.push_str("<redacted>");
        rest = &tail[stop..];
    }
    out.push_str(rest);
    out
}

/// The byte offset of the next `name=` whose name looks like a credential.
fn find_secret_param(value: &str) -> Option<usize> {
    const NAMES: [&str; 4] = ["api_key=", "apikey=", "token=", "x-emby-token="];
    let lower = value.to_ascii_lowercase();
    NAMES.iter().filter_map(|name| lower.find(name)).min()
}

impl std::error::Error for Error {}

/// What a failing HTTP call means for the UI: the connect screen says one thing, a dead session
/// another. Kept here so every screen agrees.
impl Error {
    pub fn needs_reconnect(&self) -> bool {
        matches!(self, Error::Status { code: 401, .. })
    }

    /// A short line safe to put on screen.
    pub fn user_message(&self) -> String {
        match self {
            Error::Status { code: 401, .. } => "Your session expired. Sign in again.".into(),
            Error::Status { code: 403, .. } => "This user may not do that.".into(),
            Error::Status { code, .. } => format!("The server refused the request ({code})."),
            Error::Transport(_) => "Can't reach the server.".into(),
            Error::Decode(_) => "The server sent something unexpected.".into(),
        }
    }
}

type Result<T> = std::result::Result<T, Error>;

/// Normalises what people type into a server field: bare host, trailing slash, a stray path.
pub fn normalise_base_url(input: &str) -> String {
    let trimmed = input.trim();
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        // A LAN Jellyfin is http far more often than https, and a wrong https guess fails
        // with a TLS error that reads like a bug. Port 8096 and .local hosts stay http.
        format!("http://{trimmed}")
    };
    with_scheme.trim_end_matches('/').to_string()
}

impl Client {
    pub fn new(base_url: &str, device_id: impl Into<String>) -> Self {
        Self {
            base: normalise_base_url(base_url),
            token: None,
            user_id: None,
            device_id: device_id.into(),
            app_version: env!("CARGO_PKG_VERSION"),
            agent: ureq::Agent::new_with_defaults(),
        }
    }

    pub fn with_session(mut self, token: impl Into<String>, user_id: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self.user_id = Some(user_id.into());
        self
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    pub fn user_id(&self) -> Option<&str> {
        self.user_id.as_deref()
    }

    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    fn user(&self) -> Result<&str> {
        self.user_id
            .as_deref()
            .ok_or_else(|| Error::Decode("no user in this client".into()))
    }

    /// `MediaBrowser Client="…", Device="…", DeviceId="…", Version="…", Token="…"`.
    /// Jellyfin parses this as a comma-separated list of quoted pairs, so values are escaped.
    fn authorization(&self) -> String {
        let mut s = format!(
            "MediaBrowser Client=\"Runefin\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\"",
            escape_header(&device_name()),
            escape_header(&self.device_id),
            self.app_version,
        );
        if let Some(token) = &self.token {
            s.push_str(&format!(", Token=\"{}\"", escape_header(token)));
        }
        s
    }

    /// Some Jellyfin reverse proxies reject a request with no `User-Agent` before the server
    /// sees the token. A normal client identity is both correct and enough to pass them.
    fn user_agent(&self) -> String {
        format!("Runefin/{}", self.app_version)
    }

    /// Idempotent reads retry once on a transport error or a 5xx. A 4xx is an answer, not a
    /// transient failure, so it is returned at once. The sleep is on the worker thread.
    fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        match self.get_once(path) {
            Ok(value) => Ok(value),
            Err(error) if error.is_transient() => {
                std::thread::sleep(std::time::Duration::from_millis(150));
                self.get_once(path)
            }
            Err(error) => Err(error),
        }
    }

    fn get_once<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base, path);
        let mut req = self
            .agent
            .get(&url)
            .header("Authorization", &self.authorization())
            .header("User-Agent", self.user_agent())
            .header("Accept", "application/json");
        if let Some(token) = &self.token {
            req = req.header("X-Emby-Token", token);
        }
        let mut res = req.call().map_err(map_err)?;
        let body = res
            .body_mut()
            .read_to_string()
            .map_err(|e| Error::Transport(e.to_string()))?;
        serde_json::from_str(&body).map_err(|e| Error::Decode(format!("{e} in {url}")))
    }

    fn post<B: serde::Serialize, T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.base, path);
        let json = serde_json::to_string(body).map_err(|e| Error::Decode(e.to_string()))?;
        let mut req = self
            .agent
            .post(&url)
            .header("Authorization", &self.authorization())
            .header("User-Agent", self.user_agent())
            .header("Accept", "application/json")
            .header("Content-Type", "application/json");
        if let Some(token) = &self.token {
            req = req.header("X-Emby-Token", token);
        }
        let mut res = req.send(json.as_str()).map_err(map_err)?;
        let text = res
            .body_mut()
            .read_to_string()
            .map_err(|e| Error::Transport(e.to_string()))?;
        serde_json::from_str(&text).map_err(|e| Error::Decode(format!("{e} in {url}")))
    }

    fn post_empty<B: serde::Serialize>(&self, path: &str, body: &B) -> Result<()> {
        let url = format!("{}{}", self.base, path);
        let json = serde_json::to_string(body).map_err(|e| Error::Decode(e.to_string()))?;
        let mut req = self
            .agent
            .post(&url)
            .header("Authorization", &self.authorization())
            .header("User-Agent", self.user_agent())
            .header("Accept", "application/json")
            .header("Content-Type", "application/json");
        if let Some(token) = &self.token {
            req = req.header("X-Emby-Token", token);
        }
        req.send(json.as_str()).map_err(map_err)?;
        Ok(())
    }

    fn post_nothing(&self, path: &str) -> Result<()> {
        let url = format!("{}{}", self.base, path);
        let mut req = self
            .agent
            .post(&url)
            .header("Authorization", &self.authorization())
            .header("User-Agent", self.user_agent())
            .header("Accept", "application/json");
        if let Some(token) = &self.token {
            req = req.header("X-Emby-Token", token);
        }
        req.send_empty().map_err(map_err)?;
        Ok(())
    }

    // ---- pre-login --------------------------------------------------------------------

    /// `GET /System/Info/Public`. Also the cheapest "is this a Jellyfin server" probe.
    pub fn public_system_info(&self) -> Result<PublicSystemInfo> {
        self.get("/System/Info/Public")
    }

    /// `POST /Users/AuthenticateByName`. The result carries the token to keep.
    pub fn authenticate(&self, username: &str, password: &str) -> Result<AuthResult> {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "PascalCase")]
        struct Body<'a> {
            username: &'a str,
            pw: &'a str,
        }
        self.post(
            "/Users/AuthenticateByName",
            &Body {
                username,
                pw: password,
            },
        )
    }

    /// `GET /QuickConnect/Enabled`: whether the server's admin turned the feature on.
    pub fn quick_connect_enabled(&self) -> Result<bool> {
        self.get("/QuickConnect/Enabled")
    }

    /// `POST /QuickConnect/Authorize`: approve a code from an already signed-in session. The
    /// app does not use this during sign-in; it exists for tests and for a future "approve on
    /// this device" action.
    pub fn quick_connect_authorize(&self, code: &str) -> Result<()> {
        self.post_empty(
            &format!(
                "/QuickConnect/Authorize?code={}&userId={}",
                urlencode(code),
                urlencode(self.user()?)
            ),
            &serde_json::json!({}),
        )
    }

    /// `POST /QuickConnect/Initiate`: start a code-based login (no password in the app).
    pub fn quick_connect_initiate(&self) -> Result<QuickConnectState> {
        self.post(
            "/QuickConnect/Initiate",
            &serde_json::json!({ "AppName": "Runefin" }),
        )
    }

    /// `GET /QuickConnect/Connect?secret=…`: the poll. `authenticated` flips once the user has
    /// entered the code on the server. Poll about once a second; the answer carries no token.
    pub fn quick_connect_poll(&self, secret: &str) -> Result<QuickConnectState> {
        self.get(&format!(
            "/QuickConnect/Connect?secret={}",
            urlencode(secret)
        ))
    }

    /// `POST /Users/AuthenticateWithQuickConnect`: the step after `authenticated` flips. This is
    /// where the token arrives.
    pub fn authenticate_with_quick_connect(&self, secret: &str) -> Result<AuthResult> {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "PascalCase")]
        struct Body<'a> {
            secret: &'a str,
        }
        self.post("/Users/AuthenticateWithQuickConnect", &Body { secret })
    }

    // ---- browsing --------------------------------------------------------------------

    /// `GET /Users/{id}/Views`: the libraries the home screen lists.
    pub fn libraries(&self) -> Result<Vec<LibraryView>> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "PascalCase")]
        struct Views {
            #[serde(default)]
            items: Vec<LibraryView>,
        }
        let v: Views = self.get(&format!("/UserViews?userId={}", urlencode(self.user()?)))?;
        Ok(v.items)
    }

    /// `GET /Users/{id}/Items` with the filters a browse screen actually uses.
    pub fn items(&self, query: &ItemQuery) -> Result<ItemsResult> {
        let path = format!(
            "/Items?userId={}&{}",
            urlencode(self.user()?),
            query.to_query()
        );
        self.get(&path)
    }

    /// `POST /Items/{id}/PlaybackInfo`: the direct-play-first negotiation.
    pub fn playback_info(
        &self,
        item_id: &str,
        request: &PlaybackInfoRequest,
    ) -> Result<PlaybackInfoResponse> {
        self.post(
            &format!("/Items/{}/PlaybackInfo", urlencode(item_id)),
            request,
        )
    }

    /// The profile this build sends. It describes the GStreamer plugins the player already
    /// needs: Matroska/MP4/AVI, H.264/HEVC/AV1/VP9, and the usual audio codecs. HDR is left to
    /// the sink, so HDR files direct-play when the decoder advertises them.
    pub fn device_profile() -> serde_json::Value {
        serde_json::json!({
            "Name": "Runefin",
            "MaxStreamingBitrate": 120_000_000,
            "MaxStaticBitrate": 120_000_000,
            "DirectPlayProfiles": [
                {
                    "Type": "Video",
                    "Container": "mkv,mp4,m4v,avi,webm,ts,mpegts",
                    "VideoCodec": "h264,hevc,av1,vp9,mpeg4",
                    "AudioCodec": "aac,ac3,eac3,dts,truehd,flac,opus,vorbis,mp3"
                },
                {
                    "Type": "Audio",
                    "Container": "mp3,flac,aac,m4a,ogg,opus,wav",
                    "AudioCodec": "mp3,aac,flac,opus,vorbis"
                }
            ],
            "TranscodingProfiles": [
                {
                    "Type": "Video",
                    "Container": "ts",
                    "Protocol": "hls",
                    "Context": "Streaming",
                    "VideoCodec": "h264",
                    "AudioCodec": "aac",
                    "MaxAudioChannels": "2"
                },
                {
                    "Type": "Audio",
                    "Container": "mp3",
                    "Protocol": "http",
                    "Context": "Streaming",
                    "AudioCodec": "mp3",
                    "MaxAudioChannels": "2"
                }
            ],
            "CodecProfiles": [
                {
                    "Type": "Video",
                    "Codec": "h264",
                    "Conditions": [
                        { "Condition": "LessThanEqual", "Property": "VideoBitDepth", "Value": "10", "IsRequired": false }
                    ]
                },
                {
                    "Type": "Video",
                    "Codec": "hevc",
                    "Conditions": [
                        { "Condition": "LessThanEqual", "Property": "VideoBitDepth", "Value": "10", "IsRequired": false }
                    ]
                }
            ],
            "SubtitleProfiles": [
                { "Format": "srt", "Method": "External" },
                { "Format": "subrip", "Method": "External" },
                { "Format": "ass", "Method": "External" },
                { "Format": "ssa", "Method": "External" },
                { "Format": "pgssub", "Method": "Encode" },
                { "Format": "dvdsub", "Method": "Encode" },
                { "Format": "dvbsub", "Method": "Encode" }
            ]
        })
    }

    /// Prefix a server-relative URL with this client's base. Absolute URLs pass through.
    pub fn absolute_url(&self, path: &str) -> String {
        if path.starts_with("http://") || path.starts_with("https://") {
            path.to_string()
        } else {
            format!("{}{}", self.base, path)
        }
    }

    /// A server URL GStreamer can fetch without our request headers: the token becomes an
    /// `api_key` query parameter when the server did not already put one there.
    pub fn authed_url(&self, path: &str) -> String {
        let mut url = self.absolute_url(path);
        if let Some(token) = &self.token
            && !url.contains("api_key=")
            && !url.contains("X-Emby-Token=")
        {
            url.push(if url.contains('?') { '&' } else { '?' });
            url.push_str("api_key=");
            url.push_str(&urlencode(token));
        }
        url
    }

    /// `/Search/Hints`: names matching a partial query, including people, genres and studios
    /// that `/Items?searchTerm=` does not return as first-class rows.
    pub fn search_hints(&self, term: &str, limit: u32) -> Result<Vec<crate::SearchHint>> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "PascalCase")]
        struct Hints {
            #[serde(default)]
            search_hints: Vec<crate::SearchHint>,
        }
        let hints: Hints = self.get(&format!(
            "/Search/Hints?userId={}&searchTerm={}&limit={limit}",
            urlencode(self.user()?),
            urlencode(term)
        ))?;
        Ok(hints.search_hints)
    }

    /// `GET /Shows/Upcoming`: episodes and films the server expects next, by air date.
    pub fn upcoming(&self, limit: u32) -> Result<Vec<Item>> {
        let path = format!(
            "/Shows/Upcoming?userId={}&limit={limit}&fields=UserData",
            urlencode(self.user()?)
        );
        Ok(self.get::<ItemsResult>(&path)?.items)
    }

    /// `POST /Collections`: create a server-owned collection. Jellyfin 12.4 takes `name` and
    /// `ids` as query parameters (the older JSON body returns 500); the answer carries the new
    /// id, which this client does not need.
    pub fn create_collection(&self, name: &str, ids: &[String]) -> Result<()> {
        let joined = ids
            .iter()
            .map(|id| urlencode(id))
            .collect::<Vec<_>>()
            .join("%2C");
        self.post_nothing(&format!(
            "/Collections?name={}&ids={joined}",
            urlencode(name)
        ))
    }

    /// `POST /Collections/{id}/Items?ids=…`: add existing items to an existing box set.
    pub fn add_to_collection(&self, collection_id: &str, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let joined = ids
            .iter()
            .map(|id| urlencode(id))
            .collect::<Vec<_>>()
            .join("%2C");
        self.post_nothing(&format!(
            "/Collections/{}/Items?ids={joined}",
            urlencode(collection_id)
        ))
    }

    /// `DELETE /Collections/{id}/Items?ids=…`.
    pub fn remove_from_collection(&self, collection_id: &str, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let joined = ids
            .iter()
            .map(|id| urlencode(id))
            .collect::<Vec<_>>()
            .join("%2C");
        let url = format!(
            "{}/Collections/{}/Items?ids={joined}",
            self.base,
            urlencode(collection_id)
        );
        let mut req = self
            .agent
            .delete(&url)
            .header("Authorization", &self.authorization())
            .header("User-Agent", self.user_agent())
            .header("Accept", "application/json");
        if let Some(token) = &self.token {
            req = req.header("X-Emby-Token", token);
        }
        req.call().map_err(map_err)?;
        Ok(())
    }

    /// The collections an item belongs to.
    pub fn item_collections(&self, item_id: &str) -> Result<Vec<Item>> {
        let path = format!(
            "/Items/{}/Collections?userId={}",
            urlencode(item_id),
            urlencode(self.user()?)
        );
        Ok(self.get::<ItemsResult>(&path)?.items)
    }

    /// A film's special features (behind the scenes, deleted scenes, interviews, …).
    pub fn special_features(&self, item_id: &str) -> Result<Vec<Item>> {
        let path = format!(
            "/Items/{}/SpecialFeatures?userId={}",
            urlencode(item_id),
            urlencode(self.user()?)
        );
        let container: crate::ItemsContainer = self.get(&path)?;
        Ok(container.items)
    }

    /// Theme songs and theme videos attached to an item or inherited from its parent.
    pub fn theme_media(&self, item_id: &str) -> Result<crate::ThemeMedia> {
        self.get(&format!(
            "/Items/{}/ThemeMedia?userId={}&inheritFromParent=true",
            urlencode(item_id),
            urlencode(self.user()?)
        ))
    }

    /// `GET /Users/{id}/Items/{item}`, with the full user state and the cast the detail page
    /// needs. `People` is the one field a detail request should not omit.
    pub fn item(&self, item_id: &str) -> Result<Item> {
        self.get(&format!(
            "/Items/{}?userId={}&fields=People,Chapters,Trickplay,MediaSources",
            urlencode(item_id),
            urlencode(self.user()?)
        ))
    }

    /// `GET /Users/{id}/Items/Resume`: what the home screen shows as Continue Watching.
    pub fn resume(&self, limit: u32) -> Result<ItemsResult> {
        self.get(&format!(
            "/UserItems/Resume?userId={}&limit={limit}&mediaTypes=Video&fields=UserData",
            urlencode(self.user()?)
        ))
    }

    /// `GET /LiveTv/Channels`: the tuner's channels, with their current program when the
    /// guide has one.
    pub fn live_channels(&self, limit: u32) -> Result<Vec<Item>> {
        let path = format!(
            "/LiveTv/Channels?userId={}&limit={limit}&enableImages=true",
            urlencode(self.user()?)
        );
        Ok(self.get::<ItemsResult>(&path)?.items)
    }

    /// `GET /LiveTv/Programs`: the current and future guide entries for one channel. The first
    /// entry is usually the program already on air, so callers compare start dates.
    pub fn live_programs(&self, channel_id: &str, limit: u32) -> Result<Vec<Item>> {
        let path = format!(
            "/LiveTv/Programs?userId={}&channelIds={}&hasAired=false&limit={limit}&sortBy=StartDate&fields=Overview",
            urlencode(self.user()?),
            urlencode(channel_id)
        );
        Ok(self.get::<ItemsResult>(&path)?.items)
    }

    /// The guide for several channels over one time window. One request covers the whole grid,
    /// which keeps a guide view from firing a call per channel.
    pub fn live_guide(&self, channel_ids: &[String], start: i64, end: i64) -> Result<Vec<Item>> {
        let path = format!(
            "/LiveTv/Programs?userId={}&channelIds={}&minStartDate={}&maxStartDate={}&limit=500&sortBy=StartDate&fields=Overview",
            urlencode(self.user()?),
            urlencode(&channel_ids.join(",")),
            urlencode(&iso_from_seconds(start)),
            urlencode(&iso_from_seconds(end))
        );
        Ok(self.get::<ItemsResult>(&path)?.items)
    }

    /// Open a channel for playback. Jellyfin splits this in two: `PlaybackInfo` hands out an
    /// open token, then `/LiveStreams/Open` starts the tuner and returns the URL and the id
    /// that must be closed when the viewer leaves.
    pub fn open_live_stream(&self, item_id: &str) -> Result<LiveStream> {
        let info: PlaybackInfo = self.get(&format!(
            "/Items/{}/PlaybackInfo?userId={}",
            urlencode(item_id),
            urlencode(self.user()?)
        ))?;
        let source = info
            .media_sources
            .first()
            .ok_or_else(|| Error::Decode("the channel has no media source".into()))?;
        let open_token = source
            .open_token
            .as_deref()
            .ok_or_else(|| Error::Decode("the channel has no open token".into()))?;
        let play_session_id = info.play_session_id.clone().unwrap_or_default();
        let path = format!(
            "/LiveStreams/Open?openToken={}&userId={}&playSessionId={}&itemId={}",
            urlencode(open_token),
            urlencode(self.user()?),
            urlencode(&play_session_id),
            urlencode(item_id)
        );
        let opened: OpenLiveStream = self.post(&path, &serde_json::json!({}))?;
        let source = opened
            .media_source
            .ok_or_else(|| Error::Decode("the server opened no live stream".into()))?;
        let url = source
            .path
            .ok_or_else(|| Error::Decode("the opened live stream has no URL".into()))?;
        Ok(LiveStream {
            url,
            live_stream_id: source.live_stream_id,
            play_session_id: (!play_session_id.is_empty()).then_some(play_session_id),
        })
    }

    /// `POST /LiveStreams/Close`: release the tuner. Every stop path must call this.
    pub fn close_live_stream(&self, live_stream_id: &str) -> Result<()> {
        self.post_empty(
            &format!(
                "/LiveStreams/Close?liveStreamId={}",
                urlencode(live_stream_id)
            ),
            &serde_json::json!({}),
        )
    }

    /// Box sets across every library, alphabetically, for the home Collections rail.
    pub fn collections(&self, limit: u32) -> Result<Vec<Item>> {
        let path = format!(
            "/Items?userId={}&recursive=true&includeItemTypes=BoxSet&sortBy=SortName&limit={limit}&fields=UserData",
            urlencode(self.user()?)
        );
        Ok(self.get::<ItemsResult>(&path)?.items)
    }

    /// `GET /Genres`: the genre names in a library, alphabetically.
    pub fn genres(&self, parent_id: Option<&str>) -> Result<Vec<Item>> {
        let mut path = format!("/Genres?userId={}&sortBy=SortName", urlencode(self.user()?));
        if let Some(parent) = parent_id {
            path.push_str(&format!("&parentId={}", urlencode(parent)));
        }
        Ok(self.get::<ItemsResult>(&path)?.items)
    }

    /// The movies and shows a person appears in. `personIds` is the server's own credit query,
    /// not a name search, so it includes roles the index does not put in the text field.
    pub fn person_items(&self, person_id: &str, limit: u32) -> Result<Vec<Item>> {
        let path = format!(
            "/Items?userId={}&personIds={}&recursive=true&includeItemTypes=Movie,Series&fields=UserData,ProviderIds&sortBy=SortName&limit={limit}",
            urlencode(self.user()?),
            urlencode(person_id)
        );
        Ok(self.get::<ItemsResult>(&path)?.items)
    }

    /// Find a movie or series by its TMDB id. Jellyfin 12.4 has no provider-id filter on
    /// `/Items`, so this searches by title and confirms `ProviderIds.Tmdb` before answering.
    /// The check is what makes a same-named remake safe.
    pub fn find_by_tmdb(
        &self,
        title: &str,
        media_type: &str,
        tmdb_id: u64,
    ) -> Result<Option<Item>> {
        let kind = if media_type == "tv" {
            "Series"
        } else {
            "Movie"
        };
        let path = format!(
            "/Items?userId={}&recursive=true&includeItemTypes={kind}&searchTerm={}&fields=ProviderIds&limit=10",
            urlencode(self.user()?),
            urlencode(title)
        );
        Ok(self
            .get::<ItemsResult>(&path)?
            .items
            .into_iter()
            .find(|item| {
                item.provider_ids
                    .get("Tmdb")
                    .and_then(|id| id.parse::<u64>().ok())
                    == Some(tmdb_id)
            }))
    }

    /// The person item whose name matches, with its provider ids. The credits query needs the
    /// server's person id; a TMDB id alone cannot address it, so a Seerr person resolves through
    /// this search and the caller compares `ProviderIds.Tmdb`.
    pub fn find_person(&self, name: &str) -> Result<Option<Item>> {
        let path = format!(
            "/Items?userId={}&includeItemTypes=Person&recursive=true&searchTerm={}&fields=ProviderIds&limit=5",
            urlencode(self.user()?),
            urlencode(name)
        );
        Ok(self.get::<ItemsResult>(&path)?.items.into_iter().next())
    }

    /// The item's own trailers, when the server has local files for them.
    pub fn local_trailers(&self, item_id: &str) -> Result<Vec<Item>> {
        self.get(&format!(
            "/Items/{}/LocalTrailers?userId={}",
            urlencode(item_id),
            urlencode(self.user()?)
        ))
    }

    /// `GET /Shows/NextUp` across every series, for the home rail.
    pub fn next_up_all(&self, limit: u32) -> Result<Vec<Item>> {
        let path = format!(
            "/Shows/NextUp?userId={}&limit={limit}&fields=UserData",
            urlencode(self.user()?)
        );
        Ok(self.get::<ItemsResult>(&path)?.items)
    }

    /// `GET /Shows/NextUp`: the first episode after the highest played one. A series with no
    /// next episode is a normal empty answer, not an error.
    pub fn next_up(&self, series_id: &str, limit: u32) -> Result<Option<Item>> {
        let path = format!(
            "/Shows/NextUp?userId={}&seriesId={}&limit={limit}&fields=UserData",
            urlencode(self.user()?),
            urlencode(series_id)
        );
        let result: ItemsResult = self.get(&path)?;
        Ok(result.items.into_iter().next())
    }

    /// `GET /Users/{id}/Items/Latest`: recently added, optionally inside one library.
    pub fn latest(&self, parent_id: Option<&str>, limit: u32) -> Result<Vec<Item>> {
        let mut path = format!(
            "/Items/Latest?userId={}&limit={limit}&fields=UserData",
            urlencode(self.user()?)
        );
        if let Some(parent) = parent_id {
            path.push_str(&format!("&parentId={}", urlencode(parent)));
        }
        self.get(&path)
    }

    /// `GET /Items/{id}/Images/{kind}`. `tag` comes from the item so the URL is immutable.
    pub fn image_url(
        &self,
        item_id: &str,
        kind: ImageKind,
        max_width: u32,
        tag: Option<&str>,
        token: Option<&str>,
    ) -> String {
        let mut url = format!(
            "{}/Items/{}/Images/{}?maxWidth={max_width}&quality=90",
            self.base,
            urlencode(item_id),
            kind.as_str()
        );
        if let Some(tag) = tag {
            url.push_str(&format!("&tag={}", urlencode(tag)));
        }
        // The web client uses api_key for images because they load without custom headers.
        let token = token.or(self.token.as_deref());
        if let Some(token) = token {
            url.push_str(&format!("&api_key={}", urlencode(token)));
        }
        url
    }

    /// The item with its media sources, for the player's audio/subtitle stream list. Kept
    /// separate from [`Client::item`] so the detail page does not pay for streams it never reads.
    /// Trickplay and chapters ride along: playback can start from a card whose `/Items` row did
    /// not ask for either, and the timeline's preview needs the manifest before the first hover.
    pub fn playback_item(&self, item_id: &str) -> Result<Item> {
        self.get(&format!(
            "/Items/{}?userId={}&fields=MediaSources,Trickplay,Chapters",
            urlencode(item_id),
            urlencode(self.user()?)
        ))
    }

    /// One subtitle track, converted to SRT by the server. Jellyfin serves external and
    /// embedded text subtitles through the same endpoint, so the player needs one parser.
    pub fn subtitle_url(&self, item_id: &str, media_source_id: &str, index: i32) -> Option<String> {
        self.subtitle_url_with(item_id, media_source_id, index, "srt")
    }

    /// The same track in a named format. The server keeps ASS tracks as ASS (`format=ass`), and
    /// converts embedded text tracks to SRT (`format=srt`); the player parses whichever it asks
    /// for. Image tracks have no text format and are never requested here.
    pub fn subtitle_url_with(
        &self,
        item_id: &str,
        media_source_id: &str,
        index: i32,
        format: &str,
    ) -> Option<String> {
        let token = self.token.as_deref()?;
        Some(format!(
            "{}/Videos/{}/{}/Subtitles/{index}/Stream.{format}?api_key={}",
            self.base,
            urlencode(item_id),
            urlencode(media_source_id),
            urlencode(token)
        ))
    }

    /// A trickplay tile URL. The server generates tiles at the widths it advertised in the
    /// item's `Trickplay` manifest; `index` is the tile, not the thumbnail.
    pub fn trickplay_url(&self, item_id: &str, width: u32, index: u32) -> Option<String> {
        let token = self.token.as_deref()?;
        Some(format!(
            "{}/Videos/{}/Trickplay/{width}/{index}.jpg?api_key={}",
            self.base,
            urlencode(item_id),
            urlencode(token)
        ))
    }

    /// Fetch an image the server already gave us a URL for. The URL carries its own
    /// `api_key`, so this does not need the session header; the app's image worker calls it.
    pub fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let mut res = self
            .agent
            .get(url)
            .header("User-Agent", self.user_agent())
            .header("Accept", "image/*,*/*")
            .call()
            .map_err(map_err)?;
        res.body_mut()
            .read_to_vec()
            .map_err(|e| Error::Transport(e.to_string()))
    }

    /// A stream URL mpv can open directly (`DirectPlay`). `None` when there is no token.
    pub fn direct_stream_url(&self, item_id: &str) -> Option<String> {
        self.direct_stream_url_with_source(item_id, None)
    }

    /// The direct stream for one media source (a version/edition). `None` lets the server pick.
    pub fn direct_stream_url_with_source(
        &self,
        item_id: &str,
        media_source_id: Option<&str>,
    ) -> Option<String> {
        let token = self.token.as_deref()?;
        let user = self.user_id.as_deref()?;
        let source = media_source_id
            .map(|source| format!("&mediaSourceId={}", urlencode(source)))
            .unwrap_or_default();
        Some(format!(
            "{}/Videos/{}/stream?static=true&api_key={}&UserId={}{source}",
            self.base,
            urlencode(item_id),
            urlencode(token),
            urlencode(user)
        ))
    }

    // ---- remote control ---------------------------------------------------------------

    /// `POST /UserItems/{itemId}/UserData`: rewrite the resume position. Zero is how a client
    /// removes an item from Continue Watching without marking it watched.
    pub fn set_resume_position(&self, item_id: &str, ticks: i64) -> Result<()> {
        self.post_empty(
            &format!("/UserItems/{}/UserData", urlencode(item_id)),
            &serde_json::json!({ "PlaybackPositionTicks": ticks }),
        )
    }

    /// `GET /Sessions`: every active session, including this client's own.
    pub fn sessions(&self) -> Result<Vec<SessionInfo>> {
        self.get("/Sessions")
    }

    /// `POST /Sessions/{id}/Playing`: start an item on another session (`PlayNow`). This is the
    /// cast path; the target client does the actual opening, the server only relays the command.
    /// The command is a **query**, not a body: the 10.11 controller binds `playCommand` and
    /// `itemIds` from the URL and answers 400 without them.
    pub fn play_on_session(&self, session_id: &str, item_ids: &[String]) -> Result<()> {
        self.play_on_session_at(session_id, item_ids, 0)
    }

    /// Play on another session at a position, in ticks. Used by "Continue on…": the receiving
    /// device starts where this one stopped instead of at zero.
    pub fn play_on_session_at(
        &self,
        session_id: &str,
        item_ids: &[String],
        position_ticks: i64,
    ) -> Result<()> {
        if item_ids.is_empty() {
            return Ok(());
        }
        self.post_nothing(&Self::play_on_session_path_at(
            session_id,
            item_ids,
            position_ticks,
        ))
    }

    pub(crate) fn play_on_session_path_at(
        session_id: &str,
        item_ids: &[String],
        position_ticks: i64,
    ) -> String {
        let ids = item_ids
            .iter()
            .map(|id| urlencode(id))
            .collect::<Vec<_>>()
            .join("%2C");
        format!(
            "/Sessions/{}/Playing?playCommand=PlayNow&itemIds={ids}&startPositionTicks={}",
            urlencode(session_id),
            position_ticks.max(0)
        )
    }

    /// `POST /Sessions/{sessionId}/Playing/{command}`: `Pause`, `Unpause` or `Stop`.
    pub fn session_command(&self, session_id: &str, command: &str) -> Result<()> {
        self.post_empty(
            &format!(
                "/Sessions/{}/Playing/{}",
                urlencode(session_id),
                urlencode(command)
            ),
            &serde_json::json!({}),
        )
    }

    /// Seek another session, in ticks (10,000 per millisecond).
    pub fn session_seek(&self, session_id: &str, position_ticks: i64) -> Result<()> {
        self.post_empty(
            &format!(
                "/Sessions/{}/Playing/Seek?seekPositionTicks={position_ticks}",
                urlencode(session_id)
            ),
            &serde_json::json!({}),
        )
    }

    /// Mark or unmark an item as a favourite for the signed-in user. Jellyfin uses a POST
    /// with no body to add and DELETE to remove.
    pub fn set_favorite(&self, item_id: &str, favorite: bool) -> Result<()> {
        let url = format!("{}/UserFavoriteItems/{}", self.base, urlencode(item_id));
        if favorite {
            let mut req = self
                .agent
                .post(&url)
                .header("Authorization", &self.authorization())
                .header("User-Agent", self.user_agent())
                .header("Accept", "application/json");
            if let Some(token) = &self.token {
                req = req.header("X-Emby-Token", token);
            }
            req.send_empty().map_err(map_err)?;
        } else {
            let mut req = self
                .agent
                .delete(&url)
                .header("Authorization", &self.authorization())
                .header("User-Agent", self.user_agent())
                .header("Accept", "application/json");
            if let Some(token) = &self.token {
                req = req.header("X-Emby-Token", token);
            }
            req.call().map_err(map_err)?;
        }
        Ok(())
    }

    /// Mark or unmark an item watched for the signed-in user. The endpoint mirrors favourites.
    /// Like, dislike or clear the rating. Jellyfin spells this as a nullable `likes` flag.
    pub fn set_rating(&self, item_id: &str, like: Option<bool>) -> Result<()> {
        let base = format!(
            "/UserItems/{}/Rating?userId={}",
            urlencode(item_id),
            urlencode(self.user()?)
        );
        match like {
            Some(value) => self.post_nothing(&format!("{base}&likes={value}")),
            None => {
                let url = format!("{}{}", self.base, base);
                let mut req = self
                    .agent
                    .delete(&url)
                    .header("Authorization", &self.authorization())
                    .header("User-Agent", self.user_agent())
                    .header("Accept", "application/json");
                if let Some(token) = &self.token {
                    req = req.header("X-Emby-Token", token);
                }
                req.call().map_err(map_err)?;
                Ok(())
            }
        }
    }

    pub fn set_played(&self, item_id: &str, played: bool) -> Result<()> {
        let url = format!("{}/UserPlayedItems/{}", self.base, urlencode(item_id));
        if played {
            let mut req = self
                .agent
                .post(&url)
                .header("Authorization", &self.authorization())
                .header("User-Agent", self.user_agent())
                .header("Accept", "application/json");
            if let Some(token) = &self.token {
                req = req.header("X-Emby-Token", token);
            }
            req.send_empty().map_err(map_err)?;
        } else {
            let mut req = self
                .agent
                .delete(&url)
                .header("Authorization", &self.authorization())
                .header("User-Agent", self.user_agent())
                .header("Accept", "application/json");
            if let Some(token) = &self.token {
                req = req.header("X-Emby-Token", token);
            }
            req.call().map_err(map_err)?;
        }
        Ok(())
    }

    /// `GET /Items?includeItemTypes=Playlist`: the user's playlists, newest name order. The
    /// server has no dedicated list endpoint; a playlist is an item of its own type.
    pub fn playlists(&self) -> Result<Vec<Item>> {
        let path = format!(
            "/Items?userId={}&recursive=true&includeItemTypes=Playlist&sortBy=SortName&fields=UserData",
            urlencode(self.user()?)
        );
        Ok(self.get::<ItemsResult>(&path)?.items)
    }

    /// `GET /Playlists/{id}/Items`: a playlist's entries in their own order. They are not
    /// children of the playlist, so `/Items?parentId=` is the wrong endpoint here.
    pub fn playlist_items(&self, playlist_id: &str) -> Result<Vec<Item>> {
        let path = format!(
            "/Playlists/{}/Items?userId={}&fields=UserData",
            urlencode(playlist_id),
            urlencode(self.user()?)
        );
        Ok(self.get::<ItemsResult>(&path)?.items)
    }

    /// `POST /Playlists`: create an empty or pre-filled playlist. The body is the server's
    /// `CreatePlaylistDto`; the answer carries the new playlist id.
    pub fn create_playlist(&self, name: &str, ids: &[String]) -> Result<String> {
        #[derive(serde::Serialize)]
        struct Body<'a> {
            #[serde(rename = "Name")]
            name: &'a str,
            #[serde(rename = "Ids")]
            ids: &'a [String],
            #[serde(rename = "MediaType")]
            media_type: &'a str,
        }
        #[derive(serde::Deserialize)]
        struct Created {
            #[serde(rename = "Id")]
            id: String,
        }
        let created: Created = self.post(
            "/Playlists",
            &Body {
                name,
                ids,
                media_type: "Video",
            },
        )?;
        Ok(created.id)
    }

    /// `POST /Playlists/{id}/Items?ids=`: append entries in the order given.
    pub fn add_to_playlist(&self, playlist_id: &str, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let joined = ids
            .iter()
            .map(|id| urlencode(id))
            .collect::<Vec<_>>()
            .join("%2C");
        self.post_nothing(&format!(
            "/Playlists/{}/Items?ids={joined}",
            urlencode(playlist_id)
        ))
    }

    /// `POST /Playlists/{id}/Items/{entryId}/Move/{newIndex}`: reorder one entry. The index is
    /// the final position, so moving the first entry to 2 puts it last in a three-entry list.
    pub fn move_playlist_entry(
        &self,
        playlist_id: &str,
        entry_id: &str,
        new_index: usize,
    ) -> Result<()> {
        self.post_nothing(&format!(
            "/Playlists/{}/Items/{}/Move/{new_index}",
            urlencode(playlist_id),
            urlencode(entry_id)
        ))
    }

    /// `DELETE /Playlists/{id}/Items?entryIds=`: remove entries by their `PlaylistItemId`, not
    /// by item id — a film can appear twice in one playlist.
    pub fn remove_from_playlist(&self, playlist_id: &str, entry_ids: &[String]) -> Result<()> {
        if entry_ids.is_empty() {
            return Ok(());
        }
        let joined = entry_ids
            .iter()
            .map(|id| urlencode(id))
            .collect::<Vec<_>>()
            .join("%2C");
        let url = format!(
            "{}/Playlists/{}/Items?entryIds={joined}",
            self.base,
            urlencode(playlist_id)
        );
        let mut req = self
            .agent
            .delete(&url)
            .header("Authorization", &self.authorization())
            .header("User-Agent", self.user_agent())
            .header("Accept", "application/json");
        if let Some(token) = &self.token {
            req = req.header("X-Emby-Token", token);
        }
        req.call().map_err(map_err)?;
        Ok(())
    }

    /// `DELETE /Items/{id}`: removing a playlist is removing an item. The server decides whether
    /// this user may; a 403 comes back as an ordinary error.
    pub fn delete_playlist(&self, playlist_id: &str) -> Result<()> {
        let url = format!("{}/Items/{}", self.base, urlencode(playlist_id));
        let mut req = self
            .agent
            .delete(&url)
            .header("Authorization", &self.authorization())
            .header("User-Agent", self.user_agent())
            .header("Accept", "application/json");
        if let Some(token) = &self.token {
            req = req.header("X-Emby-Token", token);
        }
        req.call().map_err(map_err)?;
        Ok(())
    }

    /// Episodes anywhere under a library, in the order asked for. `SortBy::Random` is the
    /// server's own shuffle: it is the only correct way to randomise a library too big to load.
    pub fn episodes_in(&self, parent_id: &str, sort: SortBy, limit: u32) -> Result<Vec<Item>> {
        let path = format!(
            "/Items?userId={}&parentId={}&recursive=true&includeItemTypes=Episode&sortBy={}&limit={limit}&fields=UserData",
            urlencode(self.user()?),
            urlencode(parent_id),
            sort.as_str()
        );
        Ok(self.get::<ItemsResult>(&path)?.items)
    }

    /// `GET /Items?parentId={season}&includeItemTypes=Episode`: one season in broadcast order.
    /// Used to offer the previous episode, which `/Shows/NextUp` cannot answer.
    pub fn episodes(&self, season_id: &str) -> Result<Vec<Item>> {
        let path = format!(
            "/Items?userId={}&parentId={}&includeItemTypes=Episode&sortBy=ParentIndexNumber,IndexNumber&fields=UserData",
            urlencode(self.user()?),
            urlencode(season_id)
        );
        Ok(self.get::<ItemsResult>(&path)?.items)
    }

    /// A chapter still: `/Items/{id}/Images/Chapter/{index}`. Chapters carry their own image
    /// tag, so the URL is cacheable for as long as the tag is.
    pub fn chapter_image_url(
        &self,
        item_id: &str,
        index: u32,
        tag: &str,
        max_width: u32,
    ) -> Option<String> {
        let token = self.token.as_deref()?;
        Some(format!(
            "{}/Items/{}/Images/Chapter/{index}?maxWidth={max_width}&quality=85&tag={}&api_key={}",
            self.base,
            urlencode(item_id),
            urlencode(tag),
            urlencode(token)
        ))
    }

    /// `GET /Items/{itemId}/Similar`: what the server recommends after this item. It answers for
    /// films and episodes; a container may have nothing, which is an empty rail, not an error.
    pub fn similar(&self, item_id: &str, limit: u32) -> Result<Vec<Item>> {
        let path = format!(
            "/Items/{}/Similar?userId={}&limit={limit}&fields=UserData",
            urlencode(item_id),
            urlencode(self.user()?)
        );
        Ok(self.get::<ItemsResult>(&path)?.items)
    }

    /// Every episode under a series (or season) in broadcast order. Recursive because a series
    /// holds seasons, not episodes; this is what lets the player offer the previous episode
    /// across a season boundary.
    pub fn episodes_under(&self, parent_id: &str) -> Result<Vec<Item>> {
        let path = format!(
            "/Items?userId={}&parentId={}&recursive=true&includeItemTypes=Episode\
             &sortBy=ParentIndexNumber,IndexNumber&fields=UserData&limit=2000",
            urlencode(self.user()?),
            urlencode(parent_id)
        );
        Ok(self.get::<ItemsResult>(&path)?.items)
    }

    /// `GET /Items/{itemId}/RemoteSearch/Subtitles/{language}`: what the configured subtitle
    /// providers offer, newest searches first from the server's cache. A server without a
    /// provider answers with an empty list, which is a normal "nothing to download".
    pub fn search_subtitles(&self, item_id: &str, language: &str) -> Result<Vec<RemoteSubtitle>> {
        let path = format!(
            "/Items/{}/RemoteSearch/Subtitles/{}",
            urlencode(item_id),
            urlencode(language)
        );
        self.get::<Vec<RemoteSubtitle>>(&path)
    }

    /// `POST /Items/{itemId}/RemoteSearch/Subtitles/{subtitleId}`: the provider's subtitle is
    /// downloaded on the server and attached to the item as an external track. Re-read the item
    /// afterwards to see it.
    pub fn download_subtitle(&self, item_id: &str, subtitle_id: &str) -> Result<()> {
        let path = format!(
            "/Items/{}/RemoteSearch/Subtitles/{}",
            urlencode(item_id),
            urlencode(subtitle_id)
        );
        self.post_nothing(&path)
    }

    /// `GET /MediaSegments/{itemId}`: intro, outro and preview spans when the server supplies
    /// them. Older servers return 404; that is an empty list, not a playback error.
    pub fn media_segments(&self, item_id: &str) -> Result<Vec<MediaSegment>> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "PascalCase")]
        struct Segments {
            #[serde(default)]
            items: Vec<MediaSegment>,
        }
        match self.get::<Segments>(&format!("/MediaSegments/{}", urlencode(item_id))) {
            Ok(segments) => Ok(segments.items),
            Err(Error::Status { code: 404, .. }) => Ok(Vec::new()),
            Err(error) => Err(error),
        }
    }

    // ---- playback reporting ----------------------------------------------------------

    /// `POST /Sessions/Playing`: a play started.
    pub fn report_playing(&self, report: &PlaybackReport) -> Result<()> {
        self.post_empty("/Sessions/Playing", report)
    }

    /// `POST /Sessions/Playing/Progress`: every ~10 s while it plays, and on pause/seek.
    pub fn report_progress(&self, report: &PlaybackReport) -> Result<()> {
        self.post_empty("/Sessions/Playing/Progress", report)
    }

    /// `POST /Sessions/Playing/Stopped`: the play ended, with the position to resume from.
    /// Always call this on every exit path or the server keeps a phantom session playing.
    pub fn report_stopped(&self, stop: &PlaybackStop) -> Result<()> {
        self.post_empty("/Sessions/Playing/Stopped", stop)
    }
}

/// `POST /Items/{id}/PlaybackInfo`. Only fields the app actually sets; the device profile is
/// the one that tells the server what this machine can decode.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlaybackInfoRequest {
    pub user_id: String,
    pub start_time_ticks: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_streaming_bitrate: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_source_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_stream_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle_stream_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_audio_channels: Option<i32>,
    pub enable_direct_play: bool,
    pub enable_direct_stream: bool,
    pub enable_transcoding: bool,
    pub allow_video_stream_copy: bool,
    pub allow_audio_stream_copy: bool,
    pub device_profile: serde_json::Value,
}

/// A live channel that is open on a tuner, and the ids needed to report and close it.
#[derive(Debug, Clone)]
pub struct LiveStream {
    pub url: String,
    pub live_stream_id: Option<String>,
    pub play_session_id: Option<String>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "PascalCase", default)]
struct PlaybackInfo {
    media_sources: Vec<LiveSource>,
    play_session_id: Option<String>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "PascalCase", default)]
struct LiveSource {
    path: Option<String>,
    open_token: Option<String>,
    live_stream_id: Option<String>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "PascalCase", default)]
struct OpenLiveStream {
    #[serde(rename = "MediaSource")]
    media_source: Option<LiveSource>,
}

/// The subset of `/Users/{id}/Items` this app uses. Built with methods so screens stay readable.
#[derive(Debug, Clone, Default)]
pub struct ItemQuery {
    pub parent_id: Option<String>,
    pub include_types: Vec<&'static str>,
    pub recursive: bool,
    pub search: Option<String>,
    /// The letter rail's jump: `nameStartsWith=A` asks the server to start there.
    pub name_starts_with: Option<String>,
    pub sort_by: Option<SortBy>,
    pub descending: bool,
    pub start_index: u32,
    pub limit: u32,
    pub unwatched_only: bool,
    /// `filters=IsFavorite`: the user's favourites, optionally combined with unwatched.
    pub favorites_only: bool,
    /// `filters=IsPlayed`: the user's played items, usually sorted by `DatePlayed`.
    pub played_only: bool,
    /// `genreIds`: one or more genre ids, combined with AND.
    pub genre_ids: Vec<String>,
    /// Exact release years, combined with OR by the server.
    pub years: Vec<i32>,
    /// Minimum community rating, 0–10.
    pub min_community_rating: Option<f64>,
    /// Three-letter audio/subtitle language codes.
    pub audio_languages: Vec<String>,
    pub subtitle_languages: Vec<String>,
    pub has_subtitles: bool,
    /// `filters=Is4K`: the server's own 4K flag, not a client-side guess from width.
    pub is_4k: bool,
    pub missing_only: bool,
    pub unaired_only: bool,
    /// Extra `/Items` fields (for example `Chapters`) appended after `UserData`.
    pub extra_fields: Option<String>,
}

impl ItemQuery {
    pub fn in_library(parent_id: impl Into<String>) -> Self {
        Self {
            parent_id: Some(parent_id.into()),
            recursive: true,
            limit: 60,
            ..Default::default()
        }
    }

    pub fn search(term: impl Into<String>) -> Self {
        Self {
            search: Some(term.into()),
            recursive: true,
            include_types: vec!["Movie", "Series", "Episode", "BoxSet", "Person"],
            limit: 40,
            ..Default::default()
        }
    }

    /// Query parameters exactly as the 12.x OpenAPI names them (camelCase). `fields=UserData` is
    /// not optional: without it `/Items` omits watch state silently and every poster loses its
    /// progress bar.
    pub fn to_query(&self) -> String {
        let mut q = String::new();
        let mut push = |k: &str, v: &str| {
            if !q.is_empty() {
                q.push('&');
            }
            q.push_str(k);
            q.push('=');
            q.push_str(&urlencode(v));
        };
        if let Some(parent) = &self.parent_id {
            push("parentId", parent);
        }
        if !self.include_types.is_empty() {
            push("includeItemTypes", &self.include_types.join(","));
        }
        if self.recursive {
            push("recursive", "true");
        }
        if let Some(term) = &self.search {
            push("searchTerm", term);
        }
        if let Some(letter) = &self.name_starts_with {
            push("nameStartsWith", letter);
        }
        if let Some(sort) = self.sort_by {
            push("sortBy", sort.as_str());
            if self.descending {
                push("sortOrder", "Descending");
            }
        }
        let mut filters: Vec<&str> = Vec::new();
        if self.unwatched_only {
            filters.push("IsUnplayed");
        }
        if self.favorites_only {
            filters.push("IsFavorite");
        }
        if self.played_only {
            filters.push("IsPlayed");
        }
        if self.has_subtitles {
            filters.push("HasSubtitles");
        }
        if self.is_4k {
            filters.push("Is4K");
        }
        if self.missing_only {
            filters.push("IsMissing");
        }
        if self.unaired_only {
            filters.push("IsUnaired");
        }
        if !self.genre_ids.is_empty() {
            push("genreIds", &self.genre_ids.join(","));
        }
        if !self.years.is_empty() {
            let years: Vec<String> = self.years.iter().map(i32::to_string).collect();
            push("years", &years.join(","));
        }
        if let Some(rating) = self.min_community_rating {
            push("minCommunityRating", &format!("{rating:.1}"));
        }
        if !self.audio_languages.is_empty() {
            push("audioLanguages", &self.audio_languages.join(","));
        }
        if !self.subtitle_languages.is_empty() {
            push("subtitleLanguages", &self.subtitle_languages.join(","));
        }
        if !filters.is_empty() {
            push("filters", &filters.join(","));
        }
        let fields = self.extra_fields.as_deref().map_or_else(
            || "UserData".to_string(),
            |extra| format!("UserData,{extra}"),
        );
        push("fields", &fields);
        push("enableUserData", "true");
        push("enableImageTypes", "Primary,Backdrop,Thumb");
        push("limit", &self.limit.to_string());
        push("startIndex", &self.start_index.to_string());
        q
    }
}

fn map_err(e: ureq::Error) -> Error {
    match e {
        ureq::Error::StatusCode(code) => Error::Status {
            code,
            body: String::new(),
        },
        other => Error::Transport(other.to_string()),
    }
}

/// Percent-encodes everything that is not unreserved, so ids, tags and tokens survive the URL.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Quotes and backslashes would break the `key="value"` list Jellyfin parses.
fn escape_header(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn device_name() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|h| !h.trim().is_empty())
        .unwrap_or_else(|| "Linux".to_string())
}

#[cfg(test)]
mod tests;
