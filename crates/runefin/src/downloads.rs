//! Downloads: one file per item under a chosen root, and a manifest that survives a restart.
//!
//! A download is a copy of the item's own file (the same stream a player opens, `static=true`),
//! so it needs no transcoding and plays back through the same pipeline. Everything here is
//! plain data plus file operations: the UI thread never calls into it for disk work. Opening the
//! store, finishing a file and saving the manifest all run on a worker, and the state keeps only
//! what a screen draws.
//!
//! The manifest *is* the queue. A crash can therefore cost at most the bytes between the last
//! save and the crash: entries come back `Paused` at the size the `.part` actually has, and a
//! resume continues from there with a `Range` request.

use std::path::{Path, PathBuf};

use jellyfin_api::Item;
use serde::{Deserialize, Serialize};

/// The shipped storage budget: about a season of 1080p, or a morning of UHD.
pub const DEFAULT_BUDGET_GB: f64 = 20.0;
const MANIFEST: &str = "downloads.json";
const VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Queued,
    Downloading,
    Paused,
    Done,
    Failed,
}

impl State {
    /// What the Downloads page and the detail button say.
    pub fn label(self) -> &'static str {
        match self {
            State::Queued => "Queued",
            State::Downloading => "Downloading",
            State::Paused => "Paused",
            State::Done => "On this device",
            State::Failed => "Failed",
        }
    }

    pub fn is_done(self) -> bool {
        self == State::Done
    }

    /// Whether the entry holds bytes we can play. Only a finished file is played: a `.part` is
    /// not a valid container yet.
    pub fn is_watchable(self) -> bool {
        self == State::Done
    }
}

/// One downloaded item, with everything the Downloads page needs to draw it offline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// The item as it was when the download was queued: the poster's tags, the runtime, the
    /// series name. Kept whole so the page renders with no server.
    pub item: Item,
    /// The media source the file came from, when the reader picked a version.
    pub source_id: Option<String>,
    /// The file's name under the root. Sanitised, never the raw item id.
    pub file: String,
    /// The file's size when the server said; zero while unknown.
    pub size: u64,
    /// Bytes on disk: the finished file, or the partial one.
    pub done: u64,
    pub state: State,
    /// The reader asked to keep this copy, so eviction never takes it for space.
    pub kept: bool,
    /// The bitrate cap the copy was made at, if the server transcoded it. An original file has
    /// none, and is the only kind that can be resumed.
    #[serde(default)]
    pub cap: Option<i64>,
    /// Unix seconds, used to evict the oldest first.
    pub created: i64,
    pub error: Option<String>,
}

impl Entry {
    /// 0–1, or `None` while the size is unknown.
    pub fn progress(&self) -> Option<f32> {
        (self.size > 0).then(|| (self.done as f32 / self.size as f32).clamp(0.0, 1.0))
    }

    /// "S2E04 · Overtime" for an episode, "1927 · 2 h 33 min" for a film.
    pub fn line(&self) -> String {
        let item = &self.item;
        if let (Some(series), Some(season), Some(episode)) = (
            item.series_name.as_deref(),
            item.parent_index_number,
            item.index_number,
        ) {
            return format!("{series} · S{season:02}E{episode:02}");
        }
        let length = runtime(item.run_time_ticks.unwrap_or(0));
        match (item.production_year, length.is_empty()) {
            (Some(year), false) => format!("{year} · {length}"),
            (Some(year), true) => format!("{year}"),
            (None, _) => length,
        }
    }
}

fn runtime(ticks: i64) -> String {
    let minutes = (ticks / 600_000_000).max(0);
    if minutes == 0 {
        String::new()
    } else if minutes >= 60 {
        format!("{} h {} min", minutes / 60, minutes % 60)
    } else {
        format!("{minutes} min")
    }
}

