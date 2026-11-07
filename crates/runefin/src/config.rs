//! Where the session lives between runs: `$XDG_CONFIG_HOME/runefin/config.toml`, mode 0600.
//!
//! The token is a bearer credential for the whole server, so the file is created 0600 and
//! rewritten atomically (write a sibling, then rename) — a crash mid-write must never leave a
//! half-valid config that silently signs the user out.

use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::shortcuts::KeyBindings;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// Normalised: scheme present, no trailing slash.
    #[serde(default)]
    pub server_url: String,
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub user_name: String,
    /// The server's display name, so a restart does not turn the status chip into a URL.
    #[serde(default)]
    pub server_name: String,
    #[serde(default)]
    pub token: String,
    /// Stable per install; the server uses it to group sessions. Regenerated on a fresh config.
    #[serde(default)]
    pub device_id: String,
    /// Optional OMDb fallback for IMDb when the public dataset cannot be downloaded.
    #[serde(default)]
    pub omdb_api_key: Option<String>,
    /// Optional Seerr/Jellyseerr companion. Empty means the app is exactly today's app.
    #[serde(default)]
    pub seerr_url: String,
    /// Seerr API key, admin-level. Stored with the same 0600 rules as the Jellyfin token.
    #[serde(default)]
    pub seerr_api_key: Option<String>,
    /// Optional `X-API-User`; attributes requests to that Seerr user.
    #[serde(default)]
    pub seerr_user_id: Option<i64>,
    /// Display name from the last successful probe. Not a credential.
    #[serde(default)]
    pub seerr_user_name: Option<String>,
    /// Subtitle size 0.7–1.8. Zero means an older file that predates the preference: use 1.0.
    #[serde(default)]
    pub subtitle_size: f32,
    /// Subtitle delay in milliseconds; positive shows the words later.
    #[serde(default)]
    pub subtitle_delay_ms: i64,
    /// Roll to the next episode at the end. `None` in an older file means on, which is what the
    /// player did before this was a preference.
    #[serde(default)]
    pub autoplay_next: Option<bool>,
    /// The plate behind subtitle text.
    #[serde(default)]
    pub subtitle_background: Option<SubtitleBackground>,
    /// Subtitle typography and colour. All optional so a file from an older build keeps the
    /// shipped defaults.
    #[serde(default)]
    pub subtitle_font: Option<SubtitleFont>,
    #[serde(default)]
    pub subtitle_color: Option<SubtitleColor>,
    #[serde(default)]
    pub subtitle_outline: Option<SubtitleOutline>,
    #[serde(default)]
    pub subtitle_position: Option<SubtitlePosition>,
    #[serde(default)]
    pub subtitle_style: Option<SubtitleStyle>,
    /// Text opacity, 0.2–1.0.
    #[serde(default)]
    pub subtitle_opacity: Option<f32>,
    /// Multiplies the plate's own alpha; 0 removes the plate without changing its mode.
    #[serde(default)]
    pub subtitle_background_opacity: Option<f32>,
    /// How the player picks a text track before the viewer touches the menu.
    #[serde(default)]
    pub subtitle_mode: Option<SubtitleMode>,
    /// Whether the player skips server-published intro/recap/credits segments.
    #[serde(default)]
    pub skip_segments: Option<SkipMode>,
    /// Pause a long run with a quiet "Still watching?" card.
    #[serde(default)]
    pub still_watching: Option<bool>,
    #[serde(default)]
    pub still_watching_episodes: Option<i64>,
    #[serde(default)]
    pub still_watching_minutes: Option<i64>,
    /// Start this many seconds before the server's resume point.
    #[serde(default)]
    pub resume_rewind_seconds: Option<i64>,
    /// Fold surround tracks into two channels.
    #[serde(default)]
    pub downmix_stereo: Option<bool>,
    /// Audio shifted against video, in milliseconds (-100 to 100).
    #[serde(default)]
    pub audio_delay_ms: Option<i64>,
    #[serde(default)]
    pub audio_mode: Option<AudioMode>,
    /// Suppress ambient and theme motion. Off by default; the setting is a promise, not a hint.
    #[serde(default)]
    pub reduce_motion: Option<bool>,
    /// The last version chosen per item id, so a restart keeps the edition.
    #[serde(default)]
    pub version_choices: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub playback_quality: Option<PlaybackQuality>,
    #[serde(default)]
    pub quality_choices: Option<std::collections::HashMap<String, PlaybackQuality>>,
    /// Home items the reader hid with “Not interested”.
    #[serde(default)]
    pub hidden_home: Option<Vec<String>>,
    #[serde(default)]
    pub spoiler_guard: Option<bool>,
    #[serde(default)]
    pub saved_views: Option<Vec<SavedView>>,
    #[serde(default)]
    pub last_place: Option<LastPlace>,
    #[serde(default)]
    pub live_favorites: Option<Vec<String>>,
    /// Programs the reader asked to be reminded about, local to this client.
    #[serde(default)]
    pub live_reminders: Option<Vec<LiveReminder>>,
    /// Opt-in desktop notifications through `notify-send`; the in-app toast is always shown.
    #[serde(default)]
    pub notifications: Option<bool>,
    /// Player shortcuts the reader remapped. Absent means every shipped default.
    #[serde(default)]
    pub shortcuts: Option<KeyBindings>,
    #[serde(default)]
    pub home_hero: Option<bool>,
    #[serde(default)]
    pub light_theme: Option<bool>,
    /// `"big-screen"` for the 10-foot layout; absent or `"desktop"` is the desk UI.
    #[serde(default)]
    pub ui_mode: Option<String>,
    /// Reader text size, 0.85–1.35. Absent means 1.0.
    #[serde(default)]
    pub text_scale: Option<f32>,
    #[serde(default)]
    pub high_contrast: Option<bool>,
    /// Three-letter code (`eng`, `spa`); empty or absent means "no preference".
    #[serde(default)]
    pub subtitle_language: Option<String>,
    #[serde(default)]
    pub audio_language: Option<String>,
    /// Arrow-key and OSD seek distance in seconds. Absent means ten.
    #[serde(default)]
    pub seek_seconds: Option<i64>,
    /// Player volume, 0–2. Absent means 1.0, which GStreamer calls "full volume".
    #[serde(default)]
    pub volume: Option<f64>,
    /// Last window size in points; zero means "use the default window".
    #[serde(default)]
    pub window_width: f32,
    #[serde(default)]
    pub window_height: f32,
    /// Last window origin in screen points. Absent or negative means "let the compositor place it".
    #[serde(default)]
    pub window_x: Option<i32>,
    #[serde(default)]
    pub window_y: Option<i32>,
}

