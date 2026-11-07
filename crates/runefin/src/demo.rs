//! Deterministic fixtures for snapshots and tests. No images and no network, so the geometry is
//! what gets frozen.

use std::collections::HashMap;
use std::time::Instant;

use seerr_api::{
    Details, DiscoverItem, DownloadingItem, EpisodeRef, MediaInfo, MediaRequest, MediaSummary,
    MovieDetails, RequestCounts, RequestSeason, SearchResults as SeerrSearchResults, Season,
    TvDetails,
};

use jellyfin_api::{
    ChapterInfo, Item, LibraryView, MediaSource, MediaStream, Person, PlayerState, SessionInfo,
    UserData,
};

use crate::detail::DetailState;
use crate::home::{HomeData, LatestRail};
use crate::library::LibraryState;
use crate::search::SearchResults;
use crate::state::{PlaybackSession, State, Step};

/// A deterministic home screen for snapshots and tests. Images stay absent on purpose: the
/// placeholder geometry is what these scenes freeze, and no test may touch the network.
pub fn home() -> HomeData {
    fn item(
        id: &str,
        name: &str,
        kind: &str,
        series: Option<(&str, &str)>,
        year: i32,
        watched: f64,
    ) -> Item {
        let (series_name, series_id) = match series {
            Some((name, id)) => (Some(name.to_string()), Some(id.to_string())),
            None => (None, None),
        };
        Item {
            id: id.to_string(),
            name: name.to_string(),
            kind: kind.to_string(),
            production_year: Some(year),
            series_name,
            series_id,
            user_data: Some(UserData {
                playback_position_ticks: 10_000 * 60_000,
                played_percentage: Some(watched),
                ..UserData::default()
            }),
            ..Item::default()
        }
    }

    fn episode(
        id: &str,
        name: &str,
        series: (&str, &str),
        season: i32,
        number: i32,
        watched: f64,
    ) -> Item {
        let mut item = item(id, name, "Episode", Some(series), 2025, watched);
        item.parent_index_number = Some(season);
        item.index_number = Some(number);
        item
    }

    let mut played_arrival = item("played-arrival", "Arrival", "Movie", None, 2016, 100.0);
    if let Some(user) = &mut played_arrival.user_data {
        user.played = true;
        user.playback_position_ticks = 0;
    }
    let mut played_shogun = episode(
        "played-shogun-s1e2",
        "Servants of Two Masters",
        ("Shōgun", "shogun"),
        1,
        2,
        100.0,
    );
    if let Some(user) = &mut played_shogun.user_data {
        user.played = true;
        user.playback_position_ticks = 0;
    }
    let mut played_last = episode(
        "played-last-s1e3",
        "Long, Long Time",
        ("The Last of Us", "last-of-us"),
        1,
        3,
        100.0,
    );
    if let Some(user) = &mut played_last.user_data {
        user.played = true;
        user.playback_position_ticks = 0;
    }

    HomeData {
        libraries: vec![
            LibraryView {
                id: "movies".into(),
                name: "Movies".into(),
                collection_type: Some("movies".into()),
            },
            LibraryView {
                id: "shows".into(),
                name: "Shows".into(),
                collection_type: Some("tvshows".into()),
            },
            LibraryView {
                id: "live-tv".into(),
                name: "Live TV".into(),
                collection_type: Some("livetv".into()),
            },
        ],
        resume: vec![
            episode(
                "severance-s2e4",
                "The After Hours",
                ("Severance", "severance"),
                2,
                4,
                65.0,
            ),
            episode(
                "the-bear-s3e2",
                "Next",
                ("The Bear", "the-bear"),
                3,
                2,
                18.0,
            ),
            item("dune-2", "Dune: Part Two", "Movie", None, 2024, 42.0),
            episode("andor-s2e8", "Who Are You?", ("Andor", "andor"), 2, 8, 80.0),
        ],
        next_up: vec![
            episode(
                "severance-s2e5",
                "Trojan's Horse",
                ("Severance", "severance"),
                2,
                5,
                0.0,
            ),
            episode(
                "the-bear-s3e3",
                "Doors",
                ("The Bear", "the-bear"),
                3,
                3,
                0.0,
            ),
            episode(
                "andor-s2e9",
                "Welcome to the Rebellion",
                ("Andor", "andor"),
                2,
                9,
                0.0,
            ),
            episode(
                "shogun-s1e2",
                "Servants of Two Masters",
                ("Shōgun", "shogun"),
                1,
                2,
                0.0,
            ),
        ],
        on_now: live_channels()
            .into_iter()
            .filter(|channel| channel.current_program.is_some())
            .collect(),
        collections: vec![
            Item {
                id: "ace-collection".into(),
                name: "Ace Ventura Collection".into(),
                kind: "BoxSet".into(),
                production_year: Some(1994),
                ..Item::default()
            },
            Item {
                id: "alien-collection".into(),
                name: "Alien Collection".into(),
                kind: "BoxSet".into(),
                production_year: Some(1979),
                ..Item::default()
            },
            Item {
                id: "spider-collection".into(),
                name: "The Amazing Spider-Man Collection".into(),
                kind: "BoxSet".into(),
                production_year: Some(2012),
                ..Item::default()
            },
            Item {
                id: "matrix-collection".into(),
                name: "The Matrix Collection".into(),
                kind: "BoxSet".into(),
                production_year: Some(1999),
                ..Item::default()
            },
        ],
        recently_played: vec![played_arrival, played_shogun, played_last],
        latest: vec![
            LatestRail {
                library: "Movies".into(),
                library_id: "movies".into(),
                items: vec![
                    item("past-lives", "Past Lives", "Movie", None, 2023, 0.0),
                    item(
                        "anatomy-fall",
                        "Anatomy of a Fall",
                        "Movie",
                        None,
                        2023,
                        0.0,
                    ),
                    item("the-substance", "The Substance", "Movie", None, 2024, 0.0),
                    item("perfect-days", "Perfect Days", "Movie", None, 2023, 0.0),
                    item("challengers", "Challengers", "Movie", None, 2024, 0.0),
                ],
            },
            LatestRail {
                library: "Shows".into(),
                library_id: "shows".into(),
                items: vec![
                    episode("shogun-s1e1", "Anjin", ("Shōgun", "shogun"), 1, 1, 0.0),
                    episode("fallout-s1e1", "The End", ("Fallout", "fallout"), 1, 1, 0.0),
                    episode(
                        "ripley-s1e1",
                        "I Am Legend",
                        ("Ripley", "ripley"),
                        1,
                        1,
                        0.0,
                    ),
                    episode(
                        "slow-horses-s4e1",
                        "Identity",
                        ("Slow Horses", "slow-horses"),
                        4,
                        1,
                        0.0,
                    ),
                    episode(
                        "the-bear-s3e1",
                        "Tomorrow",
                        ("The Bear", "the-bear-2"),
                        3,
                        1,
                        0.0,
                    ),
                ],
            },
        ],
    }
}

