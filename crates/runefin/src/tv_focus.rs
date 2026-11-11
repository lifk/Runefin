//! The living-room focus model: zones with explicit edges.
//!
//! Rune's own arrow handling moves inside a focus group and treats Up/Down as Left/Right in a
//! horizontal one; a TV needs the opposite. Screens here declare zones in reading order and this
//! router decides where a direction goes, then asks Rune to focus the key. Rune still draws the
//! ring and activates the focused control.

use crate::state::{Route, State};
use crate::tv_input::TvAction;

pub struct Zone {
    keys: Vec<String>,
    /// Items per row. `0` means one row of every key (a rail); `1` is a vertical list.
    columns: usize,
    /// Where Left at the first column, or Up from the first row, goes.
    parent: Option<usize>,
}

/// Whether `key` is one of the zones' keys. A focused panel, menu or scrollbar is not, and the
/// router must leave those to Rune instead of swallowing their directions.
pub fn contains(s: &State, key: &str) -> bool {
    let (zones, _) = zones(s);
    zones
        .iter()
        .any(|zone| zone.keys.iter().any(|candidate| candidate == key))
}

/// Whether this router owns the current screen. A screen it does not know is left to Rune's own
/// group handling.
pub fn owns_screen(s: &State) -> bool {
    matches!(
        s.route,
        Route::Home
            | Route::Library { .. }
            | Route::Detail { .. }
            | Route::Search { .. }
            | Route::Person { .. }
            | Route::Seerr
            | Route::SeerrDetail { .. }
            | Route::LiveTv
            | Route::Remote
            | Route::Settings
    ) || (s.in_app.is_some() && s.osd_visible)
}

/// The key a direction should focus, or `None` when the edge is reached and nothing should move.
pub fn step(s: &State, focused: Option<&str>, action: TvAction) -> Option<String> {
    if !action.is_direction() {
        return None;
    }
    let (mut zones, sidebar) = zones(s);
    zones.retain(|zone| !zone.keys.is_empty());
    let first = zones.first()?;
    let Some(focused) = focused else {
        // Nothing focused: the first content item is a better start than doing nothing.
        return if sidebar {
            zones.get(1).and_then(|zone| zone.keys.first()).cloned()
        } else {
            first.keys.first().cloned()
        };
    };
    let (zi, idx) = zones.iter().enumerate().find_map(|(zi, zone)| {
        zone.keys
            .iter()
            .position(|key| key == focused)
            .map(|idx| (zi, idx))
    })?;
    let zone = &zones[zi];
    let cols = if zone.columns == 0 {
        zone.keys.len().max(1)
    } else {
        zone.columns
    };
    let col = idx % cols;
    let row = idx / cols;
    match action {
        TvAction::Left if col > 0 => Some(zone.keys[idx - 1].clone()),
        TvAction::Left => parent_key(s, &zones, sidebar, zi, idx),
        TvAction::Right if col + 1 < cols && idx + 1 < zone.keys.len() => {
            Some(zone.keys[idx + 1].clone())
        }
        // The sidebar's Right enters the content it belongs to.
        TvAction::Right if sidebar && zi == 0 => {
            zones.get(1).and_then(|zone| zone.keys.first()).cloned()
        }
        TvAction::Right => None,
        TvAction::Up if row > 0 => Some(zone.keys[idx - cols].clone()),
        TvAction::Up => adjacent(s, &zones, sidebar, zi, idx, -1),
        TvAction::Down if idx + cols < zone.keys.len() => Some(zone.keys[idx + cols].clone()),
        TvAction::Down => adjacent(s, &zones, sidebar, zi, idx, 1),
        _ => None,
    }
}

fn parent_key(s: &State, zones: &[Zone], sidebar: bool, zi: usize, idx: usize) -> Option<String> {
    let parent = zones[zi].parent?;
    if parent >= zones.len() {
        return None;
    }
    if sidebar && parent == 0 {
        // The sidebar: go to the destination the current screen belongs to.
        return nav_key(s);
    }
    let zone = &zones[parent];
    zone.keys
        .get(idx.min(zone.keys.len().saturating_sub(1)))
        .cloned()
}

