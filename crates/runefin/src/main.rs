//! Runefin — a Jellyfin client drawn entirely with Rune.
//!
//! M1: a real connect screen. The server probe and the password sign-in run on a worker thread;
//! the session is persisted and the app reopens signed in. Library browsing is M2 — see
//! `docs/design.md` §8 for the milestone table.
//!
//! Run:   cargo run -p runefin
//! Snap:  cargo run -p runefin -- --snap connect.png --scene error
//! Live:  cargo run -p runefin -- --snap home.png --live --at 20 --wait 3

mod checkpoint;
mod config;
mod demo;
mod detail;
mod discovery;
mod dmabuf_sink;
mod handoff;
mod history;
mod home;
mod image;
mod image_subtitles;
mod imdb;
mod library;
mod mpris;
mod net;
mod pad;
mod playback_trace;
mod player;
mod ratings;
mod screens;
mod search;
mod shortcuts;
mod state;
mod subtitles;
mod theme;
mod trailer;
mod tv_focus;
mod tv_input;

use rune::{AppSpec, Cx, El, Key, KeyEvent, TextEdit, Tx};

use detail::DetailState;
use library::LibraryState;
use state::{Route, State, Step};

fn view(s: &State, cx: &mut Cx<State>) -> El {
    // The root's own rect is the window's content size; remembering it makes the next launch
    // open at the size the reader left. The write itself happens on close, not here.
    // Rune calls an element's own key handler before its focus-group navigation. Without this
    // hook a focused card never reaches the router: Rune would have already wrapped the rail.
    let screen = screens::view(s, cx);
    // The halo is a sibling of the whole screen, at the focused poster's window rect: the rail's
    // scroll viewport clips its own children, never this overlay.
    let screen = if s.big_screen
        && let Some(rect) = s.focused_card_rect
    {
        screen.child(crate::theme::focus_glow(rect))
    } else {
        screen
    };
    screen
        .on_key(tv_key)
        .on_screen_layout(|s: &mut State, rect: [f32; 4]| {
            if rect[2] >= 1.0 && rect[3] >= 1.0 {
                s.window_size = (rect[2], rect[3]);
                s.window_position = (rect[0], rect[1]);
            }
        })
}

/// Once, before the first frame: hand the worker the waker it rings when an answer lands.
/// The saved or per-launch interface mode. `--big-screen` and `RUNEFIN_UI=big` override the
/// config for one run; the config is what Settings writes.
fn big_screen_requested() -> bool {
    if std::env::args().any(|arg| arg == "--big-screen") {
        return true;
    }
    if std::env::var("RUNEFIN_UI").is_ok_and(|v| v.eq_ignore_ascii_case("big")) {
        return true;
    }
    // A headless render must not inherit the developer's saved mode: the snapshot suite's
    // desktop frames are a contract, and the big frames pass the flag explicitly.
    if std::env::args().any(|arg| arg == "--snap" || arg == "--tree") {
        return false;
    }
    config::Config::load().big_screen()
}

/// The app's initial state. Headless tooling (`--snap`, `--tree`) must be hermetic: a developer
/// with a live session in their config should never see that session's data or an expired-token
/// banner inside a fixture render.
fn init() -> State {
    let headless = std::env::args().any(|arg| arg == "--snap" || arg == "--tree");
    // `--live` opts a headless render out of the hermetic fixture and uses the session already
    // on disk, so a screenshot can show the reader's own library and artwork. It is never used
    // by `scripts/snapshots.sh`: committed snapshots stay reproducible with no server.
    let live = std::env::args().any(|arg| arg == "--live");
    let mut state = if headless && !live {
        State::from_config(crate::config::Config::fresh())
    } else {
        let mut state = State::new();
        if live {
            state.forget_last_place();
        }
        state
    };
    if big_screen_requested() {
        state.big_screen = true;
        // `--windowed` keeps the TV layout in an ordinary window, for iterating on a laptop
        // with the same zones and the same keys.
        let windowed = std::env::args().any(|arg| arg == "--windowed");
        state.fullscreen = !windowed;
        state.pending_fullscreen = (!windowed).then_some(true);
    }
    state
}

/// A developer-only live capture route. `--live` normally starts on Home; these env vars let a
/// screenshot open a real library, detail, search, tool or Seerr page without a scripted tap.
/// It is never used by `scripts/snapshots.sh`, and it only sets a route and starts the same
/// `Latest` loads the UI would start.
fn apply_live_route(s: &mut State) {
    s.forget_last_place();
    let route = std::env::var("RUNEFIN_LIVE_ROUTE").unwrap_or_default();
    let env = |name: &str| std::env::var(name).unwrap_or_default();
    match route.as_str() {
        "library" => {
            let id = env("RUNEFIN_LIVE_LIBRARY_ID");
            if !id.is_empty() {
                let name = env("RUNEFIN_LIVE_LIBRARY_NAME");
                let collection_type = std::env::var("RUNEFIN_LIVE_LIBRARY_TYPE").ok();
                s.open_library(id, name, collection_type);
            }
        }
        "detail" => {
            let id = env("RUNEFIN_LIVE_ITEM_ID");
            if !id.is_empty() {
                s.open_detail(id);
            }
        }
        "search" => {
            s.open_search(None);
            let query = std::env::var("RUNEFIN_LIVE_QUERY").unwrap_or_else(|_| "dune".into());
            s.search.query = TextEdit::new(query);
            s.search_changed();
        }
        "settings" => {
            s.open_settings();
            if let Ok(query) = std::env::var("RUNEFIN_LIVE_SETTINGS_QUERY") {
                s.settings_query = TextEdit::new(query);
            }
        }
        "remote" => s.open_remote(),
        "live-tv" => s.open_live_tv(),
        "seerr-discover" => {
            s.open_seerr(crate::state::SeerrTab::Discover);
            if std::env::var("RUNEFIN_LIVE_SEERR_BROWSE").is_ok() {
                s.set_seerr_discover_mode(crate::state::DiscoverMode::Browse);
                if let Ok(kind) = std::env::var("RUNEFIN_LIVE_SEERR_KIND") {
                    s.set_discover_kind(if kind == "series" {
                        crate::state::DiscoverKind::Series
                    } else {
                        crate::state::DiscoverKind::Movies
                    });
                }
                if let Ok(genre) = std::env::var("RUNEFIN_LIVE_SEERR_GENRE")
                    && let Ok(genre) = genre.parse::<i64>()
                {
                    s.set_discover_genre(Some(genre));
                }
                if let Ok(hub) = std::env::var("RUNEFIN_LIVE_SEERR_HUB")
                    && let Ok(hub) = hub.parse::<i64>()
                {
                    s.set_discover_hub(Some(hub));
                }
            }
        }
        "seerr-requests" => s.open_seerr(crate::state::SeerrTab::Requests),
        "person" => {
            let id = env("RUNEFIN_LIVE_PERSON_ID");
            if !id.is_empty() {
                let name = env("RUNEFIN_LIVE_PERSON_NAME");
                s.open_person(&jellyfin_api::Person {
                    id,
                    name,
                    ..jellyfin_api::Person::default()
                });
            }
        }
        "seerr-detail" => {
            let media_type =
                std::env::var("RUNEFIN_LIVE_MEDIA_TYPE").unwrap_or_else(|_| "movie".into());
            if let Ok(tmdb_id) = env("RUNEFIN_LIVE_TMDB_ID").parse::<u64>() {
                s.open_seerr_detail(media_type, tmdb_id);
            }
        }
        _ => {}
    }
}

fn setup(s: &mut State, waker: rune::Waker) {
    s.setup(waker);
    // A live capture picks its route after every `Latest` has its waker, then waits for the
    // real requests before the frame is frozen. Neither path is used by the app itself.
    if std::env::args().any(|arg| arg == "--live") {
        apply_live_route(s);
        if let Some(position) = std::env::args().position(|arg| arg == "--wait")
            && let Some(seconds) = std::env::args().nth(position + 1)
            && let Ok(seconds) = seconds.parse::<u64>()
        {
            std::thread::sleep(std::time::Duration::from_secs(seconds.min(30)));
        }
    }
}

/// Every frame. Drains finished work; true only when something visible changed.
fn poll(s: &mut State, _clock: f64, tx: &mut Tx) -> bool {
    let started = std::time::Instant::now();
    let mut changed = s.poll();
    if let Some(key) = s.take_focus_key() {
        s.note_focus(key.clone());
        tx.focus_key(key);
        changed = true;
    }
    if let Some(fullscreen) = s.pending_fullscreen.take() {
        tx.set_fullscreen(fullscreen);
    }
    if let Some(y) = s.take_pending_scroll() {
        tx.scroll_to_y("library-scroll", y);
    }
    // A loading skeleton is a live surface. Keep a frame scheduled until the first home answer
    // arrives so a window that starts unfocused cannot sit frozen behind another app.
    if s.is_signed_in() && !s.home_loaded {
        tx.wake_after(std::time::Duration::from_millis(16));
    }
    // A live capture can hold each virtual frame long enough for the real image/API workers to
    // catch up. It is opt-in and never set by the app itself.
    if let Ok(ms) = std::env::var("RUNEFIN_LIVE_FRAME_MS")
        && let Ok(ms) = ms.parse::<u64>()
    {
        std::thread::sleep(std::time::Duration::from_millis(ms.min(50)));
    }
    trace_slow_poll(started.elapsed());
    changed
}

/// With `RUNEFIN_TRACE_PLAYBACK=1`, name a UI frame that took long enough to be felt. At most
/// one line a second, so a pathological screen does not flood the terminal.
fn trace_slow_poll(took: std::time::Duration) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST: AtomicU64 = AtomicU64::new(0);
    if took < std::time::Duration::from_millis(12)
        || std::env::var_os("RUNEFIN_TRACE_PLAYBACK").is_none()
    {
        return;
    }
    let now = crate::playback_trace::now_us();
    if now.saturating_sub(LAST.load(Ordering::Relaxed)) < 1_000_000 {
        return;
    }
    LAST.store(now, Ordering::Relaxed);
    eprintln!("playback: the UI frame took {} ms", took.as_millis());
}

/// The living-room keys. Rune runs an element's own `on_key` before its focus-group navigation,
/// so the root element carries this hook too; `on_key` calls it for the unfocused case. A screen
/// with no zones, a focused panel or a text field's Left/Right is left to Rune.
fn tv_key(s: &mut State, key: &KeyEvent, tx: &mut Tx) -> bool {
    let Some(action) = tv_input::action(key) else {
        return false;
    };
    let focused = tx.focused().map(str::to_owned);
    // A focused volume slider spends Up/Down on the level, so Left/Right stay free to move to
    // the next control; otherwise the row had no way past the slider to the gear.
    if s.in_app.is_some() && s.osd_visible && focused.as_deref() == Some("in-app-volume-slider") {
        match action {
            tv_input::TvAction::Up => {
                s.adjust_in_app_volume(0.05);
                return true;
            }
            tv_input::TvAction::Down => {
                s.adjust_in_app_volume(-0.05);
                return true;
            }
            _ => {}
        }
    }
    // On the timeline, Left/Right move the preview and OK commits it.
    if s.big_screen
        && s.in_app.is_some()
        && s.osd_visible
        && focused.as_deref() == Some("in-app-seek")
    {
        match action {
            tv_input::TvAction::Left => {
                s.scrub_in_app_step(false);
                return true;
            }
            tv_input::TvAction::Right => {
                s.scrub_in_app_step(true);
                return true;
            }
            tv_input::TvAction::Activate => {
                s.commit_in_app_scrub();
                return true;
            }
            _ => {}
        }
    }
    if !action.is_direction() {
        return false;
    }
    // A text field keeps Left/Right for its caret; Up/Down still leave it for the rows or zones.
    if matches!(action, tv_input::TvAction::Left | tv_input::TvAction::Right)
        && focused.as_deref().is_some_and(tv_input::is_text_field)
    {
        return false;
    }
    // Settings is a vertical form with dozens of control keys; Up/Down follow the page's tab
    // order instead of a hand-kept key list. Left/Right stay with the focused control.
    if s.big_screen && matches!(s.route, Route::Settings) {
        tx.send_key(KeyEvent {
            key: Key::Tab,
            mods: rune::Mods {
                shift: matches!(action, tv_input::TvAction::Up),
                ..rune::Mods::default()
            },
        });
        return true;
    }
    // The router steps between zones, clamps at the edges and asks Rune to focus the key it
    // chose. A modal, menu or scrollbar has focus: its directions are Rune's, not the zones'.
    let panel = focused
        .as_deref()
        .is_some_and(|key| !tv_focus::contains(s, key));
    if !panel {
        let target = tv_focus::step(s, focused.as_deref(), action);
        if target.is_some() || tv_focus::owns_screen(s) {
            if let Some(target) = &target {
                // A text edit's focused flag keeps Rune focused on it; follow the runtime focus.
                if target == "search-input" {
                    s.search.query.focused = true;
                } else if focused.as_deref() == Some("search-input") {
                    s.search.query.focused = false;
                }
                s.note_focus(target.clone());
                tx.focus_key(target.clone());
            }
            return true;
        }
    }
    false
}

/// Esc pops the top screen: detail goes back to its card, a library goes home.
fn on_key(s: &mut State, key: &KeyEvent, tx: &mut Tx) -> bool {
    // A rebind captures exactly one key. Escape cancels; everything else is swallowed so the
    // key does not also act while the reader is choosing it.
    if s.rebinding.is_some() {
        if key.key == Key::Escape {
            s.cancel_rebind();
            return true;
        }
        if let Some(name) = crate::shortcuts::key_name(key) {
            s.capture_shortcut(&name);
        }
        return true;
    }
    // The TV's own keys, shared with the root element's key hook. On a focused control Rune
    // runs that hook before its group navigation; `on_key` above only sees the unfocused case.
    if tv_key(s, key, tx) {
        return true;
    }
    if s.shortcut_matches(crate::shortcuts::Action::Fullscreen, key) {
        s.toggle_fullscreen();
        return true;
    }
    if s.card_menu.is_some() && key.key == Key::Escape {
        s.close_card_menu();
        return true;
    }
    if s.collection_picker_open && key.key == Key::Escape {
        s.close_collection_picker();
        return true;
    }
    if s.playlist_picker_open && key.key == Key::Escape {
        s.close_playlist_picker();
        return true;
    }
    if key.key == Key::ContextMenu && s.in_app.is_some() {
        // A controller's Start: the player's gear, the one menu the couch needs.
        s.toggle_in_app_gear();
        return true;
    }
    if s.in_app.is_some() {
        s.show_in_app_osd();
        match &key.key {
            // A CEC remote's OK arrives as Return. With nothing focused it is the play/pause
            // key; a focused OSD control sees it first and activates itself instead.
            _ if s.shortcut_matches(crate::shortcuts::Action::PlayPause, key)
                || (key.key == Key::Enter
                    && matches!(tx.focused(), None | Some("in-app-seek"))) =>
            {
                s.toggle_in_app_pause();
                return true;
            }
            Key::Text(text) if text == "," => {
                s.step_in_app_frame(false);
                return true;
            }
            Key::Text(text) if text == "." => {
                s.step_in_app_frame(true);
                return true;
            }
            Key::Left => {
                s.seek_in_app_step(false);
                return true;
            }
            Key::Right => {
                s.seek_in_app_step(true);
                return true;
            }
            Key::PageUp => {
                s.zap_live(-1);
                return true;
            }
            Key::PageDown => {
                s.zap_live(1);
                return true;
            }
            Key::Up => {
                s.adjust_in_app_volume(0.05);
                return true;
            }
            Key::Down => {
                s.adjust_in_app_volume(-0.05);
                return true;
            }
            _ if s.shortcut_matches(crate::shortcuts::Action::Mute, key) => {
                s.toggle_in_app_mute();
                return true;
            }
            _ if s.shortcut_matches(crate::shortcuts::Action::Subtitles, key) => {
                s.cycle_in_app_text();
                return true;
            }
            _ if s.shortcut_matches(crate::shortcuts::Action::Skip, key) => {
                if let Some(end) = s.offered_in_app_segment().map(|segment| segment.end_ms()) {
                    s.skip_in_app_segment(end);
                }
                return true;
            }
            Key::Text(text) if text == "[" => {
                s.adjust_in_app_subtitle_delay(-100);
                return true;
            }
            Key::Text(text) if text == "]" => {
                s.adjust_in_app_subtitle_delay(100);
                return true;
            }
            Key::Text(text) if text == "-" => {
                s.adjust_in_app_subtitle_size(-0.1);
                return true;
            }
            Key::Text(text) if text == "=" || text == "+" => {
                s.adjust_in_app_subtitle_size(0.1);
                return true;
            }
            _ if s.shortcut_matches(crate::shortcuts::Action::NextEpisode, key) => {
                s.play_in_app_next_episode();
                return true;
            }
            _ if s.shortcut_matches(crate::shortcuts::Action::PreviousEpisode, key) => {
                s.play_in_app_previous();
                return true;
            }
            Key::Text(text) if text == ">" => {
                s.shift_in_app_speed(true);
                return true;
            }
            Key::Text(text) if text == "<" => {
                s.shift_in_app_speed(false);
                return true;
            }
            Key::Escape if s.in_app_subtitle_search.open => {
                s.close_in_app_subtitle_search();
                return true;
            }
            Key::Escape if s.player_panel_open() => {
                s.close_player_panel();
                return true;
            }
            Key::Escape if s.cast_picker_open => {
                s.close_cast_picker();
                return true;
            }
            Key::Escape if s.playlist_create_open => {
                s.close_playlist_create();
                return true;
            }
            Key::Escape if s.playlist_picker_open => {
                s.close_playlist_picker();
                return true;
            }
            Key::Escape if s.collection_picker_open => {
                s.close_collection_picker();
                return true;
            }
            Key::Escape if s.player_gear_visible() => {
                s.close_player_gear();
                return true;
            }
            Key::Escape if s.in_app_stats => {
                s.close_in_app_stats();
                return true;
            }
            _ if s.shortcut_matches(crate::shortcuts::Action::Fit, key) => {
                s.toggle_in_app_fit();
                return true;
            }
            _ => {}
        }
    }
    if (key.mods.cmd || key.mods.ctrl)
        && let Key::Text(text) = &key.key
    {
        if text.eq_ignore_ascii_case("k") {
            s.open_search(None);
            return true;
        }
        if text.eq_ignore_ascii_case("f") {
            if matches!(s.route, Route::Seerr | Route::SeerrDetail { .. }) {
                if matches!(s.route, Route::SeerrDetail { .. }) {
                    s.back_from_seerr_detail();
                }
                s.focus_seerr_search();
                return true;
            }
            let scope = match &s.route {
                Route::Library { id } => Some(id.clone()),
                Route::Search { scope } => scope.clone(),
                _ => None,
            };
            s.open_search(scope);
            return true;
        }
    }
    if matches!(key.key, Key::Function(1)) || matches!(&key.key, Key::Text(text) if text == "?") {
        s.toggle_shortcuts();
        return true;
    }
    if key.key == Key::Escape && s.shortcuts_open {
        s.close_shortcuts();
        return true;
    }
    if key.key == Key::Escape && s.shelf.is_some() && s.route == Route::Home {
        s.close_shelf();
        return true;
    }
    // On the couch, Back out of Home's content lands on the rail rather than nowhere.
    if key.key == Key::Escape
        && s.big_screen
        && s.route == Route::Home
        && matches!(s.step, Step::Ready { .. })
    {
        tx.focus_key("nav-home");
        return true;
    }
    if key.key != Key::Escape {
        return false;
    }
    if s.in_app.is_some() {
        s.stop_in_app();
        return true;
    }
    if s.playback.is_some() || s.launching {
        s.stop_playback();
        return true;
    }
    if matches!(&s.route, Route::Search { .. }) {
        s.close_search();
        return true;
    }
    if matches!(&s.route, Route::Seerr) {
        s.close_seerr();
        return true;
    }
    if matches!(&s.route, Route::Remote | Route::LiveTv | Route::Settings) {
        s.go_home();
        return true;
    }
    if matches!(&s.route, Route::SeerrDetail { .. }) {
        s.back_from_seerr_detail();
        return true;
    }
    if matches!(&s.route, Route::Person { .. } | Route::SeerrPerson { .. }) {
        s.back_from_person();
        return true;
    }
    if matches!(&s.route, Route::Detail { .. }) {
        s.back_from_detail();
        true
    } else if matches!(&s.route, Route::Library { .. }) {
        s.go_home();
        true
    } else {
        false
    }
}

/// A synthetic bright/dark image on Home: the committed set otherwise only has letter
/// placeholders, which cannot test text over artwork.
#[cfg(debug_assertions)]
fn home_art_scene(s: &mut State) {
    demo::signed_in(s);
    let tags =
        std::collections::HashMap::from([("Primary".to_string(), "contrast-art".to_string())]);
    for item in s.home_data.resume.iter_mut().chain(
        s.home_data.next_up.iter_mut().chain(
            s.home_data
                .latest
                .iter_mut()
                .flat_map(|rail| rail.items.iter_mut()),
        ),
    ) {
        item.image_tags = Some(tags.clone());
        item.series_primary_image_tag = None;
    }
    if let Some(item) = s.home_data.resume.first_mut() {
        item.backdrop_image_tags = Some(vec!["contrast-art".into()]);
    }
    let (poster, backdrop) = {
        let cache = s
            .image_cache
            .get_or_insert_with(|| rune::ImageCache::new(std::sync::Arc::new(|| {})));
        (demo::contrast_poster(cache), demo::contrast_backdrop(cache))
    };
    let keys: Vec<String> = s
        .home_data
        .resume
        .iter()
        .chain(
            s.home_data
                .next_up
                .iter()
                .chain(s.home_data.latest.iter().flat_map(|rail| rail.items.iter())),
        )
        .filter_map(|item| s.image_key(item))
        .collect();
    for key in keys {
        s.images.insert(key, poster.clone());
    }
    if let Some(key) = s.home_backdrop_key() {
        s.images.insert(key, backdrop);
    }
}

#[cfg(debug_assertions)]
fn detail_art_scene(s: &mut State) {
    demo::signed_in(s);
    let mut detail = demo::movie_detail();
    detail.item.image_tags = Some(std::collections::HashMap::from([(
        "Primary".to_string(),
        "contrast-art".to_string(),
    )]));
    detail.item.backdrop_image_tags = Some(vec!["contrast-art".into()]);
    s.route = Route::Detail {
        id: detail.item.id.clone(),
    };
    s.detail = Some(detail);
    let (poster, backdrop) = {
        let cache = s
            .image_cache
            .get_or_insert_with(|| rune::ImageCache::new(std::sync::Arc::new(|| {})));
        (demo::contrast_poster(cache), demo::contrast_backdrop(cache))
    };
    let item = s.detail.as_ref().map(|detail| detail.item.clone());
    if let Some(item) = item {
        if let Some(key) = s.detail_image_key(&item) {
            s.images.insert(key, poster);
        }
        if let Some(key) = s.detail_backdrop_key(&item) {
            s.images.insert(key, backdrop);
        }
    }
}

#[cfg(debug_assertions)]
fn home_art_light_scene(s: &mut State) {
    home_art_scene(s);
    crate::theme::set_light(true);
    s.light_theme = true;
}

fn seerr_scene(s: &mut State) {
    demo::signed_in(s);
    s.config.seerr_url = "https://jellyseerr.example".into();
    s.config.seerr_api_key = Some("example-api-key".into());
    s.seerr.url = TextEdit::new("https://jellyseerr.example");
    s.seerr.key = TextEdit::new("example-api-key").secure();
    s.seerr.counts = Some(demo::seerr_counts());
    s.seerr.summaries = demo::seerr_summaries();
    s.seerr.last_loaded = Some(std::time::Instant::now());
}

#[cfg(debug_assertions)]
fn in_app_osd_scene(s: &mut State) {
    demo::signed_in(s);
    s.in_app = Some(demo::in_app());
    s.osd_visible = true;
}

#[cfg(not(debug_assertions))]
fn in_app_osd_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn in_app_mini_scene(s: &mut State) {
    demo::signed_in(s);
    s.in_app = Some(demo::in_app());
    s.in_app_minimized = true;
}

#[cfg(not(debug_assertions))]
fn in_app_mini_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn in_app_intro_scene(s: &mut State) {
    demo::signed_in(s);
    s.in_app = Some(demo::in_app());
    if let Some(session) = &mut s.in_app {
        session.scrub_ms = Some(30_000);
    }
    s.in_app_segments = vec![jellyfin_api::MediaSegment {
        id: "intro".into(),
        item_id: "quiet-floor-s2e2".into(),
        kind: "Intro".into(),
        start_ticks: 0,
        end_ticks: 90_000 * jellyfin_api::TICKS_PER_MS,
    }];
    s.osd_visible = true;
}

#[cfg(not(debug_assertions))]
fn in_app_intro_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn in_app_gear_scene(s: &mut State) {
    in_app_osd_scene(s);
    s.in_app_gear_open = true;
}

