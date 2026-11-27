//! Screens. Each is a pure function of `State`: no I/O, no state mutation outside a handler
//! closure, so a headless test can drive them and a snapshot can freeze them.

use jellyfin_api::Item;
use rune::tok;

use crate::theme::FocusVisual;
use crate::theme::SurfaceStyle;
use crate::theme::{self, type_};
use rune::{
    Color, Cx, El, GrowthSpec, Icon, ScrollSpec, SurfaceFit, TextEdit, TextStyle, col, el, icon,
    row, text, text_input,
};

use crate::state::{Route, State, Step};

mod detail;
mod downloads;
mod home_rails;
mod hub;
mod in_app;
mod keyboard;
mod library;
mod live;
mod now_playing;
mod person;
mod remote;
mod search;
mod seerr;
mod settings;

/// THE card geometry: a 2:3 poster at the design's 180 pt width, rounded to whole points.
pub(super) const CARD_W: f32 = theme::CARD_W;
pub(super) const CARD_H: f32 = theme::CARD_H;
pub(super) const CARD_GAP: f32 = theme::CARD_GAP;
pub(super) const RESUME_W: f32 = 174.0;
pub(super) const RESUME_H: f32 = 98.0;
pub(super) fn hero_style() -> TextStyle {
    // A page title, not a poster line: 40/650 with the tracking large type needs, and a line
    // height the subtitle can sit under without a gap.
    TextStyle::new(46.0, 730.0).tracking(-1.3).lh(46.0)
}
pub(super) fn subtitle_style() -> TextStyle {
    type_::body()
}

/// One authored page transition. The key changes with the route, so Rune fades the old page
/// out and the new one in as a group; `instant_changes` keeps every child from fading on its
/// own, and a matched poster/portrait escapes it with `animated_changes`.
fn route_key(s: &State) -> String {
    match &s.route {
        Route::Home => s
            .shelf
            .map_or_else(|| "home".to_string(), |shelf| format!("shelf-{shelf:?}")),
        Route::Libraries => "libraries".into(),
        Route::Library { id } => format!("library-{id}"),
        Route::Detail { id } => format!("detail-{id}"),
        Route::Person { id } => format!("person-{id}"),
        Route::SeerrPerson { tmdb_id } => format!("seerr-person-{tmdb_id}"),
        Route::Search { scope } => format!("search-{}", scope.as_deref().unwrap_or("all")),
        Route::Remote => "remote".into(),
        Route::Downloads => "downloads".into(),
        Route::LiveTv => "live".into(),
        Route::Settings => "settings".into(),
        Route::Seerr => "seerr".into(),
        Route::SeerrDetail {
            media_type,
            tmdb_id,
        } => format!("seerr-{media_type}-{tmdb_id}"),
    }
}

pub(super) fn page_transition(s: &State, base: El) -> El {
    el().key(format!("page-{}", route_key(s)))
        .w_full()
        .h_full()
        .transition(
            rune::Transition::opacity()
                .combined(rune::Transition::offset(
                    0.0,
                    if s.nav_delta < 0 {
                        -6.0
                    } else if s.nav_delta > 0 {
                        8.0
                    } else {
                        0.0
                    },
                ))
                .springs(theme::motion::PAGE),
        )
        .instant_changes()
        .child(base)
}

pub fn view(s: &State, _cx: &mut Cx<State>) -> El {
    let base = if !s.is_signed_in() {
        connect(s)
    } else if s.in_app.is_some() && !s.in_app_minimized {
        in_app::in_app_screen(s)
    } else if s.in_app_launching {
        in_app::launching_screen(s)
    } else if s.launching || s.playback.is_some() {
        now_playing::now_playing(s)
    } else if s.route == Route::Home
        && let Some(shelf) = s.shelf
    {
        shelf_screen(s, shelf)
    } else {
        match &s.route {
            Route::Library { .. } => match &s.library {
                Some(library) => library::library_screen(s, library),
                None => home(s),
            },
            Route::Detail { .. } => match &s.detail {
                Some(detail) => detail::detail_screen(s, detail),
                None => home(s),
            },
            Route::Person { .. } | Route::SeerrPerson { .. } => match &s.person {
                Some(person) => person::person_screen(s, person),
                None => home(s),
            },
            Route::Search { .. } => search::search_screen(s),
            Route::Remote => remote::remote_screen(s),
            Route::Downloads => downloads::downloads_screen(s),
            Route::LiveTv => live::live_tv_screen(s),
            Route::Settings => settings::settings_screen(s),
            Route::Seerr => seerr::seerr_screen(s),
            Route::SeerrDetail { .. } => seerr::seerr_detail_screen(s),
            Route::Libraries => hub::libraries_screen(s),
            Route::Home => home(s),
        }
    };
    let mut base = page_transition(s, base);
    if s.in_app_minimized {
        base = el()
            .key("mini-player-layer")
            .w_full()
            .h_full()
            .child(base)
            .child(mini_player(s));
    }
    let base = with_rail(s, base);
    let base = if s.home_rails_open {
        base.child(home_rails::home_rails_sheet(s))
    } else {
        base
    };
    let base = if s.shortcuts_open {
        el().key("shortcut-layer")
            .w_full()
            .h_full()
            .child(base)
            .child(shortcut_overlay(s))
    } else {
        base
    };
    let base = if s.card_menu.is_some() {
        base.child(card_menu(s))
    } else {
        base
    };
    // The picker's item is the card that opened it, or the detail page it belongs to.
    let playlist_item = s
        .playlist_picker_item
        .as_ref()
        .or_else(|| s.detail.as_ref().map(|detail| &detail.item));
    let base = if s.playlist_picker_open {
        match playlist_item {
            Some(item) => base.child(detail::playlist_picker(s, item, &theme::colors())),
            None => base,
        }
    } else {
        base
    };
    let collection_item = s
        .collection_picker_item
        .as_ref()
        .or_else(|| s.detail.as_ref().map(|detail| &detail.item));
    let base = if s.collection_picker_open {
        base.child(detail::collection_picker(
            s,
            collection_item,
            &theme::colors(),
        ))
    } else {
        base
    };
    let base = if s.cast_picker_open {
        base.child(cast_picker(s, &theme::colors()))
    } else {
        base
    };
    let base = if s.playlist_create_open {
        match playlist_item {
            Some(item) => base.child(detail::playlist_create_modal(s, item, &theme::colors())),
            None => base,
        }
    } else {
        base
    };
    let base = match s.toast.message.as_ref() {
        Some(message) => base.child(
            el().key("toast")
                .absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .justify_center()
                .items_end()
                .pb(36.0)
                .child(
                    row()
                        .padding(12.0)
                        .px(18.0)
                        .radius(12.0)
                        .transition(
                            rune::Transition::offset(0.0, 12.0)
                                .combined(rune::Transition::opacity())
                                .springs(theme::motion::TOAST_PAIR),
                        )
                        .motion(theme::motion::TOAST)
                        .backdrop_blur(14.0)
                        .fill(theme::colors().card.alpha(0.68))
                        .gradient(Color::WHITE.alpha(0.06), Color::TRANSPARENT)
                        .outline(1.0, 0.0, Color::WHITE.alpha(0.10))
                        .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
                        .shadow(4.0, 14.0, Color::BLACK.alpha(0.28))
                        .child(
                            text(message.clone())
                                .font(type_::label())
                                .color(theme::colors().text),
                        )
                        .children(s.toast.undo.as_ref().map(|_| {
                            el().key("toast-undo")
                                .role("button")
                                .h(28.0)
                                .px(10.0)
                                .radius(7.0)
                                .center()
                                .fill(crate::theme::accent().alpha(0.16))
                                .hover(theme::colors().hover)
                                .motion(crate::theme::motion::CONTROL)
                                .focusable(true)
                                .focus_visual(2.0)
                                .press_scale(0.96)
                                .child(
                                    text("Undo")
                                        .font(type_::label())
                                        .color(theme::colors().text),
                                )
                                .cursor(rune::Cursor::Pointer)
                                .on_press(|s: &mut State| s.undo_toast())
                        })),
                ),
        ),
        None => base,
    };
    let base = if s.keyboard.open {
        base.child(keyboard::keyboard(s))
    } else {
        base
    };
    // A TV has no pointer; hide it where no control asks for one, and keep the hand over
    // buttons for the occasional mouse.
    base.cursor(if s.big_screen {
        rune::Cursor::Hidden
    } else {
        rune::Cursor::Default
    })
}

fn mini_player(s: &State) -> El {
    let Some(session) = &s.in_app else {
        return el();
    };
    let t = theme::colors();
    let title = session
        .item
        .series_name
        .clone()
        .unwrap_or_else(|| session.item.name.clone());
    let subtitle = session
        .item
        .episode_label()
        .map(|label| format!("{label} · {}", session.item.name))
        .unwrap_or_else(|| session.item.name.clone());
    let progress = session.duration_ms.map_or(0.0, |duration| {
        if duration > 0 {
            (session.position_ms as f32 / duration as f32).clamp(0.0, 1.0)
        } else {
            0.0
        }
    });
    let bar = row()
        .key("mini-player-bar")
        .w_full()
        .h(96.0)
        .padding(12.0)
        .gap(14.0)
        .items_center()
        .fill(t.card)
        .outline(1.0, 0.0, t.card_border)
        .shadow(0.0, 18.0, t.shadow_soft)
        .child(
            el().key("mini-player-thumb")
                .role("button")
                .w(128.0)
                .h(72.0)
                .radius(8.0)
                .clip_rounded(8.0)
                .fill(Color::BLACK)
                .hover(t.hover)
                .motion(crate::theme::motion::CONTROL)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.98)
                .label("Restore the player")
                .child(
                    el().w_full()
                        .h_full()
                        .native_surface(session.player.surface(), SurfaceFit::Fit),
                )
                .cursor(rune::Cursor::Pointer)
                .on_press(|s: &mut State| s.restore_in_app()),
        )
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(3.0)
                .child(
                    text(title)
                        .font(type_::card_title())
                        .color(t.text)
                        .truncate(),
                )
                .child(
                    text(subtitle)
                        .font(type_::mono_caption())
                        .color(t.text_secondary)
                        .truncate(),
                )
                .child(
                    el().w_full()
                        .h(3.0)
                        .radius(1.5)
                        .fill(t.control_track)
                        .child(
                            el().w_full().h(3.0).radius(1.5).child(
                                el().grow(progress.max(0.001))
                                    .h(3.0)
                                    .fill(crate::theme::accent()),
                            ),
                        ),
                ),
        )
        .child(mini_button(
            if session.paused {
                &theme::icon::PLAY
            } else {
                &theme::icon::PAUSE
            },
            "Play / pause",
            "mini-player-play",
            &t,
            |s: &mut State| s.toggle_in_app_pause(),
        ))
        .child(mini_button(
            &theme::icon::STOP,
            "Stop",
            "mini-player-stop",
            &t,
            |s: &mut State| s.stop_in_app(),
        ))
        .child(mini_button(
            &theme::icon::CHEVRON_RIGHT,
            "Open player",
            "mini-player-open",
            &t,
            |s: &mut State| s.restore_in_app(),
        ));
    col()
        .key("mini-player")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .justify_end()
        .child(bar)
}

fn mini_button(
    glyph: &'static Icon,
    label: &'static str,
    key: &str,
    t: &tok::Theme,
    on: impl Fn(&mut State) + 'static,
) -> El {
    el().key(key)
        .role("button")
        .w(40.0)
        .h(40.0)
        .radius(9.0)
        .center()
        .fill(t.bg)
        .hover(t.hover)
        .motion(crate::theme::motion::CONTROL)
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.94)
        .cursor(rune::Cursor::Pointer)
        .label(label)
        .help(label)
        .child(icon(glyph, 17.0).color(t.text))
        .on_press(move |s: &mut State| on(s))
}

