//! The Seerr companion's state: Settings buffers, the connection probe, the Requests page, the
//! Discover catalogue and the request panel.
//!
//! `GET /request` has no title or poster, so the list fetches a `/movie` or `/tv` summary per
//! visible key and caches it. A season pack repeats one `downloadId` and one size on every
//! episode row; every aggregate here is per unique download.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rune::TextEdit;
use seerr_api::{
    CreateRequest, Details, DiscoverItem, MediaRequest, MediaStatus, MediaSummary, RequestCounts,
    SearchResults, Seasons,
};
use zeroize::Zeroize;

use super::*;
use crate::net::ImagePriority;

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const REQUEST_PAGE: u32 = 50;
const SUMMARY_WORKERS: usize = 4;
const TMDB_POSTER: u32 = 342;
const TMDB_BACKDROP: u32 = 780;

pub type SeerrKey = (String, u64);
pub type SummaryBatch = Vec<(SeerrKey, MediaSummary)>;
/// `(media type, TMDB id)` → the server's item id, or None when it is not in the library.
pub type LibraryRows = Vec<((String, u64), Option<String>)>;
pub type GenrePair = (Vec<seerr_api::Genre>, Vec<seerr_api::Genre>);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeerrProbe {
    pub version: String,
    pub user: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeerrTab {
    Discover,
    Requests,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RequestsFilter {
    #[default]
    Active,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiscoverMode {
    #[default]
    Explore,
    Browse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiscoverKind {
    #[default]
    Movies,
    Series,
}

impl DiscoverKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Movies => "Films",
            Self::Series => "Series",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiscoverSort {
    #[default]
    Popular,
    Rating,
    Newest,
}

impl DiscoverSort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Popular => "Popular",
            Self::Rating => "Top rated",
            Self::Newest => "Newest",
        }
    }

    pub fn sort_by(self) -> &'static str {
        match self {
            Self::Popular => "popularity",
            Self::Rating => "vote_average",
            Self::Newest => "release_date",
        }
    }
}

/// The Browse filters. `hub` is a studio for films and a network for series; the two lists never
/// mix because changing the kind clears it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiscoverFilters {
    pub kind: DiscoverKind,
    pub genre: Option<i64>,
    pub hub: Option<i64>,
    pub sort: DiscoverSort,
    /// `Some(2020)` is the 2020s, `Some(0)` is everything before 1990, `None` is any year.
    pub decade: Option<i32>,
}

impl DiscoverFilters {
    pub fn date_window(&self) -> (Option<String>, Option<String>) {
        match self.decade {
            None => (None, None),
            Some(0) => (None, Some("1989-12-31".to_string())),
            Some(year) => (
                Some(format!("{year}-01-01")),
                Some(format!("{}-12-31", year + 9)),
            ),
        }
    }
}

/// The networks offered on Browse, with their TMDB ids. Verified against the live instance.
pub const NETWORKS: &[(&str, i64)] = &[
    ("Netflix", 213),
    ("HBO", 49),
    ("Apple TV+", 2552),
    ("Disney+", 2739),
    ("Prime Video", 1024),
    ("Hulu", 453),
    ("BBC One", 4),
    ("AMC", 174),
    ("FX", 88),
    ("Access Seven", 67),
    ("CBS", 16),
    ("NBC", 6),
    ("ABC", 2),
    ("The CW", 71),
    ("Paramount+", 4330),
    ("Peacock", 3353),
];

/// The studios offered on Browse, with their TMDB ids. Verified against the live instance.
pub const STUDIOS: &[(&str, i64)] = &[
    ("A24", 41077),
    ("Marvel", 420),
    ("Pixar", 3),
    ("Disney", 2),
    ("Warner Bros.", 174),
    ("Universal", 33),
    ("Paramount", 4),
    ("Columbia", 34),
    ("Lionsgate", 1632),
    ("Ghibli", 10342),
    ("DreamWorks", 521),
    ("Blumhouse", 3172),
];

#[derive(Debug, Clone, Default)]
pub struct SeerrDiscover {
    pub trending: Vec<DiscoverItem>,
    pub movies: Vec<DiscoverItem>,
    pub tv: Vec<DiscoverItem>,
    pub browse: Vec<DiscoverItem>,
    pub browse_page: u32,
    pub browse_pages: u32,
    pub browse_loading: bool,
    pub browse_loaded: bool,
    pub browse_error: Option<String>,
    pub filters: DiscoverFilters,
    pub genres_movie: Vec<seerr_api::Genre>,
    pub genres_tv: Vec<seerr_api::Genre>,
    pub genres_loading: bool,
}

impl SeerrDiscover {
    pub fn is_empty(&self) -> bool {
        self.trending.is_empty() && self.movies.is_empty() && self.tv.is_empty()
    }

    /// The genre list for the current kind.
    pub fn genres(&self) -> &[seerr_api::Genre] {
        match self.filters.kind {
            DiscoverKind::Movies => &self.genres_movie,
            DiscoverKind::Series => &self.genres_tv,
        }
    }
}

pub struct SeerrDetailState {
    pub media_type: String,
    pub tmdb_id: u64,
    pub details: Option<Details>,
    pub selected_seasons: HashSet<i64>,
    pub loading: bool,
    pub error: Option<String>,
    pub requesting: bool,
}

pub struct SeerrState {
    pub url: TextEdit,
    pub key: TextEdit,
    pub user: TextEdit,
    pub testing: bool,
    pub probe: Option<SeerrProbe>,
    pub error: Option<String>,

    pub tab: SeerrTab,
    pub discover_mode: DiscoverMode,
    pub filter: RequestsFilter,
    pub requests: Vec<MediaRequest>,
    pub counts: Option<RequestCounts>,
    pub loading: bool,
    pub requests_error: Option<String>,
    pub last_loaded: Option<Instant>,
    pub in_flight: bool,
    pub summaries: HashMap<(String, u64), MediaSummary>,
    /// Jellyfin item ids resolved by TMDB id, for titles Seerr has not linked itself.
    pub library_ids: HashMap<(String, u64), Option<String>>,
    pub summaries_loading: bool,
    pub summaries_error: Option<String>,

    pub discover: SeerrDiscover,
    pub discover_loading: bool,
    pub discover_error: Option<String>,
    pub query: TextEdit,
    pub search: SearchResults,
    pub searching: bool,
    pub search_error: Option<String>,

