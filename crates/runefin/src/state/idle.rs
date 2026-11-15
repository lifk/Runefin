//! Keep the display and the machine awake while a video plays.
//!
//! Two paths, both off the UI thread: `systemd-inhibit` on a native install, and a direct
//! `org.freedesktop.login1` `Inhibit` call through the app's existing D-Bus stack. The second
//! exists because the Freedesktop runtime a Flatpak runs on does not ship the
//! `systemd-inhibit` binary, so the first silently failed on the recommended install. Where
//! neither is available playback is untouched: an inhibitor is a courtesy, never a requirement.

use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};

type Release = Sender<()>;

enum Held {
    Process(Child),
    /// The logind inhibitor holds an fd on its own thread; dropping this sender tells that
    /// thread to return, which closes the fd and releases the lock.
    Session(Release, std::thread::JoinHandle<()>),
}

/// A held idle/sleep lock, if one could be taken.
#[derive(Default)]
pub struct IdleGuard {
    held: Option<Held>,
}

impl IdleGuard {
    /// Take the lock. Calling it twice keeps the one already held.
    pub fn hold(&mut self) {
        if self.active() {
            return;
        }
        // The native tool includes the reason in `systemd-inhibit --list`, which is what a
        // desktop's power panel shows. Inside a Flatpak it does not exist, and the spawn fails.
        match Command::new("systemd-inhibit")
            .args([
                "--what=idle:sleep",
                "--who=Runefin",
                "--why=Playing a video",
                "--mode=block",
                "sleep",
                "infinity",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => self.held = Some(Held::Process(child)),
            Err(_) => self.held = logind_lock(),
        }
    }

    /// Release the lock and reap the helper, so no zombie is left behind.
    pub fn release(&mut self) {
        if let Some(held) = self.held.take() {
            match held {
                Held::Process(mut child) => {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                Held::Session(release, thread) => {
                    // Closing the fd on the thread is the release; do not block the UI on it.
                    drop(release);
                    let _ = thread;
                }
            }
        }
        // A logind thread whose sender is dropped returns on its own; the JoinHandle is not
        // joined because a wedged bus must never hold the exit path.
    }

    /// True while a lock is alive. A helper that exits immediately (no session bus, or no
    /// systemd) reads as not held, which is exactly what the OSD should say.
    pub fn active(&mut self) -> bool {
        match &mut self.held {
            None => false,
            Some(Held::Process(child)) => match child.try_wait() {
                Ok(None) => true,
                _ => {
                    self.held = None;
                    false
                }
            },
            // The logind thread only ends when the release sender is dropped (or the bus call
            // failed, in which case the fd was never taken and the thread is gone).
            Some(Held::Session(_, thread)) => !thread.is_finished(),
        }
    }
}

/// Ask logind for an `idle` inhibitor on the system bus and hold its fd on a worker thread.
/// `Inhibit` returns a file descriptor that belongs to the caller; closing it releases the
/// lock, so the thread parks on a channel until `release`.
fn logind_lock() -> Option<Held> {
    let (release, wait): (Release, Receiver<()>) = channel();
    let thread = std::thread::Builder::new()
        .name("runefin-inhibit".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => return,
            };
            runtime.block_on(async move {
                let Ok(connection) = zbus::Connection::system().await else {
                    return;
                };
                let reply = connection
                    .call_method(
                        Some("org.freedesktop.login1"),
                        "/org/freedesktop/login1",
                        Some("org.freedesktop.login1.Manager"),
                        "Inhibit",
                        &("idle", "Runefin", "Playing a video", "block"),
                    )
                    .await;
                let Ok(reply) = reply else {
                    return;
                };
                let Ok(fd) = reply.body().deserialize::<zbus::OwnedFd>() else {
                    return;
                };
                // The fd is the lock. Park until `release` drops the sender (or the app exits,
                // which closes it too). Nothing here runs on the UI thread.
                let _ = wait.recv();
                drop(fd);
            });
        })
        .ok()?;
    // The thread takes the lock when the bus answers; `active` reads whether it is still alive.
    // There is no wait here: a missing logind must not stall the frame that starts playback.
    Some(Held::Session(release, thread))
}

impl Drop for IdleGuard {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_guard_that_never_held_a_lock_is_not_active_and_releases_cleanly() {
        let mut guard = IdleGuard::default();
        assert!(!guard.active());
        guard.release();
        assert!(!guard.active());
    }
}