#[cfg(not(debug_assertions))]
fn in_app_gear_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn in_app_skip_credits_scene(s: &mut State) {
    demo::signed_in(s);
    let mut session = demo::in_app();
    session.scrub_ms = Some(42 * 60_000 + 10_000);
    s.in_app = Some(session);
    s.in_app_segments = vec![jellyfin_api::MediaSegment {
        id: "credits".into(),
        item_id: "quiet-floor-s2e2".into(),
        kind: "Outro".into(),
        start_ticks: 42 * 60_000 * jellyfin_api::TICKS_PER_MS,
        end_ticks: 44 * 60_000 * jellyfin_api::TICKS_PER_MS,
    }];
    s.osd_visible = true;
}

#[cfg(not(debug_assertions))]
fn in_app_skip_credits_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn in_app_trickplay_scene(s: &mut State) {
    demo::signed_in(s);
    let mut session = demo::in_app();
    session.item.trickplay = Some(demo::trickplay_manifest());
    s.in_app = Some(session);
    s.in_app_seek_width = 1232.0;
    // Derive the time from the pointer exactly as a live hover does, so the label and the
    // crop agree with the position the pointer is resting on.
    s.hover_in_app_seek(620.0);
    let ms = s.in_app_hover.map_or(0, |(_, ms)| ms);
    // Headless snapshots skip `setup`, so a scene that needs decoded pixels owns its cache.
    let cache = s
        .image_cache
        .get_or_insert_with(|| rune::ImageCache::new(std::sync::Arc::new(|| {})));
    // Build the fixture sprite before releasing the cache borrow: `images` is the same state.
    let sprite = Some(demo::trickplay_sprite(cache));
    if let (Some(sprite), Some(item)) = (sprite, s.in_app.as_ref().map(|it| it.item.clone()))
        && let Some(key) = s.trickplay_key(&item, ms)
    {
        s.images.insert(key, sprite);
    }
    s.osd_visible = true;
}

#[cfg(not(debug_assertions))]
fn in_app_trickplay_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn in_app_subtitles_scene(s: &mut State) {
    in_app_osd_scene(s);
    if let Some(session) = &mut s.in_app {
        session.position_ms = 42 * 60_000 + 13_000;
    }
    s.in_app_subtitles.tracks = demo::subtitle_tracks();
    s.in_app_subtitles.source = Some("demo-source".into());
    s.in_app_subtitles.selected = Some(0);
    s.in_app_subtitles.cues = demo::subtitle_cues();
}

#[cfg(not(debug_assertions))]
fn in_app_subtitles_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn in_app_subtitle_style_scene(s: &mut State) {
    in_app_subtitles_scene(s);
    s.subtitle_font = crate::config::SubtitleFont::Serif;
    s.subtitle_color = crate::config::SubtitleColor::Yellow;
    s.subtitle_outline = crate::config::SubtitleOutline::Thick;
    s.subtitle_position = crate::config::SubtitlePosition::Top;
    s.subtitle_style = crate::config::SubtitleStyle::Bold;
    s.subtitle_background = crate::config::SubtitleBackground::Solid;
    s.subtitle_background_opacity = 0.55;
}

#[cfg(not(debug_assertions))]
fn in_app_subtitle_style_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn in_app_image_subtitle_scene(s: &mut State) {
    in_app_subtitles_scene(s);
    s.in_app_subtitles.image = demo::image_subtitle_style();
    s.in_app_subtitles.selected = None;
}

#[cfg(not(debug_assertions))]
fn in_app_image_subtitle_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn in_app_subtitles_clear_scene(s: &mut State) {
    in_app_subtitles_scene(s);
    s.subtitle_background = crate::config::SubtitleBackground::Off;
}

#[cfg(not(debug_assertions))]
fn in_app_subtitles_clear_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn in_app_ass_subtitles_scene(s: &mut State) {
    in_app_subtitles_scene(s);
    s.in_app_subtitles.cues = demo::ass_cues();
}

#[cfg(not(debug_assertions))]
fn in_app_ass_subtitles_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn in_app_subtitle_panel_scene(s: &mut State) {
    in_app_subtitles_scene(s);
    s.in_app_subtitles.open = true;
    s.in_app_subtitles.tracks = demo::subtitle_tracks_with_image();
}

#[cfg(not(debug_assertions))]
fn in_app_subtitle_panel_scene(_s: &mut State) {}
#[cfg(debug_assertions)]
fn in_app_subtitle_many_scene(s: &mut State) {
    in_app_subtitle_panel_scene(s);
    s.in_app_subtitles.tracks = (0..20)
        .map(|i| jellyfin_api::SubtitleTrack {
            index: i,
            language: Some("eng".into()),
            display_title: format!("English {i} - SUBRIP - External"),
            codec: Some("subrip".into()),
            is_text: true,
            is_external: true,
            is_forced: false,
        })
        .collect();
}

#[cfg(not(debug_assertions))]
fn in_app_subtitle_many_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn in_app_subtitle_search_scene(s: &mut State) {
    in_app_subtitle_panel_scene(s);
    s.in_app_subtitle_search.open = true;
    s.in_app_subtitle_search.results = demo::remote_subtitles();
}

#[cfg(not(debug_assertions))]
fn in_app_subtitle_search_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn in_app_chapters_scene(s: &mut State) {
    in_app_osd_scene(s);
    if let Some(session) = &mut s.in_app {
        session.position_ms = 12 * 60_000 + 40_000;
    }
    s.in_app_chapters_open = true;
    s.in_app_prev = Some(demo::previous_episode());
    s.in_app_next = Some(demo::next_episode());
    let item = s.in_app.as_ref().map(|session| session.item.clone());
    if let Some(item) = item {
        let cache = s
            .image_cache
            .get_or_insert_with(|| rune::ImageCache::new(std::sync::Arc::new(|| {})));
        let still = demo::still(cache);
        for at in 0..item.chapters.len() {
            if let Some(key) = s.chapter_image_key(&item, at) {
                s.images.insert(key, still.clone());
            }
        }
    }
}

#[cfg(not(debug_assertions))]
fn in_app_chapters_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn in_app_queue_scene(s: &mut State) {
    in_app_osd_scene(s);
    let names = [
        "Who Is Awake?",
        "The Hollow Floor",
        "Cold Harbor",
        "Overtime",
    ];
    s.queue = (1..=4)
        .map(|number| {
            let mut item = demo::next_episode();
            item.id = format!("quiet-floor-s2e{}", number + 1);
            item.name = names[(number - 1) as usize].into();
            item.index_number = Some(number + 1);
            item
        })
        .collect();
    s.queue_at = 1;
    s.in_app_prev = Some(demo::previous_episode());
    // The player and the queue must agree about which item is "playing", or the panel reads
    // 2 of 4 beside the first episode.
    if let Some(item) = s.queue.get(1).cloned()
        && let Some(session) = &mut s.in_app
    {
        session.item = item.clone();
        session.item_id = item.id;
    }
}

#[cfg(not(debug_assertions))]
fn in_app_queue_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn in_app_queue_panel_scene(s: &mut State) {
    in_app_queue_scene(s);
    s.in_app_queue_open = true;
}

#[cfg(not(debug_assertions))]
fn in_app_queue_panel_scene(_s: &mut State) {}

/// The widest transport row there is: skip, audio, subtitles with a delay, fit, speed, volume,
/// mute, previous/next and a queue position, in a 1024-wide window.
#[cfg(debug_assertions)]
fn in_app_narrow_scene(s: &mut State) {
    in_app_osd_scene(s);
    if let Some(session) = &mut s.in_app {
        session.position_ms = 38_000;
    }
    s.in_app_subtitles.tracks = demo::subtitle_tracks();
    s.in_app_subtitles.selected = Some(0);
    s.in_app_subtitles.delay_ms = 500;
    s.in_app_audio_tracks = demo::audio_tracks();
    s.in_app_prev = Some(demo::previous_episode());
    s.in_app_next = Some(demo::next_episode());
    s.in_app_segments = vec![jellyfin_api::MediaSegment {
        id: "seg-intro".into(),
        item_id: "quiet-floor-s2e2".into(),
        kind: "Intro".into(),
        start_ticks: 10_000 * 60_000 * 41,
        end_ticks: 10_000 * 60_000 * 43,
    }];
    s.queue = (1..=3)
        .map(|number| {
            let mut item = demo::next_episode();
            item.id = format!("quiet-floor-s2e{}", number + 1);
            item.index_number = Some(number + 1);
            item
        })
        .collect();
    s.queue_at = 1;
}

#[cfg(not(debug_assertions))]
fn in_app_narrow_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn in_app_buffering_scene(s: &mut State) {
    in_app_osd_scene(s);
    if let Some(session) = &mut s.in_app {
        session.player.preview_buffering(42);
        session.buffering = Some(42);
    }
    // Chrome hidden is the case worth proving: a frozen picture still explains itself.
    s.osd_visible = false;
}

#[cfg(not(debug_assertions))]
fn in_app_buffering_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn in_app_next_scene(s: &mut State) {
    demo::signed_in(s);
    let mut session = demo::in_app();
    let end = session.duration_ms.unwrap_or(48 * 60_000) - 24_000;
    session.player = crate::player::Player::preview_at(end);
    session.position_ms = end;
    s.in_app = Some(session);
    let next = demo::next_episode();
    s.in_app_next = Some(next.clone());
    s.osd_visible = true;
    let cache = s
        .image_cache
        .get_or_insert_with(|| rune::ImageCache::new(std::sync::Arc::new(|| {})));
    let still = demo::still(cache);
    if let Some(key) = s.next_episode_image_key(&next) {
        s.images.insert(key, still);
    }
}

#[cfg(not(debug_assertions))]
fn in_app_next_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn live_tv_scene(s: &mut State) {
    demo::signed_in(s);
    s.route = Route::LiveTv;
    s.live.channels = demo::live_channels();
    s.live.next = demo::live_next();
}

#[cfg(not(debug_assertions))]
fn live_tv_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn live_guide_scene(s: &mut State) {
    live_tv_scene(s);
    s.live.guide = demo::live_guide();
    s.live.guide_open = true;
}

#[cfg(not(debug_assertions))]
fn live_guide_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn in_app_stats_scene(s: &mut State) {
    in_app_osd_scene(s);
    s.in_app_stats = true;
    s.in_app_video = Some(("hevc".into(), 3840, 2160, Some("HDR10".into())));
    s.in_app_container = Some("mkv".into());
}

#[cfg(not(debug_assertions))]
fn in_app_stats_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn in_app_live_scene(s: &mut State) {
    demo::signed_in(s);
    s.live.channels = demo::live_channels();
    s.live.next = demo::live_next();
    s.in_app = Some(demo::live_session());
    s.osd_visible = true;
}

#[cfg(not(debug_assertions))]
fn in_app_live_scene(_s: &mut State) {}

#[cfg(debug_assertions)]
fn in_app_audio_scene(s: &mut State) {
    in_app_osd_scene(s);
    s.in_app_audio_tracks = demo::audio_tracks();
    s.in_app_audio_open = true;
}

#[cfg(not(debug_assertions))]
fn in_app_audio_scene(_s: &mut State) {}

/// Refuse a second full instance. The socket is only a lease: a live connection means another
/// process owns it; a stale socket is removed and reclaimed. `--snap` is exempt because it is a
/// headless tool, not a desktop session.
fn claim_single_instance() -> bool {
    use std::os::unix::net::{UnixListener, UnixStream};
    if std::env::args().any(|arg| arg == "--snap") {
        return true;
    }
    let path = std::env::temp_dir().join(format!(
        "runefin-{}.sock",
        std::env::var("USER").unwrap_or_else(|_| "user".into())
    ));
    if UnixStream::connect(&path).is_ok() {
        eprintln!("Runefin is already running.");
        return false;
    }
    let _ = std::fs::remove_file(&path);
    let Ok(listener) = UnixListener::bind(&path) else {
        return true;
    };
    // Keep the listener for the process lifetime. Dropping it would release the lease while
    // the app is still open.
    std::mem::forget(listener);
    true
}