    pub detail: Option<SeerrDetailState>,
    /// The tab the detail was opened from, so Back returns there.
    pub detail_tab: SeerrTab,
    /// A person page that opened the detail; Back returns to it instead of the tab.
    pub detail_return: Option<Route>,
    /// The Seerr TV detail for the open Jellyfin series, so the page can offer missing seasons.
    pub series: Option<(u64, seerr_api::TvDetails)>,
    pub series_error: Option<String>,
    pub return_route: Route,
}

impl SeerrState {
    pub fn new(config: &Config) -> Self {
        Self {
            url: TextEdit::new(config.seerr_url.clone()),
            key: TextEdit::new(config.seerr_api_key.clone().unwrap_or_default()).secure(),
            user: TextEdit::new(
                config
                    .seerr_user_id
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
            ),
            testing: false,
            probe: None,
            error: None,
            tab: SeerrTab::Discover,
            discover_mode: DiscoverMode::Explore,
            filter: RequestsFilter::default(),
            requests: Vec::new(),
            counts: None,
            loading: false,
            requests_error: None,
            last_loaded: None,
            in_flight: false,
            summaries: HashMap::new(),
            library_ids: HashMap::new(),
            summaries_loading: false,
            summaries_error: None,
            discover: SeerrDiscover::default(),
            discover_loading: false,
            discover_error: None,
            query: TextEdit::new(""),
            search: SearchResults::default(),
            searching: false,
            search_error: None,
            detail: None,
            detail_tab: SeerrTab::Discover,
            detail_return: None,
            series: None,
            series_error: None,
            return_route: Route::Home,
        }
    }

    /// The server's item for a Seerr title, when the lookup found one.
    pub fn library_id(&self, media_type: &str, tmdb_id: u64) -> Option<String> {
        self.library_ids
            .get(&(media_type.to_string(), tmdb_id))
            .and_then(|id| id.clone())
    }

    pub fn summary(&self, media_type: &str, tmdb_id: u64) -> Option<&MediaSummary> {
        self.summaries.get(&(media_type.to_string(), tmdb_id))
    }
}

/// Turn the two probe answers into one line. A 403 here is a rejected key, not a permission
/// problem, because `/auth/me` only needs any authenticated user.
fn probe_result(
    status: Result<seerr_api::Status, seerr_api::Error>,
    auth: Result<seerr_api::User, seerr_api::Error>,
) -> Result<SeerrProbe, String> {
    let status = status.map_err(|error| error.user_message())?;
    let user = match auth {
        Ok(user) => user.display_name().to_string(),
        Err(seerr_api::Error::Status { code: 403, .. }) => {
            return Err("Seerr rejected the API key.".to_string());
        }
        Err(error) => return Err(error.user_message()),
    };
    Ok(SeerrProbe {
        version: status.version,
        user,
    })
}

pub(super) struct SeerrRequestsLoad {
    requests: Vec<MediaRequest>,
    counts: RequestCounts,
}

fn load_requests(
    client: seerr_api::Client,
    filter: RequestsFilter,
) -> Result<SeerrRequestsLoad, String> {
    let mut requests = Vec::new();
    match filter {
        RequestsFilter::Active => {
            for filter in ["pending", "processing"] {
                let page = client
                    .requests(filter, 0, REQUEST_PAGE)
                    .map_err(|error| error.user_message())?;
                requests.extend(page.results);
            }
        }
        RequestsFilter::All => {
            let page = client
                .requests("all", 0, REQUEST_PAGE)
                .map_err(|error| error.user_message())?;
            requests = page.results;
        }
    }
    let counts = client
        .request_count()
        .map_err(|error| error.user_message())?;
    Ok(SeerrRequestsLoad { requests, counts })
}

fn missing_summary_keys(
    requests: &[MediaRequest],
    summaries: &HashMap<SeerrKey, MediaSummary>,
) -> Vec<SeerrKey> {
    let mut seen = HashSet::new();
    requests
        .iter()
        .filter_map(|request| {
            let tmdb_id = request.media.tmdb_id?;
            let key = (request.media_type.clone(), tmdb_id);
            (!summaries.contains_key(&key) && seen.insert(key.clone())).then_some(key)
        })
        .take(REQUEST_PAGE as usize)
        .collect()
}

/// The seasons Seerr knows that the Jellyfin library does not. Season 0 (specials) is excluded.
pub fn missing_seasons(present: &HashSet<i64>, tv: &seerr_api::TvDetails) -> Vec<i64> {
    tv.seasons
        .iter()
        .filter(|season| {
            season.season_number > 0
                && season.episode_count > 0
                && !present.contains(&season.season_number)
        })
        .map(|season| season.season_number)
        .collect()
}

/// Fetch summaries for up to four keys at a time. A failure is a missing title, not a broken page.
fn load_summaries(client: seerr_api::Client, keys: Vec<SeerrKey>) -> Result<SummaryBatch, String> {
    let queue = Mutex::new(keys.into_iter().collect::<VecDeque<_>>());
    let results = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..SUMMARY_WORKERS {
            let client = client.clone();
            let queue = &queue;
            let results = &results;
            scope.spawn(move || {
                loop {
                    let next = queue
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .pop_front();
                    let Some((media_type, tmdb_id)) = next else {
                        break;
                    };
                    let summary = match media_type.as_str() {
                        "movie" => client.movie(tmdb_id).map(MediaSummary::from),
                        "tv" => client.tv(tmdb_id).map(MediaSummary::from),
                        _ => continue,
                    };
                    if let Ok(summary) = summary {
                        results
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .push(((media_type, tmdb_id), summary));
                    }
                }
            });
        }
    });
    let results = results
        .into_inner()
        .unwrap_or_else(|error| error.into_inner());
    if results.is_empty() {
        Err("Seerr could not load the request titles.".into())
    } else {
        Ok(results)
    }
}

impl State {
    // ---- Settings probe -----------------------------------------------------------------

    pub(super) fn apply_seerr_probe(&mut self, result: Result<SeerrProbe, String>) {
        self.seerr.testing = false;
        match result {
            Ok(probe) => {
                self.seerr.probe = Some(probe);
                self.seerr.error = None;
            }
            Err(message) => {
                self.seerr.probe = None;
                self.seerr.error = Some(message);
            }
        }
    }

