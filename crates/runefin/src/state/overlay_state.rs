//! `State`'s side of the Back/Escape ladder in [`crate::overlay`].

use crate::overlay::{self, Overlay};
use crate::state::{Route, State};

impl State {
    /// The topmost overlay a Back/Escape press should close, if any.
    pub fn top_overlay(&self) -> Option<Overlay> {
        overlay::top(|overlay| self.overlay_open(overlay))
    }

    fn overlay_open(&self, overlay: Overlay) -> bool {
        match overlay {
            Overlay::Rebind => self.rebinding.is_some(),
            Overlay::Keyboard => self.keyboard.open,
            Overlay::PlaylistCreate => self.playlist_create_open,
            Overlay::CastPicker => self.cast_picker_open,
            Overlay::CollectionPicker => self.collection_picker_open,
            Overlay::PlaylistPicker => self.playlist_picker_open,
            Overlay::CardMenu => self.card_menu.is_some(),
            Overlay::Shortcuts => self.shortcuts_open,
            Overlay::SubtitleSearch => self.in_app_subtitle_search.open,
            Overlay::PlayerPanel => self.player_panel_open(),
            Overlay::PlayerGear => self.player_gear_visible(),
            Overlay::PlayerStats => self.in_app_stats,
            Overlay::Preplay => self.preplay_open,
            Overlay::Shelf => self.shelf.is_some() && self.route == Route::Home,
        }
    }

    /// Closes exactly the overlay [`Self::top_overlay`] named.
    pub fn close_overlay(&mut self, overlay: Overlay) {
        match overlay {
            Overlay::Rebind => self.cancel_rebind(),
            Overlay::Keyboard => self.keyboard_hide(),
            Overlay::PlaylistCreate => self.close_playlist_create(),
            Overlay::CastPicker => self.close_cast_picker(),
            Overlay::CollectionPicker => self.close_collection_picker(),
            Overlay::PlaylistPicker => self.close_playlist_picker(),
            Overlay::CardMenu => self.close_card_menu(),
            Overlay::Shortcuts => self.close_shortcuts(),
            Overlay::SubtitleSearch => self.close_in_app_subtitle_search(),
            Overlay::PlayerPanel => self.close_player_panel(),
            Overlay::PlayerGear => self.close_player_gear(),
            Overlay::PlayerStats => self.close_in_app_stats(),
            Overlay::Preplay => self.close_preplay(),
            Overlay::Shelf => self.close_shelf(),
        }
    }
}
