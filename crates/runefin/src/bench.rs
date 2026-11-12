//! Opt-in measurements for the benchmark pass: `RUNEFIN_BENCH=1`.
//!
//! Two kinds of number, both printed to stdout as `bench ...` lines so a run can be grepped,
//! diffed against the next commit and pasted into a report:
//!
//! - **launch**: Rune records a mark for every step on the way to a window's first presented
//!   frame (event loop, window, GPU, renderer, glyphs, the frame itself). This module adds the
//!   app's own steps — `main`, `config`, `app`, `setup`, `home`, `detail`, `player` — and prints
//!   the whole list once, relative to process start, when `on_ready` says the first frame is up.
//! - **frames**: one line a second with what the window drew, what a frame cost, and how long an
//!   input waited for the frame it caused. That is the idle-cost and input-latency meter.
//!
//! Nothing is recorded unless the variable is set: a mark is one flag check, and the frame hook
//! is the `Option` Rune already tests. The printer thread is spawned only in a benchmark run.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use rune::{FrameEncoded, FrameOutcome};

use crate::state::State;

/// Marks this process's first step; every launch number is relative to it.
static START: OnceLock<Instant> = OnceLock::new();
static ENABLED: OnceLock<bool> = OnceLock::new();

/// Times one piece of work on a worker: `lap` prints how long it took and when it finished, so
/// a slow first paint separates the server's answers from the drawing.
pub fn timer() -> Option<Instant> {
    enabled().then(Instant::now)
}

pub fn lap(name: &'static str, since: Option<Instant>) {
    let (Some(start), Some(since)) = (START.get().copied(), since) else {
        return;
    };
    eprintln!(
        "bench net {name}={:.1}ms at +{:.1}ms",
        since.elapsed().as_secs_f64() * 1000.0,
        start.elapsed().as_secs_f64() * 1000.0
    );
}

/// How long the app's own `view` took to build this frame's element tree, in microseconds. The
/// runtime's layout, paint and encode come after it, in the frame's `update_ms`.
static VIEW_US: AtomicU64 = AtomicU64::new(0);

pub fn view_started() -> Option<Instant> {
    enabled().then(Instant::now)
}

pub fn view_built(started: Option<Instant>) {
    if let Some(started) = started {
        VIEW_US.store(started.elapsed().as_micros() as u64, Ordering::Relaxed);
    }
}