    pub fn test_seerr(&mut self) {
        let url = self.seerr.url.text.trim().to_string();
        if url.is_empty() {
            self.seerr.probe = None;
            self.seerr.error = Some("Enter the Seerr address.".into());
            return;
        }
        let key = self.seerr.key.text.trim().to_string();
        if key.is_empty() {
            self.seerr.probe = None;
            self.seerr.error = Some("Enter the Seerr API key.".into());
            return;
        }
        let user_id = self.seerr.user.text.trim().parse::<i64>().ok();
        self.seerr.testing = true;
        self.seerr.probe = None;
        self.seerr.error = None;
        self.seerr_probe.request((), move |()| {
            let mut client = seerr_api::Client::new(&url, key);
            if let Some(user_id) = user_id {
                client = client.with_user(user_id);
            }
            probe_result(client.status(), client.auth_me())
        });
    }

    pub fn clear_seerr_key(&mut self) {
        self.seerr.key = TextEdit::new("").secure();
        self.seerr.probe = None;
        self.seerr.error = None;
        self.seerr.testing = false;
        self.seerr_probe.cancel();
        self.seerr_requests.cancel();
        self.seerr_counts.cancel();
        self.seerr_summaries.cancel();
        self.seerr_discover.cancel();
        self.seerr_search.cancel();
        self.seerr_detail_load.cancel();
        self.seerr.in_flight = false;
        self.seerr.loading = false;
        self.seerr.requests.clear();
        self.seerr.summaries.clear();
        self.seerr.counts = None;
        if let Some(mut old) = self.config.seerr_api_key.take() {
            old.zeroize();
        }
    }

    pub(super) fn seerr_client(&self) -> Option<seerr_api::Client> {
        let key = self.config.seerr_api_key.as_deref()?.trim();
        if self.config.seerr_url.trim().is_empty() || key.is_empty() {
            return None;
        }
        let mut client = seerr_api::Client::new(&self.config.seerr_url, key);
        if let Some(user_id) = self.config.seerr_user_id {
            client = client.with_user(user_id);
        }
        Some(client)
    }

    pub fn seerr_configured(&self) -> bool {
        self.seerr_client().is_some()
    }

    // ---- page entry ---------------------------------------------------------------------

    /// Copy the Settings buffers into the config without writing disk. The Settings "Open
    /// Discover" action uses it so a freshly typed URL/key works before the window closes.
    pub(super) fn apply_seerr_buffers(&mut self) {
        self.config.seerr_url = self.seerr.url.text.trim().to_string();
        let key = self.seerr.key.text.trim().to_string();
        if let Some(mut old) = self.config.seerr_api_key.take() {
            old.zeroize();
        }
        self.config.seerr_api_key = if key.is_empty() { None } else { Some(key) };
        self.config.seerr_user_id = self.seerr.user.text.trim().parse::<i64>().ok();
    }

    pub fn open_seerr_from_settings(&mut self) {
        self.apply_seerr_buffers();
        self.open_seerr(SeerrTab::Discover);
    }

    pub fn open_seerr(&mut self, tab: SeerrTab) {
        self.stop_theme_media();
        if !matches!(self.route, Route::Seerr | Route::SeerrDetail { .. }) {
            self.seerr.return_route = self.route.clone();
        }
        self.route = Route::Seerr;
        self.seerr.tab = tab;
        self.seerr.query.focused = false;
        match tab {
            SeerrTab::Discover => {
                if self.seerr.discover.is_empty() && !self.seerr.discover_loading {
                    self.load_seerr_discover();
                }
            }
            SeerrTab::Requests => {
                if !self.seerr.loading && self.seerr.requests.is_empty() {
                    self.load_seerr_requests();
                }
            }
        }
        self.refresh_seerr_badge();
    }

    pub fn close_seerr(&mut self) {
        self.seerr_requests.cancel();
        self.seerr_search.cancel();
        self.seerr.in_flight = false;
        self.seerr.loading = false;
        self.route = std::mem::replace(&mut self.seerr.return_route, Route::Home);
    }

    pub fn set_seerr_tab(&mut self, tab: SeerrTab) {
        self.seerr.tab = tab;
        self.seerr.searching = false;
        match tab {
            SeerrTab::Discover => {
                if self.seerr.discover.is_empty() && !self.seerr.discover_loading {
                    self.load_seerr_discover();
                }
            }
            SeerrTab::Requests => {
                self.reset_seerr_requests();
                self.load_seerr_requests();
            }
        }
    }

    pub fn set_seerr_filter(&mut self, filter: RequestsFilter) {
        if self.seerr.filter == filter {
            return;
        }
        self.seerr.filter = filter;
        self.reset_seerr_requests();
        self.load_seerr_requests();
    }

    pub fn refresh_seerr(&mut self) {
        self.seerr.summaries_error = None;
        match self.seerr.tab {
            SeerrTab::Discover => self.load_seerr_discover(),
            SeerrTab::Requests => {
                self.reset_seerr_requests();
                self.load_seerr_requests();
            }
        }
    }

    fn reset_seerr_requests(&mut self) {
        self.seerr_requests.cancel();
        self.seerr.in_flight = false;
        self.seerr.loading = false;
    }

    pub fn focus_seerr_search(&mut self) {
        self.seerr.tab = SeerrTab::Discover;
        self.seerr.query.focused = true;
    }

    // ---- Requests -----------------------------------------------------------------------

    fn load_seerr_requests(&mut self) {
        let Some(client) = self.seerr_client() else {
            self.seerr.requests_error = Some("Set up Seerr in Settings first.".into());
            self.seerr.loading = false;
            self.seerr.in_flight = false;
            return;
        };
        if self.seerr.in_flight {
            return;
        }
        self.seerr.loading = true;
        self.seerr.in_flight = true;
        self.seerr.requests_error = None;
        let filter = self.seerr.filter;
        self.seerr_requests
            .request((), move |()| load_requests(client, filter));
    }

    pub(super) fn refresh_seerr_badge(&mut self) {
        let Some(client) = self.seerr_client() else {
            return;
        };
        self.seerr_counts.request((), move |()| {
            client.request_count().map_err(|error| error.user_message())
        });
    }