fn adjacent(
    s: &State,
    zones: &[Zone],
    sidebar: bool,
    zi: usize,
    idx: usize,
    delta: isize,
) -> Option<String> {
    // The sidebar is a fixed list: Down at its end and Up at its start stay put, and Right is
    // the way into content.
    if sidebar && zi == 0 {
        return None;
    }
    let target = zi.checked_add_signed(delta)?;
    if target >= zones.len() {
        return None;
    }
    if sidebar && target == 0 {
        return nav_key(s);
    }
    let zone = &zones[target];
    // Entering a rail from a vertical list starts at its first item: the list's row number is
    // not a column position. Rail-to-rail moves still keep the column.
    let at = if zones[zi].columns == 1 && zone.columns == 0 {
        0
    } else {
        idx.min(zone.keys.len().saturating_sub(1))
    };
    zone.keys.get(at).cloned()
}

/// The sidebar entry for the current screen, so Up/Left from content lands on the destination
/// rather than always on Home.
fn nav_key(s: &State) -> Option<String> {
    match &s.route {
        Route::Home => Some("nav-home".into()),
        Route::Library { id } if id == "__watchlist__" => Some("nav-watchlist".into()),
        Route::Library { id } => Some(format!("library-{id}")),
        Route::Search { .. } => Some("nav-search".into()),
        Route::Remote => Some("nav-remote".into()),
        Route::Settings => Some("nav-settings".into()),
        Route::Seerr | Route::SeerrDetail { .. } => Some("nav-seerr".into()),
        Route::LiveTv => s
            .home_data
            .libraries
            .iter()
            .find(|library| library.collection_type.as_deref() == Some("livetv"))
            .map(|library| format!("library-{}", library.id))
            .or_else(|| Some("nav-home".into())),
        Route::Detail { .. } => match &s.detail_return {
            Route::Library { id } if id == "__watchlist__" => Some("nav-watchlist".into()),
            Route::Library { id } => Some(format!("library-{id}")),
            _ => Some("nav-home".into()),
        },
        _ => Some("nav-home".into()),
    }
}

fn zones(s: &State) -> (Vec<Zone>, bool) {
    if s.in_app.is_some() {
        return (in_app_zones(s), false);
    }
    match &s.route {
        Route::Home => (home_zones(s), true),
        Route::Library { .. } => (library_zones(s), true),
        Route::Detail { .. } => (detail_zones(s), false),
        Route::Search { .. } => (search_zones(s), false),
        Route::Person { .. } => (person_zones(s), false),
        Route::Seerr => (seerr_zones(s), false),
        Route::LiveTv => (live_zones(s), true),
        Route::Remote => (remote_zones(s), true),
        Route::SeerrDetail { .. } => (seerr_detail_zones(s), false),
        _ => (Vec::new(), false),
    }
}

fn sidebar(s: &State) -> Zone {
    let mut keys = vec!["nav-home".to_string()];
    for library in &s.home_data.libraries {
        keys.push(format!("library-{}", library.id));
    }
    keys.push("nav-watchlist".into());
    for playlist in &s.playlists {
        keys.push(format!("playlist-{}", playlist.id));
    }
    if !s.config.seerr_url.is_empty() {
        keys.push("nav-seerr".into());
    }
    keys.extend(
        ["nav-search", "nav-remote", "nav-controls", "nav-settings"]
            .into_iter()
            .map(String::from),
    );
    Zone {
        keys,
        columns: 1,
        parent: None,
    }
}

fn item_key(rail: &str, item: &jellyfin_api::Item) -> String {
    format!("{rail}-{}", item.series_id.as_deref().unwrap_or(&item.id))
}

