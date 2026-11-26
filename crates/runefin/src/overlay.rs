//! What a Back/Escape press closes, topmost first.
//!
//! The screens compose these layers in `screens::view`; this list is that order read backwards,
//! so one key never depends on which screen happened to check its own flag first. Toasts are
//! deliberately absent: they time out, and Back should not dismiss a message the reader may be
//! reading.
//!
//! `Rebind` and `Keyboard` are handled before this list in `on_key` because they own every key
//! while they are open; they are listed so a test can still see the whole ladder.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    /// A player shortcut is being recorded.
    Rebind,
    /// The on-screen keyboard is up.
    Keyboard,
    /// Naming a new playlist.
    PlaylistCreate,
    /// The cast picker.
    CastPicker,
    /// "Add to a collection".
    CollectionPicker,
    /// "Add to a playlist".
    PlaylistPicker,
    /// A card's context menu.
    CardMenu,
    /// The shortcut sheet.
    Shortcuts,
    /// The player's online subtitle lookup.
    SubtitleSearch,
    /// One of the player's panels (audio, subtitles, quality, chapters, queue, sleep, go-to).
    PlayerPanel,
    /// The player's settings sheet.
    PlayerGear,
    /// The player's stats overlay.
    PlayerStats,
    /// "Play" with its source and track choices.
    Preplay,
    /// A Home rail expanded to its grid.
    Shelf,
}

/// Topmost first: the last layer painted is the first one Back closes.
pub const ORDER: &[Overlay] = &[
    Overlay::Rebind,
    Overlay::Keyboard,
    Overlay::PlaylistCreate,
    Overlay::CastPicker,
    Overlay::CollectionPicker,
    Overlay::PlaylistPicker,
    Overlay::CardMenu,
    Overlay::Shortcuts,
    Overlay::SubtitleSearch,
    Overlay::PlayerPanel,
    Overlay::PlayerGear,
    Overlay::PlayerStats,
    Overlay::Preplay,
    Overlay::Shelf,
];

/// The first open overlay in [`ORDER`], or `None` when nothing is open.
pub fn top(active: impl Fn(Overlay) -> bool) -> Option<Overlay> {
    ORDER.iter().copied().find(|overlay| active(*overlay))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_menu_closes_before_the_layer_it_sits_on() {
        let open = [Overlay::Shortcuts, Overlay::CardMenu];
        assert_eq!(
            top(|overlay| open.contains(&overlay)),
            Some(Overlay::CardMenu)
        );
    }

    #[test]
    fn a_player_panel_closes_before_the_page_behind_it() {
        let open = [Overlay::Shelf, Overlay::PlayerPanel];
        assert_eq!(
            top(|overlay| open.contains(&overlay)),
            Some(Overlay::PlayerPanel)
        );
    }

    #[test]
    fn the_modal_over_the_player_closes_first() {
        let open = [Overlay::PlayerGear, Overlay::PlaylistPicker];
        assert_eq!(
            top(|overlay| open.contains(&overlay)),
            Some(Overlay::PlaylistPicker)
        );
    }

    #[test]
    fn an_empty_window_has_nothing_to_close() {
        assert_eq!(top(|_| false), None);
    }
}