/// The plate behind subtitle text. Dim is the shipped default; Off suits letterboxed films
/// whose subtitles already sit on black bars.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubtitleBackground {
    #[default]
    Dim,
    Solid,
    Off,
}

impl SubtitleBackground {
    pub fn label(self) -> &'static str {
        match self {
            SubtitleBackground::Dim => "Dim",
            SubtitleBackground::Solid => "Solid",
            SubtitleBackground::Off => "Off",
        }
    }

    pub fn alpha(self) -> f32 {
        match self {
            SubtitleBackground::Dim => 0.58,
            SubtitleBackground::Solid => 0.86,
            SubtitleBackground::Off => 0.0,
        }
    }

    pub fn next(self) -> SubtitleBackground {
        match self {
            SubtitleBackground::Dim => SubtitleBackground::Solid,
            SubtitleBackground::Solid => SubtitleBackground::Off,
            SubtitleBackground::Off => SubtitleBackground::Dim,
        }
    }
}

/// A subtitle face that can be resolved without leaking a runtime string into Rune's
/// `Family::Named`, which needs a `'static` name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubtitleFont {
    #[default]
    Sans,
    Serif,
    Mono,
}

impl SubtitleFont {
    pub fn label(self) -> &'static str {
        match self {
            SubtitleFont::Sans => "Sans",
            SubtitleFont::Serif => "Serif",
            SubtitleFont::Mono => "Mono",
        }
    }

    pub fn next(self) -> SubtitleFont {
        match self {
            SubtitleFont::Sans => SubtitleFont::Serif,
            SubtitleFont::Serif => SubtitleFont::Mono,
            SubtitleFont::Mono => SubtitleFont::Sans,
        }
    }
}

/// Preset colours that stay legible over bright and dark scenes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubtitleColor {
    #[default]
    White,
    Warm,
    Yellow,
    Cyan,
    Green,
    Pink,
}

