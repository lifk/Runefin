//! Wire types for the Seerr/Jellyseerr v1 API.
//!
//! The published OpenAPI is behind the runtime: `downloadStatus`, `mediaUrl`, `jellyfinMediaId`
//! and the five-value request status are all real. Every field the UI does not strictly need is
//! optional, and unknown status numbers become `Unknown` instead of failing the page.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// `GET /status`. Public.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Status {
    #[serde(default)]
    pub version: String,
    #[serde(default, rename = "commitTag")]
    pub commit_tag: Option<String>,
    #[serde(default, rename = "updateAvailable")]
    pub update_available: Option<bool>,
    #[serde(default, rename = "commitsBehind")]
    pub commits_behind: Option<i64>,
    #[serde(default, rename = "restartRequired")]
    pub restart_required: Option<bool>,
}

/// `GET /settings/public`. Public; the flags the request UI needs.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PublicSettings {
    #[serde(default)]
    pub initialized: bool,
    #[serde(default, rename = "applicationTitle")]
    pub application_title: Option<String>,
    #[serde(default, rename = "mediaServerType")]
    pub media_server_type: Option<i64>,
    #[serde(default, rename = "partialRequestsEnabled")]
    pub partial_requests_enabled: Option<bool>,
    #[serde(default, rename = "enableSpecialEpisodes")]
    pub enable_special_episodes: Option<bool>,
    #[serde(default, rename = "movie4kEnabled")]
    pub movie_4k_enabled: Option<bool>,
    #[serde(default, rename = "series4kEnabled")]
    pub series_4k_enabled: Option<bool>,
    #[serde(default, rename = "hideAvailable")]
    pub hide_available: Option<bool>,
    #[serde(default, rename = "hideRequested")]
    pub hide_requested: Option<bool>,
    #[serde(default, rename = "localLogin")]
    pub local_login: Option<bool>,
    #[serde(default, rename = "mediaServerLogin")]
    pub media_server_login: Option<bool>,
}

/// `GET /auth/me` and the `requestedBy`/`modifiedBy` rows. `username` is often null on a
/// Jellyfin-backed instance; the readable name is `displayName` (or `jellyfinUsername`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct User {
    pub id: i64,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default, rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(default, rename = "jellyfinUsername")]
    pub jellyfin_username: Option<String>,
    #[serde(default)]
    pub permissions: i64,
    #[serde(default, rename = "requestCount")]
    pub request_count: i64,
}

impl User {
    /// The name to show: display name, Jellyfin name, username, email, then a neutral fallback.
    pub fn display_name(&self) -> &str {
        self.display_name
            .as_deref()
            .or(self.jellyfin_username.as_deref())
            .or(self.username.as_deref())
            .or(self.email.as_deref())
            .unwrap_or("Unknown")
    }
}

/// `pageInfo` on `/request` and `/media`. `results` is the **total** count, not the page length.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct PageInfo {
    #[serde(default)]
    pub page: i64,
    #[serde(default)]
    pub pages: i64,
    #[serde(default)]
    pub results: i64,
    #[serde(default, rename = "pageSize")]
    pub page_size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Paginated<T> {
    #[serde(default, rename = "pageInfo")]
    pub page_info: PageInfo,
    #[serde(default)]
    pub results: Vec<T>,
}

/// `/search` and `/discover/*` use `page`/`totalPages`/`totalResults`, not `pageInfo`.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct SearchResults {
    #[serde(default)]
    pub page: i64,
    #[serde(default, rename = "totalPages")]
    pub total_pages: i64,
    #[serde(default, rename = "totalResults")]
    pub total_results: i64,
    #[serde(default)]
    pub results: Vec<DiscoverItem>,
}

/// A search/discover row. Movies use `title`/`releaseDate`; TV uses `name`/`firstAirDate`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DiscoverItem {
    pub id: u64,
    #[serde(default, rename = "mediaType")]
    pub media_type: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "releaseDate")]
    pub release_date: Option<String>,
    #[serde(default, rename = "firstAirDate")]
    pub first_air_date: Option<String>,
    #[serde(default, rename = "posterPath")]
    pub poster_path: Option<String>,
    #[serde(default, rename = "backdropPath")]
    pub backdrop_path: Option<String>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default, rename = "voteAverage")]
    pub vote_average: Option<f64>,
    #[serde(default, rename = "mediaInfo")]
    pub media_info: Option<MediaInfo>,
}