pub fn detail() -> DetailState {
    let seasons = vec![
        Item {
            id: "severance-s1".into(),
            name: "Season 1".into(),
            kind: "Season".into(),
            index_number: Some(1),
            ..Item::default()
        },
        Item {
            id: "severance-s2".into(),
            name: "Season 2".into(),
            kind: "Season".into(),
            index_number: Some(2),
            ..Item::default()
        },
    ];
    let episodes: Vec<Item> = (1..=4)
        .map(|number| Item {
            id: format!("severance-s2e{number}"),
            name: [
                "Hello, Ms. Cobel",
                "Goodbye, Mrs. Selvig",
                "Who Is Alive?",
                "Woe's Hollow",
            ][number - 1]
                .into(),
            kind: "Episode".into(),
            series_id: Some("severance".into()),
            series_name: Some("Severance".into()),
            parent_index_number: Some(2),
            index_number: Some(number as i32),
            production_year: Some(2025),
            premiere_date: Some(format!("2025-02-{:02}T00:00:00.0000000Z", number + 6)),
            run_time_ticks: Some(10_000 * 60_000 * 48),
            overview: Some(
                "Mark and his team return to the office after the events of the season one \
                 finale, with the severed floor's rules under review."
                    .into(),
            ),
            user_data: Some(UserData {
                playback_position_ticks: 10_000 * 60_000 * 12,
                played_percentage: Some(25.0),
                ..UserData::default()
            }),
            chapters: vec![
                ChapterInfo {
                    start_position_ticks: 0,
                    name: "Cold open".into(),
                    image_tag: Some("demo-chapter-1".into()),
                },
                ChapterInfo {
                    start_position_ticks: 10_000 * 60_000 * 5,
                    name: "The severed floor".into(),
                    image_tag: Some("demo-chapter-2".into()),
                },
                ChapterInfo {
                    start_position_ticks: 10_000 * 60_000 * 12,
                    name: "The break room".into(),
                    image_tag: Some("demo-chapter-3".into()),
                },
                ChapterInfo {
                    start_position_ticks: 10_000 * 60_000 * 24,
                    name: "The board".into(),
                    image_tag: Some("demo-chapter-4".into()),
                },
            ],
            ..Item::default()
        })
        .collect();
    let mut episodes = episodes;
    // The third episode is finished, so the list shows a check beside the progress rail.
    if let Some(episode) = episodes.get_mut(2) {
        episode.user_data = Some(UserData {
            played: true,
            ..UserData::default()
        });
    }
    let next_up = episodes.get(1).cloned();
    DetailState {
        item: Item {
            id: "severance".into(),
            name: "Severance".into(),
            kind: "Series".into(),
            production_year: Some(2025),
            official_rating: Some("TV-MA".into()),
            run_time_ticks: Some(10_000 * 60_000 * 48),
            overview: Some(
                "Mark leads a team of office workers whose memories have been surgically \
                 divided between their work and personal lives. When a mysterious colleague \
                 appears outside of work, it begins a journey to discover the truth about \
                 their jobs, their identities, and each other."
                    .into(),
            ),
            genres: Some(vec![
                "Drama".into(),
                "Mystery".into(),
                "Science Fiction".into(),
            ]),
            community_rating: Some(8.7),
            critic_rating: Some(96.0),
            provider_ids: [("Imdb".to_string(), "tt11280740".to_string())].into(),
            remote_trailers: vec![jellyfin_api::RemoteTrailer {
                url: Some("https://www.youtube.com/watch?v=xEQP4VVuyrY".into()),
                name: Some("Official Trailer".into()),
            }],
            image_tags: Some([("Primary".to_string(), "demo-poster".to_string())].into()),
            backdrop_image_tags: Some(vec!["demo-backdrop".into()]),
            people: vec![
                Person {
                    id: "adam-scott".into(),
                    name: "Adam Scott".into(),
                    role: Some("Mark Scout".into()),
                    kind: Some("Actor".into()),
                    ..Person::default()
                },
                Person {
                    id: "britt-lower".into(),
                    name: "Britt Lower".into(),
                    role: Some("Helly R.".into()),
                    kind: Some("Actor".into()),
                    ..Person::default()
                },
                Person {
                    id: "patricia-arquette".into(),
                    name: "Patricia Arquette".into(),
                    role: Some("Harmony Cobel".into()),
                    kind: Some("Actor".into()),
                    ..Person::default()
                },
                Person {
                    id: "john-turturro".into(),
                    name: "John Turturro".into(),
                    role: Some("Irving Bailiff".into()),
                    kind: Some("Actor".into()),
                    ..Person::default()
                },
                Person {
                    id: "zach-cherry".into(),
                    name: "Zach Cherry".into(),
                    role: Some("Dylan George".into()),
                    kind: Some("Actor".into()),
                    ..Person::default()
                },
                Person {
                    id: "dan-erickson".into(),
                    name: "Dan Erickson".into(),
                    role: Some("Creator".into()),
                    kind: Some("Writer".into()),
                    ..Person::default()
                },
            ],
            user_data: Some(UserData {
                playback_position_ticks: 10_000 * 2_533_000,
                played_percentage: Some(65.0),
                ..UserData::default()
            }),
            ..Item::default()
        },
        seasons,
        active_season: Some("severance-s2".into()),
        episodes,
        next_up,
        collection: Vec::new(),
        collection_sort: crate::detail::CollectionSort::default(),
        local_trailers: Vec::new(),
        similar: Vec::new(),
        loading: false,
        loading_episodes: false,
        error: None,
    }
}

#[cfg(any(test, debug_assertions))]
pub fn in_app() -> crate::state::InAppSession {
    let mut detail = detail();
    let item = detail.episodes.remove(1);
    crate::state::InAppSession {
        player: crate::player::Player::preview(),
        item_id: item.id.clone(),
        item,
        session_id: "demo-in-app".into(),
        position_ms: 42 * 60_000,
        duration_ms: Some(48 * 60_000),
        paused: false,
        scrub_ms: None,
        seek_hold: None,
        skipped_segments: std::collections::HashSet::new(),
        last_report: Instant::now(),
        live: false,
        live_stream_id: None,
        buffering: None,
        report: true,
        play_method: "DirectPlay",
        play_session_id: None,
        history_start_ms: 0,
    }
}

/// A trickplay manifest with the live server's geometry, at a small thumbnail size so a
/// snapshot can freeze the crop without a network fetch: 156 thumbnails in a 10×10 sprite.
#[cfg(any(test, debug_assertions))]
pub fn trickplay_manifest()
-> std::collections::HashMap<String, std::collections::HashMap<String, jellyfin_api::TrickplayInfo>>
{
    use jellyfin_api::TrickplayInfo;
    std::collections::HashMap::from([(
        "severance-s2e2".to_string(),
        std::collections::HashMap::from([(
            "640".to_string(),
            TrickplayInfo {
                width: 64,
                height: 36,
                tile_width: 10,
                tile_height: 10,
                thumbnail_count: 156,
                interval: 10_000,
                bandwidth: 1,
            },
        )]),
    )])
}