    fn apply_seerr_requests(&mut self, result: Result<SeerrRequestsLoad, String>) {
        self.seerr.loading = false;
        self.seerr.in_flight = false;
        match result {
            Ok(load) => {
                let was_loaded = self.seerr.last_loaded.is_some();
                let before: HashSet<u64> = self
                    .seerr
                    .requests
                    .iter()
                    .filter(|request| request.media_status() == MediaStatus::Available)
                    .map(|request| request.id)
                    .collect();
                let newly_available = was_loaded
                    .then(|| {
                        load.requests
                            .iter()
                            .find(|request| {
                                request.media_status() == MediaStatus::Available
                                    && !before.contains(&request.id)
                            })
                            .cloned()
                    })
                    .flatten();
                self.seerr.requests = load.requests;
                self.seerr.counts = Some(load.counts);
                self.seerr.requests_error = None;
                self.seerr.last_loaded = Some(Instant::now());
                if let Some(request) = newly_available {
                    let title = request
                        .media
                        .tmdb_id
                        .and_then(|tmdb| self.seerr.summary(&request.media_type, tmdb))
                        .map(|summary| summary.title.clone())
                        .unwrap_or_else(|| "A Seerr request".into());
                    self.show_toast(format!("{title} is now available."));
                }
                self.request_seerr_summaries();
                self.request_seerr_images();
            }
            Err(message) => {
                self.seerr.requests_error = Some(message);
            }
        }
        self.resolve_seerr_library();
    }

    fn apply_seerr_counts(&mut self, result: Result<RequestCounts, String>) {
        if let Ok(counts) = result {
            self.seerr.counts = Some(counts);
        }
    }

    /// Fill in the Jellyfin id for titles Seerr has not linked itself. A partially downloaded
    /// series often has episodes on the server while its Seerr media row still says Processing
    /// with no `jellyfinMediaId`; the library is the truth for "can I watch it".
    pub(super) fn resolve_seerr_library(&mut self) {
        let Some(client) = self.client() else {
            return;
        };
        let mut todo: Vec<(String, u64, String)> = Vec::new();
        let mut seen: HashSet<(String, u64)> = HashSet::new();
        {
            let mut add = |media_type: &str,
                           tmdb_id: u64,
                           title: Option<&str>,
                           ids: &HashMap<(String, u64), Option<String>>| {
                let Some(title) = title else {
                    return;
                };
                if tmdb_id == 0 {
                    return;
                }
                let key = (media_type.to_string(), tmdb_id);
                if ids.contains_key(&key) || !seen.insert(key) {
                    return;
                }
                todo.push((media_type.to_string(), tmdb_id, title.to_string()));
            };
            for request in &self.seerr.requests {
                if request.media.jellyfin_media_id.is_some() {
                    continue;
                }
                let tmdb_id = request.media.tmdb_id.unwrap_or_default();
                let title = self
                    .seerr
                    .summary(&request.media_type, tmdb_id)
                    .map(|summary| summary.title.as_str());
                add(&request.media_type, tmdb_id, title, &self.seerr.library_ids);
            }
            if let Some(detail) = &self.seerr.detail {
                let linked = detail
                    .details
                    .as_ref()
                    .and_then(|details| details.media_info())
                    .and_then(|info| info.jellyfin_media_id.as_ref())
                    .is_some();
                if !linked {
                    let title = detail
                        .details
                        .as_ref()
                        .map(|details| details.title().to_string());
                    add(
                        &detail.media_type,
                        detail.tmdb_id,
                        title.as_deref(),
                        &self.seerr.library_ids,
                    );
                }
            }
        }
        if todo.is_empty() {
            return;
        }
        self.seerr_library_load.request(todo, move |todo| {
            Ok(todo
                .into_iter()
                .map(|(media_type, tmdb_id, title)| {
                    let id = client
                        .find_by_tmdb(&title, &media_type, tmdb_id)
                        .ok()
                        .flatten()
                        .map(|item| item.id);
                    ((media_type, tmdb_id), id)
                })
                .collect())
        });
    }

    fn apply_seerr_library(&mut self, result: Result<LibraryRows, String>) {
        if let Ok(rows) = result {
            for (key, id) in rows {
                self.seerr.library_ids.insert(key, id);
            }
        }
    }

    /// Drain every Seerr answer and start the visible-only refresh. The caller sets `changed` from
    /// the return value, so a timer tick alone never repaints.
    pub(super) fn poll_seerr(&mut self) -> bool {
        let mut changed = false;
        if let Some(result) = self.seerr_requests.take() {
            self.apply_seerr_requests(result);
            changed = true;
        }
        if let Some(result) = self.seerr_counts.take() {
            self.apply_seerr_counts(result);
            changed = true;
        }
        if let Some(result) = self.seerr_summaries.take() {
            self.apply_seerr_summaries(result);
            changed = true;
        }
        if let Some(result) = self.seerr_library_load.take() {
            self.apply_seerr_library(result);
            changed = true;
        }
        if let Some(result) = self.seerr_genres.take() {
            self.apply_seerr_genres(result);
            changed = true;
        }
        if let Some(result) = self.seerr_browse.take() {
            self.apply_seerr_browse(result);
            changed = true;
        }
        if let Some(result) = self.seerr_discover.take() {
            self.apply_seerr_discover(result);
            changed = true;
        }
        if let Some(result) = self.seerr_search.take() {
            self.apply_seerr_search(result);
            changed = true;
        }
        if let Some(result) = self.seerr_detail_load.take() {
            self.apply_seerr_detail(result);
            changed = true;
        }
        if let Some(result) = self.seerr_series_load.take() {
            self.apply_seerr_series(result);
            changed = true;
        }
        self.start_seerr_poll();
        changed
    }

    fn start_seerr_poll(&mut self) {
        if self.route != Route::Seerr || self.seerr.tab != SeerrTab::Requests {
            return;
        }
        if !self.seerr_configured() || self.seerr.in_flight || !self.is_signed_in() {
            return;
        }
        let due = self
            .seerr
            .last_loaded
            .is_none_or(|at| at.elapsed() >= POLL_INTERVAL);
        if due {
            self.load_seerr_requests();
        }
    }

    fn request_seerr_summaries(&mut self) {
        let Some(client) = self.seerr_client() else {
            return;
        };
        if self.seerr.summaries_error.is_some() {
            return;
        }
        let keys = missing_summary_keys(&self.seerr.requests, &self.seerr.summaries);
        if keys.is_empty() {
            return;
        }
        self.seerr.summaries_loading = true;
        self.seerr.summaries_error = None;
        self.seerr_summaries
            .request(keys.clone(), move |keys| load_summaries(client, keys));
    }