impl DiscoverItem {
    pub fn title(&self) -> &str {
        self.title
            .as_deref()
            .or(self.name.as_deref())
            .unwrap_or("Untitled")
    }

    pub fn year(&self) -> Option<&str> {
        self.release_date
            .as_deref()
            .or(self.first_air_date.as_deref())
            .and_then(|date| date.get(..4))
    }

    pub fn is_movie(&self) -> bool {
        self.media_type == "movie"
    }

    pub fn is_tv(&self) -> bool {
        self.media_type == "tv"
    }
}

/// `/movie/{tmdbId}`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MovieDetails {
    pub id: u64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, rename = "releaseDate")]
    pub release_date: Option<String>,
    #[serde(default)]
    pub runtime: Option<i64>,
    #[serde(default, rename = "posterPath")]
    pub poster_path: Option<String>,
    #[serde(default, rename = "backdropPath")]
    pub backdrop_path: Option<String>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default, rename = "voteAverage")]
    pub vote_average: Option<f64>,
    #[serde(default)]
    pub tagline: Option<String>,
    #[serde(default)]
    pub genres: Vec<Genre>,
    #[serde(default)]
    pub credits: Credits,
    #[serde(default, rename = "mediaInfo")]
    pub media_info: Option<MediaInfo>,
}

impl MovieDetails {
    pub fn year(&self) -> Option<&str> {
        self.release_date.as_deref().and_then(|date| date.get(..4))
    }
}

/// `/tv/{tmdbId}`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TvDetails {
    pub id: u64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "firstAirDate")]
    pub first_air_date: Option<String>,
    #[serde(default, rename = "numberOfSeasons")]
    pub number_of_seasons: Option<i64>,
    #[serde(default)]
    pub seasons: Vec<Season>,
    #[serde(default, rename = "posterPath")]
    pub poster_path: Option<String>,
    #[serde(default, rename = "backdropPath")]
    pub backdrop_path: Option<String>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default, rename = "voteAverage")]
    pub vote_average: Option<f64>,
    #[serde(default)]
    pub tagline: Option<String>,
    #[serde(default)]
    pub genres: Vec<Genre>,
    #[serde(default)]
    pub credits: Credits,
    #[serde(default, rename = "mediaInfo")]
    pub media_info: Option<MediaInfo>,
}

impl TvDetails {
    pub fn year(&self) -> Option<&str> {
        self.first_air_date
            .as_deref()
            .and_then(|date| date.get(..4))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Season {
    #[serde(default, rename = "seasonNumber")]
    pub season_number: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "episodeCount")]
    pub episode_count: i64,
    #[serde(default, rename = "airDate")]
    pub air_date: Option<String>,
}

/// A movie or a series detail, for one screen that has to draw both.
#[derive(Debug, Clone, PartialEq)]
pub enum Details {
    Movie(MovieDetails),
    Tv(TvDetails),
}

impl Details {
    pub fn title(&self) -> &str {
        match self {
            Self::Movie(movie) => movie.title.as_deref().unwrap_or("Untitled"),
            Self::Tv(tv) => tv.name.as_deref().unwrap_or("Untitled"),
        }
    }

    pub fn year(&self) -> Option<&str> {
        match self {
            Self::Movie(movie) => movie.year(),
            Self::Tv(tv) => tv.year(),
        }
    }

    pub fn poster_path(&self) -> Option<&str> {
        match self {
            Self::Movie(movie) => movie.poster_path.as_deref(),
            Self::Tv(tv) => tv.poster_path.as_deref(),
        }
    }

    pub fn backdrop_path(&self) -> Option<&str> {
        match self {
            Self::Movie(movie) => movie.backdrop_path.as_deref(),
            Self::Tv(tv) => tv.backdrop_path.as_deref(),
        }
    }

    pub fn overview(&self) -> Option<&str> {
        match self {
            Self::Movie(movie) => movie.overview.as_deref(),
            Self::Tv(tv) => tv.overview.as_deref(),
        }
    }

    pub fn media_info(&self) -> Option<&MediaInfo> {
        match self {
            Self::Movie(movie) => movie.media_info.as_ref(),
            Self::Tv(tv) => tv.media_info.as_ref(),
        }
    }

