//! Settings: the facts about this install, plus the two actions that belong here.

use rune::tok;

use crate::theme::FocusVisual;
use crate::theme::SurfaceStyle;
use crate::theme::{self, type_};
use rune::{Color, El, col, el, row, text, text_input};

use crate::state::State;

use super::sidebar;

/// Text fields keep Left/Right for their caret, but on a TV Up/Down must leave the field for
/// the form. Rune's input consumes all four arrows; this hands Up/Down to the zone router, so
/// the move follows the visible rows rather than the page's tab order.
fn arrows_leave(input: El) -> El {
    input.on_input_key(|s: &mut State, key: &rune::KeyEvent, tx: &mut rune::Tx| {
        s.big_screen
            && matches!(key.key, rune::Key::Up | rune::Key::Down)
            && crate::tv_key(s, key, tx)
    })
}

fn stepper_button(label: &str, key: &str, t: &tok::Theme, on: impl Fn(&mut State) + 'static) -> El {
    let big = crate::theme::is_big_screen();
    el().key(key)
        .role("button")
        .h(if big { 40.0 } else { 32.0 })
        .w(if big { 44.0 } else { 36.0 })
        .radius(8.0)
        .center()
        .fill(t.bg)
        .hover(t.hover)
        .motion(crate::theme::motion::CONTROL)
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.95)
        .cursor(rune::Cursor::Pointer)
        .on_press(on)
        .child(text(label.to_string()).font(type_::label()).color(t.text))
}

fn cycle_autoplay(s: &mut State) {
    s.cycle_autoplay_next();
}

fn cycle_seek(s: &mut State) {
    s.cycle_seek_step();
}

fn cycle_subtitle_mode(s: &mut State) {
    s.cycle_subtitle_mode();
}

fn cycle_subtitle_background(s: &mut State) {
    s.cycle_subtitle_background();
}

fn cycle_subtitle_font(s: &mut State) {
    s.cycle_subtitle_font();
}

fn cycle_subtitle_color(s: &mut State) {
    s.cycle_subtitle_color();
}

fn cycle_subtitle_outline(s: &mut State) {
    s.cycle_subtitle_outline();
}

fn cycle_subtitle_position(s: &mut State) {
    s.cycle_subtitle_position();
}

fn cycle_subtitle_style(s: &mut State) {
    s.cycle_subtitle_style();
}

fn cycle_subtitle_language(s: &mut State) {
    s.cycle_subtitle_language();
}

fn cycle_audio_language(s: &mut State) {
    s.cycle_audio_language();
}

/// The three SteamOS mode controls. Only the Media session sees them: the other two modes
/// own their own shell, so a mode button there would be a second way to do what Steam or
/// Plasma already does.
fn modes_section(s: &State, t: &tok::Theme) -> El {
    if !s.media {
        return el();
    }
    let action = |key: &str, label: &str, code: i32| {
        el().key(key)
            .role("button")
            .h(36.0)
            .px(14.0)
            .radius(8.0)
            .center()
            .fill(t.card)
            .hover(t.hover)
            .motion(crate::theme::motion::CONTROL)
            .focusable(true)
            .focus_visual(2.0)
            .press_scale(0.97)
            .cursor(rune::Cursor::Pointer)
            .child(text(label).font(type_::label()).color(t.text))
            .on_press(move |s: &mut State| s.request_exit(code))
    };
    col()
        .w_full()
        .gap(10.0)
        .mt(8.0)
        .child(
            text("SteamOS modes")
                .key("settings-section-modes")
                .font(type_::section())
                .color(t.text_secondary),
        )
        .child(
            row()
                .key("settings-mode-actions")
                .w_full()
                .gap(10.0)
                .child(action("mode-switch-desktop", "Switch to desktop", 43))
                .child(action("mode-switch-game", "Switch to game", 42))
                .child(action("mode-restart", "Restart app", 0)),
        )
        .child(
            text("Desktop is the stock Plasma session; Game is Steam. This Media session has no desktop behind it.")
                .font(type_::caption())
                .color(t.text_secondary),
        )
}

/// The section list on the left. It replaces the horizontal index: one column names every
/// section, and the right pane scrolls to it without losing the reader's place.
fn settings_nav(t: &tok::Theme) -> El {
    let rows = [
        (
            "Interface",
            "appearance",
            "settings-section-appearance",
            "setting-ui-mode-value",
        ),
        (
            "Playback",
            "playback",
            "settings-section-playback",
            "setting-autoplay-value",
        ),
        (
            "Subtitles",
            "subtitles",
            "settings-section-subtitles",
            "setting-subtitle-mode-value",
        ),
        (
            "Keyboard",
            "keyboard",
            "settings-section-keyboard",
            "setting-key-change-play-/-pause",
        ),
        (
            "Metadata",
            "metadata",
            "settings-section-metadata",
            "setting-omdb-key",
        ),
        (
            "Seerr",
            "seerr",
            "settings-section-seerr",
            "setting-seerr-url",
        ),
        (
            "Downloads",
            "downloads",
            "settings-section-downloads",
            "setting-downloads-path",
        ),
        (
            "Storage",
            "storage",
            "settings-section-storage",
            "settings-clear-images",
        ),
        (
            "History",
            "history",
            "settings-section-history",
            "settings-history-export",
        ),
        (
            "Account",
            "account",
            "settings-section-account",
            "settings-reset",
        ),
    ];
    let mut nav = col()
        .key("settings-nav")
        .role("list")
        .w(220.0)
        .h_full()
        .padding(24.0)
        .gap(2.0)
        .child(
            text("SETTINGS")
                .key("settings-nav-title")
                .font(type_::caption())
                .color(t.text_secondary)
                .mb(8.0),
        );
    for (label, id, target, first) in rows {
        let key = format!("settings-nav-{id}");
        nav = nav.child(
            el().key(key)
                .role("button")
                .label(label)
                .h(38.0)
                .px(12.0)
                .radius(8.0)
                .center_y()
                .fill(Color::TRANSPARENT)
                .hover(t.hover)
                .motion(crate::theme::motion::CONTROL)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.98)
                .cursor(rune::Cursor::Pointer)
                .child(text(label).font(type_::label()).color(t.text_secondary))
                .on_press_tx(move |_s: &mut State, tx: &mut rune::Tx| {
                    tx.focus(first, true);
                    tx.scroll_center(target);
                }),
        );
    }
    nav = nav.child(
        el().key("settings-nav-home-rails")
            .role("button")
            .label("Home rails")
            .h(38.0)
            .px(12.0)
            .radius(8.0)
            .center_y()
            .fill(Color::TRANSPARENT)
            .hover(t.hover)
            .motion(crate::theme::motion::CONTROL)
            .focusable(true)
            .focus_visual(2.0)
            .press_scale(0.98)
            .cursor(rune::Cursor::Pointer)
            .child(
                text("Home rails")
                    .font(type_::label())
                    .color(t.text_secondary),
            )
            .on_press(|s: &mut State| s.open_home_rails()),
    );
    nav
}