/// A 10×10 sprite of tiny landscapes. Each cell has its own hue and horizon, so a wrong crop
/// shows up as the wrong picture rather than as a plausible rectangle.
#[cfg(any(test, debug_assertions))]
pub fn trickplay_sprite(cache: &rune::ImageCache) -> rune::Image {
    const THUMB: (u32, u32) = (64, 36);
    let cells = 10;
    let (width, height) = (THUMB.0 * cells, THUMB.1 * cells);
    let mut bytes = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let cell = (y / THUMB.1) * cells + (x / THUMB.0);
            let u = (x % THUMB.0) as f32 / THUMB.0 as f32;
            let v = (y % THUMB.1) as f32 / THUMB.1 as f32;
            let t = cell as f32 / (cells * cells) as f32;
            let (r, g, b) = hue(t);
            let horizon = 0.62 + 0.08 * (t * 7.0).fract();
            // Sky, then a dark ridge, then a darker foreground: a frame, not a swatch.
            let (r, g, b) = if v < horizon {
                let shade = 0.45 + 0.55 * (1.0 - v / horizon);
                (r as f32 * shade, g as f32 * shade, b as f32 * shade)
            } else if v < horizon + 0.07 {
                (18.0, 17.0, 22.0)
            } else {
                let shade = 0.30 + 0.12 * (1.0 - v);
                (r as f32 * shade, g as f32 * shade, b as f32 * shade)
            };
            // A sun low on the horizon, moving across the sprite so every cell differs.
            let sun = (0.18 + 0.6 * (t * 11.0).fract(), horizon - 0.12);
            let distance = ((u - sun.0).powi(2) + ((v - sun.1) * 0.6).powi(2)).sqrt();
            let (r, g, b) = if distance < 0.075 {
                (255.0, 240.0, 205.0)
            } else {
                (r, g, b)
            };
            bytes.extend_from_slice(&[
                r.clamp(0.0, 255.0) as u8,
                g.clamp(0.0, 255.0) as u8,
                b.clamp(0.0, 255.0) as u8,
                255,
            ]);
        }
    }
    let image = cache.rgba(width, height, bytes);
    let _ = image.wait();
    image
}

#[cfg(any(test, debug_assertions))]
fn hue(t: f32) -> (u8, u8, u8) {
    let h = (t.fract() * 6.0).clamp(0.0, 5.999);
    let x = 1.0 - (h.fract() - 0.5).abs() * 2.0;
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// The episode after the demo playhead, for the up-next strip.
#[cfg(any(test, debug_assertions))]
pub fn next_episode() -> Item {
    Item {
        id: "severance-s2e3".into(),
        name: "Who Is Alive?".into(),
        kind: "Episode".into(),
        series_id: Some("severance".into()),
        series_name: Some("Severance".into()),
        parent_index_number: Some(2),
        index_number: Some(3),
        production_year: Some(2025),
        image_tags: Some(std::collections::HashMap::from([(
            "Primary".to_string(),
            "demo-still".to_string(),
        )])),
        ..Item::default()
    }
}

/// The episode before the demo one, for the player's previous-episode action.
#[cfg(any(test, debug_assertions))]
pub fn previous_episode() -> Item {
    Item {
        id: "severance-s2e1".into(),
        name: "Hello, Ms. Cobel".into(),
        kind: "Episode".into(),
        series_id: Some("severance".into()),
        series_name: Some("Severance".into()),
        parent_index_number: Some(2),
        index_number: Some(1),
        production_year: Some(2025),
        ..Item::default()
    }
}

/// A single 16:9 still for the up-next card: the same little landscape language as the
/// trickplay sprite, at card size and without a sprite grid.
#[cfg(any(test, debug_assertions))]
pub fn still(cache: &rune::ImageCache) -> rune::Image {
    const W: u32 = 320;
    const H: u32 = 180;
    let mut bytes = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            let u = x as f32 / W as f32;
            let v = y as f32 / H as f32;
            let horizon = 0.64;
            let (r, g, b) = if v < horizon {
                let shade = 0.42 + 0.58 * (1.0 - v / horizon);
                (86.0 * shade, 118.0 * shade, 168.0 * shade)
            } else {
                let shade = 0.30 + 0.14 * (1.0 - v);
                (40.0 * shade, 46.0 * shade, 54.0 * shade)
            };
            let sun = (0.62, horizon - 0.14);
            let distance = ((u - sun.0).powi(2) + ((v - sun.1) * 0.55).powi(2)).sqrt();
            let (r, g, b) = if distance < 0.09 {
                (255.0, 226.0, 176.0)
            } else {
                (r, g, b)
            };
            bytes.extend_from_slice(&[
                r.clamp(0.0, 255.0) as u8,
                g.clamp(0.0, 255.0) as u8,
                b.clamp(0.0, 255.0) as u8,
                255,
            ]);
        }
    }
    let image = cache.rgba(W, H, bytes);
    let _ = image.wait();
    image
}

/// A deliberately high-contrast synthetic image: a bright left half, a near-black right
/// half and fine vertical bands. Committed snapshots use it to test text over artwork without
/// putting personal library art in the repository.
#[cfg(any(test, debug_assertions))]
fn contrast_pixels(cache: &rune::ImageCache, width: u32, height: u32) -> rune::Image {
    let mut bytes = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let u = x as f32 / width as f32;
            let v = y as f32 / height as f32;
            let band = ((x / 7) % 2) as f32 * 0.06;
            let (r, g, b) = if u < 0.48 {
                // Warm white with a slow wash, then narrow grey stripes.
                let shade = 0.82 + 0.18 * (1.0 - v);
                (
                    250.0 * shade - band * 255.0,
                    246.0 * shade - band * 255.0,
                    228.0 * shade,
                )
            } else {
                // Deep navy/black; the same text is unreadable without a proper shadow.
                (
                    8.0 + 24.0 * v + band * 255.0,
                    16.0 + 34.0 * v,
                    30.0 + 54.0 * v,
                )
            };
            let (r, g, b) = if (u - 0.48).abs() < 0.012 || (v - 0.12).abs() < 0.008 {
                (210.0, 210.0, 210.0)
            } else {
                (r, g, b)
            };
            bytes.extend_from_slice(&[
                r.clamp(0.0, 255.0) as u8,
                g.clamp(0.0, 255.0) as u8,
                b.clamp(0.0, 255.0) as u8,
                255,
            ]);
        }
    }
    let image = cache.rgba(width, height, bytes);
    let _ = image.wait();
    image
}

#[cfg(any(test, debug_assertions))]
pub fn contrast_poster(cache: &rune::ImageCache) -> rune::Image {
    contrast_pixels(cache, 300, 450)
}

#[cfg(any(test, debug_assertions))]
pub fn contrast_backdrop(cache: &rune::ImageCache) -> rune::Image {
    contrast_pixels(cache, 1280, 720)
}

/// A Shows library: series cards, no streams on the page, so the header offers a server-side
/// episode shuffle instead of "Play all".
#[cfg(any(test, debug_assertions))]
pub fn shows_library() -> LibraryState {
    let mut library = LibraryState::new("shows".into(), "Shows".into(), Some("tvshows".into()));
    library.items = [
        "Severance",
        "The Bear",
        "Andor",
        "Shōgun",
        "Slow Horses",
        "Foundation",
    ]
    .into_iter()
    .enumerate()
    .map(|(index, name)| Item {
        id: format!("show-{index}"),
        name: name.into(),
        kind: "Series".into(),
        production_year: Some(2020 + index as i32),
        ..Item::default()
    })
    .collect();
    library.total = 6;
    library
}

/// Six recommendations for the movie detail page's "More like this" rail.
pub fn similar() -> Vec<Item> {
    [
        ("Dune: Part One", 2021),
        ("Blade Runner 2049", 2017),
        ("Arrival", 2016),
        ("Interstellar", 2014),
        ("Children of Men", 2006),
        ("The Creator", 2023),
    ]
    .into_iter()
    .map(|(name, year)| Item {
        id: format!("similar-{}", name.to_ascii_lowercase().replace(' ', "-")),
        name: name.into(),
        kind: "Movie".into(),
        production_year: Some(year),
        ..Item::default()
    })
    .collect()
}