fn shortcut_overlay(s: &State) -> El {
    let t = theme::colors();
    let binding = |action| crate::shortcuts::display(s.key_bindings.get(action));
    let rows = vec![
        ("?".to_string(), "Show this shortcut sheet".to_string()),
        (
            binding(crate::shortcuts::Action::PlayPause),
            "Play or pause".to_string(),
        ),
        ("← / →".to_string(), "Seek 10 seconds".to_string()),
        (binding(crate::shortcuts::Action::Mute), "Mute".to_string()),
        (
            binding(crate::shortcuts::Action::Subtitles),
            "Cycle subtitles".to_string(),
        ),
        (
            binding(crate::shortcuts::Action::Skip),
            "Skip intro / credits".to_string(),
        ),
        ("[ / ]".to_string(), "Subtitle delay".to_string()),
        ("- / =".to_string(), "Subtitle size".to_string()),
        ("> / <".to_string(), "Playback speed".to_string()),
        (
            format!(
                "{} / {}",
                binding(crate::shortcuts::Action::NextEpisode),
                binding(crate::shortcuts::Action::PreviousEpisode)
            ),
            "Next / previous episode".to_string(),
        ),
        (
            "PgUp / PgDn".to_string(),
            "Live channel up / down".to_string(),
        ),
        (
            binding(crate::shortcuts::Action::Fit),
            "Fit / fill the picture".to_string(),
        ),
        (
            binding(crate::shortcuts::Action::Fullscreen),
            "Fullscreen".to_string(),
        ),
        ("↑ / ↓".to_string(), "Volume".to_string()),
        (
            "Menu / ⇧F10".to_string(),
            "Open a poster's item menu".to_string(),
        ),
        ("Esc".to_string(), "Back, close or stop".to_string()),
        ("Ctrl K".to_string(), "Search the whole server".to_string()),
        ("Ctrl F".to_string(), "Search this library".to_string()),
    ];
    // The sheet is taller than a short window. Cap it and scroll the rows so the title and
    // the close button stay reachable instead of drawing above the top edge.
    let window_h = if s.window_size.1 >= 1.0 {
        s.window_size.1
    } else {
        800.0
    };
    let max_h = (window_h - 96.0).clamp(320.0, 760.0);
    let mut card = col()
        .key("shortcut-sheet")
        .w(if s.big_screen { theme::dialog::SHORTCUTS_BIG } else { theme::dialog::SHORTCUTS })
        .h(max_h)
        .padding(24.0)
        .gap(14.0)
        .radius(16.0)
        .child(
            row()
                .w_full()
                .gap(12.0)
                .items_center()
                .child(
                    text(if s.big_screen {
                        "Controls"
                    } else {
                        "Keyboard shortcuts"
                    })
                        .font(type_::section())
                        .color(t.text)
                        .grow(1.0),
                )
                .child(
                    el().key("shortcut-close")
                        .role("button")
                        .h(30.0)
                        .px(10.0)
                        .radius(7.0)
                        .center()
                        .fill(t.bg)
                        .hover(t.hover)
                        .motion(crate::theme::motion::CONTROL)
                        .focusable(true)
                        .focus_visual(2.0)
                        .press_scale(0.94)
                        .child(
                            text(if s.big_screen { "B / Esc" } else { "Esc" })
                                .font(type_::mono_caption())
                                .color(t.text_secondary),
                        )
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| s.close_shortcuts()),
                ),
        )
        .child(
            text("These work everywhere; player keys work while a video is open.")
                .font(type_::meta())
                .color(t.text_secondary),
        )
        .children(s.big_screen.then(|| {
            text("Controller: D-pad moves, A selects, B goes back, LB/RB change section, Start opens a card menu or the player gear.")
                .key("shortcut-controller")
                .w_full()
                .max_lines(2)
                .font(type_::meta())
                .color(t.text_secondary)
        }));
    let mut list = col()
        .key("shortcut-list")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(10.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .scroll_edge_fade(12.0);
    for (keys, action) in rows {
        list = list.child(
            row()
                .w_full()
                .gap(16.0)
                .items_center()
                .child(
                    el().w(if s.big_screen { 120.0 } else { 94.0 })
                        .h(28.0)
                        .radius(7.0)
                        .center()
                        .fill(t.bg)
                        .outline(1.0, 0.0, t.card_border)
                        .child(text(keys).font(type_::mono_caption()).color(t.text)),
                )
                .child(text(action).font(type_::body()).color(t.text_secondary)),
        );
    }
    card = card.child(list);
    el().key("shortcut-scrim")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .fill(Color::BLACK.alpha(0.58))
        .center()
        .child(theme::elevation::frosted(card, &t))
}

fn connect(s: &State) -> El {
    let t = theme::colors();
    let title = el().key("connect-logo").w(300.0).h(122.0).child(
        el().w_full()
            .h_full()
            .pixels_fit(theme::logo(), SurfaceFit::Fit),
    );
    let subtitle = text("Sign in to your Jellyfin server")
        .font(type_::body())
        .color(t.text_secondary);

    let server = field(
        "Server",
        &s.server,
        "server",
        FieldOptions {
            placeholder: "https://jellyfin.example",
            hint: Some("The address you open in a browser, including http:// or https://."),
            action: Some(discover_action(s, &t)),
            ..FieldOptions::default()
        },
        |s: &mut State| &mut s.server,
    );
    let user = field(
        "Username",
        &s.user,
        "username",
        FieldOptions {
            placeholder: "Username",
            ..FieldOptions::default()
        },
        |s: &mut State| &mut s.user,
    );
    let password = field(
        "Password",
        &s.password,
        "password",
        FieldOptions {
            secure: true,
            placeholder: "Password",
            ..FieldOptions::default()
        },
        |s: &mut State| &mut s.password,
    );

    let label = match &s.step {
        Step::Idle | Step::Failed(_) => "Connect",
        Step::SigningIn => "Connecting…",
        Step::Ready { .. } => "Connect",
    };
    let connect = el()
        .key("connect")
        .role("button")
        .h(38.0)
        .radius(10.0)
        .fill(crate::theme::accent())
        .gradient(Color::WHITE.alpha(0.18), Color::TRANSPARENT)
        .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
        .shadow(4.0, 12.0, crate::theme::accent().alpha(0.35))
        .hover(t.hover)
        .motion(crate::theme::motion::CONTROL)
        .center()
        .focusable(true)
        .default_button()
        .focus_visual(2.0)
        .press_scale(0.97)
        .child(text(label).font(type_::label()).color(theme::on_accent()))
        .cursor(rune::Cursor::Pointer)
        .on_press(|s: &mut State| {
            s.connect();
        });

    // One line, one place: whichever half failed, the message says which.
    let status = match &s.step {
        Step::Failed(failure) => Some(
            text(failure.message.clone())
                .key("error")
                .font(type_::label())
                // accent(2) is the red end of the six-accent ramp: an error, not a brand colour.
                .color(theme::status::danger()),
        ),
        Step::SigningIn => Some(
            text("Checking the server…")
                .key("status")
                .font(type_::label())
                .color(t.text_secondary),
        ),
        _ => None,
    };

    let mut form = col()
        .key("connect-card")
        .w(420.0)
        .gap(14.0)
        .child(title)
        .child(subtitle);
    if let Some(quick) = &s.quick {
        form = form.child(quick_card(quick, &t));
    } else {
        form = form.child(server);
        for discovered in &s.discovered {
            let address = discovered.address.clone();
            form = form.child(
                row()
                    .key(format!("discover-{}", discovered.id))
                    .role("button")
                    .w_full()
                    .h(34.0)
                    .px(10.0)
                    .radius(8.0)
                    .items_center()
                    .gap(8.0)
                    .fill(t.card)
                    .hover(t.hover)
                    .motion(crate::theme::motion::CONTROL)
                    .focusable(true)
                    .focus_visual(2.0)
                    .press_scale(0.98)
                    .cursor(rune::Cursor::Pointer)
                    .on_press(move |s: &mut State| s.use_discovered(address.clone()))
                    .child(
                        text(discovered.name.clone())
                            .font(type_::label())
                            .color(t.text),
                    )
                    .child(el().grow(1.0))
                    .child(
                        text(discovered.address.clone())
                            .font(type_::mono_caption())
                            .color(t.text_secondary)
                            .truncate(),
                    ),
            );
        }
        form = form.child(user).child(password);
        if let Some(status) = status {
            form = form.child(status);
        }
        form = form.child(connect).child(
            el().key("quick-connect-start")
                .role("button")
                .h(32.0)
                .center()
                .hover(t.hover)
                .motion(crate::theme::motion::CONTROL)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.97)
                .child(
                    text("Use Quick Connect instead")
                        .font(type_::label())
                        .color(t.text_secondary),
                )
                .cursor(rune::Cursor::Pointer)
                .on_press(|s: &mut State| s.start_quick_connect()),
        );
    }
    col()
        .key("connect-screen")
        .w_full()
        .h_full()
        .padding(28.0)
        .fill(t.bg)
        .center()
        .child(
            el().key("connect-glow")
                .absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .radial_gradient(
                    [0.5, 0.22],
                    430.0,
                    [
                        (0.0, crate::theme::accent().alpha(0.10)),
                        (0.6, crate::theme::accent().alpha(0.035)),
                        (1.0, Color::TRANSPARENT),
                    ],
                ),
        )
        .child(form)
}

/// A label over a text field. `secure` masks the value; the key is what tests and scripted input
/// address, so it never comes from the label.
/// The code Jellyfin's own UI asks for, plus the waiting state and a way out.
fn quick_card(quick: &crate::state::QuickLogin, t: &tok::Theme) -> El {
    let status = if let Some(error) = &quick.error {
        text(error.clone())
            .key("quick-connect-error")
            .font(type_::label())
            .color(theme::status::danger())
    } else if quick.authenticated {
        text("Approved. Signing in…")
            .key("quick-connect-status")
            .font(type_::label())
            .color(t.text_secondary)
    } else {
        text("Waiting for approval…")
            .key("quick-connect-status")
            .font(type_::label())
            .color(t.text_secondary)
    };
    col()
        .key("quick-connect-card")
        .w_full()
        .padding(16.0)
        .gap(10.0)
        .radius(12.0)
        .fill(t.card)
        .outline(1.0, 0.0, t.card_border)
        .child(text("Quick Connect").font(type_::section()).color(t.text))
        .child(
            text("Enter this code in Jellyfin, under Settings → Quick Connect.")
                .font(type_::meta())
                .color(t.text_secondary),
        )
        .child(
            el().key("quick-connect-code")
                .w_full()
                .h(64.0)
                .radius(10.0)
                .center()
                .fill(t.bg)
                .child(
                    text(if quick.code.is_empty() {
                        "Starting…".to_string()
                    } else {
                        quick.code.clone()
                    })
                    .font(rune::TextStyle::new(36.0, 700.0).tracking(4.0))
                    .color(if quick.code.is_empty() {
                        t.text_secondary
                    } else {
                        crate::theme::accent()
                    }),
                ),
        )
        .child(
            text(if quick.server_name.is_empty() {
                quick.server_url.clone()
            } else {
                format!("{} · {}", quick.server_name, quick.server_url)
            })
            .font(type_::mono_caption())
            .color(t.text_secondary)
            .truncate(),
        )
        .child(status)
        .child(
            el().key("quick-connect-cancel")
                .role("button")
                .h(32.0)
                .px(12.0)
                .radius(8.0)
                .center()
                .fill(t.bg)
                .hover(t.hover)
                .motion(crate::theme::motion::CONTROL)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.97)
                .child(text("Cancel").font(type_::label()).color(t.text))
                .cursor(rune::Cursor::Pointer)
                .on_press(|s: &mut State| s.cancel_quick_connect()),
        )
}

