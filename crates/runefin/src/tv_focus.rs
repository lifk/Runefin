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
    /// The parent key to land on, when the parent is a column of its own (Settings' section
    /// list). `None` keeps the source column index.
    parent_at: Option<usize>,
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
            | Route::Downloads
            | Route::Settings
            | Route::Libraries
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
    // A rail's "See all" sits on the header line above its cards, not at the end of the row.
    // Treated as a rail key, Left jumped to the far-right card and Right did nothing.
    let rail_all = is_rail_all(focused);
    match action {
        TvAction::Left if !rail_all && col > 0 => Some(zone.keys[idx - 1].clone()),
        TvAction::Left => parent_key(s, &zones, sidebar, zi, idx),
        // The section list is a column at the top of the page: its ends stop, and Right is the
        // way into a section. Up at the top used to jump down to the rail's Settings row.
        TvAction::Up if focused.starts_with("settings-nav-") && idx == 0 => None,
        TvAction::Down if focused.starts_with("settings-nav-") && idx + 1 >= zone.keys.len() => {
            None
        }
        TvAction::Down if rail_all => zone.keys.iter().find(|key| !is_rail_all(key)).cloned(),
        // The card before a rail's See all does not slide onto it: that key is above the row.
        TvAction::Right
            if col + 1 < cols && idx + 1 < zone.keys.len() && !is_rail_all(&zone.keys[idx + 1]) =>
        {
            Some(zone.keys[idx + 1].clone())
        }
        // The sidebar's Right enters the content it belongs to.
        TvAction::Right if sidebar && zi == 0 => {
            zones.get(1).and_then(|zone| zone.keys.first()).cloned()
        }
        TvAction::Right if rail_all => None,
        // A rail's header row is above its cards: Up from any card reaches its See all, then
        // Up again leaves the rail. Without this the header was reachable by no key at all.
        TvAction::Up if !rail_all && zone.keys.last().is_some_and(|key| is_rail_all(key)) => {
            zone.keys.last().cloned()
        }
        // Right from the section list enters that section's first control, not the top of the
        // whole pane.
        TvAction::Right if focused.starts_with("settings-nav-") => zones
            .iter()
            .skip(zi + 1)
            .find(|zone| zone.parent == Some(zi) && zone.parent_at == Some(idx))
            .and_then(|zone| zone.keys.first().cloned()),
        TvAction::Right => None,
        TvAction::Up if row > 0 => Some(zone.keys[idx - cols].clone()),
        TvAction::Up => adjacent(&zones, sidebar, zi, idx, -1),
        TvAction::Down if idx + cols < zone.keys.len() => Some(zone.keys[idx + cols].clone()),
        TvAction::Down => adjacent(&zones, sidebar, zi, idx, 1),
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
    let at = zones[zi].parent_at.unwrap_or(idx);
    zone.keys
        .get(at.min(zone.keys.len().saturating_sub(1)))
        .cloned()
}

/// A rail's "See all": `rail-recent-0-all`, `rail-continue-watching-all`. The strip's own
/// `home-library-all` is a chip at the end of its row, not a header, so it must not match.
fn is_rail_all(key: &str) -> bool {
    key.starts_with("rail-") && key.ends_with("-all")
}

fn adjacent(zones: &[Zone], sidebar: bool, zi: usize, idx: usize, delta: isize) -> Option<String> {
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
        // Up/Down stop at the page's first or last row; the rail is reached with Left.
        return None;
    }
    let zone = &zones[target];
    // Entering a rail from a vertical list starts at its first item: the list's row number is
    // not a column position. A move into the hero always lands on its primary decision, not on
    // whichever decision happens to sit at the rail's column. Rail-to-rail moves keep the
    // column.
    let hero = zone
        .keys
        .first()
        .is_some_and(|key| key.starts_with("home-hero"));
    let source_cols = if zones[zi].columns == 0 {
        zones[zi].keys.len().max(1)
    } else {
        zones[zi].columns
    };
    let col = idx % source_cols;
    let at = if hero || (zones[zi].columns == 1 && zone.columns == 0) {
        0
    } else {
        let at = col.min(zone.keys.len().saturating_sub(1));
        // Entering a rail from above lands on a card, not on its "See all" header when the
        // column ran past the shorter rail's cards.
        if is_rail_all(&zone.keys[at]) && at > 0 {
            at - 1
        } else {
            at
        }
    };
    zone.keys.get(at).cloned()
}

/// The sidebar entry for the current screen, so Up/Left from content lands on the destination
/// rather than always on Home.
fn nav_key(s: &State) -> Option<String> {
    match &s.route {
        Route::Home => Some("nav-home".into()),
        Route::Libraries | Route::Library { .. } => Some("nav-libraries".into()),
        Route::Search { .. } => Some("nav-search".into()),
        Route::LiveTv => Some("nav-live".into()),
        Route::Settings => Some("nav-settings".into()),
        Route::Remote | Route::Downloads | Route::Seerr | Route::SeerrDetail { .. } => {
            Some("nav-libraries".into())
        }
        Route::Detail { .. } | Route::Person { .. } | Route::SeerrPerson { .. } => {
            match &s.detail_return {
                Route::Library { .. } => Some("nav-libraries".into()),
                _ => Some("nav-home".into()),
            }
        }
    }
}

fn zones(s: &State) -> (Vec<Zone>, bool) {
    if s.in_app.is_some() {
        return (in_app_zones(s), false);
    }
    if s.shelf.is_some() {
        return (shelf_zones(s), true);
    }
    match &s.route {
        Route::Home => (home_zones(s), true),
        Route::Libraries => (hub_zones(s), true),
        Route::Library { .. } => (library_zones(s), true),
        Route::Detail { .. } => (detail_zones(s), false),
        Route::Search { .. } => (search_zones(s), false),
        Route::Person { .. } => (person_zones(s), false),
        Route::Seerr => (seerr_zones(s), false),
        Route::Settings => (settings_zones(s), true),
        Route::LiveTv => (live_zones(s), true),
        Route::Remote => (remote_zones(s), true),
        Route::Downloads => (download_zones(s), false),
        Route::SeerrDetail { .. } => (seerr_detail_zones(s), false),
        _ => (Vec::new(), false),
    }
}

