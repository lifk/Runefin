//! Controller input for the couch. One thread polls `gilrs` and sends the same keys a keyboard
//! would, so focus, shortcuts and the player need no second input path.
//!
//! Mapping: D-pad and left stick move (with a repeat), A is Space (activate; play/pause in the
//! player), B and Back are Escape, Start is the context key (card menu; the player's gear), and
//! LB/RB are Shift+Tab/Tab between focus groups. `RUNEFIN_GAMEPAD=0` disables the thread.

use std::time::{Duration, Instant};

use gilrs::{Axis, Button, EventType, Gilrs};
use rune::{AppHandle, Key, KeyEvent, Mods};

use crate::state::State;

const DEADZONE: f32 = 0.5;
const REPEAT_DELAY: Duration = Duration::from_millis(400);
const REPEAT_RATE: Duration = Duration::from_millis(80);
const POLL: Duration = Duration::from_millis(8);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    fn key(self) -> Key {
        match self {
            Dir::Up => Key::Up,
            Dir::Down => Key::Down,
            Dir::Left => Key::Left,
            Dir::Right => Key::Right,
        }
    }
}

fn direction(button: Button) -> Option<Dir> {
    match button {
        Button::DPadUp => Some(Dir::Up),
        Button::DPadDown => Some(Dir::Down),
        Button::DPadLeft => Some(Dir::Left),
        Button::DPadRight => Some(Dir::Right),
        _ => None,
    }
}

/// The keys a face/shoulder/start button sends on press. B and Select are both back; Start is
/// the context key the app already uses for a card menu.
fn button_key(button: Button) -> Option<KeyEvent> {
    let shift = Mods {
        shift: true,
        ..Mods::default()
    };
    let plain = Mods::default();
    let press = |key, mods| Some(KeyEvent { key, mods });
    match button {
        // Space activates a focused control and pauses the player; Enter only reaches the
        // default button and rows.
        Button::South => press(Key::Text(" ".into()), plain),
        Button::East | Button::Select => press(Key::Escape, plain),
        Button::Start => press(Key::ContextMenu, plain),
        // gilrs names the shoulders LeftTrigger/RightTrigger and the analog triggers
        // LeftTrigger2/RightTrigger2; the mapping follows the standard Linux gamepad.
        Button::LeftTrigger => press(Key::Tab, shift),
        Button::RightTrigger => press(Key::Tab, plain),
        _ => None,
    }
}

/// The pad's held direction and repeat clock. Split from `gilrs` so the mapping has tests.
#[derive(Default)]
struct Pad {
    held: Option<Dir>,
    next: Option<Instant>,
    stick: (f32, f32),
}

impl Pad {
    fn on_button(&mut self, button: Button, pressed: bool, now: Instant) -> Option<KeyEvent> {
        if let Some(dir) = direction(button) {
            if pressed {
                self.held = Some(dir);
                self.next = Some(now + REPEAT_DELAY);
                return Some(KeyEvent {
                    key: dir.key(),
                    mods: Mods::default(),
                });
            }
            if self.held == Some(dir) {
                self.held = None;
                self.next = None;
            }
            return None;
        }
        pressed.then(|| button_key(button)).flatten()
    }

    fn on_axis(&mut self, axis: Axis, value: f32, now: Instant) -> Option<KeyEvent> {
        match axis {
            Axis::LeftStickX => self.stick.0 = value,
            Axis::LeftStickY => self.stick.1 = value,
            _ => return None,
        }
        let (x, y) = self.stick;
        let next = if x.abs() >= DEADZONE || y.abs() >= DEADZONE {
            if x.abs() >= y.abs() {
                Some(if x > 0.0 { Dir::Right } else { Dir::Left })
            } else {
                // gilrs follows SDL: positive Y points down.
                Some(if y < 0.0 { Dir::Up } else { Dir::Down })
            }
        } else {
            None
        };
        if next == self.held {
            return None;
        }
        self.held = next;
        self.next = next.map(|_| now + REPEAT_DELAY);
        next.map(|dir| KeyEvent {
            key: dir.key(),
            mods: Mods::default(),
        })
    }

    /// A held direction repeats after the initial delay, then at the faster rate.
    fn tick(&mut self, now: Instant) -> Option<KeyEvent> {
        let dir = self.held?;
        let next = self.next?;
        if now < next {
            return None;
        }
        self.next = Some(now + REPEAT_RATE);
        Some(KeyEvent {
            key: dir.key(),
            mods: Mods::default(),
        })
    }
}

