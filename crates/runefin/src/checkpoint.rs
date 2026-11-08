//! Local playback checkpoints: a tiny JSON map, not a catalogue. It exists so a crash or a
//! restart does not lose the last few minutes; the server remains the source of truth.
//!
//! Reads happen once at startup (before the first frame). Writes happen on a worker thread, so
//! the UI never waits for the disk.

use std::collections::HashMap;
use std::path::PathBuf;

const MAX_ENTRIES: usize = 500;

pub fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(base.join("runefin").join("positions.json"))
}

pub fn load() -> HashMap<String, i64> {
    path().map_or_else(HashMap::new, load_from)
}

fn load_from(path: PathBuf) -> HashMap<String, i64> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return HashMap::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

/// Persist a snapshot on a worker thread. A failure is silent: a checkpoint is a convenience,
/// not a reason to interrupt playback.
pub fn save(map: HashMap<String, i64>) {
    std::thread::spawn(move || {
        if let Some(path) = path() {
            save_to(map, path);
        }
    });
}

fn save_to(map: HashMap<String, i64>, path: PathBuf) {
    let Some(parent) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let mut entries: Vec<(String, i64)> = map.into_iter().filter(|(_, ms)| *ms > 0).collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.1));
    entries.truncate(MAX_ENTRIES);
    let map: HashMap<String, i64> = entries.into_iter().collect();
    let Ok(text) = serde_json::to_vec(&map) else {
        return;
    };
    let temporary = path.with_extension("json.part");
    if std::fs::write(&temporary, text).is_ok() {
        let _ = std::fs::rename(temporary, path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_checkpoint_survives_a_restart() {
        let directory =
            std::env::temp_dir().join(format!("runefin-checkpoint-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("the temp directory");
        let path = directory.join("positions.json");
        let mut map = HashMap::new();
        map.insert("quiet-floor-s2e4".to_string(), 1_234_000);
        map.insert("metropolis".to_string(), 42_000);
        save_to(map.clone(), path.clone());
        assert_eq!(load_from(path), map);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn zero_positions_are_not_written() {
        let directory =
            std::env::temp_dir().join(format!("runefin-checkpoint-zero-{}", std::process::id()));
        let path = directory.join("positions.json");
        let mut map = HashMap::new();
        map.insert("finished".to_string(), 0);
        save_to(map, path.clone());
        assert!(load_from(path).is_empty());
        let _ = std::fs::remove_dir_all(directory);
    }
}