/// The "Find my server" action that sits on the Server field's caption line, so discovery does
/// not make the connect card taller in a small window.
fn discover_action(s: &State, t: &tok::Theme) -> El {
    el().key("discover")
        .role("button")
        .h(22.0)
        .px(8.0)
        .radius(6.0)
        .center()
        .fill(rune::Color::TRANSPARENT)
        .hover(t.hover)
        .motion(crate::theme::motion::CONTROL)
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.96)
        .child(
            text(if s.discovering {
                "Searching the network…".to_string()
            } else if s.discovered.is_empty() {
                "Find my server".to_string()
            } else {
                format!("Find my server · {} found", s.discovered.len())
            })
            .font(type_::caption())
            .color(t.text_secondary),
        )
        .cursor(rune::Cursor::Pointer)
        .on_press(|s: &mut State| s.discover_servers())
}

#[derive(Default)]
struct FieldOptions<'a> {
    secure: bool,
    placeholder: &'a str,
    hint: Option<&'a str>,
    action: Option<El>,
}

fn field(
    label: &'static str,
    edit: &TextEdit,
    key: &'static str,
    options: FieldOptions,
    access: fn(&mut State) -> &mut TextEdit,
) -> El {
    let FieldOptions {
        secure,
        placeholder,
        hint,
        action,
    } = options;
    let t = theme::colors();
    let mut caption = row()
        .w_full()
        .items_center()
        .gap(8.0)
        .child(text(label).font(type_::label()).color(t.text_secondary));
    if let Some(action) = action {
        caption = caption.child(el().grow(1.0)).child(action);
    }
    let mut input = text_input(edit, type_::body())
        .key(key)
        .label(label.to_string())
        .placeholder(placeholder, t.text_secondary.alpha(0.55))
        .on_edit(access)
        .on_submit(move |s: &mut State, _tx| {
            // In the TV layout the field's OK asks for the keyboard; On-demand and a physical
            // keyboard (Off) keep the form's own submit.
            if s.keyboard_enter_open(key) {
                return;
            }
            s.connect();
        })
        .role("textbox")
        .single_line()
        .grow(1.0)
        .color(t.text);
    if secure {
        input = input.secure();
    }
    // The caret of the field that holds focus is what the runtime tracks; a border that follows
    // it is the cheapest honest focus cue at this stage (the ring design lands with the shell).
    let focused = edit.focused;
    let box_ = el()
        .key(format!("{key}-box"))
        .h(40.0)
        .radius(10.0)
        .px(10.0)
        .center_y()
        .fill(t.card)
        .outline(if focused { 1.5 } else { 0.0 }, 0.0, crate::theme::accent())
        .box_shadow(rune::Shadow {
            y: 0.0,
            blur: 0.0,
            spread: if focused { 3.0 } else { 0.0 },
            color: crate::theme::accent().alpha(if focused { 0.18 } else { 0.0 }),
            inset: false,
            ..rune::Shadow::default()
        })
        .child(input);
    let mut field = col()
        .key(format!("{key}-field"))
        .gap(6.0)
        .w_full()
        .child(caption)
        .child(box_);
    if let Some(hint) = hint {
        field = field.child(
            text(hint)
                .key(format!("{key}-hint"))
                .font(type_::caption())
                .color(t.text_secondary.alpha(0.75)),
        );
    }
    field
}

/// A Home rail's full grid. It is a local shelf, not a server library, so it never changes the
/// route; Back returns to the same Home scroll.
fn shelf_screen(s: &State, shelf: crate::state::HomeShelf) -> El {
    let t = theme::colors();
    let items: &[Item] = match shelf {
        crate::state::HomeShelf::ContinueWatching => &s.home_data.resume,
        crate::state::HomeShelf::NextUp => &s.home_data.next_up,
        crate::state::HomeShelf::RecentlyAdded => s
            .home_data
            .latest
            .first()
            .map_or(&[], |rail| rail.items.as_slice()),
    };
    let mut grid = row().w_full().gap(CARD_GAP).flex_wrap();
    for item in items {
        grid = grid.child(card(item, "shelf", s, &t));
    }
    let body = if items.is_empty() {
        text("Nothing here yet.")
            .key("shelf-empty")
            .font(type_::body())
            .color(t.text_secondary)
    } else {
        grid
    };
    row()
        .key("shelf-screen")
        .w_full()
        .h_full()
        .fill(t.bg)
        .child(sidebar(s, &t))
        .child(el().w(1.0).h_full().fill(t.separator))
        .child(
            col()
                .key("shelf-content")
                .grow(1.0)
                .min_w(0.0)
                .h_full()
                .padding(theme::gutter())
                .gap(18.0)
                .focus_group()
                .child(
                    row()
                        .w_full()
                        .items_center()
                        .gap(12.0)
                        .child(
                            el().key("shelf-back")
                                .role("button")
                                .h(32.0)
                                .px(12.0)
                                .radius(8.0)
                                .center()
                                .fill(t.card)
                                .hover(t.hover)
                                .motion(crate::theme::motion::CONTROL)
                                .focusable(true)
                                .focus_visual(2.0)
                                .press_scale(0.97)
                                .child(text("← Home").font(type_::label()).color(t.text))
                                .cursor(rune::Cursor::Pointer)
                                .on_press(|s: &mut State| s.close_shelf()),
                        )
                        .child(
                            text(shelf.title())
                                .key("shelf-title")
                                .font(type_::page())
                                .color(t.text),
                        )
                        .child(el().grow(1.0))
                        .child(
                            text(format!("{} items", items.len()))
                                .key("shelf-count")
                                .font(type_::caption())
                                .color(t.text_secondary),
                        ),
                )
                .child(
                    col()
                        .w_full()
                        .grow(1.0)
                        .min_h(0.0)
                        .scroll(rune::ScrollSpec {
                            stick_bottom: false,
                        })
                        .child(body),
                ),
        )
}

fn home(s: &State) -> El {
    let t = theme::colors();
    row()
        .key("home-screen")
        .w_full()
        .h_full()
        .fill(t.bg)
        .child(sidebar(s, &t))
        .child(el().w(1.0).h_full().fill(t.separator))
        .child(content(s, &t))
}

/// Every screen reserves the rail's width with this spacer. The rail itself is one overlay in
/// `with_rail`, so it can expand over the page without reflowing a single card.
pub(super) fn sidebar(_s: &State, _t: &tok::Theme) -> El {
    el().w(theme::SIDEBAR_W).h_full()
}

/// The five destinations, in the screen spec's order: Search is the second anchor.
pub(super) const RAIL_KEYS: [&str; 5] = [
    "nav-home",
    "nav-search",
    "nav-libraries",
    "nav-live",
    "nav-settings",
];

fn rail_expanded(s: &State) -> bool {
    s.focused_key
        .as_deref()
        .is_some_and(|key| RAIL_KEYS.contains(&key))
}

fn rail_visible(s: &State) -> bool {
    // Detail and Seerr detail are full-bleed pages with their own Back; the rail would cover
    // the poster and contradict the captures.
    if matches!(s.route, Route::Detail { .. } | Route::SeerrDetail { .. }) {
        return false;
    }
    let full_player = s.in_app.is_some() && !s.in_app_minimized;
    s.is_signed_in() && !full_player && !s.in_app_launching && !s.launching && s.playback.is_none()
}

/// Wrap a page in the shell. The page already carries the 76 pt spacer from `sidebar`; the rail
/// is an absolute overlay, so the content never moves when it opens.
pub(super) fn with_rail(s: &State, base: El) -> El {
    if !rail_visible(s) {
        return base;
    }
    let t = theme::colors();
    let expanded = rail_expanded(s);
    let scrim = expanded.then(|| {
        el().key("rail-scrim")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .fill(Color::BLACK.alpha(0.38))
            .motion(theme::motion::RAIL)
            .transition(rune::Transition::opacity().springs(theme::motion::PAGE))
            .no_focus_ring()
    });
    el().key("app-shell")
        .w_full()
        .h_full()
        .child(base)
        .children(scrim)
        .child(nav_rail(s, &t, expanded))
}

fn nav_rail(s: &State, t: &tok::Theme, expanded: bool) -> El {
    let width = if expanded {
        theme::RAIL_OPEN_W
    } else {
        theme::SIDEBAR_W
    };
    let mark = if expanded {
        el().w(84.0)
            .h(34.0)
            .pixels_fit(theme::logo(), SurfaceFit::Fit)
    } else {
        el().w(32.0)
            .h(32.0)
            .pixels_fit(theme::app_icon(), SurfaceFit::Fit)
    };
    let mut list = col()
        .key("rail-list")
        // A list group moves on Up/Down and stops at its ends; Left/Right fall through to the
        // zone router. Without the role the group kept wrapping Home -> Settings.
        .role("list")
        .w_full()
        .gap(if expanded { 4.0 } else { 7.0 })
        .focus_group()
        .child(
            el().key("rail-mark")
                .w_full()
                .h(if expanded { 46.0 } else { 44.0 })
                .center()
                .child(mark),
        )
        .child(rail_item(
            "Home",
            "nav-home",
            &theme::icon::HOME,
            matches!(s.route, Route::Home),
            RailMode {
                expanded,
                focused: s.focused_key.as_deref() == Some("nav-home"),
            },
            t,
            |s| s.go_home(),
        ))
        .child(rail_item(
            "Search",
            "nav-search",
            &theme::icon::SEARCH,
            matches!(s.route, Route::Search { .. }),
            RailMode {
                expanded,
                focused: s.focused_key.as_deref() == Some("nav-search"),
            },
            t,
            |s| s.open_search(None),
        ))
        .child(rail_item(
            "Libraries",
            "nav-libraries",
            &theme::icon::LIBRARY,
            matches!(s.route, Route::Libraries | Route::Library { .. }),
            RailMode {
                expanded,
                focused: s.focused_key.as_deref() == Some("nav-libraries"),
            },
            t,
            |s| s.open_libraries(),
        ))
        .child(rail_item(
            "Live TV",
            "nav-live",
            &theme::icon::ANTENNA,
            matches!(s.route, Route::LiveTv),
            RailMode {
                expanded,
                focused: s.focused_key.as_deref() == Some("nav-live"),
            },
            t,
            |s| s.open_live_tv(),
        ))
        .child(el().grow(1.0))
        .child(rail_item(
            "Settings",
            "nav-settings",
            &theme::icon::SETTINGS,
            matches!(s.route, Route::Settings),
            RailMode {
                expanded,
                focused: s.focused_key.as_deref() == Some("nav-settings"),
            },
            t,
            |s| s.open_settings(),
        ));
    list = list.padding(if expanded { 12.0 } else { 14.0 });
    col()
        .key("rail")
        .absolute(0.0, 0.0)
        .w(width)
        .h_full()
        .fill(t.bg)
        .gradient(t.card, t.bg)
        .outline(1.0, 0.0, t.separator)
        .motion(theme::motion::RAIL)
        .child(list)
}

#[derive(Clone, Copy)]
struct RailMode {
    expanded: bool,
    focused: bool,
}

fn rail_item(
    label: &str,
    key: &str,
    glyph: &'static Icon,
    selected: bool,
    mode: RailMode,
    t: &tok::Theme,
    on: impl Fn(&mut State) + 'static,
) -> El {
    let expanded = mode.expanded;
    let focused = mode.focused;
    let mut item = el()
        .key(key)
        .role("button")
        .h(if expanded { 40.0 } else { 44.0 })
        .radius(9.0)
        .gap(10.0)
        .center_y()
        .fill(if selected {
            Color::WHITE.alpha(0.08)
        } else {
            Color::TRANSPARENT
        })
        .outline(
            if selected { 1.0 } else { 0.0 },
            0.0,
            Color::WHITE.alpha(0.12),
        )
        .hover(t.hover)
        .motion(theme::motion::HOVER)
        .scale(if focused { 1.05 } else { 1.0 })
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.96)
        .cursor(rune::Cursor::Pointer)
        .label(label.to_string())
        .help(label.to_string())
        .child(icon(glyph, 19.0).color(if selected { t.text } else { t.text_secondary }))
        .on_press(move |s: &mut State| on(s));
    if expanded {
        item = item
            .w_full()
            .px(10.0)
            .child(text(label).font(type_::label()).color(if selected {
                t.text
            } else {
                t.text_secondary
            }))
            .transition(rune::Transition::opacity().springs(theme::motion::SHEET_PAIR));
    } else {
        item = item.w(48.0).center();
    }
    item
}

