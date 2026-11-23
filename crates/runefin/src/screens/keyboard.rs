//! The in-app on-screen keyboard sheet. A pure read of `State::keyboard` and the target
//! `TextEdit`: keys mutate the model through `State`'s keyboard operations.

use rune::tok;
use rune::{Color, El, col, el, row, text};

use crate::state::State;
use crate::theme::{self, FocusVisual, SurfaceStyle, type_};

/// One sheet for both layouts. The desktop variant is narrower but uses the same keys.
pub(super) fn keyboard(s: &State) -> El {
    let t = theme::colors();
    let Some(target) = s.keyboard.target else {
        return el();
    };
    let big = theme::is_big_screen();
    let key_h = if big { 52.0 } else { 36.0 };
    let key_font = if big {
        type_::label()
    } else {
        type_::caption()
    };

    let header = row()
        .w_full()
        .items_center()
        .gap(10.0)
        .child(
            text("On-screen keyboard")
                .font(type_::section())
                .color(t.text)
                .grow(1.0),
        )
        .child(
            el().key("osk-close")
                .role("button")
                .h(if big { 36.0 } else { 28.0 })
                .px(10.0)
                .radius(7.0)
                .center()
                .fill(t.bg)
                .hover(t.hover)
                .motion(crate::theme::motion::CONTROL)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.94)
                .label("Hide keyboard")
                .help("Hide the keyboard")
                .cursor(rune::Cursor::Pointer)
                .child(
                    text(if big { "B / Esc" } else { "Esc" })
                        .font(type_::mono_caption())
                        .color(t.text_secondary),
                )
                .on_press(|s: &mut State| s.keyboard_hide()),
        );

    let value = s.keyboard.value(s);
    let strip = row()
        .key("osk-target")
        .w_full()
        .padding(10.0)
        .gap(8.0)
        .items_center()
        .radius(8.0)
        .fill(t.bg)
        .outline(1.0, 0.0, t.card_border)
        .child(
            text(target.label())
                .font(type_::caption())
                .color(t.text_secondary),
        )
        .child(
            text(value)
                .font(type_::body())
                .color(t.text)
                .max_lines(1)
                .truncate()
                .grow(1.0),
        )
        .children(
            (target.secure() && s.keyboard.reveal)
                .then(|| text("shown").font(type_::caption()).color(t.text_secondary)),
        );

    let mut grid = col().w_full().gap(6.0);
    for (index, line) in rows(s).iter().enumerate() {
        let mut line_row = row().w_full().gap(6.0);
        for ch in line.chars() {
            let typed = if s.keyboard.shift && ch.is_ascii_alphabetic() {
                ch.to_ascii_uppercase()
            } else {
                ch
            };
            let label = typed.to_string();
            let key = format!("osk-{}", ch);
            line_row = line_row.child(key_button(
                &key,
                &label,
                key_h,
                key_font,
                &t,
                false,
                1.0,
                move |s: &mut State| {
                    s.keyboard_type(&typed.to_string());
                },
            ));
        }
        // The three letter rows are all ten keys; the grid's grow keeps the columns
        // aligned even when a symbol page has a shorter row.
        grid = grid.child(line_row.key(format!("osk-row-{index}")));
    }

    let mut controls = Vec::new();
    if s.keyboard.page == crate::state::KeyboardPage::Letters {
        controls.push(key_button(
            "osk-shift",
            "Shift",
            key_h,
            key_font,
            &t,
            s.keyboard.shift,
            1.2,
            |s: &mut State| s.keyboard_toggle_shift(),
        ));
    }
    controls.push(key_button(
        "osk-page",
        if s.keyboard.page == crate::state::KeyboardPage::Letters {
            "123"
        } else {
            "ABC"
        },
        key_h,
        key_font,
        &t,
        false,
        1.2,
        |s: &mut State| s.keyboard_toggle_page(),
    ));
    controls.push(key_button(
        "osk-space",
        "Space",
        key_h,
        key_font,
        &t,
        false,
        4.0,
        |s: &mut State| {
            s.keyboard_type(" ");
        },
    ));
    controls.push(key_button(
        "osk-backspace",
        "⌫",
        key_h,
        key_font,
        &t,
        false,
        1.6,
        |s: &mut State| {
            s.keyboard_backspace();
        },
    ));
    let mut actions = Vec::new();
    if target.next().is_some() {
        actions.push(key_button(
            "osk-next",
            "Next",
            key_h,
            key_font,
            &t,
            false,
            1.4,
            |s: &mut State| s.keyboard_next(),
        ));
    }
    if target.secure() {
        actions.push(key_button(
            "osk-reveal",
            if s.keyboard.reveal { "Hide" } else { "Show" },
            key_h,
            key_font,
            &t,
            s.keyboard.reveal,
            1.4,
            |s: &mut State| s.keyboard_toggle_reveal(),
        ));
    }
    actions.push(key_button(
        "osk-enter",
        "Enter",
        key_h,
        key_font,
        &t,
        false,
        1.8,
        |s: &mut State| {
            s.keyboard_enter();
        },
    ));
    actions.push(key_button(
        "osk-hide",
        "Hide",
        key_h,
        key_font,
        &t,
        false,
        1.2,
        |s: &mut State| s.keyboard_hide(),
    ));

    let panel = col()
        .key("osk")
        .w_full()
        .max_w(980.0)
        .padding(18.0)
        .gap(10.0)
        .radius(16.0)
        .fill(t.card)
        .surface_sheen()
        .outline(1.0, 0.0, t.card_border)
        .shadow(1.0, 2.0, t.shadow)
        .shadow(10.0, 28.0, t.shadow_soft)
        .role("dialog")
        .label("On-screen keyboard")
        .focus_group()
        .child(header)
        .child(strip)
        .child(grid)
        .child(row().w_full().gap(6.0).children(controls))
        .child(row().w_full().gap(6.0).children(actions));

    el().key("osk-scrim")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .padding(if big { 24.0 } else { 14.0 })
        .fill(Color::BLACK.alpha(0.58))
        .justify_end()
        .items_center()
        .child(panel)
}