impl SubtitleColor {
    pub fn label(self) -> &'static str {
        match self {
            SubtitleColor::White => "White",
            SubtitleColor::Warm => "Warm",
            SubtitleColor::Yellow => "Yellow",
            SubtitleColor::Cyan => "Cyan",
            SubtitleColor::Green => "Green",
            SubtitleColor::Pink => "Pink",
        }
    }

    pub fn rgb(self) -> [u8; 3] {
        match self {
            SubtitleColor::White => [255, 255, 255],
            SubtitleColor::Warm => [255, 236, 196],
            SubtitleColor::Yellow => [255, 226, 84],
            SubtitleColor::Cyan => [128, 235, 255],
            SubtitleColor::Green => [150, 245, 160],
            SubtitleColor::Pink => [255, 170, 205],
        }
    }

    pub fn next(self) -> SubtitleColor {
        match self {
            SubtitleColor::White => SubtitleColor::Warm,
            SubtitleColor::Warm => SubtitleColor::Yellow,
            SubtitleColor::Yellow => SubtitleColor::Cyan,
            SubtitleColor::Cyan => SubtitleColor::Green,
            SubtitleColor::Green => SubtitleColor::Pink,
            SubtitleColor::Pink => SubtitleColor::White,
        }
    }
}

/// A text outline, faked with offset shadows because Rune text has no stroke.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubtitleOutline {
    #[default]
    Off,
    Thin,
    Thick,
}

impl SubtitleOutline {
    pub fn label(self) -> &'static str {
        match self {
            SubtitleOutline::Off => "Off",
            SubtitleOutline::Thin => "Thin",
            SubtitleOutline::Thick => "Thick",
        }
    }

    pub fn width(self) -> f32 {
        match self {
            SubtitleOutline::Off => 0.0,
            SubtitleOutline::Thin => 1.4,
            SubtitleOutline::Thick => 2.6,
        }
    }

    pub fn next(self) -> SubtitleOutline {
        match self {
            SubtitleOutline::Off => SubtitleOutline::Thin,
            SubtitleOutline::Thin => SubtitleOutline::Thick,
            SubtitleOutline::Thick => SubtitleOutline::Off,
        }
    }
}

/// Where plain SRT/VTT cues sit. ASS cues keep the alignment and `\pos` their author chose.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubtitlePosition {
    #[default]
    Bottom,
    Middle,
    Top,
}

impl SubtitlePosition {
    pub fn label(self) -> &'static str {
        match self {
            SubtitlePosition::Bottom => "Bottom",
            SubtitlePosition::Middle => "Middle",
            SubtitlePosition::Top => "Top",
        }
    }

    pub fn band(self) -> u8 {
        match self {
            SubtitlePosition::Top => 0,
            SubtitlePosition::Middle => 1,
            SubtitlePosition::Bottom => 2,
        }
    }

    pub fn next(self) -> SubtitlePosition {
        match self {
            SubtitlePosition::Bottom => SubtitlePosition::Middle,
            SubtitlePosition::Middle => SubtitlePosition::Top,
            SubtitlePosition::Top => SubtitlePosition::Bottom,
        }
    }
}

/// Weight/italic preset for plain cues; ASS inline tags still win inside a cue.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubtitleStyle {
    #[default]
    Normal,
    Bold,
    Italic,
    BoldItalic,
}

impl SubtitleStyle {
    pub fn label(self) -> &'static str {
        match self {
            SubtitleStyle::Normal => "Normal",
            SubtitleStyle::Bold => "Bold",
            SubtitleStyle::Italic => "Italic",
            SubtitleStyle::BoldItalic => "Bold italic",
        }
    }

    pub fn bold(self) -> bool {
        matches!(self, SubtitleStyle::Bold | SubtitleStyle::BoldItalic)
    }

    pub fn italic(self) -> bool {
        matches!(self, SubtitleStyle::Italic | SubtitleStyle::BoldItalic)
    }

    pub fn next(self) -> SubtitleStyle {
        match self {
            SubtitleStyle::Normal => SubtitleStyle::Bold,
            SubtitleStyle::Bold => SubtitleStyle::Italic,
            SubtitleStyle::Italic => SubtitleStyle::BoldItalic,
            SubtitleStyle::BoldItalic => SubtitleStyle::Normal,
        }
    }
}

/// What the player does with text tracks on its own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubtitleMode {
    /// Never pick one; the viewer opens the menu.
    Off,
    /// Pick a forced track, which only carries what the dub leaves foreign.
    #[default]
    Forced,
    /// Pick the preferred language, or the first track when there is no preference.
    Always,
}

