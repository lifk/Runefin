//! MPRIS2 over the session bus: what the desktop's media widget, `playerctl` and the lock
//! screen read while a film plays.
//!
//! The service lives on its own thread with a current-thread Tokio runtime; the UI thread only
//! publishes an immutable snapshot and drains a small command queue, so a missing or busy bus
//! can never block a frame. zbus and Tokio are already in the tree for Rune's D-Bus services.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use zbus::{OwnedObjectPath, OwnedVariant, Variant};

const PATH: &str = "/org/mpris/MediaPlayer2";
const NAME: &str = "org.mpris.MediaPlayer2.runefin";

/// What the service exposes. The UI thread replaces it; the bus thread reads it.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub status: PlaybackStatus,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub art_url: Option<String>,
    pub track_id: String,
    pub length_us: i64,
    pub position_us: i64,
    pub volume: f64,
    pub rate: f64,
    pub can_next: bool,
    pub can_previous: bool,
    pub can_seek: bool,
    pub fullscreen: bool,
    pub shuffle: bool,
    pub loop_status: String,
}

impl Default for Snapshot {
    fn default() -> Self {
        Snapshot {
            status: PlaybackStatus::Stopped,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            art_url: None,
            track_id: String::new(),
            length_us: 0,
            position_us: 0,
            // A stopped player still has a rate and a volume; zero would tell a desktop widget
            // the app is muted and frozen.
            volume: 1.0,
            rate: 1.0,
            can_next: false,
            can_previous: false,
            can_seek: false,
            fullscreen: false,
            shuffle: false,
            loop_status: "None".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlaybackStatus {
    #[default]
    Stopped,
    Playing,
    Paused,
}

impl PlaybackStatus {
    fn as_str(self) -> &'static str {
        match self {
            PlaybackStatus::Stopped => "Stopped",
            PlaybackStatus::Playing => "Playing",
            PlaybackStatus::Paused => "Paused",
        }
    }
}

/// A remote request, drained on the UI thread. Keeping it a value means the bus thread never
/// touches `State`.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
    /// Relative, in microseconds.
    Seek(i64),
    /// Absolute, in microseconds.
    SetPosition(i64),
    SetVolume(f64),
    SetRate(f64),
    SetFullscreen(bool),
    SetShuffle(bool),
    /// `None`, `Track` or `Playlist`.
    SetLoopStatus(String),
}

#[derive(Default)]
struct Inner {
    snapshot: Mutex<Snapshot>,
    commands: Mutex<VecDeque<Command>>,
}

impl Inner {
    fn snapshot(&self) -> Snapshot {
        self.snapshot
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }

    fn push(&self, command: Command) {
        self.commands
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .push_back(command);
    }
}

/// The app's handle to the service. `None` simply means no MPRIS, which is what a headless
/// render and a session without a bus get.
#[derive(Clone)]
pub struct Mpris {
    inner: Arc<Inner>,
}

impl Mpris {
    pub fn start() -> Option<Mpris> {
        let inner = Arc::new(Inner::default());
        let served = Arc::clone(&inner);
        std::thread::Builder::new()
            .name("runefin-mpris".into())
            .spawn(move || serve(served))
            .ok()?;
        Some(Mpris { inner })
    }

    pub fn publish(&self, snapshot: Snapshot) {
        *self
            .inner
            .snapshot
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = snapshot;
    }

    pub fn take_commands(&self) -> Vec<Command> {
        let mut commands = self
            .inner
            .commands
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        commands.drain(..).collect()
    }
}

fn serve(inner: Arc<Inner>) {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return;
    };
    runtime.block_on(async move {
        let result = async {
            let connection = zbus::connection::Builder::session()
                .name(NAME)
                .serve_at(
                    PATH,
                    Root {
                        inner: Arc::clone(&inner),
                    },
                )
                .serve_at(
                    PATH,
                    Player {
                        inner: Arc::clone(&inner),
                    },
                )
                .build()
                .await?;
            std::future::pending::<()>().await;
            drop(connection);
            Ok::<(), zbus::Error>(())
        }
        .await;
        if let Err(error) = result
            && cfg!(debug_assertions)
        {
            eprintln!("runefin: MPRIS unavailable: {error}");
        }
    });
}

struct Root {
    inner: Arc<Inner>,
}