/// Whether `key` is one of the sidebar's nav rows.
pub fn in_sidebar(s: &State, key: &str) -> bool {
    let (zones, has_sidebar) = zones(s);
    has_sidebar
        && zones
            .first()
            .is_some_and(|zone| zone.keys.iter().any(|zone_key| zone_key == key))
}

fn sidebar(_s: &State) -> Zone {
    Zone {
        keys: [
            "nav-home",
            "nav-search",
            "nav-libraries",
            "nav-live",
            "nav-settings",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        columns: 1,
        parent: None,
        parent_at: None,
    }
}

fn hub_zones(s: &State) -> Vec<Zone> {
    let libraries: Vec<&jellyfin_api::LibraryView> = s.sidebar_libraries().collect();
    let recent: Vec<&jellyfin_api::LibraryView> = {
        let picked: Vec<_> = s
            .recent_libraries
            .iter()
            .filter_map(|id| libraries.iter().copied().find(|library| &library.id == id))
            .take(3)
            .collect();
        if picked.is_empty() {
            libraries.iter().copied().take(3).collect()
        } else {
            picked
        }
    };
    let mut zones = vec![sidebar(s)];
    zones.push(Zone {
        keys: vec!["hub-filter".into()],
        columns: 1,
        parent: Some(0),
        parent_at: None,
    });
    if !recent.is_empty() {
        zones.push(Zone {
            keys: recent
                .iter()
                .map(|library| format!("hub-recent-{}", library.id))
                .collect(),
            columns: 3,
            parent: Some(0),
            parent_at: None,
        });
    }
    if !libraries.is_empty() {
        zones.push(Zone {
            keys: libraries
                .iter()
                .map(|library| format!("hub-library-{}", library.id))
                .collect(),
            columns: 6,
            parent: Some(0),
            parent_at: None,
        });
    }
    let mut tools = vec![
        "hub-tool-downloads".to_string(),
        "hub-tool-remote".to_string(),
        "hub-tool-home-rails".to_string(),
    ];
    if s.big_screen {
        tools.push("hub-tool-controls".into());
    }
    let tool_count = tools.len();
    zones.push(Zone {
        keys: tools,
        columns: tool_count,
        parent: Some(0),
        parent_at: None,
    });
    if !s.playlists.is_empty() {
        zones.push(Zone {
            keys: s
                .playlists
                .iter()
                .map(|playlist| format!("hub-playlist-{}", playlist.id))
                .collect(),
            columns: 4,
            parent: Some(0),
            parent_at: None,
        });
    }
    let mut extras = vec!["hub-watchlist".to_string()];
    if !s.config.seerr_url.is_empty() {
        extras.push("hub-seerr".into());
    }
    zones.push(Zone {
        keys: extras,
        columns: 2,
        parent: Some(0),
        parent_at: None,
    });
    zones
}

fn item_key(rail: &str, item: &jellyfin_api::Item) -> String {
    format!("{rail}-{}", item.series_id.as_deref().unwrap_or(&item.id))
}

fn home_zones(s: &State) -> Vec<Zone> {
    let mut zones = vec![sidebar(s)];
    let visible =
        |items: &[jellyfin_api::Item]| items.iter().any(|item| !s.is_home_hidden(&item.id));
    if s.home_hero && s.hero_item().is_some() {
        zones.push(Zone {
            keys: vec!["home-hero-play".into(), "home-hero-open".into()],
            columns: 0,
            parent: Some(0),
            parent_at: None,
        });
    }
    if visible(&s.home_data.resume) {
        zones.push(Zone {
            keys: s
                .home_data
                .resume
                .iter()
                .filter(|item| !s.is_home_hidden(&item.id))
                .map(|item| item_key("rail-continue-watching", item))
                .chain(std::iter::once("rail-continue-watching-all".to_string()))
                .collect(),
            columns: 0,
            parent: Some(0),
            parent_at: None,
        });
    }
    if !s.home_data.libraries.is_empty() {
        zones.push(Zone {
            keys: s
                .sidebar_libraries()
                .take(8)
                .map(|library| format!("home-library-{}", library.id))
                .chain(std::iter::once("home-library-all".to_string()))
                .collect(),
            columns: 0,
            parent: Some(0),
            parent_at: None,
        });
    }
    for (index, recent) in s
        .home_data
        .latest
        .iter()
        .filter(|rail| rail.items.iter().any(|item| !s.is_home_hidden(&item.id)))
        .enumerate()
    {
        let mut keys: Vec<String> = recent
            .items
            .iter()
            .filter(|item| !s.is_home_hidden(&item.id))
            .map(|item| item_key(&format!("rail-recent-{index}"), item))
            .collect();
        keys.push(format!("rail-recent-{index}-all"));
        zones.push(Zone {
            keys,
            columns: 0,
            parent: Some(0),
            parent_at: None,
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
                .chain(std::iter::once("rail-next-up-all".to_string()))
                .collect(),
            columns: 0,
            parent: Some(0),
            parent_at: None,
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
            parent_at: None,
        });
    }
    if !s.home_data.collections.is_empty() {
        zones.push(Zone {
            keys: s
                .home_data
                .collections
                .iter()
                .filter(|item| !s.is_home_hidden(&item.id))
                .map(|item| item_key("rail-collections", item))
                .chain(std::iter::once("rail-collections-all".to_string()))
                .collect(),
            columns: 0,
            parent: Some(0),
            parent_at: None,
        });
    }
    zones
}

/// The settings screen is two panes: the section list on the left and its form on the right.
/// Rune's own traversal wrapped the rail (Up at Home landed on Settings) and treated a row of
/// two stepper buttons as one horizontal run, so Down inside a row moved right. The rows are
/// declared here in reading order; every content row knows which section it belongs to, so
/// Left lands on that section in the list and Right from a section enters its first control.
fn settings_zones(s: &State) -> Vec<Zone> {
    let nav = [
        "settings-nav-appearance",
        "settings-nav-playback",
        "settings-nav-subtitles",
        "settings-nav-keyboard",
        "settings-nav-metadata",
        "settings-nav-seerr",
        "settings-nav-downloads",
        "settings-nav-storage",
        "settings-nav-history",
        "settings-nav-account",
        "settings-nav-home-rails",
    ]
    .map(String::from)
    .to_vec();
    let mut zones = vec![
        sidebar(s),
        Zone {
            keys: nav,
            columns: 1,
            parent: Some(0),
            parent_at: None,
        },
    ];
    // A filtered form hides rows; leave those to Rune rather than step onto an absent key.
    if !s.settings_query.text.trim().is_empty() {
        return zones;
    }
    // One zone per visual row. `parent_at` is the section list row Left should land on.
    let mut rows: Vec<(&[&str], usize)> = Vec::new();
    if s.media {
        rows.push((
            &["mode-switch-desktop", "mode-switch-game", "mode-restart"],
            0,
        ));
    }
    let rest: Vec<(&[&str], usize)> = vec![
        (&["settings-search"], 0),
        (&["settings-home-rails"], 10),
        (&["setting-ui-mode-value"], 0),
        (&["setting-keyboard-value"], 0),
        (&["setting-high-contrast-value"], 0),
        (&["setting-light-theme-value"], 0),
        (&["setting-text-scale-value"], 0),
        (&["setting-home-hero-value"], 0),
        (&["setting-hover-preview-value"], 0),
        (&["setting-reduce-motion-value"], 0),
        (&["setting-autoplay-value"], 1),
        (&["setting-seek-step-value"], 1),
        (&["setting-skip-segments-value"], 1),
        (&["setting-still-watching-value"], 1),
        (&["setting-spoiler-guard-value"], 1),
        (&["setting-notifications-value"], 1),
        (&["setting-audio-passthrough-value"], 1),
        (&["setting-hdr-value"], 1),
        (&["setting-loudness-value"], 1),
        (&["setting-resume-rewind-value"], 1),
        (&["setting-volume-down", "setting-volume-up"], 1),
        (&["setting-downmix-value"], 1),
        (&["setting-audio-mode-value"], 1),
        (&["setting-subtitle-mode-value"], 2),
        (&["setting-subtitle-background-value"], 2),
        (&["setting-subtitle-font-value"], 2),
        (&["setting-subtitle-color-value"], 2),
        (&["setting-subtitle-outline-value"], 2),
        (&["setting-subtitle-style-value"], 2),
        (&["setting-subtitle-position-value"], 2),
        (&["setting-subtitle-language-value"], 2),
        (&["setting-audio-language-value"], 2),
        (
            &[
                "setting-subtitle-opacity-down",
                "setting-subtitle-opacity-up",
            ],
            2,
        ),
        (
            &[
                "setting-subtitle-plate-opacity-down",
                "setting-subtitle-plate-opacity-up",
            ],
            2,
        ),
        (&["setting-key-change-play-/-pause"], 3),
        (&["setting-key-change-mute"], 3),
        (&["setting-key-change-cycle-subtitles"], 3),
        (&["setting-key-change-skip-intro-/-credits"], 3),
        (&["setting-key-change-next-episode"], 3),
        (&["setting-key-change-previous-episode"], 3),
        (&["setting-key-change-fit-/-fill-the-picture"], 3),
        (&["setting-key-change-fullscreen"], 3),
        (&["settings-shortcuts-reset"], 3),
        (&["setting-omdb-key"], 4),
        (&["setting-omdb-get-key"], 4),
        (&["setting-seerr-url"], 5),
        (&["setting-seerr-user"], 5),
        (
            &[
                "setting-seerr-test",
                "setting-seerr-clear",
                "setting-seerr-open",
            ],
            5,
        ),
        (&["setting-downloads-path"], 6),
        (&["setting-downloads-path-apply"], 6),
        (&["setting-download-quality-value"], 6),
        (
            &[
                "setting-downloads-budget-down",
                "setting-downloads-budget-up",
            ],
            6,
        ),
        (&["settings-clear-images", "settings-remote"], 7),
        (&["settings-history-export"], 8),
        (&["settings-history-clear"], 8),
        (&["settings-reset", "settings-export"], 9),
        (&["settings-sign-out"], 9),
        (&["settings-quit"], 9),
    ];
    rows.extend(rest);
    for (keys, section) in rows {
        zones.push(Zone {
            keys: keys.iter().map(|key| (*key).to_string()).collect(),
            columns: keys.len(),
            parent: Some(1),
            parent_at: Some(section),
        });
    }
    zones
}

fn library_zones(s: &State) -> Vec<Zone> {
    let sidebar = sidebar(s);
    let Some(library) = &s.library else {
        return vec![sidebar];
    };
    let playlist = library.collection_type.as_deref() == Some("playlists");
    let tv = library.collection_type.as_deref() == Some("tvshows");
    let playable = library.items.iter().any(crate::state::is_playable);
    let mut header = vec![ZONE_LIBRARY_BACK.to_string()];
    if !playlist {
        if !tv && playable {
            header.push("library-play-all".into());
        }
        if tv || playable {
            header.push("library-shuffle".into());
        }
    }
    // The toolbar wraps onto two lines at 720p: the sort row, then the filters. One zone for
    // both made Right at the end of the sorts jump back left to the first filter.
    let mut lead = vec!["library-search".to_string()];
    if playlist {
        lead.push("playlist-edit".into());
    } else {
        lead.extend(
            ["sort-name", "sort-added", "sort-rating", "sort-direction"]
                .into_iter()
                .map(String::from),
        );
    }
    let mut trail: Vec<String> = Vec::new();
    if !playlist {
        trail.extend(
            [
                "filter-unwatched",
                "filter-favorites",
                "library-genres",
                "library-filters",
                "library-select",
            ]
            .into_iter()
            .map(String::from),
        );
    }
    trail.extend(["view-grid", "view-list"].into_iter().map(String::from));
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
    let mut zones = vec![
        sidebar,
        Zone {
            keys: header,
            columns: 0,
            parent: Some(0),
            parent_at: None,
        },
        Zone {
            keys: lead,
            columns: 0,
            parent: Some(0),
            parent_at: None,
        },
        Zone {
            keys: trail,
            columns: 0,
            parent: Some(0),
            parent_at: None,
        },
    ];
    if library.filters_open {
        zones.extend(filters_zones(s, library));
    } else if library.genres_open {
        let mut genre_keys = vec!["genre-all".to_string()];
        genre_keys.extend(
            library
                .genres
                .iter()
                .map(|genre| format!("genre-{}", genre.id)),
        );
        zones.push(Zone {
            keys: genre_keys,
            columns: 0,
            parent: Some(0),
            parent_at: None,
        });
    }
    zones.push(Zone {
        keys,
        columns: if grid { library.columns.max(1) } else { 1 },
        parent: Some(0),
        parent_at: None,
    });
    zones
}

/// The inline filter sheet as rows, so Up/Down walk it and the page scrolls to each row.
fn filters_zones(s: &State, library: &crate::library::LibraryState) -> Vec<Zone> {
    let filtered = library.unwatched
        || library.favorites
        || library.genre.is_some()
        || library.letter.is_some();
    let mut filters = vec!["filters-unwatched".to_string(), "filters-favorites".into()];
    if filtered {
        filters.push("filters-clear".into());
    }
    // The sheet's header carries its own way out; it belongs to the first zone so the D-pad
    // can reach it without opening another layer.
    filters.push("library-filters-done".into());
    let sorts = [
        "filters-sort-name",
        "filters-sort-added",
        "filters-sort-rating",
        "filters-sort-played",
        "filters-sort-release",
        "filters-sort-runtime",
        "filters-sort-plays",
        "filters-sort-random",
        "filters-sort-direction",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let mut saved = Vec::new();
    for (index, _) in s.saved_views.iter().enumerate() {
        saved.push(format!("saved-view-{index}"));
        saved.push(format!("saved-view-delete-{index}"));
    }
    saved.push("saved-view-save".into());
    let mut deep = vec![
        "filter-year-down".to_string(),
        "filter-year-up".into(),
        "filter-rating".into(),
        "filter-audio-language".into(),
        "filter-subtitle-language".into(),
        "filter-has-subtitles".into(),
        "filter-4k".into(),
        "filter-missing".into(),
        "filter-unaired".into(),
    ];
    if library.collection_type.as_deref() == Some("tvshows") {
        deep.push("filter-status".into());
    }
    deep.push("filter-studios".into());
    deep.push("filter-tags".into());
    let mut zones = vec![
        Zone {
            keys: filters,
            columns: 0,
            parent: Some(0),
            parent_at: None,
        },
        Zone {
            keys: sorts,
            columns: 0,
            parent: Some(0),
            parent_at: None,
        },
    ];
    if !saved.is_empty() {
        zones.push(Zone {
            keys: saved,
            columns: 0,
            parent: Some(0),
            parent_at: None,
        });
    }
    zones.push(Zone {
        keys: deep,
        columns: 0,
        parent: Some(0),
        parent_at: None,
    });
    if !library.genres.is_empty() {
        zones.push(Zone {
            keys: library
                .genres
                .iter()
                .map(|genre| format!("filter-genre-{}", genre.id))
                .collect(),
            columns: 0,
            parent: Some(0),
            parent_at: None,
        });
    }
    if library.studios_open {
        let mut studios = vec!["studio-any".to_string()];
        studios.extend(
            library
                .studios
                .iter()
                .take(200)
                .map(|studio| format!("studio-{}", studio.id)),
        );
        zones.push(Zone {
            keys: studios,
            columns: 0,
            parent: Some(0),
            parent_at: None,
        });
    }
    if library.tags_open {
        let mut tags = vec!["tag-any".to_string()];
        tags.extend((0..library.tags.len().min(200)).map(|index| format!("tag-{index}")));
        zones.push(Zone {
            keys: tags,
            columns: 0,
            parent: Some(0),
            parent_at: None,
        });
    }
    zones
}

const ZONE_LIBRARY_BACK: &str = "library-back";

fn shelf_zones(s: &State) -> Vec<Zone> {
    let Some(shelf) = s.shelf else {
        return Vec::new();
    };
    let items: &[jellyfin_api::Item] = match shelf {
        crate::state::HomeShelf::ContinueWatching => &s.home_data.resume,
        crate::state::HomeShelf::NextUp => &s.home_data.next_up,
        crate::state::HomeShelf::RecentlyAdded => s
            .home_data
            .latest
            .first()
            .map_or(&[], |rail| rail.items.as_slice()),
    };
    let card_w = crate::theme::CARD_W + crate::theme::CARD_GAP;
    let width = s.window_size.0.max(900.0) - crate::theme::SIDEBAR_W - 2.0 * crate::theme::gutter();
    let columns = ((width / card_w).floor() as usize).clamp(2, 12);
    let cards: Vec<String> = items
        .iter()
        .map(|item| format!("shelf-{}", item.series_id.as_deref().unwrap_or(&item.id)))
        .collect();
    let mut zones = vec![
        sidebar(s),
        Zone {
            keys: vec!["shelf-back".into()],
            columns: 1,
            parent: Some(0),
            parent_at: None,
        },
    ];
    if !cards.is_empty() {
        zones.push(Zone {
            keys: cards,
            columns,
            parent: Some(0),
            parent_at: None,
        });
    }
    zones
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
    // Mirror `detail::detail_screen`: Download sits between Played and Playlist, and reaches an
    // episode from a series page by downloading the one Play would start.
    if !is_boxset && detail.play_target().is_some() {
        actions.push("detail-download".into());
    }
    if !is_boxset && crate::state::is_playable(&detail.item) {
        actions.push("detail-playlist".into());
        actions.push("detail-collection-add".into());
    }

    let mut zones = vec![Zone {
        keys: vec!["detail-back".into()],
        columns: 1,
        parent: None,
        parent_at: None,
    }];
    // The hero's own facts: the IMDb fallback link and the version chips sit under the title,
    // above the actions. The IMDb link used to be declared after them, so Down from Play went
    // up to it.
    let mut facts = Vec::new();
    if detail.item.media_sources.len() > 1 {
        facts.extend(
            detail
                .item
                .media_sources
                .iter()
                .map(|source| format!("detail-version-{}", source.id)),
        );
    }
    if crate::detail::technical_line(&detail.item).is_some() {
        facts.push("detail-tech-toggle".into());
    }
    if detail.item.provider_ids.contains_key("Imdb")
        && !detail.item.provider_ids.contains_key("Tmdb")
    {
        facts.push("detail-imdb-link".into());
    }
    if !facts.is_empty() {
        zones.push(Zone {
            keys: facts,
            columns: 0,
            parent: Some(0),
            parent_at: None,
        });
    }
    zones.push(Zone {
        keys: actions,
        columns: 0,
        parent: Some(0),
        parent_at: None,
    });
    if !detail.seasons.is_empty() {
        zones.push(Zone {
            keys: detail
                .seasons
                .iter()
                .map(|season| format!("season-{}", season.id))
                .collect(),
            columns: 0,
            parent: Some(0),
            parent_at: None,
        });
    }
    // The page then reads: the server's next episode, the season's actions, the episode list.
    if detail.next_up.is_some() {
        zones.push(Zone {
            keys: vec!["detail-next-up-card".into()],
            columns: 1,
            parent: Some(0),
            parent_at: None,
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
            parent_at: None,
        });
        zones.push(Zone {
            keys: detail
                .episodes
                .iter()
                .map(|episode| format!("detail-episode-{}", episode.id))
                .collect(),
            columns: 1,
            parent: Some(0),
            parent_at: None,
        });
    }
    if !s.detail_collections.is_empty() {
        zones.push(Zone {
            keys: s
                .detail_collections
                .iter()
                .map(|collection| format!("detail-collection-{}", collection.id))
                .collect(),
            columns: 0,
            parent: Some(0),
            parent_at: None,
        });
    }
    if !detail.item.people.is_empty() {
        zones.push(Zone {
            keys: detail
                .item
                .people
                .iter()
                .take(16)
                .map(|person| format!("detail-cast-{}", person.id))
                .collect(),
            columns: 0,
            parent: Some(0),
            parent_at: None,
        });
    }
    if !s.detail_extras.is_empty() {
        zones.push(Zone {
            keys: s
                .detail_extras
                .iter()
                .map(|extra| format!("detail-extra-{}", extra.id))
                .collect(),
            columns: 0,
            parent: Some(0),
            parent_at: None,
        });
    }
    if !detail.similar.is_empty() {
        let mut seen = std::collections::HashSet::new();
        zones.push(Zone {
            keys: detail
                .similar
                .iter()
                .filter(|item| {
                    seen.insert(
                        item.series_id
                            .as_deref()
                            .unwrap_or(item.id.as_str())
                            .to_string(),
                    )
                })
                .take(12)
                .map(|item| format!("similar-{}", item.series_id.as_deref().unwrap_or(&item.id)))
                .collect(),
            columns: 0,
            parent: Some(0),
            parent_at: None,
        });
    }
    zones
}

fn search_zones(s: &State) -> Vec<Zone> {
    let results = &s.search.results;
    let mut zones = vec![
        sidebar(s),
        Zone {
            keys: vec!["search-input".into()],
            columns: 1,
            parent: Some(0),
            parent_at: None,
        },
    ];
    let rail = |prefix: &str, items: &[jellyfin_api::Item]| {
        // Mirror the screen: six cards while collapsed, every card when expanded, plus the
        // header's See all / Show less. A zone that names an undrawn card strands the router.
        let shown = if s.is_search_expanded(prefix) {
            items.len()
        } else {
            items.len().min(6)
        };
        let mut keys: Vec<String> = items
            .iter()
            .take(shown)
            .map(|item| format!("{prefix}-{}", item.id))
            .collect();
        if items.len() > 6 {
            keys.push(format!("{prefix}-all"));
        }
        Zone {
            keys,
            columns: 0,
            parent: Some(0),
            parent_at: None,
        }
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
            parent_at: None,
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
            parent_at: None,
        });
    }
    zones
}