fn home_zones(s: &State) -> Vec<Zone> {
    let mut zones = vec![sidebar(s)];
    let visible =
        |items: &[jellyfin_api::Item]| items.iter().any(|item| !s.is_home_hidden(&item.id));
    if visible(&s.home_data.resume) {
        zones.push(Zone {
            keys: s
                .home_data
                .resume
                .iter()
                .filter(|item| !s.is_home_hidden(&item.id))
                .map(|item| item_key("rail-continue-watching", item))
                .collect(),
            columns: 0,
            parent: Some(0),
        });
    }
    if visible(&s.home_data.next_up) {
        zones.push(Zone {
            keys: s
                .home_data
                .next_up
                .iter()
                .filter(|item| !s.is_home_hidden(&item.id))
                .map(|item| item_key("rail-next-up", item))
                .collect(),
            columns: 0,
            parent: Some(0),
        });
    }
    if visible(&s.home_data.on_now) {
        zones.push(Zone {
            keys: s
                .home_data
                .on_now
                .iter()
                .map(|channel| format!("rail-on-now-{}", channel.id))
                .collect(),
            columns: 0,
            parent: Some(0),
        });
    }
    for (index, recent) in s
        .home_data
        .latest
        .iter()
        .filter(|rail| rail.items.iter().any(|item| !s.is_home_hidden(&item.id)))
        .enumerate()
    {
        zones.push(Zone {
            keys: recent
                .items
                .iter()
                .filter(|item| !s.is_home_hidden(&item.id))
                .map(|item| item_key(&format!("rail-recent-{index}"), item))
                .collect(),
            columns: 0,
            parent: Some(0),
        });
    }
    if visible(&s.home_data.recently_played) {
        zones.push(Zone {
            keys: s
                .home_data
                .recently_played
                .iter()
                .filter(|item| !s.is_home_hidden(&item.id))
                .map(|item| item_key("rail-recently-played", item))
                .collect(),
            columns: 0,
            parent: Some(0),
        });
    }
    if !s.home_data.collections.is_empty() {
        zones.push(Zone {
            keys: s
                .home_data
                .collections
                .iter()
                .map(|item| item_key("rail-collections", item))
                .collect(),
            columns: 0,
            parent: Some(0),
        });
    }
    zones
}

fn library_zones(s: &State) -> Vec<Zone> {
    let Some(library) = &s.library else {
        return vec![sidebar(s)];
    };
    let grid = library.view == crate::library::ViewMode::Grid;
    let keys = library
        .items
        .iter()
        .map(|item| {
            if grid {
                format!("library-{}", item.id)
            } else {
                format!("library-item-{}", item.id)
            }
        })
        .collect();
    vec![
        sidebar(s),
        Zone {
            keys,
            columns: if grid { library.columns.max(1) } else { 1 },
            parent: Some(0),
        },
    ]
}