impl SubtitleMode {
    pub fn label(self) -> &'static str {
        match self {
            SubtitleMode::Off => "Off",
            SubtitleMode::Forced => "Forced only",
            SubtitleMode::Always => "Always",
        }
    }

    /// The next mode in the cycle Off → Forced → Always → Off.
    pub fn next(self) -> SubtitleMode {
        match self {
            SubtitleMode::Off => SubtitleMode::Forced,
            SubtitleMode::Forced => SubtitleMode::Always,
            SubtitleMode::Always => SubtitleMode::Off,
        }
    }
}

/// What the player does when the server marks an intro, recap or credits segment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkipMode {
    /// Show the skip button; never move the playhead on its own.
    #[default]
    Ask,
    /// Seek past the segment and offer Undo.
    Always,
    /// Do not show the button and do not skip.
    Never,
}

impl SkipMode {
    pub fn label(self) -> &'static str {
        match self {
            SkipMode::Ask => "Ask",
            SkipMode::Always => "Always",
            SkipMode::Never => "Never",
        }
    }

    pub fn next(self) -> SkipMode {
        match self {
            SkipMode::Ask => SkipMode::Always,
            SkipMode::Always => SkipMode::Never,
            SkipMode::Never => SkipMode::Ask,
        }
    }
}

/// The audio shaping the player offers without a DSP dependency.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioMode {
    #[default]
    Off,
    /// Lift dialogue over the mix: a gentle compressor.
    Dialogue,
    /// Compress the whole range for quiet listening at night.
    Night,
}

impl AudioMode {
    pub fn label(self) -> &'static str {
        match self {
            AudioMode::Off => "Off",
            AudioMode::Dialogue => "Dialogue boost",
            AudioMode::Night => "Night mode",
        }
    }

    pub fn next(self) -> AudioMode {
        match self {
            AudioMode::Off => AudioMode::Dialogue,
            AudioMode::Dialogue => AudioMode::Night,
            AudioMode::Night => AudioMode::Off,
        }
    }
}

/// What the reader asked the server for. Auto and Original both ask for the file itself;
/// Auto may still be converted when the profile cannot reproduce it, Original does not cap.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackQuality {
    #[default]
    Auto,
    Original,
    Mbps20,
    Mbps12,
    Mbps8,
    Mbps4,
    Mbps2,
}

impl PlaybackQuality {
    pub fn label(self) -> &'static str {
        match self {
            PlaybackQuality::Auto => "Auto",
            PlaybackQuality::Original => "Original",
            PlaybackQuality::Mbps20 => "20 Mbps",
            PlaybackQuality::Mbps12 => "12 Mbps",
            PlaybackQuality::Mbps8 => "8 Mbps",
            PlaybackQuality::Mbps4 => "4 Mbps",
            PlaybackQuality::Mbps2 => "2 Mbps",
        }
    }

    /// The bitrate cap sent as `MaxStreamingBitrate`. Auto and Original send none.
    pub fn max_bitrate(self) -> Option<i64> {
        match self {
            PlaybackQuality::Auto | PlaybackQuality::Original => None,
            PlaybackQuality::Mbps20 => Some(20_000_000),
            PlaybackQuality::Mbps12 => Some(12_000_000),
            PlaybackQuality::Mbps8 => Some(8_000_000),
            PlaybackQuality::Mbps4 => Some(4_000_000),
            PlaybackQuality::Mbps2 => Some(2_000_000),
        }
    }

    pub fn next(self) -> PlaybackQuality {
        match self {
            PlaybackQuality::Auto => PlaybackQuality::Original,
            PlaybackQuality::Original => PlaybackQuality::Mbps20,
            PlaybackQuality::Mbps20 => PlaybackQuality::Mbps12,
            PlaybackQuality::Mbps12 => PlaybackQuality::Mbps8,
            PlaybackQuality::Mbps8 => PlaybackQuality::Mbps4,
            PlaybackQuality::Mbps4 => PlaybackQuality::Mbps2,
            PlaybackQuality::Mbps2 => PlaybackQuality::Auto,
        }
    }
}

/// A named library filter/sort combination. Local, like the hidden Home ids.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedView {
    pub name: String,
    pub sort: crate::library::SortMode,
    pub ascending: bool,
    pub unwatched: bool,
    pub favorites: bool,
    pub genre: Option<(String, String)>,
    pub year: Option<i32>,
    pub min_rating: Option<u8>,
    pub audio_language: Option<String>,
    pub subtitle_language: Option<String>,
    pub has_subtitles: bool,
    pub only_4k: bool,
    pub missing_only: bool,
    pub unaired_only: bool,
    pub letter: Option<String>,
    pub search: Option<String>,
}