/// A handle on the download root. Cheap to clone; the entries live in the app state.
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Create the root (and its parents) and read the manifest. An entry that was downloading
    /// when the app quit comes back paused at the bytes on disk, and a finished entry whose file
    /// vanished comes back failed, rather than pretending there is something to play.
    pub fn open(root: PathBuf) -> Result<(Store, Vec<Entry>), String> {
        std::fs::create_dir_all(&root).map_err(|error| format!("{}: {error}", root.display()))?;
        let store = Store { root };
        let mut entries = store.load();
        for entry in &mut entries {
            let part = store.part_path(entry);
            let file = store.file_path(entry);
            match entry.state {
                State::Downloading | State::Queued => {
                    entry.state = State::Paused;
                    entry.done = part.metadata().map(|meta| meta.len()).unwrap_or(entry.done);
                    if entry.done == 0 {
                        entry.state = State::Queued;
                    }
                }
                State::Done => {
                    let Ok(meta) = file.metadata() else {
                        entry.state = State::Failed;
                        entry.error = Some("The file is no longer on this device.".into());
                        continue;
                    };
                    entry.size = meta.len();
                    entry.done = entry.size;
                }
                _ => {
                    entry.done = part.metadata().map(|meta| meta.len()).unwrap_or(0);
                }
            }
        }
        Ok((store, entries))
    }

    fn manifest_path(&self) -> PathBuf {
        self.root.join(MANIFEST)
    }

    fn load(&self) -> Vec<Entry> {
        let Ok(text) = std::fs::read_to_string(self.manifest_path()) else {
            return Vec::new();
        };
        let Ok(file) = serde_json::from_str::<Manifest>(&text) else {
            return Vec::new();
        };
        if file.version != VERSION {
            return Vec::new();
        }
        file.entries
    }

    /// Rewrite the manifest atomically: a crash mid-write leaves the old one, never half of one.
    pub fn save(&self, entries: &[Entry]) -> Result<(), String> {
        let manifest = Manifest {
            version: VERSION,
            entries: entries.to_vec(),
        };
        let text =
            serde_json::to_vec(&manifest).map_err(|error| format!("the download list: {error}"))?;
        let path = self.manifest_path();
        let temporary = path.with_extension("json.part");
        std::fs::write(&temporary, text)
            .map_err(|error| format!("{}: {error}", temporary.display()))?;
        std::fs::rename(&temporary, &path).map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn file_path(&self, entry: &Entry) -> PathBuf {
        self.root.join(&entry.file)
    }

    pub fn part_path(&self, entry: &Entry) -> PathBuf {
        self.root.join(format!("{}.part", entry.file))
    }

    /// Finish a download: the partial file becomes the real one, at its final size.
    pub fn finish(&self, entry: &Entry) -> Result<u64, String> {
        let part = self.part_path(entry);
        let file = self.file_path(entry);
        std::fs::rename(&part, &file).map_err(|error| format!("{}: {error}", part.display()))?;
        file.metadata()
            .map(|meta| meta.len())
            .map_err(|error| format!("{}: {error}", file.display()))
    }

    /// Delete an entry's bytes. A file already gone is not an error: the entry goes either way.
    pub fn remove(&self, entry: &Entry) {
        let _ = std::fs::remove_file(self.part_path(entry));
        let _ = std::fs::remove_file(self.file_path(entry));
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    version: u32,
    entries: Vec<Entry>,
}

/// `$XDG_CACHE_HOME/runefin/downloads`, else `~/.cache/runefin/downloads`.
pub fn default_root() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("runefin").join("downloads")
}

/// A configured path, with `~` expanded. Empty means the default.
pub fn root_from(configured: Option<&str>) -> PathBuf {
    let Some(text) = configured.map(str::trim).filter(|text| !text.is_empty()) else {
        return default_root();
    };
    if let Some(rest) = text.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    PathBuf::from(text)
}

/// What a copy at `cap` bits per second is expected to weigh: the runtime times the bitrate.
/// The server sends no length for a transcode, so this is what the budget check and the progress
/// bar use until the real size is known.
pub fn expected_size(runtime_ticks: i64, cap: i64) -> u64 {
    if runtime_ticks <= 0 || cap <= 0 {
        return 0;
    }
    let seconds = runtime_ticks as u64 / 10_000_000;
    seconds.saturating_mul(cap as u64) / 8
}

/// The budget in bytes, clamped to something a disk can hold.
pub fn budget_bytes(gb: f64) -> u64 {
    let gb = if gb.is_finite() {
        gb.clamp(0.5, 10_000.0)
    } else {
        DEFAULT_BUDGET_GB
    };
    (gb * 1_000_000_000.0) as u64
}

/// Bytes the downloads hold: finished files, partial files and queued work's expected size.
pub fn used(entries: &[Entry]) -> u64 {
    entries
        .iter()
        .map(|entry| match entry.state {
            State::Done => entry.size.max(entry.done),
            State::Queued => entry.size,
            _ => entry.done,
        })
        .sum()
}

/// The entries to drop so `need` more bytes fit under `budget`: finished unkept copies, oldest
/// first. Empty when it already fits. `Err` when even dropping every unkept copy is not enough,
/// so the caller can say what the shortfall is instead of starting a download that cannot end.
pub fn eviction_plan(entries: &[Entry], budget: u64, need: u64) -> Result<Vec<String>, String> {
    let held = used(entries);
    if held + need <= budget {
        return Ok(Vec::new());
    }
    let mut candidates: Vec<&Entry> = entries
        .iter()
        .filter(|entry| !entry.kept && entry.state.is_done())
        .collect();
    candidates.sort_by_key(|entry| entry.created);
    let mut free = held;
    let mut plan = Vec::new();
    for entry in candidates {
        if free + need <= budget {
            break;
        }
        free = free.saturating_sub(entry.size.max(entry.done));
        plan.push(entry.item.id.clone());
    }
    if free + need <= budget {
        Ok(plan)
    } else {
        Err(shortfall(free + need - budget))
    }
}