/// A movie detail with a full media source, so the hero's technical line and the Details
/// disclosure have real facts to show.
pub fn movie_detail() -> DetailState {
    DetailState {
        item: Item {
            id: "dune-2".into(),
            name: "Dune: Part Two".into(),
            kind: "Movie".into(),
            production_year: Some(2024),
            official_rating: Some("PG-13".into()),
            run_time_ticks: Some(10_000 * 60_000 * 166),
            overview: Some(
                "Paul Atreides unites with the Fremen to wage war against the House Harkonnen, \
                 while trying to prevent a terrible future only he can foresee."
                    .into(),
            ),
            genres: Some(vec!["Science Fiction".into(), "Adventure".into()]),
            original_title: Some("Dune: Part Two".into()),
            premiere_date: Some("2024-02-27T00:00:00.0000000Z".into()),
            tags: vec!["IMAX".into(), "Epic".into()],
            studios: vec![jellyfin_api::NameGuid {
                id: "legendary".into(),
                name: "Legendary Pictures".into(),
            }],
            community_rating: Some(8.5),
            critic_rating: Some(92.0),
            provider_ids: [("Imdb".to_string(), "tt15239678".to_string())].into(),
            remote_trailers: vec![jellyfin_api::RemoteTrailer {
                url: Some("https://www.youtube.com/watch?v=Way9Dexny3w".into()),
                name: Some("Official Trailer".into()),
            }],
            // Twelve minutes in: the snapshot shows both Resume and Start over.
            user_data: Some(UserData {
                playback_position_ticks: 10_000 * 60_000 * 12,
                played_percentage: Some(7.2),
                ..UserData::default()
            }),
            media_sources: vec![
                MediaSource {
                    id: "dune-source".into(),
                    name: Some("4K HDR10".into()),
                    container: Some("mkv".into()),
                    size: Some(42_100_000_000),
                    bitrate: Some(38_000_000),
                    media_streams: vec![
                        MediaStream {
                            index: -1,
                            kind: "Video".into(),
                            codec: Some("hevc".into()),
                            width: Some(3840),
                            height: Some(2160),
                            video_range: Some("HDR10".into()),
                            profile: Some("Main 10".into()),
                            ..MediaStream::default()
                        },
                        MediaStream {
                            index: 1,
                            kind: "Audio".into(),
                            codec: Some("eac3".into()),
                            language: Some("eng".into()),
                            channels: Some(6),
                            display_title: Some("English - EAC3 - 5.1".into()),
                            ..MediaStream::default()
                        },
                    ],
                },
                MediaSource {
                    id: "dune-1080".into(),
                    name: Some("1080p".into()),
                    container: Some("mkv".into()),
                    size: Some(12_400_000_000),
                    bitrate: Some(8_000_000),
                    media_streams: Vec::new(),
                },
            ],
            ..Item::default()
        },
        seasons: Vec::new(),
        active_season: None,
        episodes: Vec::new(),
        next_up: None,
        collection: Vec::new(),
        collection_sort: crate::detail::CollectionSort::default(),
        local_trailers: Vec::new(),
        similar: similar(),
        loading: false,
        loading_episodes: false,
        error: None,
    }
}

/// Three special features, enough to show the grouped extras rail.
#[cfg(any(test, debug_assertions))]
pub fn extras() -> Vec<Item> {
    [
        ("extra-behind", "Building Arrakis", "BehindTheScenes", 8),
        ("extra-deleted", "The Spice Harvester", "DeletedScene", 4),
        ("extra-interview", "Denis Villeneuve", "Interview", 12),
    ]
    .into_iter()
    .map(|(id, name, kind, minutes)| Item {
        id: id.into(),
        name: name.into(),
        kind: "Video".into(),
        extra_type: Some(kind.into()),
        run_time_ticks: Some(10_000 * 60_000 * minutes),
        ..Item::default()
    })
    .collect()
}

/// The sidebar's playlists. Small on purpose: the live probe creates and deletes its own, so
/// these are the only deterministic playlists any screenshot can show.
#[cfg(any(test, debug_assertions))]
pub fn playlists() -> Vec<Item> {
    [
        ("playlist-marvel", "Marvel Marathon"),
        ("playlist-halloween", "Halloween 2025"),
        ("playlist-openings", "Anime Openings"),
    ]
    .into_iter()
    .map(|(id, name)| Item {
        id: id.into(),
        name: name.into(),
        kind: "Playlist".into(),
        ..Item::default()
    })
    .collect()
}

/// One playlist opened in the library screen: ordered entries, no sort, no filters.
#[cfg(any(test, debug_assertions))]
pub fn playlist_library() -> LibraryState {
    let mut library = LibraryState::new(
        "playlist-marvel".into(),
        "Marvel Marathon".into(),
        Some("playlists".into()),
    );
    library.items = library_items(6)
        .into_iter()
        .map(|mut item| {
            item.playlist_item_id = Some(item.id.clone());
            item
        })
        .collect();
    library.total = 6;
    library
}

/// A BoxSet detail page: six members, no stream of its own, and a hero that omits Play.
pub fn collection() -> DetailState {
    let members: Vec<Item> = [
        ("Ace Ventura: Pet Detective", 1994),
        ("Ace Ventura: When Nature Calls", 1995),
        ("The Cable Guy", 1996),
        ("Liar Liar", 1997),
        ("The Truman Show", 1998),
        ("Man on the Moon", 1999),
    ]
    .into_iter()
    .enumerate()
    .map(|(number, (name, year))| Item {
        id: format!("ace-{number}"),
        name: name.into(),
        kind: "Movie".into(),
        production_year: Some(year),
        collection_type: None,
        ..Item::default()
    })
    .collect();
    DetailState {
        item: Item {
            id: "ace-collection".into(),
            name: "Ace Ventura Collection".into(),
            kind: "BoxSet".into(),
            overview: Some(
                "Two pet detectives, one alarming haircut, and a run of 1990s comedies.".into(),
            ),
            ..Item::default()
        },
        seasons: Vec::new(),
        active_season: None,
        episodes: Vec::new(),
        next_up: None,
        collection: members,
        collection_sort: crate::detail::CollectionSort::default(),
        local_trailers: Vec::new(),
        similar: Vec::new(),
        loading: false,
        loading_episodes: false,
        error: None,
    }
}

/// A tuner's channels, with the guide's current program where the server has one.
pub fn live_channels() -> Vec<Item> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    [
        ("formula-1", "Formula 1", "1", Some("British Grand Prix")),
        ("cinemax", "Cinemax", "2", Some("Dune: Part Two")),
        ("hbo-hd", "HBO HD", "3", Some("The Last of Us")),
        ("showtime", "Showtime", "4", None),
    ]
    .into_iter()
    .map(|(id, name, number, program)| Item {
        id: id.into(),
        name: name.into(),
        kind: "TvChannel".into(),
        channel_number: Some(number.into()),
        current_program: program.map(|name| jellyfin_api::CurrentProgram {
            name: name.into(),
            start_date: Some(jellyfin_api::iso_from_seconds(now - 1_800)),
            end_date: Some(jellyfin_api::iso_from_seconds(now + 1_800)),
            ..jellyfin_api::CurrentProgram::default()
        }),
        ..Item::default()
    })
    .collect()
}

