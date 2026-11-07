//! A typed, blocking Seerr/Jellyseerr REST client.
//!
//! Coverage is the request flow: prove the instance, read the catalogue, create a request, and
//! read the request/download rows. Like `jellyfin-api`, there is no async runtime here; the app
//! runs these calls on its own worker.
//!
//! ```no_run
//! use seerr_api::Client;
//!
//! let client = Client::new("http://198.51.100.2:5055", "api-key");
//! let status = client.status()?;
//! println!("connected to Seerr {}", status.version);
//! let trending = client.discover_trending(1)?;
//! # Ok::<(), seerr_api::Error>(())
//! ```

pub mod client;
pub mod models;

pub use client::{Client, Error, fetch_image, normalise_base_url};
pub use models::{
    CastMember, CreateRequest, Credit, Credits, Details, DiscoverItem, DiscoverQuery,
    DownloadingItem, EpisodeRef, Genre, MediaInfo, MediaRequest, MediaStatus, MediaSummary,
    MovieDetails, PageInfo, Paginated, PersonCredits, PersonDetails, PublicSettings, RequestCounts,
    RequestSeason, RequestStatus, SearchResults, Season, Seasons, Status, TvDetails, User,
    aggregate, all_completed,
};