fn person_zones(s: &State) -> Vec<Zone> {
    let Some(person) = &s.person else {
        return Vec::new();
    };
    let mut zones = vec![
        sidebar(s),
        Zone {
            keys: vec!["person-back".into()],
            columns: 1,
            parent: Some(0),
            parent_at: None,
        },
    ];
    // The screen shows the first five as "Known for" only when more than five exist, and the
    // rest as "More in your library". The zones have to split the same way, or the cards on one
    // side of the split fall out of the D-pad graph.
    let split = if person.items.len() > 5 { 5 } else { 0 };
    let key = |item: &jellyfin_api::Item| {
        format!("person-{}", item.series_id.as_deref().unwrap_or(&item.id))
    };
    let known: Vec<String> = person.items.iter().take(split).map(key).collect();
    if !known.is_empty() {
        zones.push(Zone {
            keys: known,
            columns: 0,
            parent: Some(0),
            parent_at: None,
        });
    }
    let grid: Vec<String> = person.items.iter().skip(split).map(key).collect();
    if !grid.is_empty() {
        zones.push(Zone {
            keys: grid,
            columns: 5,
            parent: Some(0),
            parent_at: None,
        });
    }
    zones
}

fn seerr_zones(s: &State) -> Vec<Zone> {
    use crate::state::{DiscoverKind, DiscoverMode, SeerrTab};

    let mut zones = vec![sidebar(s)];
    let mut toolbar = vec![
        "seerr-back",
        "seerr-refresh",
        "seerr-tab-discover",
        "seerr-tab-requests",
    ];
    if s.seerr.tab == SeerrTab::Discover {
        // The search field owns Left/Right, so it ends the toolbar row; the mode tabs are a
        // row of their own below it.
        toolbar.push("seerr-search");
    }
    zones.push(Zone {
        keys: toolbar.into_iter().map(String::from).collect(),
        columns: 0,
        parent: Some(0),
        parent_at: None,
    });
    match s.seerr.tab {
        SeerrTab::Discover => {
            zones.push(Zone {
                keys: vec!["seerr-mode-explore".into(), "seerr-mode-browse".into()],
                columns: 0,
                parent: Some(0),
                parent_at: None,
            });
            if s.seerr.discover_mode == DiscoverMode::Browse {
                // One zone per filter row, then the result grid.
                zones.push(Zone {
                    keys: [
                        "seerr-browse-films",
                        "seerr-browse-series",
                        "seerr-browse-popular",
                        "seerr-browse-rating",
                        "seerr-browse-newest",
                    ]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                    columns: 0,
                    parent: Some(0),
                    parent_at: None,
                });
                zones.push(Zone {
                    keys: [
                        "seerr-browse-year-any",
                        "seerr-browse-year-2020",
                        "seerr-browse-year-2010",
                        "seerr-browse-year-2000",
                        "seerr-browse-year-1990",
                        "seerr-browse-year-older",
                    ]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                    columns: 0,
                    parent: Some(0),
                    parent_at: None,
                });
                let mut genres = vec!["seerr-browse-genre-all".to_string()];
                genres.extend(
                    s.seerr
                        .discover
                        .genres()
                        .iter()
                        .map(|genre| format!("seerr-browse-genre-{}", genre.id)),
                );
                zones.push(Zone {
                    keys: genres,
                    columns: 0,
                    parent: Some(0),
                    parent_at: None,
                });
                let hubs = match s.seerr.discover.filters.kind {
                    DiscoverKind::Movies => crate::state::STUDIOS,
                    DiscoverKind::Series => crate::state::NETWORKS,
                };
                let mut hub_keys = vec!["seerr-browse-hub-all".to_string()];
                hub_keys.extend(hubs.iter().map(|(_, id)| format!("seerr-browse-hub-{id}")));
                zones.push(Zone {
                    keys: hub_keys,
                    columns: 0,
                    parent: Some(0),
                    parent_at: None,
                });
                let mut cards: Vec<String> = s
                    .seerr
                    .discover
                    .browse
                    .iter()
                    .map(|item| format!("seerr-card-{}-{}", item.media_type, item.id))
                    .collect();
                if s.seerr.discover.browse_pages > 0
                    && s.seerr.discover.browse_page < s.seerr.discover.browse_pages
                {
                    cards.push("seerr-browse-more".into());
                }
                if !cards.is_empty() {
                    zones.push(Zone {
                        keys: cards,
                        columns: 5,
                        parent: Some(0),
                        parent_at: None,
                    });
                }
            } else {
                let cards: Vec<String> = s
                    .seerr
                    .discover
                    .trending
                    .iter()
                    .chain(s.seerr.discover.movies.iter())
                    .chain(s.seerr.discover.tv.iter())
                    .map(|item| format!("seerr-card-{}-{}", item.media_type, item.id))
                    .collect();
                if !cards.is_empty() {
                    zones.push(Zone {
                        keys: cards,
                        columns: 5,
                        parent: Some(0),
                        parent_at: None,
                    });
                }
            }
        }
        SeerrTab::Requests => {
            let mut keys = vec!["seerr-filter-active".to_string(), "seerr-filter-all".into()];
            for request in &s.seerr.requests {
                keys.push(format!("seerr-request-{}", request.id));
                let has_open = request.media.jellyfin_media_id.is_some()
                    || s.seerr
                        .library_id(
                            &request.media_type,
                            request.media.tmdb_id.unwrap_or_default(),
                        )
                        .is_some();
                if has_open {
                    keys.push(format!("seerr-open-{}", request.id));
                }
            }
            zones.push(Zone {
                keys,
                columns: 1,
                parent: Some(0),
                parent_at: None,
            });
        }
    }
    zones
}