    fn apply_seerr_summaries(&mut self, result: Result<SummaryBatch, String>) {
        self.seerr.summaries_loading = false;
        match result {
            Ok(summaries) => {
                for (key, summary) in summaries {
                    self.seerr.summaries.insert(key, summary);
                }
                self.seerr.summaries_error = None;
                self.request_seerr_images();
            }
            Err(message) => self.seerr.summaries_error = Some(message),
        }
        self.resolve_seerr_library();
    }

    // ---- Discover -----------------------------------------------------------------------

    fn load_seerr_discover(&mut self) {
        let Some(client) = self.seerr_client() else {
            self.seerr.discover_error = Some("Set up Seerr in Settings first.".into());
            return;
        };
        if self.seerr.discover_loading {
            return;
        }
        self.seerr.discover_loading = true;
        self.seerr.discover_error = None;
        self.seerr_discover.request((), move |()| {
            let trending = client
                .discover_trending(1)
                .map_err(|error| error.user_message())?
                .results;
            let movies = client
                .discover_movies(1)
                .map_err(|error| error.user_message())?
                .results;
            let tv = client
                .discover_tv(1)
                .map_err(|error| error.user_message())?
                .results;
            Ok(SeerrDiscover {
                trending,
                movies,
                tv,
                ..SeerrDiscover::default()
            })
        });
    }

    fn apply_seerr_discover(&mut self, result: Result<SeerrDiscover, String>) {
        self.seerr.discover_loading = false;
        match result {
            Ok(discover) => {
                self.seerr.discover.trending = discover.trending;
                self.seerr.discover.movies = discover.movies;
                self.seerr.discover.tv = discover.tv;
                self.seerr.discover_error = None;
                self.request_seerr_images();
            }
            Err(message) => self.seerr.discover_error = Some(message),
        }
    }

    // ---- Browse --------------------------------------------------------------------------

    pub fn set_seerr_discover_mode(&mut self, mode: DiscoverMode) {
        self.seerr.discover_mode = mode;
        if mode == DiscoverMode::Browse {
            self.ensure_seerr_browse();
        }
    }

    fn ensure_seerr_browse(&mut self) {
        if self.seerr.discover.genres_movie.is_empty()
            && self.seerr.discover.genres_tv.is_empty()
            && !self.seerr.discover.genres_loading
        {
            self.load_seerr_genres();
        }
        if !self.seerr.discover.browse_loaded && !self.seerr.discover.browse_loading {
            self.load_seerr_browse(true);
        }
    }

    fn load_seerr_genres(&mut self) {
        let Some(client) = self.seerr_client() else {
            return;
        };
        self.seerr.discover.genres_loading = true;
        self.seerr_genres.request((), move |()| {
            let movie = client
                .genres("movie")
                .map_err(|error| error.user_message())?;
            let tv = client.genres("tv").map_err(|error| error.user_message())?;
            Ok((movie, tv))
        });
    }

    fn apply_seerr_genres(
        &mut self,
        result: Result<(Vec<seerr_api::Genre>, Vec<seerr_api::Genre>), String>,
    ) {
        self.seerr.discover.genres_loading = false;
        if let Ok((movie, tv)) = result {
            self.seerr.discover.genres_movie = movie;
            self.seerr.discover.genres_tv = tv;
        }
    }

    fn load_seerr_browse(&mut self, reset: bool) {
        let Some(client) = self.seerr_client() else {
            self.seerr.discover.browse_error = Some("Set up Seerr in Settings first.".into());
            return;
        };
        if self.seerr.discover.browse_loading && !reset {
            return;
        }
        let filters = self.seerr.discover.filters.clone();
        let page = if reset {
            1
        } else {
            self.seerr.discover.browse_page + 1
        };
        self.seerr.discover.browse_loading = true;
        self.seerr.discover.browse_error = None;
        self.seerr_browse_for = Some((filters.clone(), page));
        self.seerr_browse
            .request((filters, page), move |(filters, page)| {
                let (from, to) = filters.date_window();
                let mut query = seerr_api::DiscoverQuery::new(page);
                query.genre = filters.genre;
                query.sort_by = Some(filters.sort.sort_by().to_string());
                query.from = from;
                query.to = to;
                match filters.kind {
                    DiscoverKind::Movies => {
                        query.studio = filters.hub;
                        client.discover_movies_filtered(&query)
                    }
                    DiscoverKind::Series => {
                        query.network = filters.hub;
                        client.discover_tv_filtered(&query)
                    }
                }
                .map_err(|error| error.user_message())
            });
    }

    fn apply_seerr_browse(&mut self, result: Result<seerr_api::SearchResults, String>) {
        let Some((filters, page)) = self.seerr_browse_for.take() else {
            return;
        };
        if filters != self.seerr.discover.filters {
            return;
        }
        let discover = &mut self.seerr.discover;
        discover.browse_loading = false;
        discover.browse_loaded = true;
        match result {
            Ok(results) => {
                if page == 1 {
                    discover.browse = results.results;
                } else {
                    discover.browse.extend(results.results);
                }
                discover.browse_page = page;
                discover.browse_pages = u32::try_from(results.total_pages).unwrap_or(0);
                discover.browse_error = None;
                self.request_seerr_images();
            }
            Err(message) => discover.browse_error = Some(message),
        }
    }

    pub fn set_discover_kind(&mut self, kind: DiscoverKind) {
        if self.seerr.discover.filters.kind == kind {
            return;
        }
        self.seerr.discover.filters.kind = kind;
        self.seerr.discover.filters.genre = None;
        self.seerr.discover.filters.hub = None;
        self.load_seerr_browse(true);
    }

    pub fn set_discover_genre(&mut self, genre: Option<i64>) {
        if self.seerr.discover.filters.genre == genre {
            return;
        }
        self.seerr.discover.filters.genre = genre;
        self.load_seerr_browse(true);
    }

    pub fn set_discover_hub(&mut self, hub: Option<i64>) {
        if self.seerr.discover.filters.hub == hub {
            return;
        }
        self.seerr.discover.filters.hub = hub;
        self.load_seerr_browse(true);
    }

    pub fn set_discover_sort(&mut self, sort: DiscoverSort) {
        if self.seerr.discover.filters.sort == sort {
            return;
        }
        self.seerr.discover.filters.sort = sort;
        self.load_seerr_browse(true);
    }

    pub fn set_discover_decade(&mut self, decade: Option<i32>) {
        if self.seerr.discover.filters.decade == decade {
            return;
        }
        self.seerr.discover.filters.decade = decade;
        self.load_seerr_browse(true);
    }