/// The guide's lookahead for the demo channels. Showtime has none, like a channel without EPG.
#[cfg(any(test, debug_assertions))]
pub fn live_next() -> std::collections::HashMap<String, Item> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    [
        ("formula-1", "Qualifying"),
        ("cinemax", "Scream"),
        ("hbo-hd", "House of the Dragon"),
    ]
    .into_iter()
    .map(|(id, name)| {
        (
            id.to_string(),
            Item {
                id: format!("{id}-next"),
                name: name.into(),
                kind: "Program".into(),
                channel_id: Some(id.into()),
                start_date: Some(jellyfin_api::iso_from_seconds(now + 1_800)),
                end_date: Some(jellyfin_api::iso_from_seconds(now + 3_600)),
                ..Item::default()
            },
        )
    })
    .collect()
}

/// Three hours of guide around now, so the grid's slices line up in a snapshot.
#[cfg(any(test, debug_assertions))]
pub fn live_guide() -> std::collections::HashMap<String, Vec<Item>> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    fn program(channel: &str, name: &str, now: i64, start: i64, end: i64) -> Item {
        Item {
            id: format!("{channel}-{start}"),
            name: name.into(),
            kind: "Program".into(),
            channel_id: Some(channel.into()),
            start_date: Some(jellyfin_api::iso_from_seconds(now + start)),
            end_date: Some(jellyfin_api::iso_from_seconds(now + end)),
            ..Item::default()
        }
    }
    std::collections::HashMap::from([
        (
            "formula-1".to_string(),
            vec![
                program("formula-1", "British Grand Prix", now, -600, 3_000),
                program("formula-1", "Qualifying", now, 3_000, 4_800),
                program("formula-1", "Practice", now, 4_800, 8_400),
            ],
        ),
        (
            "cinemax".to_string(),
            vec![
                program("cinemax", "Dune: Part Two", now, -1_800, 3_600),
                program("cinemax", "Scream", now, 3_600, 7_800),
            ],
        ),
        (
            "hbo-hd".to_string(),
            vec![
                program("hbo-hd", "The Last of Us", now, -900, 1_800),
                program("hbo-hd", "House of the Dragon", now, 1_800, 5_400),
            ],
        ),
    ])
}

/// The Seerr half of a person page: a few credits, one already in the library.
pub fn seerr_person_credits() -> Vec<seerr_api::Credit> {
    [
        (945961, "movie", "Alien: Romulus", "2024", "Rain"),
        (1020006, "movie", "Priscilla", "2023", "Priscilla"),
        (81349, "tv", "Devs", "2020", "Lyndon"),
        (429197, "movie", "Vice", "2018", "Lynne Vincent"),
        (929590, "movie", "Civil War", "2024", "Jessie"),
        (1019671, "movie", "Unlimited World", "2022", "Lars"),
    ]
    .into_iter()
    .map(|(id, media, title, year, character)| seerr_api::Credit {
        id,
        media_type: Some(media.into()),
        title: Some(title.into()),
        name: None,
        character: Some(character.into()),
        release_date: Some(format!("{year}-01-01")),
        first_air_date: None,
        poster_path: None,
        vote_average: None,
        episode_count: None,
    })
    .collect()
}

/// A person page with the demo movie credits, for the filmography snapshot.
pub fn person() -> crate::state::PersonPage {
    crate::state::PersonPage {
        id: "adam-scott".into(),
        name: "Adam Scott".into(),
        image_tag: None,
        profile_path: None,
        tmdb_id: None,
        biography: None,
        birthday: None,
        place_of_birth: None,
        items: search_results().movies,
        library_tmdb: Default::default(),
        seerr_credits: Vec::new(),
        loading: false,
        error: None,
        seerr_loading: false,
        seerr_error: None,
    }
}

/// A pending Quick Connect card for the connect-screen snapshot.
pub fn quick_login() -> crate::state::QuickLogin {
    crate::state::QuickLogin {
        server_url: "https://jellyfin.example".into(),
        server_name: "example".into(),
        version: "10.11.2".into(),
        code: "8F2Q9A".into(),
        secret: "demo-secret".into(),
        authenticated: false,
        exchanging: false,
        polling: false,
        poll_at: Instant::now(),
        error: None,
    }
}

/// A tuner session at 95 s, with no duration: the OSD must show LIVE, not a scrub bar.
#[cfg(any(test, debug_assertions))]
pub fn live_session() -> crate::state::InAppSession {
    let channel = live_channels().remove(0);
    crate::state::InAppSession {
        player: crate::player::Player::preview_at(95_000),
        item_id: channel.id.clone(),
        item: channel,
        session_id: "demo-live".into(),
        position_ms: 95_000,
        duration_ms: None,
        paused: false,
        scrub_ms: None,
        seek_hold: None,
        skipped_segments: std::collections::HashSet::new(),
        last_report: Instant::now(),
        live: true,
        live_stream_id: Some("demo-live-stream".into()),
        buffering: None,
        report: true,
        play_method: "DirectPlay",
        play_session_id: None,
        history_start_ms: 0,
    }
}

#[cfg(any(test, debug_assertions))]
pub fn audio_tracks() -> Vec<jellyfin_api::AudioTrack> {
    vec![
        jellyfin_api::AudioTrack {
            ordinal: 0,
            display_title: "Japanese - OPUS - Stereo".into(),
            language: Some("jpn".into()),
            codec: Some("opus".into()),
            channels: Some(2),
        },
        jellyfin_api::AudioTrack {
            ordinal: 1,
            display_title: "English - AAC - 5.1".into(),
            language: Some("eng".into()),
            codec: Some("aac".into()),
            channels: Some(6),
        },
    ]
}

#[cfg(any(test, debug_assertions))]
pub fn subtitle_tracks() -> Vec<jellyfin_api::SubtitleTrack> {
    vec![
        jellyfin_api::SubtitleTrack {
            index: 0,
            language: Some("eng".into()),
            display_title: "English - SUBRIP - External".into(),
            codec: Some("subrip".into()),
            is_text: true,
            is_external: true,
            is_forced: false,
        },
        jellyfin_api::SubtitleTrack {
            index: 3,
            language: None,
            display_title: "Undefined - Default - ASS".into(),
            codec: Some("ass".into()),
            is_text: true,
            is_external: false,
            is_forced: false,
        },
    ]
}

/// Online subtitle results, shaped like the live Open Subtitles answers: a hash match, a
/// well-rated retail rip, a lower-download alternate and a hearing-impaired one.
#[cfg(any(test, debug_assertions))]
pub fn remote_subtitles() -> Vec<jellyfin_api::RemoteSubtitle> {
    vec![
        jellyfin_api::RemoteSubtitle {
            id: "demo-sub-1".into(),
            name: "Severance.S02E04.1080p.WEB-DL.DDP5.1.H.264".into(),
            provider_name: Some("Open Subtitles".into()),
            format: Some("srt".into()),
            author: Some("tedi".into()),
            comment: Some("Retail NF".into()),
            download_count: Some(64_498),
            community_rating: Some(3.0),
            language: Some("eng".into()),
            is_hash_match: true,
            forced: false,
            hearing_impaired: false,
            ai_translated: false,
            machine_translated: false,
            frame_rate: Some(23.976),
        },
        jellyfin_api::RemoteSubtitle {
            id: "demo-sub-2".into(),
            name: "Severance S02E04 720p HDTV x264".into(),
            provider_name: Some("Open Subtitles".into()),
            format: Some("srt".into()),
            author: Some("obnoxiousowl".into()),
            comment: None,
            download_count: Some(28_444),
            community_rating: Some(4.5),
            language: Some("eng".into()),
            is_hash_match: false,
            forced: false,
            hearing_impaired: false,
            ai_translated: false,
            machine_translated: false,
            frame_rate: Some(23.98),
        },
        jellyfin_api::RemoteSubtitle {
            id: "demo-sub-3".into(),
            name: "severance.s02e04.hdtv".into(),
            provider_name: Some("Open Subtitles".into()),
            format: Some("srt".into()),
            author: Some("os-auto".into()),
            comment: None,
            download_count: Some(1_204),
            community_rating: Some(2.5),
            language: Some("eng".into()),
            is_hash_match: false,
            forced: false,
            hearing_impaired: true,
            ai_translated: false,
            machine_translated: false,
            frame_rate: None,
        },
    ]
}

