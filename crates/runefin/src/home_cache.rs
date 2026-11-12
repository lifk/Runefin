//! The last home screen on disk, so a launch paints its rails instead of a skeleton while the
//! server answers.
//!
//! The first home request on this server takes 350–580 ms (libraries, then Continue Watching and
//! one Recently Added query per library, all over one TLS connection each). Nothing about that
//! is ours to make faster, so the app shows the rails it had when it last ran and replaces them
//! as the answers arrive. The file is a cache: keyed by server and user, versioned, rewritten
//! after the rails settle, ignored when it does not read, and removed on sign-out.
//!
//! It holds what the server already showed this reader — titles, image tags, progress — in
//! `$XDG_CACHE_HOME/runefin/home-<hash>.json`, beside the image cache and the checkpoints, with
//! no token and no credential in it.

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::home::HomeData;

const VERSION: u32 = 1;
/// Quiet time after the last rail landed before the snapshot is written: the rails arrive within
/// a few hundred milliseconds of each other, and one write is enough.
const SETTLE: std::time::Duration = std::time::Duration::from_millis(600);

#[derive(Serialize, Deserialize)]
struct Snapshot {
    v: u32,
    server: String,
    user: String,
    saved: u64,
    home: HomeData,
}

fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(base.join("runefin"))
}

/// Stable and filesystem-safe: a server and user id are a URL and a GUID.
fn key(server: &str, user: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    server.hash(&mut hasher);
    user.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub fn path_for(server: &str, user: &str) -> Option<PathBuf> {
    Some(cache_dir()?.join(format!("home-{}.json", key(server, user))))
}

/// The rails this reader last saw, when the file is ours and fresh enough. `now` is seconds since
/// the epoch, as the snapshot's own `saved` is.
pub fn load(server: &str, user: &str, now: i64) -> Option<HomeData> {
    load_from(&path_for(server, user)?, server, user, now)
}

fn load_from(path: &std::path::Path, server: &str, user: &str, now: i64) -> Option<HomeData> {
    let text = std::fs::read_to_string(path).ok()?;
    let snapshot: Snapshot = serde_json::from_str(&text).ok()?;
    if snapshot.v != VERSION || snapshot.server != server || snapshot.user != user {
        return None;
    }
    // A rail from last month is a lie about what is new; a day-old one is what the reader saw
    // when they last opened the app.
    if now.saturating_sub(snapshot.saved as i64) > 24 * 60 * 60 {
        return None;
    }
    Some(snapshot.home)
}

/// Seconds since the epoch, the clock both the snapshot and `load` compare on.
pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// Writes the snapshot once the rails have settled. Cheap to call after every rail: the newest
/// one wins, one thread writes.
pub fn save_soon(server: String, user: String, home: HomeData) {
    static PENDING: OnceLock<Mutex<Option<(String, String, HomeData)>>> = OnceLock::new();
    let pending = PENDING.get_or_init(|| Mutex::new(None));
    *pending.lock().unwrap_or_else(|e| e.into_inner()) = Some((server, user, home));
    static THREAD: OnceLock<()> = OnceLock::new();
    if THREAD.set(()).is_ok() {
        std::thread::Builder::new()
            .name("runefin-home-cache".into())
            .spawn(|| {
                let pending = PENDING.get().expect("pending slot");
                loop {
                    std::thread::sleep(SETTLE);
                    let next = pending.lock().unwrap_or_else(|e| e.into_inner()).take();
                    if let Some((server, user, home)) = next
                        && let Some(path) = path_for(&server, &user)
                    {
                        let _ = write(&path, &server, &user, &home);
                    }
                }
            })
            .ok();
    }
}

/// Sign-out, or a different account: nothing of the last reader stays on disk.
pub fn forget(server: &str, user: &str) {
    if let Some(path) = path_for(server, user) {
        let _ = std::fs::remove_file(path);
    }
}

fn write(path: &std::path::Path, server: &str, user: &str, home: &HomeData) -> Option<()> {
    write_at(path, server, user, home, now_secs() as u64)
}

fn write_at(
    path: &std::path::Path,
    server: &str,
    user: &str,
    home: &HomeData,
    saved: u64,
) -> Option<()> {
    let snapshot = Snapshot {
        v: VERSION,
        server: server.to_string(),
        user: user.to_string(),
        saved,
        home: home.clone(),
    };
    let text = serde_json::to_string(&snapshot).ok()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).ok()?;
    }
    // A reader that dies mid-write leaves the old file, never half of a new one.
    let temp = path.with_extension("json.new");
    let mut file = std::fs::File::create(&temp).ok()?;
    file.write_all(text.as_bytes()).ok()?;
    file.sync_all().ok()?;
    std::fs::rename(&temp, path).ok()?;
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use jellyfin_api::Item;

    fn home() -> HomeData {
        HomeData {
            resume: vec![Item {
                id: "quiet-floor".into(),
                name: "The Quiet Floor".into(),
                ..Item::default()
            }],
            ..HomeData::default()
        }
    }

    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "runefin-home-cache-{name}-{}.json",
            std::process::id()
        ))
    }

    #[test]
    fn a_snapshot_round_trips_for_its_own_server_and_user() {
        let path = temp("round-trip");
        let _ = std::fs::remove_file(&path);
        write_at(&path, "https://a.example", "user-1", &home(), 1_000).expect("written");
        let loaded = load_from(&path, "https://a.example", "user-1", 2_000).expect("loaded");
        assert_eq!(loaded.resume.len(), 1);
        assert_eq!(loaded.resume[0].name, "The Quiet Floor");
        // Another server or user reads nothing, and neither does a day-old snapshot.
        assert!(load_from(&path, "https://b.example", "user-1", 2_000).is_none());
        assert!(load_from(&path, "https://a.example", "user-2", 2_000).is_none());
        assert!(
            load_from(
                &path,
                "https://a.example",
                "user-1",
                1_000 + 2 * 24 * 60 * 60
            )
            .is_none()
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_file_that_does_not_read_is_no_snapshot() {
        let path = temp("garbage");
        std::fs::write(&path, b"{\"v\":1,\"home\":").expect("written");
        assert!(load_from(&path, "https://a.example", "user-1", 0).is_none());
        let _ = std::fs::remove_file(&path);
    }
}
