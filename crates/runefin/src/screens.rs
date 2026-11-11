//! Screens. Each is a pure function of `State`: no I/O, no state mutation outside a handler
//! closure, so a headless test can drive them and a snapshot can freeze them.

use jellyfin_api::Item;
use rune::tok::{self, motion};

use crate::theme::FocusVisual;
use crate::theme::SurfaceStyle;
use crate::theme::{self, type_};
use rune::{
    Color, Cx, El, Icon, ScrollSpec, SurfaceFit, TextEdit, TextStyle, col, el, icon, row, text,
    text_input,
};

use crate::state::{Route, State, Step};

mod detail;
mod in_app;
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
pub(super) const RESUME_W: f32 = 304.0;
pub(super) const RESUME_H: f32 = 171.0;
pub(super) fn hero_style() -> TextStyle {
    // A page title, not a poster line: 40/650 with the tracking large type needs, and a line
    // height the subtitle can sit under without a gap.
    TextStyle::new(40.0, 650.0).tracking(-0.9).lh(46.0)
}
pub(super) fn subtitle_style() -> TextStyle {
    type_::body()
}

pub fn view(s: &State, _cx: &mut Cx<State>) -> El {
    let mut base = if !s.is_signed_in() {
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
            Route::LiveTv => live::live_tv_screen(s),
            Route::Settings => settings::settings_screen(s),
            Route::Seerr => seerr::seerr_screen(s),
            Route::SeerrDetail { .. } => seerr::seerr_detail_screen(s),
            Route::Home => home(s),
        }
    };
    if s.in_app_minimized {
        base = el()
            .key("mini-player-layer")
            .w_full()
            .h_full()
            .child(base)
            .child(mini_player(s));
    }
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
    let base = match &s.toast {
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
                                .combined(rune::Transition::opacity()),
                        )
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
                        .children(s.toast_undo.as_ref().map(|_| {
                            el().key("toast-undo")
                                .role("button")
                                .h(28.0)
                                .px(10.0)
                                .radius(7.0)
                                .center()
                                .fill(tok::accent(0, true).alpha(0.16))
                                .hover(theme::colors().hover)
                                .motion(motion::SNAPPY)
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
                .motion(motion::SNAPPY)
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
                                    .fill(tok::accent(0, true)),
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
        .motion(motion::SNAPPY)
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
        .w(if s.big_screen { 680.0 } else { 560.0 })
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
                        .motion(motion::SNAPPY)
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
        .fill(tok::accent(0, true))
        .gradient(Color::WHITE.alpha(0.18), Color::TRANSPARENT)
        .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
        .shadow(4.0, 12.0, tok::accent(0, true).alpha(0.35))
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .center()
        .focusable(true)
        .default_button()
        .focus_visual(2.0)
        .press_scale(0.97)
        .child(text(label).font(type_::label()).color(t.bg))
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
                .color(tok::accent(2, true)),
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
                    .motion(motion::SNAPPY)
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
                .motion(motion::SNAPPY)
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
                        (0.0, tok::accent(0, true).alpha(0.10)),
                        (0.6, tok::accent(0, true).alpha(0.035)),
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
            .color(tok::accent(2, true))
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
                        tok::accent(0, true)
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
                .motion(motion::SNAPPY)
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
        .motion(motion::SNAPPY)
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
        .on_submit(|s: &mut State, _tx| {
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
        .outline(if focused { 1.5 } else { 0.0 }, 0.0, tok::accent(0, true))
        .box_shadow(rune::Shadow {
            y: 0.0,
            blur: 0.0,
            spread: if focused { 3.0 } else { 0.0 },
            color: tok::accent(0, true).alpha(if focused { 0.18 } else { 0.0 }),
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
        crate::state::HomeShelf::RecentlyPlayed => &s.home_data.recently_played,
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
                                .motion(motion::SNAPPY)
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

pub(super) fn sidebar(s: &State, t: &tok::Theme) -> El {
    let collapsed = s.sidebar_collapsed;
    let width = if collapsed { 88.0 } else { theme::SIDEBAR_W };
    let brand = if collapsed {
        el().key("wordmark")
            .role("button")
            .label("Expand sidebar")
            .help("Expand sidebar")
            .w(40.0)
            .h(40.0)
            .radius(8.0)
            .center()
            .hover(t.hover)
            .focusable(true)
            .focus_visual(2.0)
            .press_scale(0.94)
            .cursor(rune::Cursor::Pointer)
            .on_press(|s: &mut State| s.toggle_sidebar())
            .child(
                el().w(32.0)
                    .h(32.0)
                    .pixels_fit(theme::app_icon(), SurfaceFit::Fit),
            )
    } else {
        let logo = el().key("wordmark").w(84.0).h(34.0).child(
            el().w_full()
                .h_full()
                .pixels_fit(theme::logo(), SurfaceFit::Fit),
        );
        // The wordmark is drawn for dark ground: pale letters with a light edge. On the
        // light theme it gets the app mark's own dark squircle as a plaque.
        if theme::is_light() {
            el().padding(5.0)
                .radius(9.0)
                .fill(Color::BLACK.alpha(0.86))
                .child(logo)
        } else {
            logo
        }
    };
    let toggle = el()
        .key("sidebar-toggle")
        .role("button")
        .w(28.0)
        .h(28.0)
        .radius(7.0)
        .center()
        .fill(t.card)
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.94)
        .help(if collapsed {
            "Expand sidebar"
        } else {
            "Collapse sidebar"
        })
        .child(
            icon(
                if collapsed {
                    &theme::icon::CHEVRON_RIGHT
                } else {
                    &theme::icon::CHEVRON_LEFT
                },
                14.0,
            )
            .color(t.text_secondary),
        )
        .cursor(rune::Cursor::Pointer)
        .on_press(|s: &mut State| s.toggle_sidebar());
    // Collapsed, the mark is the expand control and sits on the nav column's centre line: a
    // row with the chevron would push it left of every icon below it.
    let header = if collapsed {
        col().w_full().items_center().child(brand)
    } else {
        row()
            .w_full()
            .gap(6.0)
            .items_center()
            .child(brand)
            .child(el().grow(1.0))
            .child(toggle)
    };
    let mut list = col()
        .key("sidebar")
        .w(width)
        .h_full()
        .padding(if collapsed { 10.0 } else { 18.0 })
        .gap(if collapsed { 8.0 } else { 6.0 })
        .fill(t.bg)
        .gradient(t.card, t.bg)
        .focus_group()
        .child(header);
    let mut nav = col()
        .key("sidebar-nav")
        .w_full()
        .gap(if collapsed { 8.0 } else { 6.0 });
    if collapsed {
        nav = nav.items_center();
    }
    nav = nav.child(
        nav_item(
            "Home",
            "nav-home",
            matches!(s.route, Route::Home),
            t,
            &theme::icon::HOME,
            collapsed,
        )
        .on_press(|s: &mut State| s.go_home()),
    );

    if !s.home_data.libraries.is_empty() {
        if !collapsed {
            nav = nav.child(
                text("LIBRARIES")
                    .key("libraries-label")
                    .font(type_::section())
                    .color(t.text_secondary)
                    .mt(14.0)
                    .mb(2.0),
            );
        }
        for library in s.home_data.libraries.iter() {
            let is_live = library.collection_type.as_deref() == Some("livetv");
            let selected = if is_live {
                matches!(s.route, Route::LiveTv)
            } else {
                matches!(&s.route, Route::Library { id } if id == &library.id)
            };
            let id = library.id.clone();
            let name = library.name.clone();
            let collection_type = library.collection_type.clone();
            let glyph: &'static rune::Icon = match collection_type.as_deref() {
                Some("movies") => &theme::icon::FILM,
                Some("tvshows") => &theme::icon::TV,
                Some("boxsets") => &theme::icon::LAYERS,
                Some("livetv") => &theme::icon::ANTENNA,
                Some("playlists") => &theme::icon::LIST,
                _ if library.name.to_ascii_lowercase().contains("record") => &theme::icon::RECORD,
                _ => &theme::icon::LIBRARY,
            };
            nav = nav.child(
                nav_item(
                    &library.name,
                    &format!("library-{}", library.id),
                    selected,
                    t,
                    glyph,
                    collapsed,
                )
                .on_press(move |s: &mut State| {
                    if collection_type.as_deref() == Some("livetv") {
                        s.open_live_tv();
                    } else {
                        s.open_library(id.clone(), name.clone(), collection_type.clone());
                    }
                }),
            );
        }
    } else if !s.home_loaded {
        list = list
            .child(skeleton_bar(80.0, t).mt(14.0))
            .child(skeleton_bar(112.0, t))
            .child(skeleton_bar(94.0, t));
    }

    nav = nav.child(
        nav_item(
            "Watchlist",
            "nav-watchlist",
            matches!(&s.route, Route::Library { id } if id == "__watchlist__"),
            t,
            &theme::icon::HEART,
            collapsed,
        )
        .on_press(|s: &mut State| s.open_watchlist()),
    );

    if !s.playlists.is_empty() {
        if !collapsed {
            nav = nav.child(
                text("PLAYLISTS")
                    .key("playlists-label")
                    .font(type_::section())
                    .color(t.text_secondary)
                    .mt(14.0)
                    .mb(2.0),
            );
        }
        for playlist in s.playlists.iter() {
            let id = playlist.id.clone();
            let name = playlist.name.clone();
            nav = nav.child(
                nav_item(
                    &playlist.name,
                    &format!("playlist-{}", playlist.id),
                    matches!(&s.route, Route::Library { id } if id == &playlist.id),
                    t,
                    &theme::icon::LIST,
                    collapsed,
                )
                .on_press(move |s: &mut State| s.open_playlist(id.clone(), name.clone())),
            );
        }
    }

    if !s.config.seerr_url.is_empty() {
        let item = nav_item(
            "Discover",
            "nav-seerr",
            matches!(s.route, Route::Seerr | Route::SeerrDetail { .. }),
            t,
            &theme::icon::COMPASS,
            collapsed,
        )
        .on_press(|s: &mut State| s.open_seerr(crate::state::SeerrTab::Discover));
        nav = nav.child(item);
    }

    if !collapsed {
        nav = nav.child(
            text("TOOLS")
                .key("tools-label")
                .font(type_::section())
                .color(t.text_secondary)
                .mt(8.0)
                .mb(2.0),
        );
    }
    nav = nav
        .child(
            nav_item(
                "Search",
                "nav-search",
                matches!(s.route, Route::Search { .. }),
                t,
                &theme::icon::SEARCH,
                collapsed,
            )
            .on_press(|s: &mut State| s.open_search(None)),
        )
        .child(
            nav_item(
                "Remote",
                "nav-remote",
                matches!(s.route, Route::Remote),
                t,
                &theme::icon::REMOTE,
                collapsed,
            )
            .on_press(|s: &mut State| s.open_remote()),
        )
        .children(s.big_screen.then(|| {
            nav_item(
                "Controls",
                "nav-controls",
                s.shortcuts_open,
                t,
                &theme::icon::GAMEPAD,
                collapsed,
            )
            .on_press(|s: &mut State| s.toggle_shortcuts())
        }));

    list = list.child(
        el().key("sidebar-nav-scroll")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .scroll_edge_fade(16.0)
            .child(nav),
    );
    list = list
        .child(
            el().key("sidebar-footer-separator")
                .w_full()
                .h(1.0)
                .mt(6.0)
                .mb(4.0)
                .fill(t.separator),
        )
        .child(
            nav_item(
                "Settings",
                "nav-settings",
                matches!(s.route, Route::Settings),
                t,
                &theme::icon::SETTINGS,
                collapsed,
            )
            .on_press(|s: &mut State| s.open_settings()),
        );

    // The TV keeps the bottom row inside the safe area: the account is in Settings, and a
    // second profile chip would push the Tools section past 720 points on a full server.
    list.children((!s.big_screen).then(|| {
        row()
            .key("user-name")
            .w_full()
            .gap(8.0)
            .child(
                el().w(28.0)
                    .h(28.0)
                    .radius(14.0)
                    .center()
                    .fill(t.card)
                    .child(
                        text(
                            s.config
                                .user_name
                                .chars()
                                .next()
                                .map_or_else(|| "?".to_string(), |c| c.to_uppercase().to_string()),
                        )
                        .font(type_::caption())
                        .color(t.text_secondary),
                    ),
            )
            .children((!collapsed).then(|| {
                text(s.config.user_name.clone())
                    .font(type_::meta())
                    .color(t.text_secondary)
            }))
    }))
}

fn nav_item(
    label: &str,
    key: &str,
    selected: bool,
    t: &tok::Theme,
    glyph: &'static Icon,
    collapsed: bool,
) -> El {
    let mut item = el()
        .key(key)
        .role("button")
        .h(36.0)
        .px(if collapsed { 0.0 } else { 10.0 })
        .gap(10.0)
        .radius(8.0)
        .center_y()
        .fill(if selected {
            tok::accent(0, true)
        } else {
            rune::Color::TRANSPARENT
        })
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.98)
        .cursor(rune::Cursor::Pointer)
        .label(label.to_string())
        .help(label.to_string())
        .child(icon(glyph, 18.0).color(if selected {
            if theme::is_light() { t.text } else { t.bg }
        } else {
            t.text_secondary
        }));
    if selected {
        // The nav scroll clips its contents, so an outer halo ends on a hard edge at the
        // viewport sides. A contained top highlight lights the pill without an edge to cut.
        item = item
            .box_shadow(rune::Shadow {
                y: 1.0,
                blur: 0.0,
                spread: 0.0,
                color: Color::WHITE.alpha(0.12),
                inset: true,
                ..rune::Shadow::default()
            })
            .box_shadow(rune::Shadow {
                y: -1.0,
                blur: 0.0,
                spread: 0.0,
                color: Color::BLACK.alpha(0.28),
                inset: true,
                ..rune::Shadow::default()
            });
    }
    if collapsed {
        item = item.w(60.0).center();
    } else {
        item = item.child(text(label).font(type_::body()).color(if selected {
            // The light theme keeps the L 0.66 blue for the selected pill; dark text on it
            // reaches 5.45:1, while the old near-white label was 2.78:1.
            if theme::is_light() { t.text } else { t.bg }
        } else {
            t.text_secondary
        }));
    }
    // A selected pill is already accent-filled; a 10% wash disappears into it, so focus lifts
    // it a little further. This is still a fill, not a ring.
    let item = theme::tv_focus(item);
    if selected && theme::is_big_screen() {
        item.focus_fill(Color::WHITE.alpha(0.20))
    } else {
        item
    }
}

fn content(s: &State, t: &tok::Theme) -> El {
    let subtitle = if s.home_data.resume.is_empty() {
        "Your library, recently added"
    } else {
        "Pick up where you left off"
    };
    let header = row()
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
                .child(theme::readable_text("Home", hero_style(), t.text).key("home-title"))
                .child(theme::readable_text(
                    subtitle,
                    subtitle_style(),
                    t.text_secondary,
                )),
        );

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
            .px(theme::gutter())
            .pb(if s.in_app_minimized { 124.0 } else { 32.0 })
            .gap(theme::RAIL_GAP)
            .children((s.home_hero && !s.home_data.resume.is_empty()).then(|| home_hero(s, t)))
            .child(rails(s, t))
    };

    let mut page = col()
        .key("home-content")
        .grow(1.0)
        .min_w(0.0)
        .min_h(0.0)
        .h_full()
        .fill(t.bg)
        .child(ambient(s, t));
    // A lit band under the header: the top of the page catches the light, the rails stay on
    // the page's own black. No artwork — one gradient, clipped to its own box.
    let band = el()
        .key("home-band")
        .absolute(0.0, 0.0)
        .w_full()
        .h(340.0)
        .linear_gradient(
            std::f32::consts::PI,
            [
                (0.0, t.card.alpha(0.75)),
                (0.55, t.card.alpha(0.32)),
                (1.0, Color::TRANSPARENT),
            ],
        );
    page = page.child(band).child(header).child(body);
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
/// action. It is off by default so the rails stay the first thing on screen.
fn home_hero(s: &State, t: &tok::Theme) -> El {
    let item = &s.home_data.resume[0];
    let image = s
        .home_backdrop_key()
        .and_then(|key| poster_layers(s, &key, SurfaceFit::Fill))
        .unwrap_or_else(|| el().w_full().h_full().fill(t.card));
    let title = item
        .series_name
        .as_deref()
        .unwrap_or(&item.name)
        .to_string();
    let subtitle = crate::home::resume_subtitle(item);
    let id = item.series_id.clone().unwrap_or_else(|| item.id.clone());
    let play = item.clone();
    el().key("home-hero")
        .w_full()
        .h(180.0)
        .radius(14.0)
        .clip_rounded(14.0)
        .fill(t.card)
        .surface_sheen()
        .outline(1.0, 0.0, t.card_border)
        .shadow(1.0, 2.0, t.shadow)
        .shadow(8.0, 24.0, t.shadow_soft)
        .child(
            el().absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .opacity(0.55)
                .child(image),
        )
        .child(el().absolute(0.0, 0.0).w_full().h_full().linear_gradient(
            std::f32::consts::FRAC_PI_2,
            [
                (0.0, t.bg.alpha(0.95)),
                (0.55, t.bg.alpha(0.55)),
                (1.0, t.bg.alpha(0.20)),
            ],
        ))
        .child(
            col()
                .absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .padding(22.0)
                .justify_center()
                .gap(8.0)
                .items_start()
                .child(
                    text("Featured")
                        .font(type_::caption())
                        .color(tok::accent(0, true)),
                )
                .child(theme::readable_text(title, type_::section(), t.text).key("home-hero-title"))
                .child(
                    theme::readable_text(subtitle, type_::meta(), t.text_secondary)
                        .key("home-hero-subtitle"),
                )
                .child(
                    row()
                        .gap(8.0)
                        .child(
                            el().key("home-hero-play")
                                .role("button")
                                .h(36.0)
                                .px(16.0)
                                .radius(9.0)
                                .center()
                                .fill(tok::accent(0, true))
                                .gradient(Color::WHITE.alpha(0.14), Color::TRANSPARENT)
                                .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
                                .shadow(4.0, 12.0, tok::accent(0, true).alpha(0.35))
                                .hover(t.hover)
                                .focusable(true)
                                .focus_visual(2.0)
                                .cursor(rune::Cursor::Pointer)
                                .child(text("Resume").font(type_::label()).color(t.bg))
                                .on_press(move |s: &mut State| s.play_episode(play.clone())),
                        )
                        .child(
                            el().key("home-hero-open")
                                .role("button")
                                .h(36.0)
                                .px(14.0)
                                .radius(9.0)
                                .center()
                                .fill(t.card)
                                .gradient(Color::WHITE.alpha(0.055), Color::TRANSPARENT)
                                .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
                                .shadow(2.0, 6.0, Color::BLACK.alpha(0.18))
                                .hover(t.hover)
                                .focusable(true)
                                .focus_visual(2.0)
                                .cursor(rune::Cursor::Pointer)
                                .child(text("Details").font(type_::label()).color(t.text))
                                .on_press(move |s: &mut State| s.open_detail(id.clone())),
                        ),
                ),
        )
}

fn rails(s: &State, t: &tok::Theme) -> El {
    let mut list = col()
        .key("home-rails")
        .w_full()
        .min_w(0.0)
        .gap(theme::RAIL_GAP);
    if let Some(message) = &s.home_error {
        list = list.child(error_banner(message, t));
    }
    // Continue Watching -> Next Up -> every library's Recently Added -> Recently Played ->
    // Collections. A server with many libraries gets many rails; Home scrolls.
    let visible = |items: &[Item]| items.iter().any(|item| !s.is_home_hidden(&item.id));
    if s.home_data
        .resume
        .iter()
        .any(|item| !s.is_home_hidden(&item.id))
    {
        list = list.child(resume_rail(
            "rail-continue-watching",
            &s.home_data.resume,
            s,
            t,
        ));
    }
    if s.home_data
        .next_up
        .iter()
        .any(|item| !s.is_home_hidden(&item.id))
    {
        list = list.child(rail(
            "Next up",
            "rail-next-up",
            &s.home_data.next_up,
            s,
            t,
            None,
            Some(crate::state::HomeShelf::NextUp),
        ));
    }
    if visible(&s.home_data.on_now) {
        list = list.child(on_now_rail(s, t));
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
        list = list.child(rail(
            &format!("Recently added in {}", recent.library),
            &format!("rail-recent-{index}"),
            &recent.items,
            s,
            t,
            library,
            None,
        ));
    }
    if s.home_data
        .recently_played
        .iter()
        .any(|item| !s.is_home_hidden(&item.id))
    {
        list = list.child(rail(
            "Recently Played",
            "rail-recently-played",
            &s.home_data.recently_played,
            s,
            t,
            None,
            Some(crate::state::HomeShelf::RecentlyPlayed),
        ));
    }
    if !s.home_data.collections.is_empty() {
        // The server exposes its box sets as a library too; "See all" opens that rather than a
        // synthetic list, so the rail and the library can never disagree.
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
        list = list.child(rail(
            "Collections",
            "rail-collections",
            &s.home_data.collections,
            s,
            t,
            collections_library,
            None,
        ));
    }
    list
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
                .motion(motion::SNAPPY)
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
                .motion(motion::SNAPPY)
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

fn resume_card(item: &Item, s: &State, t: &tok::Theme, highlight: bool) -> El {
    let key = s.resume_image_key(item);
    let image = key
        .as_deref()
        .and_then(|key| poster_layers(s, key, SurfaceFit::Fill))
        .unwrap_or_else(|| placeholder(item, t));
    let title = item.series_name.as_deref().unwrap_or(&item.name);
    let id = item.series_id.clone().unwrap_or_else(|| item.id.clone());
    let focus_key = format!("rail-continue-watching-{id}");
    let watched = crate::home::progress(item);
    let mut art = el()
        .w(RESUME_W)
        .h(RESUME_H)
        .radius(12.0)
        .clip_rounded(12.0)
        .outline(
            if highlight { 1.5 } else { 0.0 },
            0.0,
            tok::accent(0, true).alpha(0.72),
        )
        .fill(t.card)
        .shadow(
            0.0,
            if highlight { 18.0 } else { 12.0 },
            if highlight {
                tok::accent(0, true).alpha(0.30)
            } else {
                t.shadow_soft
            },
        )
        .matched(format!("poster-{id}"))
        .child(image)
        .child(el().absolute(0.0, 0.0).w_full().h_full().linear_gradient(
            std::f32::consts::PI,
            [
                (0.0, rune::Color::TRANSPARENT),
                (0.55, rune::Color::BLACK.alpha(0.12)),
                (1.0, rune::Color::BLACK.alpha(0.82)),
            ],
        ))
        .child(
            el().absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .linear_gradient(
                    std::f32::consts::FRAC_PI_4,
                    [
                        (0.0, rune::Color::WHITE.alpha(0.09)),
                        (0.45, rune::Color::TRANSPARENT),
                    ],
                )
                .group_opacity(0.0, 1.0),
        )
        .child(
            el().absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .radius(12.0)
                .outline(2.0, -1.0, rune::Color::WHITE.alpha(0.62))
                .group_opacity(0.0, 1.0),
        )
        .child(
            col()
                .absolute(12.0, RESUME_H - 66.0)
                .w(RESUME_W - 24.0)
                .gap(2.0)
                .child(
                    text(title.to_string())
                        .font(type_::card_title())
                        .color(rune::Color::WHITE)
                        .truncate(),
                )
                .child(
                    text(crate::home::resume_subtitle(item))
                        .font(type_::caption())
                        .color(rune::Color::WHITE.alpha(0.72))
                        .truncate(),
                ),
        );
    let remove_id = item.id.clone();
    art = art.child(
        el().key(format!("resume-remove-{remove_id}"))
            .role("button")
            .absolute(RESUME_W - 34.0, 8.0)
            .w(24.0)
            .h(24.0)
            .radius(12.0)
            .center()
            .fill(rune::Color::BLACK.alpha(0.55))
            .hover(rune::Color::WHITE.alpha(0.18))
            .motion(motion::SNAPPY)
            .focusable(true)
            .focus_visual(2.0)
            .press_scale(0.92)
            .group_opacity(0.0, 1.0)
            .label("Remove from Continue Watching")
            .help("Remove from Continue Watching")
            .cursor(rune::Cursor::Pointer)
            .child(text("×").font(type_::label()).color(rune::Color::WHITE))
            .on_press(move |s: &mut State| s.remove_from_resume(&remove_id)),
    );
    if watched > 0.0 {
        let filled = RESUME_W * watched;
        art = art.child(
            el().absolute(0.0, RESUME_H - 4.0)
                .w(filled)
                .h(4.0)
                .fill(tok::accent(0, true))
                .shadow(0.0, 6.0, tok::accent(0, true).alpha(0.35))
                .child(
                    el().absolute(0.0, 0.0)
                        .w_full()
                        .h(1.0)
                        .fill(rune::Color::WHITE.alpha(0.35)),
                )
                .children((filled > 8.0).then(|| {
                    el().absolute(filled - 4.0, 0.0)
                        .w(4.0)
                        .h(4.0)
                        .radius(2.0)
                        .fill(rune::Color::WHITE)
                })),
        );
    }
    art = art.on_drawn_layout_tx({
        let key = focus_key.clone();
        move |s: &mut State, rect: [f32; 4], tx: &mut rune::Tx| {
            if s.focused_key.as_deref() == Some(key.as_str()) && s.focused_card_rect != Some(rect) {
                s.focused_card_rect = Some(rect);
                tx.wake_after(std::time::Duration::from_millis(0));
            }
        }
    });
    el().key(focus_key.clone())
        .role("button")
        .w(RESUME_W)
        .hover_group_focus()
        .hover_motion(motion::SNAPPY, motion::SNAPPY)
        .focusable(true)
        .focus_lift_only(4.0)
        .press_scale(0.985)
        .motion(motion::SNAPPY)
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut State| s.open_detail(id.clone()))
        .child(art)
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
        rail = rail.child(
            col()
                .key(format!("rail-on-now-{}", channel.id))
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
                .motion(motion::SNAPPY)
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
                                .color(tok::accent(2, true)),
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
                ),
        );
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

fn resume_rail(key: &str, items: &[Item], s: &State, t: &tok::Theme) -> El {
    col()
        .key(key)
        .w_full()
        .min_w(0.0)
        .gap(10.0)
        .child(
            row()
                .w_full()
                .items_end()
                .gap(12.0)
                .child(
                    theme::readable_text("Continue Watching", type_::section(), t.text_secondary)
                        .key(format!("{key}-title"))
                        .grow(1.0),
                )
                .child(
                    el().key("rail-continue-watching-all")
                        .role("button")
                        .h(26.0)
                        .px(8.0)
                        .radius(6.0)
                        .center()
                        .fill(t.bg)
                        .hover(t.hover)
                        .motion(motion::SNAPPY)
                        .focusable(true)
                        .focus_visual(2.0)
                        .press_scale(0.96)
                        .child(text("See all").font(type_::label()).color(t.text_secondary))
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| {
                            s.open_shelf(crate::state::HomeShelf::ContinueWatching)
                        }),
                ),
        )
        .child(
            el().w_full()
                .min_w(0.0)
                .scroll_x()
                .scroll_edge_fade(28.0)
                .child(
                    row().gap(theme::CARD_GAP).focus_group().children(
                        items
                            .iter()
                            .filter(|item| !s.is_home_hidden(&item.id))
                            .enumerate()
                            .map(|(index, item)| resume_card(item, s, t, index == 0)),
                    ),
                ),
        )
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
        .w(300.0)
        .padding(6.0)
        .gap(2.0)
        .radius(10.0)
        .child(
            text(menu.item.name.clone())
                .font(type_::caption())
                .color(t.text_secondary)
                .padding(8.0)
                .truncate(),
        )
        .child(el().w_full().h(1.0).fill(t.separator).mb(2.0))
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
        panel = panel.child(menu_row(
            "Add to playlist…",
            "card-menu-playlist",
            &t,
            |s: &mut State| s.card_menu_add_to_playlist(),
        ));
        panel = panel.child(menu_row(
            "Add to collection…",
            "card-menu-collection-add",
            &t,
            |s: &mut State| s.card_menu_add_to_collection(),
        ));
        panel = panel.child(menu_row(
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
        panel = panel.child(menu_row(
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
        panel = panel.child(menu_row(
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
        .w(380.0)
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
                        .motion(motion::SNAPPY)
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
                .motion(motion::SNAPPY)
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
            .motion(motion::SNAPPY)
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
            .motion(motion::SNAPPY)
            .focusable(true)
            .focus_visual(2.0)
            .cursor(rune::Cursor::Pointer)
            .press_scale(0.99)
            .on_press(on)
            .child(
                text(label.to_string())
                    .font(type_::label())
                    .color(t.text)
                    .truncate(),
            ),
    )
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
    let match_id = format!("poster-{detail_id}");
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
        .matched(match_id)
        .on_drawn_layout_tx({
            let key = focus_key.clone();
            move |s: &mut State, rect: [f32; 4], tx: &mut rune::Tx| {
                if s.focused_key.as_deref() == Some(key.as_str())
                    && s.focused_card_rect != Some(rect)
                {
                    s.focused_card_rect = Some(rect);
                    tx.wake_after(std::time::Duration::from_millis(0));
                }
            }
        })
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
    // A play button over the poster, on hover: the fastest path from a card to the film.
    // Containers (a Series) have no stream of their own, so they keep the plain card.
    if crate::state::is_playable(item) {
        let episode = item.clone();
        poster_box = poster_box.child(
            el().key(format!("play-{}", item.id))
                .role("button")
                .absolute(CARD_W - 46.0, CARD_H - 46.0)
                .w(36.0)
                .h(36.0)
                .radius(18.0)
                .center()
                .fill(tok::accent(0, true))
                .shadow(0.0, 8.0, rune::Color::BLACK.alpha(0.4))
                .group_opacity(0.0, 1.0)
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.94)
                .cursor(rune::Cursor::Pointer)
                .label("Play")
                .help("Play")
                .child(icon(&theme::icon::PLAY, 15.0).color(t.bg))
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
                    .motion(motion::SNAPPY)
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
                .motion(motion::SNAPPY)
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
                    tok::accent(0, true)
                } else {
                    rune::Color::BLACK.alpha(0.68)
                })
                .outline(1.0, 0.0, rune::Color::WHITE.alpha(0.35))
                .hover(t.hover)
                .motion(motion::SNAPPY)
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
                .absolute(8.0, 8.0)
                .w(24.0)
                .h(24.0)
                .radius(12.0)
                .center()
                .fill(tok::accent(0, true))
                .outline(1.0, 0.0, rune::Color::WHITE.alpha(0.18))
                .shadow(0.0, 4.0, rune::Color::BLACK.alpha(0.35))
                .child(text("✓").font(type_::caption()).color(t.bg)),
        );
    }
    let watched = crate::home::progress(item);
    if watched > 0.0 {
        poster_box = poster_box.child(
            el().absolute(0.0, CARD_H - 4.0)
                .w(CARD_W * watched)
                .h(4.0)
                .fill(tok::accent(0, true)),
        );
    }
    let title = item.series_name.as_deref().unwrap_or(&item.name);
    col()
        .key(focus_key.clone())
        .role("button")
        .w(CARD_W)
        .gap(6.0)
        .radius(10.0)
        .hover_group_focus()
        .hover_motion(motion::SNAPPY, motion::SNAPPY)
        .focusable(true)
        .focus_lift_only(4.0)
        .press_scale(0.985)
        .motion(motion::SNAPPY)
        .label(title.to_string())
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut State| s.open_detail(press_id.clone()))
        .on_secondary_press({
            let item = item.clone();
            let focus_key = focus_key.clone();
            move |s: &mut State, press: rune::PointerPress, _tx: &mut rune::Tx| {
                s.open_card_menu_at(item.clone(), press.x, press.y, Some(focus_key.clone()));
            }
        })
        .child(poster_box)
        .child(
            text(title.to_string())
                .font(type_::card_title())
                .color(t.text)
                .max_lines(2)
                .truncate(),
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
                .opacity_motion(motion::SNAPPY),
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
        .px(28.0)
        .pb(28.0)
        .gap(30.0)
        .child(
            text("Loading your library…")
                .key("home-loading-label")
                .font(subtitle_style())
                .color(t.text_secondary),
        )
        .children((0..3).map(|_| skeleton_rail(t)))
}

fn skeleton_rail(t: &tok::Theme) -> El {
    col()
        .w_full()
        .gap(12.0)
        .child(skeleton_bar(150.0, t))
        .child(
            row()
                .gap(CARD_GAP)
                .children((0..5).map(|_| el().w(CARD_W).h(CARD_H).radius(10.0).fill(t.card))),
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
                .color(tok::accent(2, true)),
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
                .color(tok::accent(2, true))
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
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.97)
        .child(text(label).font(type_::label()).color(t.text))
        .cursor(rune::Cursor::Pointer)
        .on_press(|s: &mut State| s.load_home())
}