    pub fn load_more_seerr_browse(&mut self) {
        if !self.seerr.discover.browse_loading {
            self.load_seerr_browse(false);
        }
    }

    pub fn seerr_search_changed(&mut self) {
        let term = self.seerr.query.text.trim().to_string();
        self.seerr.search_error = None;
        if term.chars().count() < 2 {
            self.seerr.search = SearchResults::default();
            self.seerr.searching = false;
            self.seerr_search.cancel();
            return;
        }
        let Some(client) = self.seerr_client() else {
            return;
        };
        self.seerr.searching = true;
        self.seerr_search.request(term.clone(), move |term| {
            client
                .search(&term, 1)
                .map_err(|error| error.user_message())
        });
    }

    fn apply_seerr_search(&mut self, result: Result<SearchResults, String>) {
        self.seerr.searching = false;
        match result {
            Ok(results) => {
                self.seerr.search = results;
                self.seerr.search_error = None;
                self.request_seerr_images();
            }
            Err(message) => self.seerr.search_error = Some(message),
        }
    }

    // ---- Detail and request -------------------------------------------------------------

    pub fn open_seerr_detail(&mut self, media_type: String, tmdb_id: u64) {
        self.seerr.detail_tab = self.seerr.tab;
        self.seerr.detail_return =
            matches!(self.route, Route::Person { .. } | Route::SeerrPerson { .. })
                .then(|| self.route.clone());
        self.seerr.detail = Some(SeerrDetailState {
            media_type: media_type.clone(),
            tmdb_id,
            details: None,
            selected_seasons: HashSet::new(),
            loading: true,
            error: None,
            requesting: false,
        });
        self.route = Route::SeerrDetail {
            media_type,
            tmdb_id,
        };
        self.load_seerr_detail();
        self.request_seerr_images();
    }

    pub fn back_from_seerr_detail(&mut self) {
        self.seerr_detail_load.cancel();
        self.seerr_detail_for = None;
        self.seerr.detail = None;
        if let Some(route) = self.seerr.detail_return.take() {
            self.route = route;
            return;
        }
        self.route = Route::Seerr;
        self.seerr.tab = self.seerr.detail_tab;
    }

    fn load_seerr_detail(&mut self) {
        let Some(detail) = &self.seerr.detail else {
            return;
        };
        let media_type = detail.media_type.clone();
        let tmdb_id = detail.tmdb_id;
        let Some(client) = self.seerr_client() else {
            return;
        };
        self.seerr_detail_for = Some((media_type.clone(), tmdb_id));
        self.seerr_detail_load.request(
            (media_type.clone(), tmdb_id),
            move |(media_type, tmdb_id)| {
                match media_type.as_str() {
                    "movie" => client.movie(tmdb_id).map(Details::Movie),
                    "tv" => client.tv(tmdb_id).map(Details::Tv),
                    _ => Err(seerr_api::Error::Decode(format!(
                        "unknown media type {media_type}"
                    ))),
                }
                .map_err(|error| error.user_message())
            },
        );
    }

    fn apply_seerr_detail(&mut self, result: Result<Details, String>) {
        let Some((media_type, tmdb_id)) = self.seerr_detail_for.take() else {
            return;
        };
        let belongs = self
            .seerr
            .detail
            .as_ref()
            .is_some_and(|detail| detail.media_type == media_type && detail.tmdb_id == tmdb_id);
        if !belongs {
            return;
        }
        if let Some(detail) = &mut self.seerr.detail {
            detail.loading = false;
            match result {
                Ok(details) => {
                    if detail.selected_seasons.is_empty()
                        && let Details::Tv(tv) = &details
                    {
                        detail.selected_seasons = tv
                            .seasons
                            .iter()
                            .filter(|season| season.season_number > 0 && season.episode_count > 0)
                            .map(|season| season.season_number)
                            .collect();
                    }
                    detail.details = Some(details);
                    detail.error = None;
                }
                Err(message) => detail.error = Some(message),
            }
        }
        self.request_seerr_images();
        self.resolve_seerr_library();
    }

    pub fn toggle_seerr_season(&mut self, season: i64) {
        let Some(detail) = &mut self.seerr.detail else {
            return;
        };
        if !detail.selected_seasons.remove(&season) {
            detail.selected_seasons.insert(season);
        }
    }

    pub fn select_all_seerr_seasons(&mut self) {
        let Some(detail) = &mut self.seerr.detail else {
            return;
        };
        let Some(Details::Tv(tv)) = &detail.details else {
            return;
        };
        let all: HashSet<i64> = tv
            .seasons
            .iter()
            .filter(|season| season.season_number > 0 && season.episode_count > 0)
            .map(|season| season.season_number)
            .collect();
        if detail.selected_seasons == all {
            detail.selected_seasons.clear();
        } else {
            detail.selected_seasons = all;
        }
    }

    pub fn request_seerr_selected(&mut self) {
        let Some(detail) = &self.seerr.detail else {
            return;
        };
        let Some(details) = &detail.details else {
            return;
        };
        let Some(client) = self.seerr_client() else {
            self.show_toast("Set up Seerr in Settings first.");
            return;
        };
        let request = match details {
            Details::Movie(movie) => CreateRequest::movie(movie.id),
            Details::Tv(tv) => {
                let all: HashSet<i64> = tv
                    .seasons
                    .iter()
                    .filter(|season| season.season_number > 0 && season.episode_count > 0)
                    .map(|season| season.season_number)
                    .collect();
                let selected = &detail.selected_seasons;
                let seasons = if selected.is_empty() || selected == &all {
                    Seasons::all()
                } else {
                    Seasons::numbers(selected.iter().copied())
                };
                CreateRequest::tv(tv.id, seasons)
            }
        };
        if let Some(detail) = &mut self.seerr.detail {
            detail.requesting = true;
        }
        self.worker.create_seerr_request(client, request);
    }

    pub(super) fn apply_seerr_requested(
        &mut self,
        result: Result<seerr_api::MediaRequest, String>,
    ) {
        if let Some(detail) = &mut self.seerr.detail {
            detail.requesting = false;
        }
        match result {
            Ok(_) => {
                self.show_toast("Request sent to Seerr.");
                self.seerr.requests.clear();
                self.seerr.last_loaded = None;
                if self.route == Route::Seerr && self.seerr.tab == SeerrTab::Requests {
                    self.load_seerr_requests();
                }
                self.refresh_seerr_badge();
                if matches!(self.route, Route::SeerrDetail { .. }) {
                    self.load_seerr_detail();
                }
            }
            Err(message) => {
                self.show_toast(message);
                if matches!(self.route, Route::SeerrDetail { .. }) {
                    self.load_seerr_detail();
                }
            }
        }
    }

