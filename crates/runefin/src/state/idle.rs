//! Keep the display and the machine awake while a video plays.
//!
//! The app has no D-Bus binding of its own and the design forbids adding one for this, so it
//! asks `systemd-inhibit` for a block lock and stops the child when playback ends. Where the
//! tool or a session bus is missing the spawn fails and playback is untouched: an inhibitor is
//! a courtesy, never a requirement.

use std::process::{Child, Command, Stdio};

/// A held `systemd-inhibit` lock, if one could be taken.
#[derive(Default)]
pub struct IdleGuard {
    child: Option<Child>,
}

impl IdleGuard {
    /// Take the lock. Calling it twice keeps the one already held.
    pub fn hold(&mut self) {
        if self.active() {
            return;
        }
        let child = Command::new("systemd-inhibit")
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
            .spawn();
        self.child = child.ok();
    }

    /// Release the lock and reap the helper, so no zombie is left behind.
    pub fn release(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// True while the helper is alive. A tool that exits immediately (no session bus, or no
    /// systemd) reads as not held, which is exactly what the OSD should say.
    pub fn active(&mut self) -> bool {
        let Some(child) = &mut self.child else {
            return false;
        };
        match child.try_wait() {
            Ok(None) => true,
            _ => {
                self.child = None;
                false
            }
        }
    }
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