fn content(s: &State, t: &tok::Theme) -> El {
    let subtitle = if s.home_data.resume.is_empty() {
        "Your library, recently added"
    } else {
        "Pick up where you left off"
    };
    let hero = s.home_hero.then(|| home_hero(s, t)).flatten();
    let header = if hero.is_some() {
        el()
    } else {
        row()
            .key("home-header")
            .w_full()
            .pt(28.0)
            .pb(22.0)
            .px(theme::gutter())
            .gap(16.0)
            .items_start()
            .child(
                col()
                    .grow(1.0)
                    .gap(6.0)
                    .child(
                        text("Home")
                            .font(hero_style())
                            .color(t.text)
                            .key("home-title"),
                    )
                    .child(
                        text(subtitle.to_string())
                            .font(subtitle_style())
                            .color(t.text_secondary),
                    ),
            )
    };
    let body = if s.home_data.is_empty() {
        if let Some(message) = &s.home_error {
            error_state(message, t)
        } else if !s.home_loaded {
            skeleton(t)
        } else {
            empty_state(t)
        }
    } else {
        col()
            .key("home-scroll")
            .w_full()
            .grow(1.0)
            .min_w(0.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .pb(if s.in_app_minimized { 124.0 } else { 32.0 })
            .children(
                s.home_error
                    .as_ref()
                    .map(|message| error_banner(message, t)),
            )
            .children(hero)
            .child(
                col()
                    .w_full()
                    .px(theme::gutter())
                    .pt(if s.home_hero { 16.0 } else { 0.0 })
                    .gap(theme::RAIL_GAP)
                    .child(rails(s, t)),
            )
    };
    let mut page = col()
        .key("home-content")
        .grow(1.0)
        .min_w(0.0)
        .min_h(0.0)
        .h_full()
        .fill(t.bg);
    // With the billboard up, its own gradient already ends in the page colour. The ambient
    // backdrop under the rest of the page put a dim image below that solid edge, which read as
    // a seam across the top item.
    if !s.home_hero {
        page = page.child(ambient(s, t));
    }
    let band = el()
        .key("home-band")
        .absolute(0.0, 0.0)
        .w_full()
        .h(340.0)
        .linear_gradient(
            std::f32::consts::PI,
            [
                (0.0, t.card.alpha(0.55)),
                (0.55, t.card.alpha(0.22)),
                (1.0, Color::TRANSPARENT),
            ],
        );
    if s.home_hero {
        page = page.child(header).child(body);
    } else {
        page = page.child(band).child(header).child(body);
    }
    page
}

/// One cached backdrop behind Home, dimmed until it is only a temperature shift. The cards and
/// text stay the subject; the image is not a hero and never intercepts pointer input.
fn ambient(s: &State, t: &tok::Theme) -> El {
    let Some(key) = s.home_backdrop_key() else {
        return el()
            .key("home-ambient")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full();
    };
    let Some(layers) = poster_layers(s, &key, SurfaceFit::Fill) else {
        return el()
            .key("home-ambient")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full();
    };
    el().key("home-ambient")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .child(
            el().absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .opacity(0.30)
                .child(layers),
        )
        .child(el().absolute(0.0, 0.0).w_full().h_full().linear_gradient(
            std::f32::consts::PI,
            [
                (0.0, t.bg.alpha(0.40)),
                (0.48, t.bg.alpha(0.78)),
                (1.0, t.bg),
            ],
        ))
}

/// Optional compact hero: one film at a time, 180 points tall, with its backdrop and one
/// action. It is off by default so the rails stay the first thing on screen. The subject is the
/// first Continue Watching entry the reader has not hidden.
fn home_hero(s: &State, t: &tok::Theme) -> Option<El> {
    let item = s.hero_item()?;
    let image_key = s.hero_backdrop_key().or_else(|| s.resume_image_key(item));
    let image = image_key
        .as_deref()
        .and_then(|key| poster_layers(s, key, SurfaceFit::Fill))
        .unwrap_or_else(|| el().w_full().h_full().fill(t.card));
    let title = item
        .series_name
        .as_deref()
        .unwrap_or(&item.name)
        .to_string();
    let subtitle = match item.episode_label() {
        Some(label) => {
            let left = crate::home::remaining(item)
                .map(|minutes| format!(" · {minutes} min left"))
                .unwrap_or_default();
            format!("{label} · {}{left}", item.name)
        }
        None => crate::home::resume_subtitle(item),
    };
    let series =
        item.kind.eq_ignore_ascii_case("episode") || item.kind.eq_ignore_ascii_case("series");
    let kicker = if series {
        "Continue watching · Series"
    } else {
        "Continue watching · Film"
    };
    let mut facts = Vec::new();
    if let Some(year) = item.production_year {
        facts.push(year.to_string());
    }
    if let Some(rating) = &item.official_rating {
        facts.push(rating.clone());
    }
    facts.push(subtitle);
    let meta = facts.join("  ·  ");
    let id = item.series_id.clone().unwrap_or_else(|| item.id.clone());
    let play = item.clone();
    let detail = id.clone();
    let height = (s.window_size.1 * 0.5).clamp(300.0, 560.0);
    Some(
        el().key("home-hero")
            .w_full()
            .h(height)
            .clip_rounded(0.0)
            .child(
                el().absolute(0.0, 0.0)
                    .w_full()
                    .h_full()
                    .opacity(0.92)
                    .child(image),
            )
            .child(el().absolute(0.0, 0.0).w_full().h_full().linear_gradient(
                std::f32::consts::FRAC_PI_2,
                [
                    (0.0, t.bg.alpha(0.98)),
                    (0.34, t.bg.alpha(0.78)),
                    (0.70, t.bg.alpha(0.18)),
                    (1.0, Color::TRANSPARENT),
                ],
            ))
            .child(el().absolute(0.0, 0.0).w_full().h_full().linear_gradient(
                std::f32::consts::PI,
                [
                    (0.0, Color::TRANSPARENT),
                    (0.55, t.bg.alpha(0.10)),
                    (1.0, t.bg),
                ],
            ))
            .child(
                col()
                    .absolute(theme::gutter(), height - 164.0)
                    .w(560.0)
                    .gap(8.0)
                    .child(text(kicker).font(type_::caption()).color(t.text_secondary))
                    .child(
                        theme::readable_text(title.clone(), hero_style(), t.text)
                            .key("home-hero-title"),
                    )
                    .child(
                        theme::readable_text(meta, type_::meta(), t.text_secondary)
                            .key("home-hero-subtitle"),
                    )
                    .child(
                        row()
                            .gap(9.0)
                            .mt(6.0)
                            .child(
                                el().key("home-hero-play")
                                    .role("button")
                                    .label("Resume")
                                    .h(38.0)
                                    .px(18.0)
                                    .radius(8.0)
                                    .center()
                                    .fill(t.text)
                                    .hover(t.hover)
                                    .motion(theme::motion::CONTROL)
                                    .focusable(true)
                                    .focus_on_light()
                                    .press_scale(0.97)
                                    .cursor(rune::Cursor::Pointer)
                                    .child(text("▸ Resume").font(type_::label()).color(t.bg))
                                    .on_press(move |s: &mut State| s.play_episode(play.clone())),
                            )
                            .child(
                                el().key("home-hero-open")
                                    .role("button")
                                    .label("More info")
                                    .h(38.0)
                                    .px(16.0)
                                    .radius(8.0)
                                    .center()
                                    .fill(Color::WHITE.alpha(0.10))
                                    .outline(1.0, 0.0, Color::WHITE.alpha(0.10))
                                    .hover(t.hover)
                                    .motion(theme::motion::CONTROL)
                                    .focusable(true)
                                    .focus_visual(2.0)
                                    .press_scale(0.97)
                                    .cursor(rune::Cursor::Pointer)
                                    .child(text("More info").font(type_::label()).color(t.text))
                                    .on_press(move |s: &mut State| s.open_detail(detail.clone())),
                            ),
                    ),
            ),
    )
}

/// One Home rail, in the order Home shows them. The list is what the virtual stack builds by
/// index, so a rail below the viewport costs nothing until it is scrolled to.
pub(crate) enum HomeRail<'a> {
    Resume(&'a [Item]),
    Libraries,
    Poster {
        title: String,
        key: String,
        items: &'a [Item],
        library: Option<(String, String, Option<String>)>,
        shelf: Option<crate::state::HomeShelf>,
        wide: bool,
    },
    OnNow,
}

impl HomeRail<'_> {
    pub(crate) fn key(&self) -> &str {
        match self {
            HomeRail::Resume(_) => "rail-continue-watching",
            HomeRail::Libraries => "rail-libraries",
            HomeRail::Poster { key, .. } => key,
            HomeRail::OnNow => "rail-on-now",
        }
    }
}

/// Home's rails, in reading order: Continue Watching, Next up, Coming up, On now, every
/// library's Recently added, Recently Played, Collections. A rail with nothing visible (every
/// item hidden) is left out, which is also what keeps its TV zone empty.
pub(crate) fn home_rails(s: &State) -> Vec<HomeRail<'_>> {
    let visible = |items: &[Item]| items.iter().any(|item| !s.is_home_hidden(&item.id));
    let mut rails = Vec::new();
    if visible(&s.home_data.resume) {
        rails.push(HomeRail::Resume(&s.home_data.resume));
    }
    if !s.home_data.libraries.is_empty() {
        rails.push(HomeRail::Libraries);
    }
    for (index, recent) in s
        .home_data
        .latest
        .iter()
        .filter(|rail| rail.items.iter().any(|item| !s.is_home_hidden(&item.id)))
        .enumerate()
    {
        let library = s
            .home_data
            .libraries
            .iter()
            .find(|library| library.id == recent.library_id)
            .map(|library| {
                (
                    library.id.clone(),
                    library.name.clone(),
                    library.collection_type.clone(),
                )
            });
        rails.push(HomeRail::Poster {
            title: if recent.library_id.is_empty() {
                "Recently added".to_string()
            } else {
                format!("Recently added in {}", recent.library)
            },
            key: format!("rail-recent-{index}"),
            items: &recent.items,
            library,
            shelf: if recent.library_id.is_empty() {
                Some(crate::state::HomeShelf::RecentlyAdded)
            } else {
                None
            },
            wide: true,
        });
    }
    if visible(&s.home_data.next_up) {
        rails.push(HomeRail::Poster {
            title: "Next up".to_string(),
            key: "rail-next-up".to_string(),
            items: &s.home_data.next_up,
            library: None,
            shelf: Some(crate::state::HomeShelf::NextUp),
            wide: false,
        });
    }
    if visible(&s.home_data.on_now) {
        rails.push(HomeRail::OnNow);
    }
    if !s.home_data.collections.is_empty() {
        let collections_library = s
            .home_data
            .libraries
            .iter()
            .find(|library| library.collection_type.as_deref() == Some("boxsets"))
            .map(|library| {
                (
                    library.id.clone(),
                    library.name.clone(),
                    library.collection_type.clone(),
                )
            });
        rails.push(HomeRail::Poster {
            title: "Collections".to_string(),
            key: "rail-collections".to_string(),
            items: &s.home_data.collections,
            library: collections_library,
            shelf: None,
            wide: true,
        });
    }
    rails
}

/// Home's rail keys in order: the page's contract, without building the page.
#[cfg(test)]
pub(crate) fn home_rail_keys(s: &State) -> Vec<String> {
    home_rails(s)
        .iter()
        .map(|rail| rail.key().to_string())
        .collect()
}