impl SavedView {
    pub fn label(&self) -> String {
        let filters = [
            self.unwatched,
            self.favorites,
            self.genre.is_some(),
            self.year.is_some(),
            self.min_rating.is_some(),
            self.audio_language.is_some(),
            self.subtitle_language.is_some(),
            self.has_subtitles,
            self.only_4k,
            self.missing_only,
            self.unaired_only,
            self.letter.is_some(),
            self.search.is_some(),
        ]
        .into_iter()
        .filter(|filter| *filter)
        .count();
        if filters == 0 {
            format!("{} · {}", self.name, self.sort.label())
        } else {
            format!("{} · {} · {}", self.name, self.sort.label(), filters)
        }
    }
}

/// The place the window was last showing, for crash-safe navigation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LastPlace {
    /// `home`, `library`, `detail` or `livetv`.
    pub kind: String,
    pub id: Option<String>,
    pub name: Option<String>,
    pub collection_type: Option<String>,
}

/// A live program the reader wants a local reminder for. Everything needed to notify without a
/// server request is stored, so the reminder survives a restart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveReminder {
    pub program_id: String,
    pub channel_id: String,
    pub channel_name: String,
    pub program_name: String,
    /// Unix seconds; the program's start on the server's clock.
    pub start_seconds: i64,
}

impl Config {
    pub fn path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("runefin").join("config.toml"))
    }

    /// Missing, unreadable or corrupt all mean "no session"; the app then asks to sign in.
    pub fn load() -> Config {
        let Some(path) = Self::path() else {
            return Config::fresh();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let mut config: Config = toml::from_str(&text).unwrap_or_default();
                if config.device_id.is_empty() {
                    config.device_id = new_device_id();
                }
                config
            }
            Err(_) => Config::fresh(),
        }
    }

    pub fn fresh() -> Config {
        Config {
            device_id: new_device_id(),
            ..Config::default()
        }
    }

    pub fn save(&self) -> io::Result<()> {
        let Some(path) = Self::path() else {
            return Err(io::Error::other("no config directory"));
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
            }
        }
        let text = toml::to_string_pretty(self).map_err(io::Error::other)?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(&tmp, &path)
    }

    /// Keeps the server and the device id, drops who was signed in.
    pub fn sign_out(&mut self) {
        self.user_id.clear();
        self.user_name.clear();
        self.token.zeroize();
        self.token.clear();
    }

    /// The saved interface mode. Big screen is the TV layout; everything else is the desk UI.
    pub fn big_screen(&self) -> bool {
        self.ui_mode.as_deref() == Some("big-screen")
    }

    pub fn has_session(&self) -> bool {
        !self.server_url.is_empty() && !self.token.is_empty() && !self.user_id.is_empty()
    }
}

// No `Drop` impl on purpose: a `Drop` type cannot be moved out of with struct-update syntax
// (`..Config::default()`), which the tests and `Config::fresh` both rely on. Instead the token is
// wiped wherever it is replaced or cleared — see `sign_out` and `State::apply`.