    pub fn tagline(&self) -> Option<&str> {
        match self {
            Self::Movie(movie) => movie.tagline.as_deref(),
            Self::Tv(tv) => tv.tagline.as_deref(),
        }
    }

    pub fn genres(&self) -> &[Genre] {
        match self {
            Self::Movie(movie) => &movie.genres,
            Self::Tv(tv) => &tv.genres,
        }
    }

    pub fn credits(&self) -> &Credits {
        match self {
            Self::Movie(movie) => &movie.credits,
            Self::Tv(tv) => &tv.credits,
        }
    }

    pub fn seasons(&self) -> &[Season] {
        match self {
            Self::Movie(_) => &[],
            Self::Tv(tv) => &tv.seasons,
        }
    }

    pub fn is_movie(&self) -> bool {
        matches!(self, Self::Movie(_))
    }

    pub fn summary(&self) -> MediaSummary {
        match self {
            Self::Movie(movie) => MediaSummary::from(movie.clone()),
            Self::Tv(tv) => MediaSummary::from(tv.clone()),
        }
    }
}

/// The title/poster/year the Requests list needs. `GET /request` has none of it; the list fetches
/// this from `/movie` or `/tv` and caches it by `(media_type, tmdb_id)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaSummary {
    pub title: String,
    pub year: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub overview: Option<String>,
    pub runtime: Option<i64>,
    pub number_of_seasons: Option<i64>,
    pub seasons: Vec<Season>,
}

impl From<MovieDetails> for MediaSummary {
    fn from(movie: MovieDetails) -> Self {
        Self {
            title: movie.title.unwrap_or_else(|| "Untitled".into()),
            year: movie
                .release_date
                .and_then(|date| date.get(..4).map(str::to_string)),
            poster_path: movie.poster_path,
            backdrop_path: movie.backdrop_path,
            overview: movie.overview,
            runtime: movie.runtime,
            number_of_seasons: None,
            seasons: Vec::new(),
        }
    }
}

impl From<TvDetails> for MediaSummary {
    fn from(tv: TvDetails) -> Self {
        Self {
            title: tv.name.unwrap_or_else(|| "Untitled".into()),
            year: tv
                .first_air_date
                .and_then(|date| date.get(..4).map(str::to_string)),
            poster_path: tv.poster_path,
            backdrop_path: tv.backdrop_path,
            overview: tv.overview,
            runtime: None,
            number_of_seasons: tv.number_of_seasons,
            seasons: tv.seasons,
        }
    }
}

/// A `GET /request` row. It has **no title, poster or year**; fetch `/movie` or `/tv` for those.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct MediaRequest {
    pub id: u64,
    #[serde(default)]
    pub status: i64,
    #[serde(default, rename = "type")]
    pub media_type: String,
    #[serde(default)]
    pub is_4k: bool,
    #[serde(default, rename = "seasonCount")]
    pub season_count: i64,
    #[serde(default)]
    pub seasons: Vec<RequestSeason>,
    #[serde(default, rename = "requestedBy")]
    pub requested_by: Option<User>,
    #[serde(default)]
    pub media: MediaInfo,
    #[serde(default, rename = "canRemove")]
    pub can_remove: bool,
    #[serde(default, rename = "createdAt")]
    pub created_at: Option<String>,
    #[serde(default, rename = "updatedAt")]
    pub updated_at: Option<String>,
}

impl MediaRequest {
    pub fn request_status(&self) -> RequestStatus {
        RequestStatus::from_i64(self.status)
    }

    pub fn media_status(&self) -> MediaStatus {
        self.media.media_status()
    }

    pub fn is_movie(&self) -> bool {
        self.media_type == "movie"
    }

    pub fn is_tv(&self) -> bool {
        self.media_type == "tv"
    }

    /// The row's progress line: a completed season pack says "Downloaded", a request with no
    /// queue rows says "Waiting for a download", and a real queue shows a percentage.
    pub fn progress_label(&self) -> String {
        if self.request_status() == RequestStatus::Pending {
            return "Awaiting approval".into();
        }
        match self.media_status() {
            MediaStatus::Available => return "Available".into(),
            MediaStatus::Deleted => return "Removed from the library".into(),
            MediaStatus::Blocklisted => return "Not requestable".into(),
            _ => {}
        }
        let downloads = &self.media.download_status;
        if downloads.is_empty() {
            return "Waiting for a download".into();
        }
        let (total, left, percent) = aggregate(downloads);
        if all_completed(downloads) {
            if self.media_status() == MediaStatus::Available {
                "Available".into()
            } else {
                "Downloaded · waiting for the library".into()
            }
        } else if let Some(percent) = percent {
            format!(
                "{}% · {:.1} of {:.1} GB",
                (percent * 100.0).round(),
                (total - left) as f64 / 1_000_000_000.0,
                total as f64 / 1_000_000_000.0
            )
        } else {
            "Downloading".into()
        }
    }