/// The character rows. They are intentionally ASCII: a server URL, a password and a
/// Jellyfin search are the jobs of this first slice; a full IME is a later decision.
fn rows(s: &State) -> Vec<String> {
    if s.keyboard.page == crate::state::KeyboardPage::Letters {
        if s.keyboard.shift {
            vec![
                "QWERTYUIOP".to_string(),
                "ASDFGHJKL".to_string(),
                "ZXCVBNM,./".to_string(),
            ]
        } else {
            vec![
                "qwertyuiop".to_string(),
                "asdfghjkl".to_string(),
                "zxcvbnm,./".to_string(),
            ]
        }
    } else {
        vec![
            "1234567890".to_string(),
            "-/:;()$&@\"".to_string(),
            ".,?!'_+=<".to_string(),
        ]
    }
}

#[allow(clippy::too_many_arguments)]
fn key_button(
    key: &str,
    label: &str,
    height: f32,
    font: rune::TextStyle,
    t: &tok::Theme,
    active: bool,
    grow: f32,
    on: impl Fn(&mut State) + 'static,
) -> El {
    // A CEC remote's OK arrives as Return, while Rune's buttons activate on Space. The
    // sheet's keys take Return themselves, so a remote does not need a pad to type.
    let on: std::rc::Rc<dyn Fn(&mut State)> = std::rc::Rc::new(on);
    let press = {
        let on = std::rc::Rc::clone(&on);
        move |s: &mut State| on(s)
    };
    let keyed = {
        let on = std::rc::Rc::clone(&on);
        move |s: &mut State, ev: &rune::KeyEvent, _tx: &mut rune::Tx| {
            if ev.key == rune::Key::Enter {
                on(s);
                true
            } else {
                false
            }
        }
    };
    el().key(key.to_string())
        .role("button")
        .h(height)
        .grow(grow)
        .min_w(0.0)
        .radius(8.0)
        .center()
        .fill(if active {
            tok::accent(0, true).alpha(0.22)
        } else {
            t.bg
        })
        .outline(
            1.0,
            0.0,
            if active {
                tok::accent(0, true).alpha(0.55)
            } else {
                t.card_border
            },
        )
        .hover(t.hover)
        .motion(crate::theme::motion::CONTROL)
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.95)
        .label(label.to_string())
        .help(label.to_string())
        .cursor(rune::Cursor::Pointer)
        .child(text(label.to_string()).font(font).color(t.text))
        .on_key(keyed)
        .on_press(press)
}
