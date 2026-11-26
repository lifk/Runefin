//! The in-app on-screen keyboard: which text field it edits, the key page, and the
//! model operations a D-pad key invokes. The sheet itself lives in `screens::keyboard`.

use rune::{Key, KeyEvent, Mods, TextEdit};

use super::*;
use crate::config::KeyboardMode;

/// Every text field in the app. A field that is not here cannot be edited by the
/// on-screen keyboard; `target_for_key` is the only place a focus key becomes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextTarget {
    ConnectServer,
    ConnectUser,
    ConnectPassword,
    Search,
    LibrarySearch,
    SettingsSearch,
    SeerrSearch,
    OmdbKey,
    SeerrUrl,
    SeerrKey,
    SeerrUser,
    PlaylistName,
    GotoTime,
    DownloadsPath,
}

impl TextTarget {
    /// The focus key that opens this target. `setting-seerr-key` is the button that reveals
    /// the real field; both keys name the same target.
    pub fn from_key(key: &str) -> Option<TextTarget> {
        Some(match key {
            "server" => TextTarget::ConnectServer,
            "username" => TextTarget::ConnectUser,
            "password" => TextTarget::ConnectPassword,
            "search-input" => TextTarget::Search,
            "library-search" => TextTarget::LibrarySearch,
            "settings-search" => TextTarget::SettingsSearch,
            "seerr-search" => TextTarget::SeerrSearch,
            "setting-omdb-key" => TextTarget::OmdbKey,
            "setting-seerr-url" => TextTarget::SeerrUrl,
            "setting-seerr-key" | "setting-seerr-key-input" => TextTarget::SeerrKey,
            "setting-seerr-user" => TextTarget::SeerrUser,
            "playlist-name-input" => TextTarget::PlaylistName,
            "in-app-goto-input" => TextTarget::GotoTime,
            "setting-downloads-path" => TextTarget::DownloadsPath,
            _ => return None,
        })
    }