fn live_zones(s: &State) -> Vec<Zone> {
    let mut zones = vec![
        sidebar(s),
        Zone {
            keys: [
                "live-guide-window",
                "live-guide-toggle",
                "live-favorites",
                "live-refresh",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            columns: 0,
            parent: Some(0),
            parent_at: None,
        },
    ];
    if s.live.guide_open {
        // One zone per channel, one key per programme in the window: Left/Right walk the
        // timeline and Down changes channel. A channel with no guide data keeps its row key
        // so an empty guide is still reachable.
        for channel in &s.live.channels {
            let keys = crate::screens::live_guide_program_keys(s, channel);
            if !keys.is_empty() {
                let columns = keys.len();
                zones.push(Zone {
                    keys,
                    columns,
                    parent: Some(0),
                    parent_at: None,
                });
            }
        }
        zones.push(Zone {
            keys: vec!["live-preview-watch".into()],
            columns: 1,
            parent: Some(2),
            parent_at: None,
        });
        return zones;
    }
    // One row per channel: the card itself, its reminder (when there is a next program) and
    // its favourite toggle.
    for channel in &s.live.channels {
        let mut keys = vec![format!("live-channel-{}", channel.id)];
        if s.live.next.contains_key(&channel.id) {
            keys.push(format!("live-remind-{}", channel.id));
        }
        keys.push(format!("live-favorite-{}", channel.id));
        zones.push(Zone {
            keys,
            columns: 0,
            parent: Some(0),
            parent_at: None,
        });
    }
    zones
}

/// One zone per download: play, pause or resume, keep, remove, in that order.
fn download_zones(s: &State) -> Vec<Zone> {
    let mut zones = vec![sidebar(s)];
    for entry in &s.downloads.entries {
        let id = &entry.item.id;
        let mut keys = Vec::new();
        if entry.state.is_watchable() {
            keys.push(format!("download-play-{id}"));
        }
        match entry.state {
            crate::downloads::State::Downloading => keys.push(format!("download-pause-{id}")),
            crate::downloads::State::Paused => keys.push(format!("download-resume-{id}")),
            crate::downloads::State::Queued | crate::downloads::State::Failed => {
                keys.push(format!("download-retry-{id}"))
            }
            crate::downloads::State::Done => {}
        }
        keys.push(format!("download-keep-{id}"));
        keys.push(format!("download-remove-{id}"));
        zones.push(Zone {
            keys,
            columns: 0,
            parent: Some(0),
            parent_at: None,
        });
    }
    zones
}

fn remote_zones(s: &State) -> Vec<Zone> {
    let mut zones = vec![
        sidebar(s),
        Zone {
            keys: vec!["remote-refresh".into()],
            columns: 1,
            parent: Some(0),
            parent_at: None,
        },
    ];
    // One row per session: the card, then its pause/seek/stop controls when it is playing.
    for session in &s.remote.sessions {
        let mut keys = vec![format!("remote-session-{}", session.id)];
        if session.play_state.is_some() {
            keys.push(format!("remote-pause-{}", session.id));
            keys.push(format!("remote-seek-{}", session.id));
            keys.push(format!("remote-stop-{}", session.id));
        }
        zones.push(Zone {
            keys,
            columns: 0,
            parent: Some(0),
            parent_at: None,
        });
    }
    zones
}

fn seerr_detail_zones(s: &State) -> Vec<Zone> {
    let Some(detail) = &s.seerr.detail else {
        return Vec::new();
    };
    let mut zones = vec![
        sidebar(s),
        Zone {
            keys: vec!["seerr-detail-back".into()],
            columns: 1,
            parent: Some(0),
            parent_at: None,
        },
    ];
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
            parent_at: None,
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
            parent_at: None,
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
            parent_at: None,
        });
    }
    zones
}

