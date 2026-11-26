//! Downloads: what the page draws, and the calls that keep the manifest in step.
//!
//! The manifest is the queue (see [`crate::downloads`]). The state holds the entries for
//! rendering and the worker owns every byte on disk: a progress tick only touches memory, and
//! the writes that must happen (the manifest, the rename that finishes a file) run on a worker
//! so the UI thread never waits for a disk.

use rune::{Latest, Tracked};

use crate::downloads::{self, Entry, State as EntryState};
use crate::net::DownloadEvent;
use crate::state::State;

#[derive(Default)]
pub struct DownloadsState {
    /// The root in use; `None` when it could not be opened, and then downloads are unavailable.
    pub store: Option<downloads::Store>,
    /// The resolved root, for Settings and the page's footer.
    pub root: String,
    pub budget_gb: f64,
    /// Why the root is unusable, if it is.
    pub error: Tracked<Option<String>>,
    pub entries: Tracked<Vec<Entry>>,
    /// Free space on the root's volume, asked for when the page opens.
    pub free: Tracked<Option<u64>>,
    /// A sentence about the last failure, shown at the top of the page or as a toast.
    pub notice: Tracked<Option<String>>,
    /// The manifest has been read once. A fixture that brings its own entries sets this, so
    /// `setup` does not replace them with the (empty) manifest of a fresh install.
    pub booted: bool,
    pub(super) save: Latest<Result<(), String>>,
    /// A new folder from Settings: reopening it reads a different manifest.
    pub(super) reload: Latest<Result<(downloads::Store, Vec<Entry>), String>>,
    pub(super) commit: Latest<(String, Result<u64, String>)>,
    pub(super) free_load: Latest<Option<u64>>,
}

impl State {
    /// Read the manifest. Called from `setup`, where the app already reads its checkpoints and
    /// history; a queued download then starts itself, so an interrupted queue resumes.
    pub fn load_downloads(&mut self) {
        if self.downloads.booted {
            return;
        }
        self.downloads.booted = true;
        let root = downloads::root_from(self.config.downloads_path.as_deref());
        self.downloads.root = root.display().to_string();
        self.downloads.budget_gb = self.downloads_budget();
        match downloads::Store::open(root) {
            Ok((store, entries)) => {
                self.downloads.store = Some(store);
                *self.downloads.entries = entries;
                *self.downloads.error = None;
                let queued: Vec<String> = self
                    .downloads
                    .entries
                    .iter()
                    .filter(|entry| entry.state == EntryState::Queued)
                    .map(|entry| entry.item.id.clone())
                    .collect();
                for id in queued {
                    self.start_download(&id);
                }
            }
            Err(message) => *self.downloads.error = Some(message),
        }
    }

    /// The reader picked a folder. The path is written to the config now (it is saved with the
    /// rest on exit) and the store is reopened on a worker: a bad path leaves the working store
    /// and its downloads alone, and the page says what is wrong.
    pub fn apply_downloads_path(&mut self) {
        let text = self.downloads_path.text.trim().to_string();
        self.config.downloads_path = if text.is_empty() { None } else { Some(text) };
        let root = downloads::root_from(self.config.downloads_path.as_deref());
        self.downloads.root = root.display().to_string();
        self.downloads
            .reload
            .request((), move |()| downloads::Store::open(root));
    }

    pub(crate) fn apply_download_reload(
        &mut self,
        result: Result<(downloads::Store, Vec<Entry>), String>,
    ) {
        match result {
            Ok((store, entries)) => {
                self.downloads.store = Some(store);
                *self.downloads.entries = entries;
                *self.downloads.error = None;
                self.show_toast(format!("Downloads now live in {}.", self.downloads.root));
                self.refresh_download_free();
                let queued: Vec<String> = self
                    .downloads
                    .entries
                    .iter()
                    .filter(|entry| entry.state == EntryState::Queued)
                    .map(|entry| entry.item.id.clone())
                    .collect();
                for id in queued {
                    self.start_download(&id);
                }
            }
            Err(message) => *self.downloads.error = Some(message),
        }
    }

    /// Step the storage budget, write it to the config, and keep the page's copy in step.
    pub fn set_downloads_budget(&mut self, gb: f64) {
        let gb = gb.clamp(5.0, 500.0);
        self.config.downloads_budget_gb = Some(gb);
        self.downloads.budget_gb = gb;
    }