/// One rail's element, by index in [`home_rails`].
fn home_rail(s: &State, index: usize, t: &tok::Theme) -> El {
    let rails = home_rails(s);
    let Some(descriptor) = rails.get(index) else {
        return col().key(format!("home-rail-{index}"));
    };
    match descriptor {
        HomeRail::Resume(items) => wide_rail(
            WideRail {
                title: "Continue watching",
                key: "rail-continue-watching",
                items,
                library: None,
                shelf: Some(crate::state::HomeShelf::ContinueWatching),
                show_progress: true,
            },
            s,
            t,
        ),
        HomeRail::Libraries => library_strip(s, t),
        HomeRail::OnNow => on_now_rail(s, t),
        HomeRail::Poster {
            title,
            key,
            items,
            library,
            shelf,
            wide,
        } => {
            if *wide {
                wide_rail(
                    WideRail {
                        title,
                        key,
                        items,
                        library: library.clone(),
                        shelf: *shelf,
                        show_progress: false,
                    },
                    s,
                    t,
                )
            } else {
                rail(title, key, items, s, t, library.clone(), *shelf)
            }
        }
    }
}

/// A poster rail's height before it is measured: its title (26), the gap under it (12), the
/// poster and its two lines (264 + ~44), and the gap to the next rail (40).
const HOME_RAIL_ESTIMATE_H: f32 = 386.0;

fn rails(s: &State, t: &tok::Theme) -> El {
    let keys: Vec<String> = home_rails(s)
        .iter()
        .map(|rail| rail.key().to_string())
        .collect();
    let count = keys.len();
    let theme = *t;
    // Only the rails in the mounted window are built: Home used to lay out every rail and
    // every card on it, three screenfuls below the fold included. Rune keeps a measured height
    // per rail key, so a page scrolled back to doesn't re-measure, and `Tx::scroll_into_view`
    // still reaches any rail by key.
    col()
        .key("home-rails")
        .w_full()
        .min_w(0.0)
        .growth(GrowthSpec::default())
        // Home's rows are tall (a rail is over half a viewport), so Rune's default of two
        // viewports each side mounts nearly every rail. One viewport of overscan is the next
        // rail coming, which is all a reader can see; the window shifts halfway through it, so
        // the reader never meets a rail still being built.
        .motion(theme::motion::LAYOUT)
        .virtual_overscan(1.0, 0.5)
        .virtual_rows(
            count,
            HOME_RAIL_ESTIMATE_H,
            move |_s: &State, i| {
                keys.get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("home-rail-{i}"))
                    .into()
            },
            move |s: &State, i| home_rail(s, i, &theme).pb(theme::RAIL_GAP),
        )
}

fn rail(
    title: &str,
    key: &str,
    items: &[Item],
    s: &State,
    t: &tok::Theme,
    library: Option<(String, String, Option<String>)>,
    shelf: Option<crate::state::HomeShelf>,
) -> El {
    let mut header = row().w_full().items_end().gap(12.0).child(
        theme::readable_text(title, type_::section(), t.text_secondary)
            .key(format!("{key}-title"))
            .grow(1.0),
    );
    if let Some((id, name, collection_type)) = library {
        header = header.child(
            el().key(format!("{key}-all"))
                .role("button")
                .h(26.0)
                .px(8.0)
                .radius(6.0)
                .center()
                .fill(t.bg)
                .hover(t.hover)
                .motion(crate::theme::motion::CONTROL)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.96)
                .child(text("See all").font(type_::label()).color(t.text_secondary))
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut State| {
                    s.open_library(id.clone(), name.clone(), collection_type.clone())
                }),
        );
    } else if let Some(shelf) = shelf {
        header = header.child(
            el().key(format!("{key}-all"))
                .role("button")
                .h(26.0)
                .px(8.0)
                .radius(6.0)
                .center()
                .fill(t.bg)
                .hover(t.hover)
                .motion(crate::theme::motion::CONTROL)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.96)
                .child(text("See all").font(type_::label()).color(t.text_secondary))
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut State| s.open_shelf(shelf)),
        );
    }
    col().key(key).w_full().gap(12.0).child(header).child(
        el().w_full().scroll_x().scroll_edge_fade(28.0).child(
            row()
                .px(6.0)
                .py(6.0)
                .gap(CARD_GAP)
                .focus_group()
                .children(items.iter().map(|item| card(item, key, s, t))),
        ),
    )
}

/// Live channels with a current program. A channel is not a poster item, so it gets its own
/// compact card and plays the tuner directly.
fn on_now_rail(s: &State, t: &tok::Theme) -> El {
    let mut rail = row().gap(12.0).focus_group();
    for channel in &s.home_data.on_now {
        let program = channel
            .current_program
            .as_ref()
            .map(|program| program.name.clone())
            .unwrap_or_default();
        let number = channel.channel_number.clone().unwrap_or_else(|| "—".into());
        let play = channel.clone();
        let card_key = format!("rail-on-now-{}", channel.id);
        let tile = col()
            .key(card_key.clone())
            .role("button")
            .w(250.0)
            .h(104.0)
            .padding(12.0)
            .gap(5.0)
            .radius(10.0)
            .fill(t.card)
            .gradient(Color::WHITE.alpha(0.045), Color::TRANSPARENT)
            .outline(1.0, 0.0, t.card_border)
            .shadow(1.0, 2.0, Color::BLACK.alpha(0.25))
            .shadow(6.0, 18.0, Color::BLACK.alpha(0.16))
            .hover(t.hover)
            .motion(crate::theme::motion::CONTROL)
            .focusable(true)
            .focus_visual(2.0)
            .press_scale(0.98)
            .cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut State| s.play_live_channel(play.clone()))
            .child(
                row()
                    .gap(8.0)
                    .items_center()
                    .child(
                        text("● LIVE")
                            .font(type_::caption())
                            .color(theme::status::danger()),
                    )
                    .child(
                        text(format!("#{number}"))
                            .font(type_::mono_caption())
                            .color(t.text_secondary),
                    ),
            )
            .child(
                text(channel.name.clone())
                    .font(type_::title())
                    .color(t.text)
                    .truncate(),
            )
            .child(
                text(program)
                    .font(type_::meta())
                    .color(t.text_secondary)
                    .truncate(),
            );
        rail = rail.child(report_focused_rect(tile, s, &card_key));
    }
    col()
        .key("rail-on-now")
        .w_full()
        .gap(12.0)
        .child(
            theme::readable_text("On now", type_::section(), t.text_secondary)
                .key("rail-on-now-title"),
        )
        .child(
            el().w_full()
                .min_w(0.0)
                .scroll_x()
                .scroll_edge_fade(28.0)
                .child(rail),
        )
}

fn wide_card(item: &Item, rail: &str, s: &State, t: &tok::Theme, show_progress: bool) -> El {
    let key_id = item.series_id.as_deref().unwrap_or(&item.id);
    let focus_key = format!("{rail}-{key_id}");
    let image = s
        .resume_image_key(item)
        .or_else(|| s.image_key(item))
        .as_deref()
        .and_then(|key| poster_layers(s, key, SurfaceFit::Fill))
        .unwrap_or_else(|| placeholder(item, t));
    let title = item
        .series_name
        .as_deref()
        .unwrap_or(&item.name)
        .to_string();
    let meta = match item.episode_label() {
        Some(label) => {
            let left = crate::home::remaining(item)
                .map(|minutes| format!(" · {minutes} min left"))
                .unwrap_or_default();
            format!("{label} · {}{left}", item.name)
        }
        None => crate::home::resume_subtitle(item),
    };
    let watched = crate::home::progress(item);
    let focused = s.focused_key.as_deref() == Some(focus_key.as_str());
    let detail_id = item.series_id.clone().unwrap_or_else(|| item.id.clone());
    let card_key = focus_key.clone();
    let mut card = el()
        .key(focus_key.clone())
        .role("button")
        .label(title.clone())
        .help(title.clone())
        .w(RESUME_W)
        .h(RESUME_H)
        .radius(10.0)
        .clip_rounded(10.0)
        .fill(t.card)
        .outline(1.0, 0.0, t.card_border)
        .shadow(1.0, 2.0, Color::BLACK.alpha(0.25))
        .shadow(6.0, 18.0, t.shadow_soft)
        .scale(if focused { 1.05 } else { 1.0 })
        .motion(theme::motion::HOVER)
        .matched(format!("poster-{focus_key}"))
        .matched_motion(theme::motion::DETAIL)
        .animated_changes()
        .hover(t.hover)
        .motion(theme::motion::CONTROL)
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.98)
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut State| s.open_detail(detail_id.clone()))
        .child(image)
        .child(el().absolute(0.0, 0.0).w_full().h_full().linear_gradient(
            std::f32::consts::PI,
            [
                (0.30, Color::TRANSPARENT),
                (0.72, Color::BLACK.alpha(0.45)),
                (1.0, Color::BLACK.alpha(0.88)),
            ],
        ))
        .children(focused.then(|| crate::theme::focus_edge(10.0)))
        .child(
            col()
                .absolute(10.0, RESUME_H - 44.0)
                .w(RESUME_W - 20.0)
                .gap(2.0)
                .child(
                    text(title.clone())
                        .font(type_::label())
                        .color(t.text)
                        .max_lines(1),
                )
                .child(
                    text(meta)
                        .font(type_::caption())
                        .color(t.text_secondary)
                        .max_lines(1),
                ),
        );
    if show_progress && watched > 0.0 {
        card = card.child(
            el().absolute(0.0, RESUME_H - 3.0)
                .w((RESUME_W * watched).max(2.0))
                .h(2.5)
                .fill(crate::theme::accent())
                .motion(theme::motion::PROGRESS),
        );
    }
    report_focused_rect(card, s, &card_key)
}

struct WideRail<'a> {
    title: &'a str,
    key: &'a str,
    items: &'a [Item],
    library: Option<(String, String, Option<String>)>,
    shelf: Option<crate::state::HomeShelf>,
    show_progress: bool,
}

fn wide_rail(rail: WideRail<'_>, s: &State, t: &tok::Theme) -> El {
    let WideRail {
        title,
        key,
        items,
        library,
        shelf,
        show_progress,
    } = rail;
    let mut header = row().w_full().items_end().gap(12.0).child(
        text(title.to_string())
            .font(type_::row())
            .color(t.text)
            .key(format!("{key}-title"))
            .grow(1.0),
    );
    if let Some((id, name, collection_type)) = library {
        header = header.child(
            el().key(format!("{key}-all"))
                .role("button")
                .label(format!("See all {title}"))
                .h(26.0)
                .px(8.0)
                .radius(6.0)
                .center()
                .fill(t.bg)
                .hover(t.hover)
                .motion(theme::motion::CONTROL)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.96)
                .child(text("See all").font(type_::label()).color(t.text_secondary))
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut State| {
                    s.open_library(id.clone(), name.clone(), collection_type.clone())
                }),
        );
    } else if let Some(shelf) = shelf {
        header = header.child(
            el().key(format!("{key}-all"))
                .role("button")
                .label(format!("See all {title}"))
                .h(26.0)
                .px(8.0)
                .radius(6.0)
                .center()
                .fill(t.bg)
                .hover(t.hover)
                .motion(theme::motion::CONTROL)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.96)
                .child(text("See all").font(type_::label()).color(t.text_secondary))
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut State| s.open_shelf(shelf)),
        );
    }
    col()
        .key(key)
        .w_full()
        .min_w(0.0)
        .gap(10.0)
        .child(header)
        .child(
            el().w_full()
                .min_w(0.0)
                .scroll_x()
                .scroll_edge_fade(28.0)
                .child(
                    row().gap(12.0).focus_group().children(
                        items
                            .iter()
                            .filter(|item| !s.is_home_hidden(&item.id))
                            .map(|item| wide_card(item, key, s, t, show_progress)),
                    ),
                ),
        )
}