/// A sentence for a budget that cannot hold the item, in the same units as the page.
pub fn shortfall(bytes: u64) -> String {
    format!(
        "Not enough room in the download budget: {} short.",
        size(bytes)
    )
}

/// The file name for an item: a sanitised id, a short hash so two odd ids never collide, and the
/// container when the server named one (GStreamer sniffs the file either way).
pub fn file_name(item: &Item) -> String {
    let container = item
        .media_sources
        .first()
        .and_then(|source| source.container.as_deref())
        .map(|container| {
            container
                .chars()
                .filter(char::is_ascii_alphanumeric)
                .collect::<String>()
                .to_ascii_lowercase()
        })
        .filter(|container| !container.is_empty() && container.len() <= 6)
        .unwrap_or_else(|| "media".into());
    format!("{}.{}", sanitised(&item.id), container)
}

/// The file name for a copy the server makes at a cap: the bytes are an MP4, whatever the
/// original container was.
pub fn file_name_for(item: &Item, transcoded: bool) -> String {
    if transcoded {
        format!("{}.mp4", sanitised(&item.id))
    } else {
        file_name(item)
    }
}

/// A path component that is safe to join: letters, digits, `-`, `_`, at most 48 of them, plus a
/// hash of the original so truncation and substitution cannot make two items share a file.
fn sanitised(id: &str) -> String {
    let mut stem: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(48)
        .collect();
    if stem.is_empty() {
        stem.push('_');
    }
    let mut hash: u32 = 0x811c_9dc5;
    for byte in id.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    format!("{stem}-{hash:08x}")
}

/// `4 Mbps`, `1.3 Mbps` — the bitrate a copy was made at, in the menu's words.
pub fn quality_label(cap: i64) -> String {
    let mbps = cap as f64 / 1_000_000.0;
    if (mbps - mbps.round()).abs() < 0.05 {
        format!("{mbps:.0} Mbps")
    } else {
        format!("{mbps:.1} Mbps")
    }
}

/// "812 MB", "4.2 GB", "0 B" — one decimal from a gigabyte up, rounded below it.
pub fn size(bytes: u64) -> String {
    const KB: f64 = 1_000.0;
    const MB: f64 = 1_000_000.0;
    const GB: f64 = 1_000_000_000.0;
    let bytes = bytes as f64;
    if bytes >= GB {
        format!("{:.1} GB", bytes / GB)
    } else if bytes >= MB {
        format!("{:.0} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.0} kB", bytes / KB)
    } else {
        format!("{bytes:.0} B")
    }
}