    /// Open the Downloads page: refresh the free-space figure and ask for the posters.
    pub fn open_downloads(&mut self) {
        self.stop_theme_media();
        self.route = crate::state::Route::Downloads;
        *self.downloads.notice = None;
        self.refresh_download_free();
        self.request_download_images();
    }

    /// Posters for the page's rows. Offline this asks for nothing: the rows stay readable and
    /// an item with no art still plays.
    pub(super) fn request_download_images(&mut self) {
        self.current_images.clear();
        if self.image_cache.is_none() {
            return;
        }
        let Some(client) = self.client() else {
            return;
        };
        let mut requests = Vec::new();
        for entry in &self.downloads.entries {
            let Some((id, tag, kind)) = crate::home::poster_source(&entry.item) else {
                continue;
            };
            if let Some(request) = self.image_request(&client, id, tag, kind, self.image_width(120))
            {
                requests.push((request.0, request.1, None, crate::net::ImagePriority::High));
            }
        }
        self.send_images(requests);
    }

    /// The size a new download asks for: the item's own file, or a copy at a bitrate cap.
    /// `Auto` is a playback choice, not a size, so it means the original here.
    pub fn download_quality(&self) -> crate::config::PlaybackQuality {
        use crate::config::PlaybackQuality;
        match self.config.download_quality {
            None | Some(PlaybackQuality::Auto) => PlaybackQuality::Original,
            Some(quality) => quality,
        }
    }

    /// Walk the download sizes and remember the choice. `Auto` is skipped: for a copy on disk
    /// there is no "whatever fits", only the file or a cap.
    pub fn cycle_download_quality(&mut self) {
        use crate::config::PlaybackQuality;
        let next = match self.download_quality() {
            PlaybackQuality::Auto | PlaybackQuality::Original => PlaybackQuality::Mbps20,
            PlaybackQuality::Mbps20 => PlaybackQuality::Mbps12,
            PlaybackQuality::Mbps12 => PlaybackQuality::Mbps8,
            PlaybackQuality::Mbps8 => PlaybackQuality::Mbps4,
            PlaybackQuality::Mbps4 => PlaybackQuality::Mbps2,
            PlaybackQuality::Mbps2 => PlaybackQuality::Original,
        };
        self.config.download_quality = Some(next);
        self.show_toast(format!("Downloads at {}.", next.label()));
    }

    /// The budget in gigabytes: the setting, or the shipped default.
    pub fn downloads_budget(&self) -> f64 {
        self.config
            .downloads_budget_gb
            .unwrap_or(downloads::DEFAULT_BUDGET_GB)
            .clamp(0.5, 10_000.0)
    }

    pub fn downloads_used(&self) -> u64 {
        downloads::used(&self.downloads.entries)
    }

    pub fn download_entry(&self, id: &str) -> Option<&Entry> {
        self.downloads
            .entries
            .iter()
            .find(|entry| entry.item.id == id)
    }

    pub fn download_entry_mut(&mut self, id: &str) -> Option<&mut Entry> {
        self.downloads
            .entries
            .iter_mut()
            .find(|entry| entry.item.id == id)
    }

    /// Queue one item and keep it: an explicit download is never evicted to make room, and it
    /// is the queue's first priority.
    pub fn download_item(&mut self, item: &jellyfin_api::Item) {
        if self.downloads.store.is_none() {
            self.show_toast(
                self.downloads
                    .error
                    .as_deref()
                    .unwrap_or("Downloads are unavailable.")
                    .to_string(),
            );
            return;
        }
        if self.download_entry(&item.id).is_some() {
            if let Some(entry) = self.download_entry_mut(&item.id) {
                entry.kept = true;
            }
            let restart = self.download_entry(&item.id).is_some_and(|entry| {
                matches!(entry.state, EntryState::Paused | EntryState::Failed)
            });
            self.save_downloads();
            if restart {
                self.start_download(&item.id);
            }
            return;
        }
        // A copy at a cap has no length until the server makes it: estimate from the runtime so
        // the budget check and the progress bar have something to work with.
        let cap = self.download_quality().max_bitrate();
        let size = match cap {
            Some(cap) => downloads::expected_size(item.run_time_ticks.unwrap_or(0), cap),
            None => item
                .media_sources
                .first()
                .and_then(|source| source.size)
                .unwrap_or(0)
                .max(0) as u64,
        };
        let budget = downloads::budget_bytes(self.downloads_budget());
        match downloads::eviction_plan(&self.downloads.entries, budget, size) {
            Ok(plan) => {
                for id in plan {
                    self.drop_download(&id);
                }
            }
            Err(message) => {
                self.show_toast(message);
                return;
            }
        }
        self.downloads.entries.push(Entry {
            item: item.clone(),
            source_id: None,
            file: downloads::file_name_for(item, cap.is_some()),
            size,
            done: 0,
            state: EntryState::Queued,
            kept: true,
            cap,
            created: crate::home_cache::now_secs(),
            error: None,
        });
        self.save_downloads();
        self.start_download(&item.id);
    }