    pub fn progress_fraction(&self) -> Option<f32> {
        if self.request_status() == RequestStatus::Pending || self.media.download_status.is_empty()
        {
            return None;
        }
        aggregate(&self.media.download_status)
            .2
            .map(|percent| percent as f32)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RequestSeason {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default, rename = "seasonNumber")]
    pub season_number: i64,
}

/// The `media` object on a request or a catalogue result. It is the only place the status and
/// download rows live; titles and artwork are not here.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct MediaInfo {
    #[serde(default)]
    pub id: i64,
    #[serde(default, rename = "tmdbId")]
    pub tmdb_id: Option<u64>,
    #[serde(default, rename = "tvdbId")]
    pub tvdb_id: Option<u64>,
    #[serde(default, rename = "mediaType")]
    pub media_type: String,
    #[serde(default)]
    pub status: i64,
    #[serde(default, rename = "status4k")]
    pub status_4k: i64,
    #[serde(default, rename = "downloadStatus")]
    pub download_status: Vec<DownloadingItem>,
    #[serde(default, rename = "downloadStatus4k")]
    pub download_status_4k: Vec<DownloadingItem>,
    #[serde(default, rename = "jellyfinMediaId")]
    pub jellyfin_media_id: Option<String>,
    #[serde(default, rename = "mediaUrl")]
    pub media_url: Option<String>,
    #[serde(default, rename = "serviceUrl")]
    pub service_url: Option<String>,
}

impl MediaInfo {
    pub fn media_status(&self) -> MediaStatus {
        MediaStatus::from_i64(self.status)
    }

    pub fn is_available(&self) -> bool {
        self.media_status().is_available()
    }
}

/// One download. A season pack repeats the same `download_id` and `size` on every episode row;
/// group by `download_id` before summing (see [`aggregate`]).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DownloadingItem {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default, rename = "sizeLeft")]
    pub size_left: u64,
    #[serde(default, rename = "timeLeft")]
    pub time_left: Option<String>,
    #[serde(default, rename = "estimatedCompletionTime")]
    pub estimated_completion_time: Option<String>,
    #[serde(default, rename = "downloadId")]
    pub download_id: String,
    #[serde(default, rename = "mediaType")]
    pub media_type: Option<String>,
    #[serde(default, rename = "externalId")]
    pub external_id: Option<i64>,
    #[serde(default)]
    pub episode: Option<EpisodeRef>,
}

impl DownloadingItem {
    /// `None` when the size is unknown or the numbers are inconsistent.
    pub fn percent(&self) -> Option<f64> {
        (self.size > 0 && self.size_left <= self.size)
            .then(|| (self.size - self.size_left) as f64 / self.size as f64)
    }

    pub fn is_completed(&self) -> bool {
        self.status == "completed" || (self.size > 0 && self.size_left == 0)
    }
}

/// A TV download row's episode tag. The live object carries many more fields; only these are
/// needed, and the rest are ignored.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EpisodeRef {
    #[serde(default, rename = "seasonNumber")]
    pub season_number: i64,
    #[serde(default, rename = "episodeNumber")]
    pub episode_number: i64,
    #[serde(default, rename = "absoluteEpisodeNumber")]
    pub absolute_episode_number: Option<i64>,
    #[serde(default)]
    pub id: Option<i64>,
}

impl EpisodeRef {
    pub fn label(&self) -> String {
        format!("S{:02}E{:02}", self.season_number, self.episode_number)
    }
}

/// One row per unique download. Rows with an empty `downloadId` are kept individually.
fn unique_downloads(items: &[DownloadingItem]) -> Vec<&DownloadingItem> {
    let mut seen = HashSet::new();
    items
        .iter()
        .filter(|item| item.download_id.is_empty() || seen.insert(item.download_id.as_str()))
        .collect()
}