    pub fn field_key(self) -> &'static str {
        match self {
            TextTarget::ConnectServer => "server",
            TextTarget::ConnectUser => "username",
            TextTarget::ConnectPassword => "password",
            TextTarget::Search => "search-input",
            TextTarget::LibrarySearch => "library-search",
            TextTarget::SettingsSearch => "settings-search",
            TextTarget::SeerrSearch => "seerr-search",
            TextTarget::OmdbKey => "setting-omdb-key",
            TextTarget::SeerrUrl => "setting-seerr-url",
            TextTarget::SeerrKey => "setting-seerr-key-input",
            TextTarget::SeerrUser => "setting-seerr-user",
            TextTarget::PlaylistName => "playlist-name-input",
            TextTarget::GotoTime => "in-app-goto-input",
            TextTarget::DownloadsPath => "setting-downloads-path",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TextTarget::ConnectServer => "Server URL",
            TextTarget::ConnectUser => "Username",
            TextTarget::ConnectPassword => "Password",
            TextTarget::Search => "Search",
            TextTarget::LibrarySearch => "Search this library",
            TextTarget::SettingsSearch => "Search settings",
            TextTarget::SeerrSearch => "Search Seerr",
            TextTarget::OmdbKey => "OMDb API key",
            TextTarget::SeerrUrl => "Seerr server",
            TextTarget::SeerrKey => "Seerr API key",
            TextTarget::SeerrUser => "Seerr user id",
            TextTarget::PlaylistName => "Playlist name",
            TextTarget::GotoTime => "Time",
            TextTarget::DownloadsPath => "Downloads folder",
        }
    }

    pub fn secure(self) -> bool {
        matches!(
            self,
            TextTarget::ConnectPassword | TextTarget::OmdbKey | TextTarget::SeerrKey
        )
    }

    /// The next field in the connect form. Other forms have no ordered chain: closing the
    /// sheet and moving with the D-pad is the right move there.
    pub fn next(self) -> Option<TextTarget> {
        match self {
            TextTarget::ConnectServer => Some(TextTarget::ConnectUser),
            TextTarget::ConnectUser => Some(TextTarget::ConnectPassword),
            _ => None,
        }
    }

    pub fn edit(self, s: &State) -> &TextEdit {
        match self {
            TextTarget::ConnectServer => &s.server,
            TextTarget::ConnectUser => &s.user,
            TextTarget::ConnectPassword => &s.password,
            TextTarget::Search => &s.search.query,
            TextTarget::LibrarySearch => &s.library_search,
            TextTarget::SettingsSearch => &s.settings_query,
            TextTarget::SeerrSearch => &s.seerr.query,
            TextTarget::OmdbKey => &s.omdb_key,
            TextTarget::SeerrUrl => &s.seerr.url,
            TextTarget::SeerrKey => &s.seerr.key,
            TextTarget::SeerrUser => &s.seerr.user,
            TextTarget::PlaylistName => &s.playlist_new_name,
            TextTarget::GotoTime => &s.in_app_goto,
            TextTarget::DownloadsPath => &s.downloads_path,
        }
    }

    pub fn edit_mut(self, s: &mut State) -> &mut TextEdit {
        match self {
            TextTarget::ConnectServer => &mut s.server,
            TextTarget::ConnectUser => &mut s.user,
            TextTarget::ConnectPassword => &mut s.password,
            TextTarget::Search => &mut s.search.query,
            TextTarget::LibrarySearch => &mut s.library_search,
            TextTarget::SettingsSearch => &mut s.settings_query,
            TextTarget::SeerrSearch => &mut s.seerr.query,
            TextTarget::OmdbKey => &mut s.omdb_key,
            TextTarget::SeerrUrl => &mut s.seerr.url,
            TextTarget::SeerrKey => &mut s.seerr.key,
            TextTarget::SeerrUser => &mut s.seerr.user,
            TextTarget::PlaylistName => &mut s.playlist_new_name,
            TextTarget::GotoTime => &mut s.in_app_goto,
            TextTarget::DownloadsPath => &mut s.downloads_path,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum KeyboardPage {
    #[default]
    Letters,
    Symbols,
}

/// The keyboard's model. It is deliberately separate from `State`: the sheet reads this and
/// the app's text models, never the other way around.
pub struct Keyboard {
    pub open: bool,
    pub target: Option<TextTarget>,
    pub page: KeyboardPage,
    pub shift: bool,
    pub reveal: bool,
    pub mode: KeyboardMode,
    field_key: Option<String>,
    /// The field to focus after the sheet closes. `main`'s poll applies it with a `Tx`.
    pending_focus: Option<String>,
    /// A field the reader closed the sheet on; auto-open stays quiet until focus moves.
    suppress: Option<String>,
}

impl Default for Keyboard {
    fn default() -> Self {
        Self::new(KeyboardMode::Auto)
    }
}

impl Keyboard {
    pub fn new(mode: KeyboardMode) -> Keyboard {
        Keyboard {
            open: false,
            target: None,
            page: KeyboardPage::Letters,
            shift: false,
            reveal: false,
            mode,
            field_key: None,
            pending_focus: None,
            suppress: None,
        }
    }

    pub fn open_for(&mut self, field_key: &str, target: TextTarget) {
        self.open = true;
        self.target = Some(target);
        self.field_key = Some(field_key.to_string());
        self.page = KeyboardPage::Letters;
        self.shift = false;
        self.reveal = false;
        self.suppress = None;
        self.pending_focus = Some("osk-a".into());
    }

    /// The auto-open edge. `focused` is Rune's runtime focus, not `State::focused_key`:
    /// several text fields never call `note_focus`.
    pub fn observe(&mut self, focused: Option<&str>, big_screen: bool) -> bool {
        if self.open {
            if let Some(key) = focused
                && key != self.field_key.as_deref().unwrap_or_default()
                && !key.starts_with("osk-")
            {
                // Something above the sheet took focus (a card menu, a route change). Close
                // without pulling focus back to a field that is no longer current.
                self.reset();
                return true;
            }
            return false;
        }
        let Some(key) = focused else {
            self.suppress = None;
            return false;
        };
        if self.suppress.as_deref() == Some(key) {
            return false;
        }
        // Focus alone never opens the sheet. On a TV the search field sits in the same row as
        // the cards, so arrowing across it must not trap the reader in the keyboard; the sheet
        // opens from the field's own action (Menu/Start or Enter). `big_screen` keeps the
        // argument the caller knows; the mode only decides whether Off forbids opening at all.
        let _ = big_screen;
        self.suppress = None;
        false
    }

    /// Enter on a focused field: Auto opens the sheet, On-demand keeps Enter for the field
    /// and asks for Menu/Start instead.
    pub fn manual_open_enter(&mut self, focused: Option<&str>) -> bool {
        if self.mode != KeyboardMode::Auto {
            return false;
        }
        self.manual_open(focused)
    }

    /// The Menu/Start key, or a field's Keyboard action: open even in On-demand mode.
    pub fn manual_open(&mut self, focused: Option<&str>) -> bool {
        if self.mode == KeyboardMode::Off {
            return false;
        }
        let Some(key) = focused else {
            return false;
        };
        let Some(target) = TextTarget::from_key(key) else {
            return false;
        };
        self.open_for(key, target);
        true
    }

    pub fn close(&mut self) {
        if let Some(key) = self.field_key.take() {
            self.suppress = Some(key.clone());
            self.pending_focus = Some(key);
        }
        self.open = false;
        self.target = None;
    }

    /// Close because the screen changed: no focus return to a field that may be gone.
    pub fn reset(&mut self) {
        self.open = false;
        self.target = None;
        self.field_key = None;
        self.pending_focus = None;
        self.suppress = None;
        self.reveal = false;
    }

    pub fn take_focus(&mut self) -> Option<String> {
        self.pending_focus.take()
    }

    pub fn next(&mut self) -> bool {
        let Some(next) = self.target.and_then(TextTarget::next) else {
            self.close();
            return true;
        };
        self.target = Some(next);
        self.field_key = Some(next.field_key().to_string());
        self.shift = false;
        self.suppress = None;
        self.pending_focus = Some("osk-a".into());
        true
    }

    pub fn toggle_page(&mut self) {
        self.page = match self.page {
            KeyboardPage::Letters => KeyboardPage::Symbols,
            KeyboardPage::Symbols => KeyboardPage::Letters,
        };
        self.shift = false;
    }

    pub fn toggle_shift(&mut self) {
        self.shift = !self.shift;
    }

    pub fn toggle_reveal(&mut self) {
        self.reveal = !self.reveal;
    }

    /// The value under the sheet's target strip. Secure fields are masked unless the reader
    /// pressed Show; the text never appears in a toast or a log.
    pub fn value(&self, s: &State) -> String {
        let Some(target) = self.target else {
            return String::new();
        };
        let edit = target.edit(s);
        if target.secure() && !self.reveal {
            if edit.text.is_empty() {
                "Password".into()
            } else {
                "•".repeat(edit.text.chars().count().min(24))
            }
        } else if edit.text.is_empty() {
            "Type here".into()
        } else {
            edit.text.clone()
        }
    }
}

impl State {
    /// Record the target's current text after a key, because `TextEdit` changes made by the
    /// app do not fire `on_input_change`. Only the search fields have work to do.
    fn keyboard_changed(&mut self, target: TextTarget) {
        match target {
            TextTarget::Search => self.search_changed(),
            TextTarget::LibrarySearch => self.library_search_changed(),
            TextTarget::SeerrSearch => self.seerr_search_changed(),
            _ => {}
        }
    }

    pub fn keyboard_type(&mut self, text: &str) -> bool {
        let Some(target) = self.keyboard.target else {
            return false;
        };
        let mut text = text.to_string();
        if self.keyboard.shift && text.chars().all(|c| c.is_ascii_alphabetic()) {
            text = text.to_uppercase();
            self.keyboard.shift = false;
        }
        target.edit_mut(self).insert(&text);
        self.keyboard_changed(target);
        true
    }

    pub fn keyboard_backspace(&mut self) -> bool {
        let Some(target) = self.keyboard.target else {
            return false;
        };
        target.edit_mut(self).key(
            &KeyEvent {
                key: Key::Backspace,
                mods: Mods::default(),
            },
            None,
        );
        self.keyboard_changed(target);
        true
    }

    pub fn keyboard_enter(&mut self) -> bool {
        let Some(target) = self.keyboard.target else {
            return true;
        };
        match target {
            TextTarget::ConnectServer | TextTarget::ConnectUser | TextTarget::ConnectPassword => {
                if self.connect() {
                    self.keyboard_close();
                }
            }
            TextTarget::GotoTime => {
                self.submit_in_app_goto();
                self.keyboard_close();
            }
            TextTarget::PlaylistName => {
                self.create_playlist_with_detail();
                self.keyboard_close();
            }
            TextTarget::DownloadsPath => {
                self.apply_downloads_path();
                self.keyboard_close();
            }
            TextTarget::OmdbKey
            | TextTarget::SeerrUrl
            | TextTarget::SeerrKey
            | TextTarget::SeerrUser => {
                self.save_preferences();
                self.keyboard_close();
            }
            TextTarget::Search
            | TextTarget::LibrarySearch
            | TextTarget::SettingsSearch
            | TextTarget::SeerrSearch => {
                self.keyboard_close();
            }
        }
        true
    }

    /// A route change drops the sheet and its temporary Show state. The field may be gone,
    /// so no focus is returned.
    pub fn keyboard_reset(&mut self) {
        if let Some(target) = self.keyboard.target {
            target.edit_mut(self).set_revealed(false);
        }
        self.keyboard.reveal = false;
        self.keyboard.reset();
    }

    /// The focus edge, with the sheet's temporary Show state reset when it closes.
    pub fn keyboard_observe(&mut self, focused: Option<&str>, big_screen: bool) -> bool {
        let target = self.keyboard.target;
        let was_open = self.keyboard.open;
        let changed = self.keyboard.observe(focused, big_screen);
        if was_open && !self.keyboard.open {
            if let Some(target) = target {
                target.edit_mut(self).set_revealed(false);
            }
            self.keyboard.reveal = false;
        }
        changed
    }

    /// Hide the sheet and return the secure field to its masked state. The sheet's `Show`
    /// toggle is a temporary reading aid, not a saved field state.
    fn keyboard_close(&mut self) {
        if let Some(target) = self.keyboard.target {
            target.edit_mut(self).set_revealed(false);
        }
        self.keyboard.reveal = false;
        self.keyboard.close();
    }

    pub fn keyboard_hide(&mut self) {
        self.keyboard_close();
    }

    /// Enter on a focused text field asks for the sheet in the TV layout (Auto). The field's
    /// own submit is what On-demand and Off keep.
    pub fn keyboard_enter_open(&mut self, key: &str) -> bool {
        self.big_screen && self.keyboard.manual_open_enter(Some(key))
    }

    pub fn keyboard_next(&mut self) {
        if self.keyboard.target.and_then(TextTarget::next).is_none() {
            self.keyboard_close();
        } else {
            self.keyboard.next();
        }
    }

    pub fn keyboard_toggle_page(&mut self) {
        self.keyboard.toggle_page();
    }

    pub fn keyboard_toggle_shift(&mut self) {
        self.keyboard.toggle_shift();
    }

    pub fn keyboard_toggle_reveal(&mut self) {
        self.keyboard.toggle_reveal();
        let reveal = self.keyboard.reveal;
        if let Some(target) = self.keyboard.target {
            target.edit_mut(self).set_revealed(reveal);
        }
    }

    pub fn cycle_keyboard_mode(&mut self) {
        let mode = self.keyboard.mode.next();
        self.keyboard.mode = mode;
        self.config.keyboard = Some(mode);
        if mode != KeyboardMode::Auto || !self.big_screen {
            self.keyboard.reset();
        }
        self.show_toast(format!("On-screen keyboard: {}", mode.label()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> State {
        let mut state = State::from_config(Config::fresh());
        state.big_screen = true;
        state
    }

    #[test]
    fn every_text_field_has_a_target() {
        let keys = [
            "server",
            "username",
            "password",
            "search-input",
            "library-search",
            "settings-search",
            "seerr-search",
            "setting-omdb-key",
            "setting-seerr-url",
            "setting-seerr-key",
            "setting-seerr-key-input",
            "setting-seerr-user",
            "playlist-name-input",
            "in-app-goto-input",
            "setting-downloads-path",
        ];
        for key in keys {
            assert!(
                TextTarget::from_key(key).is_some(),
                "{key} must be reachable by the on-screen keyboard"
            );
        }
        assert_eq!(TextTarget::from_key("detail-back"), None);
    }

    #[test]
    fn typing_reaches_the_target_and_backspace_removes_it() {
        let mut state = state();
        state.keyboard.open_for("server", TextTarget::ConnectServer);
        assert!(state.keyboard_type("h"));
        assert!(state.keyboard_type("t"));
        assert_eq!(state.server.text, "ht");
        assert!(state.keyboard_backspace());
        assert_eq!(state.server.text, "h");
        assert!(state.keyboard_type("t"));
        assert_eq!(state.server.text, "ht");
    }

    #[test]
    fn shift_applies_to_one_letter_and_secure_values_are_masked() {
        let mut state = state();
        state
            .keyboard
            .open_for("password", TextTarget::ConnectPassword);
        state.keyboard.toggle_shift();
        assert!(state.keyboard_type("a"));
        assert_eq!(state.password.text, "A");
        assert!(!state.keyboard.shift);
        assert!(state.keyboard_type("b"));
        assert_eq!(state.password.text, "Ab");
        assert_eq!(state.keyboard.value(&state), "••");
        state.keyboard_toggle_reveal();
        assert_eq!(state.keyboard.value(&state), "Ab");
    }

    #[test]
    fn typing_in_search_updates_the_query_and_enter_hides_the_sheet() {
        let mut state = state();
        state.route = Route::Search { scope: None };
        state.keyboard.open_for("search-input", TextTarget::Search);
        assert!(state.keyboard_type("d"));
        assert!(state.keyboard_type("u"));
        assert_eq!(state.search.query.text, "du");
        state.keyboard_enter();
        assert!(!state.keyboard.open);
        assert_eq!(state.search.query.text, "du");
    }

    #[test]
    fn focus_never_opens_the_keyboard() {
        let mut state = state();
        assert!(!state.keyboard.observe(Some("nav-home"), true));
        assert!(!state.keyboard.observe(Some("server"), true));
        assert!(
            !state.keyboard.open,
            "passing through a field must not trap the D-pad in the sheet"
        );
        assert!(state.keyboard.manual_open(Some("server")));
        assert_eq!(state.keyboard.target, Some(TextTarget::ConnectServer));
        state.keyboard.close();
        assert!(!state.keyboard.observe(Some("server"), true));
        assert!(!state.keyboard.open);
        // Moving to another field is still not an open.
        assert!(!state.keyboard.observe(Some("username"), true));
        assert!(!state.keyboard.open);
        assert!(state.keyboard.manual_open(Some("username")));
        assert_eq!(state.keyboard.target, Some(TextTarget::ConnectUser));
    }

    #[test]
    fn enter_opens_only_in_auto() {
        let mut state = state();
        state.keyboard.mode = KeyboardMode::OnDemand;
        assert!(!state.keyboard.manual_open_enter(Some("server")));
        assert!(!state.keyboard.open);
        assert!(state.keyboard.manual_open(Some("server")));
        state.keyboard.reset();
        state.keyboard.mode = KeyboardMode::Auto;
        assert!(state.keyboard.manual_open_enter(Some("server")));
    }

    #[test]
    fn on_demand_needs_the_menu_key_and_off_never_opens() {
        let mut state = state();
        state.keyboard.mode = KeyboardMode::OnDemand;
        assert!(!state.keyboard.observe(Some("server"), true));
        assert!(state.keyboard.manual_open(Some("server")));
        state.keyboard.reset();
        state.keyboard.mode = KeyboardMode::Off;
        assert!(!state.keyboard.observe(Some("server"), true));
        assert!(!state.keyboard.manual_open(Some("server")));
    }

    #[test]
    fn enter_in_the_connect_form_advances_to_the_next_field() {
        let mut state = state();
        state.keyboard.open_for("server", TextTarget::ConnectServer);
        state.keyboard_next();
        assert_eq!(state.keyboard.target, Some(TextTarget::ConnectUser));
        assert_eq!(state.keyboard.field_key.as_deref(), Some("username"));
        state.keyboard_next();
        assert_eq!(state.keyboard.target, Some(TextTarget::ConnectPassword));
        state.keyboard_next();
        assert!(!state.keyboard.open);
    }
}