    /// The item is already on this device: the detail page shows its state instead of a button
    /// that would queue it twice.
    pub fn download_state(&self, id: &str) -> Option<EntryState> {
        self.download_entry(id).map(|entry| entry.state)
    }

    pub fn pause_download(&mut self, id: &str) {
        self.worker.download_pause(id);
        if let Some(entry) = self.download_entry_mut(id) {
            entry.state = EntryState::Paused;
        }
        self.save_downloads();
    }

    pub fn resume_download(&mut self, id: &str) {
        self.start_download(id);
    }

    /// Pin or unpin a copy. An unkept copy is the first thing eviction takes.
    pub fn keep_download(&mut self, id: &str) {
        if let Some(entry) = self.download_entry_mut(id) {
            entry.kept = !entry.kept;
        }
        self.save_downloads();
    }

    /// Delete an entry and its bytes. A running download stops at its next chunk.
    pub fn remove_download(&mut self, id: &str) {
        self.drop_download(id);
    }

    pub fn play_download(&mut self, id: &str) {
        let Some(store) = self.downloads.store.clone() else {
            return;
        };
        let Some(entry) = self.download_entry(id).cloned() else {
            return;
        };
        if !entry.state.is_watchable() {
            self.show_toast("That download is not finished yet.".to_string());
            return;
        }
        let path = store.file_path(&entry);
        if !path.exists() {
            self.fail_download(id, "The file is no longer on this device.");
            return;
        }
        self.play_in_app_download(entry.item, path);
    }

    /// Ask how much room the volume has. The page calls this on open; `df` runs on a worker.
    pub fn refresh_download_free(&mut self) {
        let Some(store) = self.downloads.store.clone() else {
            return;
        };
        self.downloads
            .free_load
            .request((), move |()| downloads::free_space(store.root()));
    }

    fn start_download(&mut self, id: &str) {
        let Some(store) = self.downloads.store.clone() else {
            return;
        };
        let Some(entry) = self.download_entry(id).cloned() else {
            return;
        };
        let Some(client) = self.client() else {
            // No session: the entry waits as `Queued` and starts on the next sign-in.
            return;
        };
        // The entry remembers the size it was queued at, so changing the setting later never
        // leaves a row promising one thing and holding another. A cap is negotiated with the
        // server by the worker (the request would block here); the item's own file is just a
        // URL, and resumes where it stopped.
        let plan = match entry.cap {
            Some(cap) => crate::net::DownloadPlan::Transcoded {
                item_id: id.to_string(),
                source_id: entry.source_id.clone(),
                cap,
            },
            None => {
                let Some(url) =
                    client.direct_stream_url_with_source(id, entry.source_id.as_deref())
                else {
                    self.fail_download(id, "The server has no stream for this item.");
                    return;
                };
                crate::net::DownloadPlan::Direct(url)
            }
        };
        let transcoded = matches!(plan, crate::net::DownloadPlan::Transcoded { .. });
        let part = store.part_path(&entry);
        self.worker.download(id.to_string(), client, plan, part);
        if let Some(entry) = self.download_entry_mut(id) {
            entry.state = EntryState::Downloading;
            entry.error = None;
            if transcoded {
                // A copy the server makes cannot resume: the session it belonged to is gone, so
                // the server starts the encode over and so does the file.
                entry.done = 0;
            }
        }
        self.save_downloads();
    }