fn in_app_zones(s: &State) -> Vec<Zone> {
    if !s.osd_visible {
        return Vec::new();
    }
    // The interruption cards are the screen while they are up.
    if s.still_watching_open {
        return vec![Zone {
            keys: vec![
                "still-watching-continue".into(),
                "still-watching-stop".into(),
            ],
            columns: 0,
            parent: None,
            parent_at: None,
        }];
    }
    if s.resume_conflict.is_some() {
        return vec![Zone {
            keys: vec![
                "resume-conflict-local".into(),
                "resume-conflict-server".into(),
            ],
            columns: 0,
            parent: None,
            parent_at: None,
        }];
    }
    // The queue is a grid of rows, each with reorder and remove buttons: four keys per row.
    if s.in_app_queue_open {
        let mut keys = Vec::new();
        for at in 0..s.queue.len().min(12) {
            keys.push(format!("in-app-queue-{at}"));
            keys.push(format!("queue-up-{at}"));
            keys.push(format!("queue-down-{at}"));
            keys.push(format!("queue-remove-{at}"));
        }
        if !s.queue.is_empty() {
            keys.push("queue-save-playlist".into());
        }
        return vec![Zone {
            keys,
            columns: 4,
            parent: None,
            parent_at: None,
        }];
    }
    // The end-of-episode card replaces the transport: its three decisions are the screen.
    if s.in_app_hover.is_none()
        && s.next_up_eligible()
        && !s.player_panel_open()
        && !s.player_gear_visible()
    {
        return vec![Zone {
            keys: vec![
                "in-app-next-play".into(),
                "in-app-next-credits".into(),
                "in-app-next-dismiss".into(),
            ],
            columns: 0,
            parent: None,
            parent_at: None,
        }];
    }
    // A live stream has no timeline: the transport is the whole OSD.
    let live = s.in_app.as_ref().is_some_and(|session| session.live);
    let mut zones = vec![
        // The top bar's Back belongs to the remote too, not only to B.
        Zone {
            keys: vec!["in-app-stop".into()],
            columns: 1,
            parent: None,
            parent_at: None,
        },
        Zone {
            keys: if live {
                vec!["in-app-pause".into()]
            } else {
                vec![
                    "in-app-back-10".into(),
                    "in-app-pause".into(),
                    "in-app-forward-10".into(),
                ]
            },
            columns: 0,
            parent: None,
            parent_at: None,
        },
    ];
    if !live {
        zones.push(Zone {
            keys: vec!["in-app-seek".into()],
            columns: 1,
            parent: None,
            parent_at: None,
        });
        if s.offered_in_app_segment().is_some() {
            zones.push(Zone {
                keys: vec!["in-app-skip-segment".into()],
                columns: 1,
                parent: None,
                parent_at: None,
            });
        }
    }
    let mut control_keys: Vec<String> = Vec::new();
    if s.queue.len() > 1 {
        control_keys.push("in-app-queue".into());
    }
    control_keys.extend(
        [
            "in-app-volume-slider",
            "in-app-mute",
            "in-app-fullscreen",
            "in-app-gear",
        ]
        .into_iter()
        .map(String::from),
    );
    zones.push(Zone {
        keys: control_keys,
        columns: 0,
        parent: None,
        parent_at: None,
    });
    zones
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
        // Up from a card reaches the rail's See all header, then the lane above it.
        assert_eq!(
            step(&state, Some(first), TvAction::Up).as_deref(),
            Some("rail-continue-watching-all")
        );
        assert_eq!(
            step(&state, Some("rail-continue-watching-all"), TvAction::Up).as_deref(),
            Some("home-hero-play")
        );
        // Down switches to the libraries strip, the next lane.
        let down = step(&state, Some(first), TvAction::Down).expect("a lane below");
        assert!(down.starts_with("home-library-"), "{down}");
    }

    #[test]
    fn guide_programmes_move_in_the_timeline_and_down_the_channels() {
        let mut state = home();
        state.route = Route::LiveTv;
        state.live.channels = crate::demo::live_channels();
        state.live.guide = crate::demo::live_guide();
        state.live.guide_open = true;
        let first = crate::screens::live_guide_program_keys(&state, &state.live.channels[0]);
        assert!(first.len() > 1, "the demo guide has a timeline");
        assert!(contains(&state, &first[0]));
        assert_eq!(
            step(&state, Some(&first[0]), TvAction::Right).as_deref(),
            Some(first[1].as_str())
        );
        let second_row = crate::screens::live_guide_program_keys(&state, &state.live.channels[1]);
        assert_eq!(
            step(&state, Some(&first[0]), TvAction::Down).as_deref(),
            Some(second_row[0].as_str()),
            "Down keeps the programme index in the next channel"
        );
    }

    #[test]
    fn the_compact_hero_is_one_zone_above_the_rails() {
        let mut state = home();
        state.home_hero = true;
        assert!(contains(&state, "home-hero-play"));
        assert!(contains(&state, "home-hero-open"));
        assert_eq!(
            step(&state, Some("home-hero-play"), TvAction::Right).as_deref(),
            Some("home-hero-open")
        );
        assert_eq!(
            step(&state, Some("home-hero-play"), TvAction::Left).as_deref(),
            Some("nav-home")
        );
        assert_eq!(
            step(&state, Some("nav-home"), TvAction::Right).as_deref(),
            Some("home-hero-play")
        );
        assert_eq!(
            step(&state, Some("home-hero-play"), TvAction::Down).as_deref(),
            Some("rail-continue-watching-quiet-floor")
        );
        // Up from a rail card reaches its See all header; Up again is the hero's primary
        // decision, not Details.
        assert_eq!(
            step(&state, Some("rail-continue-watching-service"), TvAction::Up).as_deref(),
            Some("rail-continue-watching-all")
        );
        assert_eq!(
            step(&state, Some("rail-continue-watching-all"), TvAction::Up).as_deref(),
            Some("home-hero-play")
        );
        // Off is the default: the sidebar's Right reaches the first rail directly.
        state.home_hero = false;
        assert!(!contains(&state, "home-hero-play"));
        assert_eq!(
            step(&state, Some("nav-home"), TvAction::Right).as_deref(),
            Some("rail-continue-watching-quiet-floor")
        );
    }

    #[test]
    fn the_hero_skips_a_hidden_first_entry() {
        let mut state = home();
        state.home_hero = true;
        let first = state.home_data.resume[0].id.clone();
        state.hide_from_home(first);
        assert!(state.hero_item().is_some());
        assert!(contains(&state, "home-hero-play"));
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
        // Right from the rail enters the billboard's first decision.
        assert_eq!(
            step(&state, Some("nav-home"), TvAction::Right).as_deref(),
            Some("home-hero-play")
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
            Some("library-film-008"),
            "six columns: down moves a row"
        );
        assert_eq!(
            step(&state, Some("library-film-000"), TvAction::Left).as_deref(),
            Some("nav-libraries")
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
            Some("detail-imdb-link"),
            "the hero's facts sit under the title, above the actions"
        );
        assert_eq!(
            step(&state, Some("detail-imdb-link"), TvAction::Down).as_deref(),
            Some("detail-play")
        );
        assert_eq!(
            step(&state, Some("detail-play"), TvAction::Right).as_deref(),
            Some("detail-restart"),
            "the resumed series offers Start over"
        );
        assert_eq!(
            step(&state, Some("detail-play"), TvAction::Up).as_deref(),
            Some("detail-imdb-link")
        );
        // The hero's facts come first; the page then reads season chips, Next up, the season's
        // actions and the episode list.
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
    fn a_movie_detail_reaches_its_cast_extras_and_similar_titles() {
        let mut state = home();
        state.route = Route::Detail {
            id: "metropolis".into(),
        };
        // The movie fixture is enough for the hero; borrow the series fixture's people so the
        // cast rail has someone to focus.
        let mut detail = crate::demo::movie_detail();
        detail.item.people = crate::demo::detail().item.people;
        state.detail = Some(detail);
        state.detail_extras = crate::demo::extras();
        let fact = step(&state, Some("detail-play"), TvAction::Up).expect("the version chips");
        assert!(fact.starts_with("detail-version-"), "{fact}");
        assert_eq!(
            step(&state, Some(&fact), TvAction::Down).as_deref(),
            Some("detail-play")
        );
        let cast = step(&state, Some("detail-play"), TvAction::Down).expect("the cast is below");
        assert!(cast.starts_with("detail-cast-"), "{cast}");
        // Right walks the cast rail; Down leaves it for the extras.
        let next_cast = step(&state, Some(&cast), TvAction::Right).expect("another cast member");
        assert!(next_cast.starts_with("detail-cast-"), "{next_cast}");
        let extra = step(&state, Some(&cast), TvAction::Down).expect("the extras rail");
        assert!(extra.starts_with("detail-extra-"), "{extra}");
        let similar = step(&state, Some(&extra), TvAction::Down).expect("More like this");
        assert!(similar.starts_with("similar-"), "{similar}");
        assert_eq!(
            step(&state, Some(&similar), TvAction::Up).as_deref(),
            Some(extra.as_str())
        );
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
        assert!(down.starts_with("person-"), "{down}");
        let grid = step(&state, Some(&down), TvAction::Down).expect("the grid");
        assert!(grid.starts_with("person-"), "{grid}");
        assert_ne!(
            grid, down,
            "the grid must be a different row from Known for"
        );
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
        state.remote.sessions = crate::demo::sessions();
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
    fn the_end_card_actions_are_the_screen() {
        let mut state = home();
        let mut session = crate::demo::in_app();
        session.position_ms = 47 * 60_000 + 40_000;
        state.in_app = Some(session);
        state.in_app_next = Some(crate::demo::next_episode());
        state.osd_visible = true;
        assert!(contains(&state, "in-app-next-play"));
        assert!(!contains(&state, "in-app-seek"));
        assert_eq!(
            step(&state, Some("in-app-next-play"), TvAction::Right).as_deref(),
            Some("in-app-next-credits")
        );
        assert_eq!(
            step(&state, Some("in-app-next-credits"), TvAction::Right).as_deref(),
            Some("in-app-next-dismiss")
        );
        assert_eq!(step(&state, Some("in-app-next-play"), TvAction::Down), None);
    }

    #[test]
    fn a_live_osd_has_no_timeline_to_scrub() {
        let mut state = home();
        let mut session = crate::demo::in_app();
        session.live = true;
        state.in_app = Some(session);
        state.osd_visible = true;
        assert!(!contains(&state, "in-app-seek"));
        // A live transport has one control; the bottom row is one Down away.
        assert_eq!(step(&state, Some("in-app-pause"), TvAction::Right), None);
        assert_eq!(
            step(&state, Some("in-app-pause"), TvAction::Down).as_deref(),
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
            Some("in-app-volume-slider")
        );
        assert_eq!(
            step(&state, Some("in-app-pause"), TvAction::Up).as_deref(),
            Some("in-app-stop")
        );
        assert_eq!(
            step(&state, Some("in-app-seek"), TvAction::Up).as_deref(),
            Some("in-app-back-10")
        );
        assert_eq!(
            step(&state, Some("in-app-pause"), TvAction::Right).as_deref(),
            Some("in-app-forward-10")
        );
    }
}