/// Cues around the demo playhead (42:00), the second one with two lines so the overlay's
/// wrapping and centring are visible in the snapshot.
#[cfg(any(test, debug_assertions))]
pub fn subtitle_cues() -> Vec<crate::subtitles::Cue> {
    vec![
        crate::subtitles::Cue {
            start_ms: 41 * 60_000,
            end_ms: 41 * 60_000 + 58_000,
            text: "Previously on Severance…".into(),
            align: crate::subtitles::Align::BottomCenter,
            ..Default::default()
        },
        crate::subtitles::Cue {
            start_ms: 41 * 60_000 + 58_000,
            end_ms: 42 * 60_000 + 5_500,
            text: "The board has reviewed your request.\nYour request has been denied.".into(),
            align: crate::subtitles::Align::BottomCenter,
            ..Default::default()
        },
    ]
}

/// A fixed PGS-like bitmap for the image-subtitle snapshot. The asset is a transparent PNG with
/// white text, so the overlay's placement and alpha are what the picture proves.
#[cfg(any(test, debug_assertions))]
pub fn image_subtitle_style() -> Option<crate::image_subtitles::ImageSubtitle> {
    let bytes = include_bytes!("../assets/image-subtitle-demo.png");
    let image = ::image::load_from_memory(bytes).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    let bitmap =
        crate::image_subtitles::bitmap(640.0, 930.0, width as u16, height as u16, image.into_raw());
    Some(crate::image_subtitles::ImageSubtitle::from_bitmaps(
        1920,
        1080,
        vec![bitmap],
    ))
}

/// The same tracks plus a PGS one, for the panel's "image subtitles are listed but not drawn"
/// state.
#[cfg(any(test, debug_assertions))]
pub fn subtitle_tracks_with_image() -> Vec<jellyfin_api::SubtitleTrack> {
    let mut tracks = subtitle_tracks();
    tracks.push(jellyfin_api::SubtitleTrack {
        index: 5,
        language: Some("eng".into()),
        display_title: "English - PGS - External".into(),
        codec: Some("pgssub".into()),
        is_text: false,
        is_external: true,
        is_forced: false,
    });
    tracks.push(jellyfin_api::SubtitleTrack {
        index: 6,
        language: Some("eng".into()),
        display_title: "English - DVB - Broadcast".into(),
        codec: Some("dvbsub".into()),
        is_text: false,
        is_external: false,
        is_forced: false,
    });
    tracks
}

/// Three ASS events at once: a bottom line, a top title and a middle-left sign. The overlay
/// must show all three in their own places, not flatten them into one centred block.
#[cfg(any(test, debug_assertions))]
pub fn ass_cues() -> Vec<crate::subtitles::Cue> {
    use crate::subtitles::{Align, Cue};
    vec![
        Cue {
            start_ms: 41 * 60_000,
            end_ms: 42 * 60_000 + 30_000,
            text: "The board has reviewed your request.\nYour request has been denied.".into(),
            align: Align::BottomCenter,
            ..Default::default()
        },
        Cue {
            start_ms: 41 * 60_000,
            end_ms: 42 * 60_000 + 30_000,
            text: "EPISODE TWENTY-SIX".into(),
            align: Align::TopCenter,
            font_percent: 0.035,
            color: [218, 215, 214],
            bold: true,
            // An ASS sign with the outline, drop shadow and turn the parser now supports.
            border_percent: 0.0022,
            shadow_percent: 0.0025,
            border_color: [210, 70, 70],
            shadow_color: [20, 40, 70],
            rotation: 6.0,
            scale: (1.1, 1.0),
            ..Default::default()
        },
        Cue {
            start_ms: 41 * 60_000,
            end_ms: 42 * 60_000 + 30_000,
            text: "Umitsuki Apartments".into(),
            align: Align::MiddleLeft,
            font_percent: 0.040,
            color: [230, 227, 228],
            italic: true,
            border_percent: 0.004,
            shadow_percent: 0.002,
            ..Default::default()
        },
    ]
}

pub fn playback(paused: bool) -> PlaybackSession {
    let mut detail = detail();
    let episode = detail.episodes.remove(0);
    PlaybackSession {
        item_id: episode.id.clone(),
        item: episode,
        session_id: "demo-session".into(),
        position_ms: 42 * 60_000,
        duration_ms: Some(48 * 60_000),
        paused,
        generation: 0,
        last_report: Instant::now(),
        handle: None,
    }
}

pub fn search_results() -> SearchResults {
    let detail = detail();
    let movies = [
        ("past-lives", "Past Lives", 2023),
        ("anatomy-fall", "Anatomy of a Fall", 2023),
        ("the-substance", "The Substance", 2024),
        ("perfect-days", "Perfect Days", 2023),
        ("challengers", "Challengers", 2024),
        ("dune-2", "Dune: Part Two", 2024),
        ("zone-interest", "The Zone of Interest", 2023),
        ("poor-things", "Poor Things", 2023),
    ]
    .into_iter()
    .map(|(id, name, year)| Item {
        id: id.into(),
        name: name.into(),
        kind: "Movie".into(),
        production_year: Some(year),
        ..Item::default()
    })
    .collect();
    SearchResults {
        movies,
        shows: vec![detail.item],
        episodes: detail.episodes,
        people: vec![Item {
            id: "adam-scott".into(),
            name: "Adam Scott".into(),
            kind: "Person".into(),
            ..Item::default()
        }],
    }
}

pub fn sessions() -> Vec<SessionInfo> {
    let detail = detail();
    vec![
        SessionInfo {
            id: "living-room".into(),
            user_id: "demo-user".into(),
            user_name: "luis".into(),
            client: "Jellyfin Web".into(),
            device_name: "living-room".into(),
            device_id: "device-living-room".into(),
            application_version: "10.11.2".into(),
            is_active: true,
            now_playing_item: Some(detail.item),
            play_state: Some(PlayerState {
                position_ticks: 10_000 * 12 * 60_000,
                is_paused: false,
                can_seek: true,
            }),
        },
        SessionInfo {
            id: "bedroom".into(),
            user_id: "demo-user".into(),
            user_name: "luis".into(),
            client: "Android TV".into(),
            device_name: "bedroom".into(),
            device_id: "device-bedroom".into(),
            application_version: "0.18.0".into(),
            is_active: true,
            ..SessionInfo::default()
        },
    ]
}

pub fn signed_in(s: &mut State) {
    s.step = Step::Ready {
        server_name: "example".into(),
    };
    // Never render the real session's identifiers into a committed snapshot.
    s.config.sign_out();
    s.config.server_url = "https://jellyfin.example".into();
    s.config.user_id = "demo-user".into();
    s.config.user_name = "luis".into();
    s.config.device_id = "demo-device".into();
    s.home_data = home();
    s.home_loaded = true;
}