    fn fail_download(&mut self, id: &str, message: &str) {
        if let Some(entry) = self.download_entry_mut(id) {
            entry.state = EntryState::Failed;
            entry.error = Some(message.to_string());
        }
        *self.downloads.notice = Some(message.to_string());
        self.save_downloads();
    }

    /// Remove an entry: stop the copy, delete its bytes on a worker, forget it, save.
    fn drop_download(&mut self, id: &str) {
        let Some(store) = self.downloads.store.clone() else {
            return;
        };
        self.worker.download_cancel(id);
        let Some(index) = self
            .downloads
            .entries
            .iter()
            .position(|entry| entry.item.id == id)
        else {
            return;
        };
        let entry = self.downloads.entries.remove(index);
        let _ = std::thread::Builder::new()
            .name("runefin-download-remove".into())
            .spawn(move || store.remove(&entry));
        self.save_downloads();
    }

    /// Write the manifest from a copy of the entries, so the disk work stays off this thread.
    fn save_downloads(&mut self) {
        let Some(store) = self.downloads.store.clone() else {
            return;
        };
        let entries = self.downloads.entries.clone();
        self.downloads
            .save
            .request((), move |()| store.save(&entries));
    }

    /// A download's progress or ending, from the worker.
    pub(crate) fn apply_download(&mut self, id: String, event: DownloadEvent) {
        match event {
            DownloadEvent::Progress { done, total } => {
                if let Some(entry) = self.download_entry_mut(&id) {
                    entry.done = done;
                    if let Some(total) = total.filter(|total| *total > 0) {
                        entry.size = total;
                    }
                    entry.state = EntryState::Downloading;
                }
            }
            DownloadEvent::Finished { size } => self.finish_download(&id, size),
            DownloadEvent::Paused { done } => {
                if let Some(entry) = self.download_entry_mut(&id) {
                    entry.done = done;
                    entry.state = EntryState::Paused;
                }
                self.save_downloads();
            }
            DownloadEvent::Failed { message } => {
                let name = self
                    .download_entry(&id)
                    .map(|entry| entry.item.name.clone())
                    .unwrap_or_default();
                self.fail_download(&id, &message);
                self.show_toast(format!("Download of {name} failed: {message}"));
            }
        }
    }

    /// The last byte arrived: rename the `.part` into place on a worker, then record it done.
    /// Until the rename lands the entry stays `Downloading` at 100%, because a `.part` is not a
    /// file that can be played.
    fn finish_download(&mut self, id: &str, size: u64) {
        let Some(store) = self.downloads.store.clone() else {
            return;
        };
        let Some(entry) = self.download_entry(id).cloned() else {
            return;
        };
        if let Some(entry) = self.download_entry_mut(id) {
            entry.done = size;
            entry.size = entry.size.max(size);
        }
        let mut entries = self.downloads.entries.clone();
        let disk = store;
        let item_id = id.to_string();
        self.downloads.commit.request((), move |()| {
            let result = disk.finish(&entry).and_then(|size| {
                if let Some(saved) = entries
                    .iter_mut()
                    .find(|saved| saved.item.id == entry.item.id)
                {
                    saved.state = EntryState::Done;
                    saved.size = size;
                    saved.done = size;
                    saved.error = None;
                }
                disk.save(&entries).map(|()| size)
            });
            (item_id, result)
        });
    }

    /// The rename finished (or did not). Either way the entry now knows which it is.
    pub(crate) fn apply_download_commit(&mut self, result: (String, Result<u64, String>)) {
        let (id, result) = result;
        match result {
            Ok(size) => {
                let name = self
                    .download_entry(&id)
                    .map(|entry| entry.item.name.clone())
                    .unwrap_or_default();
                if let Some(entry) = self.download_entry_mut(&id) {
                    entry.state = EntryState::Done;
                    entry.size = size;
                    entry.done = size;
                    entry.error = None;
                }
                self.show_toast(format!("{name} is on this device."));
            }
            Err(message) => self.fail_download(&id, &message),
        }
    }

    pub(crate) fn apply_download_saved(&mut self, result: Result<(), String>) {
        if let Err(message) = result {
            *self.downloads.notice = Some(format!("Could not save the download list: {message}"));
        }
    }
}
