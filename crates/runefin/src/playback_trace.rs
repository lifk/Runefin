//! Opt-in playback instrumentation. `RUNEFIN_TRACE_PLAYBACK=1` prints one line every two
//! seconds with the frame pacing the sink actually achieved and whether the media clock
//! advanced evenly. It is how a report of "it stutters and then races" becomes a number.
//!
//! Off by default: the counters cost one atomic increment per rendered frame and the thread is
//! only spawned when the variable is set.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use gstreamer as gst;
use gstreamer::prelude::*;

/// Wall-clock microseconds from a monotonic clock. The sink renders on GStreamer's streaming
/// thread, so the gap between two `render` calls is the decoder/sink pacing, not the UI's.
pub fn now_us() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_micros() as u64
}

#[derive(Default)]
pub struct Trace {
    frames: AtomicU64,
    last_us: AtomicU64,
    max_gap_us: AtomicU64,
    /// Gaps longer than this are a visible freeze rather than jitter.
    stalls: AtomicU64,
    /// Renders closer together than this, following a stall, are the catch-up burst.
    bursts: AtomicU64,
}

impl Trace {
    /// A video frame reached the sink.
    pub fn tick(&self) {
        self.tick_at(now_us());
    }

    pub fn tick_at(&self, now_us: u64) {
        let last = self.last_us.swap(now_us, Ordering::Relaxed);
        self.frames.fetch_add(1, Ordering::Relaxed);
        if last == 0 || now_us <= last {
            return;
        }
        let gap = now_us - last;
        self.max_gap_us.fetch_max(gap, Ordering::Relaxed);
        if gap > 50_000 {
            self.stalls.fetch_add(1, Ordering::Relaxed);
        }
        if gap < 5_000 {
            self.bursts.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// `(frames, stalls, bursts, max gap in ms)` since start. Read-and-continue: the caller
    /// subtracts consecutive reads to get a window.
    pub fn snapshot(&self) -> TraceSnapshot {
        TraceSnapshot {
            frames: self.frames.load(Ordering::Relaxed),
            stalls: self.stalls.load(Ordering::Relaxed),
            bursts: self.bursts.load(Ordering::Relaxed),
            max_gap_ms: self.max_gap_us.load(Ordering::Relaxed) / 1_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TraceSnapshot {
    pub frames: u64,
    pub stalls: u64,
    pub bursts: u64,
    pub max_gap_ms: u64,
}

impl TraceSnapshot {
    /// What happened between an earlier snapshot and this one.
    pub fn between(earlier: TraceSnapshot, later: TraceSnapshot) -> TraceSnapshot {
        TraceSnapshot {
            frames: later.frames.saturating_sub(earlier.frames),
            stalls: later.stalls.saturating_sub(earlier.stalls),
            bursts: later.bursts.saturating_sub(earlier.bursts),
            max_gap_ms: later.max_gap_ms,
        }
    }
}

/// Spawn the reporter. It samples the pipeline clock every 100 ms while the window is open, so
/// a frozen clock (audio stall) and a burst of renders (sink backlog) can be told apart.
pub fn spawn_reporter(
    trace: Arc<Trace>,
    pipeline: gst::Pipeline,
    path: String,
    stopped: Arc<AtomicBool>,
) {
    std::thread::Builder::new()
        .name("runefin-trace".into())
        .spawn(move || {
            let mut last = trace.snapshot();
            let mut clock = pipeline.query_position::<gst::ClockTime>();
            loop {
                for _ in 0..20 {
                    std::thread::sleep(Duration::from_millis(100));
                    if stopped.load(Ordering::Relaxed) {
                        return;
                    }
                }
                let mut frozen = 0u32;
                let mut jumps = 0u32;
                for _ in 0..20 {
                    std::thread::sleep(Duration::from_millis(100));
                    if stopped.load(Ordering::Relaxed) {
                        return;
                    }
                    let now = pipeline.query_position::<gst::ClockTime>();
                    if let (Some(a), Some(b)) = (clock, now) {
                        let delta = b.mseconds() as i64 - a.mseconds() as i64;
                        if delta <= 0 {
                            frozen += 1;
                        } else if delta > 300 {
                            jumps += 1;
                        }
                    }
                    clock = now;
                }
                let now = trace.snapshot();
                let window = TraceSnapshot::between(last, now);
                last = now;
                eprintln!(
                    "playback: {path} | renders {} in 4 s ({:.1}/s), stalls {}, bursts {}, \
                     longest gap {} ms | clock frozen samples {frozen}, jumps {jumps}",
                    window.frames,
                    window.frames as f64 / 4.0,
                    window.stalls,
                    window.bursts,
                    window.max_gap_ms,
                );
            }
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gap_after_a_stall_is_counted_as_a_stall_and_the_renders_after_it_as_bursts() {
        let trace = Trace::default();
        trace.tick_at(1_000_000);
        trace.tick_at(1_900_000); // 900 ms freeze, then a burst
        trace.tick_at(1_902_000);
        trace.tick_at(1_904_000);
        let snapshot = trace.snapshot();
        assert_eq!(snapshot.frames, 4);
        assert_eq!(snapshot.stalls, 1);
        assert_eq!(snapshot.bursts, 2);
        assert_eq!(snapshot.max_gap_ms, 900);
    }

    #[test]
    fn a_steady_24_fps_has_no_stalls_and_no_bursts() {
        let trace = Trace::default();
        for frame in 0..50u64 {
            trace.tick_at(1_000_000 + frame * 41_667);
        }
        let snapshot = trace.snapshot();
        assert_eq!(snapshot.frames, 50);
        assert_eq!(snapshot.stalls, 0);
        assert_eq!(snapshot.bursts, 0);
    }

    #[test]
    fn a_window_subtracts_an_earlier_snapshot() {
        let earlier = TraceSnapshot {
            frames: 10,
            stalls: 1,
            bursts: 2,
            max_gap_ms: 300,
        };
        let later = TraceSnapshot {
            frames: 14,
            stalls: 1,
            bursts: 5,
            max_gap_ms: 420,
        };
        assert_eq!(
            TraceSnapshot::between(earlier, later),
            TraceSnapshot {
                frames: 4,
                stalls: 0,
                bursts: 3,
                max_gap_ms: 420,
            }
        );
    }
}