#[zbus::interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    #[zbus(property)]
    fn identity(&self) -> &str {
        "Runefin"
    }

    #[zbus(property)]
    fn desktop_entry(&self) -> &str {
        "runefin"
    }

    #[zbus(property)]
    fn can_quit(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn can_raise(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn can_set_fullscreen(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn fullscreen(&self) -> bool {
        self.inner.snapshot().fullscreen
    }

    #[zbus(property)]
    fn set_fullscreen(&self, fullscreen: bool) {
        self.inner.push(Command::SetFullscreen(fullscreen));
    }

    fn raise(&self) {}

    fn quit(&self) {}
}

struct Player {
    inner: Arc<Inner>,
}

#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    #[zbus(property)]
    fn playback_status(&self) -> String {
        self.inner.snapshot().status.as_str().to_string()
    }

    #[zbus(property)]
    fn loop_status(&self) -> String {
        self.inner.snapshot().loop_status
    }

    #[zbus(property)]
    fn set_loop_status(&self, status: String) {
        self.inner.push(Command::SetLoopStatus(status));
    }

    #[zbus(property)]
    fn rate(&self) -> f64 {
        self.inner.snapshot().rate
    }

    #[zbus(property)]
    fn set_rate(&self, rate: f64) {
        self.inner.push(Command::SetRate(rate));
    }

    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        0.5
    }

    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        2.0
    }

    #[zbus(property)]
    fn shuffle(&self) -> bool {
        self.inner.snapshot().shuffle
    }

    #[zbus(property)]
    fn set_shuffle(&self, shuffle: bool) {
        self.inner.push(Command::SetShuffle(shuffle));
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedVariant> {
        let snapshot = self.inner.snapshot();
        let mut metadata = HashMap::new();
        insert(
            &mut metadata,
            "mpris:trackid",
            track_path(&snapshot.track_id),
        );
        insert(
            &mut metadata,
            "mpris:length",
            Variant::from(snapshot.length_us),
        );
        insert(&mut metadata, "xesam:title", Variant::from(snapshot.title));
        if !snapshot.artist.is_empty() {
            insert(
                &mut metadata,
                "xesam:artist",
                Variant::new(vec![snapshot.artist]),
            );
        }
        if !snapshot.album.is_empty() {
            insert(&mut metadata, "xesam:album", Variant::from(snapshot.album));
        }
        if let Some(art) = snapshot.art_url {
            insert(&mut metadata, "mpris:artUrl", Variant::from(art));
        }
        metadata
    }

    #[zbus(property)]
    fn volume(&self) -> f64 {
        self.inner.snapshot().volume
    }

    #[zbus(property)]
    fn set_volume(&self, volume: f64) {
        self.inner.push(Command::SetVolume(volume));
    }

    #[zbus(property)]
    fn position(&self) -> i64 {
        self.inner.snapshot().position_us
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        self.inner.snapshot().can_next
    }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        self.inner.snapshot().can_previous
    }

    #[zbus(property)]
    fn can_play(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_seek(&self) -> bool {
        self.inner.snapshot().can_seek
    }

    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }

    fn next(&self) {
        self.inner.push(Command::Next);
    }

    fn previous(&self) {
        self.inner.push(Command::Previous);
    }

    fn pause(&self) {
        self.inner.push(Command::Pause);
    }

    fn play_pause(&self) {
        self.inner.push(Command::PlayPause);
    }

    fn stop(&self) {
        self.inner.push(Command::Stop);
    }

    fn play(&self) {
        self.inner.push(Command::Play);
    }

    fn seek(&self, offset: i64) {
        self.inner.push(Command::Seek(offset));
    }

    fn set_position(&self, _track_id: OwnedObjectPath, position: i64) {
        self.inner.push(Command::SetPosition(position));
    }

    fn open_uri(&self, _uri: &str) {}
}

fn insert(metadata: &mut HashMap<String, OwnedVariant>, key: &str, value: Variant<'static>) {
    if let Ok(value) = OwnedVariant::try_from(value) {
        metadata.insert(key.to_string(), value);
    }
}

/// A valid D-Bus object path for `mpris:trackid`; item ids can contain hyphens and slashes.
fn track_path(item_id: &str) -> Variant<'static> {
    let safe: String = item_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let path = format!("/org/mpris/MediaPlayer2/track/{safe}");
    match OwnedObjectPath::try_from(path) {
        Ok(path) => Variant::new(path),
        Err(_) => Variant::from("/org/mpris/MediaPlayer2/track/unknown"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_track_id_becomes_a_valid_object_path() {
        let path = track_path("episode 1/2");
        assert_eq!(path.value_signature().to_string(), "o");
        assert!(path.to_string().contains("episode_1_2"));
    }

    #[test]
    fn commands_queue_and_drain_in_order() {
        let inner = Arc::new(Inner::default());
        inner.push(Command::Pause);
        inner.push(Command::Seek(10));
        let mpris = Mpris {
            inner: Arc::clone(&inner),
        };
        assert_eq!(
            mpris.take_commands(),
            vec![Command::Pause, Command::Seek(10)]
        );
        assert!(mpris.take_commands().is_empty());
    }
}