/// 16 hex characters from the hostname and the clock: enough to be unique per install, and
/// deliberately not a real UUID, so nobody mistakes it for a user identifier.
fn new_device_id() -> String {
    use std::hash::{Hash, Hasher};
    let host = std::env::var("HOSTNAME").unwrap_or_default();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    host.hash(&mut hasher);
    nanos.hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_config_is_not_signed_in() {
        let c = Config::fresh();
        assert!(!c.has_session());
        assert_eq!(c.device_id.len(), 16);
    }

    #[test]
    fn signing_out_keeps_the_server_but_forgets_the_user() {
        let mut c = Config {
            server_url: "http://198.51.100.2:8096".into(),
            user_id: "u1".into(),
            user_name: "luis".into(),
            server_name: "example".into(),
            token: "secret".into(),
            device_id: "dev".into(),
            ..Config::default()
        };
        assert!(c.has_session());
        c.sign_out();
        assert!(!c.has_session());
        assert_eq!(c.server_url, "http://198.51.100.2:8096");
        assert!(c.token.is_empty() && c.user_id.is_empty());
    }

    #[test]
    fn the_config_round_trips_through_toml() {
        let c = Config {
            server_url: "http://198.51.100.2:8096".into(),
            user_id: "u1".into(),
            user_name: "luis".into(),
            server_name: "example".into(),
            token: "secret".into(),
            device_id: "dev".into(),
            ..Config::default()
        };
        let text = toml::to_string_pretty(&c).expect("serialises");
        let back: Config = toml::from_str(&text).expect("parses");
        assert_eq!(back.server_url, c.server_url);
        assert_eq!(back.token, c.token);
        assert!(back.has_session());
    }

    #[test]
    fn playback_preferences_default_when_an_older_file_lacks_them() {
        let back: Config = toml::from_str("server_url = \"http://x\"\n").expect("parses");
        assert_eq!(
            back.autoplay_next, None,
            "absent means on, decided by the app"
        );
        assert_eq!(back.subtitle_mode, None);
        assert_eq!(back.seek_seconds, None);
    }

    #[test]
    fn playback_quality_caps_are_explicit() {
        assert_eq!(PlaybackQuality::Auto.max_bitrate(), None);
        assert_eq!(PlaybackQuality::Original.max_bitrate(), None);
        assert_eq!(PlaybackQuality::Mbps2.max_bitrate(), Some(2_000_000));
        assert_eq!(PlaybackQuality::Mbps12.max_bitrate(), Some(12_000_000));
        assert_eq!(PlaybackQuality::Auto.next(), PlaybackQuality::Original);
        assert_eq!(PlaybackQuality::Mbps2.next(), PlaybackQuality::Auto);
    }

    #[test]
    fn skip_modes_serialise_lowercase_and_cycle() {
        let c = Config {
            skip_segments: Some(SkipMode::Always),
            ..Config::default()
        };
        let text = toml::to_string(&c).expect("serialises");
        assert!(text.contains("skip_segments = \"always\""), "{text}");
        assert_eq!(SkipMode::default(), SkipMode::Ask);
        assert_eq!(SkipMode::Ask.next(), SkipMode::Always);
        assert_eq!(SkipMode::Always.next(), SkipMode::Never);
        assert_eq!(SkipMode::Never.next(), SkipMode::Ask);
    }

    #[test]
    fn subtitle_modes_serialise_lowercase_and_cycle() {
        let c = Config {
            subtitle_mode: Some(SubtitleMode::Always),
            ..Config::default()
        };
        let text = toml::to_string(&c).expect("serialises");
        assert!(text.contains("subtitle_mode = \"always\""), "{text}");
        assert_eq!(SubtitleMode::default(), SubtitleMode::Forced);
        assert_eq!(SubtitleMode::Off.next(), SubtitleMode::Forced);
        assert_eq!(SubtitleMode::Forced.next(), SubtitleMode::Always);
        assert_eq!(SubtitleMode::Always.next(), SubtitleMode::Off);
    }

    #[test]
    fn the_subtitle_background_cycles_and_serialises_lowercase() {
        let c = Config {
            subtitle_background: Some(SubtitleBackground::Solid),
            ..Config::default()
        };
        let text = toml::to_string(&c).expect("serialises");
        assert!(text.contains("subtitle_background = \"solid\""), "{text}");
        assert_eq!(SubtitleBackground::default(), SubtitleBackground::Dim);
        assert_eq!(SubtitleBackground::Dim.next(), SubtitleBackground::Solid);
        assert_eq!(SubtitleBackground::Solid.next(), SubtitleBackground::Off);
        assert_eq!(SubtitleBackground::Off.next(), SubtitleBackground::Dim);
        assert_eq!(SubtitleBackground::Off.alpha(), 0.0);
        assert!(SubtitleBackground::Solid.alpha() > SubtitleBackground::Dim.alpha());
    }

    #[test]
    fn an_omdb_key_round_trips_and_is_absent_in_an_older_file() {
        let c = Config {
            omdb_api_key: Some("abcd1234".into()),
            ..Config::default()
        };
        let text = toml::to_string(&c).expect("serialises");
        assert!(text.contains("omdb_api_key = \"abcd1234\""), "{text}");
        let back: Config = toml::from_str(&text).expect("parses");
        assert_eq!(back.omdb_api_key.as_deref(), Some("abcd1234"));
        let old: Config = toml::from_str("server_url = \"http://x\"\n").expect("parses");
        assert_eq!(old.omdb_api_key, None);
    }

    #[test]
    fn the_seerr_settings_round_trip_and_survive_a_jellyfin_sign_out() {
        let mut c = Config {
            server_url: "http://198.51.100.2:8096".into(),
            user_id: "u1".into(),
            user_name: "luis".into(),
            token: "secret".into(),
            seerr_url: "https://jellyseerr.example".into(),
            seerr_api_key: Some("seerr-key".into()),
            seerr_user_id: Some(1),
            seerr_user_name: Some("luis".into()),
            ..Config::default()
        };
        let text = toml::to_string(&c).expect("serialises");
        assert!(text.contains("seerr_api_key = \"seerr-key\""), "{text}");
        let back: Config = toml::from_str(&text).expect("parses");
        assert_eq!(back.seerr_url, "https://jellyseerr.example");
        assert_eq!(back.seerr_api_key.as_deref(), Some("seerr-key"));
        assert_eq!(back.seerr_user_id, Some(1));
        assert_eq!(back.seerr_user_name.as_deref(), Some("luis"));

        c.sign_out();
        assert_eq!(c.seerr_api_key.as_deref(), Some("seerr-key"));
        assert_eq!(c.seerr_url, "https://jellyseerr.example");
        assert!(!c.has_session());
    }

    #[test]
    fn a_config_without_seerr_fields_still_loads() {
        let old: Config = toml::from_str("server_url = \"http://x\"\n").expect("parses");
        assert!(old.seerr_url.is_empty());
        assert_eq!(old.seerr_api_key, None);
        assert_eq!(old.seerr_user_id, None);
    }

    #[test]
    fn subtitle_typography_cycles_and_round_trips() {
        let c = Config {
            subtitle_font: Some(SubtitleFont::Serif),
            subtitle_color: Some(SubtitleColor::Yellow),
            subtitle_outline: Some(SubtitleOutline::Thin),
            subtitle_position: Some(SubtitlePosition::Top),
            subtitle_style: Some(SubtitleStyle::BoldItalic),
            subtitle_opacity: Some(0.8),
            subtitle_background_opacity: Some(0.4),
            ..Config::default()
        };
        let text = toml::to_string(&c).expect("serialises");
        for line in [
            "subtitle_font = \"serif\"",
            "subtitle_color = \"yellow\"",
            "subtitle_outline = \"thin\"",
            "subtitle_position = \"top\"",
            "subtitle_style = \"bolditalic\"",
            "subtitle_opacity = 0.8",
        ] {
            assert!(text.contains(line), "{line} in {text}");
        }
        let back: Config = toml::from_str(&text).expect("parses");
        assert_eq!(back.subtitle_font, Some(SubtitleFont::Serif));
        assert_eq!(back.subtitle_style, Some(SubtitleStyle::BoldItalic));
        assert!(back.subtitle_style.is_some_and(SubtitleStyle::italic));
        assert!(back.subtitle_style.is_some_and(SubtitleStyle::bold));
        assert_eq!(SubtitleFont::Mono.next(), SubtitleFont::Sans);
        assert_eq!(SubtitleColor::Pink.next(), SubtitleColor::White);
        assert_eq!(SubtitleOutline::Thick.next(), SubtitleOutline::Off);
        assert_eq!(SubtitlePosition::Bottom.next(), SubtitlePosition::Middle);
        assert_eq!(SubtitleStyle::BoldItalic.next(), SubtitleStyle::Normal);
        assert_eq!(SubtitlePosition::Top.band(), 0);
        assert_eq!(SubtitleColor::Yellow.rgb(), [255, 226, 84]);
    }

    #[test]
    fn the_volume_round_trips_and_is_absent_in_an_older_file() {
        let c = Config {
            volume: Some(0.65),
            ..Config::default()
        };
        let text = toml::to_string(&c).expect("serialises");
        assert!(text.contains("volume = 0.65"), "{text}");
        let back: Config = toml::from_str(&text).expect("parses");
        assert_eq!(back.volume, Some(0.65));
        let old: Config = toml::from_str("server_url = \"http://x\"\n").expect("parses");
        assert_eq!(old.volume, None, "absent means full volume");
    }

    #[test]
    fn a_config_missing_optional_fields_still_loads() {
        let back: Config = toml::from_str("server_url = \"http://x\"\n").expect("parses");
        assert_eq!(back.server_url, "http://x");
        assert!(!back.has_session());
    }
}