/// Byte totals and percentage over unique downloads, never over episode rows and never an average
/// of percentages.
pub fn aggregate(items: &[DownloadingItem]) -> (u64, u64, Option<f64>) {
    let unique = unique_downloads(items);
    let total: u64 = unique.iter().map(|item| item.size).sum();
    let left: u64 = unique.iter().map(|item| item.size_left).sum();
    let percent = (total > 0 && left <= total).then(|| (total - left) as f64 / total as f64);
    (total, left, percent)
}

/// True when there is at least one row and every unique download is finished.
pub fn all_completed(items: &[DownloadingItem]) -> bool {
    let unique = unique_downloads(items);
    !unique.is_empty() && unique.iter().all(|item| item.is_completed())
}

/// `GET /request/count`. There is no `failed` field.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct RequestCounts {
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub movie: i64,
    #[serde(default)]
    pub tv: i64,
    #[serde(default)]
    pub pending: i64,
    #[serde(default)]
    pub approved: i64,
    #[serde(default)]
    pub declined: i64,
    #[serde(default)]
    pub processing: i64,
    #[serde(default)]
    pub available: i64,
    #[serde(default)]
    pub completed: i64,
}

impl RequestCounts {
    pub fn active(&self) -> i64 {
        self.pending + self.processing
    }
}

/// `POST /request` body. `is4k` is omitted in v1; TV sends `seasons: "all"` or a sorted list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreateRequest {
    #[serde(rename = "mediaType")]
    pub media_type: String,
    #[serde(rename = "mediaId")]
    pub media_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seasons: Option<Seasons>,
}

impl CreateRequest {
    pub fn movie(tmdb_id: u64) -> Self {
        Self {
            media_type: "movie".into(),
            media_id: tmdb_id,
            seasons: None,
        }
    }

    pub fn tv(tmdb_id: u64, seasons: Seasons) -> Self {
        Self {
            media_type: "tv".into(),
            media_id: tmdb_id,
            seasons: Some(seasons),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Seasons {
    All(String),
    Numbers(Vec<i64>),
}

impl Seasons {
    pub fn all() -> Self {
        Self::All("all".into())
    }

    /// Sorted and deduplicated, so the body is stable.
    pub fn numbers(numbers: impl IntoIterator<Item = i64>) -> Self {
        let mut numbers: Vec<i64> = numbers.into_iter().collect();
        numbers.sort_unstable();
        numbers.dedup();
        Self::Numbers(numbers)
    }
}

/// `MediaStatus` from `server/constants/media.ts`. The OpenAPI's description is stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaStatus {
    Unknown,
    Pending,
    Processing,
    PartiallyAvailable,
    Available,
    Blocklisted,
    Deleted,
}

impl MediaStatus {
    pub fn from_i64(value: i64) -> Self {
        match value {
            2 => Self::Pending,
            3 => Self::Processing,
            4 => Self::PartiallyAvailable,
            5 => Self::Available,
            6 => Self::Blocklisted,
            7 => Self::Deleted,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Pending => "Requested",
            Self::Processing => "Processing",
            Self::PartiallyAvailable => "Partially available",
            Self::Available => "Available",
            Self::Blocklisted => "Not requestable",
            Self::Deleted => "Removed from the library",
        }
    }

    pub fn is_available(self) -> bool {
        matches!(self, Self::Available | Self::PartiallyAvailable)
    }
}

/// `MediaRequestStatus` from `server/constants/media.ts`; the OpenAPI documents only 1–3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestStatus {
    Unknown,
    Pending,
    Approved,
    Declined,
    Failed,
    Completed,
}

impl RequestStatus {
    pub fn from_i64(value: i64) -> Self {
        match value {
            1 => Self::Pending,
            2 => Self::Approved,
            3 => Self::Declined,
            4 => Self::Failed,
            5 => Self::Completed,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Pending => "Awaiting approval",
            Self::Approved => "Approved",
            Self::Declined => "Declined",
            Self::Failed => "Failed",
            Self::Completed => "Completed",
        }
    }
}

/// A filtered `/discover` query. `studio` applies to films and `network` to series; the date
/// window uses the media type's own field, which the client supplies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoverQuery {
    pub page: u32,
    pub genre: Option<i64>,
    pub studio: Option<i64>,
    pub network: Option<i64>,
    pub sort_by: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}