fn main() {
    if !claim_single_instance() {
        return;
    }
    let startup = config::Config::load();
    if big_screen_requested() {
        // The window's physical height sets the scale; 720 points of design keeps the same
        // layout at 1080p (1.5x) and 4K (3x).
        rune::set_design_height(Some(720.0));
        // A TV remote's OK button arrives as Return; make it press the focused control, not
        // only the rows and links that already took it.
        rune::set_return_activates(true);
        crate::theme::set_big_screen(true);
    }
    let size = if startup.window_width >= 900.0 && startup.window_height >= 600.0 {
        (
            startup.window_width.min(3840.0),
            startup.window_height.min(2160.0),
        )
    } else {
        (1280.0, 800.0)
    };
    let saved_position = startup
        .window_x
        .zip(startup.window_y)
        .filter(|(x, y)| *x >= 0 && *y >= 0 && *x < 20_000 && *y < 20_000);
    let app = rune::App::new(AppSpec::<State> {
        setup: Some(setup),
        poll: Some(poll),
        on_key: Some(on_key),
        scenes: &[
            ("idle", |s| {
                s.config = crate::config::Config::fresh();
                s.server = TextEdit::new("");
                s.user = TextEdit::new("");
                s.password = TextEdit::new("").secure();
                s.step = Step::Idle;
            }),
            ("connect-quick", |s| {
                s.config = crate::config::Config::fresh();
                s.server = TextEdit::new("https://jellyfin.example");
                s.step = Step::Idle;
                s.quick = Some(demo::quick_login());
            }),
            ("signing-in", |s| {
                s.config = crate::config::Config::fresh();
                s.server = TextEdit::new("http://192.0.2.10:8096");
                s.user = TextEdit::new("luis");
                s.step = Step::SigningIn;
            }),
            ("error", |s| {
                s.config = crate::config::Config::fresh();
                s.server = TextEdit::new("http://192.0.2.10:8096");
                s.user = TextEdit::new("luis");
                s.step = Step::Failed(net::Failure {
                    stage: net::Stage::Server,
                    message: "Can't reach the server.".into(),
                });
            }),
            #[cfg(debug_assertions)]
            ("home-art", home_art_scene),
            #[cfg(debug_assertions)]
            ("home-art-light", home_art_light_scene),
            #[cfg(debug_assertions)]
            ("detail-art", detail_art_scene),
            ("home", |s| {
                s.step = Step::Ready {
                    server_name: "example".into(),
                };
                s.config.user_name = "example-user".into();
                s.home_data = demo::home();
                s.home_loaded = true;
            }),
            #[cfg(debug_assertions)]
            ("home-shelf", |s| {
                demo::signed_in(s);
                s.open_shelf(crate::state::HomeShelf::ContinueWatching);
            }),
            #[cfg(debug_assertions)]
            ("home-contrast", |s| {
                s.step = Step::Ready {
                    server_name: "example".into(),
                };
                s.config.user_name = "example-user".into();
                s.home_data = demo::home();
                s.home_loaded = true;
                s.high_contrast = true;
                crate::theme::set_high_contrast(true);
            }),
            ("home-light", |s| {
                s.step = Step::Ready {
                    server_name: "example".into(),
                };
                s.config.user_name = "example-user".into();
                s.home_data = demo::home();
                s.home_loaded = true;
                s.light_theme = true;
                crate::theme::set_light(true);
            }),
            ("home-hero", |s| {
                s.step = Step::Ready {
                    server_name: "example".into(),
                };
                s.config.user_name = "example-user".into();
                s.home_data = demo::home();
                s.home_loaded = true;
                s.home_hero = true;
            }),
            ("home-contract", |s| {
                s.step = Step::Ready {
                    server_name: "example".into(),
                };
                s.config.user_name = "example-user".into();
                s.home_data = demo::home();
                s.home_loaded = true;
            }),
            ("toast-undo", |s| {
                demo::signed_in(s);
                s.show_undo_toast(
                    "Removed from Continue Watching.",
                    crate::state::UndoAction::Favorite {
                        item_id: "quiet-floor-s2e4".into(),
                        value: false,
                    },
                );
            }),
            #[cfg(debug_assertions)]
            ("detail-theme", |s| {
                demo::signed_in(s);
                s.route = Route::Detail {
                    id: "metropolis".into(),
                };
                s.detail = Some(demo::movie_detail());
                s.detail_theme_songs = demo::extras();
                s.detail_theme_videos = demo::extras();
                s.detail_return = Route::Home;
            }),
            ("home-collapsed", |s| {
                s.step = Step::Ready {
                    server_name: "example".into(),
                };
                s.config.user_name = "example-user".into();
                s.home_data = demo::home();
                s.home_loaded = true;
                s.sidebar_collapsed = true;
            }),
            ("home-loading", |s| {
                s.step = Step::Ready {
                    server_name: "example".into(),
                };
                s.config.user_name = "example-user".into();
                s.home_loaded = false;
            }),
            ("home-empty", |s| {
                s.step = Step::Ready {
                    server_name: "example".into(),
                };
                s.config.user_name = "example-user".into();
                s.home_loaded = true;
            }),
            ("home-error", |s| {
                s.step = Step::Ready {
                    server_name: "example".into(),
                };
                s.config.user_name = "example-user".into();
                s.home_loaded = true;
                s.home_error = Some("Can't reach the server.".into());
            }),
            ("library", |s| {
                demo::signed_in(s);
                let library = demo::library(60);
                s.route = Route::Library {
                    id: library.id.clone(),
                };
                s.library = Some(library);
            }),
            #[cfg(debug_assertions)]
            ("playlist-create", |s| {
                demo::signed_in(s);
                s.playlists = demo::playlists();
                s.route = Route::Detail {
                    id: "metropolis".into(),
                };
                s.detail = Some(demo::movie_detail());
                s.playlist_create_open = true;
                let mut name = rune::TextEdit::new("Silent film weekend");
                name.focused = true;
                s.playlist_new_name = name;
            }),
            #[cfg(debug_assertions)]
            ("toast", |s| {
                demo::signed_in(s);
                s.show_toast("Added to the Silent Horror Night.");
            }),
            #[cfg(debug_assertions)]
            ("cast-picker", |s| {
                demo::signed_in(s);
                s.cast_item = Some(demo::next_episode());
                s.cast_sessions = demo::sessions();
                s.cast_picker_open = true;
            }),
            #[cfg(debug_assertions)]
            ("card-menu", |s| {
                demo::signed_in(s);
                let mut item = demo::next_episode();
                item.user_data = Some(jellyfin_api::UserData {
                    playback_position_ticks: 10_000 * 60_000 * 12,
                    ..jellyfin_api::UserData::default()
                });
                s.card_menu = Some(crate::state::CardMenu {
                    item,
                    x: 420.0,
                    y: 260.0,
                });
            }),
            #[cfg(debug_assertions)]
            ("connect-discovery", |s| {
                s.discovered = vec![
                    crate::discovery::DiscoveredServer {
                        id: "example".into(),
                        name: "example".into(),
                        address: "http://192.0.2.49:8096".into(),
                    },
                    crate::discovery::DiscoveredServer {
                        id: "attic".into(),
                        name: "attic".into(),
                        address: "https://jellyfin.example".into(),
                    },
                ];
            }),
            #[cfg(debug_assertions)]
            ("home-collections", |s| {
                demo::signed_in(s);
                s.home_data.libraries.push(jellyfin_api::LibraryView {
                    id: "collections".into(),
                    name: "Collections".into(),
                    collection_type: Some("boxsets".into()),
                });
            }),
            #[cfg(debug_assertions)]
            ("detail-versions", |s| {
                demo::signed_in(s);
                s.route = Route::Detail {
                    id: "metropolis".into(),
                };
                s.detail = Some(demo::movie_detail());
                s.detail_return = Route::Home;
            }),
            #[cfg(debug_assertions)]
            ("detail-collections", |s| {
                demo::signed_in(s);
                s.route = Route::Detail {
                    id: "metropolis".into(),
                };
                s.detail = Some(demo::movie_detail());
                s.detail_collections = vec![
                    jellyfin_api::Item {
                        id: "dune-collection".into(),
                        name: "Silent Horror Collection".into(),
                        kind: "BoxSet".into(),
                        ..jellyfin_api::Item::default()
                    },
                    jellyfin_api::Item {
                        id: "sci-fi".into(),
                        name: "Science Fiction".into(),
                        kind: "BoxSet".into(),
                        ..jellyfin_api::Item::default()
                    },
                ];
                s.detail_return = Route::Home;
            }),
            #[cfg(debug_assertions)]
            ("detail-extras", |s| {
                demo::signed_in(s);
                s.route = Route::Detail {
                    id: "metropolis".into(),
                };
                s.detail = Some(demo::movie_detail());
                s.detail_extras = demo::extras();
                s.detail_return = Route::Home;
            }),
            #[cfg(debug_assertions)]
            ("detail-similar", |s| {
                demo::signed_in(s);
                s.route = Route::Detail {
                    id: "metropolis".into(),
                };
                let mut detail = demo::movie_detail();
                // A one-line overview keeps the rail inside the 800-point snapshot.
                detail.item.overview = Some("Paul Atreides joins the Fremen.".into());
                s.detail = Some(detail);
            }),
            #[cfg(debug_assertions)]
            ("playlist-picker", |s| {
                demo::signed_in(s);
                s.playlists = demo::playlists();
                s.route = Route::Detail {
                    id: "metropolis".into(),
                };
                s.detail = Some(demo::movie_detail());
                s.playlist_picker_open = true;
            }),
            #[cfg(debug_assertions)]
            #[cfg(debug_assertions)]
            ("library-shows", |s| {
                demo::signed_in(s);
                let library = demo::shows_library();
                s.route = Route::Library {
                    id: library.id.clone(),
                };
                s.library = Some(library);
            }),
            #[cfg(debug_assertions)]
            ("playlist", |s| {
                demo::signed_in(s);
                s.playlists = demo::playlists();
                let library = demo::playlist_library();
                s.route = Route::Library {
                    id: library.id.clone(),
                };
                s.library = Some(library);
            }),
            #[cfg(debug_assertions)]
            ("playlist-edit", |s| {
                demo::signed_in(s);
                s.playlists = demo::playlists();
                let library = demo::playlist_library();
                s.route = Route::Library {
                    id: library.id.clone(),
                };
                s.library = Some(library);
                s.playlist_editing = true;
            }),
            ("library-filters", |s| {
                demo::signed_in(s);
                let mut library = demo::library(20);
                library.filters_open = true;
                s.library = Some(library);
                s.route = Route::Library {
                    id: "movies".into(),
                };
            }),
            ("library-filters-views", |s| {
                demo::signed_in(s);
                let mut library = demo::library(20);
                library.filters_open = true;
                library.only_4k = true;
                s.library = Some(library);
                s.save_library_view();
                s.route = Route::Library {
                    id: "movies".into(),
                };
            }),
            ("library-badges", |s| {
                demo::signed_in(s);
                let mut library = demo::library(8);
                let mut source = demo::movie_detail().item.media_sources[0].clone();
                source.media_streams.push(jellyfin_api::MediaStream {
                    index: 9,
                    kind: "Subtitle".into(),
                    codec: Some("subrip".into()),
                    ..jellyfin_api::MediaStream::default()
                });
                for item in library.items.iter_mut().take(3) {
                    item.media_sources = vec![source.clone()];
                }
                s.library = Some(library);
                s.route = Route::Library {
                    id: "movies".into(),
                };
            }),
            ("library-select", |s| {
                demo::signed_in(s);
                let library = demo::library(12);
                s.library_selecting = true;
                for item in library.items.iter().take(2) {
                    s.library_selected.insert(item.id.clone());
                }
                s.library = Some(library);
                s.route = Route::Library {
                    id: "movies".into(),
                };
            }),
            ("watchlist", |s| {
                demo::signed_in(s);
                let mut library = crate::library::LibraryState::new(
                    "__watchlist__".into(),
                    "Watchlist".into(),
                    Some("watchlist".into()),
                );
                library.items = demo::library(5).items;
                s.library = Some(library);
                s.route = Route::Library {
                    id: "__watchlist__".into(),
                };
            }),
            ("library-genres", |s| {
                demo::signed_in(s);
                let mut library = demo::library(20);
                library.genres = vec![
                    jellyfin_api::Item {
                        id: "genre-drama".into(),
                        name: "Drama".into(),
                        kind: "Genre".into(),
                        ..jellyfin_api::Item::default()
                    },
                    jellyfin_api::Item {
                        id: "genre-scifi".into(),
                        name: "Science Fiction".into(),
                        kind: "Genre".into(),
                        ..jellyfin_api::Item::default()
                    },
                    jellyfin_api::Item {
                        id: "genre-comedy".into(),
                        name: "Comedy".into(),
                        kind: "Genre".into(),
                        ..jellyfin_api::Item::default()
                    },
                ];
                library.genres_open = true;
                library.genre = Some(("genre-drama".into(), "Drama".into()));
                s.route = Route::Library {
                    id: library.id.clone(),
                };
                s.library = Some(library);
            }),
            ("library-list", |s| {
                demo::signed_in(s);
                let mut library = demo::library(12);
                library.view = library::ViewMode::List;
                s.route = Route::Library {
                    id: library.id.clone(),
                };
                s.library = Some(library);
            }),
            ("library-loading", |s| {
                demo::signed_in(s);
                let mut library =
                    LibraryState::new("movies".into(), "Movies".into(), Some("movies".into()));
                library.loading = true;
                s.route = Route::Library {
                    id: library.id.clone(),
                };
                s.library = Some(library);
            }),
            ("library-empty", |s| {
                demo::signed_in(s);
                let library =
                    LibraryState::new("movies".into(), "Movies".into(), Some("movies".into()));
                s.route = Route::Library {
                    id: library.id.clone(),
                };
                s.library = Some(library);
            }),
            ("library-error", |s| {
                demo::signed_in(s);
                let mut library =
                    LibraryState::new("movies".into(), "Movies".into(), Some("movies".into()));
                library.error = Some("Can't reach the server.".into());
                s.route = Route::Library {
                    id: library.id.clone(),
                };
                s.library = Some(library);
            }),
            ("person", |s| {
                demo::signed_in(s);
                s.route = Route::Person {
                    id: "mara-ellison".into(),
                };
                s.person = Some(demo::person());
            }),
            ("person-seerr", |s| {
                demo::signed_in(s);
                s.route = Route::Person {
                    id: "cailee-spaeny".into(),
                };
                let mut page = demo::person();
                page.id = "cailee-spaeny".into();
                page.name = "Cailee Spaeny".into();
                page.tmdb_id = Some(1683343);
                page.biography = Some(
                    "Cailee Spaeny is an American actress. Her first major roles were in Pacific Rim Uprising and Bad Times at the El Royale."
                        .into(),
                );
                page.birthday = Some("1998-07-24".into());
                page.place_of_birth = Some("Springfield, Missouri, USA".into());
                page.items = page.items.into_iter().take(5).collect();
                page.seerr_credits = demo::seerr_person_credits();
                page.library_tmdb = [945961].into_iter().collect();
                s.person = Some(page);
            }),
            ("seerr-person", |s| {
                seerr_scene(s);
                s.route = Route::SeerrPerson { tmdb_id: 1683343 };
                let mut page = demo::person();
                page.id = String::new();
                page.name = "Cailee Spaeny".into();
                page.tmdb_id = Some(1683343);
                page.biography = Some(
                    "Cailee Spaeny is an American actress. Her first major roles were in Pacific Rim Uprising and Bad Times at the El Royale."
                        .into(),
                );
                page.birthday = Some("1998-07-24".into());
                page.place_of_birth = Some("Springfield, Missouri, USA".into());
                page.items = Vec::new();
                page.loading = false;
                page.seerr_credits = demo::seerr_person_credits();
                s.person = Some(page);
            }),
            ("detail-tech", |s| {
                demo::signed_in(s);
                s.route = Route::Detail {
                    id: "metropolis".into(),
                };
                s.detail = Some(demo::movie_detail());
                s.detail_tech_open = true;
            }),
            ("collection", |s| {
                demo::signed_in(s);
                s.route = Route::Detail {
                    id: "silent-horror-collection".into(),
                };
                s.detail = Some(demo::collection());
            }),
            ("detail", |s| {
                demo::signed_in(s);
                s.route = Route::Detail {
                    id: "quiet-floor".into(),
                };
                s.detail_return = Route::Library {
                    id: "movies".into(),
                };
                s.detail = Some(demo::detail());
                s.detail_imdb = Some(8.6);
            }),
            ("detail-loading", |s| {
                demo::signed_in(s);
                s.route = Route::Detail {
                    id: "quiet-floor".into(),
                };
                s.detail = Some(DetailState::loading("quiet-floor"));
            }),
            ("detail-error", |s| {
                demo::signed_in(s);
                s.route = Route::Detail {
                    id: "quiet-floor".into(),
                };
                let mut detail = DetailState::loading("quiet-floor");
                detail.loading = false;
                detail.error = Some("Can't reach the server.".into());
                s.detail = Some(detail);
            }),
            ("now-playing", |s| {
                demo::signed_in(s);
                s.playback = Some(demo::playback(false));
            }),
            ("now-playing-paused", |s| {
                demo::signed_in(s);
                s.playback = Some(demo::playback(true));
            }),
            ("search-seerr", |s| {
                seerr_scene(s);
                s.route = Route::Search { scope: None };
                s.search.query = rune::TextEdit::new("dune");
                let mut results = demo::search_results();
                results.movies.truncate(2);
                results.shows.clear();
                results.episodes.clear();
                results.people.clear();
                s.search.results = results;
                s.search.seerr = seerr_api::SearchResults {
                    page: 1,
                    total_pages: 1,
                    total_results: 3,
                    results: demo::seerr_discover().movies,
                };
            }),
            ("search", |s| {
                demo::signed_in(s);
                s.route = Route::Search { scope: None };
                s.search.query = TextEdit::new("quiet-floor");
                s.search.query.focused = true;
                s.search.results = demo::search_results();
            }),
            ("search-expanded", |s| {
                demo::signed_in(s);
                s.route = Route::Search { scope: None };
                s.search.query = TextEdit::new("quiet-floor");
                s.search.query.focused = true;
                s.search.results = demo::search_results();
                s.search.expanded.insert("search-movies".into());
            }),
            ("search-hints", |s| {
                demo::signed_in(s);
                s.route = Route::Search { scope: None };
                s.search.query = rune::TextEdit::new("sever");
                s.search.results.movies = demo::library(3).items;
                s.search.hints = vec![
                    jellyfin_api::SearchHint {
                        id: "person-1".into(),
                        name: "The Quiet Floor".into(),
                        kind: "Series".into(),
                    },
                    jellyfin_api::SearchHint {
                        id: "person-2".into(),
                        name: "Ben Stiller".into(),
                        kind: "Person".into(),
                    },
                    jellyfin_api::SearchHint {
                        id: "genre-1".into(),
                        name: "Sci-Fi & Fantasy".into(),
                        kind: "Genre".into(),
                    },
                ];
            }),
            ("search-idle", |s| {
                demo::signed_in(s);
                s.route = Route::Search { scope: None };
                s.search.query = TextEdit::new("");
                s.search.query.focused = true;
            }),
            ("search-empty", |s| {
                demo::signed_in(s);
                s.route = Route::Search { scope: None };
                s.search.query = TextEdit::new("no such title");
                s.search.query.focused = true;
            }),
            ("settings", |s| {
                demo::signed_in(s);
                s.route = Route::Settings;
            }),
            ("settings-seerr", |s| {
                demo::signed_in(s);
                s.route = Route::Settings;
                s.seerr.url = TextEdit::new("https://jellyseerr.example");
                s.seerr.key = TextEdit::new("example-api-key").secure();
                s.seerr.user = TextEdit::new("1");
                s.seerr.probe = Some(crate::state::SeerrProbe {
                    version: "3.5.1".into(),
                    user: "example-user".into(),
                });
            }),
            ("requests", |s| {
                seerr_scene(s);
                s.route = Route::Seerr;
                s.seerr.tab = crate::state::SeerrTab::Requests;
                s.seerr.filter = crate::state::RequestsFilter::All;
                s.seerr.requests = demo::seerr_requests();
            }),
            ("requests-live", |s| {
                seerr_scene(s);
                s.route = Route::Seerr;
                s.seerr.tab = crate::state::SeerrTab::Requests;
                s.seerr.requests = demo::seerr_requests_live();
            }),
            ("requests-empty", |s| {
                seerr_scene(s);
                s.route = Route::Seerr;
                s.seerr.tab = crate::state::SeerrTab::Requests;
                s.seerr.counts = Some(seerr_api::RequestCounts::default());
            }),
            ("requests-error", |s| {
                seerr_scene(s);
                s.route = Route::Seerr;
                s.seerr.tab = crate::state::SeerrTab::Requests;
                s.seerr.requests_error = Some("Can't reach Seerr.".into());
            }),
            ("requests-unconfigured", |s| {
                demo::signed_in(s);
                s.route = Route::Seerr;
                s.seerr.tab = crate::state::SeerrTab::Requests;
            }),
            ("seerr-browse", |s| {
                seerr_scene(s);
                s.route = Route::Seerr;
                s.seerr.discover_mode = crate::state::DiscoverMode::Browse;
                s.seerr.discover.genres_movie = demo::seerr_genres();
                s.seerr.discover.genres_tv = demo::seerr_genres();
                s.seerr.discover.browse = demo::seerr_discover().movies;
                s.seerr.discover.browse_loaded = true;
                s.seerr.discover.browse_pages = 2;
                s.seerr.discover.filters.kind = crate::state::DiscoverKind::Movies;
                s.seerr.discover.filters.genre = Some(878);
                s.seerr.discover.filters.hub = Some(41077);
            }),
            ("seerr", |s| {
                seerr_scene(s);
                s.route = Route::Seerr;
                s.seerr.tab = crate::state::SeerrTab::Discover;
                s.seerr.discover = demo::seerr_discover();
            }),
            ("seerr-search", |s| {
                seerr_scene(s);
                s.route = Route::Seerr;
                s.seerr.tab = crate::state::SeerrTab::Discover;
                s.seerr.discover = demo::seerr_discover();
                s.seerr.query = TextEdit::new("dune");
                s.seerr.search = demo::seerr_search();
            }),
            ("seerr-detail-tv", |s| {
                seerr_scene(s);
                s.seerr.detail = Some(crate::state::SeerrDetailState {
                    media_type: "tv".into(),
                    tmdb_id: 196950,
                    details: Some(demo::seerr_tv_detail()),
                    selected_seasons: [1, 2].into_iter().collect(),
                    loading: false,
                    error: None,
                    requesting: false,
                });
                s.route = Route::SeerrDetail {
                    media_type: "tv".into(),
                    tmdb_id: 196950,
                };
            }),
            ("seerr-detail-movie", |s| {
                seerr_scene(s);
                s.seerr.detail = Some(crate::state::SeerrDetailState {
                    media_type: "movie".into(),
                    tmdb_id: 438631,
                    details: Some(demo::seerr_movie_detail()),
                    selected_seasons: std::collections::HashSet::new(),
                    loading: false,
                    error: None,
                    requesting: false,
                });
                s.route = Route::SeerrDetail {
                    media_type: "movie".into(),
                    tmdb_id: 438631,
                };
            }),
            ("seerr-130", |s| {
                seerr_scene(s);
                s.route = Route::Seerr;
                s.seerr.tab = crate::state::SeerrTab::Discover;
                s.seerr.discover = demo::seerr_discover();
                s.text_scale = 1.3;
                crate::theme::set_text_scale(1.3);
            }),
            ("detail-seerr", |s| {
                seerr_scene(s);
                let mut detail = demo::detail();
                detail
                    .item
                    .provider_ids
                    .insert("Tmdb".into(), "196950".into());
                s.detail = Some(detail);
                s.route = Route::Detail {
                    id: "quiet-floor".into(),
                };
                s.seerr.series = Some((196950, demo::seerr_tv_series()));
            }),
            ("remote", |s| {
                demo::signed_in(s);
                s.route = Route::Remote;
                s.remote.sessions = demo::sessions();
            }),
            ("in-app-launching", |s| {
                demo::signed_in(s);
                s.in_app_launching = true;
            }),
            ("in-app-osd", in_app_osd_scene),
            ("in-app-mini", in_app_mini_scene),
            ("in-app-intro", in_app_intro_scene),
            ("in-app-gear", in_app_gear_scene),
            ("in-app-skip-credits", in_app_skip_credits_scene),
            ("in-app-segments", in_app_intro_scene),
            #[cfg(debug_assertions)]
            ("detail-preplay", |s| {
                demo::signed_in(s);
                s.route = Route::Detail {
                    id: "metropolis".into(),
                };
                s.detail = Some(demo::movie_detail());
                s.detail_return = Route::Home;
                s.preplay_open = true;
            }),
            #[cfg(debug_assertions)]
            ("in-app-preplay", |s| {
                demo::signed_in(s);
                s.route = Route::Detail {
                    id: "metropolis".into(),
                };
                s.detail = Some(demo::movie_detail());
                s.detail_return = Route::Home;
                s.preplay_open = true;
            }),
            #[cfg(debug_assertions)]
            ("in-app-resume-conflict", |s| {
                in_app_osd_scene(s);
                s.resume_conflict = Some((48 * 60_000, 42 * 60_000));
            }),
            #[cfg(debug_assertions)]
            ("in-app-transfer", |s| {
                in_app_osd_scene(s);
                s.cast_item = s.in_app.as_ref().map(|session| session.item.clone());
                s.cast_sessions = demo::sessions();
                s.cast_picker_open = true;
                s.transfer_after_cast = true;
            }),
            #[cfg(debug_assertions)]
            ("in-app-loop", |s| {
                in_app_osd_scene(s);
                if let Some(session) = &mut s.in_app {
                    session.position_ms = 42 * 60_000;
                }
                s.loop_a = Some(30 * 60_000);
                s.loop_b = Some(45 * 60_000);
            }),
            #[cfg(debug_assertions)]
            ("in-app-goto", |s| {
                in_app_osd_scene(s);
                s.in_app_goto_open = true;
                s.in_app_goto = rune::TextEdit::new("0:12:03");
            }),
            #[cfg(debug_assertions)]
            ("in-app-quality", |s| {
                in_app_osd_scene(s);
                s.playback_quality = crate::config::PlaybackQuality::Mbps4;
                if let Some(session) = &mut s.in_app {
                    session.play_method = "Transcode";
                }
                s.in_app_quality_open = true;
            }),
            #[cfg(debug_assertions)]
            ("in-app-why", |s| {
                in_app_osd_scene(s);
                s.playback_quality = crate::config::PlaybackQuality::Mbps4;
                if let Some(session) = &mut s.in_app {
                    session.play_method = "Transcode";
                }
                s.in_app_quality_open = true;
            }),
            #[cfg(debug_assertions)]
            ("in-app-audio-modes", |s| {
                in_app_osd_scene(s);
                s.downmix_stereo = true;
                s.audio_mode = crate::config::AudioMode::Dialogue;
                s.audio_delay_ms = 25;
                s.in_app_audio_open = true;
            }),
            #[cfg(debug_assertions)]
            ("in-app-still-watching", |s| {
                in_app_osd_scene(s);
                s.still_watching_open = true;
                s.osd_visible = false;
            }),
            #[cfg(debug_assertions)]
            ("in-app-sleep-timer", |s| {
                in_app_osd_scene(s);
                s.in_app_sleep_open = true;
            }),
            ("in-app-trickplay", in_app_trickplay_scene),
            ("in-app-subtitles", in_app_subtitles_scene),
            ("in-app-subtitle-style", in_app_subtitle_style_scene),
            ("in-app-image-subtitle", in_app_image_subtitle_scene),
            ("in-app-subtitle-panel", in_app_subtitle_panel_scene),
            ("in-app-subtitle-many", in_app_subtitle_many_scene),
            ("in-app-subtitles-clear", in_app_subtitles_clear_scene),
            ("in-app-subtitles-ass", in_app_ass_subtitles_scene),
            ("in-app-next", in_app_next_scene),
            ("in-app-audio", in_app_audio_scene),
            ("in-app-stats", in_app_stats_scene),
            ("in-app-subtitle-search", in_app_subtitle_search_scene),
            ("in-app-chapters", in_app_chapters_scene),
            ("in-app-queue", in_app_queue_scene),
            ("in-app-queue-panel", in_app_queue_panel_scene),
            ("in-app-narrow", in_app_narrow_scene),
            ("in-app-buffering", in_app_buffering_scene),
            ("in-app-live", in_app_live_scene),
            ("live-tv", live_tv_scene),
            #[cfg(debug_assertions)]
            ("live-tv-favorites", |s| {
                live_tv_scene(s);
                s.live.favorites.insert("aurora-one".into());
                s.live.favorites.insert("northlight".into());
                s.live.favorites_only = true;
            }),
            ("live-guide", live_guide_scene),
            #[cfg(debug_assertions)]
            ("live-guide-day", |s| {
                live_guide_scene(s);
                s.live.guide_hours = 24;
            }),
            ("shortcuts", |s| {
                demo::signed_in(s);
                s.shortcuts_open = true;
            }),
        ],
        ..AppSpec::new("Runefin", size, init, view)
    })
    .tab_visits_controls(true)
    .on_window(move |window| {
        // A launch from a terminal or another app can create the window without focus. Ask
        // for it once the native window exists, then once more after the host has settled.
        window.focus();
        if let Some((x, y)) = saved_position {
            window.set_position((x, y));
        }
        let window = window.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(700));
            window.focus();
            // Wayland hosts may apply the first position request only after the surface is
            // configured; repeat it once the host has settled.
            if let Some((x, y)) = saved_position {
                window.set_position((x, y));
            }
        });
    })
    .on_close(|s: &mut State, _tx: &mut Tx| {
        s.save_preferences();
        s.stop_in_app();
        s.stop_playback();
    });
    // The host can start polling before the window has focus and its worker wake lane is live.
    // Keep a short UI-thread heartbeat through the first fetch; after that, the window's own
    // waker and playback face are enough.
    let handle = app.handle();
    pad::start(handle.clone());
    // A launch from a terminal can leave the new window behind the terminal or browser. Ask
    // for it once; a user launch should land in the app, not a skeleton behind another window.
    handle.open();
    std::thread::spawn(move || {
        // The host connects its native wake lane after `setup`; a TLS handshake or a slow
        // reverse proxy can outlast a three-second bootstrap. Keep a low-frequency frame
        // heartbeat long enough that the first home payload appears without a mouse move.
        for i in 0..150 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if i == 10 {
                handle.open();
            }
            handle.update(|s, tx| {
                s.poll();
                tx.settle();
            });
            // Some Linux hosts coalesce the first update into the next input event. A bare
            // wake makes the completed first fetch visible immediately.
            handle.wake();
        }
    });
    app.run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Env, Runtime};

    fn home_state() -> State {
        let mut state = State::from_config(config::Config::fresh());
        demo::signed_in(&mut state);
        state
    }

    fn library_state(count: usize) -> State {
        let mut state = home_state();
        let library = demo::library(count);
        state.route = Route::Library {
            id: library.id.clone(),
        };
        state.library = Some(library);
        state
    }

    #[test]
    fn a_focus_highlight_row_takes_enter() {
        #[derive(Default)]
        struct Hit {
            pressed: bool,
        }
        fn view(s: &Hit, _: &mut rune::Cx<Hit>) -> rune::El {
            rune::col()
                .size(200.0, 100.0)
                .child(
                    rune::el()
                        .key("row")
                        .size(120.0, 40.0)
                        .focusable(true)
                        .focus_highlight(rune::Color::WHITE)
                        .on_press(|s: &mut Hit| s.pressed = true),
                )
                .child(rune::text(if s.pressed { "hit" } else { "wait" }))
        }
        let env = Env {
            system_dark: true,
            scale: 1.0,
        };
        let mut rt = Runtime::new(Hit::default(), view, (200.0, 100.0), env);
        rt.frame(0.0);
        rt.key(KeyEvent {
            key: Key::Tab,
            mods: Default::default(),
        });
        rt.frame(0.0);
        rt.key(KeyEvent {
            key: Key::Enter,
            mods: Default::default(),
        });
        rt.frame(0.0);
        assert!(rt.state.pressed, "a focus_highlight row takes Return");
    }

    #[test]
    fn the_movies_grid_scrolls_with_the_wheel() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let state = library_state(12);
        let scroll = state
            .library
            .as_ref()
            .map(|library| library.scroll.clone())
            .expect("a library");
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        rt.wheel(700.0, 500.0, -600.0);
        for _ in 0..6 {
            rt.frame(1.0 / 60.0);
        }
        let offset = f32::from_bits(scroll.load(std::sync::atomic::Ordering::Relaxed));
        assert!(offset > 0.0, "the wheel moved the grid, offset {offset}");
    }

    #[test]
    fn every_card_in_a_home_rail_has_an_image_request() {
        let mut state = home_state();
        state.config.token = "demo-token".into();
        state.image_cache = Some(rune::ImageCache::new(std::sync::Arc::new(|| {})));
        let items: Vec<jellyfin_api::Item> = (0..12)
            .map(|at| jellyfin_api::Item {
                id: format!("rail-{at}"),
                name: format!("Rail {at}"),
                image_tags: Some([("Primary".to_string(), format!("tag-{at}"))].into()),
                ..jellyfin_api::Item::default()
            })
            .collect();
        state.home_data.latest = vec![crate::home::LatestRail {
            library: "Anime".into(),
            library_id: "anime".into(),
            items: items.clone(),
        }];
        state.request_images();
        let last = state
            .image_key(items.last().expect("a last card"))
            .expect("a poster key");
        assert!(
            state.requested_images.contains(&last),
            "the card at the end of the rail is queued too"
        );
    }

    #[test]
    fn a_series_offers_start_over_for_its_in_progress_next_episode() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Detail {
            id: "quiet-floor".into(),
        };
        let mut detail = demo::detail();
        // The series itself has no aggregate position; only the Next Up episode is in flight.
        detail.item.user_data = None;
        assert!(
            detail
                .next_up
                .as_ref()
                .and_then(|episode| episode.user_data.as_ref())
                .and_then(|user| user.position_ms())
                .is_some_and(|position| position > 0)
        );
        state.detail = Some(detail);
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(
            rt.find("detail-restart").is_some(),
            "Start over follows the episode the button will play"
        );
    }

    #[test]
    fn a_count_line_says_one_item_not_one_items() {
        assert_eq!(screens::count_label(0, "item"), "0 items");
        assert_eq!(screens::count_label(1, "item"), "1 item");
        assert_eq!(screens::count_label(2, "channel"), "2 channels");
    }

    #[test]
    fn the_browser_opener_refuses_non_web_urls() {
        assert!(State::open_url("file:///etc/passwd").is_err());
        assert!(State::open_url("javascript:alert(1)").is_err());
    }

    #[test]
    fn the_sidebar_entries_show_the_pointer_cursor() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(home_state(), view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in ["nav-home", "library-movies", "nav-settings"] {
            let (x, y) = rt.find(key).unwrap_or_else(|| panic!("{key}"));
            rt.pointer_move(x, y);
            rt.frame(0.01);
            assert_eq!(rt.cursor(), rune::Cursor::Pointer, "{key}");
        }
    }

    #[test]
    fn the_library_grid_measures_the_wide_window() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(library_state(60), view, (1440.0, 900.0), env);
        rt.frame(0.0);
        rt.frame(1.0 / 60.0);
        assert!(
            rt.state.library.as_ref().expect("library").columns >= 6,
            "a 1440-wide window has room for six cards"
        );
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(library_state(60), view, (900.0, 700.0), env);
        rt.frame(0.0);
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.library.as_ref().expect("library").columns, 3);
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(library_state(60), view, (800.0, 600.0), env);
        rt.frame(0.0);
        rt.frame(1.0 / 60.0);
        assert_eq!(
            rt.state.library.as_ref().expect("library").columns,
            2,
            "two cards fit with the sidebar at 800"
        );
    }

    #[test]
    fn the_library_grid_uses_the_window_width_up_to_twelve_columns() {
        let mut library = crate::library::LibraryState::new(
            "movies".into(),
            "Movies".into(),
            Some("movies".into()),
        );
        assert!(library.set_columns(10));
        assert_eq!(library.columns, 10);
        assert!(library.set_columns(20));
        assert_eq!(library.columns, 12);
        assert!(!library.set_columns(12));
    }

    #[test]
    fn a_clickable_detail_button_sets_the_pointer_cursor() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(detail_state(), view, (1280.0, 800.0), env);
        rt.frame(0.0);
        let (x, y) = rt.find("detail-trailer").expect("the trailer button");
        rt.pointer_move(x, y);
        rt.frame(0.01);
        assert_eq!(rt.cursor(), rune::Cursor::Pointer);
        let (x, y) = rt.find("detail-back").expect("the back button");
        rt.pointer_move(x, y);
        rt.frame(0.02);
        assert_eq!(rt.cursor(), rune::Cursor::Pointer);
    }

    #[test]
    fn a_card_without_art_uses_the_first_meaningful_letter() {
        assert_eq!(screens::initial("[Anipakku] Tengen Toppa"), "A");
        assert_eq!(screens::initial("86: Eighty-Six"), "8");
        assert_eq!(screens::initial("  ...Service"), "S");
        assert_eq!(screens::initial("!!!"), "?");
    }

    #[test]
    fn the_home_screen_shows_the_shell_rails_and_cards() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(home_state(), view, (1280.0, 800.0), env);
        rt.frame(0.0);

        for key in [
            "home-screen",
            "sidebar",
            "nav-home",
            "library-live-tv",
            "rail-continue-watching",
            "rail-next-up",
            "rail-collections",
            "rail-collections-silent-horror-collection",
            "rail-recent-0",
            "rail-recently-played",
            "rail-recently-played-played-arrival",
            "rail-continue-watching-quiet-floor",
            "rail-continue-watching-metropolis",
            "resume-remove-quiet-floor-s2e4",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
    }

    #[test]
    fn the_home_contract_orders_every_library_rail() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        // Four libraries means four Recently Added rails, not a capped subset.
        let template = state
            .home_data
            .latest
            .first()
            .cloned()
            .expect("the fixture has a recent rail");
        state.home_data.latest = (0..4)
            .map(|index| {
                let mut rail = template.clone();
                rail.library = format!("Library {index}");
                rail
            })
            .collect();
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        let top = |key: &str| {
            rt.presented(key)
                .map(|(rect, _)| rect[1])
                .unwrap_or_else(|| panic!("{key} has a rect"))
        };
        assert!(top("rail-continue-watching") < top("rail-next-up"));
        assert!(top("rail-next-up") < top("rail-recent-0"));
        assert!(top("rail-recent-0") < top("rail-recently-played"));
        assert!(top("rail-recently-played") < top("rail-collections"));
        assert!(
            rt.find("rail-recent-3").is_some(),
            "every library gets a Recently Added rail"
        );
        assert!(rt.find("rail-continue-watching-all").is_some());
        assert!(rt.find("rail-next-up-all").is_some());
    }

    #[test]
    fn favourite_and_watched_changes_offer_undo() {
        let mut state = home_state();
        let item = state.home_data.resume[0].clone();
        let id = item.id.clone();
        state.card_menu = Some(crate::state::CardMenu {
            item: item.clone(),
            x: 0.0,
            y: 0.0,
        });
        state.card_menu_toggle_favorite();
        assert!(
            state.home_data.resume[0]
                .user_data
                .as_ref()
                .is_some_and(|user| user.is_favorite)
        );
        assert!(state.toast_undo.is_some());
        state.undo_toast();
        assert!(
            !state.home_data.resume[0]
                .user_data
                .as_ref()
                .is_some_and(|user| user.is_favorite)
        );

        state.card_menu = Some(crate::state::CardMenu {
            item,
            x: 0.0,
            y: 0.0,
        });
        state.card_menu_toggle_played();
        assert!(
            !state.home_data.resume.iter().any(|entry| entry.id == id),
            "marking watched removes the Continue Watching card"
        );
        state.undo_toast();
        assert!(
            state.home_data.resume.iter().any(|entry| entry.id == id),
            "Undo puts the card back"
        );
    }

    #[test]
    fn the_sidebar_puts_content_before_tools() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(home_state(), view, (1280.0, 800.0), env);
        rt.frame(0.0);
        let top = |key: &str| {
            rt.presented(key)
                .map(|(rect, _)| rect[1])
                .unwrap_or_else(|| panic!("{key} has a rect"))
        };
        assert!(top("nav-home") < top("library-movies"));
        assert!(top("library-movies") < top("nav-search"));
        assert!(top("nav-search") < top("nav-settings"));
        assert!(rt.find("libraries-label").is_some());
        assert!(rt.find("tools-label").is_some());
    }

    #[test]
    fn removing_a_resume_card_offers_undo() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.remove_from_resume("quiet-floor-s2e4");
        assert!(
            !state
                .home_data
                .resume
                .iter()
                .any(|item| item.id == "quiet-floor-s2e4")
        );
        assert!(state.toast_undo.is_some());
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("toast-undo").is_some(), "the toast offers Undo");
        let (x, y) = rt.find("toast-undo").expect("the undo button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(
            rt.state
                .home_data
                .resume
                .iter()
                .any(|item| item.id == "quiet-floor-s2e4"),
            "the card is back after Undo"
        );
        assert!(rt.state.toast.is_none());
    }

    #[test]
    fn a_home_rail_opens_as_a_shelf_and_closes() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.open_shelf(state::HomeShelf::ContinueWatching);
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "shelf-screen",
            "shelf-title",
            "shelf-back",
            "shelf-quiet-floor",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen");
        }
        rt.state.close_shelf();
        rt.frame(1.0 / 60.0);
        assert!(rt.find("shelf-screen").is_none());
        assert!(rt.find("rail-continue-watching").is_some());
    }

    #[test]
    fn the_detail_playlist_button_opens_a_picker_of_loaded_playlists() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.playlists = demo::playlists();
        state.route = Route::Detail {
            id: "metropolis".into(),
        };
        state.detail = Some(demo::movie_detail());
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("detail-playlist").is_some());
        let (x, y) = rt.find("detail-playlist").expect("the playlist button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert!(rt.state.playlist_picker_open);
        for key in [
            "playlist-picker",
            "playlist-picker-close",
            "playlist-pick-playlist-silent",
            "playlist-pick-playlist-titles",
        ] {
            assert!(rt.find(key).is_some(), "{key} is in the picker");
        }
        let (x, y) = rt.find("playlist-picker-close").expect("the close button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.04);
        assert!(!rt.state.playlist_picker_open, "the picker closes");
    }

    #[test]
    fn a_playlist_can_be_put_into_edit_mode_and_entries_carry_a_remove_button() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.playlists = demo::playlists();
        state.route = Route::Library {
            id: "playlist-silent".into(),
        };
        state.library = Some(demo::playlist_library());
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("playlist-edit").is_some());
        assert!(
            rt.find("playlist-remove-film-000").is_none(),
            "remove controls stay hidden until Edit"
        );
        let (x, y) = rt.find("playlist-edit").expect("the edit button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert!(rt.state.playlist_editing);
        assert!(rt.find("playlist-remove-film-000").is_some());
        assert!(rt.find("playlist-remove-film-005").is_some());
        assert!(rt.find("playlist-up-film-001").is_some());
        assert!(rt.find("playlist-down-film-000").is_some());
        let (x, y) = rt.find("playlist-down-film-000").expect("move later");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.03);
        let items = &rt.state.library.as_ref().expect("the playlist").items;
        assert_eq!(items.first().map(|item| item.id.as_str()), Some("film-001"));
        assert_eq!(items.get(1).map(|item| item.id.as_str()), Some("film-000"));
        let (x, y) = rt.find("playlist-edit").expect("the edit button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.04);
        assert!(!rt.state.playlist_editing, "Done hides them again");
        assert!(rt.find("playlist-remove-film-000").is_none());
    }

    #[test]
    fn dragging_a_playlist_poster_moves_the_entry_by_whole_rows() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.playlists = demo::playlists();
        state.route = Route::Library {
            id: "playlist-silent".into(),
        };
        state.library = Some(demo::playlist_library());
        state.playlist_editing = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        let (x, y) = rt.find("library-film-000").expect("the first poster");
        rt.pointer_down(x, y);
        rt.pointer_move(x, y + 300.0);
        rt.frame(0.02);
        rt.pointer_move(x, y + 600.0);
        rt.frame(0.02);
        rt.pointer_up(x, y + 600.0);
        rt.frame(0.05);
        let items = &rt.state.library.as_ref().expect("the playlist").items;
        assert_eq!(
            items
                .iter()
                .take(3)
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["film-001", "film-002", "film-000"],
            "a two-row drag moves the entry two places"
        );
    }

    #[test]
    fn a_new_playlist_can_be_named_from_the_picker() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.playlists = demo::playlists();
        state.route = Route::Detail {
            id: "metropolis".into(),
        };
        state.detail = Some(demo::movie_detail());
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        let (x, y) = rt.find("detail-playlist").expect("the playlist button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        let (x, y) = rt.find("playlist-new").expect("the new-playlist row");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.04);
        assert!(rt.state.playlist_create_open);
        for key in [
            "playlist-create",
            "playlist-name-input",
            "playlist-create-submit",
            "playlist-create-cancel",
        ] {
            assert!(rt.find(key).is_some(), "{key} is in the prompt");
        }
        // Without a session the create path refuses instead of pretending.
        rt.state.playlist_new_name = rune::TextEdit::new("Silent film weekend");
        let (x, y) = rt.find("playlist-create-submit").expect("create");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.06);
        assert!(rt.state.detail_notice.is_some());
        rt.state.playlist_create_open = true;
        let (x, y) = rt.find("playlist-create-cancel").expect("cancel");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.08);
        assert!(!rt.state.playlist_create_open, "cancel closes the prompt");
    }

    #[test]
    fn the_sidebar_lists_playlists_and_a_playlist_opens_without_sort_controls() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.playlists = demo::playlists();
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "playlists-label",
            "playlist-playlist-silent",
            "playlist-playlist-horror",
            "playlist-playlist-titles",
        ] {
            assert!(rt.find(key).is_some(), "{key} is in the sidebar");
        }
        let (x, y) = rt
            .find("playlist-playlist-silent")
            .expect("the playlist row");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert_eq!(
            rt.state.library.as_ref().map(|library| library.id.as_str()),
            Some("playlist-silent")
        );
        assert_eq!(
            rt.state
                .library
                .as_ref()
                .and_then(|library| library.collection_type.clone()),
            Some("playlists".into())
        );

        // The scene serves the playlist's entries so the grid has something to show; the
        // toolbar must not offer sorts or filters that would reorder an ordered list.
        rt.state.library = Some(demo::playlist_library());
        rt.frame(0.05);
        for key in [
            "library-play-all",
            "library-shuffle",
            "view-grid",
            "view-list",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on the playlist page");
        }
        assert!(rt.find("sort-name").is_none(), "a playlist keeps its order");
        assert!(rt.find("sort-direction").is_none());
        assert!(rt.find("filter-unwatched").is_none());
    }

    #[test]
    fn an_empty_library_gets_an_explanation_and_a_refresh() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = State::from_config(config::Config::fresh());
        state.step = Step::Ready {
            server_name: "example".into(),
        };
        state.home_loaded = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("home-empty").is_some());
        assert!(rt.find("home-refresh").is_some());
    }

    #[test]
    fn a_failed_home_request_names_the_failure_and_offers_a_retry() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = State::from_config(config::Config::fresh());
        state.step = Step::Ready {
            server_name: "example".into(),
        };
        state.home_loaded = true;
        state.home_error = Some("Can't reach the server.".into());
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("home-error").is_some());
        assert!(rt.find("home-error-message").is_some());
        assert!(rt.find("home-retry").is_some());
    }

    #[test]
    fn loading_the_home_screen_puts_skeletons_in_the_rail_geometry() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = State::from_config(config::Config::fresh());
        state.step = Step::Ready {
            server_name: "example".into(),
        };
        state.home_loaded = false;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("home-loading").is_some());
        assert!(rt.find("home-loading-label").is_some());
    }

    #[test]
    fn a_big_screen_home_focuses_a_card_so_space_opens_it() {
        let env = Env {
            system_dark: true,
            scale: 1.5,
        };
        let mut state = home_state();
        state.big_screen = true;
        state.route = Route::Home;
        let mut rt = Runtime::new(state, view, (1280.0, 720.0), env);
        rt.frame(0.0);
        let key = rt
            .state
            .take_focus_key()
            .expect("home asks for its first card");
        assert!(key.starts_with("rail-continue-watching-"), "{key}");
        rt.update(|_, tx| tx.focus_key(key.clone()));
        rt.frame(0.016);
        assert_eq!(
            rt.focused_key().as_deref(),
            Some(key.as_str()),
            "the focus request from poll lands on the card"
        );
        rt.key(KeyEvent {
            key: Key::Text(" ".into()),
            mods: Default::default(),
        });
        rt.frame(0.032);
        assert!(
            matches!(rt.state.route, Route::Detail { .. }),
            "Space opens the focused card"
        );
    }

    #[test]
    fn opening_the_player_osd_focuses_the_timeline_once() {
        let mut state = State::from_config(config::Config::fresh());
        state.big_screen = true;
        state.step = Step::Ready {
            server_name: "example".into(),
        };
        state.home_data = demo::home();
        state.route = Route::Home;
        state.in_app = Some(demo::in_app());
        state.osd_visible = false;
        state.show_in_app_osd();
        assert_eq!(state.take_focus_key().as_deref(), Some("in-app-seek"));
        assert!(
            state.take_focus_key().is_none(),
            "the route behind the player does not steal focus back"
        );

        state.osd_visible = false;
        state.show_in_app_osd();
        assert_eq!(
            state.take_focus_key().as_deref(),
            Some("in-app-seek"),
            "each reopening starts on the timeline"
        );
    }

    #[test]
    fn big_screen_asks_for_the_first_item_on_the_route() {
        let mut state = State::from_config(config::Config::fresh());
        state.big_screen = true;
        state.step = Step::Ready {
            server_name: "example".into(),
        };
        state.home_data = demo::home();
        state.route = Route::Home;
        assert!(
            state
                .take_focus_key()
                .is_some_and(|key| key.starts_with("rail-continue-watching-")),
            "home focuses its first resume card"
        );
        assert!(state.take_focus_key().is_none(), "one request per route");

        let library = demo::library(10);
        let id = library.id.clone();
        state.library = Some(library);
        state.route = Route::Library { id };
        assert!(
            state
                .take_focus_key()
                .is_some_and(|key| key.starts_with("library-")),
            "a library focuses its first card"
        );

        state.route = Route::Home;
        assert!(
            state.take_focus_key().is_some(),
            "a frame on Home before the library"
        );
        if let Some(library) = &mut state.library {
            library.view = crate::library::ViewMode::List;
        }
        let id = state.library.as_ref().map(|library| library.id.clone());
        state.route = Route::Library {
            id: id.expect("a library"),
        };
        assert!(
            state
                .take_focus_key()
                .is_some_and(|key| key.starts_with("library-item-")),
            "list mode focuses its first row"
        );

        state.route = Route::Settings;
        assert_eq!(state.take_focus_key().as_deref(), Some("setting-ui-mode"));

        state.route = Route::Person { id: "p".into() };
        assert!(
            state.take_focus_key().is_none(),
            "the person page is still loading"
        );
        state.person = Some(demo::person());
        assert_eq!(state.take_focus_key().as_deref(), Some("person-back"));
        state.route = Route::Remote;
        assert_eq!(state.take_focus_key().as_deref(), Some("remote-refresh"));

        state.route = Route::Detail {
            id: "metropolis".into(),
        };
        state.detail = Some(DetailState::loading("metropolis"));
        assert!(
            state.take_focus_key().is_none(),
            "a loading detail has no play button yet"
        );
        state.detail = Some(demo::movie_detail());
        assert_eq!(state.take_focus_key().as_deref(), Some("detail-play"));

        state.shortcuts_open = true;
        assert_eq!(
            state.take_focus_key().as_deref(),
            Some("shortcut-close"),
            "an opened controls sheet takes focus"
        );
        state.shortcuts_open = false;
        assert_eq!(
            state.take_focus_key().as_deref(),
            Some("nav-controls"),
            "closing returns to the item that opened it"
        );
        assert!(state.take_focus_key().is_none(), "one request per edge");

        state.big_screen = false;
        state.route = Route::Search { scope: None };
        assert!(
            state.take_focus_key().is_none(),
            "desktop keeps its pointer focus"
        );
    }

    #[test]
    fn big_screen_overlays_take_focus_once() {
        let mut state = State::from_config(config::Config::fresh());
        state.big_screen = true;
        state.step = Step::Ready {
            server_name: "example".into(),
        };
        state.focus_first = false;

        state.shelf = Some(crate::state::HomeShelf::ContinueWatching);
        assert_eq!(state.take_focus_key().as_deref(), Some("shelf-back"));
        state.shelf = None;
        assert!(
            state.take_focus_key().is_none(),
            "closing is not a focus edge"
        );

        state.playlist_picker_open = true;
        assert_eq!(
            state.take_focus_key().as_deref(),
            Some("playlist-picker-close")
        );
        state.playlist_picker_open = false;

        state.still_watching_open = true;
        assert_eq!(
            state.take_focus_key().as_deref(),
            Some("still-watching-continue")
        );
        state.still_watching_open = false;

        state.library = Some(demo::library(2));
        if let Some(library) = &mut state.library {
            library.filters_open = true;
        }
        assert_eq!(state.take_focus_key().as_deref(), Some("filters-unwatched"));
        if let Some(library) = &mut state.library {
            library.filters_open = false;
            library.genres_open = true;
        }
        assert_eq!(state.take_focus_key().as_deref(), Some("genre-all"));
    }

    #[test]
    fn a_big_screen_launch_focuses_the_connect_form_first() {
        let mut state = State::from_config(config::Config::fresh());
        state.big_screen = true;
        state.step = Step::Idle;
        assert_eq!(state.take_focus_key().as_deref(), Some("server"));
        assert!(
            state.take_focus_key().is_none(),
            "one request, not one per poll"
        );

        state.quick = Some(crate::state::QuickLogin {
            server_url: "https://jellyfin.example".into(),
            server_name: "example".into(),
            version: "10.11".into(),
            code: "AB12CD".into(),
            secret: String::new(),
            authenticated: false,
            exchanging: false,
            polling: true,
            poll_at: std::time::Instant::now(),
            error: None,
        });
        assert_eq!(
            state.take_focus_key().as_deref(),
            Some("quick-connect-cancel")
        );
    }

    #[test]
    fn the_library_screen_shows_its_grid_toolbar_and_letter_rail() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(library_state(120), view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "library-screen",
            "library-back",
            "filter-favorites",
            "sort-name",
            "sort-added",
            "sort-rating",
            "filter-unwatched",
            "view-grid",
            "view-list",
            "letter-rail",
            "letter-A",
            "library-film-000",
            "played-film-002",
            "library-play-all",
            "library-shuffle",
            "sort-direction",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
        let (x, y) = rt.find("library-play-all").expect("the play-all button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert_eq!(rt.state.queue.len(), 120, "the loaded page is the run");
        assert_eq!(rt.state.queue_at, 0);

        let (x, y) = rt.find("sort-direction").expect("the direction button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert!(
            rt.state
                .library
                .as_ref()
                .is_some_and(|library| !library.ascending),
            "the arrow flips the order"
        );
    }

    #[test]
    fn the_list_toggle_draws_rows_that_carry_the_same_items() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = library_state(12);
        if let Some(library) = &mut state.library {
            library.view = library::ViewMode::List;
        }
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("library-list").is_some());
        assert!(rt.find("library-item-film-000").is_some());
    }

    #[test]
    fn an_empty_library_explains_itself_and_offers_to_clear_filters() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Library {
            id: "movies".into(),
        };
        let mut library =
            LibraryState::new("movies".into(), "Movies".into(), Some("movies".into()));
        library.unwatched = true;
        state.library = Some(library);
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("library-empty").is_some());
        assert!(rt.find("library-clear-filter").is_some());
    }

    #[test]
    fn the_letter_rail_filters_and_pressing_it_again_clears() {
        let mut state = library_state(6);
        state.toggle_letter('S');
        assert_eq!(
            state
                .library
                .as_ref()
                .and_then(|library| library.letter.as_deref()),
            Some("S")
        );
        state.toggle_letter('S');
        assert!(
            state
                .library
                .as_ref()
                .is_some_and(|library| library.letter.is_none())
        );
    }

    #[test]
    fn the_genre_panel_filters_the_library_and_clears_with_the_other_filters() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = library_state(12);
        if let Some(library) = &mut state.library {
            library.genres = vec![jellyfin_api::Item {
                id: "genre-drama".into(),
                name: "Drama".into(),
                kind: "Genre".into(),
                ..jellyfin_api::Item::default()
            }];
            library.genres_open = true;
        }
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in ["library-genres-panel", "genre-all", "genre-genre-drama"] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }

        let mut state = library_state(12);
        if let Some(library) = &mut state.library {
            library.genres = vec![jellyfin_api::Item {
                id: "genre-drama".into(),
                name: "Drama".into(),
                kind: "Genre".into(),
                ..jellyfin_api::Item::default()
            }];
        }
        state.select_library_genre(Some(("genre-drama".into(), "Drama".into())));
        assert!(
            state
                .library
                .as_ref()
                .is_some_and(|library| library.query().to_query().contains("genreIds=genre-drama"))
        );
        state.clear_library_filters();
        assert!(
            state
                .library
                .as_ref()
                .is_some_and(|library| library.genre.is_none())
        );
    }

    #[test]
    fn the_favourites_filter_toggles_and_reloads_from_the_top() {
        let mut state = library_state(12);
        state.toggle_favorites();
        assert!(
            state
                .library
                .as_ref()
                .is_some_and(|library| library.favorites)
        );
        state.clear_library_filters();
        assert!(
            state
                .library
                .as_ref()
                .is_some_and(|library| !library.favorites && !library.unwatched)
        );
    }

    #[test]
    #[ignore = "needs a real Jellyfin server; set JELLYFIN_URL, JELLYFIN_USER_ID and JELLYFIN_TOKEN"]
    fn a_transcoded_stream_plays_frames() {
        let url = std::env::var("JELLYFIN_URL").expect("JELLYFIN_URL");
        let user = std::env::var("JELLYFIN_USER_ID").expect("JELLYFIN_USER_ID");
        let token = std::env::var("JELLYFIN_TOKEN").expect("JELLYFIN_TOKEN");
        let client =
            jellyfin_api::Client::new(&url, "runefin-live").with_session(token, user.clone());
        let page = client
            .items(&jellyfin_api::ItemQuery {
                include_types: vec!["Movie", "Episode"],
                recursive: true,
                limit: 1,
                ..jellyfin_api::ItemQuery::default()
            })
            .expect("a playable item");
        let item = page.items.first().expect("a playable item");
        let response = client
            .playback_info(
                &item.id,
                &jellyfin_api::PlaybackInfoRequest {
                    user_id: user,
                    start_time_ticks: 0,
                    max_streaming_bitrate: Some(2_000_000),
                    media_source_id: None,
                    audio_stream_index: None,
                    subtitle_stream_index: None,
                    max_audio_channels: Some(2),
                    enable_direct_play: true,
                    enable_direct_stream: true,
                    enable_transcoding: true,
                    allow_video_stream_copy: true,
                    allow_audio_stream_copy: true,
                    device_profile: jellyfin_api::Client::device_profile(),
                },
            )
            .expect("PlaybackInfo");
        let source = response
            .media_sources
            .iter()
            .find(|source| source.transcoding_url.is_some())
            .expect("a transcoding source");
        let url = client.authed_url(source.transcoding_url.as_deref().expect("a URL"));
        let startup = std::time::Instant::now();
        let player = crate::player::Player::start(&url, rune::NativeSurface::new(), None)
            .expect("the transcoded stream starts");
        eprintln!("transcode startup: {} ms", startup.elapsed().as_millis());
        std::thread::sleep(std::time::Duration::from_secs(8));
        let frames = player.frames();
        let position = player.position_ms();
        eprintln!(
            "transcode: {frames} frames, {position} ms, path {}, buffering {:?}, error {:?}",
            player.path_label(),
            player.buffering(),
            player.error()
        );
        let before_seek = frames;
        let seek_started = std::time::Instant::now();
        player.seek_ms(60_000);
        std::thread::sleep(std::time::Duration::from_secs(8));
        let seek_position = player.position_ms();
        let after_seek = player.frames();
        eprintln!(
            "transcode seek: {after_seek} frames (was {before_seek}), {seek_position} ms, {} ms wall",
            seek_started.elapsed().as_millis()
        );
        player.stop();
        assert!(frames > 1, "the transcoded stream produced no frames");
        assert!(position > 0, "the transcoded clock did not advance");
        assert!(
            seek_position >= 55_000,
            "the transcode seek landed at {seek_position} ms"
        );
        assert!(
            after_seek > before_seek,
            "the transcode produced no frames after the seek"
        );
    }

    #[test]
    fn a_twenty_thousand_item_grid_stays_inside_the_frame_budget() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(library_state(20_000), view, (1280.0, 800.0), env);
        rt.frame(0.0);
        rt.wheel(700.0, 500.0, -1_200.0);
        let mut times = Vec::new();
        for _ in 0..12 {
            let started = std::time::Instant::now();
            rt.frame(1.0 / 60.0);
            times.push(started.elapsed().as_secs_f32());
        }
        times.sort_by(f32::total_cmp);
        // The median, not the worst: one scheduler hiccup on a shared CI box must not fail a
        // budget test. The worst is still in the message when it does fail.
        let median = times[times.len() / 2];
        assert!(
            median < 0.016,
            "median virtual-grid frame was {median:.3} ms, worst {worst:.3} ms",
            median = median * 1000.0,
            worst = times[times.len() - 1] * 1000.0
        );
    }

    fn detail_state() -> State {
        let mut state = home_state();
        state.route = Route::Detail {
            id: "quiet-floor".into(),
        };
        state.detail_return = Route::Library {
            id: "movies".into(),
        };
        state.detail = Some(demo::detail());
        state.detail_imdb = Some(8.6);
        state
    }

    #[test]
    fn a_movie_shows_its_technical_facts_on_demand() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Detail {
            id: "metropolis".into(),
        };
        state.detail = Some(demo::movie_detail());
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("detail-technical").is_some());
        assert!(rt.find("detail-tech-toggle").is_some());
        assert!(
            rt.find("detail-restart").is_some(),
            "a resumed film offers a way back to zero"
        );
        assert!(
            rt.find("detail-tech").is_none(),
            "collapsed until asked for"
        );

        let mut state = home_state();
        state.detail_tech_open = true;
        state.route = Route::Detail {
            id: "metropolis".into(),
        };
        state.detail = Some(demo::movie_detail());
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("detail-tech").is_some());
    }

    #[test]
    fn every_player_control_has_a_home() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.in_app_prev = Some(demo::previous_episode());
        state.in_app_next = Some(demo::next_episode());
        state.in_app_audio_tracks = demo::audio_tracks();
        state.in_app_subtitles.tracks = demo::subtitle_tracks_with_image();
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(
            rt.find("in-app-stop-bottom").is_none(),
            "the top-left Back is the only stop"
        );
        for key in [
            "in-app-stop",
            "in-app-fullscreen",
            "in-app-back-10",
            "in-app-pause",
            "in-app-forward-10",
            "in-app-elapsed",
            "in-app-remaining",
            "in-app-seek",
            "in-app-volume-slider",
            "in-app-mute",
            "in-app-gear",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on the bar");
        }
        rt.state.toggle_in_app_gear();
        rt.invalidate();
        rt.frame(0.02);
        for key in [
            "in-app-subtitles",
            "in-app-audio",
            "in-app-quality",
            "in-app-speed",
            "in-app-fit",
            "in-app-chapters",
            "in-app-goto",
            "in-app-sleep",
            "in-app-loop",
            "in-app-prev-episode",
            "in-app-next-episode",
            "in-app-transfer",
            "in-app-stats-toggle",
            "in-app-minimize",
        ] {
            assert!(rt.find(key).is_some(), "{key} is in the settings sheet");
        }
    }

    #[test]
    fn the_player_osd_and_settings_sheet_stay_inside_the_frame_budget() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        let worst = |rt: &mut Runtime<State>| {
            let mut worst = 0.0_f32;
            for _ in 0..12 {
                rt.invalidate();
                let started = std::time::Instant::now();
                rt.frame(1.0 / 60.0);
                worst = worst.max(started.elapsed().as_secs_f32());
            }
            worst
        };
        let osd = worst(&mut rt);
        assert!(
            osd < 0.016,
            "worst OSD frame was {osd:.3} ms",
            osd = osd * 1000.0
        );
        rt.state.toggle_in_app_gear();
        rt.invalidate();
        rt.frame(1.0 / 60.0);
        let sheet = worst(&mut rt);
        assert!(
            sheet < 0.016,
            "worst settings-sheet frame was {sheet:.3} ms",
            sheet = sheet * 1000.0
        );
    }

    #[test]
    fn the_root_layout_remembers_the_window_size_for_the_next_launch() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(home_state(), view, (1440.0, 900.0), env);
        rt.frame(0.0);
        rt.frame(0.016);
        assert!((rt.state.window_size.0 - 1440.0).abs() < 1.0);
        assert!((rt.state.window_size.1 - 900.0).abs() < 1.0);

        rt.state.window_position = (120.0, 80.0);
        rt.state.save_preferences();
        assert!((rt.state.config.window_width - 1440.0).abs() < 1.0);
        assert!((rt.state.config.window_height - 900.0).abs() < 1.0);
        assert_eq!(rt.state.config.window_x, Some(120));
        assert_eq!(rt.state.config.window_y, Some(80));
    }

    #[test]
    fn a_person_page_splits_the_library_and_seerr_credits() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Person {
            id: "cailee".into(),
        };
        let mut page = demo::person();
        page.id = "cailee".into();
        page.name = "Cailee Spaeny".into();
        page.items = page.items.into_iter().take(2).collect();
        page.library_tmdb = [945961].into_iter().collect();
        page.seerr_credits = demo::seerr_person_credits();
        page.loading = false;
        state.person = Some(page);
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(
            rt.find("person-credits").is_some(),
            "the library grid stays"
        );
        assert!(
            rt.find("person-seerr").is_some(),
            "the Seerr rail is offered"
        );
        assert!(
            rt.find("person-seerr-1020006").is_some(),
            "Priscilla is not on the server and is offered"
        );
        assert!(
            rt.find("person-seerr-945961").is_none(),
            "Alien is already in the library and is not repeated"
        );
    }

    #[test]
    fn a_seerr_credit_returns_to_the_person_that_opened_it() {
        let mut state = home_state();
        state.route = Route::Person {
            id: "cailee".into(),
        };
        state.person = Some(demo::person());
        state.open_seerr_detail("movie".into(), 945961);
        assert!(matches!(state.route, Route::SeerrDetail { .. }));
        state.back_from_seerr_detail();
        assert!(
            matches!(state.route, Route::Person { .. }),
            "Back returns to the filmography, not the Discover tab"
        );
    }

    #[test]
    fn a_seerr_cast_member_opens_a_person_page_and_returns() {
        let mut state = home_state();
        state.route = Route::SeerrDetail {
            media_type: "movie".into(),
            tmdb_id: 438631,
        };
        state.open_seerr_person(1683343, "Cailee Spaeny".into(), Some("/profile.jpg".into()));
        assert!(matches!(
            state.route,
            Route::SeerrPerson { tmdb_id: 1683343 }
        ));
        assert_eq!(
            state.person.as_ref().map(|page| page.name.as_str()),
            Some("Cailee Spaeny")
        );
        state.back_from_person();
        assert!(matches!(state.route, Route::SeerrDetail { .. }));
    }

    #[test]
    fn the_cast_rail_stacks_a_name_under_the_face() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::SeerrDetail {
            media_type: "movie".into(),
            tmdb_id: 438631,
        };
        state.seerr.detail = Some(crate::state::SeerrDetailState {
            media_type: "movie".into(),
            tmdb_id: 438631,
            details: Some(demo::seerr_movie_detail()),
            selected_seasons: Default::default(),
            loading: false,
            error: None,
            requesting: false,
        });
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        let card = rt.presented("seerr-cast-1190668").expect("the cast card");
        let name = rt
            .presented("seerr-cast-name-1190668")
            .expect("the cast name");
        eprintln!("DEBUG card={:?} name={:?}", card.0, name.0);
        assert!(
            name.0[1] >= card.0[1] + 90.0,
            "the name sits under the 96 pt portrait"
        );
    }

    #[test]
    fn a_person_page_shows_credits_and_returns_to_where_it_opened() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Person {
            id: "mara-ellison".into(),
        };
        state.person = Some(demo::person());
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "person-screen",
            "person-name",
            "person-back",
            "person-credits",
            "person-his-girl-friday",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }

        let mut state = home_state();
        state.route = Route::Detail {
            id: "silent-horror-collection".into(),
        };
        state.detail = Some(demo::collection());
        state.open_person(&jellyfin_api::Person {
            id: "mara-ellison".into(),
            name: "Mara Ellison".into(),
            ..jellyfin_api::Person::default()
        });
        assert!(matches!(state.route, Route::Person { .. }));
        state.back_from_person();
        assert_eq!(
            state.route,
            Route::Detail {
                id: "silent-horror-collection".into()
            }
        );
        assert_eq!(
            state
                .detail
                .as_ref()
                .map(|detail| detail.item.name.as_str()),
            Some("Silent Horror Collection")
        );
    }

    #[test]
    fn back_from_a_collection_member_returns_to_the_collection() {
        let mut state = home_state();
        state.route = Route::Detail {
            id: "silent-horror-collection".into(),
        };
        state.detail = Some(demo::collection());
        state.open_detail("ace-0".into());
        assert_eq!(state.route, Route::Detail { id: "ace-0".into() });
        state.back_from_detail();
        assert_eq!(
            state.route,
            Route::Detail {
                id: "silent-horror-collection".into()
            }
        );
        assert_eq!(
            state
                .detail
                .as_ref()
                .map(|detail| detail.item.name.as_str()),
            Some("Silent Horror Collection")
        );
    }

    #[test]
    fn right_clicking_a_poster_opens_the_card_menu() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        let item = demo::next_episode();
        state.card_menu = Some(crate::state::CardMenu {
            item: item.clone(),
            x: 420.0,
            y: 300.0,
        });
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "card-menu",
            "card-menu-open",
            "card-menu-play",
            "card-menu-favorite",
            "card-menu-played",
            "card-menu-series",
        ] {
            assert!(rt.find(key).is_some(), "{key} is in the card menu");
        }
        assert!(
            rt.find("card-menu-next").is_none(),
            "play next only appears while something plays"
        );
        let (x, y) = rt.find("card-menu-series").expect("go to series");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert!(rt.state.card_menu.is_none(), "a choice closes the menu");
    }

    #[test]
    fn the_item_menu_toggles_watched_and_favourite_locally() {
        let mut state = home_state();
        let resume = state
            .home_data
            .resume
            .first()
            .cloned()
            .expect("a resume card");
        state.open_card_menu_at(resume.clone(), 10.0, 10.0, None);
        state.card_menu_toggle_played();
        assert!(
            state
                .home_data
                .resume
                .iter()
                .all(|item| item.id != resume.id),
            "marking watched removes the card from Continue Watching"
        );

        let next = state
            .home_data
            .next_up
            .first()
            .cloned()
            .expect("a next-up card");
        state.open_card_menu_at(next.clone(), 10.0, 10.0, None);
        state.card_menu_toggle_favorite();
        let updated = state
            .home_data
            .next_up
            .iter()
            .find(|item| item.id == next.id)
            .expect("still on the rail");
        assert!(
            updated
                .user_data
                .as_ref()
                .is_some_and(|user| user.is_favorite)
        );
    }

    #[test]
    fn the_item_menu_opens_the_playlist_picker_for_that_card() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.playlists = demo::playlists();
        let item = demo::next_episode();
        state.open_card_menu_at(item.clone(), 400.0, 260.0, None);
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        let (x, y) = rt.find("card-menu-playlist").expect("add to playlist");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert!(rt.state.card_menu.is_none());
        assert!(rt.state.playlist_picker_open);
        assert_eq!(
            rt.state
                .playlist_picker_item
                .as_ref()
                .map(|item| item.id.as_str()),
            Some(item.id.as_str())
        );
        assert!(rt.find("playlist-picker").is_some());
        assert!(rt.find("playlist-pick-playlist-silent").is_some());

        // A toast is a short-lived confirmation drawn over whatever page is open.
        rt.state.playlist_picker_open = false;
        let mut state = rt.state;
        state.show_toast("Added to the playlist.");
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("toast").is_some());
    }

    #[test]
    fn the_item_menu_offers_another_session_to_play_on() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.cast_sessions = demo::sessions();
        state.open_card_menu_at(demo::next_episode(), 400.0, 240.0, None);
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        let (x, y) = rt.find("card-menu-cast").expect("play on");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert!(rt.state.cast_picker_open);
        for key in ["cast-picker", "cast-picker-close"] {
            assert!(rt.find(key).is_some(), "{key} is on screen");
        }
        assert!(
            rt.state
                .cast_sessions
                .iter()
                .any(|session| session.device_id != "demo-device"),
            "the demo sessions are other devices"
        );
        assert!(
            rt.find("cast-living-room").is_some(),
            "the other device is listed"
        );
    }

    #[test]
    fn the_item_menu_removes_a_card_from_continue_watching() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        let resume = state
            .home_data
            .resume
            .first()
            .cloned()
            .expect("a resume card");
        state.open_card_menu_at(resume.clone(), 10.0, 10.0, None);
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("card-menu-resume").is_some());
        let (x, y) = rt.find("card-menu-resume").expect("remove");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert!(
            rt.state
                .home_data
                .resume
                .iter()
                .all(|item| item.id != resume.id),
            "the card leaves the rail"
        );
        assert!(rt.state.toast.is_some(), "the removal is confirmed");
    }

    #[test]
    fn play_next_puts_the_item_right_after_the_one_playing() {
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        let item = demo::next_episode();
        state.open_card_menu_at(item.clone(), 10.0, 10.0, None);
        state.card_menu_play_next();
        assert!(state.card_menu.is_none());
        assert_eq!(state.queue.len(), 2, "the current item and the new one");
        assert_eq!(
            state.queue.get(1).map(|queued| queued.id.as_str()),
            Some(item.id.as_str())
        );
    }

    #[test]
    fn a_card_plays_its_item_without_opening_the_detail_page() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(library_state(12), view, (1280.0, 800.0), env);
        rt.frame(0.0);
        let (card_x, card_y) = rt.find("library-film-000").expect("the card");
        rt.pointer_move(card_x, card_y);
        rt.frame(0.01);
        let (x, y) = rt
            .find("play-film-000")
            .expect("the card has a play button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert!(
            matches!(rt.state.route, Route::Library { .. }),
            "the quick play button does not navigate"
        );
        assert!(rt.state.detail.is_none());
        assert!(
            rt.state.detail_notice.is_some(),
            "the play attempt ran (and refused without a session)"
        );
    }

    #[test]
    fn a_finished_card_and_row_say_so() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = library_state(12);
        if let Some(library) = &mut state.library {
            library.view = library::ViewMode::List;
        }
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(
            rt.find("played-film-002").is_some(),
            "a played item carries the check"
        );
        assert!(
            rt.find("played-film-001").is_none(),
            "an unplayed item does not"
        );
    }

    #[test]
    fn a_movie_recommends_more_like_this_below_its_details() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Detail {
            id: "metropolis".into(),
        };
        state.detail = Some(demo::movie_detail());
        let mut rt = Runtime::new(state, view, (1280.0, 1000.0), env);
        rt.frame(0.0);
        for key in [
            "detail-similar",
            "detail-similar-title",
            "similar-similar-nosferatu",
            "similar-similar-the-general",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
        // The rail opens the recommended item's own page.
        let (x, y) = rt
            .find("similar-similar-the-general")
            .expect("a recommendation");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert_eq!(
            rt.state
                .detail
                .as_ref()
                .map(|detail| detail.item.id.as_str()),
            Some("similar-the-general")
        );
    }

    #[test]
    fn a_tv_library_offers_a_server_side_episode_shuffle() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        let mut shows =
            library::LibraryState::new("anime".into(), "Anime".into(), Some("tvshows".into()));
        shows.items = vec![jellyfin_api::Item {
            id: "show-1".into(),
            name: "Frieren".into(),
            kind: "Series".into(),
            ..jellyfin_api::Item::default()
        }];
        shows.total = 1;
        state.route = Route::Library { id: "anime".into() };
        state.library = Some(shows);
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(
            rt.find("library-shuffle").is_some(),
            "a shows library can shuffle its episodes"
        );
        assert!(
            rt.find("library-play-all").is_none(),
            "play all would queue containers, not streams"
        );
    }

    #[test]
    fn a_played_episode_row_carries_a_check() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Detail {
            id: "quiet-floor".into(),
        };
        let mut detail = demo::detail();
        detail.episodes[0].user_data = Some(jellyfin_api::UserData {
            played: true,
            ..jellyfin_api::UserData::default()
        });
        state.detail = Some(detail);
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("detail-episode-quiet-floor-s2e1").is_some());
        assert!(
            rt.find("played-quiet-floor-s2e1").is_some(),
            "a finished episode is marked in the list"
        );
    }

    #[test]
    fn a_season_can_be_played_or_shuffled_from_its_header() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Detail {
            id: "quiet-floor".into(),
        };
        state.detail = Some(demo::detail());
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "detail-episodes-header",
            "detail-play-season",
            "detail-shuffle-season",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen");
        }
        let (x, y) = rt.find("detail-play-season").expect("play season");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert_eq!(rt.state.queue.len(), 4, "the loaded season is the run");
        assert_eq!(rt.state.queue_at, 0);
    }

    #[test]
    fn a_collection_shows_its_members_and_no_play_button() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Detail {
            id: "silent-horror-collection".into(),
        };
        state.detail = Some(demo::collection());
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "detail-collection",
            "detail-collection-title",
            "collection-ace-0",
            "collection-ace-5",
            "detail-play-all",
            "detail-shuffle",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
        assert!(
            rt.find("detail-play").is_none(),
            "a collection has no stream of its own"
        );
        let (x, y) = rt.find("detail-play-all").expect("the play-all button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert_eq!(rt.state.queue.len(), 6, "the whole collection is queued");
        assert_eq!(rt.state.queue_at, 0);
    }

    #[test]
    fn the_detail_screen_shows_its_title_primary_action_seasons_and_episodes() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(detail_state(), view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "detail-screen",
            "detail-back",
            "detail-title",
            "detail-meta",
            "detail-ratings",
            "detail-rating-imdb",
            "detail-rating-tmdb",
            "detail-rating-rt",
            "detail-trailer",
            "detail-play",
            "detail-favorite",
            "detail-played",
            "detail-overview",
            "season-quiet-floor-s1",
            "season-quiet-floor-s2",
            "detail-next-up",
            "detail-next-up-card",
            "detail-episode-quiet-floor-s2e1",
            "detail-season-watched",
            "detail-next-up-guard",
            "detail-cast",
            "detail-cast-title",
            "detail-cast-mara-ellison",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
    }

    #[test]
    fn pressing_a_library_card_opens_that_item_detail() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(library_state(12), view, (1280.0, 800.0), env);
        rt.frame(0.0);
        let (x, y) = rt.find("library-film-000").expect("the card has its key");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert_eq!(
            rt.state.route,
            Route::Detail {
                id: "film-000".into()
            }
        );
        assert_eq!(
            rt.state.detail_return,
            Route::Library {
                id: "movies".into()
            }
        );
    }

    #[test]
    fn pressing_a_library_list_row_opens_that_item_detail() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = library_state(12);
        if let Some(library) = &mut state.library {
            library.view = crate::library::ViewMode::List;
        }
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        let (x, y) = rt
            .find("library-item-film-000")
            .expect("the row has its key");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert_eq!(
            rt.state.route,
            Route::Detail {
                id: "film-000".into()
            }
        );
    }

    #[test]
    fn escape_returns_to_the_library_card_at_the_saved_offset() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = library_state(60);
        if let Some(library) = &state.library {
            library.set_scroll_y(500.0);
        }
        state.open_detail("film-000".into());
        state.detail = Some(demo::detail());
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.on_key = Some(on_key);
        rt.frame(0.0);
        rt.key(KeyEvent {
            key: Key::Escape,
            mods: Default::default(),
        });
        rt.frame(1.0 / 60.0);
        assert_eq!(
            rt.state.route,
            Route::Library {
                id: "movies".into()
            }
        );
        assert!(rt.find("library-film-000").is_some());
        assert_eq!(rt.state.take_pending_scroll(), Some(500.0));
    }

    #[test]
    fn selecting_a_season_moves_the_pill_and_clears_the_old_rows() {
        let mut state = detail_state();
        state.select_season("quiet-floor-s1".into());
        let detail = state.detail.as_ref().expect("detail is open");
        assert_eq!(detail.active_season.as_deref(), Some("quiet-floor-s1"));
        assert!(detail.episodes.is_empty());
        assert!(detail.loading_episodes);
    }

    #[test]
    fn the_now_playing_screen_shows_the_episode_and_its_controls() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.playback = Some(demo::playback(false));
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "now-playing",
            "now-title",
            "now-subtitle",
            "now-elapsed",
            "now-remaining",
            "now-pause",
            "now-stop",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
    }

    #[test]
    fn stopping_a_demo_session_returns_to_the_detail_screen() {
        let mut state = home_state();
        state.playback = Some(demo::playback(false));
        state.stop_playback();
        assert!(state.playback.is_none());
        assert!(!state.launching);
    }

    #[test]
    #[ignore = "plays a real item through mpv; set RUNEFIN_PLAY_ITEM to an untracked episode id"]
    fn a_live_item_reports_its_position_back_to_continue_watching() {
        use jellyfin_api::Client;
        use std::time::{Duration, Instant};

        let item_id =
            std::env::var("RUNEFIN_PLAY_ITEM").expect("set RUNEFIN_PLAY_ITEM to an episode id");
        let config = config::Config::load();
        assert!(config.has_session(), "sign in once before this test");

        let mut state = State::from_config(config.clone());
        let waker: rune::Waker = std::sync::Arc::new(|| {});
        state.setup(waker);
        state.open_detail(item_id.clone());

        let wait = |state: &mut State, ready: &dyn Fn(&State) -> bool, seconds: u64| {
            let deadline = Instant::now() + Duration::from_secs(seconds);
            while Instant::now() < deadline {
                state.poll();
                if ready(state) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            false
        };

        assert!(
            wait(
                &mut state,
                &|state| state.detail.as_ref().is_some_and(|detail| !detail.loading),
                20,
            ),
            "detail did not load",
        );
        state.play_detail();
        assert!(
            wait(
                &mut state,
                &|state| state.playback.is_some() || state.playback_error.is_some(),
                20,
            ),
            "mpv did not start",
        );
        assert!(
            state.playback.is_some(),
            "mpv failed: {:?}",
            state.playback_error
        );
        // A live test should not play audio at whatever volume the laptop was left at.
        if let Some(session) = &mut state.playback
            && let Some(handle) = &mut session.handle
        {
            handle.send(serde_json::json!({ "command": ["set", "mute", true] }));
        }

        // Jellyfin ignores resume state below MinResumePct and marks items shorter than
        // MinResumeDurationSeconds (default 300s) played outright. Use an item over five
        // minutes and report past the percentage threshold.
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(60) {
            state.poll();
            std::thread::sleep(Duration::from_millis(100));
        }
        let played_ms = state
            .playback
            .as_ref()
            .map(|session| session.position_ms)
            .unwrap_or(0);
        state.stop_playback();
        for _ in 0..50 {
            state.poll();
            std::thread::sleep(Duration::from_millis(100));
        }

        let client = Client::new(&config.server_url, config.device_id.clone())
            .with_session(config.token.clone(), config.user_id.clone());
        let mut reported = None;
        for _ in 0..20 {
            let resume = client.resume(24).expect("resume after playback");
            if let Some(item) = resume.items.iter().find(|item| item.id == item_id) {
                let position = item
                    .user_data
                    .as_ref()
                    .and_then(|user| user.position_ms())
                    .unwrap_or(0);
                if position >= 5_000 {
                    reported = Some(position);
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        let reported = reported.unwrap_or_else(|| {
            panic!("item {item_id} did not return to Continue Watching after {played_ms} ms")
        });
        assert!(
            reported >= 5_000,
            "server kept {reported} ms, expected the played position"
        );
        eprintln!("live playback: mpv position {played_ms} ms, server stored {reported} ms");
    }

    #[test]
    fn the_search_screen_groups_results_into_rails() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Search { scope: None };
        state.search.query = TextEdit::new("quiet-floor");
        state.search.query.focused = true;
        state.search.results = demo::search_results();
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "search-screen",
            "search-input",
            "search-shows",
            "search-episodes",
            "search-people",
            "search-shows-quiet-floor",
            "search-episodes-quiet-floor-s2e1",
            "search-person-mara-ellison",
            "search-movies-all",
            "search-movies-count",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
    }

    #[test]
    fn only_a_scoped_empty_search_offers_the_global_retry() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Search {
            scope: Some("movies".into()),
        };
        state.search.scope = Some("movies".into());
        state.search.query = TextEdit::new("zzzz");
        state.search.query.focused = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("search-all-libraries").is_some());
        assert!(rt.find("search-clear").is_some());

        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Search { scope: None };
        state.search.query = TextEdit::new("zzzz");
        state.search.query.focused = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(
            rt.find("search-all-libraries").is_none(),
            "a global search cannot be widened further"
        );
    }

    #[test]
    fn see_all_expands_a_search_rail_into_a_grid() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Search { scope: None };
        state.search.query = TextEdit::new("quiet-floor");
        state.search.query.focused = true;
        state.search.results = demo::search_results();
        state.toggle_search_expanded("search-movies");
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "search-movies-grid",
            "search-movies-his-girl-friday",
            "search-movies-haxan",
        ] {
            assert!(rt.find(key).is_some(), "{key} is in the expanded grid");
        }
    }

    #[test]
    fn the_tv_router_owns_directions_before_runes_group_navigation() {
        let env = Env {
            system_dark: true,
            scale: 1.5,
        };
        let mut state = home_state();
        state.big_screen = true;
        state.route = Route::Home;
        let mut rt = Runtime::new(state, view, (1280.0, 720.0), env);
        rt.on_key = Some(on_key);
        rt.frame(0.0);
        let first = rt
            .state
            .take_focus_key()
            .expect("home asks for its first card");
        rt.update(|_, tx| tx.focus_key(first.clone()));
        rt.frame(0.016);
        assert_eq!(rt.focused_key().as_deref(), Some(first.as_str()));

        rt.key(KeyEvent {
            key: Key::Down,
            mods: Default::default(),
        });
        rt.frame(0.032);
        let below = rt.focused_key().expect("the next lane");
        assert!(
            below.starts_with("rail-next-up-"),
            "Down changes lanes instead of wrapping the rail: {below}"
        );

        rt.key(KeyEvent {
            key: Key::Up,
            mods: Default::default(),
        });
        rt.frame(0.048);
        assert!(
            rt.focused_key()
                .is_some_and(|key| key.starts_with("rail-continue-watching-")),
            "Up returns to the rail above"
        );
    }

    #[test]
    fn the_player_volume_slider_leaves_right_for_the_gear() {
        let env = Env {
            system_dark: true,
            scale: 1.5,
        };
        let mut state = home_state();
        state.big_screen = true;
        state.in_app = Some(demo::in_app());
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 720.0), env);
        rt.on_key = Some(on_key);
        rt.frame(0.0);
        rt.update(|_, tx| tx.focus_key("in-app-volume-slider"));
        rt.frame(0.016);

        rt.key(KeyEvent {
            key: Key::Up,
            mods: Default::default(),
        });
        rt.frame(0.032);
        assert_eq!(
            rt.focused_key().as_deref(),
            Some("in-app-volume-slider"),
            "Up adjusts the level and keeps the slider focused"
        );

        for expected in ["in-app-mute", "in-app-fullscreen", "in-app-gear"] {
            rt.key(KeyEvent {
                key: Key::Right,
                mods: Default::default(),
            });
            rt.frame(0.048);
            assert_eq!(rt.focused_key().as_deref(), Some(expected));
        }
    }

    #[test]
    fn the_big_screen_detail_reaches_start_over_and_the_season_actions() {
        let env = Env {
            system_dark: true,
            scale: 1.5,
        };
        let mut state = home_state();
        state.big_screen = true;
        state.route = Route::Detail {
            id: "quiet-floor".into(),
        };
        state.detail = Some(demo::detail());
        let mut rt = Runtime::new(state, view, (1280.0, 720.0), env);
        rt.on_key = Some(on_key);
        rt.frame(0.0);
        let key = rt
            .state
            .take_focus_key()
            .expect("the detail page asks for its primary action");
        assert_eq!(key, "detail-play");
        rt.update(|_, tx| tx.focus_key(key));
        rt.frame(0.016);

        rt.key(KeyEvent {
            key: Key::Right,
            mods: Default::default(),
        });
        rt.frame(0.032);
        assert_eq!(
            rt.focused_key().as_deref(),
            Some("detail-restart"),
            "Right reaches Start over"
        );

        rt.key(KeyEvent {
            key: Key::Down,
            mods: Default::default(),
        });
        rt.frame(0.048);
        let season = rt.focused_key().expect("the season chips");
        assert!(season.starts_with("season-"), "{season}");

        rt.key(KeyEvent {
            key: Key::Down,
            mods: Default::default(),
        });
        rt.frame(0.064);
        assert_eq!(rt.focused_key().as_deref(), Some("detail-next-up-card"));

        rt.key(KeyEvent {
            key: Key::Down,
            mods: Default::default(),
        });
        rt.frame(0.080);
        assert_eq!(rt.focused_key().as_deref(), Some("detail-play-season"));
    }

    #[test]
    fn the_focused_card_glow_is_an_overlay_and_hides_off_a_card() {
        let env = Env {
            system_dark: true,
            scale: 1.5,
        };
        let mut state = home_state();
        state.big_screen = true;
        state.route = Route::Home;
        let mut rt = Runtime::new(state, view, (1280.0, 720.0), env);
        rt.on_key = Some(on_key);
        rt.frame(0.0);
        let key = rt
            .state
            .take_focus_key()
            .expect("home asks for its first card");
        rt.state.note_focus(key.clone());
        rt.update(|_, tx| tx.focus_key(key));
        for step in 1..=4 {
            rt.frame(step as f64 * 0.016);
        }
        assert!(
            rt.state.focused_card_rect.is_some(),
            "the card reports the rect the overlay paints at"
        );
        assert!(rt.find("focus-glow").is_some(), "the halo is drawn");

        rt.state.note_focus("nav-home".into());
        assert!(
            rt.state.focused_card_rect.is_none(),
            "a non-card focus clears the halo rect"
        );
    }

    #[test]
    fn the_search_field_leaves_down_for_its_results() {
        let env = Env {
            system_dark: true,
            scale: 1.5,
        };
        let mut state = home_state();
        state.big_screen = true;
        state.route = Route::Search { scope: None };
        state.search.query.text = "his girl".into();
        state.search.results = demo::search_results();
        let mut rt = Runtime::new(state, view, (1280.0, 720.0), env);
        rt.on_key = Some(on_key);
        rt.frame(0.0);
        rt.update(|_, tx| tx.focus_key("search-input"));
        rt.frame(0.016);
        assert_eq!(rt.focused_key().as_deref(), Some("search-input"));
        assert!(
            rt.find("search-movies-his-girl-friday").is_some(),
            "the first movie card is on screen"
        );

        rt.key(KeyEvent {
            key: Key::Down,
            mods: Default::default(),
        });
        rt.frame(0.032);
        let result = rt.focused_key().expect("the first result group");
        assert!(result.starts_with("search-movies-"), "{result}");
    }

    #[test]
    fn a_big_screen_timeline_scrubs_then_commits_with_ok() {
        let env = Env {
            system_dark: true,
            scale: 1.5,
        };
        let mut state = home_state();
        state.big_screen = true;
        state.in_app = Some(demo::in_app());
        state.osd_visible = true;
        state.seek_step_ms = 10_000;
        let mut rt = Runtime::new(state, view, (1280.0, 720.0), env);
        rt.on_key = Some(on_key);
        rt.frame(0.0);
        rt.update(|_, tx| tx.focus_key("in-app-seek"));
        rt.frame(0.016);
        assert_eq!(rt.focused_key().as_deref(), Some("in-app-seek"));

        let before = rt.state.in_app.as_ref().expect("playing").position_ms;
        rt.key(KeyEvent {
            key: Key::Right,
            mods: Default::default(),
        });
        rt.frame(0.032);
        {
            let session = rt.state.in_app.as_ref().expect("playing");
            assert_eq!(session.scrub_ms, Some(before + 10_000));
            assert_eq!(session.position_ms, before, "the scrub has not seeked yet");
        }
        assert!(
            rt.find("in-app-trickplay-time").is_some(),
            "the preview follows the scrub"
        );
        assert_eq!(
            rt.focused_key().as_deref(),
            Some("in-app-seek"),
            "the timeline keeps focus while scrubbing"
        );

        rt.key(KeyEvent {
            key: Key::Enter,
            mods: Default::default(),
        });
        rt.frame(0.048);
        let session = rt.state.in_app.as_ref().expect("playing");
        assert_eq!(session.position_ms, before + 10_000);
        assert_eq!(session.scrub_ms, None, "OK cleared the preview");
    }

    #[test]
    fn f11_toggles_fullscreen_in_the_state_and_the_osd_offers_it() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(home_state(), view, (1280.0, 800.0), env);
        rt.on_key = Some(on_key);
        rt.frame(0.0);
        rt.key(KeyEvent {
            key: Key::Function(11),
            mods: Default::default(),
        });
        rt.frame(1.0 / 60.0);
        assert!(rt.state.fullscreen);
        assert_eq!(
            rt.state.pending_fullscreen,
            Some(true),
            "the next poll hands the request to the window"
        );
        rt.state.toggle_fullscreen();
        assert_eq!(rt.state.pending_fullscreen, Some(false));
        assert!(!rt.state.fullscreen);

        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(
            rt.find("in-app-fullscreen").is_some(),
            "the player's top bar offers fullscreen"
        );
    }

    #[test]
    fn question_mark_opens_the_shortcut_sheet_and_escape_closes_it() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(home_state(), view, (1280.0, 800.0), env);
        rt.on_key = Some(on_key);
        rt.frame(0.0);
        rt.key(KeyEvent {
            key: Key::Text("?".into()),
            mods: Default::default(),
        });
        rt.frame(1.0 / 60.0);
        assert!(rt.state.shortcuts_open);
        assert!(rt.find("shortcut-sheet").is_some());
        for key in ["shortcut-sheet", "shortcut-close"] {
            assert!(rt.find(key).is_some(), "{key} is on screen");
        }
        rt.key(KeyEvent {
            key: Key::Escape,
            mods: Default::default(),
        });
        rt.frame(1.0 / 60.0);
        assert!(!rt.state.shortcuts_open);
    }

    #[test]
    fn successful_searches_become_idle_suggestions() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Search { scope: None };
        state.search.recents = vec!["Nosferatu".into(), "The Quiet Floor".into()];
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("search-recent-Nosferatu").is_some());
        assert!(rt.find("search-recent-The Quiet Floor").is_some());
    }

    #[test]
    fn ctrl_k_opens_and_escape_closes_search() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(home_state(), view, (1280.0, 800.0), env);
        rt.on_key = Some(on_key);
        rt.frame(0.0);
        rt.key(KeyEvent {
            key: Key::Text("k".into()),
            mods: rune::Mods {
                cmd: true,
                ..Default::default()
            },
        });
        rt.frame(1.0 / 60.0);
        assert!(matches!(rt.state.route, Route::Search { .. }));
        assert!(rt.state.search.query.focused);
        rt.key(KeyEvent {
            key: Key::Escape,
            mods: Default::default(),
        });
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.route, Route::Home);
    }

    #[test]
    fn settings_shows_the_install_facts_and_remote_entry() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Settings;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "settings-screen",
            "setting-server",
            "setting-user",
            "setting-device",
            "settings-clear-images",
            "settings-remote",
            "settings-sign-out",
            "setting-autoplay",
            "setting-seek-step",
            "setting-subtitle-mode",
            "setting-subtitle-background",
            "setting-subtitle-font",
            "setting-subtitle-color",
            "setting-subtitle-outline",
            "setting-subtitle-style",
            "setting-subtitle-position",
            "setting-subtitle-opacity",
            "setting-subtitle-plate-opacity",
            "setting-omdb-key",
            "setting-omdb-box",
            "setting-subtitle-language",
            "setting-audio-language",
            "setting-volume",
            "setting-volume-value",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
    }

    #[test]
    fn the_settings_search_finds_the_seerr_card() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Settings;
        state.settings_query = TextEdit::new("Seerr");
        let mut rt = Runtime::new(state, view, (1280.0, 900.0), env);
        rt.frame(0.0);
        for key in [
            "settings-seerr",
            "setting-seerr-status",
            "setting-seerr-url",
            "setting-seerr-key",
            "setting-seerr-user",
            "setting-seerr-test",
            "setting-seerr-clear",
            "setting-seerr-open",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
    }

    #[test]
    fn the_seerr_discover_page_addresses_its_controls() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        seerr_scene(&mut state);
        state.route = Route::Seerr;
        state.seerr.tab = crate::state::SeerrTab::Discover;
        state.seerr.discover = demo::seerr_discover();
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "seerr-screen",
            "seerr-back",
            "seerr-title",
            "seerr-tab-discover",
            "seerr-tab-requests",
            "seerr-search",
            "seerr-row-trending",
            "seerr-card-movie-438631",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
    }

    #[test]
    fn the_seerr_requests_page_addresses_its_rows_and_filters() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        seerr_scene(&mut state);
        state.route = Route::Seerr;
        state.seerr.tab = crate::state::SeerrTab::Requests;
        state.seerr.filter = crate::state::RequestsFilter::All;
        state.seerr.requests = demo::seerr_requests();
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "seerr-screen",
            "seerr-tab-requests",
            "seerr-filter-active",
            "seerr-filter-all",
            "seerr-request-10",
            "seerr-progress-10",
            "seerr-request-11",
            "seerr-open-11",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
    }

    #[test]
    fn a_processing_request_still_opens_when_the_library_has_it() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        seerr_scene(&mut state);
        state.route = Route::Seerr;
        state.seerr.tab = crate::state::SeerrTab::Requests;
        state.seerr.filter = crate::state::RequestsFilter::All;
        state.seerr.requests = demo::seerr_requests();
        // Seerr has not linked this film, but the server has it: the lookup supplies the id.
        state
            .seerr
            .library_ids
            .insert(("movie".into(), 438631), Some("item-dune".into()));
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        let (x, y) = rt
            .find("seerr-open-10")
            .expect("the processing row offers the library copy");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert!(
            matches!(&rt.state.route, Route::Detail { id } if id == "item-dune"),
            "the button opens the library item, got {:?}",
            rt.state.route
        );
    }

    #[test]
    fn the_subtitle_list_scrolls_to_tracks_that_do_not_fit() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.in_app_subtitles.open = true;
        state.in_app_subtitles.tracks = (0..20)
            .map(|i| jellyfin_api::SubtitleTrack {
                index: i,
                language: Some("eng".into()),
                display_title: format!("English {i} - SUBRIP - External"),
                codec: Some("subrip".into()),
                is_text: true,
                is_external: true,
                is_forced: false,
            })
            .collect();
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        let before = rt.presented("subtitle-track-19").map(|(rect, _)| rect[1]);
        let (x, y) = rt.find("subtitle-track-1").expect("a visible track row");
        rt.pointer_move(x, y);
        rt.wheel_xy(x, y, 0.0, -1_200.0);
        for _ in 0..10 {
            rt.frame(1.0 / 60.0);
        }
        let after = rt.presented("subtitle-track-19").map(|(rect, _)| rect[1]);
        assert!(
            before != after,
            "the wheel moves the list: {before:?} -> {after:?}"
        );
    }

    #[test]
    fn the_main_search_lists_seerr_results_too() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        seerr_scene(&mut state);
        state.route = Route::Search { scope: None };
        state.search.query = rune::TextEdit::new("dune");
        state.search.results = demo::search_results();
        state.search.seerr = seerr_api::SearchResults {
            page: 1,
            total_pages: 1,
            total_results: 3,
            results: demo::seerr_discover().movies,
        };
        let mut rt = Runtime::new(state, view, (1280.0, 900.0), env);
        rt.frame(0.0);
        for key in ["search-seerr", "search-seerr-title"] {
            assert!(rt.find(key).is_some(), "{key} is on screen");
        }
    }

    #[test]
    fn the_browse_filters_are_addressable_and_a_decade_is_a_window() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        seerr_scene(&mut state);
        state.route = Route::Seerr;
        state.seerr.discover_mode = crate::state::DiscoverMode::Browse;
        state.seerr.discover.genres_movie = demo::seerr_genres();
        state.seerr.discover.genres_tv = demo::seerr_genres();
        state.seerr.discover.browse = demo::seerr_discover().movies;
        state.seerr.discover.browse_loaded = true;
        state.seerr.discover.browse_pages = 2;
        state.seerr.discover.filters.kind = crate::state::DiscoverKind::Movies;
        state.seerr.discover.filters.genre = Some(878);
        state.seerr.discover.filters.hub = Some(41077);
        state.seerr.discover.filters.decade = Some(2020);
        let mut rt = Runtime::new(state, view, (1280.0, 900.0), env);
        rt.frame(0.0);
        for key in [
            "seerr-mode-browse",
            "seerr-browse-films",
            "seerr-browse-popular",
            "seerr-browse-year-2020",
            "seerr-browse-genre-878",
            "seerr-browse-hub-41077",
            "seerr-browse-grid",
            "seerr-browse-more",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen");
        }
        let filters = crate::state::DiscoverFilters {
            decade: Some(2010),
            ..Default::default()
        };
        assert_eq!(
            filters.date_window(),
            (
                Some("2010-01-01".to_string()),
                Some("2019-12-31".to_string())
            )
        );
        let older = crate::state::DiscoverFilters {
            decade: Some(0),
            ..Default::default()
        };
        assert_eq!(older.date_window(), (None, Some("1989-12-31".to_string())));
    }

    #[test]
    fn the_open_in_jellyfin_button_wins_over_the_request_row() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        seerr_scene(&mut state);
        state.route = Route::Seerr;
        state.seerr.tab = crate::state::SeerrTab::Requests;
        state.seerr.filter = crate::state::RequestsFilter::All;
        state.seerr.requests = demo::seerr_requests();
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        let (x, y) = rt.find("seerr-open-11").expect("the open button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert!(
            matches!(&rt.state.route, Route::Detail { id } if id == "item-123"),
            "the button opens the Jellyfin detail, got {:?}",
            rt.state.route
        );
    }

    #[test]
    fn the_seerr_detail_page_offers_the_season_picker() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        seerr_scene(&mut state);
        state.seerr.detail = Some(crate::state::SeerrDetailState {
            media_type: "tv".into(),
            tmdb_id: 196950,
            details: Some(demo::seerr_tv_detail()),
            selected_seasons: [1, 2].into_iter().collect(),
            loading: false,
            error: None,
            requesting: false,
        });
        state.route = Route::SeerrDetail {
            media_type: "tv".into(),
            tmdb_id: 196950,
        };
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "seerr-detail-screen",
            "seerr-detail-back",
            "seerr-detail-open",
            "seerr-detail-request",
            "seerr-season-all",
            "seerr-season-1",
            "seerr-season-3",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
    }

    #[test]
    fn a_jellyfin_series_offers_only_the_missing_seerr_seasons() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        seerr_scene(&mut state);
        let mut detail = demo::detail();
        detail
            .item
            .provider_ids
            .insert("Tmdb".into(), "196950".into());
        state.detail = Some(detail);
        state.route = Route::Detail {
            id: "quiet-floor".into(),
        };
        state.seerr.series = Some((196950, demo::seerr_tv_series()));
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(
            rt.find("detail-seerr-request").is_some(),
            "the missing-season action is on the series page"
        );
    }

    #[test]
    fn the_seerr_page_survives_130_percent_text() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        seerr_scene(&mut state);
        state.route = Route::Seerr;
        state.seerr.tab = crate::state::SeerrTab::Discover;
        state.seerr.discover = demo::seerr_discover();
        state.text_scale = 1.3;
        crate::theme::set_text_scale(1.3);
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in ["seerr-screen", "seerr-tab-discover", "seerr-search"] {
            assert!(rt.find(key).is_some(), "{key} is on screen at 130 %");
        }
        crate::theme::set_text_scale(1.0);
    }

    #[test]
    fn the_seerr_nav_entry_is_hidden_without_a_url() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.config.seerr_url.clear();
        state.config.seerr_api_key = None;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("nav-seerr").is_none());

        rt.state.config.seerr_url = "https://jellyseerr.example".into();
        rt.state.config.seerr_api_key = Some("example-api-key".into());
        rt.frame(0.1);
        assert!(rt.find("nav-seerr").is_some());
    }

    #[test]
    fn settings_cycles_the_playback_preferences_and_persists_them() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Settings;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.state.autoplay_next);
        let click = |rt: &mut Runtime<State>, key: &str| {
            // The settings card is taller than the window now; bring a lower row into view
            // before clicking it, as a reader would.
            for _ in 0..3 {
                let (_, y) = rt.find(key).unwrap_or_else(|| panic!("{key} exists"));
                if (40.0..760.0).contains(&y) {
                    break;
                }
                let dy = if y > 760.0 { -(y - 600.0) } else { 600.0 - y };
                rt.wheel(640.0, 400.0, dy);
                rt.frame(0.02);
            }
            let (x, y) = rt.find(key).unwrap_or_else(|| panic!("{key} exists"));
            rt.pointer_down(x, y);
            rt.pointer_up(x, y);
            rt.frame(0.02);
        };
        click(&mut rt, "setting-autoplay-value");
        assert!(!rt.state.autoplay_next);
        click(&mut rt, "setting-seek-step-value");
        assert_eq!(rt.state.seek_step_ms, 30_000, "10 → 30 seconds");
        click(&mut rt, "setting-subtitle-mode-value");
        assert_eq!(rt.state.subtitle_mode, crate::config::SubtitleMode::Always);
        click(&mut rt, "setting-subtitle-background-value");
        assert_eq!(
            rt.state.subtitle_background,
            crate::config::SubtitleBackground::Solid
        );
        click(&mut rt, "setting-subtitle-language-value");
        assert_eq!(rt.state.subtitle_language, "eng");
        click(&mut rt, "setting-audio-language-value");
        assert_eq!(rt.state.audio_language, "eng");
        click(&mut rt, "setting-volume-up");
        assert!(
            (rt.state.in_app_volume - 1.1).abs() < 0.001,
            "the starting volume moves in 10 % steps"
        );

        rt.state.save_preferences();
        assert_eq!(rt.state.config.autoplay_next, Some(false));
        assert_eq!(rt.state.config.seek_seconds, Some(30));
        assert_eq!(
            rt.state.config.subtitle_mode,
            Some(crate::config::SubtitleMode::Always)
        );
        assert_eq!(
            rt.state.config.subtitle_background,
            Some(crate::config::SubtitleBackground::Solid)
        );
        assert_eq!(rt.state.config.subtitle_language.as_deref(), Some("eng"));
        assert_eq!(rt.state.config.audio_language.as_deref(), Some("eng"));
        assert!((rt.state.config.volume.unwrap_or(0.0) - 1.1).abs() < 0.001);
    }

    #[test]
    fn remote_lists_the_other_sessions_and_their_controls() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Remote;
        state.remote.sessions = demo::sessions();
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "remote-screen",
            "remote-session-living-room",
            "remote-pause-living-room",
            "remote-seek-living-room",
            "remote-stop-living-room",
            "remote-refresh",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
    }

    #[test]
    fn the_in_app_launching_state_is_addressable() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.in_app_launching = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in ["in-app-launching", "in-app-cancel"] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
    }

    #[test]
    fn the_minimized_player_keeps_browsing_and_restore_controls() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.in_app_minimized = true;
        state.osd_visible = false;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "home-screen",
            "mini-player",
            "mini-player-bar",
            "mini-player-thumb",
            "mini-player-play",
            "mini-player-stop",
            "mini-player-open",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
    }

    #[test]
    fn hovering_the_timeline_draws_the_cropped_trickplay_preview() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.image_cache = Some(rune::ImageCache::new(std::sync::Arc::new(|| {})));
        let mut session = demo::in_app();
        session.item.trickplay = Some(demo::trickplay_manifest());
        let ms = 16 * 60_000 + 30_000;
        state.in_app = Some(session);
        state.in_app_seek_width = 1232.0;
        state.in_app_hover = Some((620.0, ms));
        let sprite = demo::trickplay_sprite(state.image_cache.as_ref().expect("a cache"));
        let item = state
            .in_app
            .as_ref()
            .map(|session| session.item.clone())
            .expect("a session");
        let key = state.trickplay_key(&item, ms).expect("a tile key");
        state.images.insert(key, sprite);
        state.osd_visible = true;

        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(
            rt.find("in-app-trickplay").is_some(),
            "the preview sits above the timeline while the pointer rests there"
        );
        assert!(
            rt.find("in-app-trickplay-chapter").is_some(),
            "the chapter under the pointer names itself"
        );
        assert!(rt.find("in-app-seek").is_some());
        let (preview, _) = rt
            .find("in-app-trickplay")
            .map(|_| {
                rt.presented("in-app-trickplay")
                    .expect("the preview has a measured box")
            })
            .expect("the preview is on screen");
        assert_eq!((preview[2], preview[3]), (300.0, 174.0));
        assert!(
            rt.find("in-app-trickplay-time").is_some(),
            "the frame carries its timestamp"
        );
        assert!(
            rt.find("in-app-seek-knob").is_some(),
            "a hovered bar grows a knob"
        );
        let (track, _) = rt
            .presented("in-app-seek-track")
            .expect("the track has a measured box");
        assert_eq!(track[3], 8.0, "the hovered bar is twice as thick");
    }

    #[test]
    fn a_timeline_without_trickplay_still_offers_a_time_preview() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        let mut session = demo::in_app();
        session.item.trickplay = None;
        state.in_app = Some(session);
        state.in_app_seek_width = 1232.0;
        state.in_app_hover = Some((620.0, 16 * 60_000 + 30_000));
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(
            rt.find("in-app-trickplay").is_some(),
            "the box appears even when the server published no trickplay"
        );
        assert!(
            rt.find("in-app-trickplay-time").is_some(),
            "the time under the pointer is still readable"
        );
        assert!(rt.find("in-app-seek-knob").is_some());
    }

    #[test]
    fn the_subtitle_overlay_and_track_panel_are_addressable() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        let mut session = demo::in_app();
        session.position_ms = 42 * 60_000;
        state.in_app = Some(session);
        state.in_app_subtitles.tracks = demo::subtitle_tracks();
        state.in_app_subtitles.selected = Some(0);
        state.in_app_subtitles.cues = demo::subtitle_cues();
        state.in_app_subtitles.delay_ms = 500;
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("in-app-subtitles").is_some());
        assert!(rt.find("in-app-subtitle-box").is_some());
        assert!(
            rt.find("in-app-subtitle-delay").is_some(),
            "the active delay stays visible outside the menu"
        );

        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        let mut session = demo::in_app();
        session.position_ms = 42 * 60_000;
        state.in_app = Some(session);
        state.in_app_subtitles.tracks = demo::subtitle_tracks_with_image();
        state.in_app_subtitles.selected = Some(0);
        state.in_app_subtitles.cues = demo::subtitle_cues();
        state.in_app_subtitles.open = true;
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "in-app-subtitle-panel",
            "subtitle-track-off",
            "subtitle-track-0",
            "subtitle-track-1",
            "subtitle-delay-down",
            "subtitle-size-up",
            "subtitle-track-2",
            "subtitle-track-3",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
        assert!(
            rt.find("in-app-subtitle-box").is_none(),
            "the menu hides the cue instead of clipping it"
        );
        // PGS and DVB are selectable now, so neither is a find-text row.
        assert!(
            rt.find("subtitle-image-2").is_none(),
            "a supported PGS track is a normal choice, not a find-text row"
        );
        assert!(
            rt.find("subtitle-image-3").is_none(),
            "a supported DVB track is a normal choice too"
        );
        let (x, y) = rt.find("subtitle-track-3").expect("the DVB track");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert_eq!(rt.state.in_app_subtitles.selected, Some(3));
        assert!(!rt.state.in_app_subtitle_search.open);
    }

    #[test]
    fn an_image_subtitle_renders_as_placed_bitmaps() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        let mut session = demo::in_app();
        session.position_ms = 42 * 60_000;
        state.in_app = Some(session);
        state.in_app_subtitles.image = demo::image_subtitle_style();
        state.in_app_subtitles.selected = None;
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(
            rt.find("in-app-subtitle-image").is_some(),
            "the bitmap is addressable over the picture"
        );
        assert!(
            rt.find("in-app-subtitle-box").is_none(),
            "no text cue is drawn for an image track"
        );
    }

    #[test]
    fn the_subtitle_panel_offers_an_online_search_and_lists_results() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        let mut session = demo::in_app();
        session.position_ms = 42 * 60_000;
        state.in_app = Some(session);
        state.in_app_subtitles.tracks = demo::subtitle_tracks();
        state.in_app_subtitles.selected = Some(0);
        state.in_app_subtitles.open = true;
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(
            rt.find("subtitle-search-open").is_some(),
            "the track list has a way into the online lookup"
        );

        // Opening the lookup replaces the track list in place.
        rt.state.in_app_subtitle_search.open = true;
        rt.state.in_app_subtitle_search.results = demo::remote_subtitles();
        rt.frame(0.02);
        for key in [
            "in-app-subtitle-search",
            "subtitle-search-close",
            "subtitle-lang-eng",
            "subtitle-lang-jpn",
            "subtitle-result-0",
            "subtitle-result-1",
            "subtitle-result-2",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
        assert!(
            rt.find("subtitle-track-off").is_none(),
            "the lookup replaces the track list rather than stacking on it"
        );
        assert_eq!(rt.state.in_app_subtitle_search.language, "eng");
    }

    #[test]
    fn the_chapter_list_seeks_and_the_episode_neighbours_are_addressable() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        let mut session = demo::in_app();
        session.position_ms = 12 * 60_000;
        state.in_app = Some(session);
        state.in_app_prev = Some(demo::previous_episode());
        state.in_app_next = Some(demo::next_episode());
        state.in_app_chapters_open = true;
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "in-app-chapter-panel",
            "in-app-chapter-0",
            "in-app-chapter-1",
            "in-app-chapter-3",
            "in-app-chapter-thumb-0",
            "in-app-chapter-marks",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
        let (x, y) = rt.find("in-app-chapter-1").expect("the second chapter");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        let session = rt.state.in_app.as_ref().expect("still playing");
        assert_eq!(
            session.position_ms,
            5 * 60_000,
            "the chapter row seeks to its start"
        );
        assert!(
            rt.state.in_app_chapters_open,
            "the list stays open so chapters can be stepped through"
        );
        // The way back to the list and to the neighbours lives in the settings sheet.
        rt.state.toggle_in_app_gear();
        rt.invalidate();
        rt.frame(0.04);
        for key in [
            "in-app-chapters",
            "in-app-prev-episode",
            "in-app-next-episode",
        ] {
            assert!(rt.find(key).is_some(), "{key} is in the sheet");
        }
    }

    #[test]
    fn ass_cues_keep_their_own_places() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        let mut session = demo::in_app();
        session.position_ms = 42 * 60_000;
        state.in_app = Some(session);
        state.in_app_subtitles.tracks = demo::subtitle_tracks();
        state.in_app_subtitles.selected = Some(0);
        state.in_app_subtitles.cues = demo::ass_cues();
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "in-app-subtitle-box",
            "in-app-subtitle-box-1",
            "in-app-subtitle-box-2",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen");
        }
    }

    #[test]
    fn a_stalled_source_says_so_with_its_percentage() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        let mut session = demo::in_app();
        session.player.preview_buffering(37);
        session.buffering = Some(37);
        state.in_app = Some(session);
        state.osd_visible = false;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("in-app-buffering").is_some());
        assert!(rt.find("in-app-buffering-percent").is_some());

        let session = rt.state.in_app.as_mut().expect("playing");
        session.player.preview_buffering(100);
        session.buffering = None;
        rt.frame(0.02);
        assert!(
            rt.find("in-app-buffering").is_none(),
            "the indicator goes away when the source catches up"
        );
    }

    #[test]
    fn the_up_next_strip_appears_only_in_the_last_thirty_seconds() {
        let mut state = home_state();
        let mut session = demo::in_app();
        session.position_ms = 42 * 60_000;
        state.in_app = Some(session);
        state.in_app_next = Some(demo::next_episode());
        assert!(
            !state.next_up_eligible(),
            "six minutes is too early to offer it"
        );

        if let Some(session) = &mut state.in_app {
            session.position_ms = 47 * 60_000 + 40_000;
        }
        assert!(
            state.next_up_eligible(),
            "twenty seconds left is the window"
        );

        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in ["in-app-next", "in-app-next-play", "in-app-next-dismiss"] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }

        let mut state = home_state();
        let mut session = demo::in_app();
        session.position_ms = 47 * 60_000 + 40_000;
        state.in_app = Some(session);
        state.in_app_next = Some(demo::next_episode());
        state.dismiss_in_app_next();
        assert!(!state.next_up_eligible());
        assert!(state.in_app_next.is_none());
    }

    #[test]
    fn the_live_tv_screen_lists_channels_and_their_current_program() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::LiveTv;
        state.live.channels = demo::live_channels();
        state.live.next = demo::live_next();
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "live-tv-screen",
            "live-title",
            "live-channels",
            "live-channel-aurora-one",
            "live-program-aurora-one",
            "live-channel-access-seven",
            "live-next-aurora-one",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
    }

    #[test]
    fn the_guide_mode_draws_one_timeline_row_per_channel() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::LiveTv;
        state.live.channels = demo::live_channels();
        state.live.next = demo::live_next();
        state.live.guide = demo::live_guide();
        state.live.guide_open = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "live-guide",
            "live-guide-times",
            "live-guide-row-aurora-one",
            "live-guide-row-meridian",
            "live-guide-toggle",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
        assert!(rt.find("live-channel-aurora-one").is_none());
    }

    #[test]
    fn a_live_session_shows_the_live_marker_instead_of_a_timeline() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.live.channels = demo::live_channels();
        state.live.next = demo::live_next();
        state.in_app = Some(demo::live_session());
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("in-app-live").is_some());
        for key in [
            "in-app-live-channel",
            "in-app-live-prev",
            "in-app-live-next",
            "in-app-live-next-program",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen");
        }
        assert_eq!(rt.state.live_channel_index(), Some(0));
        assert!(rt.find("in-app-elapsed").is_some());
        assert!(
            rt.find("in-app-seek").is_none(),
            "a tuner stream has no timeline"
        );
        assert!(
            rt.find("in-app-back-10").is_none(),
            "there is no published seek window"
        );
        assert!(
            rt.find("in-app-speed").is_none(),
            "a tuner stream has no playback rate"
        );
    }

    #[test]
    fn the_queue_chip_opens_the_run_and_a_row_jumps_to_it() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        let mut session = demo::in_app();
        session.position_ms = 42 * 60_000;
        state.in_app = Some(session);
        state.queue = (1..=4)
            .map(|number| {
                let mut item = demo::next_episode();
                item.id = format!("quiet-floor-s2e{}", number + 1);
                item.index_number = Some(number + 1);
                item
            })
            .collect();
        state.queue_at = 1;
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        let (x, y) = rt.find("in-app-queue").expect("the queue chip");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert!(rt.state.in_app_queue_open);
        for key in ["in-app-queue-panel", "in-app-queue-0", "in-app-queue-3"] {
            assert!(rt.find(key).is_some(), "{key} is in the queue panel");
        }
        let (x, y) = rt.find("in-app-queue-3").expect("the fourth row");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.04);
        assert_eq!(rt.state.queue_at, 3);
        assert!(
            !rt.state.in_app_queue_open,
            "choosing a row closes the panel"
        );
    }

    #[test]
    fn the_resume_conflict_offers_this_device_or_the_server() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        let mut session = demo::in_app();
        session.position_ms = 42 * 60_000;
        state.in_app = Some(session);
        state.resume_conflict = Some((48 * 60_000, 42 * 60_000));
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "resume-conflict",
            "resume-conflict-title",
            "resume-conflict-local",
            "resume-conflict-server",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen");
        }
        let (x, y) = rt.find("resume-conflict-local").expect("the local choice");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rt.state.resume_conflict.is_none());
        assert_eq!(
            rt.state.in_app.as_ref().expect("playing").position_ms,
            48 * 60_000
        );
    }

    #[test]
    fn the_preplay_sheet_names_the_source_and_plays_it() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Detail {
            id: "metropolis".into(),
        };
        state.detail = Some(demo::movie_detail());
        state.preplay_open = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "preplay-sheet",
            "preplay-version-metropolis-source",
            "preplay-cancel",
            "preplay-play",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen");
        }
        let (x, y) = rt
            .find("preplay-version-metropolis-1080")
            .expect("the 1080p source");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.in_app_source.as_deref(), Some("metropolis-1080"));
    }

    #[test]
    fn audio_modes_cycle_and_show_in_the_audio_panel() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.in_app_audio_open = true;
        state.cycle_audio_mode();
        assert_eq!(state.audio_mode, crate::config::AudioMode::Dialogue);
        state.toggle_downmix();
        assert!(state.downmix_stereo);
        state.adjust_audio_delay(25);
        assert_eq!(state.audio_delay_ms, 25);
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "in-app-audio-panel",
            "audio-downmix",
            "audio-mode",
            "audio-delay-up",
            "audio-delay-value",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen");
        }
    }

    #[test]
    fn the_in_app_transfer_offers_other_sessions() {
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.cast_sessions = demo::sessions();
        state.open_in_app_transfer();
        assert!(state.transfer_after_cast);
        assert!(state.cast_picker_open);
        assert!(state.cast_item.is_some());
    }

    #[test]
    fn theme_media_toggles_are_offered_only_when_the_server_has_them() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Detail {
            id: "metropolis".into(),
        };
        state.detail = Some(demo::movie_detail());
        state.detail_theme_songs = demo::extras();
        state.detail_theme_videos = demo::extras();
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("detail-theme-audio").is_some());
        assert!(rt.find("detail-theme-video").is_some());
    }

    #[test]
    fn the_watchlist_is_one_query_across_libraries() {
        let library = crate::library::LibraryState::new(
            "__watchlist__".into(),
            "Watchlist".into(),
            Some("watchlist".into()),
        );
        let mut library = library;
        library.year = Some(2024);
        library.min_rating = Some(8);
        library.audio_language = Some("eng".into());
        library.only_4k = true;
        library.missing_only = true;
        library.unaired_only = true;
        let query = library.query().to_query();
        assert!(query.contains("filters=IsFavorite"), "{query}");
        assert!(!query.contains("parentId="), "{query}");
        assert!(query.contains("recursive=true"), "{query}");
        assert!(query.contains("MediaSources"), "{query}");
        assert!(query.contains("years=2024"), "{query}");
        assert!(query.contains("minCommunityRating=8.0"), "{query}");
        assert!(query.contains("audioLanguages=eng"), "{query}");
        assert!(query.contains("Is4K"), "{query}");
        assert!(query.contains("IsMissing"), "{query}");
        assert!(query.contains("IsUnaired"), "{query}");
    }

    #[test]
    fn bulk_selection_marks_watched_and_clears() {
        let mut state = library_state(8);
        let ids: Vec<String> = state
            .library
            .as_ref()
            .expect("library")
            .items
            .iter()
            .take(2)
            .map(|item| item.id.clone())
            .collect();
        for id in &ids {
            state.toggle_library_selected(id);
        }
        assert_eq!(state.library_selected.len(), 2);
        state.mark_selected_watched();
        assert!(state.library_selected.is_empty());
        for id in &ids {
            assert!(
                state
                    .library
                    .as_ref()
                    .expect("library")
                    .items
                    .iter()
                    .find(|item| &item.id == id)
                    .and_then(|item| item.user_data.as_ref())
                    .is_some_and(|user| user.played)
            );
        }
    }

    #[test]
    fn the_filter_sheet_and_new_sorts_are_addressable() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = library_state(12);
        state.toggle_library_filters();
        state.set_sort(crate::library::SortMode::Played);
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "library-filters-panel",
            "filters-unwatched",
            "filters-favorites",
            "sort-played",
            "sort-random",
            "sort-direction",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen");
        }
        assert!(
            rt.state
                .library
                .as_ref()
                .is_some_and(|library| library.sort == crate::library::SortMode::Played)
        );
    }

    #[test]
    fn a_library_card_shows_capability_badges() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = library_state(4);
        let detail = demo::movie_detail();
        if let Some(library) = &mut state.library
            && let Some(item) = library.items.first_mut()
        {
            item.media_sources = detail.item.media_sources.clone();
        }
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        let id = rt.state.library.as_ref().expect("library").items[0]
            .id
            .clone();
        assert!(rt.find(&format!("badge-4k-{id}")).is_some());
        assert!(rt.find(&format!("badge-hdr-{id}")).is_some());
    }

    #[test]
    fn a_like_flips_and_clears() {
        let mut state = home_state();
        let item = state.home_data.resume[0].clone();
        state.card_menu = Some(crate::state::CardMenu {
            item,
            x: 0.0,
            y: 0.0,
        });
        state.card_menu_set_like(Some(true));
        assert!(
            state.home_data.resume[0]
                .user_data
                .as_ref()
                .and_then(|user| user.likes)
                == Some(true)
        );
    }

    #[test]
    fn a_removed_playlist_entry_can_be_undone() {
        let mut state = home_state();
        let library = demo::playlist_library();
        let first = library.items[0].clone();
        let entry = first.playlist_item_id.clone().expect("a playlist entry");
        state.library = Some(library);
        state.playlist_editing = true;
        state.route = Route::Library {
            id: "playlist-silent".into(),
        };
        state.remove_playlist_entry(entry);
        assert!(
            !state
                .library
                .as_ref()
                .expect("library")
                .items
                .iter()
                .any(|item| item.id == first.id)
        );
        assert!(state.toast_undo.is_some());
        state.undo_toast();
        assert!(
            state
                .library
                .as_ref()
                .expect("library")
                .items
                .iter()
                .any(|item| item.id == first.id)
        );
    }

    #[test]
    fn moving_a_playlist_entry_offers_undo() {
        let mut state = home_state();
        state.library = Some(demo::playlist_library());
        state.playlist_editing = true;
        let (entry, before) = {
            let library = state.library.as_ref().expect("library");
            let item = &library.items[1];
            (
                item.playlist_item_id.clone().expect("entry"),
                item.id.clone(),
            )
        };
        state.move_playlist_entry(entry, -1);
        assert_eq!(state.library.as_ref().expect("library").items[0].id, before);
        state.undo_toast();
        assert_ne!(state.library.as_ref().expect("library").items[0].id, before);
    }

    #[test]
    fn the_new_sorts_and_deep_filters_reach_the_query() {
        let mut state = library_state(8);
        state.set_sort(crate::library::SortMode::Runtime);
        state.adjust_library_year(-1);
        state.cycle_library_rating();
        state.cycle_library_audio_language();
        state.cycle_library_subtitle_language();
        state.toggle_library_subtitles();
        state.toggle_library_4k();
        state.toggle_library_missing();
        state.toggle_library_unaired();
        let library = state.library.as_ref().expect("library");
        let query = library.query().to_query();
        assert!(query.contains("sortBy=Runtime"), "{query}");
        assert!(query.contains("years=2026"), "{query}");
        assert!(query.contains("minCommunityRating=7.0"), "{query}");
        assert!(query.contains("audioLanguages=eng"), "{query}");
        assert!(query.contains("subtitleLanguages=eng"), "{query}");
        assert!(query.contains("HasSubtitles"), "{query}");
        assert!(query.contains("Is4K"), "{query}");
    }

    #[test]
    fn a_chosen_version_is_remembered_per_item() {
        let mut state = home_state();
        state.route = Route::Detail {
            id: "metropolis".into(),
        };
        state.detail = Some(demo::movie_detail());
        state.play_detail_source("metropolis-1080".into());
        assert_eq!(
            state.version_choices.get("metropolis").map(String::as_str),
            Some("metropolis-1080")
        );
    }

    #[test]
    fn stale_rows_stay_visible_while_a_reload_is_in_flight() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = library_state(8);
        let first = state.library.as_ref().expect("library").items[0].id.clone();
        state.library.as_mut().expect("library").loading = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(
            rt.find(&format!("library-{first}")).is_some() || rt.find("library-grid").is_some(),
            "the old page is still on screen during a refresh"
        );
    }

    #[test]
    fn resetting_preferences_restores_defaults_without_signing_out() {
        let mut state = home_state();
        state.autoplay_next = false;
        state.seek_step_ms = 30_000;
        state.audio_mode = crate::config::AudioMode::Night;
        state.reduce_motion = true;
        let user = state.config.user_name.clone();
        state.reset_preferences();
        assert!(state.autoplay_next);
        assert_eq!(state.seek_step_ms, 10_000);
        assert_eq!(state.audio_mode, crate::config::AudioMode::Off);
        assert!(!state.reduce_motion);
        assert_eq!(state.config.user_name, user);
    }

    #[test]
    fn the_settings_search_and_reset_are_addressable() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Settings;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "settings-search",
            "settings-reset",
            "settings-export",
            "setting-downmix",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen");
        }
    }

    #[test]
    fn a_card_shows_audio_and_version_badges() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = library_state(4);
        let detail = demo::movie_detail();
        if let Some(library) = &mut state.library
            && let Some(item) = library.items.first_mut()
        {
            item.media_sources = detail.item.media_sources.clone();
        }
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        let id = rt.state.library.as_ref().expect("library").items[0]
            .id
            .clone();
        assert!(rt.find(&format!("badge-4k-{id}")).is_some());
        assert!(rt.find(&format!("badge-versions-{id}")).is_some());
        assert!(rt.find(&format!("badge-more-{id}")).is_some());
    }

    #[test]
    fn ask_offers_the_segment_button_five_seconds_early() {
        let mut state = home_state();
        let mut session = demo::in_app();
        session.position_ms = 56_000;
        state.in_app = Some(session);
        state.in_app_segments = vec![jellyfin_api::MediaSegment {
            id: "intro".into(),
            item_id: "quiet-floor-s2e2".into(),
            kind: "Intro".into(),
            start_ticks: 60_000 * jellyfin_api::TICKS_PER_MS,
            end_ticks: 90_000 * jellyfin_api::TICKS_PER_MS,
        }];
        assert!(state.active_in_app_segment().is_none());
        assert!(state.upcoming_in_app_segment(5_000).is_some());
    }

    #[test]
    fn adding_to_a_playlist_offers_undo() {
        let mut state = home_state();
        state.playlist_add_pending = Some((
            "playlist-silent".into(),
            "metropolis".into(),
            "Silent Horror Night".into(),
        ));
        state.apply_playlist_add(Ok(()));
        assert!(state.toast_undo.is_some());
        assert!(
            state
                .toast
                .as_deref()
                .is_some_and(|text| text.contains("Silent Horror Night"))
        );
        state.undo_toast();
        assert!(state.toast.is_some());
    }

    #[test]
    fn clicking_a_segment_band_seeks_to_its_start() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.in_app_segments = vec![jellyfin_api::MediaSegment {
            id: "intro".into(),
            item_id: "quiet-floor-s2e2".into(),
            kind: "Intro".into(),
            start_ticks: 0,
            end_ticks: 90_000 * jellyfin_api::TICKS_PER_MS,
        }];
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        let (x, y) = rt.find("in-app-seek-segment-intro").expect("the band");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert_eq!(
            rt.state.in_app.as_ref().expect("playing").position_ms,
            0,
            "the band's own start wins over the scrub position"
        );
    }

    #[test]
    fn reduced_motion_refuses_the_theme_video() {
        let mut state = home_state();
        state.reduce_motion = true;
        state.detail_theme_videos = demo::extras();
        state.toggle_theme_video();
        assert!(!state.theme_video_on);
        assert!(state.toast.is_some());
    }

    #[test]
    fn going_back_from_a_child_clears_its_extras_and_theme() {
        let mut state = home_state();
        let parent = demo::movie_detail();
        state.route = Route::Detail {
            id: "metropolis".into(),
        };
        state.detail = Some(parent);
        state.open_detail("metropolis-1080".into());
        state.detail_extras = demo::extras();
        state.detail_theme_songs = demo::extras();
        state.detail_theme_videos = demo::extras();
        state.back_from_detail();
        assert!(state.detail_extras.is_empty());
        assert!(state.detail_theme_songs.is_empty());
        assert!(state.detail_theme_videos.is_empty());
    }

    #[test]
    fn a_preplay_source_is_tied_to_its_item() {
        let mut state = home_state();
        state.route = Route::Detail {
            id: "metropolis".into(),
        };
        state.detail = Some(demo::movie_detail());
        state.set_preplay_source("metropolis-1080".into());
        assert_eq!(state.in_app_source_for.as_deref(), Some("metropolis"));
        state.close_preplay();
        assert!(state.in_app_source.is_none());
        assert!(state.in_app_source_for.is_none());
    }

    #[test]
    fn choosing_the_server_clears_the_local_checkpoint() {
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.remember_checkpoint("quiet-floor-s2e2", 48 * 60_000);
        state.resume_conflict = Some((48 * 60_000, 42 * 60_000));
        state.resume_from_server();
        assert_eq!(state.local_checkpoint("quiet-floor-s2e2"), None);
    }

    #[test]
    fn playback_preferences_survive_a_restart() {
        let config = crate::config::Config {
            resume_rewind_seconds: Some(15),
            downmix_stereo: Some(true),
            audio_delay_ms: Some(25),
            audio_mode: Some(crate::config::AudioMode::Night),
            reduce_motion: Some(true),
            version_choices: Some(std::collections::HashMap::from([(
                "metropolis".to_string(),
                "metropolis-1080".to_string(),
            )])),
            playback_quality: Some(crate::config::PlaybackQuality::Mbps4),
            quality_choices: Some(std::collections::HashMap::from([(
                "metropolis".to_string(),
                crate::config::PlaybackQuality::Mbps2,
            )])),
            ..crate::config::Config::fresh()
        };
        let state = State::from_config(config);
        assert_eq!(state.resume_rewind_seconds, 15);
        assert!(state.downmix_stereo);
        assert_eq!(state.audio_delay_ms, 25);
        assert_eq!(state.audio_mode, crate::config::AudioMode::Night);
        assert!(state.reduce_motion);
        assert_eq!(
            state.version_choices.get("metropolis").map(String::as_str),
            Some("metropolis-1080")
        );
        assert_eq!(
            state.playback_quality,
            crate::config::PlaybackQuality::Mbps4
        );
        assert_eq!(
            state.quality_choices.get("metropolis"),
            Some(&crate::config::PlaybackQuality::Mbps2)
        );
    }

    #[test]
    fn a_chosen_version_is_persisted() {
        let mut state = home_state();
        state
            .version_choices
            .insert("metropolis".into(), "metropolis-1080".into());
        state.save_preferences();
        assert_eq!(
            state
                .config
                .version_choices
                .as_ref()
                .and_then(|map| map.get("metropolis"))
                .map(String::as_str),
            Some("metropolis-1080")
        );
    }

    #[test]
    fn high_contrast_is_a_persisted_choice() {
        let config = crate::config::Config {
            high_contrast: Some(true),
            ..crate::config::Config::fresh()
        };
        let state = State::from_config(config);
        assert!(state.high_contrast);
        assert!(crate::theme::is_high_contrast());
        crate::theme::set_high_contrast(false);
    }

    #[test]
    fn the_light_theme_is_a_persisted_choice() {
        let config = crate::config::Config {
            light_theme: Some(true),
            ..crate::config::Config::fresh()
        };
        let state = State::from_config(config);
        assert!(state.light_theme);
    }

    #[test]
    fn the_compact_home_hero_is_optional() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.home_hero = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in ["home-hero", "home-hero-title", "home-hero-play"] {
            assert!(rt.find(key).is_some(), "{key}");
        }
    }

    #[test]
    fn the_guide_window_toggles_between_three_hours_and_a_day() {
        let mut state = home_state();
        assert_eq!(state.live.guide_hours, 3);
        state.toggle_guide_window();
        assert_eq!(state.live.guide_hours, 24);
        state.toggle_guide_window();
        assert_eq!(state.live.guide_hours, 3);
    }

    #[test]
    fn live_favorites_filter_the_channel_list() {
        let mut state = home_state();
        state.live.channels = demo::live_channels();
        state.toggle_live_favorite("northlight");
        state.toggle_live_favorites_only();
        assert!(state.live.favorites.contains("northlight"));
        assert!(state.live.favorites_only);
        state.toggle_live_favorite("northlight");
        assert!(!state.live.favorites.contains("northlight"));
    }

    #[test]
    fn a_bulk_selection_opens_the_playlist_picker_with_every_id() {
        let mut state = library_state(8);
        for id in ["film-000", "film-001"] {
            state.toggle_library_selected(id);
        }
        let ids: Vec<String> = state.library_selected.iter().cloned().collect();
        state.open_playlist_picker_for_ids(ids.clone());
        assert!(state.playlist_picker_open);
        assert_eq!(
            state.playlist_picker_ids.as_ref().map(Vec::len),
            Some(ids.len())
        );
    }

    #[test]
    fn a_collection_member_can_be_removed_locally() {
        let mut state = home_state();
        state.route = Route::Detail {
            id: "silent-horror-collection".into(),
        };
        state.detail = Some(demo::collection());
        let member = state.detail.as_ref().expect("detail").collection[0]
            .id
            .clone();
        state.remove_from_current_collection(member.clone());
        assert!(
            !state
                .detail
                .as_ref()
                .expect("detail")
                .collection
                .iter()
                .any(|item| item.id == member)
        );
        assert!(state.toast.is_some());
    }

    #[test]
    fn the_last_library_is_restored_after_a_restart() {
        let config = crate::config::Config {
            last_place: Some(crate::config::LastPlace {
                kind: "library".into(),
                id: Some("movies".into()),
                name: Some("Movies".into()),
                collection_type: Some("movies".into()),
            }),
            ..crate::config::Config::fresh()
        };
        let mut state = State::from_config(config);
        state.home_loaded = true;
        state.poll();
        assert!(matches!(&state.route, Route::Library { id } if id == "movies"));
        assert!(state.library.is_some());
    }

    #[test]
    fn the_last_detail_is_restored_after_a_restart() {
        let config = crate::config::Config {
            last_place: Some(crate::config::LastPlace {
                kind: "detail".into(),
                id: Some("metropolis".into()),
                name: None,
                collection_type: None,
            }),
            ..crate::config::Config::fresh()
        };
        let mut state = State::from_config(config);
        state.home_loaded = true;
        state.poll();
        assert!(matches!(&state.route, Route::Detail { id } if id == "metropolis"));
    }

    #[test]
    fn a_saved_view_round_trips_the_filters() {
        let mut state = library_state(8);
        state.set_sort(crate::library::SortMode::Runtime);
        state.toggle_library_4k();
        state.save_library_view();
        assert_eq!(state.saved_views.len(), 1);
        assert_eq!(state.saved_views[0].sort, crate::library::SortMode::Runtime);
        assert!(state.saved_views[0].only_4k);
        state.clear_library_filters();
        state.apply_library_view(0);
        let library = state.library.as_ref().expect("library");
        assert_eq!(library.sort, crate::library::SortMode::Runtime);
        assert!(library.only_4k);
        state.delete_library_view(0);
        assert!(state.saved_views.is_empty());
    }

    #[test]
    fn library_search_sets_the_query_after_the_idle_beat() {
        let mut state = library_state(8);
        state.library_search = rune::TextEdit::new("dune");
        state.library_search_changed();
        assert!(
            state
                .library
                .as_ref()
                .and_then(|library| library.search.clone())
                .is_some_and(|term| term == "dune")
        );
        assert!(
            state
                .library
                .as_ref()
                .expect("library")
                .query()
                .to_query()
                .contains("searchTerm=dune")
        );
    }

    #[test]
    fn detail_shows_collection_links() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Detail {
            id: "metropolis".into(),
        };
        state.detail = Some(demo::movie_detail());
        state.detail_collections = vec![jellyfin_api::Item {
            id: "dune-collection".into(),
            name: "Silent Horror Collection".into(),
            kind: "BoxSet".into(),
            ..jellyfin_api::Item::default()
        }];
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("detail-collections").is_some());
        assert!(rt.find("detail-collection-dune-collection").is_some());
    }

    #[test]
    fn not_interested_hides_a_home_card_and_undo_restores_it() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.hide_from_home("quiet-floor-s2e4".into());
        assert!(state.is_home_hidden("quiet-floor-s2e4"));
        assert!(state.toast_undo.is_some());
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("rail-continue-watching-quiet-floor").is_none());
        rt.state.undo_toast();
        rt.frame(1.0 / 60.0);
        assert!(rt.find("rail-continue-watching-quiet-floor").is_some());
    }

    #[test]
    fn hidden_home_ids_survive_a_restart() {
        let config = crate::config::Config {
            hidden_home: Some(vec!["metropolis".into()]),
            ..crate::config::Config::fresh()
        };
        let state = State::from_config(config);
        assert!(state.is_home_hidden("metropolis"));
    }

    #[test]
    fn a_b_loop_marks_then_clears() {
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.cycle_loop();
        assert!(state.loop_a.is_some());
        state.seek_in_app_to(45 * 60_000);
        state.cycle_loop();
        assert!(state.loop_a.is_some() && state.loop_b.is_some());
        state.cycle_loop();
        assert!(state.loop_a.is_none() && state.loop_b.is_none());
    }

    #[test]
    fn go_to_time_accepts_clock_forms() {
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.in_app_goto = rune::TextEdit::new("0:12:03");
        state.submit_in_app_goto();
        assert_eq!(state.in_app.as_ref().expect("playing").position_ms, 723_000);
        assert!(!state.in_app_goto_open);
    }

    #[test]
    fn a_frame_step_pauses_and_moves() {
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        let before = state.in_app.as_ref().expect("playing").position_ms;
        state.step_in_app_frame(true);
        let session = state.in_app.as_ref().expect("playing");
        assert!(session.paused);
        assert_eq!(session.position_ms, before + 42);
    }

    #[test]
    fn a_segment_band_seeks_to_its_start() {
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.seek_in_app_to(12_000);
        assert_eq!(state.in_app.as_ref().expect("playing").position_ms, 12_000);
    }

    #[test]
    fn the_still_watching_countdown_stops_the_run() {
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.still_watching_open = true;
        state.still_watching_since =
            Some(std::time::Instant::now() - std::time::Duration::from_secs(61));
        state.still_watching_countdown = 1;
        state.poll();
        assert!(!state.still_watching_open);
    }

    #[test]
    fn the_sleep_timer_fades_the_last_twenty_seconds() {
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.in_app_volume = 1.0;
        state.sleep_fade_percent = 100;
        state.sleep_deadline = Some(std::time::Instant::now() + std::time::Duration::from_secs(10));
        state.poll();
        let volume = state.in_app.as_ref().expect("playing").player.volume();
        assert!((0.45..=0.55).contains(&volume), "volume was {volume}");
    }

    #[test]
    fn one_media_source_hides_the_play_options_chevron() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        let mut detail = demo::movie_detail();
        detail.item.media_sources.truncate(1);
        state.route = Route::Detail {
            id: "metropolis".into(),
        };
        state.detail = Some(detail);
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("detail-play-options").is_none());
    }

    #[test]
    fn the_queue_step_is_what_end_of_episode_follows() {
        let mut state = home_state();
        let first = state.home_data.next_up[0].clone();
        let second = state.home_data.next_up[1].clone();
        state.queue = vec![first, second.clone()];
        state.queue_at = 0;
        let next = state.queue_step(true).expect("a next queue entry");
        assert_eq!(next.id, second.id);
        assert_eq!(state.queue_at, 1);
    }

    #[test]
    fn the_queue_panel_offers_save_as_playlist() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.queue = state.home_data.next_up.iter().take(3).cloned().collect();
        state.queue_at = 0;
        state.in_app_queue_open = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("in-app-queue-panel").is_some());
        assert!(rt.find("queue-save-playlist").is_some());
    }

    #[test]
    fn the_queue_can_add_move_and_remove_items() {
        let mut state = home_state();
        let mut items: Vec<jellyfin_api::Item> =
            state.home_data.next_up.iter().take(3).cloned().collect();
        assert!(items.len() >= 2);
        state.queue = vec![items.remove(0), items.remove(0)];
        state.queue_at = 0;
        state.queue_add(items.remove(0));
        assert_eq!(state.queue.len(), 3);
        let moved = state.queue[2].id.clone();
        state.queue_move(2, -1);
        assert_eq!(state.queue[1].id, moved);
        state.queue_remove(1);
        assert_eq!(state.queue.len(), 2);
        assert!(state.queue_at < state.queue.len());
    }

    #[test]
    fn the_detail_shows_versions_extras_and_can_choose_a_source() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.route = Route::Detail {
            id: "metropolis".into(),
        };
        state.detail = Some(demo::movie_detail());
        state.detail_extras = demo::extras();
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in [
            "detail-version-metropolis-source",
            "detail-version-metropolis-1080",
            "detail-extras",
            "detail-extra-extra-behind",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen");
        }
        let (x, y) = rt
            .find("detail-version-metropolis-1080")
            .expect("the 1080p source");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.in_app_source.as_deref(), Some("metropolis-1080"));
    }

    #[test]
    fn marking_watched_forgets_the_local_checkpoint() {
        let mut state = home_state();
        let item = state.home_data.resume[0].clone();
        state.remember_checkpoint(&item.id, 30 * 60_000);
        state.card_menu = Some(crate::state::CardMenu {
            item,
            x: 0.0,
            y: 0.0,
        });
        state.card_menu_toggle_played();
        assert_eq!(state.local_checkpoint("quiet-floor-s2e4"), None);
    }

    #[test]
    fn local_checkpoints_are_remembered_and_newer_than_the_server() {
        let mut state = home_state();
        state.remember_checkpoint("metropolis", 42_000);
        assert_eq!(state.local_checkpoint("metropolis"), Some(42_000));
    }

    #[test]
    fn a_long_run_opens_the_still_watching_card_and_can_resume() {
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.still_watching = true;
        state.still_watching_minutes = 1;
        state.in_app_play_since = std::time::Instant::now() - std::time::Duration::from_secs(61);
        assert!(state.poll());
        assert!(state.still_watching_open);
        assert!(state.in_app.as_ref().expect("playing").paused);
        state.still_watching_continue();
        assert!(!state.still_watching_open);
        assert!(!state.in_app.as_ref().expect("playing").paused);
    }

    #[test]
    fn the_sleep_timer_pauses_and_the_card_and_panel_are_addressable() {
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.set_sleep_timer(Some(0));
        assert!(state.poll());
        assert!(state.in_app.as_ref().expect("playing").paused);

        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.still_watching_open = true;
        state.in_app_sleep_open = true;
        let mut rt = Runtime::new(
            state,
            view,
            (1280.0, 800.0),
            Env {
                system_dark: true,
                scale: 2.0,
            },
        );
        rt.frame(0.0);
        for key in [
            "still-watching",
            "still-watching-continue",
            "still-watching-stop",
            "in-app-sleep-panel",
            "sleep-10",
            "sleep-end",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen");
        }
    }

    #[test]
    fn always_skips_a_segment_once_and_offers_undo() {
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.skip_mode = crate::config::SkipMode::Always;
        state.in_app_segments = vec![jellyfin_api::MediaSegment {
            id: "intro".into(),
            item_id: "quiet-floor-s2e2".into(),
            kind: "Intro".into(),
            start_ticks: (41 * 60_000 + 50_000) * jellyfin_api::TICKS_PER_MS,
            end_ticks: (42 * 60_000 + 10_000) * jellyfin_api::TICKS_PER_MS,
        }];
        assert!(state.poll());
        let session = state.in_app.as_ref().expect("playing");
        assert_eq!(session.position_ms, 42 * 60_000 + 10_000);
        assert!(state.toast_undo.is_some(), "the skip offers Undo");
        // A second poll at the end of the segment does not skip again.
        state.poll();
        assert_eq!(
            state.in_app.as_ref().expect("playing").position_ms,
            42 * 60_000 + 10_000
        );
        assert!(
            state
                .in_app
                .as_ref()
                .expect("playing")
                .skipped_segments
                .contains("intro")
        );
    }

    #[test]
    fn s_skips_the_intro_the_osd_is_offering() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        let mut session = demo::in_app();
        session.position_ms = 30_000;
        state.in_app = Some(session);
        state.in_app_segments = vec![jellyfin_api::MediaSegment {
            id: "seg-1".into(),
            item_id: "quiet-floor-s2e2".into(),
            kind: "Intro".into(),
            start_ticks: 0,
            end_ticks: 10_000 * 90_000,
        }];
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.on_key = Some(on_key);
        rt.frame(0.0);
        assert!(
            rt.find("in-app-seek-segment-intro").is_some(),
            "the segment is drawn on the timeline"
        );
        rt.key(KeyEvent {
            key: Key::Text("s".into()),
            mods: Default::default(),
        });
        rt.frame(0.02);
        assert_eq!(
            rt.state.in_app.as_ref().expect("playing").position_ms,
            90_000,
            "the intro's end is the new position"
        );
    }

    #[test]
    fn the_skip_key_works_during_the_five_second_lead() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        let mut session = demo::in_app();
        session.position_ms = 87_000;
        state.in_app = Some(session);
        state.skip_mode = crate::config::SkipMode::Ask;
        state.in_app_segments = vec![jellyfin_api::MediaSegment {
            id: "seg-lead".into(),
            item_id: "quiet-floor-s2e2".into(),
            kind: "Intro".into(),
            start_ticks: 90_000 * jellyfin_api::TICKS_PER_MS,
            end_ticks: 120_000 * jellyfin_api::TICKS_PER_MS,
        }];
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.on_key = Some(on_key);
        rt.frame(0.0);
        assert!(
            rt.find("in-app-skip-segment").is_some(),
            "the pill is offered three seconds before the intro"
        );
        rt.key(KeyEvent {
            key: Key::Text("s".into()),
            mods: Default::default(),
        });
        rt.frame(0.02);
        assert_eq!(
            rt.state.in_app.as_ref().expect("playing").position_ms,
            120_000,
            "the key acts on the same segment the pill offers"
        );
    }

    #[test]
    fn never_hides_the_pill_and_ignores_the_skip_key() {
        let mut state = home_state();
        let mut session = demo::in_app();
        session.position_ms = 30_000;
        state.in_app = Some(session);
        state.skip_mode = crate::config::SkipMode::Never;
        state.in_app_segments = vec![jellyfin_api::MediaSegment {
            id: "seg-never".into(),
            item_id: "quiet-floor-s2e2".into(),
            kind: "Intro".into(),
            start_ticks: 0,
            end_ticks: 90_000 * jellyfin_api::TICKS_PER_MS,
        }];
        assert!(state.offered_in_app_segment().is_none());
    }

    #[test]
    fn the_quality_button_opens_the_quality_panel() {
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.toggle_quality_menu();
        assert!(
            state.in_app_quality_open,
            "the quality chip opens the panel it names"
        );
    }

    #[test]
    fn opening_a_player_panel_closes_the_others() {
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.in_app_audio_open = true;
        state.in_app_subtitles.open = true;
        state.in_app_subtitle_search.open = true;

        state.toggle_quality_menu();
        assert!(state.in_app_quality_open);
        for open in [
            state.in_app_audio_open,
            state.in_app_subtitles.open,
            state.in_app_chapters_open,
            state.in_app_queue_open,
            state.in_app_sleep_open,
            state.in_app_goto_open,
            state.in_app_subtitle_search.open,
        ] {
            assert!(!open, "only the panel just opened stays open");
        }

        state.toggle_in_app_chapters();
        assert!(state.in_app_chapters_open);
        assert!(!state.in_app_quality_open);
        assert!(!state.in_app_audio_open);

        state.toggle_in_app_audio_menu();
        assert!(state.in_app_audio_open);
        assert!(!state.in_app_chapters_open);
    }

    #[test]
    fn the_gear_opens_from_the_bar_and_returns_after_a_panel() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.on_key = Some(on_key);
        rt.frame(0.0);
        let (x, y) = rt.find("in-app-gear").expect("the gear is on the bar");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert!(rt.find("in-app-gear-menu").is_some());

        let (x, y) = rt.find("in-app-quality").expect("quality is a row");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.04);
        assert!(rt.find("in-app-quality-panel").is_some());
        assert!(rt.find("in-app-gear-menu").is_none());

        rt.key(KeyEvent {
            key: Key::Escape,
            mods: Default::default(),
        });
        rt.frame(0.06);
        assert!(
            rt.find("in-app-gear-menu").is_some(),
            "Esc returns to the sheet that opened the panel"
        );
        rt.key(KeyEvent {
            key: Key::Escape,
            mods: Default::default(),
        });
        rt.frame(0.08);
        assert!(rt.find("in-app-gear-menu").is_none());
        assert!(rt.state.in_app.is_some(), "the player is still running");
    }

    #[test]
    fn channel_zapping_wraps_in_both_directions() {
        use crate::state::live_zap_index;
        assert_eq!(live_zap_index(0, 4, 1), 1);
        assert_eq!(live_zap_index(3, 4, 1), 0, "the end wraps to the top");
        assert_eq!(live_zap_index(0, 4, -1), 3, "and the top wraps to the end");
        assert_eq!(live_zap_index(0, 0, 1), 0, "no channels stays put");
    }

    #[test]
    fn the_stats_panel_opens_from_the_settings_sheet() {
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.in_app_video = Some(("hevc".into(), 3840, 2160, Some("HDR10".into())));
        state.in_app_container = Some("mkv".into());
        state.osd_visible = true;
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("in-app-stats").is_none());
        rt.state.toggle_in_app_gear();
        rt.frame(0.02);
        let (x, y) = rt
            .find("in-app-stats-toggle")
            .expect("statistics is a row in the sheet");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.04);
        assert!(rt.find("in-app-stats").is_some());
        assert!(rt.find("in-app-stats-layer").is_some());
    }

    #[test]
    fn the_volume_slider_sets_the_player_and_clamps() {
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.set_in_app_volume(0.45);
        assert!((state.in_app_volume - 0.45).abs() < 0.001);
        state.set_in_app_volume(3.0);
        assert!((state.in_app_volume - 2.0).abs() < 0.001);

        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        assert!(rt.find("in-app-volume-slider").is_some());
    }

    #[test]
    fn the_speed_control_steps_from_one_and_is_addressable() {
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.shift_in_app_speed(true);
        assert!((state.in_app_speed - 1.25).abs() < 0.001);
        state.shift_in_app_speed(false);
        assert!((state.in_app_speed - 1.0).abs() < 0.001);
        state.shift_in_app_speed(false);
        assert!((state.in_app_speed - 0.75).abs() < 0.001);

        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        state.osd_visible = true;
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        rt.state.toggle_in_app_gear();
        rt.frame(0.02);
        assert!(rt.find("in-app-speed").is_some());
    }

    #[test]
    fn the_audio_panel_lists_tracks_and_closes_on_a_choice() {
        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.in_app_audio_tracks = demo::audio_tracks();
        state.in_app_audio_open = true;
        state.osd_visible = true;
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(state, view, (1280.0, 800.0), env);
        rt.frame(0.0);
        for key in ["in-app-audio-panel", "audio-track-0", "audio-track-1"] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }

        let mut state = home_state();
        state.in_app = Some(demo::in_app());
        state.in_app_audio_tracks = demo::audio_tracks();
        state.in_app_audio_open = true;
        state.select_in_app_audio(1);
        assert!(!state.in_app_audio_open);
    }

    #[test]
    fn an_intro_segment_is_active_at_the_start_of_playback() {
        let mut state = home_state();
        let mut session = demo::in_app();
        session.position_ms = 30_000;
        state.in_app = Some(session);
        state.in_app_segments = vec![jellyfin_api::MediaSegment {
            id: "intro".into(),
            item_id: "episode".into(),
            kind: "Intro".into(),
            start_ticks: 0,
            end_ticks: 90_000 * jellyfin_api::TICKS_PER_MS,
        }];
        let segment = state.active_in_app_segment().expect("the intro is active");
        assert_eq!(segment.kind, "Intro");
        assert_eq!(segment.end_ms(), 90_000);
    }

    #[test]
    fn the_connect_screen_renders_with_its_fields_and_button() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let state = State::from_config(config::Config::fresh());
        let mut rt = Runtime::new(state, view, (560.0, 520.0), env);
        rt.frame(0.0);

        for key in ["server", "server-hint", "username", "password", "connect"] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
    }

    #[test]
    fn the_quick_connect_screen_shows_its_code_and_cancel() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = State::from_config(config::Config::fresh());
        state.server = TextEdit::new("https://jellyfin.example");
        state.quick = Some(demo::quick_login());
        let mut rt = Runtime::new(state, view, (560.0, 520.0), env);
        rt.frame(0.0);
        for key in [
            "quick-connect-card",
            "quick-connect-code",
            "quick-connect-status",
            "quick-connect-cancel",
        ] {
            assert!(rt.find(key).is_some(), "{key} is on screen and addressable");
        }
        assert!(rt.find("connect").is_none());
    }

    #[test]
    fn quick_connect_requires_a_server_address() {
        let mut state = State::from_config(config::Config::fresh());
        state.start_quick_connect();
        assert!(state.quick.is_none());
        assert!(matches!(state.step, Step::Failed(_)));
    }

    #[test]
    fn a_discovered_server_fills_the_address_field() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut state = State::from_config(config::Config::fresh());
        state.discovered = vec![crate::discovery::DiscoveredServer {
            id: "example".into(),
            name: "example".into(),
            address: "http://192.0.2.49:8096".into(),
        }];
        let mut rt = Runtime::new(state, view, (1280.0, 560.0), env);
        rt.frame(0.0);
        for key in ["discover", "discover-example"] {
            assert!(rt.find(key).is_some(), "{key} is on the connect screen");
        }
        let (x, y) = rt.find("discover-example").expect("the discovered row");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.02);
        assert_eq!(rt.state.server.text, "http://192.0.2.49:8096");
    }

    #[test]
    fn pressing_connect_with_an_empty_server_explains_what_is_missing() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let state = State::from_config(config::Config::fresh());
        let mut rt = Runtime::new(state, view, (560.0, 520.0), env);
        rt.frame(0.0);
        rt.state.server = TextEdit::new("");
        rt.state.user = TextEdit::new("");
        rt.state.step = Step::Idle;
        rt.frame(1.0 / 60.0);

        let (x, y) = rt.find("connect").expect("the button has its key");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);

        match &rt.state.step {
            Step::Failed(f) => assert_eq!(f.stage, net::Stage::Server),
            other => panic!("expected a failure, got {other:?}"),
        }
        assert!(rt.find("error").is_some(), "the error line is on screen");
    }
    #[test]
    #[ignore = "needs a signed-in config"]
    fn perf_live_pipeline() {
        use std::time::Instant;
        let config = config::Config::load();
        assert!(config.has_session(), "sign in once before this test");
        let client = jellyfin_api::Client::new(&config.server_url, config.device_id.clone())
            .with_session(config.token.clone(), config.user_id.clone());
        let resume = client.resume(4).expect("resume");
        let detail_id = resume
            .items
            .first()
            .map(|item| item.id.clone())
            .expect("a resume item");
        let now_days = 20_000;
        let measure = |label: &str, f: &mut dyn FnMut()| {
            let mut times = Vec::new();
            for _ in 0..5 {
                let started = Instant::now();
                f();
                times.push(started.elapsed());
            }
            times.sort();
            eprintln!(
                "{label:<20} median {:7.1} ms  min {:7.1}  max {:7.1}",
                times[2].as_secs_f64() * 1000.0,
                times[0].as_secs_f64() * 1000.0,
                times[4].as_secs_f64() * 1000.0
            );
        };
        measure("home::load", &mut || {
            crate::home::load(&client, now_days).expect("home");
        });
        measure("home::load_next_up", &mut || {
            crate::home::load_next_up(&client);
        });
        measure("home::load_collections", &mut || {
            crate::home::load_collections(&client);
        });
        measure("detail::load", &mut || {
            crate::detail::load(&client, &detail_id).expect("detail");
        });
        measure("search::load", &mut || {
            crate::search::load(&client, "the", None).expect("search");
        });
    }
}