fn library_strip(s: &State, t: &tok::Theme) -> El {
    let libraries: Vec<&jellyfin_api::LibraryView> = s.sidebar_libraries().take(8).collect();
    let mut chips = row().key("home-libraries").w_full().flex_wrap().gap(8.0);
    for library in &libraries {
        let id = library.id.clone();
        let name = library.name.clone();
        let kind = library.collection_type.clone();
        let is_live = kind.as_deref() == Some("livetv");
        chips = chips.child(
            el().key(format!("home-library-{}", library.id))
                .role("button")
                .label(format!("{} library", library.name))
                .help(format!("Open {}", library.name))
                .h(34.0)
                .px(11.0)
                .radius(8.0)
                .gap(8.0)
                .center_y()
                .fill(t.card)
                .outline(1.0, 0.0, t.card_border)
                .hover(t.hover)
                .motion(theme::motion::CONTROL)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.97)
                .cursor(rune::Cursor::Pointer)
                .child(icon(&theme::icon::LIBRARY, 16.0).color(t.text_secondary))
                .child(
                    text(library.name.clone())
                        .font(type_::label())
                        .color(t.text),
                )
                .on_press(move |s: &mut State| {
                    if is_live {
                        s.open_live_tv();
                    } else {
                        s.open_library(id.clone(), name.clone(), kind.clone());
                    }
                }),
        );
    }
    chips = chips.child(
        el().key("home-library-all")
            .role("button")
            .label("All libraries")
            .help("Open all libraries")
            .h(34.0)
            .px(11.0)
            .radius(8.0)
            .center()
            .fill(t.card)
            .outline(1.0, 0.0, t.card_border)
            .hover(t.hover)
            .motion(theme::motion::CONTROL)
            .focusable(true)
            .focus_visual(2.0)
            .press_scale(0.97)
            .child(
                text(format!("All {}", libraries.len()))
                    .font(type_::label())
                    .color(t.text_secondary),
            )
            .cursor(rune::Cursor::Pointer)
            .on_press(|s: &mut State| s.open_libraries()),
    );
    col()
        .key("rail-libraries")
        .w_full()
        .gap(8.0)
        .child(
            row().w_full().items_end().gap(12.0).child(
                text("Your libraries")
                    .font(type_::row())
                    .color(t.text)
                    .key("rail-libraries-title")
                    .grow(1.0),
            ),
        )
        .child(chips)
}

/// The right-click menu on a poster. It is drawn in-app on every platform: Rune's native
/// popup menus only exist on macOS, and the actions are the same everywhere.
pub(super) fn card_menu(s: &State) -> El {
    let t = theme::colors();
    let Some(menu) = &s.card_menu else {
        return el();
    };
    let (window_w, window_h) = if s.window_size.0 >= 1.0 {
        s.window_size
    } else if theme::is_big_screen() {
        (1280.0, 720.0)
    } else {
        (1280.0, 800.0)
    };
    let playable = crate::state::is_playable(&menu.item);
    let resumable = menu
        .item
        .user_data
        .as_ref()
        .and_then(|user| user.position_ms())
        .is_some_and(|position| position > 0);
    let play_label = if resumable { "Resume" } else { "Play" };
    // The menu clamps against its real height. A fixed nine-row guess let a Home episode's
    // menu run off the bottom of a 720 pt TV frame.
    let mut rows = 6usize; // Open, Copy link, Favourite, Watched, Like, Dislike
    if playable {
        rows += 4; // Play, Add to playlist, Add to collection, Play on
        if resumable {
            rows += 1;
        }
        if s.in_app.is_some() {
            rows += 2;
        }
    }
    let mut panel = col()
        .key("card-menu")
        .transition(rune::Transition::offset(0.0, 8.0).combined(rune::Transition::opacity()))
        .w(theme::dialog::MENU)
        .padding(6.0)
        .gap(2.0)
        .radius(10.0)
        .focus_group()
        .child(
            text(menu.item.name.clone())
                .font(type_::caption())
                .color(t.text_secondary)
                .padding(8.0)
                .truncate(),
        )
        .child(rune::hairline(t.separator).w_full().mb(2.0))
        .child(menu_row("Open", "card-menu-open", &t, |s: &mut State| {
            s.card_menu_open_item()
        }));
    if playable {
        panel = panel.child(menu_row(
            play_label,
            "card-menu-play",
            &t,
            |s: &mut State| s.card_menu_play(false),
        ));
        if resumable {
            panel = panel.child(menu_row(
                "Start over",
                "card-menu-start-over",
                &t,
                |s: &mut State| s.card_menu_play(true),
            ));
        }
        if s.in_app.is_some() {
            panel = panel.child(menu_row(
                "Play next",
                "card-menu-next",
                &t,
                |s: &mut State| s.card_menu_play_next(),
            ));
            panel = panel.child(menu_row(
                "Add to queue",
                "card-menu-queue",
                &t,
                |s: &mut State| s.card_menu_add_to_queue(),
            ));
        }
        panel = panel.child(menu_row_menu(
            "Add to playlist…",
            "card-menu-playlist",
            &t,
            |s: &mut State| s.card_menu_add_to_playlist(),
        ));
        panel = panel.child(menu_row_menu(
            "Add to collection…",
            "card-menu-collection-add",
            &t,
            |s: &mut State| s.card_menu_add_to_collection(),
        ));
        panel = panel.child(menu_row_menu(
            "Play on…",
            "card-menu-cast",
            &t,
            |s: &mut State| s.card_menu_cast(),
        ));
    }
    panel = panel.child(menu_separator(&t));
    rows += 1;
    if s.detail
        .as_ref()
        .is_some_and(|detail| detail.item.kind == "BoxSet")
        && s.detail
            .as_ref()
            .is_some_and(|detail| detail.collection.iter().any(|item| item.id == menu.item.id))
    {
        let id = menu.item.id.clone();
        panel = panel.child(menu_row_danger(
            "Remove from this collection",
            "card-menu-collection-remove",
            &t,
            move |s: &mut State| s.remove_from_current_collection(id.clone()),
        ));
        rows += 1;
    }
    panel = panel.child(menu_separator(&t));
    rows += 1;
    let share_url = format!(
        "{}/web/index.html#!/details?id={}",
        s.config.server_url.trim_end_matches('/'),
        menu.item.id
    );
    panel = panel.child(menu_row_tx(
        "Copy link",
        "card-menu-copy-link",
        &t,
        move |s: &mut State, tx: &mut rune::Tx| {
            tx.copy(&share_url);
            s.show_toast("Link copied.");
        },
    ));
    if matches!(s.route, Route::Home) {
        let id = menu.item.id.clone();
        panel = panel
            .child(menu_row(
                "Why this?",
                "card-menu-why",
                &t,
                move |s: &mut State| s.explain_home_item(&id),
            ))
            .child(menu_row(
                "Not interested",
                "card-menu-not-interested",
                &t,
                |s: &mut State| {
                    let id = s
                        .card_menu
                        .take()
                        .map(|menu| menu.item.id)
                        .unwrap_or_default();
                    s.hide_from_home(id);
                },
            ));
        rows += 2;
    }
    panel = panel.child(menu_separator(&t));
    rows += 1;
    let favorite = menu
        .item
        .user_data
        .as_ref()
        .is_some_and(|user| user.is_favorite);
    let watched = crate::home::is_watched(&menu.item);
    panel = panel
        .child(menu_row(
            if favorite {
                "Remove favourite"
            } else {
                "Favourite"
            },
            "card-menu-favorite",
            &t,
            |s: &mut State| s.card_menu_toggle_favorite(),
        ))
        .child(menu_row(
            if watched {
                "Mark unwatched"
            } else {
                "Mark watched"
            },
            "card-menu-played",
            &t,
            |s: &mut State| s.card_menu_toggle_played(),
        ))
        .child(menu_row("Like", "card-menu-like", &t, |s: &mut State| {
            s.card_menu_set_like(Some(true))
        }))
        .child(menu_row(
            "Dislike",
            "card-menu-dislike",
            &t,
            |s: &mut State| s.card_menu_set_like(Some(false)),
        ));
    panel = panel.child(menu_separator(&t));
    rows += 1;
    if resumable && !watched {
        panel = panel.child(menu_row_danger(
            "Remove from Continue Watching",
            "card-menu-resume",
            &t,
            |s: &mut State| s.card_menu_remove_from_resume(),
        ));
        rows += 1;
    }
    if menu.item.series_id.is_some() {
        panel = panel.child(menu_row(
            "Go to series",
            "card-menu-series",
            &t,
            |s: &mut State| s.card_menu_go_to_series(),
        ));
        rows += 1;
    }
    // 30 pt rows with a 2 pt gap, the title line and padding.
    let height = 72.0 + 32.0 * rows as f32;
    let x = menu.x.min(window_w - 308.0).max(8.0);
    let y = menu.y.min(window_h - height).max(8.0);
    el().key("card-menu-layer")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .on_press(|s: &mut State| s.close_card_menu())
        .child(
            el().absolute(x, y)
                .child(theme::elevation::frosted(panel, &t)),
        )
}

/// Pick another session to play the waiting item on. Only this user's sessions are offered,
/// and never this window: the item is already here.
fn cast_picker(s: &State, t: &tok::Theme) -> El {
    let mut panel = col()
        .key("cast-picker")
        .focus_group()
        .w(theme::dialog::PICKER_WIDE)
        .padding(16.0)
        .gap(6.0)
        .radius(14.0)
        .fill(t.card)
        .outline(1.0, 0.0, t.card_border)
        .shadow(0.0, 18.0, theme::colors().shadow_soft)
        .child(
            row()
                .w_full()
                .items_center()
                .gap(8.0)
                .child(
                    text("Play on")
                        .font(type_::section())
                        .color(t.text)
                        .grow(1.0),
                )
                .child(
                    el().key("cast-picker-close")
                        .role("button")
                        .h(26.0)
                        .px(8.0)
                        .radius(13.0)
                        .center()
                        .fill(rune::Color::TRANSPARENT)
                        .hover(t.hover)
                        .motion(crate::theme::motion::CONTROL)
                        .focusable(true)
                        .focus_visual(2.0)
                        .press_scale(0.96)
                        .child(text("Esc").font(type_::caption()).color(t.text_secondary))
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| s.close_cast_picker()),
                ),
        )
        .children(s.cast_item.as_ref().map(|item| {
            text(
                item.series_name
                    .clone()
                    .unwrap_or_else(|| item.name.clone()),
            )
            .font(type_::meta())
            .color(t.text_secondary)
            .truncate()
        }));
    let targets: Vec<&jellyfin_api::SessionInfo> = s
        .cast_sessions
        .iter()
        .filter(|session| session.device_id != s.config.device_id)
        .filter(|session| s.config.user_id.is_empty() || session.user_id == s.config.user_id)
        .collect();
    if s.cast_loading {
        panel = panel.child(
            text("Looking for other devices…")
                .font(type_::meta())
                .color(t.text_secondary),
        );
    } else if targets.is_empty() {
        panel = panel.child(
            text("No other device is signed in to this server.")
                .font(type_::meta())
                .color(t.text_secondary),
        );
    }
    for session in targets {
        let id = session.id.clone();
        let name = if session.device_name.is_empty() {
            session.client.clone()
        } else {
            session.device_name.clone()
        };
        let target = name.clone();
        panel = panel.child(
            row()
                .key(format!("cast-{}", session.id))
                .role("button")
                .w_full()
                .h(38.0)
                .px(10.0)
                .radius(8.0)
                .items_center()
                .gap(10.0)
                .fill(rune::Color::TRANSPARENT)
                .hover(t.hover)
                .motion(crate::theme::motion::CONTROL)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.98)
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut State| s.cast_to_session(id.clone(), target.clone()))
                .child(text(name).font(type_::label()).color(t.text))
                .child(el().grow(1.0))
                .child(
                    text(session.client.clone())
                        .font(type_::mono_caption())
                        .color(t.text_secondary)
                        .truncate(),
                ),
        );
    }
    el().key("cast-layer")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .justify_center()
        .items_center()
        .fill(rune::Color::BLACK.alpha(0.42))
        .on_press(|s: &mut State| s.close_cast_picker())
        .child(panel)
}

fn menu_separator(t: &tok::Theme) -> El {
    el().w_full().h(1.0).my(3.0).fill(t.separator)
}

