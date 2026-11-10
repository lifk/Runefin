//! The TV's semantic actions, from whichever device sent them.
//!
//! A CEC remote, a keyboard and the gamepad thread all arrive as `KeyEvent`s; this module is the
//! one place that decides what a key means in the living room. The focus router and the player
//! read these actions instead of raw keys, so adding a device (a HID remote, a phone) means
//! translating into the same few actions.

use rune::{Key, KeyEvent};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TvAction {
    Up,
    Down,
    Left,
    Right,
    /// OK, A, or Return: press what is focused.
    Activate,
    /// Back, B, or Escape.
    Back,
    /// Menu/Start: the context menu, or the player's gear.
    Menu,
    PagePrev,
    PageNext,
}

impl TvAction {
    pub fn is_direction(self) -> bool {
        matches!(
            self,
            TvAction::Up | TvAction::Down | TvAction::Left | TvAction::Right
        )
    }
}

/// The action a key carries, or `None` when it is not part of the TV vocabulary (typing, a
/// shortcut letter, a modifier).
pub fn action(key: &KeyEvent) -> Option<TvAction> {
    Some(match key.key {
        Key::Up => TvAction::Up,
        Key::Down => TvAction::Down,
        Key::Left => TvAction::Left,
        Key::Right => TvAction::Right,
        Key::Enter => TvAction::Activate,
        Key::Escape => TvAction::Back,
        Key::ContextMenu => TvAction::Menu,
        Key::PageUp => TvAction::PagePrev,
        Key::PageDown => TvAction::PageNext,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: Key) -> KeyEvent {
        KeyEvent {
            key,
            mods: Default::default(),
        }
    }

    #[test]
    fn the_four_directions_and_ok_come_from_the_remote_and_the_pad() {
        assert_eq!(action(&key(Key::Up)), Some(TvAction::Up));
        assert_eq!(action(&key(Key::Left)), Some(TvAction::Left));
        assert_eq!(action(&key(Key::Enter)), Some(TvAction::Activate));
        assert_eq!(action(&key(Key::Escape)), Some(TvAction::Back));
        assert_eq!(action(&key(Key::Text("a".into()))), None);
    }
}