/// Free space on the volume that holds the root, in bytes. `None` when `df` cannot answer: the
/// budget still applies, and a download that runs the disk out fails with its own message.
pub fn free_space(root: &Path) -> Option<u64> {
    let output = std::process::Command::new("df")
        .arg("-Pk")
        .arg(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().nth(1)?;
    let available = line.split_whitespace().nth(3)?;
    available.parse::<u64>().ok().map(|blocks| blocks * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, name: &str) -> Item {
        Item {
            id: id.into(),
            name: name.into(),
            ..Item::default()
        }
    }

    fn entry(id: &str, state: State, size: u64, kept: bool, created: i64) -> Entry {
        Entry {
            item: item(id, id),
            source_id: None,
            file: file_name(&item(id, id)),
            size,
            done: size,
            state,
            kept,
            cap: None,
            created,
            error: None,
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "runefin-downloads-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        dir
    }

    #[test]
    fn the_manifest_survives_a_restart() {
        let root = scratch("round-trip");
        let (store, entries) = Store::open(root.clone()).expect("the store opens");
        assert!(entries.is_empty());
        let mut entries = vec![entry("metropolis", State::Done, 64, true, 10)];
        // The file the entry promises must exist for it to load as done.
        std::fs::write(store.file_path(&entries[0]), vec![0u8; 64]).expect("a file");
        store.save(&entries).expect("the manifest");
        let (again, loaded) = Store::open(root.clone()).expect("reopen");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].item.id, "metropolis");
        assert_eq!(loaded[0].state, State::Done);
        assert_eq!(loaded[0].size, 64);
        assert_eq!(again.root(), root.as_path());
        entries.clear();
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn an_interrupted_download_comes_back_paused_at_the_bytes_on_disk() {
        let root = scratch("interrupted");
        let (store, _) = Store::open(root.clone()).expect("the store opens");
        let mut entry = entry("quiet-floor", State::Downloading, 900, true, 1);
        entry.done = 0;
        std::fs::write(store.part_path(&entry), vec![0u8; 300]).expect("a partial file");
        store
            .save(std::slice::from_ref(&entry))
            .expect("the manifest");
        let (_, loaded) = Store::open(root.clone()).expect("reopen");
        assert_eq!(loaded[0].state, State::Paused);
        assert_eq!(loaded[0].done, 300);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_finished_entry_whose_file_vanished_is_failed_not_played() {
        let root = scratch("vanished");
        let (store, _) = Store::open(root.clone()).expect("the store opens");
        let entry = entry("gone", State::Done, 12, true, 1);
        store
            .save(std::slice::from_ref(&entry))
            .expect("the manifest");
        let (_, loaded) = Store::open(root.clone()).expect("reopen");
        assert_eq!(loaded[0].state, State::Failed);
        assert!(loaded[0].error.is_some());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn finishing_renames_the_part_into_place() {
        let root = scratch("finish");
        let (store, _) = Store::open(root.clone()).expect("the store opens");
        let entry = entry("movie", State::Downloading, 0, true, 1);
        std::fs::write(store.part_path(&entry), vec![7u8; 42]).expect("a partial file");
        let size = store.finish(&entry).expect("the rename");
        assert_eq!(size, 42);
        assert!(store.file_path(&entry).exists());
        assert!(!store.part_path(&entry).exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_hostile_item_id_never_escapes_the_root() {
        let root = scratch("paths");
        let (store, _) = Store::open(root.clone()).expect("the store opens");
        for id in ["../../etc/passwd", "a/b", "", "with space", "ünïcode"] {
            let item = item(id, "x");
            let entry = Entry {
                file: file_name(&item),
                ..entry("placeholder", State::Queued, 0, false, 0)
            };
            let path = store.file_path(&entry);
            assert_eq!(
                path.parent(),
                Some(root.as_path()),
                "{id} stays in the root"
            );
            assert!(!entry.file.contains('/'), "{id} is one component");
            assert!(!entry.file.contains(".."), "{id} cannot walk up");
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn eviction_takes_the_oldest_unkept_copy_and_never_a_kept_one() {
        let entries = vec![
            entry("old", State::Done, 100, false, 1),
            entry("older", State::Done, 100, false, 0),
            entry("kept", State::Done, 100, true, 0),
        ];
        // 300 held, 50 needed, budget 200: drop two unkept copies to fit.
        assert_eq!(
            eviction_plan(&entries, 200, 50).expect("a plan"),
            ["older", "old"]
        );
        // Already fits: nothing is dropped.
        assert!(
            eviction_plan(&entries, 1_000, 50)
                .expect("a plan")
                .is_empty()
        );
        // A kept copy is not space to reclaim.
        let kept_only = vec![entry("kept", State::Done, 100, true, 0)];
        assert!(eviction_plan(&kept_only, 150, 100).is_err());
    }

    #[test]
    fn used_counts_queued_work_at_its_expected_size() {
        let entries = vec![
            entry("done", State::Done, 100, true, 1),
            entry("queued", State::Queued, 40, true, 2),
            Entry {
                done: 10,
                ..entry("paused", State::Paused, 90, true, 3)
            },
        ];
        assert_eq!(used(&entries), 150);
    }

    #[test]
    fn a_transcoded_copy_is_an_mp4_whatever_the_original_was() {
        let mut movie = item("metropolis", "Metropolis");
        movie.media_sources = vec![jellyfin_api::MediaSource {
            id: "s".into(),
            container: Some("mkv".into()),
            ..jellyfin_api::MediaSource::default()
        }];
        assert!(
            file_name(&movie).ends_with(".mkv"),
            "the original keeps its container"
        );
        assert!(file_name_for(&movie, true).ends_with(".mp4"));
        let _ = movie;
    }

    #[test]
    fn a_transcoded_copy_is_estimated_from_its_runtime_and_cap() {
        // 48 minutes at 4 Mbps: 2880 s * 4_000_000 bits / 8 = 1.44 GB.
        assert_eq!(
            expected_size(48 * 60 * 10_000_000, 4_000_000),
            1_440_000_000
        );
        // Nothing to estimate without a runtime or a cap.
        assert_eq!(expected_size(0, 4_000_000), 0);
        assert_eq!(expected_size(48 * 60 * 10_000_000, 0), 0);
    }

    #[test]
    fn sizes_read_at_a_glance() {
        assert_eq!(size(0), "0 B");
        assert_eq!(size(999), "999 B");
        assert_eq!(size(1_500), "2 kB");
        assert_eq!(size(812_000_000), "812 MB");
        assert_eq!(size(4_200_000_000), "4.2 GB");
    }

    #[test]
    fn a_configured_path_is_trimmed_and_an_empty_one_means_the_default() {
        assert_eq!(root_from(Some(" /srv/media ")), PathBuf::from("/srv/media"));
        assert_eq!(root_from(None), default_root());
        assert_eq!(root_from(Some("  ")), default_root());
    }
}