/// An ISO timestamp `days` days before today. Fixtures use it so a snapshot's NEW badges do not
/// change when the wall clock crosses midnight: "new" is a 30-day window, and a fixed October
/// date silently expires.
fn days_ago(days: usize) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs() as i64);
    let z = now / 86_400 - days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02}T00:00:00.0000000Z")
}

/// A deterministic library for snapshots and tests: fixed geometry, no images, no network.
pub fn library(count: usize) -> LibraryState {
    let mut library = LibraryState::new("movies".into(), "Movies".into(), Some("movies".into()));
    library.items = library_items(count);
    library.total = count as i32;
    library
}

fn library_items(count: usize) -> Vec<Item> {
    const TITLES: [&str; 10] = [
        "Aurora", "Burning", "Caché", "Drive", "Everest", "Fallen", "Gravity", "Hana", "Iris",
        "Joker",
    ];
    (0..count)
        .map(|index| Item {
            id: format!("film-{index:03}"),
            name: format!("{} {index:03}", TITLES[index % TITLES.len()]),
            kind: "Movie".into(),
            production_year: Some(1990 + (index % 30) as i32),
            // Every third card is finished, so the watched badge is visible in snapshots. The
            // huge fixture used by the frame-budget test stays lean: 20 000 copies of the same
            // watched state only make that test clone more.
            official_rating: (count <= 500).then(|| "PG-13".into()),
            run_time_ticks: (count <= 500).then(|| 10_000 * 60_000 * (90 + index as i64)),
            date_created: (count <= 500).then(|| days_ago(index % 40 + 1)),
            user_data: (count <= 500).then(|| UserData {
                played: index % 3 == 2,
                play_count: (index % 4) as i32,
                ..UserData::default()
            }),
            ..Item::default()
        })
        .collect()
}

// ---- Seerr companion ------------------------------------------------------------------

fn seerr_user() -> seerr_api::User {
    seerr_api::User {
        id: 1,
        username: None,
        email: Some("user@example.invalid".into()),
        display_name: Some("example-user".into()),
        jellyfin_username: Some("example-user".into()),
        permissions: 2,
        request_count: 4,
    }
}

fn seerr_download(
    title: &str,
    download_id: &str,
    size: u64,
    size_left: u64,
    status: &str,
    season: Option<(i64, i64)>,
) -> DownloadingItem {
    DownloadingItem {
        title: title.into(),
        status: status.into(),
        size,
        size_left,
        time_left: Some("00:12:34".into()),
        estimated_completion_time: Some("2026-11-05T18:00:00.000Z".into()),
        download_id: download_id.into(),
        media_type: Some(if season.is_some() { "tv" } else { "movie" }.into()),
        external_id: Some(if season.is_some() { 167 } else { 59 }),
        episode: season.map(|(season, episode)| EpisodeRef {
            season_number: season,
            episode_number: episode,
            absolute_episode_number: Some(episode),
            id: Some(episode),
        }),
    }
}

fn seerr_media(
    id: i64,
    tmdb_id: u64,
    media_type: &str,
    status: i64,
    downloads: Vec<DownloadingItem>,
    jellyfin: Option<&str>,
) -> MediaInfo {
    MediaInfo {
        id,
        tmdb_id: Some(tmdb_id),
        tvdb_id: None,
        media_type: media_type.into(),
        status,
        status_4k: 1,
        download_status: downloads,
        download_status_4k: Vec::new(),
        jellyfin_media_id: jellyfin.map(str::to_string),
        media_url: jellyfin
            .map(|id| format!("https://jellyfin.example/web/index.html#!/details?id={id}")),
        service_url: Some("https://radarr.example/movie/example".into()),
    }
}

fn seerr_request(
    id: u64,
    status: i64,
    media_type: &str,
    tmdb_id: u64,
    media_status: i64,
    downloads: Vec<DownloadingItem>,
    jellyfin: Option<&str>,
) -> MediaRequest {
    MediaRequest {
        id,
        status,
        media_type: media_type.into(),
        is_4k: false,
        season_count: if media_type == "tv" { 1 } else { 0 },
        seasons: if media_type == "tv" {
            vec![RequestSeason {
                id: Some(1),
                season_number: 1,
            }]
        } else {
            Vec::new()
        },
        requested_by: Some(seerr_user()),
        media: seerr_media(
            id as i64,
            tmdb_id,
            media_type,
            media_status,
            downloads,
            jellyfin,
        ),
        can_remove: true,
        created_at: Some("2026-11-05T00:00:00.000Z".into()),
        updated_at: Some("2026-11-05T00:00:00.000Z".into()),
    }
}

fn seerr_summary(
    title: &str,
    year: &str,
    poster: &str,
    backdrop: &str,
    seasons: Vec<Season>,
) -> MediaSummary {
    MediaSummary {
        title: title.into(),
        year: Some(year.into()),
        poster_path: Some(poster.into()),
        backdrop_path: Some(backdrop.into()),
        overview: Some("A deterministic fixture for the Seerr companion.".into()),
        runtime: None,
        number_of_seasons: Some(seasons.len() as i64),
        seasons,
    }
}

fn seerr_season(number: i64, episodes: i64) -> Season {
    Season {
        season_number: number,
        name: Some(format!("Season {number}")),
        episode_count: episodes,
        air_date: Some("2026-10-02".into()),
    }
}

fn seerr_discover_item(
    media_type: &str,
    id: u64,
    title: &str,
    year: &str,
    poster: &str,
    media_status: Option<i64>,
) -> DiscoverItem {
    DiscoverItem {
        id,
        media_type: media_type.into(),
        title: (media_type == "movie").then(|| title.into()),
        name: (media_type == "tv").then(|| title.into()),
        release_date: (media_type == "movie").then(|| format!("{year}-01-01")),
        first_air_date: (media_type == "tv").then(|| format!("{year}-01-01")),
        poster_path: Some(poster.into()),
        backdrop_path: Some(format!("/{id}-backdrop.jpg")),
        overview: Some("A deterministic fixture for the Seerr companion.".into()),
        vote_average: Some(7.5),
        media_info: media_status
            .map(|status| seerr_media(0, id, media_type, status, Vec::new(), None)),
    }
}

pub fn seerr_counts() -> RequestCounts {
    RequestCounts {
        total: 5,
        movie: 3,
        tv: 2,
        pending: 1,
        approved: 3,
        declined: 0,
        processing: 2,
        available: 1,
        completed: 0,
    }
}

pub fn seerr_requests() -> Vec<MediaRequest> {
    vec![
        seerr_request(10, 2, "movie", 438631, 3, Vec::new(), None),
        seerr_request(
            14,
            2,
            "movie",
            335984,
            3,
            vec![seerr_download(
                "Blade.Runner.2049.2160p",
                "movie-2",
                10_000_000_000,
                4_000_000_000,
                "downloading",
                None,
            )],
            None,
        ),
        seerr_request(
            11,
            2,
            "tv",
            196950,
            4,
            vec![
                seerr_download(
                    "Example.Show.S01.1080p",
                    "pack-1",
                    43_063_640_191,
                    0,
                    "completed",
                    Some((1, 1)),
                ),
                seerr_download(
                    "Example.Show.S01.1080p",
                    "pack-1",
                    43_063_640_191,
                    0,
                    "completed",
                    Some((1, 2)),
                ),
                seerr_download(
                    "Example.Show.S01.1080p",
                    "pack-1",
                    43_063_640_191,
                    0,
                    "completed",
                    Some((1, 3)),
                ),
            ],
            Some("item-123"),
        ),
        seerr_request(12, 5, "movie", 693134, 5, Vec::new(), Some("item-456")),
        seerr_request(13, 1, "tv", 2316, 2, Vec::new(), None),
    ]
}