pub(super) fn settings_screen(s: &State) -> El {
    let t = theme::colors();
    let mut content = col()
        .key("settings-content")
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .max_w(940.0)
        .padding(40.0)
        .pb(if s.in_app_minimized { 124.0 } else { 40.0 })
        .gap(22.0)
        .scroll(rune::ScrollSpec {
            stick_bottom: false,
        })
        .child(text("Settings").font(type_::page()).color(t.text))
        .children(s.media.then(|| modes_section(s, &t)))
        .child(
            el().key("settings-search-box")
                .h(40.0)
                .w_full()
                .max_w(520.0)
                .radius(10.0)
                .px(12.0)
                .center_y()
                .fill(t.card)
                .outline(
                    if s.settings_query.focused { 1.5 } else { 1.0 },
                    0.0,
                    if s.settings_query.focused {
                        crate::theme::accent()
                    } else {
                        t.card_border
                    },
                )
                .box_shadow(rune::Shadow {
                    y: 0.0,
                    blur: 0.0,
                    spread: if s.settings_query.focused { 3.0 } else { 0.0 },
                    color: crate::theme::accent().alpha(if s.settings_query.focused {
                        0.18
                    } else {
                        0.0
                    }),
                    inset: false,
                    ..rune::Shadow::default()
                })
                .child(arrows_leave(
                    text_input(&s.settings_query, type_::body())
                        .key("settings-search")
                        .label("Search settings")
                        .placeholder("Search settings", t.text_secondary.alpha(0.55))
                        .role("textbox")
                        .single_line()
                        .w_full()
                        .on_edit(|s: &mut State| &mut s.settings_query)
                        .on_submit(|s: &mut State, _tx| {
                            s.keyboard_enter_open("settings-search");
                        })
                        .color(t.text),
                )),
        )
        .child(
            row()
                .key("settings-home-rails-row")
                .w_full()
                .gap(6.0)
                .child(
                    el().key("settings-home-rails")
                        .role("button")
                        .label("Home rails")
                        .help("Choose which libraries get their own Home rail")
                        .h(30.0)
                        .px(12.0)
                        .radius(7.0)
                        .center()
                        .fill(t.bg)
                        .outline(1.0, 0.0, t.card_border)
                        .hover(t.hover)
                        .motion(crate::theme::motion::CONTROL)
                        .focusable(true)
                        .focus_visual(2.0)
                        .press_scale(0.96)
                        .cursor(rune::Cursor::Pointer)
                        .child(text("Home rails").font(type_::label()).color(t.text))
                        .on_press(|s: &mut State| s.open_home_rails()),
                )
                .child(
                    text("Choose up to four libraries for their own Recently added rail.")
                        .font(type_::meta())
                        .color(t.text_secondary),
                ),
        )
        .child(
            text("This install")
                .key("settings-section-install")
                .font(type_::section())
                .color(t.text_secondary),
        );
    let query = s.settings_query.text.trim().to_ascii_lowercase();
    let shown = |label: &str| query.is_empty() || label.to_ascii_lowercase().contains(&query);
    let mut install = col()
        .w_full()
        .padding(18.0)
        .gap(14.0)
        .fill(t.card)
        .surface_sheen()
        .radius(12.0)
        .outline(1.0, 0.0, t.card_border)
        .box_shadow(rune::Shadow {
            y: 1.0,
            blur: 0.0,
            spread: 0.0,
            color: Color::WHITE.alpha(0.05),
            inset: true,
            ..rune::Shadow::default()
        })
        .shadow(1.0, 2.0, t.shadow)
        .shadow(6.0, 18.0, t.shadow_soft);
    for (key, label, value) in [
        ("setting-server", "Server", s.config.server_url.clone()),
        ("setting-user", "Signed in as", s.config.user_name.clone()),
        ("setting-device", "Device id", s.config.device_id.clone()),
        (
            "setting-version",
            "Version",
            env!("CARGO_PKG_VERSION").to_string(),
        ),
    ] {
        if !shown(label) {
            continue;
        }
        install = install.child(
            row()
                .key(key)
                .w_full()
                .gap(16.0)
                .child(
                    text(label)
                        .font(type_::label())
                        .color(t.text_secondary)
                        .w(130.0),
                )
                .child(text(value).font(type_::body()).color(t.text)),
        );
    }
    content = content.child(install);

    // Playback defaults: what the player does on its own before the viewer touches a menu.
    // They are written to the config on exit, with the subtitle size and delay.
    let mut playback = col()
        .w_full()
        .padding(18.0)
        .gap(10.0)
        .fill(t.card)
        .surface_sheen()
        .radius(12.0)
        .outline(1.0, 0.0, t.card_border)
        .box_shadow(rune::Shadow {
            y: 1.0,
            blur: 0.0,
            spread: 0.0,
            color: Color::WHITE.alpha(0.05),
            inset: true,
            ..rune::Shadow::default()
        })
        .shadow(1.0, 2.0, t.shadow)
        .shadow(6.0, 18.0, t.shadow_soft);
    // Subtitle and audio rows are a separate surface: they are about the mix, not what the
    // player does between episodes.
    let mut listening = col()
        .w_full()
        .padding(18.0)
        .gap(10.0)
        .fill(t.card)
        .surface_sheen()
        .radius(12.0)
        .outline(1.0, 0.0, t.card_border)
        .box_shadow(rune::Shadow {
            y: 1.0,
            blur: 0.0,
            spread: 0.0,
            color: Color::WHITE.alpha(0.05),
            inset: true,
            ..rune::Shadow::default()
        })
        .shadow(1.0, 2.0, t.shadow)
        .shadow(6.0, 18.0, t.shadow_soft);
    let mut appearance = col()
        .w_full()
        .padding(18.0)
        .gap(10.0)
        .fill(t.card)
        .surface_sheen()
        .radius(12.0)
        .outline(1.0, 0.0, t.card_border)
        .box_shadow(rune::Shadow {
            y: 1.0,
            blur: 0.0,
            spread: 0.0,
            color: Color::WHITE.alpha(0.05),
            inset: true,
            ..rune::Shadow::default()
        })
        .shadow(1.0, 2.0, t.shadow)
        .shadow(6.0, 18.0, t.shadow_soft);
    for (key, label, value, action) in [
        (
            "setting-ui-mode",
            "Interface",
            if s.shown_big_screen() {
                "Big screen"
            } else {
                "Desktop"
            }
            .to_string(),
            State::toggle_ui_mode as fn(&mut State),
        ),
        (
            "setting-keyboard",
            "On-screen keyboard",
            s.keyboard.mode.label().to_string(),
            State::cycle_keyboard_mode as fn(&mut State),
        ),
        (
            "setting-autoplay",
            "Autoplay next episode",
            if s.autoplay_next { "On" } else { "Off" }.to_string(),
            cycle_autoplay as fn(&mut State),
        ),
        (
            "setting-seek-step",
            "Seek step",
            format!("{} s", s.seek_step_ms / 1000),
            cycle_seek as fn(&mut State),
        ),
        (
            "setting-skip-segments",
            "Skip intro/recap/credits",
            s.skip_mode.label().to_string(),
            (|s: &mut State| s.cycle_skip_mode()) as fn(&mut State),
        ),
        (
            "setting-still-watching",
            "Still watching?",
            if s.still_watching { "On" } else { "Off" }.to_string(),
            (|s: &mut State| s.toggle_still_watching()) as fn(&mut State),
        ),
        (
            "setting-high-contrast",
            "High contrast",
            if s.high_contrast { "On" } else { "Off" }.to_string(),
            (|s: &mut State| s.toggle_high_contrast()) as fn(&mut State),
        ),
        (
            "setting-light-theme",
            "Theme",
            if s.light_theme { "Light" } else { "Dark" }.to_string(),
            (|s: &mut State| s.toggle_light_theme()) as fn(&mut State),
        ),
        (
            "setting-text-scale",
            "Text size",
            format!("{}%", (s.text_scale * 100.0).round() as i32),
            (|s: &mut State| s.cycle_text_scale()) as fn(&mut State),
        ),
        (
            "setting-home-hero",
            "Home billboard",
            if s.home_hero { "On" } else { "Off" }.to_string(),
            (|s: &mut State| s.toggle_home_hero()) as fn(&mut State),
        ),
        (
            "setting-hover-preview",
            "Hover previews",
            if s.hover_preview_enabled { "On" } else { "Off" }.to_string(),
            (|s: &mut State| s.toggle_hover_preview()) as fn(&mut State),
        ),
        (
            "setting-spoiler-guard",
            "Spoiler guard",
            if s.spoiler_guard { "On" } else { "Off" }.to_string(),
            (|s: &mut State| s.toggle_spoiler_guard()) as fn(&mut State),
        ),
        (
            "setting-reduce-motion",
            "Reduced motion",
            if s.reduce_motion { "On" } else { "Off" }.to_string(),
            (|s: &mut State| s.toggle_reduce_motion()) as fn(&mut State),
        ),
        (
            "setting-notifications",
            "Desktop notifications",
            if s.notifications { "On" } else { "Off" }.to_string(),
            (|s: &mut State| s.toggle_notifications()) as fn(&mut State),
        ),
        (
            "setting-audio-passthrough",
            "HDMI audio passthrough",
            if s.audio_passthrough { "On" } else { "Off" }.to_string(),
            (|s: &mut State| s.toggle_audio_passthrough()) as fn(&mut State),
        ),
        (
            "setting-hdr",
            "HDR output",
            s.hdr.label().to_string(),
            (|s: &mut State| s.toggle_hdr()) as fn(&mut State),
        ),
        (
            "setting-downmix",
            "Downmix to stereo",
            if s.downmix_stereo { "On" } else { "Off" }.to_string(),
            (|s: &mut State| s.toggle_downmix()) as fn(&mut State),
        ),
        (
            "setting-audio-mode",
            "Sound",
            s.audio_mode.label().to_string(),
            (|s: &mut State| s.cycle_audio_mode()) as fn(&mut State),
        ),
        (
            "setting-loudness",
            "Loudness",
            if s.loudness { "Normalised" } else { "Off" }.to_string(),
            (|s: &mut State| s.toggle_loudness()) as fn(&mut State),
        ),
        (
            "setting-resume-rewind",
            "Rewind on resume",
            format!("{} s", s.resume_rewind_seconds),
            (|s: &mut State| s.cycle_resume_rewind()) as fn(&mut State),
        ),
        (
            "setting-subtitle-mode",
            "Subtitles",
            s.subtitle_mode.label().to_string(),
            cycle_subtitle_mode as fn(&mut State),
        ),
        (
            "setting-subtitle-background",
            "Subtitle background",
            s.subtitle_background.label().to_string(),
            cycle_subtitle_background as fn(&mut State),
        ),
        (
            "setting-subtitle-font",
            "Subtitle font",
            s.subtitle_font.label().to_string(),
            cycle_subtitle_font as fn(&mut State),
        ),
        (
            "setting-subtitle-color",
            "Subtitle colour",
            s.subtitle_color.label().to_string(),
            cycle_subtitle_color as fn(&mut State),
        ),
        (
            "setting-subtitle-outline",
            "Subtitle outline",
            s.subtitle_outline.label().to_string(),
            cycle_subtitle_outline as fn(&mut State),
        ),
        (
            "setting-subtitle-style",
            "Subtitle style",
            s.subtitle_style.label().to_string(),
            cycle_subtitle_style as fn(&mut State),
        ),
        (
            "setting-subtitle-position",
            "Subtitle position",
            s.subtitle_position.label().to_string(),
            cycle_subtitle_position as fn(&mut State),
        ),
        (
            "setting-subtitle-language",
            "Subtitle language",
            State::language_label(&s.subtitle_language).to_string(),
            cycle_subtitle_language as fn(&mut State),
        ),
        (
            "setting-audio-language",
            "Audio language",
            State::language_label(&s.audio_language).to_string(),
            cycle_audio_language as fn(&mut State),
        ),
    ] {
        if !shown(label) {
            continue;
        }
        let listening_row = key.starts_with("setting-subtitle")
            || key == "setting-downmix"
            || key == "setting-audio-mode"
            || key == "setting-audio-language";
        let appearance_row = matches!(
            key,
            "setting-ui-mode"
                | "setting-keyboard"
                | "setting-high-contrast"
                | "setting-light-theme"
                | "setting-text-scale"
                | "setting-home-hero"
                | "setting-hover-preview"
                | "setting-reduce-motion"
        );
        let target = if listening_row {
            &mut listening
        } else if appearance_row {
            &mut appearance
        } else {
            &mut playback
        };
        let boolean = value == "On" || value == "Off";
        let control = theme::tv_focus(if boolean {
            let on = value == "On";
            let big = crate::theme::is_big_screen();
            el().key(format!("{key}-value"))
                .role("switch")
                .label(label.to_string())
                .checked(on)
                .w(if big { 56.0 } else { 38.0 })
                .h(if big { 32.0 } else { 22.0 })
                .radius(if big { 16.0 } else { 11.0 })
                .fill(if on {
                    crate::theme::accent()
                } else {
                    t.control_off
                })
                .motion(crate::theme::motion::CONTROL)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.97)
                .cursor(rune::Cursor::Pointer)
                .child(
                    el().absolute(
                        if on {
                            if big { 26.0 } else { 18.0 }
                        } else {
                            2.0
                        },
                        2.0,
                    )
                    .size(if big { 28.0 } else { 18.0 }, if big { 28.0 } else { 18.0 })
                    .radius(if big { 14.0 } else { 9.0 })
                    .fill(t.knob)
                    .shadow(0.0, 2.0, t.shadow),
                )
                .on_press(action)
        } else {
            el().key(format!("{key}-value"))
                .role("button")
                .h(if crate::theme::is_big_screen() {
                    40.0
                } else {
                    32.0
                })
                .min_w(if crate::theme::is_big_screen() {
                    180.0
                } else {
                    150.0
                })
                .px(14.0)
                .radius(10.0)
                .center()
                .fill(t.bg)
                .hover(t.hover)
                .motion(crate::theme::motion::CONTROL)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.97)
                .child(text(value).font(type_::label()).color(t.text))
                .child(
                    text("▾")
                        .font(type_::caption())
                        .color(t.text_secondary)
                        .ml(8.0),
                )
                .cursor(rune::Cursor::Pointer)
                .on_press(action)
        });
        *target = target.clone().child(
            row()
                .key(key)
                .w_full()
                .gap(16.0)
                .items_center()
                .child(
                    text(label)
                        .font(type_::label())
                        .color(t.text_secondary)
                        .w(220.0),
                )
                .child(el().grow(1.0))
                .child(
                    el().w(if crate::theme::is_big_screen() {
                        180.0
                    } else {
                        150.0
                    })
                    .justify_end()
                    .child(control),
                ),
        );
    }
    if s.ui_mode_restart && s.config.big_screen() != s.big_screen {
        appearance = appearance.child(
            text("Restart to apply.")
                .key("setting-ui-restart")
                .font(type_::caption())
                .color(t.text_secondary)
                .padding(2.0),
        );
    }
    for (key, label, value, down, up, action) in [
        (
            "setting-subtitle-opacity",
            "Subtitle opacity",
            format!("{}%", (s.subtitle_opacity * 100.0).round() as i32),
            "setting-subtitle-opacity-down",
            "setting-subtitle-opacity-up",
            (|s: &mut State| s.adjust_subtitle_opacity(-0.1)) as fn(&mut State),
        ),
        (
            "setting-subtitle-plate-opacity",
            "Subtitle plate opacity",
            format!(
                "{}%",
                (s.subtitle_background_opacity * 100.0).round() as i32
            ),
            "setting-subtitle-plate-opacity-down",
            "setting-subtitle-plate-opacity-up",
            (|s: &mut State| s.adjust_subtitle_background_opacity(-0.1)) as fn(&mut State),
        ),
    ] {
        if !shown(label) {
            continue;
        }
        let listening_row = key.starts_with("setting-subtitle")
            || key == "setting-downmix"
            || key == "setting-audio-mode"
            || key == "setting-audio-language";
        let target = if listening_row {
            &mut listening
        } else {
            &mut playback
        };
        *target = target.clone().child(
            row()
                .key(key)
                .w_full()
                .gap(16.0)
                .items_center()
                .child(
                    text(label)
                        .font(type_::label())
                        .color(t.text_secondary)
                        .w(220.0),
                )
                .child(el().grow(1.0))
                .child(
                    row()
                        .gap(8.0)
                        .items_center()
                        .child(stepper_button("−", down, &t, move |s: &mut State| {
                            action(s)
                        }))
                        .child(
                            text(value)
                                .key(format!("{key}-value"))
                                .font(type_::label())
                                .color(t.text)
                                .w(56.0),
                        )
                        .child(stepper_button("+", up, &t, move |s: &mut State| {
                            if key.starts_with("setting-subtitle-plate") {
                                s.adjust_subtitle_background_opacity(0.1)
                            } else {
                                s.adjust_subtitle_opacity(0.1)
                            }
                        })),
                ),
        );
    }
    playback = playback.child(
        row()
            .key("setting-volume")
            .w_full()
            .gap(16.0)
            .items_center()
            .child(
                text("Volume")
                    .font(type_::label())
                    .color(t.text_secondary)
                    .w(220.0),
            )
            .child(el().grow(1.0))
            .child(
                row()
                    .gap(8.0)
                    .items_center()
                    .child(stepper_button(
                        "−",
                        "setting-volume-down",
                        &t,
                        |s: &mut State| s.adjust_preferred_volume(-0.1),
                    ))
                    .child(
                        text(format!("{}%", (s.in_app_volume * 100.0).round() as i32))
                            .key("setting-volume-value")
                            .font(type_::label())
                            .color(t.text)
                            .w(56.0),
                    )
                    .child(stepper_button(
                        "+",
                        "setting-volume-up",
                        &t,
                        |s: &mut State| s.adjust_preferred_volume(0.1),
                    )),
            ),
    );
    if s.audio_passthrough {
        playback = playback.child(
            text("Needs an HDMI receiver that decodes the source format. Bypasses downmix and sound modes; volume is controlled by the receiver. Takes effect on the next play.")
                .key("setting-passthrough-note")
                .font(type_::caption())
                .color(t.text_secondary)
                .padding(2.0),
        );
    }
    let hdr_note = match s.hdr_status {
        rune::HdrStatus::Active {
            format,
            color_space,
        } => format!(
            "Active: {format}, {color_space}. The compositor owns tone mapping and HDR metadata."
        ),
        rune::HdrStatus::Unsupported => {
            "This display or compositor offers no HDR surface; SDR is used.".to_string()
        }
        rune::HdrStatus::Failed => "The driver refused the HDR surface; SDR is used.".to_string(),
        _ if s.hdr == crate::config::HdrMode::Auto => {
            "Uses extended-range HDR when the display and compositor offer it; otherwise SDR."
                .to_string()
        }
        _ => "Always SDR. Auto uses HDR when the display offers it.".to_string(),
    };
    playback = playback.child(
        text(hdr_note)
            .key("setting-hdr-note")
            .font(type_::caption())
            .color(t.text_secondary)
            .padding(2.0),
    );
    content = content
        .child(
            text("Appearance")
                .key("settings-section-appearance")
                .font(type_::section())
                .color(t.text_secondary)
                .mt(4.0),
        )
        .child(appearance)
        .child(
            text("Playback")
                .key("settings-section-playback")
                .font(type_::section())
                .color(t.text_secondary)
                .mt(4.0),
        )
        .child(playback)
        .child(
            text("Subtitles & audio")
                .key("settings-section-subtitles")
                .font(type_::section())
                .color(t.text_secondary)
                .mt(4.0),
        )
        .child(listening);

    // Player shortcuts are remappable; navigation keys stay fixed so a bad binding cannot
    // trap the window. A duplicate is refused by the state, not silently overwritten.
    let mut keyboard = col()
        .key("settings-keyboard")
        .w_full()
        .padding(18.0)
        .gap(10.0)
        .fill(t.card)
        .surface_sheen()
        .radius(12.0)
        .outline(1.0, 0.0, t.card_border)
        .box_shadow(rune::Shadow {
            y: 1.0,
            blur: 0.0,
            spread: 0.0,
            color: Color::WHITE.alpha(0.05),
            inset: true,
            ..rune::Shadow::default()
        })
        .shadow(1.0, 2.0, t.shadow)
        .shadow(6.0, 18.0, t.shadow_soft);
    for action in crate::shortcuts::Action::ALL {
        let waiting = s.rebinding == Some(action);
        let binding = crate::shortcuts::display(s.key_bindings.get(action));
        keyboard = keyboard.child(
            row()
                .key(format!(
                    "setting-key-{}",
                    action.label().to_lowercase().replace(' ', "-")
                ))
                .w_full()
                .gap(16.0)
                .items_center()
                .child(
                    text(action.label())
                        .font(type_::label())
                        .color(t.text_secondary)
                        .w(220.0),
                )
                .child(el().grow(1.0))
                .child(
                    el().key(format!(
                        "setting-key-value-{}",
                        action.label().to_lowercase().replace(' ', "-")
                    ))
                    .h(30.0)
                    .min_w(92.0)
                    .px(12.0)
                    .radius(15.0)
                    .center()
                    .fill(t.bg)
                    .outline(1.0, 0.0, t.card_border)
                    .child(
                        text(if waiting {
                            "Press a key…".to_string()
                        } else {
                            binding
                        })
                        .font(type_::mono_caption())
                        .color(if waiting {
                            tok::accent(3, true)
                        } else {
                            t.text
                        }),
                    ),
                )
                .child(
                    el().key(format!(
                        "setting-key-change-{}",
                        action.label().to_lowercase().replace(' ', "-")
                    ))
                    .role("button")
                    .h(30.0)
                    .px(10.0)
                    .radius(8.0)
                    .center()
                    .fill(if waiting {
                        tok::accent(3, true).alpha(0.18)
                    } else {
                        t.bg
                    })
                    .hover(t.hover)
                    .motion(crate::theme::motion::CONTROL)
                    .focusable(true)
                    .focus_visual(2.0)
                    .press_scale(0.97)
                    .cursor(rune::Cursor::Pointer)
                    .child(
                        text(if waiting { "Cancel" } else { "Change" })
                            .font(type_::caption())
                            .color(t.text),
                    )
                    .on_press(move |s: &mut State| {
                        if s.rebinding == Some(action) {
                            s.cancel_rebind();
                        } else {
                            s.begin_rebind(action);
                        }
                    }),
                ),
        );
    }
    keyboard = keyboard
        .child(
            text("Shortcuts apply while a video is open. Navigation keys are fixed.")
                .font(type_::caption())
                .color(t.text_secondary),
        )
        .child(
            el().key("settings-shortcuts-reset")
                .role("button")
                .h(32.0)
                .w(160.0)
                .radius(8.0)
                .center()
                .fill(t.bg)
                .hover(t.hover)
                .motion(crate::theme::motion::CONTROL)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.97)
                .cursor(rune::Cursor::Pointer)
                .child(text("Reset shortcuts").font(type_::label()).color(t.text))
                .on_press(|s: &mut State| s.reset_shortcuts()),
        );
    content = content
        .child(
            text("Keyboard")
                .key("settings-section-keyboard")
                .font(type_::section())
                .color(t.text_secondary)
                .mt(4.0),
        )
        .child(keyboard);

    let key_input = arrows_leave(
        text_input(&s.omdb_key, type_::body())
            .key("setting-omdb-key")
            .label("OMDb API key")
            .role("textbox")
            .single_line()
            .secure()
            .w_full()
            .on_edit(|s: &mut State| &mut s.omdb_key)
            .on_submit(|s: &mut State, _tx| {
                s.keyboard_enter_open("setting-omdb-key");
            })
            .color(t.text),
    );
    let metadata = col()
        .key("settings-metadata")
        .w_full()
        .padding(18.0)
        .gap(10.0)
        .fill(t.card)
        .surface_sheen()
        .radius(12.0)
        .outline(1.0, 0.0, t.card_border)
        .box_shadow(rune::Shadow {
            y: 1.0,
            blur: 0.0,
            spread: 0.0,
            color: Color::WHITE.alpha(0.05),
            inset: true,
            ..rune::Shadow::default()
        })
        .shadow(1.0, 2.0, t.shadow)
        .shadow(6.0, 18.0, t.shadow_soft)
        .child(
            row()
                .w_full()
                .gap(16.0)
                .items_center()
                .child(
                    text("OMDb fallback key")
                        .font(type_::label())
                        .color(t.text_secondary)
                        .w(220.0),
                )
                .child(
                    el().key("setting-omdb-box")
                        .grow(1.0)
                        .h(36.0)
                        .radius(8.0)
                        .px(10.0)
                        .center_y()
                        .fill(t.bg)
                        .outline(1.0, 0.0, t.card_border)
                        .child(key_input),
                ),
        )
        .child(
            row()
                .w_full()
                .items_center()
                .gap(12.0)
                .child(
                    text(
                        "IMDb scores come from the public IMDb dataset. An OMDb key is only a \
                         fallback when that download fails; it is stored in the 0600 config.",
                    )
                    .font(type_::caption())
                    .color(t.text_secondary)
                    .grow(1.0)
                    .min_w(0.0)
                    .max_lines(2),
                )
                .child(
                    el().key("setting-omdb-get-key")
                        .role("button")
                        .h(24.0)
                        .px(8.0)
                        .radius(6.0)
                        .center()
                        .fill(t.bg)
                        .hover(t.hover)
                        .motion(crate::theme::motion::CONTROL)
                        .focusable(true)
                        .focus_visual(2.0)
                        .press_scale(0.97)
                        .child(
                            text("Get a key")
                                .font(type_::caption())
                                .color(crate::theme::accent()),
                        )
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| {
                            let _ = s;
                            std::thread::spawn(|| {
                                let _ = State::open_url("https://www.omdbapi.com/apikey.aspx");
                            });
                        }),
                ),
        );
    content = content
        .child(
            text("Metadata")
                .key("settings-section-metadata")
                .font(type_::section())
                .color(t.text_secondary)
                .mt(4.0),
        )
        .child(metadata);

    // The optional companion. It sits between metadata and storage because it is a service
    // setting, not a playback one, and the key is a credential with the same 0600 rules.
    let seerr_shown =
        shown("Requests (Seerr)") || shown("Seerr server") || shown("API key") || shown("User id");
    if seerr_shown {
        let status = if s.seerr.testing {
            "Testing…".to_string()
        } else if let Some(probe) = &s.seerr.probe {
            format!("Connected · Seerr {} · {}", probe.version, probe.user)
        } else if let Some(error) = &s.seerr.error {
            error.clone()
        } else {
            "Not connected. Seerr is optional.".to_string()
        };
        let url_input = arrows_leave(
            text_input(&s.seerr.url, type_::body())
                .key("setting-seerr-url")
                .label("Seerr server")
                .role("textbox")
                .single_line()
                .w_full()
                .on_edit(|s: &mut State| &mut s.seerr.url)
                .on_submit(|s: &mut State, _tx| {
                    s.keyboard_enter_open("setting-seerr-url");
                })
                .color(t.text),
        );
        // A secure field draws one bullet per character, and the platform text overlay is not
        // clipped by the box: a 68-character key ran past the card. Show a fixed mask until
        // the reader clicks it, then hand over to the real field.
        let key_input = arrows_leave(if s.seerr.key.focused {
            text_input(&s.seerr.key, type_::body())
                .key("setting-seerr-key-input")
                .label("Seerr API key")
                .role("textbox")
                .single_line()
                .secure()
                .w_full()
                .on_edit(|s: &mut State| &mut s.seerr.key)
                .on_submit(|s: &mut State, _tx| {
                    s.keyboard_enter_open("setting-seerr-key-input");
                })
                .color(t.text)
        } else {
            let label = if s.seerr.key.text.is_empty() {
                "No key set".to_string()
            } else {
                "•".repeat(12)
            };
            el().key("setting-seerr-key")
                .role("button")
                .w_full()
                .h(24.0)
                .items_center()
                .cursor(rune::Cursor::Pointer)
                .label("Seerr API key")
                .on_press(|s: &mut State| s.seerr.key.focused = true)
                .child(text(label).font(type_::body()).color(t.text_secondary))
        });
        let user_input = arrows_leave(
            text_input(&s.seerr.user, type_::body())
                .key("setting-seerr-user")
                .label("Seerr user id")
                .role("textbox")
                .single_line()
                .w_full()
                .on_edit(|s: &mut State| &mut s.seerr.user)
                .color(t.text),
        );
        let seerr = col()
            .key("settings-seerr")
            .w_full()
            .padding(18.0)
            .gap(10.0)
            .fill(t.card)
            .surface_sheen()
            .radius(12.0)
            .outline(1.0, 0.0, t.card_border)
            .box_shadow(rune::Shadow {
                y: 1.0,
                blur: 0.0,
                spread: 0.0,
                color: Color::WHITE.alpha(0.05),
                inset: true,
                ..rune::Shadow::default()
            })
            .shadow(1.0, 2.0, t.shadow)
            .shadow(6.0, 18.0, t.shadow_soft)
            .child(
                text(status)
                    .key("setting-seerr-status")
                    .font(type_::meta())
                    .color(t.text_secondary),
            )
            .child(
                row()
                    .key("setting-seerr-url-row")
                    .w_full()
                    .gap(16.0)
                    .items_center()
                    .child(
                        text("Seerr server")
                            .font(type_::label())
                            .color(t.text_secondary)
                            .w(220.0),
                    )
                    .child(
                        el().key("setting-seerr-url-box")
                            .grow(1.0)
                            .h(36.0)
                            .radius(8.0)
                            .clip_rounded(8.0)
                            .px(10.0)
                            .center_y()
                            .fill(t.bg)
                            .outline(1.0, 0.0, t.card_border)
                            .child(url_input),
                    ),
            )
            .child(
                row()
                    .key("setting-seerr-key-row")
                    .w_full()
                    .gap(16.0)
                    .items_center()
                    .child(
                        text("API key")
                            .font(type_::label())
                            .color(t.text_secondary)
                            .w(220.0),
                    )
                    .child(
                        el().key("setting-seerr-key-box")
                            .grow(1.0)
                            .h(36.0)
                            .radius(8.0)
                            .clip_rounded(8.0)
                            .px(10.0)
                            .center_y()
                            .fill(t.bg)
                            .outline(1.0, 0.0, t.card_border)
                            .child(key_input),
                    ),
            )
            .child(
                row()
                    .key("setting-seerr-user-row")
                    .w_full()
                    .gap(16.0)
                    .items_center()
                    .child(
                        text("User id")
                            .font(type_::label())
                            .color(t.text_secondary)
                            .w(220.0),
                    )
                    .child(
                        el().key("setting-seerr-user-box")
                            .grow(1.0)
                            .h(36.0)
                            .radius(8.0)
                            .clip_rounded(8.0)
                            .px(10.0)
                            .center_y()
                            .fill(t.bg)
                            .outline(1.0, 0.0, t.card_border)
                            .child(user_input),
                    ),
            )
            .child(
                row()
                    .w_full()
                    .gap(10.0)
                    .focus_group()
                    .child(
                        el().key("setting-seerr-test")
                            .role("button")
                            .h(36.0)
                            .w(120.0)
                            .radius(8.0)
                            .center()
                            .fill(crate::theme::accent())
                            .gradient(Color::WHITE.alpha(0.18), Color::TRANSPARENT)
                            .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
                            .shadow(4.0, 12.0, crate::theme::accent().alpha(0.35))
                            .hover(t.hover)
                            .motion(crate::theme::motion::CONTROL)
                            .focusable(true)
                            .focus_visual(2.0)
                            .press_scale(0.97)
                            .child(text("Test").font(type_::label()).color(theme::on_accent()))
                            .cursor(rune::Cursor::Pointer)
                            .on_press(|s: &mut State| s.test_seerr()),
                    )
                    .child(
                        el().key("setting-seerr-clear")
                            .role("button")
                            .h(36.0)
                            .w(120.0)
                            .radius(8.0)
                            .center()
                            .fill(t.bg)
                            .hover(t.hover)
                            .motion(crate::theme::motion::CONTROL)
                            .focusable(true)
                            .focus_visual(2.0)
                            .press_scale(0.97)
                            .child(text("Clear key").font(type_::label()).color(t.text))
                            .cursor(rune::Cursor::Pointer)
                            .on_press(|s: &mut State| s.clear_seerr_key()),
                    )
                    .child(
                        el().key("setting-seerr-open")
                            .role("button")
                            .h(36.0)
                            .w(150.0)
                            .radius(8.0)
                            .center()
                            .fill(t.bg)
                            .hover(t.hover)
                            .motion(crate::theme::motion::CONTROL)
                            .focusable(true)
                            .focus_visual(2.0)
                            .press_scale(0.97)
                            .child(text("Open Discover").font(type_::label()).color(t.text))
                            .cursor(rune::Cursor::Pointer)
                            .on_press(|s: &mut State| s.open_seerr_from_settings()),
                    ),
            )
            .child(
                text(
                    "Optional. The key is stored in this file with mode 0600 and is admin-level; \
                     use HTTPS when Seerr is not on a trusted LAN.",
                )
                .font(type_::caption())
                .color(t.text_secondary),
            );
        content = content
            .child(
                text("Requests (Seerr)")
                    .key("settings-section-seerr")
                    .font(type_::section())
                    .color(t.text_secondary)
                    .mt(8.0),
            )
            .child(seerr);
    }

    let storage = col()
        .w_full()
        .padding(18.0)
        .gap(12.0)
        .fill(t.card)
        .surface_sheen()
        .radius(12.0)
        .outline(1.0, 0.0, t.card_border)
        .box_shadow(rune::Shadow {
            y: 1.0,
            blur: 0.0,
            spread: 0.0,
            color: Color::WHITE.alpha(0.05),
            inset: true,
            ..rune::Shadow::default()
        })
        .shadow(1.0, 2.0, t.shadow)
        .shadow(6.0, 18.0, t.shadow_soft)
        .child(
            text("Posters are cached under ~/.cache/runefin/images.")
                .font(type_::meta())
                .color(t.text_secondary),
        )
        .child(
            row()
                .w_full()
                .gap(10.0)
                .focus_group()
                .child(
                    el().key("settings-clear-images")
                        .role("button")
                        .h(36.0)
                        .w(190.0)
                        .radius(8.0)
                        .center()
                        .fill(t.bg)
                        .hover(t.hover)
                        .motion(crate::theme::motion::CONTROL)
                        .focusable(true)
                        .focus_visual(2.0)
                        .press_scale(0.97)
                        .child(
                            text("Clear poster cache")
                                .font(type_::label())
                                .color(t.text),
                        )
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| s.clear_image_cache()),
                )
                .child(
                    el().key("settings-remote")
                        .role("button")
                        .h(36.0)
                        .w(160.0)
                        .radius(8.0)
                        .center()
                        .fill(t.bg)
                        .hover(t.hover)
                        .motion(crate::theme::motion::CONTROL)
                        .focusable(true)
                        .focus_visual(2.0)
                        .press_scale(0.97)
                        .child(text("Remote control").font(type_::label()).color(t.text))
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| s.open_remote()),
                ),
        );
    // Downloads: where the copies live and how much room they may take. The folder applies on
    // a button rather than per keystroke: moving the store is a real change, not typing.
    let budget = s.downloads_budget();
    let downloads = col()
        .w_full()
        .padding(18.0)
        .gap(12.0)
        .fill(t.card)
        .surface_sheen()
        .radius(12.0)
        .outline(1.0, 0.0, t.card_border)
        .box_shadow(rune::Shadow {
            y: 1.0,
            blur: 0.0,
            spread: 0.0,
            color: Color::WHITE.alpha(0.05),
            inset: true,
            ..rune::Shadow::default()
        })
        .shadow(1.0, 2.0, t.shadow)
        .shadow(6.0, 18.0, t.shadow_soft)
        .child(
            text("Downloads are copies of your own files, kept under this folder. Leave the field empty for the default.")
                .font(type_::meta())
                .color(t.text_secondary),
        )
        .child(
            row()
                .key("setting-downloads-path-row")
                .w_full()
                .gap(10.0)
                .items_center()
                .focus_group()
                .child(
                    text("Folder")
                        .font(type_::label())
                        .color(t.text_secondary)
                        .w(110.0),
                )
                .child(
                    el().grow(1.0).min_w(0.0).child(arrows_leave(
                        text_input(&s.downloads_path, type_::body())
                            .key("setting-downloads-path")
                            .label("Downloads folder")
                            .role("textbox")
                            .single_line()
                            .w_full()
                            .padding(6.0)
                            .radius(7.0)
                            .fill(t.bg)
                            .outline(1.0, 0.0, t.card_border)
                            .color(t.text)
                            .on_edit(|s: &mut State| &mut s.downloads_path)
                            .on_submit(|s: &mut State, _tx| {
                                s.keyboard_enter_open("setting-downloads-path");
                            }),
                    )),
                )
        )
        // Beside a text field the arrows belong to the caret, so the button that applies it
        // sits under the field: Down (or Tab) from the field reaches it.
        .child(
            row()
                .key("setting-downloads-path-actions")
                .w_full()
                .gap(10.0)
                .items_center()
                .focus_group()
                .child(
                    el().key("setting-downloads-path-apply")
                        .role("button")
                        .label("Use this folder")
                        .h(32.0)
                        .px(12.0)
                        .radius(8.0)
                        .center()
                        .fill(t.bg)
                        .outline(1.0, 0.0, t.card_border)
                        .hover(t.hover)
                        .motion(crate::theme::motion::CONTROL)
                        .focusable(true)
                        .focus_visual(2.0)
                        .press_scale(0.97)
                        .child(
                            text("Use this folder")
                                .font(type_::label())
                                .color(t.text),
                        )
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| s.apply_downloads_path()),
                )
                .child(
                    text("Existing downloads stay in the old folder.")
                        .font(type_::caption())
                        .color(t.text_secondary),
                ),
        )
        .child(
            row()
                .key("setting-download-quality")
                .w_full()
                .gap(16.0)
                .items_center()
                .focus_group()
                .child(
                    text("Download quality")
                        .font(type_::label())
                        .color(t.text_secondary)
                        .w(220.0),
                )
                .child(el().grow(1.0))
                .child(
                    el().key("setting-download-quality-value")
                        .role("button")
                        .label("Download quality")
                        .h(32.0)
                        .min_w(150.0)
                        .px(14.0)
                        .radius(10.0)
                        .center()
                        .fill(t.bg)
                        .hover(t.hover)
                        .motion(crate::theme::motion::CONTROL)
                        .focusable(true)
                        .focus_visual(2.0)
                        .press_scale(0.97)
                        .child(
                            text(s.download_quality().label())
                                .font(type_::label())
                                .color(t.text),
                        )
                        .child(
                            text("▾")
                                .font(type_::caption())
                                .color(t.text_secondary)
                                .ml(8.0),
                        )
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| s.cycle_download_quality()),
                ),
        )
        .child(
            text("Original keeps the file as the server stores it. A bitrate asks the server to transcode a smaller copy; that copy cannot resume after a pause.")
                .key("setting-download-quality-note")
                .font(type_::caption())
                .color(t.text_secondary)
                .padding(2.0),
        )
        .child(
            row()
                .key("setting-downloads-budget-row")
                .w_full()
                .gap(10.0)
                .items_center()
                .focus_group()
                .child(
                    text("Storage budget")
                        .font(type_::label())
                        .color(t.text_secondary)
                        .w(110.0),
                )
                .child(stepper_button("−", "setting-downloads-budget-down", &t, move |s| {
                    s.set_downloads_budget(budget - 5.0)
                }))
                .child(
                    text(super::downloads::format_gb(budget))
                        .key("setting-downloads-budget-value")
                        .font(type_::body())
                        .color(t.text),
                )
                .child(stepper_button("+", "setting-downloads-budget-up", &t, move |s| {
                    s.set_downloads_budget(budget + 5.0)
                }))
                .child(el().grow(1.0))
                .child(
                    text(format!(
                        "{} used{}",
                        crate::downloads::size(s.downloads_used()),
                        s.downloads
                            .free
                            .map(|free| format!(
                                " · {} free",
                                crate::downloads::size(free)
                            ))
                            .unwrap_or_default()
                    ))
                    .key("setting-downloads-usage")
                    .font(type_::caption())
                    .color(t.text_secondary),
                ),
        )
        .children(s.downloads.error.as_ref().map(|error| {
            text(format!("This folder cannot be used: {error}"))
                .key("setting-downloads-error")
                .font(type_::caption())
                .color(theme::status::danger())
        }))
        .child(
            text(format!("In use: {}", s.downloads.root))
                .key("setting-downloads-root")
                .font(type_::mono_caption())
                .color(t.text_secondary.alpha(0.8)),
        );

    content = content
        .child(
            text("Downloads")
                .key("settings-section-downloads")
                .font(type_::section())
                .color(t.text_secondary)
                .mt(8.0),
        )
        .child(downloads)
        .child(
            text("Storage")
                .key("settings-section-storage")
                .font(type_::section())
                .color(t.text_secondary),
        )
        .child(storage)
        .child(
            text("History")
                .key("settings-section-history")
                .font(type_::section())
                .color(t.text_secondary)
                .mt(8.0),
        )
        .child(
            col()
                .key("settings-history")
                .w_full()
                .padding(18.0)
                .gap(8.0)
                .fill(t.card)
                .surface_sheen()
                .radius(12.0)
                .outline(1.0, 0.0, t.card_border)
                .box_shadow(rune::Shadow {
                    y: 1.0,
                    blur: 0.0,
                    spread: 0.0,
                    color: Color::WHITE.alpha(0.05),
                    inset: true,
                    ..rune::Shadow::default()
                })
                .shadow(1.0, 2.0, t.shadow)
                .shadow(6.0, 18.0, t.shadow_soft)
                .child(
                    text(format!(
                        "{} session(s) · {} min watched on this device",
                        s.history.len(),
                        s.history_ms / 60_000
                    ))
                    .key("settings-history-summary")
                    .font(type_::meta())
                    .color(t.text_secondary),
                )
                .children((!s.history.is_empty()).then(|| {
                    col()
                        .gap(4.0)
                        .children(s.history.iter().rev().take(5).map(|entry| {
                            text(format!(
                                "{} · {} min",
                                entry.name,
                                (entry.played_ms / 60_000).max(1)
                            ))
                            .font(type_::caption())
                            .color(t.text_secondary)
                            .truncate()
                        }))
                }))
                .child(
                    el().key("settings-history-export")
                        .role("button")
                        .h(32.0)
                        .w(180.0)
                        .radius(8.0)
                        .center()
                        .fill(t.bg)
                        .surface_sheen()
                        .shadow(1.0, 2.0, t.shadow)
                        .hover(t.hover)
                        .motion(crate::theme::motion::CONTROL)
                        .focusable(true)
                        .focus_visual(2.0)
                        .press_scale(0.97)
                        .cursor(rune::Cursor::Pointer)
                        .child(
                            text("Export Letterboxd CSV")
                                .font(type_::label())
                                .color(t.text),
                        )
                        .on_press(|s: &mut State| s.export_history()),
                )
                .child(
                    el().key("settings-history-clear")
                        .role("button")
                        .h(32.0)
                        .w(150.0)
                        .radius(8.0)
                        .center()
                        .fill(t.bg)
                        .surface_sheen()
                        .shadow(1.0, 2.0, t.shadow)
                        .hover(t.hover)
                        .motion(crate::theme::motion::CONTROL)
                        .focusable(true)
                        .focus_visual(2.0)
                        .press_scale(0.97)
                        .cursor(rune::Cursor::Pointer)
                        .child(text("Clear history").font(type_::label()).color(t.text))
                        .on_press(|s: &mut State| s.clear_history()),
                ),
        )
        .child(
            text("Account")
                .key("settings-section-account")
                .font(type_::section())
                .color(t.text_secondary)
                .mt(8.0),
        )
        .child(
            row()
                .w_full()
                .gap(10.0)
                .child(
                    el().key("settings-reset")
                        .role("button")
                        .h(36.0)
                        .w(190.0)
                        .radius(8.0)
                        .center()
                        .fill(t.card)
                        .hover(t.hover)
                        .motion(crate::theme::motion::CONTROL)
                        .focusable(true)
                        .focus_visual(2.0)
                        .press_scale(0.97)
                        .cursor(rune::Cursor::Pointer)
                        .child(text("Reset preferences").font(type_::label()).color(t.text))
                        .on_press(|s: &mut State| s.reset_preferences()),
                )
                .child(
                    el().key("settings-export")
                        .role("button")
                        .h(36.0)
                        .w(190.0)
                        .radius(8.0)
                        .center()
                        .fill(t.card)
                        .hover(t.hover)
                        .motion(crate::theme::motion::CONTROL)
                        .focusable(true)
                        .focus_visual(2.0)
                        .press_scale(0.97)
                        .cursor(rune::Cursor::Pointer)
                        .child(
                            text("Export preferences")
                                .font(type_::label())
                                .color(t.text),
                        )
                        .on_press(|s: &mut State| s.export_preferences()),
                ),
        )
        .child(
            el().key("settings-sign-out")
                .role("button")
                .h(36.0)
                .w(160.0)
                .radius(8.0)
                .center()
                .fill(t.card)
                .hover(t.hover)
                .motion(crate::theme::motion::CONTROL)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.97)
                .child(
                    text("Sign out")
                        .font(type_::label())
                        .color(crate::theme::status::danger()),
                )
                .cursor(rune::Cursor::Pointer)
                .on_press(|s: &mut State| s.sign_out()),
        )
        .child(
            // The only in-app exit. On a Steam Machine, "Exit game" is the overlay's job, but
            // a couch should still be able to leave without hunting for it.
            el().key("settings-quit")
                .role("button")
                .h(36.0)
                .w(160.0)
                .radius(8.0)
                .center()
                .fill(t.card)
                .hover(t.hover)
                .motion(crate::theme::motion::CONTROL)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.97)
                .label(if s.media {
                    "Restart app"
                } else {
                    "Quit Runefin"
                })
                .child(
                    text(if s.media {
                        "Restart app"
                    } else {
                        "Quit Runefin"
                    })
                    .font(type_::label())
                    .color(t.text),
                )
                .cursor(rune::Cursor::Pointer)
                .on_press_tx(|_s: &mut State, tx: &mut rune::Tx| tx.quit()),
        );
    if let Some(notice) = &s.image_notice {
        content = content.child(
            text(notice.clone())
                .key("settings-image-notice")
                .font(type_::mono_caption())
                .color(t.text_secondary),
        );
    }

    row()
        .key("settings-screen")
        .w_full()
        .h_full()
        .fill(t.bg)
        .child(sidebar(s, &t))
        .child(el().w(1.0).h_full().fill(t.separator))
        .child(settings_nav(&t))
        .child(el().w(1.0).h_full().fill(t.separator))
        .child(content)
}
