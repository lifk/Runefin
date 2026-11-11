//! Settings: the facts about this install, plus the two actions that belong here.

use rune::tok::{self, motion};

use crate::theme::FocusVisual;
use crate::theme::SurfaceStyle;
use crate::theme::{self, type_};
use rune::{Color, El, col, el, row, text, text_input};

use crate::state::State;

use super::sidebar;

fn stepper_button(label: &str, key: &str, t: &tok::Theme, on: impl Fn(&mut State) + 'static) -> El {
    el().key(key)
        .role("button")
        .h(32.0)
        .w(36.0)
        .radius(8.0)
        .center()
        .fill(t.bg)
        .hover(t.hover)
        .motion(motion::SNAPPY)
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
                        tok::accent(0, true)
                    } else {
                        t.card_border
                    },
                )
                .box_shadow(rune::Shadow {
                    y: 0.0,
                    blur: 0.0,
                    spread: if s.settings_query.focused { 3.0 } else { 0.0 },
                    color: tok::accent(0, true).alpha(if s.settings_query.focused {
                        0.18
                    } else {
                        0.0
                    }),
                    inset: false,
                    ..rune::Shadow::default()
                })
                .child(
                    text_input(&s.settings_query, type_::body())
                        .key("settings-search")
                        .label("Search settings")
                        .placeholder("Search settings", t.text_secondary.alpha(0.55))
                        .role("textbox")
                        .single_line()
                        .w_full()
                        .on_edit(|s: &mut State| &mut s.settings_query)
                        .color(t.text),
                ),
        )
        .child(
            row()
                .key("settings-index")
                .w_full()
                .gap(6.0)
                .flex_wrap()
                .children(
                    [
                        (
                            "Appearance",
                            "settings-jump-appearance",
                            "settings-section-appearance",
                        ),
                        (
                            "Playback",
                            "settings-jump-playback",
                            "settings-section-playback",
                        ),
                        (
                            "Subtitles",
                            "settings-jump-subtitles",
                            "settings-section-subtitles",
                        ),
                        (
                            "Keyboard",
                            "settings-jump-keyboard",
                            "settings-section-keyboard",
                        ),
                        (
                            "Metadata",
                            "settings-jump-metadata",
                            "settings-section-metadata",
                        ),
                        ("Seerr", "settings-jump-seerr", "settings-section-seerr"),
                        (
                            "Storage",
                            "settings-jump-storage",
                            "settings-section-storage",
                        ),
                        (
                            "History",
                            "settings-jump-history",
                            "settings-section-history",
                        ),
                        (
                            "Account",
                            "settings-jump-account",
                            "settings-section-account",
                        ),
                    ]
                    .map(|(label, key, target)| {
                        el().key(key)
                            .role("button")
                            .h(28.0)
                            .px(10.0)
                            .radius(7.0)
                            .center()
                            .fill(t.bg)
                            .outline(1.0, 0.0, t.card_border)
                            .hover(t.hover)
                            .motion(motion::SNAPPY)
                            .focusable(true)
                            .focus_visual(2.0)
                            .press_scale(0.96)
                            .child(text(label).font(type_::caption()).color(t.text_secondary))
                            .cursor(rune::Cursor::Pointer)
                            .on_press_tx(move |_s: &mut State, tx: &mut rune::Tx| {
                                tx.scroll_key_to(target, 8.0);
                            })
                    }),
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
            "Compact Home hero",
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
            el().key(format!("{key}-value"))
                .role("switch")
                .label(label.to_string())
                .checked(on)
                .w(38.0)
                .h(22.0)
                .radius(11.0)
                .fill(if on {
                    tok::accent(0, true)
                } else {
                    t.control_off
                })
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.97)
                .cursor(rune::Cursor::Pointer)
                .child(
                    el().absolute(if on { 18.0 } else { 2.0 }, 2.0)
                        .size(18.0, 18.0)
                        .radius(9.0)
                        .fill(t.knob)
                        .shadow(0.0, 2.0, t.shadow),
                )
                .on_press(action)
        } else {
            el().key(format!("{key}-value"))
                .role("button")
                .h(32.0)
                .min_w(150.0)
                .px(14.0)
                .radius(10.0)
                .center()
                .fill(t.bg)
                .hover(t.hover)
                .motion(motion::SNAPPY)
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
                .child(el().w(150.0).justify_end().child(control)),
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
                    .motion(motion::SNAPPY)
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
                .motion(motion::SNAPPY)
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

    let key_input = text_input(&s.omdb_key, type_::body())
        .key("setting-omdb-key")
        .label("OMDb API key")
        .role("textbox")
        .single_line()
        .secure()
        .w_full()
        .on_edit(|s: &mut State| &mut s.omdb_key)
        .color(t.text);
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
                        .motion(motion::SNAPPY)
                        .focusable(true)
                        .focus_visual(2.0)
                        .press_scale(0.97)
                        .child(
                            text("Get a key")
                                .font(type_::caption())
                                .color(tok::accent(0, true)),
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
        let url_input = text_input(&s.seerr.url, type_::body())
            .key("setting-seerr-url")
            .label("Seerr server")
            .role("textbox")
            .single_line()
            .w_full()
            .on_edit(|s: &mut State| &mut s.seerr.url)
            .color(t.text);
        // A secure field draws one bullet per character, and the platform text overlay is not
        // clipped by the box: a 68-character key ran past the card. Show a fixed mask until
        // the reader clicks it, then hand over to the real field.
        let key_input = if s.seerr.key.focused {
            text_input(&s.seerr.key, type_::body())
                .key("setting-seerr-key-input")
                .label("Seerr API key")
                .role("textbox")
                .single_line()
                .secure()
                .w_full()
                .on_edit(|s: &mut State| &mut s.seerr.key)
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
        };
        let user_input = text_input(&s.seerr.user, type_::body())
            .key("setting-seerr-user")
            .label("Seerr user id")
            .role("textbox")
            .single_line()
            .w_full()
            .on_edit(|s: &mut State| &mut s.seerr.user)
            .color(t.text);
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
                            .fill(tok::accent(0, true))
                            .gradient(Color::WHITE.alpha(0.18), Color::TRANSPARENT)
                            .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
                            .shadow(4.0, 12.0, tok::accent(0, true).alpha(0.35))
                            .hover(t.hover)
                            .motion(motion::SNAPPY)
                            .focusable(true)
                            .focus_visual(2.0)
                            .press_scale(0.97)
                            .child(text("Test").font(type_::label()).color(t.bg))
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
                            .motion(motion::SNAPPY)
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
                            .motion(motion::SNAPPY)
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
                        .motion(motion::SNAPPY)
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
                        .motion(motion::SNAPPY)
                        .focusable(true)
                        .focus_visual(2.0)
                        .press_scale(0.97)
                        .child(text("Remote control").font(type_::label()).color(t.text))
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| s.open_remote()),
                ),
        );
    content = content
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
                        .motion(motion::SNAPPY)
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
                        .motion(motion::SNAPPY)
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
                        .motion(motion::SNAPPY)
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
                        .motion(motion::SNAPPY)
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
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.97)
                .child(
                    text("Sign out")
                        .font(type_::label())
                        .color(tok::accent(2, true)),
                )
                .cursor(rune::Cursor::Pointer)
                .on_press(|s: &mut State| s.sign_out()),
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
        .child(content)
}