fn menu_row_tx(
    label: &str,
    key: &str,
    t: &tok::Theme,
    on: impl Fn(&mut State, &mut rune::Tx) + 'static,
) -> El {
    theme::tv_focus(
        row()
            .key(key)
            .role("button")
            .w_full()
            .h(30.0)
            .px(8.0)
            .radius(6.0)
            .items_center()
            .fill(rune::Color::TRANSPARENT)
            .hover(t.hover)
            .motion(crate::theme::motion::CONTROL)
            .focusable(true)
            .focus_visual(2.0)
            .cursor(rune::Cursor::Pointer)
            .press_scale(0.99)
            .on_press_tx(on)
            .child(
                text(label.to_string())
                    .font(type_::label())
                    .color(t.text)
                    .truncate(),
            ),
    )
}

fn menu_row(label: &str, key: &str, t: &tok::Theme, on: impl Fn(&mut State) + 'static) -> El {
    menu_row_inner(label, key, t, false, false, on)
}

/// A row that opens a picker. The chevron distinguishes it from a row that acts in place.
fn menu_row_menu(label: &str, key: &str, t: &tok::Theme, on: impl Fn(&mut State) + 'static) -> El {
    menu_row_inner(label, key, t, true, false, on)
}

/// A row that removes something. The danger tone is the only place a menu uses colour.
fn menu_row_danger(
    label: &str,
    key: &str,
    t: &tok::Theme,
    on: impl Fn(&mut State) + 'static,
) -> El {
    menu_row_inner(label, key, t, false, true, on)
}

fn menu_row_inner(
    label: &str,
    key: &str,
    t: &tok::Theme,
    chevron: bool,
    danger: bool,
    on: impl Fn(&mut State) + 'static,
) -> El {
    theme::tv_focus(
        row()
            .key(key)
            .role("button")
            .w_full()
            .h(30.0)
            .px(8.0)
            .radius(6.0)
            .items_center()
            .gap(8.0)
            .fill(rune::Color::TRANSPARENT)
            .hover(t.hover)
            .motion(crate::theme::motion::CONTROL)
            .focusable(true)
            .focus_visual(2.0)
            .cursor(rune::Cursor::Pointer)
            .press_scale(0.99)
            .on_press(on)
            .child(
                text(label.to_string())
                    .font(type_::label())
                    .color(if danger {
                        crate::theme::status::danger()
                    } else {
                        t.text
                    })
                    .grow(1.0)
                    .truncate(),
            )
            .children(chevron.then(|| {
                text("›")
                    .font(type_::label())
                    .color(t.text_secondary.alpha(0.7))
            })),
    )
}

/// Hand the window the drawn rect of the card that holds focus, for the halo it paints outside
/// the rail's scroll clip.
///
/// Rune reports a stationary element's drawn rect on its first draw and then only when it moves.
/// A card that reported once would never report again after focus came back to it, so a sideways
/// move in a rail that had nothing to scroll left the halo gone until the card moved for another
/// reason. Attaching the handler only while the card holds focus makes gaining focus a first draw
/// again; a card that isn't focused drops the handler, so its next report is its next focus.
fn report_focused_rect(el: El, s: &State, key: &str) -> El {
    if s.focused_key.as_deref() != Some(key) {
        return el;
    }
    let key = key.to_string();
    el.on_drawn_layout_tx(move |s: &mut State, rect: [f32; 4], tx: &mut rune::Tx| {
        if s.focused_key.as_deref() == Some(key.as_str()) && s.focused_card_rect != Some(rect) {
            s.focused_card_rect = Some(rect);
            tx.wake_after(std::time::Duration::from_millis(0));
        }
    })
}

pub(super) fn card(item: &Item, rail: &str, s: &State, t: &tok::Theme) -> El {
    // Home dedupes by series; a library lists the items it actually contains.
    let key_id = if rail == "library" || rail.starts_with("search-") {
        item.id.as_str()
    } else {
        item.series_id.as_deref().unwrap_or(&item.id)
    };
    let focus_key = format!("{rail}-{key_id}");
    let detail_id = item.series_id.clone().unwrap_or_else(|| item.id.clone());
    // The card's own key is the shared-element id: the same series can sit in two rails, and a
    // search rail can hold two episodes of one series, while Rune matches the last id in tree
    // order. The detail page takes the pressed card's id through `open_detail_from_card`.
    let match_id = format!("poster-{focus_key}");
    let focused = s.focused_key.as_deref() == Some(focus_key.as_str());
    let press_id = detail_id.clone();
    let key = s.image_key(item);
    let poster = key
        .as_deref()
        .and_then(|key| poster_layers(s, key, SurfaceFit::Fill))
        .unwrap_or_else(|| placeholder(item, t));
    let mut poster_box = el()
        .w(CARD_W)
        .h(CARD_H)
        .radius(10.0)
        .clip_rounded(10.0)
        .fill(t.card)
        .inset_edge()
        .outline(1.0, 0.0, t.card_border)
        .shadow(10.0, 24.0, t.shadow_soft)
        .scale(if focused { 1.05 } else { 1.0 })
        .motion(theme::motion::HOVER)
        .matched(match_id.clone())
        .matched_motion(theme::motion::DETAIL)
        .animated_changes()
        .child(
            el().absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .fill(rune::Color::WHITE.alpha(0.055))
                .group_opacity(0.0, 1.0),
        )
        .child(poster)
        .child(
            el().absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .linear_gradient(
                    std::f32::consts::FRAC_PI_4,
                    [
                        (0.0, rune::Color::WHITE.alpha(0.10)),
                        (0.45, rune::Color::TRANSPARENT),
                    ],
                )
                .group_opacity(0.0, 1.0),
        )
        .child(
            el().absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .radius(10.0)
                .outline(2.0, -1.0, rune::Color::WHITE.alpha(0.62))
                .group_opacity(0.0, 1.0),
        );
    poster_box = report_focused_rect(poster_box, s, &focus_key);
    // The silent hover preview: a trickplay frame from the item, drawn over its poster once
    // the pointer has rested. It is opt-in and has no audio track at all.
    if s.hover_preview_enabled
        && let Some(preview) = s.hover_preview.as_ref()
        && preview.item_id == item.id
        && let Some(key) = preview.tile.as_deref()
        && let Some(image) = s.images.get(key)
        && let Some(manifest) = preview.item.as_ref()
        && let Some((_, info)) = manifest.trickplay_manifest_for(s.hover_trickplay_min_width())
    {
        let frame_ms = preview.frame_ms;
        let (column, row) = info.cell_of(info.thumbnail_at(frame_ms));
        let scale = CARD_W / info.width.max(1) as f32;
        let sprite_width = info.width.max(1) as f32 * info.tile_width.max(1) as f32 * scale;
        let sprite_height = info.height.max(1) as f32 * info.tile_height.max(1) as f32 * scale;
        poster_box = poster_box
            .child(
                el().absolute(
                    -(column as f32) * info.width.max(1) as f32 * scale,
                    -(row as f32) * info.height.max(1) as f32 * scale,
                )
                .w(sprite_width)
                .h(sprite_height)
                .inert()
                .pixels_fit(image.pixels(), SurfaceFit::Fill),
            )
            .child(
                el().key(format!("hover-preview-{}", item.id))
                    .inert()
                    .absolute(8.0, CARD_H - 30.0)
                    .h(20.0)
                    .px(7.0)
                    .radius(6.0)
                    .center()
                    .fill(rune::Color::BLACK.alpha(0.68))
                    .child(
                        text(format!(
                            "PREVIEW · {}",
                            crate::detail::format_time(frame_ms)
                        ))
                        .font(type_::mono_caption())
                        .color(rune::Color::WHITE.alpha(0.92)),
                    ),
            );
    }
    let hover_id = item.id.clone();
    poster_box = poster_box.on_hover_change(move |s: &mut State, inside, tx: &mut rune::Tx| {
        if inside {
            s.start_hover_preview(hover_id.clone());
        } else {
            s.stop_hover_preview();
        }
        tx.settle();
    });
    // A play button over the poster, on hover: the fastest path from a card to the film.
    // Containers (a Series) have no stream of their own, so they keep the plain card.
    if crate::state::is_playable(item) && !s.big_screen {
        let episode = item.clone();
        poster_box = poster_box.child(
            el().key(format!("play-{}", item.id))
                .role("button")
                .absolute(CARD_W - 46.0, CARD_H - 46.0)
                .w(36.0)
                .h(36.0)
                .radius(18.0)
                .center()
                .fill(crate::theme::accent())
                .shadow(0.0, 8.0, rune::Color::BLACK.alpha(0.4))
                .group_opacity(0.0, 1.0)
                .hover(t.hover)
                .motion(crate::theme::motion::CONTROL)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.94)
                .cursor(rune::Cursor::Pointer)
                .label("Play")
                .help("Play")
                .child(icon(&theme::icon::PLAY, 15.0).color(theme::on_accent()))
                .on_press(move |s: &mut State| s.play_episode(episode.clone())),
        );
    }
    if s.playlist_editing
        && rail == "library"
        && let Some(entry) = &item.playlist_item_id
    {
        let entry = entry.clone();
        // The ends of the list have no move in that direction, so the control is not there.
        let at = s.library.as_ref().and_then(|library| {
            library
                .items
                .iter()
                .position(|candidate| candidate.id == item.id)
        });
        let len = s.library.as_ref().map_or(0, |library| library.items.len());
        for (delta, glyph, key, top) in [(-1isize, "↑", "up", 42.0f32), (1, "↓", "down", 72.0)]
        {
            if (delta < 0 && at == Some(0)) || (delta > 0 && at.is_some_and(|at| at + 1 >= len)) {
                continue;
            }
            let entry = entry.clone();
            poster_box = poster_box.child(
                el().key(format!("playlist-{key}-{}", item.id))
                    .role("button")
                    .absolute(CARD_W - 32.0, top)
                    .w(26.0)
                    .h(24.0)
                    .radius(6.0)
                    .center()
                    .fill(rune::Color::BLACK.alpha(0.72))
                    .outline(1.0, 0.0, rune::Color::WHITE.alpha(0.22))
                    .hover(t.hover)
                    .motion(crate::theme::motion::CONTROL)
                    .focusable(true)
                    .focus_visual(2.0)
                    .press_scale(0.94)
                    .label(if delta < 0 {
                        "Move earlier"
                    } else {
                        "Move later"
                    })
                    .help(if delta < 0 {
                        "Move earlier"
                    } else {
                        "Move later"
                    })
                    .child(
                        text(glyph.to_string())
                            .font(type_::caption())
                            .color(rune::Color::WHITE.alpha(0.92)),
                    )
                    .cursor(rune::Cursor::Pointer)
                    .on_press(move |s: &mut State| s.move_playlist_entry(entry.clone(), delta)),
            );
        }
        poster_box = poster_box.child(
            el().key(format!("playlist-remove-{}", item.id))
                .role("button")
                .absolute(CARD_W - 34.0, 6.0)
                .w(28.0)
                .h(28.0)
                .radius(14.0)
                .center()
                .fill(rune::Color::BLACK.alpha(0.72))
                .outline(1.0, 0.0, rune::Color::WHITE.alpha(0.22))
                .hover(t.hover)
                .motion(crate::theme::motion::CONTROL)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.94)
                .label("Remove from playlist")
                .help("Remove from playlist")
                .child(
                    text("✕".to_string())
                        .font(type_::caption())
                        .color(rune::Color::WHITE.alpha(0.92)),
                )
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut State| s.remove_playlist_entry(entry.clone())),
        );
        // A drag moves the entry by the nearest whole row; a short drag is ignored so a
        // slightly shaky press still opens the item.
        let drag_entry = item.playlist_item_id.clone().unwrap_or_default();
        poster_box = poster_box.on_drag(move |s: &mut State, drag: &mut rune::Drag, _tx| {
            if drag.phase == rune::Phase::Up && !drag.cancelled() {
                let steps = (drag.dy / (CARD_H + CARD_GAP)).round() as isize;
                if steps != 0 {
                    s.move_playlist_entry(drag_entry.clone(), steps);
                }
            }
        });
    }
    let video = item
        .media_sources
        .iter()
        .flat_map(|source| source.media_streams.iter())
        .find(|stream| stream.kind == "Video");
    let has_4k = video
        .and_then(|video| video.height)
        .is_some_and(|height| height >= 2000);
    let has_hdr = video
        .and_then(|video| video.video_range.as_deref())
        .is_some_and(|range| !range.eq_ignore_ascii_case("SDR"));
    let has_subs = item.media_sources.iter().any(|source| {
        source
            .media_streams
            .iter()
            .any(|stream| stream.kind == "Subtitle")
    });
    let audio = item
        .media_sources
        .iter()
        .flat_map(|source| source.media_streams.iter())
        .find(|stream| stream.kind == "Audio");
    let surround = audio
        .and_then(|audio| audio.channels)
        .is_some_and(|channels| channels >= 6);
    let atmos = audio.is_some_and(|audio| {
        audio
            .display_title
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains("atmos")
    });
    let versions = item.media_sources.len() > 1;
    let new = crate::home::is_new(item);
    let extras = item.special_feature_count.unwrap_or(0);
    let mut labels: Vec<(&str, &str)> = Vec::new();
    for (on, label, key) in [
        (has_4k, "4K", "4k"),
        (has_hdr, "HDR", "hdr"),
        (versions, "VERS", "versions"),
        (has_subs, "CC", "cc"),
        (surround, "5.1", "audio"),
        (atmos, "Atmos", "atmos"),
        (new, "NEW", "new"),
    ] {
        if on {
            labels.push((label, key));
        }
    }
    if extras > 0 {
        // One tray, not a second pill in the same corner as the watched check: the extras fact
        // belongs with the other facts.
        labels.push(("EXTRA", "extras"));
    }
    if !labels.is_empty() {
        // One tray, not four floating pills: the poster shows through the gaps of separate
        // badges and the edge reads as clutter. Three facts plus a count is what a glance needs.
        let hidden = labels.len().saturating_sub(3);
        let mut badges = row().gap(6.0).items_center();
        for (label, key) in labels.iter().take(3) {
            badges = badges.child(
                text(*label)
                    .key(format!("badge-{key}-{}", item.id))
                    .font(type_::mono_caption())
                    .color(rune::Color::WHITE),
            );
        }
        if hidden > 0 {
            badges = badges.child(
                text(format!("+{hidden}"))
                    .key(format!("badge-more-{}", item.id))
                    .font(type_::mono_caption())
                    .color(rune::Color::WHITE.alpha(0.72)),
            );
        }
        poster_box = poster_box.child(
            el().absolute(8.0, 8.0)
                .w(CARD_W - 16.0)
                .justify_end()
                .child(
                    row()
                        .h(20.0)
                        .px(7.0)
                        .radius(6.0)
                        .items_center()
                        .fill(rune::Color::BLACK.alpha(0.62))
                        .outline(1.0, 0.0, rune::Color::WHITE.alpha(0.10))
                        .shadow(1.0, 2.0, rune::Color::BLACK.alpha(0.30))
                        .child(badges),
                ),
        );
    }
    if s.library_selecting && rail == "library" {
        let id = item.id.clone();
        let chosen = s.library_selected.contains(&id);
        poster_box = poster_box.child(
            el().key(format!("select-{}", item.id))
                .role("button")
                .absolute(8.0, CARD_H - 36.0)
                .w(28.0)
                .h(28.0)
                .radius(7.0)
                .center()
                .fill(if chosen {
                    crate::theme::accent()
                } else {
                    rune::Color::BLACK.alpha(0.68)
                })
                .outline(1.0, 0.0, rune::Color::WHITE.alpha(0.35))
                .hover(t.hover)
                .motion(crate::theme::motion::CONTROL)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.94)
                .label("Select")
                .help("Select")
                .cursor(rune::Cursor::Pointer)
                .child(
                    text(if chosen { "✓" } else { "+" })
                        .font(type_::caption())
                        .color(if chosen { t.bg } else { rune::Color::WHITE }),
                )
                .on_press(move |s: &mut State| s.toggle_library_selected(&id)),
        );
    }
    if crate::home::is_watched(item) {
        poster_box = poster_box.child(
            el().key(format!("played-{}", item.id))
                .role("img")
                .label("Watched")
                .absolute(8.0, 8.0)
                .w(24.0)
                .h(24.0)
                .radius(12.0)
                .center()
                // A dark chip with a white check, not the accent fill the Select checkbox
                // uses: the two must never read as the same state on one card.
                .fill(rune::Color::BLACK.alpha(0.72))
                .outline(1.0, 0.0, rune::Color::WHITE.alpha(0.28))
                .shadow(0.0, 4.0, rune::Color::BLACK.alpha(0.35))
                .child(text("✓").font(type_::caption()).color(rune::Color::WHITE)),
        );
    }
    let watched = crate::home::progress(item);
    if watched > 0.0 {
        poster_box = poster_box.child(
            el().absolute(0.0, CARD_H - 4.0)
                .w(CARD_W * watched)
                .h(4.0)
                .fill(crate::theme::accent()),
        );
    }
    if focused {
        poster_box = poster_box.child(crate::theme::focus_edge(10.0));
    }
    let title = item.series_name.as_deref().unwrap_or(&item.name);
    col()
        .key(focus_key.clone())
        .role("button")
        .w(CARD_W)
        .gap(6.0)
        .radius(10.0)
        .hover_group_focus()
        .hover_motion(crate::theme::motion::HOVER, crate::theme::motion::HOVER)
        .focusable(true)
        .focus_lift_only(4.0)
        .press_scale(0.985)
        .motion(crate::theme::motion::CONTROL)
        .label(title.to_string())
        .cursor(rune::Cursor::Pointer)
        .on_press({
            let match_id = match_id.clone();
            move |s: &mut State| s.open_detail_from_card(press_id.clone(), match_id.clone())
        })
        .on_secondary_press({
            let item = item.clone();
            let focus_key = focus_key.clone();
            move |s: &mut State, press: rune::PointerPress, _tx: &mut rune::Tx| {
                s.open_card_menu_at(item.clone(), press.x, press.y, Some(focus_key.clone()));
            }
        })
        .child(poster_box)
        .child(
            // `.truncate()` forces `one_line` in Rune, so applying it after `max_lines(2)`
            // clipped every long title to one line. Two lines fit inside GRID_ROW_H.
            text(title.to_string())
                .font(type_::card_title())
                .color(t.text)
                .max_lines(2),
        )
        .child(
            text(crate::home::subtitle(item))
                .font(type_::meta())
                .color(t.text_secondary)
                .truncate(),
        )
}

