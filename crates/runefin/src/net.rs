//! Everything that can block: Jellyfin requests and (later) the mpv process.
//!
//! Rune calls `poll` on the UI thread every frame, so no call may block there. Each action spawns
//! a thread that pushes one outcome and rings the waker Rune handed us in `setup`; `poll` drains
//! the queue and returns whether anything visible changed.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use jellyfin_api::{Client, PlaybackReport, PlaybackStop};
use rune::Waker;

use crate::handoff::PlaybackEvent;

/// Wall time of the most recent worker request. Diagnostics reads it; nothing schedules on it.
static LAST_JOB_MS: AtomicU64 = AtomicU64::new(0);

pub fn last_job_ms() -> u64 {
    LAST_JOB_MS.load(Ordering::Relaxed)
}

/// Which half of signing in failed — the connect screen says different things for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Server,
    Credentials,
    Player,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub stage: Stage,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub server_url: String,
    pub server_name: String,
    pub version: String,
    pub user_id: String,
    pub user_name: String,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg {
    Connected(Result<Session, Failure>),
    /// One poster fetched (or read from the disk cache) by the image worker.
    Image {
        key: String,
        bytes: Result<Vec<u8>, String>,
        /// Straight-alpha RGBA at `image::PLACEHOLDER_WIDTH` × `PLACEHOLDER_HEIGHT`.
        placeholder: Option<Vec<u8>>,
    },
    /// One event from the mpv IPC reader, tagged with the session that owns the socket.
    Playback {
        generation: u64,
        event: PlaybackEvent,
    },
    /// A Jellyfin playback report finished. The stop report is what triggers the home refresh.
    Reported {
        kind: ReportKind,
        result: Result<(), String>,
    },
    /// A remote-control command finished.
    Remote(Result<(), String>),
    /// A favourite write finished.
    Favorite(Result<(), String>),
    /// A watched-state write finished.
    Played(Result<(), String>),
    /// A like/dislike write finished.
    Rated(Result<(), String>),
    /// A playlist reorder finished.
    PlaylistOrder(Result<(), String>),
    /// A playlist add finished (used by Undo).
    PlaylistAdded(Result<(), String>),
    /// A collection write finished.
    Collection(Result<(), String>),
    /// A Continue Watching removal finished.
    ResumeCleared(Result<(), String>),
    /// The poster cache clear finished, with the number of files removed.
    ImagesCleared(Result<usize, String>),
    /// A Seerr request write finished. Boxed: the 201 body is much larger than every other Msg.
    SeerrRequested(Result<Box<seerr_api::MediaRequest>, String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportKind {
    Started,
    Progress,
    Stopped,
}

/// The current screen's images outrank rails the user is not looking at yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePriority {
    High,
    Low,
}

/// Where an image comes from. Jellyfin images use the session client; TMDB posters are public and
/// do not carry the Jellyfin token. The worker's disk cache is shared by both.
#[derive(Clone)]
pub enum ImageSource {
    Jellyfin(Client),
    Public,
}

struct ImageRequest {
    source: ImageSource,
    key: String,
    url: String,
    /// Tried when the first URL answers with an empty body. Jellyfin's home server has a few
    /// items whose Primary image is 0 bytes while their Backdrop is fine.
    fallback_url: Option<String>,
}

#[derive(Default)]
struct ImageQueue {
    high: VecDeque<ImageRequest>,
    low: VecDeque<ImageRequest>,
    closed: bool,
}

impl ImageQueue {
    fn pop(&mut self) -> Option<ImageRequest> {
        self.high.pop_front().or_else(|| self.low.pop_front())
    }
}

fn image_worker(
    queue: Arc<Mutex<Vec<Msg>>>,
    wake: Arc<Mutex<Option<Waker>>>,
    images: Arc<(Mutex<ImageQueue>, Condvar)>,
) {
    loop {
        let request = {
            let (slot, ready) = &*images;
            let mut slot = slot.lock().unwrap_or_else(|error| error.into_inner());
            loop {
                if let Some(request) = slot.pop() {
                    break request;
                }
                if slot.closed {
                    return;
                }
                slot = ready.wait(slot).unwrap_or_else(|error| error.into_inner());
            }
        };
        let started = std::time::Instant::now();
        let bytes = crate::image::load_or_fetch(
            &request.source,
            &request.url,
            request.fallback_url.as_deref(),
            &request.key,
        );
        LAST_JOB_MS.store(
            started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            Ordering::Relaxed,
        );
        let placeholder = bytes
            .as_ref()
            .ok()
            .and_then(|bytes| crate::image::placeholder(bytes).ok());
        if let Ok(mut queue) = queue.lock() {
            queue.push(Msg::Image {
                key: request.key,
                bytes,
                placeholder,
            });
        }
        if let Some(wake) = wake.lock().ok().and_then(|slot| slot.clone()) {
            wake();
        }
    }
}

pub struct Worker {
    queue: Arc<Mutex<Vec<Msg>>>,
    wake: Arc<Mutex<Option<Waker>>>,
    images: Arc<(Mutex<ImageQueue>, Condvar)>,
}

impl Default for Worker {
    fn default() -> Self {
        Self::new()
    }
}

impl Worker {
    pub fn new() -> Self {
        let queue = Arc::new(Mutex::new(Vec::new()));
        let wake = Arc::new(Mutex::new(None::<Waker>));
        let images = Arc::new((Mutex::new(ImageQueue::default()), Condvar::new()));
        // Several fetchers, not one. Posters are independent requests; a serial queue made a
        // rail fill one card at a time even when the server and link could answer together.
        const IMAGE_WORKERS: usize = 4;
        for index in 0..IMAGE_WORKERS {
            let queue = Arc::clone(&queue);
            let wake = Arc::clone(&wake);
            let images = Arc::clone(&images);
            std::thread::Builder::new()
                .name(format!("runefin-images-{index}"))
                .spawn(move || image_worker(queue, wake, images))
                .expect("image worker");
        }
        Worker {
            queue,
            wake,
            images,
        }
    }

    /// Called once from `AppSpec::setup`, with the waker Rune makes for the window.
    pub fn set_waker(&self, waker: Waker) {
        if let Ok(mut slot) = self.wake.lock() {
            *slot = Some(waker);
        }
    }

    fn spawn(&self, work: impl FnOnce() -> Msg + Send + 'static) {
        let queue = Arc::clone(&self.queue);
        let wake = self.wake.lock().ok().and_then(|slot| slot.clone());
        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let msg = work();
            LAST_JOB_MS.store(
                started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                Ordering::Relaxed,
            );
            if let Ok(mut queue) = queue.lock() {
                queue.push(msg);
            }
            if let Some(wake) = wake {
                wake();
            }
        });
    }

    /// Queue one Jellyfin poster. The worker reads the disk cache first and only then the network.
    pub fn request_image(&self, client: Client, key: String, url: String, priority: ImagePriority) {
        self.push_image(ImageSource::Jellyfin(client), key, url, None, priority);
    }

    /// A poster with a second URL for the empty-body case (a broken Primary, a valid Backdrop).
    pub fn request_image_with_fallback(
        &self,
        client: Client,
        key: String,
        url: String,
        fallback: String,
        priority: ImagePriority,
    ) {
        self.push_image(
            ImageSource::Jellyfin(client),
            key,
            url,
            Some(fallback),
            priority,
        );
    }

    /// Queue one public (TMDB) poster. No Jellyfin client and no token.
    pub fn request_public_image(&self, key: String, url: String, priority: ImagePriority) {
        self.push_image(ImageSource::Public, key, url, None, priority);
    }

    fn push_image(
        &self,
        source: ImageSource,
        key: String,
        url: String,
        fallback_url: Option<String>,
        priority: ImagePriority,
    ) {
        let (slot, ready) = &*self.images;
        let mut slot = slot.lock().unwrap_or_else(|e| e.into_inner());
        let request = ImageRequest {
            source,
            key,
            url,
            fallback_url,
        };
        match priority {
            ImagePriority::High => slot.high.push_back(request),
            ImagePriority::Low => slot.low.push_back(request),
        }
        ready.notify_one();
    }

    /// Read mpv's IPC until the player exits. Runs on its own thread, so a dead socket or a
    /// blocking read never touches the UI thread.
    pub fn watch_playback(&self, stream: std::os::unix::net::UnixStream, generation: u64) {
        let queue = Arc::clone(&self.queue);
        let wake = self.wake.lock().ok().and_then(|slot| slot.clone());
        std::thread::Builder::new()
            .name("runefin-mpv".into())
            .spawn(move || {
                crate::handoff::watch(stream, |event| {
                    if let Ok(mut queue) = queue.lock() {
                        queue.push(Msg::Playback { generation, event });
                    }
                    if let Some(wake) = &wake {
                        wake();
                    }
                });
            })
            .expect("mpv reader thread");
    }

    pub fn report_playing(&self, client: Client, report: PlaybackReport) {
        self.spawn(move || Msg::Reported {
            kind: ReportKind::Started,
            result: client
                .report_playing(&report)
                .map_err(|error| error.user_message()),
        });
    }

    pub fn report_progress(&self, client: Client, report: PlaybackReport) {
        self.spawn(move || Msg::Reported {
            kind: ReportKind::Progress,
            result: client
                .report_progress(&report)
                .map_err(|error| error.user_message()),
        });
    }

    pub fn clear_image_cache(&self) {
        self.spawn(|| Msg::ImagesCleared(crate::image::clear_cache()));
    }

    /// One Seerr request write. The 201 body and the 409/403 messages all come back through the
    /// same `Msg`, so the screen never blocks on the server.
    pub fn create_seerr_request(
        &self,
        client: seerr_api::Client,
        request: seerr_api::CreateRequest,
    ) {
        self.spawn(move || {
            Msg::SeerrRequested(
                client
                    .create_request(&request)
                    .map(Box::new)
                    .map_err(|error| error.user_message()),
            )
        });
    }

    pub fn remote_command(&self, client: Client, session_id: String, command: &'static str) {
        self.spawn(move || {
            Msg::Remote(
                client
                    .session_command(&session_id, command)
                    .map_err(|error| error.user_message()),
            )
        });
    }

    pub fn remote_seek(&self, client: Client, session_id: String, position_ticks: i64) {
        self.spawn(move || {
            Msg::Remote(
                client
                    .session_seek(&session_id, position_ticks)
                    .map_err(|error| error.user_message()),
            )
        });
    }

    pub fn set_favorite(&self, client: Client, item_id: String, favorite: bool) {
        self.spawn(move || {
            Msg::Favorite(
                client
                    .set_favorite(&item_id, favorite)
                    .map_err(|error| error.user_message()),
            )
        });
    }

    pub fn set_rating(&self, client: Client, item_id: String, like: Option<bool>) {
        self.spawn(move || {
            Msg::Rated(
                client
                    .set_rating(&item_id, like)
                    .map_err(|error| error.user_message()),
            )
        });
    }

    pub fn set_played(&self, client: Client, item_id: String, played: bool) {
        self.spawn(move || {
            Msg::Played(
                client
                    .set_played(&item_id, played)
                    .map_err(|error| error.user_message()),
            )
        });
    }

    pub fn set_resume_position(&self, client: Client, item_id: String, ticks: i64) {
        self.spawn(move || {
            Msg::ResumeCleared(
                client
                    .set_resume_position(&item_id, ticks)
                    .map_err(|error| error.user_message()),
            )
        });
    }

    /// Undo an add: read the playlist, find the last entry for the item, remove that entry.
    pub fn undo_playlist_add(&self, client: Client, playlist_id: String, item_id: String) {
        self.spawn(move || {
            let result = client
                .playlist_items(&playlist_id)
                .map_err(|error| error.user_message())
                .and_then(|items| {
                    items
                        .into_iter()
                        .rev()
                        .find(|item| item.id == item_id)
                        .and_then(|item| item.playlist_item_id)
                        .ok_or_else(|| "the entry is not in the playlist".to_string())
                })
                .and_then(|entry| {
                    client
                        .remove_from_playlist(&playlist_id, &[entry])
                        .map_err(|error| error.user_message())
                });
            Msg::PlaylistAdded(result)
        });
    }

    pub fn add_to_playlist(&self, client: Client, playlist_id: String, ids: Vec<String>) {
        self.spawn(move || {
            Msg::PlaylistAdded(
                client
                    .add_to_playlist(&playlist_id, &ids)
                    .map_err(|error| error.user_message()),
            )
        });
    }

    pub fn create_collection(&self, client: Client, name: String, ids: Vec<String>) {
        self.spawn(move || {
            Msg::Collection(
                client
                    .create_collection(&name, &ids)
                    .map_err(|error| error.user_message()),
            )
        });
    }

    pub fn remove_from_collection(&self, client: Client, collection_id: String, ids: Vec<String>) {
        self.spawn(move || {
            Msg::Collection(
                client
                    .remove_from_collection(&collection_id, &ids)
                    .map_err(|error| error.user_message()),
            )
        });
    }

    pub fn move_playlist_entry(
        &self,
        client: Client,
        playlist_id: String,
        entry_id: String,
        index: usize,
    ) {
        self.spawn(move || {
            Msg::PlaylistOrder(
                client
                    .move_playlist_entry(&playlist_id, &entry_id, index)
                    .map_err(|error| error.user_message()),
            )
        });
    }

    pub fn report_stopped(&self, client: Client, stop: PlaybackStop) {
        self.spawn(move || Msg::Reported {
            kind: ReportKind::Stopped,
            result: client
                .report_stopped(&stop)
                .map_err(|error| error.user_message()),
        });
    }

    /// Probe the server, then sign in. Two failures are possible and the UI distinguishes them.
    pub fn connect(
        &self,
        server_url: String,
        device_id: String,
        user_name: String,
        password: String,
    ) {
        self.spawn(move || {
            let client = Client::new(&server_url, device_id);
            let info = match client.public_system_info() {
                Ok(info) => info,
                Err(e) => {
                    return Msg::Connected(Err(Failure {
                        stage: Stage::Server,
                        message: e.user_message(),
                    }));
                }
            };
            match client.authenticate(&user_name, &password) {
                Ok(auth) => Msg::Connected(Ok(Session {
                    server_url: client.base_url().to_string(),
                    server_name: info.server_name,
                    version: info.version,
                    user_id: auth.user.id,
                    user_name: auth.user.name,
                    token: auth.access_token,
                })),
                Err(e) => Msg::Connected(Err(Failure {
                    stage: Stage::Credentials,
                    message: e.user_message(),
                })),
            }
        });
    }

    /// Takes every finished outcome. Cheap and lock-free on the common empty path.
    pub fn drain(&self) -> Vec<Msg> {
        match self.queue.lock() {
            Ok(mut queue) if !queue.is_empty() => std::mem::take(&mut *queue),
            _ => Vec::new(),
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let (slot, ready) = &*self.images;
        if let Ok(mut slot) = slot.lock() {
            slot.closed = true;
        }
        ready.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draining_an_idle_worker_yields_nothing_and_is_repeatable() {
        let w = Worker::new();
        assert!(w.drain().is_empty());
        assert!(w.drain().is_empty());
    }

    #[test]
    fn high_priority_images_pop_before_queued_low_priority_ones() {
        let mut queue = ImageQueue::default();
        let client = Client::new("http://localhost", "test");
        for (key, priority) in [("low", ImagePriority::Low), ("high", ImagePriority::High)] {
            let request = ImageRequest {
                source: ImageSource::Jellyfin(client.clone()),
                key: key.into(),
                url: String::new(),
                fallback_url: None,
            };
            match priority {
                ImagePriority::High => queue.high.push_back(request),
                ImagePriority::Low => queue.low.push_back(request),
            }
        }
        assert_eq!(queue.pop().expect("first").key, "high");
        assert_eq!(queue.pop().expect("second").key, "low");
    }

    #[test]
    fn a_failure_names_the_stage_so_the_screen_can_say_which_half_failed() {
        let f = Failure {
            stage: Stage::Credentials,
            message: "Your session expired. Sign in again.".into(),
        };
        assert_eq!(f.stage, Stage::Credentials);
        assert!(f.message.contains("Sign in"));
    }
}