fn detail_zones(s: &State) -> Vec<Zone> {
    let Some(detail) = &s.detail else {
        return Vec::new();
    };
    let is_boxset = detail.item.kind == "BoxSet";
    // Mirror `detail::detail_screen`: a key that is not drawn must never take focus.
    let mut actions = Vec::new();
    if !is_boxset {
        actions.push("detail-play".to_string());
        if detail.item.media_sources.len() > 1 {
            actions.push("detail-play-options".into());
        }
        if detail
            .play_target()
            .and_then(|item| item.user_data.as_ref())
            .and_then(|user| user.position_ms())
            .is_some_and(|position| position > 0)
        {
            actions.push("detail-restart".into());
        }
        if !s.detail_theme_songs.is_empty() {
            actions.push("detail-theme-audio".into());
        }
        if !s.detail_theme_videos.is_empty() {
            actions.push("detail-theme-video".into());
        }
        if !detail.local_trailers.is_empty() || !detail.item.remote_trailers.is_empty() {
            actions.push("detail-trailer".into());
        }
        if detail.item.kind == "Series"
            && let Some((tmdb, tv)) = &s.seerr.series
            && detail
                .item
                .provider_ids
                .get("Tmdb")
                .and_then(|id| id.parse::<u64>().ok())
                == Some(*tmdb)
        {
            let present: std::collections::HashSet<i64> = detail
                .seasons
                .iter()
                .filter_map(|season| season.index_number.map(i64::from))
                .collect();
            if !crate::state::missing_seasons(&present, tv).is_empty() {
                actions.push("detail-seerr-request".into());
            }
        }
    } else if !detail.collection.is_empty() {
        actions.push("detail-play-all".into());
        actions.push("detail-shuffle".into());
    }
    actions.push("detail-favorite".into());
    actions.push("detail-played".into());
    if !is_boxset && crate::state::is_playable(&detail.item) {
        actions.push("detail-playlist".into());
    }

    let mut zones = vec![
        Zone {
            keys: vec!["detail-back".into()],
            columns: 1,
            parent: None,
        },
        Zone {
            keys: actions,
            columns: 0,
            parent: Some(0),
        },
    ];
    if !detail.seasons.is_empty() {
        zones.push(Zone {
            keys: detail
                .seasons
                .iter()
                .map(|season| format!("season-{}", season.id))
                .collect(),
            columns: 0,
            parent: Some(0),
        });
    }
    if detail.next_up.is_some() {
        zones.push(Zone {
            keys: vec!["detail-next-up-card".into()],
            columns: 1,
            parent: Some(0),
        });
    }
    if !detail.loading_episodes && !detail.episodes.is_empty() {
        zones.push(Zone {
            keys: [
                "detail-play-season",
                "detail-shuffle-season",
                "detail-season-watched",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            columns: 0,
            parent: Some(0),
        });
        zones.push(Zone {
            keys: detail
                .episodes
                .iter()
                .map(|episode| format!("detail-episode-{}", episode.id))
                .collect(),
            columns: 1,
            parent: Some(0),
        });
    }
    zones
}

fn search_zones(s: &State) -> Vec<Zone> {
    let results = &s.search.results;
    let mut zones = vec![Zone {
        keys: vec!["search-input".into()],
        columns: 1,
        parent: None,
    }];
    let rail = |prefix: &str, items: &[jellyfin_api::Item]| Zone {
        keys: items
            .iter()
            .map(|item| format!("{prefix}-{}", item.id))
            .collect(),
        columns: 0,
        parent: Some(0),
    };
    if !results.movies.is_empty() {
        zones.push(rail("search-movies", &results.movies));
    }
    if !results.shows.is_empty() {
        zones.push(rail("search-shows", &results.shows));
    }
    if !results.episodes.is_empty() {
        zones.push(rail("search-episodes", &results.episodes));
    }
    if !results.people.is_empty() {
        zones.push(Zone {
            keys: results
                .people
                .iter()
                .map(|person| format!("search-person-{}", person.id))
                .collect(),
            columns: 0,
            parent: Some(0),
        });
    }
    if !s.search.seerr.results.is_empty() {
        zones.push(Zone {
            keys: s
                .search
                .seerr
                .results
                .iter()
                .map(|item| format!("seerr-card-{}-{}", item.media_type, item.id))
                .collect(),
            columns: 0,
            parent: Some(0),
        });
    }
    zones
}

fn person_zones(s: &State) -> Vec<Zone> {
    let Some(person) = &s.person else {
        return Vec::new();
    };
    let mut zones = vec![Zone {
        keys: vec!["person-back".into()],
        columns: 1,
        parent: None,
    }];
    let known: Vec<String> = person
        .items
        .iter()
        .take(5)
        .map(|item| {
            format!(
                "person-known-{}",
                item.series_id.as_deref().unwrap_or(&item.id)
            )
        })
        .collect();
    if !known.is_empty() {
        zones.push(Zone {
            keys: known,
            columns: 0,
            parent: Some(0),
        });
    }
    let grid: Vec<String> = person
        .items
        .iter()
        .map(|item| format!("person-{}", item.series_id.as_deref().unwrap_or(&item.id)))
        .collect();
    if !grid.is_empty() {
        zones.push(Zone {
            keys: grid,
            columns: 5,
            parent: Some(0),
        });
    }
    zones
}

fn seerr_zones(s: &State) -> Vec<Zone> {
    let mut zones = vec![Zone {
        keys: vec!["seerr-tab-discover".into(), "seerr-tab-requests".into()],
        columns: 0,
        parent: None,
    }];
    match s.seerr.tab {
        crate::state::SeerrTab::Discover => {
            let mut keys = Vec::new();
            // The page renders either the editorial rails or the browse result, never both.
            let items: Vec<&seerr_api::DiscoverItem> =
                if s.seerr.discover_mode == crate::state::DiscoverMode::Browse {
                    s.seerr.discover.browse.iter().collect()
                } else {
                    s.seerr
                        .discover
                        .trending
                        .iter()
                        .chain(s.seerr.discover.movies.iter())
                        .chain(s.seerr.discover.tv.iter())
                        .collect()
                };
            for item in items {
                keys.push(format!("seerr-card-{}-{}", item.media_type, item.id));
            }
            if !keys.is_empty() {
                zones.push(Zone {
                    keys,
                    columns: 0,
                    parent: Some(0),
                });
            }
        }
        crate::state::SeerrTab::Requests => {
            let keys: Vec<String> = s
                .seerr
                .requests
                .iter()
                .map(|request| format!("seerr-request-{}", request.id))
                .collect();
            if !keys.is_empty() {
                zones.push(Zone {
                    keys,
                    columns: 1,
                    parent: Some(0),
                });
            }
        }
    }
    zones
}

fn live_zones(s: &State) -> Vec<Zone> {
    let mut zones = vec![sidebar(s)];
    let keys: Vec<String> = s
        .live
        .channels
        .iter()
        .map(|channel| {
            if s.live.guide_open {
                format!("live-guide-row-{}", channel.id)
            } else {
                format!("live-channel-{}", channel.id)
            }
        })
        .collect();
    if !keys.is_empty() {
        zones.push(Zone {
            keys,
            columns: 1,
            parent: Some(0),
        });
    }
    zones
}

fn remote_zones(s: &State) -> Vec<Zone> {
    let mut zones = vec![sidebar(s)];
    let keys: Vec<String> = s
        .cast_sessions
        .iter()
        .map(|session| format!("remote-session-{}", session.id))
        .collect();
    if !keys.is_empty() {
        zones.push(Zone {
            keys,
            columns: 1,
            parent: Some(0),
        });
    }
    zones
}

fn seerr_detail_zones(s: &State) -> Vec<Zone> {
    let Some(detail) = &s.seerr.detail else {
        return Vec::new();
    };
    let mut zones = vec![Zone {
        keys: vec!["seerr-detail-back".into()],
        columns: 1,
        parent: None,
    }];
    let Some(details) = &detail.details else {
        return zones;
    };
    // Mirror `detail_action`: a key the page does not render must never take focus.
    let media_status = details
        .media_info()
        .map(|info| info.media_status())
        .unwrap_or(seerr_api::MediaStatus::Unknown);
    let library_id = details
        .media_info()
        .and_then(|info| info.jellyfin_media_id.clone())
        .or_else(|| s.seerr.library_id(&detail.media_type, detail.tmdb_id));
    let mut actions = Vec::new();
    if library_id.is_some() {
        actions.push("seerr-detail-open".to_string());
    }
    let settled = matches!(
        media_status,
        seerr_api::MediaStatus::Available
            | seerr_api::MediaStatus::Processing
            | seerr_api::MediaStatus::Pending
            | seerr_api::MediaStatus::Blocklisted
    );
    let seasons_chosen =
        !matches!(details, seerr_api::Details::Tv(_)) || !detail.selected_seasons.is_empty();
    if !settled && seasons_chosen && !detail.requesting {
        actions.push("seerr-detail-request".to_string());
    }
    if !actions.is_empty() {
        let parent = zones.len() - 1;
        zones.push(Zone {
            keys: actions,
            columns: 0,
            parent: Some(parent),
        });
    }
    if let seerr_api::Details::Tv(tv) = details
        && !tv.seasons.is_empty()
    {
        // The picker shows All in its header and one row per season below it, skipping
        // specials and seasons without episodes.
        let mut seasons = vec!["seerr-season-all".to_string()];
        seasons.extend(
            tv.seasons
                .iter()
                .filter(|season| season.season_number > 0 && season.episode_count > 0)
                .map(|season| format!("seerr-season-{}", season.season_number)),
        );
        zones.push(Zone {
            keys: seasons,
            columns: 1,
            parent: Some(0),
        });
    }
    // The rail is drawn only from named cast, and only the first twelve.
    let cast: Vec<String> = details
        .credits()
        .cast
        .iter()
        .filter(|member| member.name.is_some())
        .take(12)
        .map(|member| format!("seerr-cast-{}", member.id))
        .collect();
    if !cast.is_empty() {
        zones.push(Zone {
            keys: cast,
            columns: 0,
            parent: Some(0),
        });
    }
    zones
}

fn in_app_zones(s: &State) -> Vec<Zone> {
    if !s.osd_visible {
        return Vec::new();
    }
    // A live stream has no timeline: the transport is the whole OSD.
    let live = s.in_app.as_ref().is_some_and(|session| session.live);
    let controls = Zone {
        keys: [
            "in-app-pause",
            "in-app-volume-slider",
            "in-app-mute",
            "in-app-fullscreen",
            "in-app-gear",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        columns: 0,
        parent: (!live).then_some(0),
    };
    if live {
        return vec![controls];
    }
    vec![
        Zone {
            keys: vec!["in-app-seek".into()],
            columns: 1,
            parent: None,
        },
        controls,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> State {
        let mut state = State::from_config(crate::config::Config::fresh());
        crate::demo::signed_in(&mut state);
        state.big_screen = true;
        state.route = Route::Home;
        state
    }

    #[test]
    fn a_rail_moves_sideways_and_up_down_switches_lanes() {
        let state = home();
        let first = "rail-continue-watching-quiet-floor";
        // Right within the rail.
        assert_eq!(
            step(&state, Some(first), TvAction::Right).as_deref(),
            Some("rail-continue-watching-service")
        );
        // Up from the first rail lands on the sidebar destination, not the rail's other end.
        assert_eq!(
            step(&state, Some(first), TvAction::Up).as_deref(),
            Some("nav-home")
        );
        // Down switches to the next lane.
        let down = step(&state, Some(first), TvAction::Down).expect("a lane below");
        assert!(down.starts_with("rail-next-up-"), "{down}");
    }

    #[test]
    fn the_left_edge_goes_to_the_sidebar_and_clamps() {
        let state = home();
        assert_eq!(
            step(
                &state,
                Some("rail-continue-watching-quiet-floor"),
                TvAction::Left
            )
            .as_deref(),
            Some("nav-home")
        );
        // Right from the sidebar enters the content.
        assert_eq!(
            step(&state, Some("nav-home"), TvAction::Right).as_deref(),
            Some("rail-continue-watching-quiet-floor")
        );
        // Up from the first sidebar row clamps.
        assert_eq!(step(&state, Some("nav-home"), TvAction::Up), None);
    }

    #[test]
    fn a_grid_moves_by_columns_and_its_left_edge_goes_to_the_library_entry() {
        let mut state = home();
        let library = crate::demo::library(12);
        let id = library.id.clone();
        state.route = Route::Library { id: id.clone() };
        state.library = Some(library);
        assert_eq!(
            step(&state, Some("library-film-002"), TvAction::Down).as_deref(),
            Some("library-film-007"),
            "five columns: down moves a row"
        );
        assert_eq!(
            step(&state, Some("library-film-000"), TvAction::Left).as_deref(),
            Some(format!("library-{id}").as_str())
        );
    }

    #[test]
    fn detail_moves_from_back_to_the_actions_and_along_them() {
        let mut state = home();
        state.route = Route::Detail {
            id: "quiet-floor".into(),
        };
        state.detail = Some(crate::demo::detail());
        assert_eq!(
            step(&state, Some("detail-back"), TvAction::Down).as_deref(),
            Some("detail-play")
        );
        assert_eq!(
            step(&state, Some("detail-play"), TvAction::Right).as_deref(),
            Some("detail-restart"),
            "the resumed series offers Start over"
        );
        assert_eq!(
            step(&state, Some("detail-play"), TvAction::Up).as_deref(),
            Some("detail-back")
        );
        // The season chips sit under the actions, then Next up, then the episode controls.
        assert!(contains(&state, "detail-next-up-card"));
        assert!(contains(&state, "detail-play-season"));
        assert!(contains(&state, "detail-shuffle-season"));
        assert!(contains(&state, "detail-season-watched"));
        let season = step(&state, Some("detail-play"), TvAction::Down).expect("the season chips");
        assert!(season.starts_with("season-"), "{season}");
        assert_eq!(
            step(&state, Some(&season), TvAction::Down).as_deref(),
            Some("detail-next-up-card")
        );
        assert_eq!(
            step(&state, Some("detail-next-up-card"), TvAction::Down).as_deref(),
            Some("detail-play-season")
        );
        assert_eq!(
            step(&state, Some("detail-play-season"), TvAction::Right).as_deref(),
            Some("detail-shuffle-season")
        );
        let episode =
            step(&state, Some("detail-play-season"), TvAction::Down).expect("the episode list");
        assert!(episode.starts_with("detail-episode-"), "{episode}");
    }

    #[test]
    fn search_moves_from_the_field_into_its_groups() {
        let mut state = home();
        state.route = Route::Search { scope: None };
        state.search.results = crate::demo::search_results();
        assert_eq!(
            step(&state, Some("search-input"), TvAction::Down).as_deref(),
            Some("search-movies-his-girl-friday")
        );
        assert_eq!(
            step(&state, Some("search-movies-his-girl-friday"), TvAction::Up).as_deref(),
            Some("search-input")
        );
        let down =
            step(&state, Some("search-movies-plan-9"), TvAction::Down).expect("the next group");
        assert!(down.starts_with("search-shows-"), "{down}");
    }

    #[test]
    fn a_person_page_reaches_known_for_and_the_library_grid() {
        let mut state = home();
        state.route = Route::Person {
            id: "mara-ellison".into(),
        };
        state.person = Some(crate::demo::person());
        let down = step(&state, Some("person-back"), TvAction::Down).expect("known for");
        assert!(down.starts_with("person-known-"), "{down}");
        let grid = step(&state, Some(&down), TvAction::Down).expect("the grid");
        assert!(grid.starts_with("person-"), "{grid}");
    }

    #[test]
    fn seerr_tabs_lead_into_the_current_list() {
        let mut state = home();
        state.route = Route::Seerr;
        state.seerr.tab = crate::state::SeerrTab::Requests;
        state.seerr.requests = crate::demo::seerr_requests();
        let down = step(&state, Some("seerr-tab-discover"), TvAction::Down).expect("a request");
        assert!(down.starts_with("seerr-request-"), "{down}");
        assert_eq!(
            step(&state, Some("seerr-tab-discover"), TvAction::Right).as_deref(),
            Some("seerr-tab-requests")
        );
    }

    #[test]
    fn a_panel_key_is_not_part_of_the_screen_zones() {
        let mut state = home();
        state.route = Route::Detail {
            id: "quiet-floor".into(),
        };
        state.detail = Some(crate::demo::detail());
        assert!(contains(&state, "detail-play"));
        assert!(
            !contains(&state, "detail-collection-picker"),
            "a modal keeps its own directions"
        );
    }

    #[test]
    fn live_tv_and_remote_move_over_their_rows() {
        let mut state = home();
        state.route = Route::LiveTv;
        state.live.channels = crate::demo::live_channels();
        let first = "live-channel-aurora-one";
        let down = step(&state, Some(first), TvAction::Down).expect("the next channel");
        assert_eq!(down, "live-channel-meridian");

        state.route = Route::Remote;
        state.cast_sessions = crate::demo::sessions();
        let down = step(&state, Some("remote-session-living-room"), TvAction::Down)
            .expect("the next session");
        assert_eq!(down, "remote-session-bedroom");
    }

    #[test]
    fn a_seerr_detail_reaches_actions_seasons_and_cast() {
        let mut state = home();
        state.route = Route::SeerrDetail {
            media_type: "tv".into(),
            tmdb_id: 196_950,
        };
        let mut tv = crate::demo::seerr_tv_series();
        tv.credits.cast = vec![
            seerr_api::CastMember {
                id: 7,
                name: Some("Owen Kestrel".into()),
                character: Some("Thomas Hutter".into()),
                job: None,
                profile_path: None,
            },
            seerr_api::CastMember {
                id: 8,
                name: Some("Iris Vane".into()),
                character: Some("Ellen Hutter".into()),
                job: None,
                profile_path: None,
            },
        ];
        state.seerr.detail = Some(crate::state::SeerrDetailState {
            media_type: "tv".into(),
            tmdb_id: 196_950,
            details: Some(seerr_api::Details::Tv(tv)),
            selected_seasons: Default::default(),
            loading: false,
            error: None,
            requesting: false,
        });
        assert_eq!(
            step(&state, Some("seerr-detail-back"), TvAction::Down).as_deref(),
            Some("seerr-detail-open")
        );
        assert_eq!(
            step(&state, Some("seerr-detail-open"), TvAction::Down).as_deref(),
            Some("seerr-season-all")
        );
        assert_eq!(
            step(&state, Some("seerr-season-all"), TvAction::Down).as_deref(),
            Some("seerr-season-1")
        );
        assert_eq!(
            step(&state, Some("seerr-season-3"), TvAction::Down).as_deref(),
            Some("seerr-cast-7")
        );
        // With a season chosen the request button joins the actions row, after the open button.
        if let Some(detail) = &mut state.seerr.detail {
            detail.selected_seasons.insert(1);
        }
        assert_eq!(
            step(&state, Some("seerr-detail-open"), TvAction::Right).as_deref(),
            Some("seerr-detail-request")
        );
    }

    #[test]
    fn a_live_osd_has_no_timeline_to_scrub() {
        let mut state = home();
        let mut session = crate::demo::in_app();
        session.live = true;
        state.in_app = Some(session);
        state.osd_visible = true;
        assert!(!contains(&state, "in-app-seek"));
        assert_eq!(
            step(&state, Some("in-app-pause"), TvAction::Right).as_deref(),
            Some("in-app-volume-slider")
        );
    }

    #[test]
    fn the_osd_moves_between_the_timeline_and_its_controls() {
        let mut state = home();
        state.in_app = Some(crate::demo::in_app());
        state.osd_visible = true;
        assert_eq!(
            step(&state, Some("in-app-seek"), TvAction::Down).as_deref(),
            Some("in-app-pause")
        );
        assert_eq!(
            step(&state, Some("in-app-pause"), TvAction::Up).as_deref(),
            Some("in-app-seek")
        );
        assert_eq!(
            step(&state, Some("in-app-pause"), TvAction::Right).as_deref(),
            Some("in-app-volume-slider")
        );
    }
}