/// A poster's blurred wash under its full image. The wash stays put while the full image fades
/// over it, so a late network answer reads as a focus pull rather than a blank card.
pub(super) fn poster_layers(s: &State, key: &str, fit: SurfaceFit) -> Option<El> {
    let image = s.images.get(key)?;
    let mut stack = el().w_full().h_full();
    // A fitted image is a logo with transparency: a blurred wash behind it would show through
    // the transparent parts as a smudge. Only opaque art gets the wash.
    if fit == SurfaceFit::Fill
        && let Some(wash) = s.placeholders.get(key)
    {
        stack = stack.child(
            el().absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .pixels_fit(wash, fit),
        );
    }
    Some(
        stack.child(
            el().absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .pixels_fit(image.pixels(), fit)
                .opacity(s.image_alpha(key))
                .opacity_motion(crate::theme::motion::CONTROL),
        ),
    )
}

/// `1 item`, `2 items` — a count line that never says "1 items".
pub(super) fn count_label(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

pub(super) fn placeholder(item: &Item, t: &tok::Theme) -> El {
    let initial = initial(item.series_name.as_deref().unwrap_or(&item.name));
    let (top, bottom) = theme::palette(&item.id);
    el().w_full()
        .h_full()
        .fill(top)
        .linear_gradient(std::f32::consts::PI, [(0.0, top), (1.0, bottom)])
        .radial_gradient(
            [0.5, 0.0],
            300.0,
            [
                (0.0, rune::Color::WHITE.alpha(0.05)),
                (1.0, rune::Color::TRANSPARENT),
            ],
        )
        .center()
        .child(
            text(initial)
                .font(TextStyle::new(44.0, 700.0))
                .color(t.text_secondary),
        )
}

/// The one letter a card shows when the server has no art. Release-folder names start with
/// punctuation (`[Group] Show`), which says nothing; the first alphanumeric does.
pub(super) fn initial(name: &str) -> String {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string())
}

fn skeleton(t: &tok::Theme) -> El {
    col()
        .key("home-loading")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .px(theme::gutter())
        .pb(32.0)
        .gap(theme::RAIL_GAP)
        .child(
            text("Loading your library…")
                .key("home-loading-label")
                .font(subtitle_style())
                .color(t.text_secondary),
        )
        // Each rail keeps the shape it will become: Continue Watching is a 304×171 resume card,
        // the poster rails are 176×264. Three identical tall rails read as one wrong rail.
        .child(skeleton_rail(t, 170.0, RESUME_W, RESUME_H, 3))
        .child(skeleton_rail(t, 220.0, CARD_W, CARD_H, 4))
        .child(skeleton_rail(t, 190.0, CARD_W, CARD_H, 4))
}

fn skeleton_rail(t: &tok::Theme, title_w: f32, w: f32, h: f32, count: usize) -> El {
    col()
        .w_full()
        .gap(12.0)
        .child(skeleton_bar(title_w, t))
        .child(
            row()
                .gap(CARD_GAP)
                .children((0..count).map(move |_| el().w(w).h(h).radius(10.0).fill(t.card))),
        )
}

fn skeleton_bar(width: f32, t: &tok::Theme) -> El {
    el().w(width).h(10.0).radius(5.0).fill(t.card)
}

fn empty_state(t: &tok::Theme) -> El {
    col()
        .key("home-empty")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .center()
        .gap(10.0)
        .padding(28.0)
        .radial_gradient(
            [0.5, 0.5],
            520.0,
            [
                (0.0, t.card.alpha(0.55)),
                (0.7, t.card.alpha(0.20)),
                (1.0, Color::TRANSPARENT),
            ],
        )
        .child(
            text("Nothing to watch yet")
                .font(hero_style())
                .color(t.text),
        )
        .child(
            text("Add a movie or show to your Jellyfin server, then refresh.")
                .font(subtitle_style())
                .color(t.text_secondary),
        )
        .child(retry_button("Refresh", "home-refresh", t))
}

fn error_state(message: &str, t: &tok::Theme) -> El {
    col()
        .key("home-error")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .center()
        .gap(10.0)
        .padding(28.0)
        .radial_gradient(
            [0.5, 0.5],
            520.0,
            [
                (0.0, t.card.alpha(0.45)),
                (0.7, t.card.alpha(0.16)),
                (1.0, Color::TRANSPARENT),
            ],
        )
        .child(
            text("Couldn't load your library")
                .font(hero_style())
                .color(t.text),
        )
        .child(
            text(message.to_string())
                .key("home-error-message")
                .font(subtitle_style())
                .color(theme::status::danger()),
        )
        .child(retry_button("Try again", "home-retry", t))
}

fn error_banner(message: &str, t: &tok::Theme) -> El {
    row()
        .key("home-error-banner")
        .w_full()
        .radius(10.0)
        .padding(12.0)
        .gap(12.0)
        .fill(t.card)
        .child(
            text(message.to_string())
                .font(type_::body())
                .color(theme::status::danger())
                .grow(1.0),
        )
        .child(retry_button("Retry", "home-retry", t))
}

fn retry_button(label: &str, key: &str, t: &tok::Theme) -> El {
    el().key(key)
        .role("button")
        .h(34.0)
        .px(14.0)
        .radius(8.0)
        .center()
        .fill(t.card)
        .hover(t.hover)
        .motion(crate::theme::motion::CONTROL)
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.97)
        .child(text(label).font(type_::label()).color(t.text))
        .cursor(rune::Cursor::Pointer)
        .on_press(|s: &mut State| s.load_home())
}