    pub fn seerr_open_in_jellyfin(&mut self, item_id: String) {
        self.open_detail(item_id);
    }

    /// On a Jellyfin series, load the matching Seerr TV detail so the page can offer only the
    /// seasons the library is missing.
    pub(super) fn request_seerr_series(&mut self) {
        self.seerr.series = None;
        self.seerr.series_error = None;
        let Some(detail) = &self.detail else {
            return;
        };
        if detail.item.kind != "Series" {
            return;
        }
        let Some(tmdb) = detail
            .item
            .provider_ids
            .get("Tmdb")
            .and_then(|id| id.parse::<u64>().ok())
        else {
            return;
        };
        let Some(client) = self.seerr_client() else {
            return;
        };
        self.seerr_series_for = Some(tmdb);
        self.seerr_series_load.request(tmdb, move |tmdb| {
            client.tv(tmdb).map_err(|error| error.user_message())
        });
    }

    fn apply_seerr_series(&mut self, result: Result<seerr_api::TvDetails, String>) {
        let Some(tmdb) = self.seerr_series_for.take() else {
            return;
        };
        let belongs = self
            .detail
            .as_ref()
            .and_then(|detail| detail.item.provider_ids.get("Tmdb"))
            .and_then(|id| id.parse::<u64>().ok())
            == Some(tmdb);
        if !belongs {
            return;
        }
        match result {
            Ok(tv) => self.seerr.series = Some((tmdb, tv)),
            Err(message) => self.seerr.series_error = Some(message),
        }
    }

    pub fn request_seerr_missing_seasons(&mut self, seasons: Vec<i64>) {
        let Some((tmdb, _)) = &self.seerr.series else {
            return;
        };
        let tmdb = *tmdb;
        let Some(client) = self.seerr_client() else {
            self.show_toast("Set up Seerr in Settings first.");
            return;
        };
        self.seerr.series = None;
        self.worker
            .create_seerr_request(client, CreateRequest::tv(tmdb, Seasons::numbers(seasons)));
    }

    // ---- Images -------------------------------------------------------------------------

    /// The image key for a TMDB profile, for screens that render one.
    pub fn seerr_profile_key(&self, tmdb_id: u64, path: &str, width: u32) -> String {
        self.seerr_image_key("person", tmdb_id, "Profile", path, width)
    }

    /// The image key for a Seerr poster, for screens that render one.
    pub fn seerr_poster_key(
        &self,
        media_type: &str,
        tmdb_id: u64,
        path: &str,
        width: u32,
    ) -> String {
        self.seerr_image_key(media_type, tmdb_id, "Poster", path, width)
    }

    pub(super) fn seerr_image_key(
        &self,
        media_type: &str,
        tmdb_id: u64,
        kind: &str,
        path: &str,
        width: u32,
    ) -> String {
        crate::image::key(
            &self.config.seerr_url,
            &format!("tmdb-{media_type}-{tmdb_id}"),
            kind,
            Some(path),
            width,
        )
    }

    pub(super) fn tmdb_url(width: u32, path: &str) -> String {
        format!("https://image.tmdb.org/t/p/w{width}{path}")
    }

    pub fn seerr_item_poster_key(&self, item: &DiscoverItem) -> Option<String> {
        let path = item.poster_path.as_deref()?;
        Some(self.seerr_image_key(&item.media_type, item.id, "Poster", path, TMDB_POSTER))
    }

    pub fn seerr_summary_poster_key(&self, media_type: &str, tmdb_id: u64) -> Option<String> {
        let summary = self.seerr.summary(media_type, tmdb_id)?;
        let path = summary.poster_path.as_deref()?;
        Some(self.seerr_image_key(media_type, tmdb_id, "Poster", path, TMDB_POSTER))
    }

    pub fn seerr_detail_poster_key(&self) -> Option<String> {
        let detail = self.seerr.detail.as_ref()?;
        let details = detail.details.as_ref()?;
        let path = details.poster_path()?;
        Some(self.seerr_image_key(
            &detail.media_type,
            detail.tmdb_id,
            "Poster",
            path,
            TMDB_POSTER,
        ))
    }

    pub fn seerr_detail_backdrop_key(&self) -> Option<String> {
        let detail = self.seerr.detail.as_ref()?;
        let details = detail.details.as_ref()?;
        let path = details.backdrop_path()?;
        Some(self.seerr_image_key(
            &detail.media_type,
            detail.tmdb_id,
            "Backdrop",
            path,
            TMDB_BACKDROP,
        ))
    }