/// Data that just landed in the state, first time only: the frame trace says how long the next
/// drawn frame took after it, so a number splits into "the server answered" and "we drew it".
static LANDED: Mutex<Vec<(&'static str, Instant)>> = Mutex::new(Vec::new());
static WAITING: Mutex<Vec<(&'static str, Instant)>> = Mutex::new(Vec::new());

pub fn landed(name: &'static str) {
    if !enabled() {
        return;
    }
    let mut seen = LANDED.lock().unwrap_or_else(|e| e.into_inner());
    if seen.iter().any(|(landed, _)| *landed == name) {
        return;
    }
    let now = Instant::now();
    seen.push((name, now));
    drop(seen);
    rune::marks::mark(name);
    if let Some(start) = START.get().copied() {
        eprintln!(
            "bench landed {name} +{:.1}ms",
            now.saturating_duration_since(start).as_secs_f64() * 1000.0
        );
    }
    WAITING
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push((name, now));
}

/// The live-capture play route: `RUNEFIN_LIVE_ROUTE=play` opens the item and starts it once the
/// detail page knows what to play. `LIVE_PLAY` is 1 while waiting, 2 once started.
static LIVE_PLAY: AtomicU32 = AtomicU32::new(0);

pub fn enabled() -> bool {
    *ENABLED.get_or_init(|| std::env::var_os("RUNEFIN_BENCH").is_some())
}

/// The first call in `main`.
pub fn start() {
    if !enabled() {
        return;
    }
    START.get_or_init(Instant::now);
    mark("bench.main");
}

/// Set once the launch trace has printed: a mark after it (the player's, for one) is printed
/// as it happens instead of waiting for a trace that already went out.
static TRACED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// One step of the app's own, for the launch trace.
pub fn mark(name: &'static str) {
    if !enabled() {
        return;
    }
    rune::marks::mark(name);
    if TRACED.load(Ordering::Relaxed)
        && let Some(start) = START.get().copied()
    {
        eprintln!(
            "bench mark {name} +{:.1}ms",
            Instant::now()
                .saturating_duration_since(start)
                .as_secs_f64()
                * 1000.0
        );
    }
}

/// The live-capture play route is armed: the next poll that has a play target starts it.
pub fn arm_live_play() {
    LIVE_PLAY.store(1, Ordering::Relaxed);
}

/// Runs the armed play route once the detail page can say what to play.
pub fn drive_live_play(s: &mut State) {
    if LIVE_PLAY.load(Ordering::Relaxed) != 1 || s.in_app.is_some() {
        return;
    }
    let target = s
        .detail
        .as_ref()
        .filter(|detail| !detail.item.name.is_empty())
        .and_then(|detail| detail.play_target())
        .cloned();
    if let Some(item) = target {
        LIVE_PLAY.store(2, Ordering::Relaxed);
        mark("bench.player.start");
        s.play_episode(item);
    }
}

/// Hooks a benchmark build into the app: the launch trace, the per-second frame summary and the
/// first-frame marks the app keeps itself.
pub fn attach(app: rune::App<State>) -> rune::App<State> {
    if !enabled() {
        return app;
    }
    start_printer();
    app.on_ready(|_| launch_trace()).on_frame_encoded(record)
}

fn launch_trace() {
    let Some(start) = START.get().copied() else {
        return;
    };
    let since = |at: Instant| at.saturating_duration_since(start).as_secs_f64() * 1000.0;
    eprintln!("bench launch total_ms={:.1}", since(Instant::now()));
    for (name, at) in rune::marks::marks() {
        eprintln!("bench mark {name} +{:.1}ms", since(at));
    }
    for (name, value) in rune::marks::metrics() {
        eprintln!("bench metric {name}={value}");
    }
    for (name, value) in rune::marks::notes() {
        eprintln!("bench note {name}={value}");
    }
    TRACED.store(true, Ordering::Relaxed);
}

/// One second's worth of frames. Written by the frame hook, read and reset by the printer.
#[derive(Default)]
struct Bucket {
    drawn: u32,
    unchanged: u32,
    skipped: u32,
    update: Vec<f32>,
    encode: Vec<f32>,
    input: Vec<f32>,
}

static BUCKET: Mutex<Option<Bucket>> = Mutex::new(None);
static TOTAL_DRAWN: AtomicU64 = AtomicU64::new(0);

fn record(frame: &FrameEncoded) {
    if frame.outcome() == FrameOutcome::Drawn {
        let waiting = std::mem::take(&mut *WAITING.lock().unwrap_or_else(|e| e.into_inner()));
        for (name, at) in waiting {
            eprintln!(
                "bench data_to_frame {name}={:.1}ms",
                frame.at().saturating_duration_since(at).as_secs_f64() * 1000.0
            );
        }
    }
    let mut guard = BUCKET.lock().unwrap_or_else(|e| e.into_inner());
    let bucket = guard.get_or_insert_with(Bucket::default);
    match frame.outcome() {
        FrameOutcome::Drawn => {
            bucket.drawn += 1;
            TOTAL_DRAWN.fetch_add(1, Ordering::Relaxed);
            if let Some(update) = frame.update_ms() {
                bucket.update.push(update as f32);
                eprintln!(
                    "bench frame update_ms={update:.2} encode_ms={:.2} view_ms={:.2}",
                    frame.encode_ms().unwrap_or(0.0),
                    VIEW_US.swap(0, Ordering::Relaxed) as f32 / 1000.0
                );
                for (name, value) in rune::marks::metrics() {
                    eprintln!("bench frame metric {name}={value}");
                }
            }
            if let Some(encode) = frame.encode_ms() {
                bucket.encode.push(encode as f32);
            }
            // A frame an input caused, not a timer's: the wait a key press sat through.
            if let Some(input) = rune::last_input() {
                let wait = frame.at().saturating_duration_since(input).as_secs_f32() * 1000.0;
                if wait < 500.0 {
                    bucket.input.push(wait);
                }
            }
        }
        FrameOutcome::Unchanged => bucket.unchanged += 1,
        FrameOutcome::Skipped => bucket.skipped += 1,
        _ => {}
    }
    // A runaway second of frames would otherwise grow this without bound.
    if bucket.update.len() > 600 {
        bucket.update.clear();
        bucket.encode.clear();
        bucket.input.clear();
    }
}

fn start_printer() {
    static ONCE: OnceLock<()> = OnceLock::new();
    if ONCE.set(()).is_err() {
        return;
    }
    std::thread::Builder::new()
        .name("runefin-bench".into())
        .spawn(|| {
            let start = *START.get_or_init(Instant::now);
            let mut last = Instant::now();
            loop {
                std::thread::sleep(Duration::from_millis(250));
                if last.elapsed() < Duration::from_secs(1) {
                    continue;
                }
                let window = last.elapsed().as_secs_f64();
                last = Instant::now();
                let bucket = BUCKET
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .take()
                    .unwrap_or_default();
                eprintln!(
                    "bench {:.1}s drawn={} unchanged={} skipped={} update_ms p50={:.2} p95={:.2} max={:.2} encode_ms p50={:.2} p95={:.2} input_ms p50={:.2} max={:.2} total_drawn={} since_ms={:.0}",
                    window,
                    bucket.drawn,
                    bucket.unchanged,
                    bucket.skipped,
                    pct(&bucket.update, 50.0),
                    pct(&bucket.update, 95.0),
                    bucket.update.iter().copied().fold(0.0, f32::max),
                    pct(&bucket.encode, 50.0),
                    pct(&bucket.encode, 95.0),
                    pct(&bucket.input, 50.0),
                    bucket.input.iter().copied().fold(0.0, f32::max),
                    TOTAL_DRAWN.load(Ordering::Relaxed),
                    start.elapsed().as_secs_f64() * 1000.0,
                );
            }
        })
        .expect("bench printer thread");
}

fn pct(values: &[f32], which: f32) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let at = ((which / 100.0) * (sorted.len() - 1) as f32).round() as usize;
    sorted[at.min(sorted.len() - 1)]
}