/// Start the controller listener. Safe when no pad is connected: it idles and picks one up later.
pub fn start(handle: AppHandle<State>) {
    if std::env::var("RUNEFIN_GAMEPAD").ok().as_deref() == Some("0") {
        return;
    }
    // A headless render must not depend on a pad or udev being present.
    if std::env::args().any(|arg| arg == "--snap" || arg == "--tree") {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("runefin-pad".into())
        .spawn(move || run(handle));
}

fn run(handle: AppHandle<State>) {
    let Ok(mut gilrs) = Gilrs::new() else { return };
    let mut pad = Pad::default();
    loop {
        while let Some(event) = gilrs.next_event() {
            let now = Instant::now();
            let key = match event.event {
                EventType::ButtonPressed(button, _) => pad.on_button(button, true, now),
                EventType::ButtonReleased(button, _) => pad.on_button(button, false, now),
                EventType::AxisChanged(axis, value, _) => pad.on_axis(axis, value, now),
                _ => None,
            };
            if let Some(key) = key {
                handle.key(key);
            }
        }
        if let Some(key) = pad.tick(Instant::now()) {
            handle.key(key);
        }
        std::thread::sleep(POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(button: Button, pressed: bool) -> Option<KeyEvent> {
        Pad::default().on_button(button, pressed, Instant::now())
    }

    #[test]
    fn face_and_shoulder_buttons_map_to_act_back_context_and_groups() {
        assert_eq!(
            key(Button::South, true)
                .expect("the pad maps this press")
                .key,
            Key::Text(" ".into())
        );
        assert_eq!(
            key(Button::East, true)
                .expect("the pad maps this press")
                .key,
            Key::Escape
        );
        assert_eq!(
            key(Button::Select, true)
                .expect("the pad maps this press")
                .key,
            Key::Escape
        );
        assert_eq!(
            key(Button::Start, true)
                .expect("the pad maps this press")
                .key,
            Key::ContextMenu
        );
        assert!(
            key(Button::LeftTrigger, true)
                .expect("the pad maps this press")
                .mods
                .shift
        );
        assert!(
            !key(Button::RightTrigger, true)
                .expect("the pad maps this press")
                .mods
                .shift
        );
        assert!(key(Button::South, false).is_none(), "release sends nothing");
    }

    #[test]
    fn the_dpad_moves_and_repeats_while_held() {
        let mut pad = Pad::default();
        let t0 = Instant::now();
        assert_eq!(
            pad.on_button(Button::DPadRight, true, t0)
                .expect("the pad maps this press")
                .key,
            Key::Right
        );
        assert!(pad.tick(t0 + REPEAT_DELAY / 2).is_none(), "still waiting");
        assert_eq!(
            pad.tick(t0 + REPEAT_DELAY)
                .expect("the pad maps this press")
                .key,
            Key::Right,
            "the first repeat"
        );
        assert_eq!(
            pad.tick(t0 + REPEAT_DELAY + REPEAT_RATE)
                .expect("the pad maps this press")
                .key,
            Key::Right
        );
        assert!(pad.on_button(Button::DPadRight, false, t0).is_none());
        assert!(pad.tick(t0 + REPEAT_DELAY * 4).is_none(), "released");
    }

    #[test]
    fn the_stick_ignores_drift_and_changes_direction() {
        let mut pad = Pad::default();
        let t0 = Instant::now();
        assert!(pad.on_axis(Axis::LeftStickX, 0.3, t0).is_none(), "deadzone");
        assert_eq!(
            pad.on_axis(Axis::LeftStickX, 0.9, t0)
                .expect("the pad maps this press")
                .key,
            Key::Right
        );
        assert!(
            pad.on_axis(Axis::LeftStickX, 0.8, t0).is_none(),
            "no retrigger"
        );
        assert_eq!(
            pad.on_axis(Axis::LeftStickX, -0.9, t0)
                .expect("the pad maps this press")
                .key,
            Key::Left
        );
        assert!(pad.on_axis(Axis::LeftStickX, 0.1, t0).is_none(), "centred");
        assert_eq!(
            pad.on_axis(Axis::LeftStickY, 0.9, t0)
                .expect("the pad maps this press")
                .key,
            Key::Down
        );
        assert_eq!(
            pad.on_axis(Axis::LeftStickY, -0.9, t0)
                .expect("the pad maps this press")
                .key,
            Key::Up
        );
    }
}
