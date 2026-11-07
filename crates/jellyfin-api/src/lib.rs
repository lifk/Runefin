//! A typed, blocking Jellyfin REST client.
//!
//! Coverage is intentional: what a *browsing* client needs, plus the three playback-reporting
//! calls that make another player's play show up in the server's Continue Watching. There is no
//! transcoding negotiation here, because no player of ours ever asks for a transcode.
//!
//! ```no_run
//! use jellyfin_api::{Client, ItemQuery};
//!
//! let client = Client::new("192.0.2.10:8096", "device-id");
//! let info = client.public_system_info()?;
//! println!("connected to {}", info.server_name);
//! let auth = client.authenticate("luis", "…")?;
//! let client = client.with_session(auth.access_token, auth.user.id);
//! let views = client.libraries()?;
//! let page = client.items(&ItemQuery::in_library(&views[0].id))?;
//! # Ok::<(), jellyfin_api::Error>(())
//! ```

pub mod client;
pub mod models;

pub use client::{Client, Error, ItemQuery, LiveStream, PlaybackInfoRequest, normalise_base_url};
pub use models::{
    AudioTrack, AuthResult, ChapterInfo, CurrentProgram, ImageKind, Item, ItemsContainer,
    ItemsResult, LibraryView, MediaSegment, MediaSource, MediaStream, NameGuid, Person,
    PlaybackInfoResponse, PlaybackMediaSource, PlaybackReport, PlaybackStop, PlayerState,
    PublicSystemInfo, QuickConnectState, RemoteSubtitle, RemoteTrailer, SearchHint, SessionInfo,
    SortBy, SubtitleTrack, TICKS_PER_MS, ThemeMedia, TrickplayInfo, User, UserData,
    iso_from_seconds, iso_seconds,
};
