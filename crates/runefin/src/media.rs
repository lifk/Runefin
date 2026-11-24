//! The host-side Media mode's first-frame proof.
//!
//! The app cannot write to the host home directly. Inside the Flatpak, `XDG_CACHE_HOME` is
//! the app's `~/.var/app/io.github.lifk.Runefin/cache`, which the host supervisor can read.
//! The write is off the UI thread; a missing cache directory simply means no ready marker and
//! the supervisor takes its normal timeout path.

use std::path::PathBuf;

const READY: &str = "media-ready";

/// The file the supervisor watches. Keep in step with `packaging/modes/supervisor.sh`.
pub fn ready_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(base.join("runefin").join(READY))
}

/// Drop the marker after the first rendered frame, without a disk write on the UI thread.
pub fn report_ready() {
    let Some(path) = ready_path() else {
        return;
    };
    std::thread::Builder::new()
        .name("runefin-media-ready".into())
        .spawn(move || {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, b"ready\n");
        })
        .ok();
}