/// The live instance's shape: processing requests with no queue rows.
pub fn seerr_requests_live() -> Vec<MediaRequest> {
    vec![
        seerr_request(20, 2, "movie", 438631, 3, Vec::new(), None),
        seerr_request(21, 2, "tv", 196950, 4, Vec::new(), None),
        seerr_request(23, 2, "movie", 335984, 3, Vec::new(), None),
    ]
}

pub fn seerr_summaries() -> HashMap<(String, u64), MediaSummary> {
    let mut summaries = HashMap::new();
    summaries.insert(
        ("movie".into(), 438631),
        seerr_summary(
            "Dune",
            "2021",
            "/dune.jpg",
            "/dune-backdrop.jpg",
            Vec::new(),
        ),
    );
    summaries.insert(
        ("tv".into(), 196950),
        seerr_summary(
            "Example Show",
            "2026",
            "/example-show.jpg",
            "/example-show-backdrop.jpg",
            vec![seerr_season(1, 12), seerr_season(2, 10), seerr_season(3, 8)],
        ),
    );
    summaries.insert(
        ("movie".into(), 693134),
        seerr_summary(
            "Dune: Part Two",
            "2024",
            "/dune-two.jpg",
            "/dune-two-backdrop.jpg",
            Vec::new(),
        ),
    );
    summaries.insert(
        ("movie".into(), 335984),
        seerr_summary(
            "Blade Runner 2049",
            "2017",
            "/blade.jpg",
            "/blade-backdrop.jpg",
            Vec::new(),
        ),
    );
    summaries.insert(
        ("tv".into(), 2316),
        seerr_summary(
            "The Office",
            "2005",
            "/office.jpg",
            "/office-backdrop.jpg",
            vec![seerr_season(1, 6), seerr_season(2, 22)],
        ),
    );
    summaries
}

pub fn seerr_genres() -> Vec<seerr_api::Genre> {
    [
        ("Action", 28),
        ("Adventure", 12),
        ("Animation", 16),
        ("Comedy", 35),
        ("Crime", 80),
        ("Documentary", 99),
        ("Drama", 18),
        ("Family", 10751),
        ("Fantasy", 14),
        ("Horror", 27),
        ("Mystery", 9648),
        ("Romance", 10749),
        ("Science Fiction", 878),
        ("Thriller", 53),
        ("War", 10752),
        ("Western", 37),
    ]
    .into_iter()
    .map(|(name, id)| seerr_api::Genre {
        id,
        name: Some(name.into()),
    })
    .collect()
}

pub fn seerr_discover() -> crate::state::SeerrDiscover {
    crate::state::SeerrDiscover {
        trending: vec![
            seerr_discover_item(
                "movie",
                1423191,
                "Resident Evil",
                "2026",
                "/resident.jpg",
                None,
            ),
            seerr_discover_item(
                "tv",
                90228,
                "Dune: Prophecy",
                "2024",
                "/prophecy.jpg",
                Some(3),
            ),
            seerr_discover_item("movie", 438631, "Dune", "2021", "/dune.jpg", Some(5)),
        ],
        movies: vec![
            seerr_discover_item(
                "movie",
                693134,
                "Dune: Part Two",
                "2024",
                "/dune-two.jpg",
                Some(5),
            ),
            seerr_discover_item("movie", 438631, "Dune", "2021", "/dune.jpg", Some(5)),
            seerr_discover_item(
                "movie",
                335984,
                "Blade Runner 2049",
                "2017",
                "/blade.jpg",
                None,
            ),
        ],
        tv: vec![
            seerr_discover_item(
                "tv",
                196950,
                "Example Show",
                "2026",
                "/example-show.jpg",
                Some(4),
            ),
            seerr_discover_item(
                "tv",
                90228,
                "Dune: Prophecy",
                "2024",
                "/prophecy.jpg",
                Some(3),
            ),
            seerr_discover_item("tv", 2316, "The Office", "2005", "/office.jpg", Some(2)),
        ],
        ..Default::default()
    }
}

pub fn seerr_search() -> SeerrSearchResults {
    SeerrSearchResults {
        page: 1,
        total_pages: 1,
        total_results: 3,
        results: vec![
            seerr_discover_item("movie", 438631, "Dune", "2021", "/dune.jpg", Some(5)),
            seerr_discover_item(
                "tv",
                196950,
                "Example Show",
                "2026",
                "/example-show.jpg",
                Some(4),
            ),
            seerr_discover_item(
                "movie",
                999999,
                "Never Requested",
                "2026",
                "/never.jpg",
                None,
            ),
        ],
    }
}

pub fn seerr_tv_series() -> TvDetails {
    TvDetails {
        id: 196950,
        name: Some("Example Show".into()),
        first_air_date: Some("2026-10-02".into()),
        number_of_seasons: Some(3),
        seasons: vec![
            seerr_season(0, 2),
            seerr_season(1, 12),
            seerr_season(2, 10),
            seerr_season(3, 8),
        ],
        poster_path: Some("/example-show.jpg".into()),
        backdrop_path: Some("/example-show-backdrop.jpg".into()),
        overview: Some(
            "A deterministic series used to freeze the Seerr request panel and season picker."
                .into(),
        ),
        tagline: None,
        genres: Vec::new(),
        credits: Default::default(),
        vote_average: Some(7.2),
        media_info: Some(seerr_media(
            11,
            196950,
            "tv",
            4,
            Vec::new(),
            Some("item-123"),
        )),
    }
}

pub fn seerr_tv_detail() -> Details {
    Details::Tv(seerr_tv_series())
}

pub fn seerr_movie_detail() -> Details {
    Details::Movie(MovieDetails {
        id: 438631,
        title: Some("Dune".into()),
        release_date: Some("2021-09-15".into()),
        runtime: Some(155),
        poster_path: Some("/dune.jpg".into()),
        backdrop_path: Some("/dune-backdrop.jpg".into()),
        overview: Some(
            "Paul Atreides, a brilliant and gifted young man born into a great destiny.".into(),
        ),
        vote_average: Some(7.8),
        tagline: Some("Beyond fear, destiny awaits.".into()),
        genres: vec![
            seerr_api::Genre {
                id: 878,
                name: Some("Science Fiction".into()),
            },
            seerr_api::Genre {
                id: 12,
                name: Some("Adventure".into()),
            },
        ],
        credits: seerr_api::Credits {
            cast: vec![
                seerr_api::CastMember {
                    id: 1190668,
                    name: Some("Timothée Chalamet".into()),
                    character: Some("Paul Atreides".into()),
                    job: None,
                    profile_path: None,
                },
                seerr_api::CastMember {
                    id: 933238,
                    name: Some("Rebecca Ferguson".into()),
                    character: Some("Lady Jessica".into()),
                    job: None,
                    profile_path: None,
                },
                seerr_api::CastMember {
                    id: 1683343,
                    name: Some("Cailee Spaeny".into()),
                    character: Some("Princess Irulan".into()),
                    job: None,
                    profile_path: None,
                },
            ],
            crew: vec![seerr_api::CastMember {
                id: 137427,
                name: Some("Denis Villeneuve".into()),
                character: None,
                job: Some("Director".into()),
                profile_path: None,
            }],
        },
        media_info: Some(seerr_media(10, 438631, "movie", 3, Vec::new(), None)),
    })
}
