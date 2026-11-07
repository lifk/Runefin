//! Player shortcuts the reader can remap. Only player actions are customisable; global
//! navigation (Ctrl K, Escape, arrows) stays fixed so a bad binding cannot trap the window.
//!
//! A binding is a canonical lower-case name: a single character, `space`, an arrow name, or a
//! function key (`F11`). Matching is on the logical key Rune reports, so a non-Latin layout can
//! bind its own letters.

use rune::{Key, KeyEvent};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    PlayPause,
    Mute,
    Subtitles,
    Skip,
    NextEpisode,
    PreviousEpisode,
    Fit,
    Fullscreen,
}

impl Action {
    pub const ALL: [Action; 8] = [
        Action::PlayPause,
        Action::Mute,
        Action::Subtitles,
        Action::Skip,
        Action::NextEpisode,
        Action::PreviousEpisode,
        Action::Fit,
        Action::Fullscreen,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Action::PlayPause => "Play / pause",
            Action::Mute => "Mute",
            Action::Subtitles => "Cycle subtitles",
            Action::Skip => "Skip intro / credits",
            Action::NextEpisode => "Next episode",
            Action::PreviousEpisode => "Previous episode",
            Action::Fit => "Fit / fill the picture",
            Action::Fullscreen => "Fullscreen",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyBindings {
    pub play_pause: String,
    pub mute: String,
    pub subtitles: String,
    pub skip: String,
    pub next_episode: String,
    pub previous_episode: String,
    pub fit: String,
    pub fullscreen: String,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            play_pause: "space".into(),
            mute: "m".into(),
            subtitles: "c".into(),
            skip: "s".into(),
            next_episode: "n".into(),
            previous_episode: "p".into(),
            fit: "f".into(),
            fullscreen: "F11".into(),
        }
    }
}

impl KeyBindings {
    pub fn get(&self, action: Action) -> &str {
        match action {
            Action::PlayPause => &self.play_pause,
            Action::Mute => &self.mute,
            Action::Subtitles => &self.subtitles,
            Action::Skip => &self.skip,
            Action::NextEpisode => &self.next_episode,
            Action::PreviousEpisode => &self.previous_episode,
            Action::Fit => &self.fit,
            Action::Fullscreen => &self.fullscreen,
        }
    }

    pub fn set(&mut self, action: Action, key: &str) {
        let slot = match action {
            Action::PlayPause => &mut self.play_pause,
            Action::Mute => &mut self.mute,
            Action::Subtitles => &mut self.subtitles,
            Action::Skip => &mut self.skip,
            Action::NextEpisode => &mut self.next_episode,
            Action::PreviousEpisode => &mut self.previous_episode,
            Action::Fit => &mut self.fit,
            Action::Fullscreen => &mut self.fullscreen,
        };
        *slot = key.to_string();
    }

    /// Whether this binding is already used by another action. A duplicate would make one of
    /// the two actions unreachable, so the settings row refuses it.
    pub fn used_by_other(&self, action: Action, key: &str) -> bool {
        Action::ALL
            .iter()
            .any(|other| *other != action && self.get(*other).eq_ignore_ascii_case(key))
    }
}

/// The canonical name of a key, or `None` for a modifier-only or unsupported key.
pub fn key_name(event: &KeyEvent) -> Option<String> {
    match &event.key {
        Key::Text(text) if text == " " => Some("space".into()),
        Key::Text(text) if text.chars().count() == 1 => Some(text.to_lowercase()),
        Key::Function(number) if (1..=24).contains(number) => Some(format!("F{number}")),
        Key::Left => Some("left".into()),
        Key::Right => Some("right".into()),
        Key::Up => Some("up".into()),
        Key::Down => Some("down".into()),
        _ => None,
    }
}

pub fn matches(binding: &str, event: &KeyEvent) -> bool {
    if event.mods.cmd || event.mods.ctrl || event.mods.alt || event.mods.shift {
        return false;
    }
    if binding.eq_ignore_ascii_case("space") {
        return matches!(&event.key, Key::Text(text) if text == " ");
    }
    match binding.to_ascii_lowercase().as_str() {
        "left" => return event.key == Key::Left,
        "right" => return event.key == Key::Right,
        "up" => return event.key == Key::Up,
        "down" => return event.key == Key::Down,
        other if other.starts_with('f') => {
            if let Ok(number) = other[1..].parse::<u8>() {
                return event.key == Key::Function(number);
            }
        }
        _ => {}
    }
    matches!(&event.key, Key::Text(text) if text.eq_ignore_ascii_case(binding))
}

/// A keycap label: `Space`, `Left`, `F11`, or an upper-case letter.
pub fn display(binding: &str) -> String {
    match binding.to_ascii_lowercase().as_str() {
        "space" => "Space".into(),
        "left" => "Left".into(),
        "right" => "Right".into(),
        "up" => "Up".into(),
        "down" => "Down".into(),
        other if other.starts_with('f') && other.len() <= 3 => other.to_uppercase(),
        _ => binding.to_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::Mods;

    fn text(value: &str) -> KeyEvent {
        KeyEvent {
            key: Key::Text(value.into()),
            mods: Mods::default(),
        }
    }

    #[test]
    fn a_binding_matches_its_logical_key_and_ignores_modified_presses() {
        let bindings = KeyBindings::default();
        assert!(matches(bindings.get(Action::Mute), &text("M")));
        let mut event = text("m");
        event.mods.ctrl = true;
        assert!(
            !matches(bindings.get(Action::Mute), &event),
            "Ctrl-M belongs to the shell, not the player"
        );
        assert_eq!(display("f11"), "F11");
        assert_eq!(display("space"), "Space");
        assert_eq!(key_name(&text(" ")).as_deref(), Some("space"));
    }

    #[test]
    fn a_duplicate_binding_is_reported() {
        let bindings = KeyBindings::default();
        assert!(bindings.used_by_other(Action::Fit, "m"));
        assert!(!bindings.used_by_other(Action::Mute, "m"));
    }
}