impl DiscoverQuery {
    pub fn new(page: u32) -> Self {
        Self {
            page,
            genre: None,
            studio: None,
            network: None,
            sort_by: None,
            from: None,
            to: None,
        }
    }

    pub fn query_string(&self, date_field: &str) -> String {
        let mut parts = vec![format!("page={}", self.page)];
        if let Some(genre) = self.genre {
            parts.push(format!("genre={genre}"));
        }
        if let Some(studio) = self.studio {
            parts.push(format!("studio={studio}"));
        }
        if let Some(network) = self.network {
            parts.push(format!("network={network}"));
        }
        if let Some(sort_by) = &self.sort_by {
            parts.push(format!("sortBy={sort_by}"));
        }
        if let Some(from) = &self.from {
            parts.push(format!("{date_field}Gte={from}"));
        }
        if let Some(to) = &self.to {
            parts.push(format!("{date_field}Lte={to}"));
        }
        parts.join("&")
    }
}

/// One genre, shared by the movie and series details.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Genre {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub name: Option<String>,
}

/// The `credits` block a movie or series detail carries. `cast` is ordered by billing; `crew`
/// is every credited job, so a director is a row with `job == "Director"`.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct Credits {
    #[serde(default)]
    pub cast: Vec<CastMember>,
    #[serde(default)]
    pub crew: Vec<CastMember>,
}

impl Credits {
    pub fn director(&self) -> Option<&str> {
        self.crew
            .iter()
            .find(|member| member.job.as_deref() == Some("Director"))
            .and_then(|member| member.name.as_deref())
    }

    pub fn creator(&self) -> Option<&str> {
        self.crew
            .iter()
            .find(|member| member.job.as_deref() == Some("Creator"))
            .and_then(|member| member.name.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CastMember {
    pub id: u64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub character: Option<String>,
    #[serde(default)]
    pub job: Option<String>,
    #[serde(default, rename = "profilePath")]
    pub profile_path: Option<String>,
}

/// `/person/{tmdbId}`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PersonDetails {
    pub id: u64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub biography: Option<String>,
    #[serde(default)]
    pub birthday: Option<String>,
    #[serde(default)]
    pub deathday: Option<String>,
    #[serde(default, rename = "placeOfBirth")]
    pub place_of_birth: Option<String>,
    #[serde(default, rename = "profilePath")]
    pub profile_path: Option<String>,
    #[serde(default, rename = "knownForDepartment")]
    pub known_for_department: Option<String>,
}

/// One row of `/person/{tmdbId}/combined_credits`. A movie carries `title`/`releaseDate`, a
/// series `name`/`firstAirDate`; `character` is the role.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Credit {
    pub id: u64,
    #[serde(default, rename = "mediaType")]
    pub media_type: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub character: Option<String>,
    #[serde(default, rename = "releaseDate")]
    pub release_date: Option<String>,
    #[serde(default, rename = "firstAirDate")]
    pub first_air_date: Option<String>,
    #[serde(default, rename = "posterPath")]
    pub poster_path: Option<String>,
    #[serde(default, rename = "voteAverage")]
    pub vote_average: Option<f64>,
    #[serde(default, rename = "episodeCount")]
    pub episode_count: Option<i64>,
}

impl Credit {
    pub fn is_movie(&self) -> bool {
        self.media_type.as_deref() == Some("movie")
    }

    pub fn is_tv(&self) -> bool {
        self.media_type.as_deref() == Some("tv")
    }

    pub fn title(&self) -> &str {
        self.title
            .as_deref()
            .or(self.name.as_deref())
            .unwrap_or("Untitled")
    }

    pub fn year(&self) -> Option<&str> {
        self.release_date
            .as_deref()
            .or(self.first_air_date.as_deref())
            .and_then(|date| date.get(..4))
    }
}

/// `/person/{tmdbId}/combined_credits`.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct PersonCredits {
    #[serde(default)]
    pub cast: Vec<Credit>,
    #[serde(default)]
    pub crew: Vec<Credit>,
}

impl PersonCredits {
    /// Every credit, cast first, with a media type the app can open.
    pub fn all(&self) -> impl Iterator<Item = &Credit> {
        self.cast
            .iter()
            .chain(&self.crew)
            .filter(|credit| credit.is_movie() || credit.is_tv())
    }
}

#[cfg(test)]
mod tests;