    pub(super) fn request_seerr_images(&mut self) {
        if self.image_cache.is_none() {
            return;
        }
        let mut requests: Vec<(String, String, ImagePriority)> = Vec::new();
        for ((media_type, tmdb_id), summary) in &self.seerr.summaries {
            if let Some(path) = &summary.poster_path {
                let key = self.seerr_image_key(media_type, *tmdb_id, "Poster", path, TMDB_POSTER);
                requests.push((key, Self::tmdb_url(TMDB_POSTER, path), ImagePriority::High));
            }
        }
        for item in self
            .seerr
            .discover
            .trending
            .iter()
            .chain(&self.seerr.discover.movies)
            .chain(&self.seerr.discover.tv)
            .chain(&self.seerr.discover.browse)
            .chain(&self.seerr.search.results)
            .chain(&self.search.seerr.results)
        {
            if let Some(path) = &item.poster_path {
                let key =
                    self.seerr_image_key(&item.media_type, item.id, "Poster", path, TMDB_POSTER);
                requests.push((key, Self::tmdb_url(TMDB_POSTER, path), ImagePriority::Low));
            }
        }
        if let Some(detail) = &self.seerr.detail
            && let Some(details) = &detail.details
        {
            if let Some(path) = details.backdrop_path() {
                let key = self.seerr_image_key(
                    &detail.media_type,
                    detail.tmdb_id,
                    "Backdrop",
                    path,
                    TMDB_BACKDROP,
                );
                requests.push((
                    key,
                    Self::tmdb_url(TMDB_BACKDROP, path),
                    ImagePriority::High,
                ));
            }
            if let Some(path) = details.poster_path() {
                let key = self.seerr_image_key(
                    &detail.media_type,
                    detail.tmdb_id,
                    "Poster",
                    path,
                    TMDB_POSTER,
                );
                requests.push((key, Self::tmdb_url(TMDB_POSTER, path), ImagePriority::High));
            }
            for member in details.credits().cast.iter().take(12) {
                if let Some(path) = &member.profile_path {
                    let key = self.seerr_image_key("person", member.id, "Profile", path, 160);
                    requests.push((key, Self::tmdb_url(160, path), ImagePriority::High));
                }
            }
        }
        for (key, url, priority) in requests {
            if self.requested_images.insert(key.clone()) {
                self.worker.request_public_image(key, url, priority);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status() -> seerr_api::Status {
        seerr_api::Status {
            version: "3.5.1".into(),
            commit_tag: None,
            update_available: None,
            commits_behind: None,
            restart_required: None,
        }
    }

    fn user() -> seerr_api::User {
        seerr_api::User {
            id: 1,
            username: None,
            email: Some("user@example.invalid".into()),
            display_name: Some("example-user".into()),
            jellyfin_username: None,
            permissions: 2,
            request_count: 0,
        }
    }

    #[test]
    fn a_403_on_auth_says_the_key_was_rejected() {
        let error = probe_result(
            Ok(status()),
            Err(seerr_api::Error::Status {
                code: 403,
                body: "{}".into(),
            }),
        )
        .expect_err("a rejected key is an error");
        assert_eq!(error, "Seerr rejected the API key.");
    }

    #[test]
    fn a_transport_error_says_seerr_is_unreachable() {
        let error = probe_result(Err(seerr_api::Error::Transport("dns".into())), Ok(user()))
            .expect_err("transport is an error");
        assert_eq!(error, "Can't reach Seerr.");
    }

    #[test]
    fn a_probe_result_fills_the_status_and_clears_the_error() {
        let mut state = State::from_config(Config::default());
        state.seerr.error = Some("old".into());
        state.apply_seerr_probe(Ok(SeerrProbe {
            version: "3.5.1".into(),
            user: "example-user".into(),
        }));
        assert_eq!(
            state.seerr.probe,
            Some(SeerrProbe {
                version: "3.5.1".into(),
                user: "example-user".into(),
            })
        );
        assert!(state.seerr.error.is_none());
        assert!(!state.seerr.testing);

        state.apply_seerr_probe(Err("nope".into()));
        assert!(state.seerr.probe.is_none());
        assert_eq!(state.seerr.error.as_deref(), Some("nope"));
    }

    #[test]
    fn clearing_the_key_empties_the_buffer_and_config() {
        let mut state = State::from_config(Config::default());
        state.seerr.key = TextEdit::new("secret").secure();
        state.config.seerr_api_key = Some("secret".into());
        state.seerr.probe = Some(SeerrProbe {
            version: "3.5.1".into(),
            user: "example-user".into(),
        });
        state.clear_seerr_key();
        assert!(state.seerr.key.text.is_empty());
        assert_eq!(state.config.seerr_api_key, None);
        assert!(state.seerr.probe.is_none());
    }

    #[test]
    fn save_preferences_writes_the_seerr_fields_from_the_buffers() {
        let mut state = State::from_config(Config::default());
        state.seerr.url = TextEdit::new("https://jellyseerr.example");
        state.seerr.key = TextEdit::new("secret");
        state.seerr.user = TextEdit::new("7");
        state.seerr.probe = Some(SeerrProbe {
            version: "3.5.1".into(),
            user: "example-user".into(),
        });
        state.save_preferences();
        assert_eq!(state.config.seerr_url, "https://jellyseerr.example");
        assert_eq!(state.config.seerr_api_key.as_deref(), Some("secret"));
        assert_eq!(state.config.seerr_user_id, Some(7));
        assert_eq!(
            state.config.seerr_user_name.as_deref(),
            Some("example-user")
        );
    }

    #[test]
    fn the_request_filters_default_to_active() {
        assert_eq!(RequestsFilter::default(), RequestsFilter::Active);
        assert_eq!(SeerrTab::Discover, SeerrTab::Discover);
    }

    fn request(id: u64, media_type: &str, tmdb_id: u64) -> MediaRequest {
        MediaRequest {
            id,
            status: 2,
            media_type: media_type.into(),
            is_4k: false,
            season_count: 0,
            seasons: Vec::new(),
            requested_by: None,
            media: seerr_api::MediaInfo {
                tmdb_id: Some(tmdb_id),
                media_type: media_type.into(),
                ..seerr_api::MediaInfo::default()
            },
            can_remove: false,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn summaries_skip_cached_keys_and_deduplicate() {
        let requests = vec![
            request(1, "movie", 10),
            request(2, "movie", 10),
            request(3, "tv", 20),
            request(4, "movie", 30),
        ];
        let mut summaries = HashMap::new();
        summaries.insert(
            ("tv".into(), 20),
            MediaSummary {
                title: "Cached".into(),
                year: None,
                poster_path: None,
                backdrop_path: None,
                overview: None,
                runtime: None,
                number_of_seasons: None,
                seasons: Vec::new(),
            },
        );
        let keys = missing_summary_keys(&requests, &summaries);
        assert_eq!(
            keys,
            vec![("movie".into(), 10), ("movie".into(), 30)],
            "cached and duplicate keys are skipped"
        );
    }

    #[test]
    fn missing_seasons_skips_specials_and_present_seasons() {
        let tv = seerr_api::TvDetails {
            id: 1,
            name: Some("Example".into()),
            first_air_date: None,
            number_of_seasons: Some(4),
            seasons: vec![
                seerr_api::Season {
                    season_number: 0,
                    name: None,
                    episode_count: 2,
                    air_date: None,
                },
                seerr_api::Season {
                    season_number: 1,
                    name: None,
                    episode_count: 8,
                    air_date: None,
                },
                seerr_api::Season {
                    season_number: 2,
                    name: None,
                    episode_count: 8,
                    air_date: None,
                },
                seerr_api::Season {
                    season_number: 3,
                    name: None,
                    episode_count: 8,
                    air_date: None,
                },
            ],
            poster_path: None,
            backdrop_path: None,
            overview: None,
            vote_average: None,
            tagline: None,
            genres: Vec::new(),
            credits: Default::default(),
            media_info: None,
        };
        let present = HashSet::from([1, 2]);
        assert_eq!(missing_seasons(&present, &tv), vec![3]);
    }
}
