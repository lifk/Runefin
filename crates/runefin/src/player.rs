//! In-app playback through GStreamer.
//!
//! The first in-app slice takes decoded NV12 frames on the CPU and hands them to a Rune
//! `NativeSurface`, which already has a two-plane YCbCr shader path. That proves the window,
//! clock, seek and reporting paths while moving half the bytes of BGRA. The next slice replaces
//! `appsink`'s CPU map with a DMA-BUF sink so 4K frames never leave the GPU.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app::AppSink;
use gstreamer_video::VideoInfo;
use rune::NativeSurface;
use rune::native_surface::{ByteFrame, ColorInfo, Format, Matrix, Transfer};

use crate::playback_trace::{self, Trace};

/// The static colour facts of the current video stream, read from the caps. `max_luminance`
/// and `min_luminance` are the mastering display's, in nits; the light levels are the content's.
#[derive(Clone, Copy, Debug)]
pub struct ColourFacts {
    pub transfer: &'static str,
    pub matrix: &'static str,
    pub max_luminance: f32,
    pub min_luminance: f32,
    pub max_cll: u16,
    pub max_fall: u16,
}

impl Default for ColourFacts {
    fn default() -> Self {
        Self {
            transfer: "",
            matrix: "",
            max_luminance: 0.0,
            min_luminance: 0.0,
            max_cll: 0,
            max_fall: 0,
        }
    }
}

pub struct Player {
    pipeline: gst::Pipeline,
    bus: gst::Bus,
    surface: NativeSurface,
    eos: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    /// Buffers that reached the sink; the stats overlay and the performance probe read it.
    frames: Arc<AtomicU64>,
    /// Latest buffering percentage from the bus; 100 means playing. A source that stalls (a
    /// slow transcode, a flaky network) reports below 100 here, and the OSD says so.
    buffering: Arc<AtomicU64>,
    /// Bits of the current rate. A rate is applied as a flush seek; repeating the same value
    /// would seek for no reason and can race the next seek.
    rate: AtomicU64,
    dmabuf: bool,
    dmabuf_kind: Arc<Mutex<Option<&'static str>>>,
    paused: std::sync::atomic::AtomicBool,
    /// Only the headless OSD snapshot sets this; a real pipeline reports its own clock.
    preview_position: Option<i64>,
    /// The audio-filter bin's live controls. `None` on a preview pipeline and when GStreamer
    /// lacks the elements; every setter then does nothing.
    downmix: Option<gst::Element>,
    delay: Option<gst::Element>,
    dynamic: Option<gst::Element>,
    /// Frame pacing counters, shared with the sink; read by the opt-in trace reporter.
    trace: Arc<Trace>,
    /// Tells the trace reporter to stop before the pipeline is torn down.
    stopped: Arc<AtomicBool>,
    /// Static HDR facts from the caps, for the Statistics panel.
    colour: Arc<Mutex<Option<ColourFacts>>>,
}

/// A pipeline started and at its resume point: `started_ms` is the position the first picture
/// actually shows, or `None` when the resume point could not be reached.
pub struct Started {
    pub player: Player,
    pub started_ms: Option<i64>,
}

impl Player {
    /// A pipeline with no source, used by the headless OSD snapshots. It proves the player
    /// chrome's geometry without requiring a video window or a server. Kept in release builds
    /// because `--snap` is a shipping tool, not a debug-only path.
    #[cfg(any(test, debug_assertions))]
    pub fn preview() -> Player {
        let _ = gst::init();
        let playbin = gst::ElementFactory::make("playbin")
            .build()
            .expect("the OSD snapshot needs playbin");
        let pipeline = playbin
            .downcast::<gst::Pipeline>()
            .expect("playbin is a pipeline");
        let bus = pipeline.bus().expect("a new GStreamer pipeline has a bus");
        Player {
            pipeline,
            bus,
            surface: NativeSurface::new(),
            eos: Arc::new(AtomicBool::new(false)),
            error: Arc::new(Mutex::new(None)),
            frames: Arc::new(AtomicU64::new(0)),
            buffering: Arc::new(AtomicU64::new(100)),
            rate: AtomicU64::new(1.0f64.to_bits()),
            dmabuf: false,
            dmabuf_kind: Arc::new(Mutex::new(None)),
            paused: std::sync::atomic::AtomicBool::new(false),
            preview_position: Some(42 * 60_000),
            downmix: None,
            delay: None,
            dynamic: None,
            trace: Arc::new(Trace::default()),
            stopped: Arc::new(AtomicBool::new(false)),
            colour: Arc::new(Mutex::new(None)),
        }
    }

    /// A headless player whose clock already sits at `position_ms`, so a snapshot can show
    /// the end-of-episode offer without waiting through the film.
    #[cfg(any(test, debug_assertions))]
    pub fn preview_at(position_ms: i64) -> Player {
        let mut player = Self::preview();
        player.preview_position = Some(position_ms);
        player
    }

    pub fn start(
        url: &str,
        surface: NativeSurface,
        waker: Option<rune::Waker>,
    ) -> Result<Player, String> {
        Self::start_at(url, surface, waker, None).map(|started| started.player)
    }

    /// [`Player::start`] from a resume point: the seek is queued the moment the pipeline is
    /// PLAYING instead of after the first picture, so the frame the wait sees is the resume
    /// frame. A seek the pipeline ignored is caught by the clock check and retried once, which
    /// is what `start` followed by `seek_and_confirm` does — only the first, wasted frame is
    /// skipped. `started` is `None` when the resume point could not be reached.
    pub fn start_at(
        url: &str,
        surface: NativeSurface,
        waker: Option<rune::Waker>,
        start_ms: Option<i64>,
    ) -> Result<Started, String> {
        gst::init().map_err(|error| format!("GStreamer could not start: {error}"))?;
        if let Some(waker) = waker {
            surface.set_waker(waker);
        }
        let target = start_ms.filter(|ms| *ms > 0);
        crate::bench::mark("bench.player.pipeline");
        // The DMA-BUF path is the fast one, but a file the VA-API converter refuses reaches
        // PLAYING and then produces no frames. Waiting the CPU path's full timeout there costs
        // six seconds on every such file; two seconds is enough for a healthy first frame.
        let dmabuf = match start_pipeline(url, surface.clone(), true) {
            Ok(player) => match player.first_frame_at(target, Duration::from_secs(8)) {
                Ok(started) => {
                    crate::bench::mark("bench.player.first_frame");
                    player.begin_trace();
                    return Ok(Started {
                        player,
                        started_ms: started,
                    });
                }
                Err(error) => error,
            },
            Err(error) => error,
        };
        eprintln!("in-app: DMA-BUF path unavailable ({dmabuf}); using CPU");
        match start_pipeline(url, surface, false) {
            Ok(player) => match player.first_frame_at(target, Duration::from_secs(12)) {
                Ok(started) => Ok(Started {
                    player,
                    started_ms: started,
                }),
                Err(cpu) => Err(format!(
                    "GStreamer produced no frames: {cpu} (DMA-BUF path: {dmabuf})"
                )),
            },
            Err(cpu) => Err(format!(
                "GStreamer couldn't start: {cpu} (DMA-BUF path: {dmabuf})"
            )),
        }
    }

    /// Queues a seek at `target`, waits for the first picture, and says whether the clock is
    /// there. A picture that arrived at the file's beginning (the pipeline took the seek before
    /// it could queue one) is followed by one confirmed seek.
    fn first_frame_at(
        &self,
        target: Option<i64>,
        timeout: Duration,
    ) -> Result<Option<i64>, String> {
        let Some(target) = target else {
            self.wait_for_first_frame(timeout)?;
            return Ok(None);
        };
        self.seek_ms(target);
        self.wait_for_first_frame(timeout)?;
        if (self.position_ms() - target).abs() <= 1_500 {
            return Ok(Some(target));
        }
        match self.seek_and_confirm(target) {
            Ok(()) => Ok(Some(target)),
            // The resume point is unreachable; the picture at the start is what plays.
            Err(_) => Ok(None),
        }
    }

    /// Audio only: no surface, no video sink, no first-frame wait. Used for a theme song,
    /// which is decoration rather than playback.
    pub fn start_audio(url: &str) -> Result<Player, String> {
        gst::init().map_err(|error| format!("GStreamer could not start: {error}"))?;
        let playbin = gst::ElementFactory::make("playbin")
            .build()
            .map_err(|error| format!("GStreamer has no playbin: {error}"))?;
        playbin.set_property_from_str("flags", "soft-volume+audio");
        playbin.set_property("uri", url);
        playbin
            .set_state(gst::State::Playing)
            .map_err(|error| format!("GStreamer couldn't start the theme: {error}"))?;
        let pipeline = playbin
            .downcast::<gst::Pipeline>()
            .map_err(|_| "GStreamer playbin is not a pipeline".to_string())?;
        let bus = pipeline
            .bus()
            .ok_or_else(|| "GStreamer pipeline has no bus".to_string())?;
        let (_, current, pending) = pipeline.state(Some(gst::ClockTime::from_seconds(5)));
        if current != gst::State::Playing && pending != gst::State::Playing {
            let _ = pipeline.set_state(gst::State::Null);
            return Err("the theme song did not start".into());
        }
        crate::bench::mark("bench.player.audio");
        Ok(Player {
            pipeline,
            bus,
            surface: NativeSurface::new(),
            eos: Arc::new(AtomicBool::new(false)),
            error: Arc::new(Mutex::new(None)),
            frames: Arc::new(AtomicU64::new(0)),
            buffering: Arc::new(AtomicU64::new(100)),
            rate: AtomicU64::new(1.0f64.to_bits()),
            dmabuf: false,
            dmabuf_kind: Arc::new(Mutex::new(None)),
            paused: std::sync::atomic::AtomicBool::new(false),
            preview_position: None,
            downmix: None,
            delay: None,
            dynamic: None,
            trace: Arc::new(Trace::default()),
            stopped: Arc::new(AtomicBool::new(false)),
            colour: Arc::new(Mutex::new(None)),
        })
    }

    /// The pipeline can reach PLAYING and still never produce a buffer (a 4K HDR
    /// VA-API postproc that refuses the caps, for example). Fail the start so the
    /// caller can fall back instead of showing a black player with a frozen clock.
    ///
    /// A source that is still starting is not a failure: a server-side transcode can take
    /// several seconds before the first buffer, and treating that as "unplayable" sent a
    /// perfectly playable episode to the mpv fallback. While the pipeline is moving toward
    /// PLAYING, or the bus reports buffering, the budget grows to 20 s; a real bus error still
    /// fails at once.
    fn wait_for_first_frame(&self, timeout: Duration) -> Result<(), String> {
        let started = Instant::now();
        let mut limit = timeout;
        loop {
            if self.position_ms() > 0 {
                return Ok(());
            }
            if let Some(error) = self.error() {
                return Err(error);
            }
            if self.has_ended() {
                return Err(
                    "the stream ended before the first frame; the file may be empty or unreadable"
                        .into(),
                );
            }
            if self.buffering().is_some() {
                limit = limit.max(Duration::from_secs(20));
            } else {
                let (_, current, pending) = self.pipeline.state(gst::ClockTime::ZERO);
                if current != gst::State::Playing && pending != gst::State::VoidPending {
                    limit = limit.max(Duration::from_secs(20));
                }
            }
            if started.elapsed() >= limit {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Err("no frames arrived before the startup timeout".into())
    }

    /// Start the opt-in reporter once the first frame has arrived, so its path label is the
    /// negotiated one rather than a guess.
    fn begin_trace(&self) {
        if std::env::var_os("RUNEFIN_TRACE_PLAYBACK").is_none() {
            return;
        }
        playback_trace::spawn_reporter(
            Arc::clone(&self.trace),
            self.pipeline.clone(),
            self.path_label(),
            Arc::clone(&self.stopped),
        );
    }

    /// Frame pacing since the player started, for the ignored probe.
    #[cfg(test)]
    pub fn pacing(&self) -> crate::playback_trace::TraceSnapshot {
        self.trace.snapshot()
    }

    /// The static colour facts negotiated for this stream, once the sink has seen caps.
    pub fn colour_facts(&self) -> Option<ColourFacts> {
        self.colour.lock().ok().and_then(|facts| *facts)
    }

    pub fn frames(&self) -> u64 {
        self.frames.load(Ordering::Relaxed)
    }

    pub fn path_label(&self) -> String {
        if !self.dmabuf {
            return "CPU".into();
        }
        match self
            .dmabuf_kind
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .as_deref()
        {
            Some(kind) => format!("DMA-BUF {kind}"),
            None => "DMA-BUF".into(),
        }
    }

    pub fn surface(&self) -> &NativeSurface {
        &self.surface
    }

    pub fn position_ms(&self) -> i64 {
        if let Some(position) = self.preview_position {
            return position;
        }
        self.pipeline
            .query_position::<gst::ClockTime>()
            .map_or(0, |time| (time.mseconds() as i64).max(0))
    }

    pub fn duration_ms(&self) -> Option<i64> {
        self.pipeline
            .query_duration::<gst::ClockTime>()
            .map(|time| time.mseconds() as i64)
    }

    /// `(count, current)` for playbin's audio streams. The live item's dual audio is the first
    /// real exercise; a single stream keeps the control off the screen.
    pub fn audio_tracks(&self) -> (i32, i32) {
        if self.pipeline.find_property("n-audio").is_none() {
            return (0, 0);
        }
        let count = self.pipeline.property::<i32>("n-audio").max(0);
        let current = self.pipeline.property::<i32>("current-audio").max(0);
        (count, current)
    }

    /// Change the playback rate. GStreamer expresses this as a seek to the current position with
    /// the new rate; the position clock then advances at that rate, so Jellyfin reporting stays
    /// in media time without scaling anything on our side.
    pub fn set_rate(&self, rate: f64) {
        let current = f64::from_bits(self.rate.load(Ordering::Relaxed));
        if (rate - current).abs() < 0.001 {
            return;
        }
        self.rate.store(rate.to_bits(), Ordering::Relaxed);
        let position = self
            .pipeline
            .query_position::<gst::ClockTime>()
            .unwrap_or(gst::ClockTime::ZERO);
        let _ = self.pipeline.seek(
            rate,
            gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
            gst::SeekType::Set,
            position,
            gst::SeekType::None,
            gst::ClockTime::NONE,
        );
    }

    /// Switch to the audio stream at `ordinal` (its order in the file, as Jellyfin lists it).
    pub fn select_audio(&self, ordinal: i32) {
        let (count, _) = self.audio_tracks();
        if ordinal >= 0 && ordinal < count {
            self.pipeline.set_property("current-audio", ordinal);
        }
    }

    pub fn is_muted(&self) -> bool {
        self.pipeline.property::<bool>("mute")
    }

    pub fn toggle_mute(&self) {
        self.pipeline.set_property("mute", !self.is_muted());
    }

    pub fn set_volume(&self, volume: f64) {
        self.pipeline.set_property("volume", volume.clamp(0.0, 2.0));
    }

    #[cfg(test)]
    pub fn volume(&self) -> f64 {
        self.pipeline.property::<f64>("volume")
    }

    /// Fold every channel into two. Off restores the file's own layout.
    pub fn set_downmix(&self, on: bool) {
        let Some(filter) = &self.downmix else {
            return;
        };
        let caps = if on {
            gst::Caps::builder("audio/x-raw")
                .field("channels", 2i32)
                .build()
        } else {
            gst::Caps::builder("audio/x-raw").build()
        };
        filter.set_property("caps", &caps);
    }

    /// Shift audio against video, in milliseconds. Positive plays audio later.
    pub fn set_audio_delay(&self, ms: i64) {
        if let Some(delay) = &self.delay {
            delay.set_property("ts-offset", ms.clamp(-100, 100) * 1_000_000);
        }
    }

    /// `off`, `dialogue` or `night`. The element is created up front so mode changes never
    /// renegotiate the pipeline.
    pub fn set_audio_dynamics(&self, mode: &str) {
        let Some(dynamic) = &self.dynamic else {
            return;
        };
        match mode {
            "dialogue" => {
                dynamic.set_property_from_str("characteristics", "soft-knee");
                dynamic.set_property_from_str("mode", "compressor");
                dynamic.set_property("threshold", -22.0f32);
                dynamic.set_property("ratio", 4.0f32);
            }
            "night" => {
                dynamic.set_property_from_str("characteristics", "hard-knee");
                dynamic.set_property_from_str("mode", "compressor");
                dynamic.set_property("threshold", -32.0f32);
                dynamic.set_property("ratio", 10.0f32);
            }
            _ => {
                dynamic.set_property("threshold", 0.0f32);
                dynamic.set_property("ratio", 1.0f32);
            }
        }
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
        let _ = self.pipeline.set_state(if paused {
            gst::State::Paused
        } else {
            gst::State::Playing
        });
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn seek_ms(&self, position_ms: i64) {
        let started = Instant::now();
        // `seek_simple` resets the rate to 1.0. A seek while watching at 1.25× must stay at
        // 1.25×, so use the full seek with the tracked rate.
        let rate = f64::from_bits(self.rate.load(Ordering::Relaxed));
        let _ = self.pipeline.seek(
            rate,
            gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
            gst::SeekType::Set,
            gst::ClockTime::from_mseconds(position_ms.max(0) as u64),
            gst::SeekType::None,
            gst::ClockTime::NONE,
        );
        if started.elapsed() > Duration::from_millis(20)
            && std::env::var_os("RUNEFIN_TRACE_PLAYBACK").is_some()
        {
            eprintln!(
                "playback: flush seek took {} ms (on the UI thread)",
                started.elapsed().as_millis()
            );
        }
    }

    /// Seek and wait for the pipeline to report the target. A resume that is announced on the
    /// clock but not applied to the pipeline shows the first frame under a resume timeline;
    /// confirming here turns that into either a real seek or a truthful zero start.
    pub fn seek_and_confirm(&self, position_ms: i64) -> Result<(), String> {
        for _ in 0..2 {
            self.seek_ms(position_ms);
            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline {
                if (self.position_ms() - position_ms).abs() <= 1_500 {
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        Err(format!(
            "the pipeline did not reach {} ms after the seek",
            position_ms
        ))
    }

    pub fn has_ended(&self) -> bool {
        self.poll_bus();
        self.eos.load(Ordering::Relaxed)
    }

    /// Only a snapshot sets this: a preview pipeline has no bus, so the stall is declared
    /// directly. It writes the same cell the bus would, so the OSD reads one value.
    #[cfg(any(test, debug_assertions))]
    pub fn preview_buffering(&mut self, percent: u32) {
        self.buffering.store(percent as u64, Ordering::Relaxed);
    }

    /// The source is below 100 % buffered; `None` means it is playing normally. Polls the bus,
    /// so the OSD's per-frame call also keeps buffering messages from piling up.
    pub fn buffering(&self) -> Option<u32> {
        self.poll_bus();
        match self.buffering.load(Ordering::Relaxed) {
            percent if percent < 100 => Some(percent as u32),
            _ => None,
        }
    }

    pub fn error(&self) -> Option<String> {
        self.poll_bus();
        self.error.lock().ok().and_then(|slot| slot.clone())
    }

    fn poll_bus(&self) {
        while let Some(message) = self.bus.timed_pop_filtered(
            gst::ClockTime::ZERO,
            &[
                gst::MessageType::Error,
                gst::MessageType::Eos,
                gst::MessageType::Buffering,
            ],
        ) {
            match message.view() {
                gst::MessageView::Error(error) => {
                    if let Ok(mut slot) = self.error.lock() {
                        *slot = Some(error.error().to_string());
                    }
                }
                gst::MessageView::Eos(_) => self.eos.store(true, Ordering::Relaxed),
                gst::MessageView::Buffering(buffering) => {
                    self.buffering
                        .store(buffering.percent() as u64, Ordering::Relaxed);
                }
                _ => {}
            }
        }
    }

    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
        // Wait for the streaming threads to leave PLAYING before the process can drop the
        // sink and surface; a bare asynchronous Null can race a flush seek at shutdown.
        let _ = self.pipeline.set_state(gst::State::Null);
        let _ = self.pipeline.state(gst::ClockTime::from_seconds(2));
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        let _ = self.pipeline.set_state(gst::State::Null);
        let _ = self.pipeline.state(gst::ClockTime::from_seconds(2));
        // The sink's streaming thread can still be unwinding a flush seek when the state
        // change returns. Give it a beat before the pipeline and surface are dropped.
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// The sink itself decides whether the pipeline can use zero-copy; if DMA-BUF fails, the caller
/// retries this function with the CPU NV12 path.
fn start_pipeline(
    url: &str,
    surface: NativeSurface,
    prefer_dmabuf: bool,
) -> Result<Player, String> {
    let playbin = gst::ElementFactory::make("playbin")
        .build()
        .or_else(|_| gst::ElementFactory::make("playbin3").build())
        .map_err(|error| format!("GStreamer has no playbin: {error}"))?;
    crate::bench::mark("bench.player.playbin");
    // The audio chain: pitch correction, a timestamp offset for A/V delay, an optional
    // compressor, and an optional downmix. Built as one bin so playbin sees a single filter.
    let (audio_filter, downmix, delay, dynamic) =
        build_audio_filter(playbin.find_property("audio-filter").is_some());
    if let Some(filter) = &audio_filter {
        playbin.set_property("audio-filter", filter);
    }
    let convert = if prefer_dmabuf {
        gst::ElementFactory::make("vapostproc")
            .build()
            .map_err(|error| format!("GStreamer has no VA-API converter: {error}"))?
    } else {
        // The CPU fallback must not keep asking VA-API to convert: a file the hardware decoder
        // rejects would otherwise fail both attempts for the same reason.
        gst::ElementFactory::make("videoconvert")
            .build()
            .map_err(|error| format!("GStreamer has no software video converter: {error}"))?
    };
    let frame_id = Arc::new(AtomicU64::new(1));
    let buffering = Arc::new(AtomicU64::new(100));
    let eos = Arc::new(AtomicBool::new(false));
    let error = Arc::new(Mutex::new(None));
    let dmabuf_kind = Arc::new(Mutex::new(None));
    let trace = Arc::new(Trace::default());
    let stopped = Arc::new(AtomicBool::new(false));
    let colour: Arc<Mutex<Option<ColourFacts>>> = Arc::new(Mutex::new(None));
    crate::bench::mark("bench.player.converter");
    let sink: gst::Element = if prefer_dmabuf {
        let caps = dmabuf_caps(&convert)
            .ok_or_else(|| "VA-API exposes no AR24 DMA-BUF modifier".to_string())?;
        let filter = gst::ElementFactory::make("capsfilter")
            .property("caps", &caps)
            .build()
            .map_err(|error| format!("GStreamer capsfilter: {error}"))?;
        let dmabuf = crate::dmabuf_sink::DmabufSink::new(
            surface.clone(),
            Arc::clone(&eos),
            Arc::clone(&error),
            Arc::clone(&frame_id),
            Arc::clone(&dmabuf_kind),
            Arc::clone(&trace),
            Arc::clone(&colour),
        );
        let sink = dmabuf.upcast::<gst::Element>();
        // A late frame is dropped rather than drawn immediately: the default (-1) renders the
        // whole backlog as fast as possible after any hiccup, which is the "freeze, then race"
        // report. 20 ms is under one frame at 30 fps, so healthy pacing is untouched.
        sink.set_property("sync", true);
        sink.set_property("qos", true);
        sink.set_property("max-lateness", 20_000_000i64);
        let bin = gst::Bin::new();
        bin.add_many([&convert, &filter, &sink])
            .map_err(|error| format!("GStreamer video sink: {error}"))?;
        gst::Element::link_many([&convert, &filter, &sink])
            .map_err(|error| format!("GStreamer video sink: {error}"))?;
        let pad = convert
            .static_pad("sink")
            .ok_or_else(|| "GStreamer converter has no sink pad".to_string())?;
        let ghost = gst::GhostPad::with_target(&pad)
            .map_err(|error| format!("GStreamer video sink pad: {error}"))?;
        bin.add_pad(&ghost)
            .map_err(|error| format!("GStreamer video sink pad: {error}"))?;
        bin.upcast::<gst::Element>()
    } else {
        let caps = gst::Caps::builder("video/x-raw")
            .field("format", "NV12")
            .build();
        let filter = gst::ElementFactory::make("capsfilter")
            .property("caps", &caps)
            .build()
            .map_err(|error| format!("GStreamer capsfilter: {error}"))?;
        let sink = AppSink::builder().max_buffers(3).sync(true).build();
        // The CPU path is slower to fill, so allow a little more slack before dropping.
        sink.set_property("qos", true);
        sink.set_property("max-lateness", 40_000_000i64);
        set_cpu_callbacks(
            &sink,
            surface.clone(),
            Arc::clone(&eos),
            Arc::clone(&error),
            Arc::clone(&frame_id),
            Arc::clone(&trace),
            Arc::clone(&colour),
        );
        let bin = gst::Bin::new();
        bin.add_many([&convert, &filter, sink.upcast_ref::<gst::Element>()])
            .map_err(|error| format!("GStreamer video sink: {error}"))?;
        gst::Element::link_many([&convert, &filter, sink.upcast_ref::<gst::Element>()])
            .map_err(|error| format!("GStreamer video sink: {error}"))?;
        let pad = convert
            .static_pad("sink")
            .ok_or_else(|| "GStreamer converter has no sink pad".to_string())?;
        let ghost = gst::GhostPad::with_target(&pad)
            .map_err(|error| format!("GStreamer video sink pad: {error}"))?;
        bin.add_pad(&ghost)
            .map_err(|error| format!("GStreamer video sink pad: {error}"))?;
        bin.upcast::<gst::Element>()
    };
    crate::bench::mark("bench.player.sink");
    playbin.set_property("video-sink", &sink);
    // The reverse proxy in front of the live server rejects GStreamer's stock user agent.
    // This is also the identity the Jellyfin API client uses.
    if playbin.find_property("user-agent").is_some() {
        playbin.set_property(
            "user-agent",
            format!("Runefin/{}", env!("CARGO_PKG_VERSION")),
        );
    }
    // Subtitles are drawn by Rune from the server's SRT; the custom sink has no textoverlay.
    // Disable playbin's TEXT flag as well as selecting no text: the flag is what stops the
    // demuxer from auto-plugging an ASS/SSA renderer, which otherwise keeps a vqueue thread
    // alive and can crash inside libgstassrender during a flush seek.
    if playbin.find_property("current-text").is_some() {
        playbin.set_property("current-text", -1i32);
    }
    if playbin.find_property("flags").is_some() {
        // `flags` is a GstPlayFlags, not a plain integer; a string setter is the portable way
        // to clear TEXT without depending on the generated flags type.
        playbin.set_property_from_str(
            "flags",
            "soft-colorbalance+deinterlace+soft-volume+audio+video",
        );
    }
    playbin.set_property("uri", url);
    playbin
        .set_state(gst::State::Playing)
        .map_err(|error| format!("GStreamer couldn't start playback: {error}"))?;
    crate::bench::mark("bench.player.playing");
    let pipeline = playbin
        .downcast::<gst::Pipeline>()
        .map_err(|_| "GStreamer playbin is not a pipeline".to_string())?;
    let bus = pipeline
        .bus()
        .ok_or_else(|| "GStreamer pipeline has no bus".to_string())?;
    let (_, current, pending) = pipeline.state(Some(gst::ClockTime::from_seconds(5)));
    if current != gst::State::Playing && pending != gst::State::Playing {
        let detail = bus
            .timed_pop_filtered(gst::ClockTime::ZERO, &[gst::MessageType::Error])
            .map(|message| match message.view() {
                gst::MessageView::Error(error) => error.error().to_string(),
                _ => "no decoder or source".to_string(),
            })
            .unwrap_or_else(|| "no decoder or source".to_string());
        let _ = pipeline.set_state(gst::State::Null);
        return Err(format!("GStreamer couldn't start: {detail}"));
    }
    if std::env::var("RUNEFIN_DEBUG_DOT").is_ok() {
        pipeline.debug_to_dot_file(gst::DebugGraphDetails::ALL, "runefin");
    }
    Ok(Player {
        pipeline,
        bus,
        surface,
        eos,
        error,
        frames: Arc::clone(&frame_id),
        buffering,
        rate: AtomicU64::new(1.0f64.to_bits()),
        dmabuf: prefer_dmabuf,
        dmabuf_kind,
        paused: std::sync::atomic::AtomicBool::new(false),
        preview_position: None,
        downmix,
        delay,
        dynamic,
        trace: Arc::clone(&trace),
        stopped: Arc::clone(&stopped),
        colour: Arc::clone(&colour),
    })
}

/// Build the audio-filter bin. Every piece is optional: a slim GStreamer install still plays,
/// it just loses the matching control.
fn build_audio_filter(
    supported: bool,
) -> (
    Option<gst::Element>,
    Option<gst::Element>,
    Option<gst::Element>,
    Option<gst::Element>,
) {
    if !supported {
        return (None, None, None, None);
    }
    let Ok(convert) = gst::ElementFactory::make("audioconvert").build() else {
        return (None, None, None, None);
    };
    let Ok(delay) = gst::ElementFactory::make("identity").build() else {
        return (None, None, None, None);
    };
    let Ok(filter) = gst::ElementFactory::make("capsfilter").build() else {
        return (None, None, None, None);
    };
    filter.set_property("caps", gst::Caps::builder("audio/x-raw").build());
    let dynamic = gst::ElementFactory::make("audiodynamic").build().ok();
    let tempo = gst::ElementFactory::make("scaletempo").build().ok();
    delay.set_property("ts-offset", 0i64);
    if let Some(dynamic) = &dynamic {
        dynamic.set_property("threshold", 0.0f32);
        dynamic.set_property("ratio", 1.0f32);
    }

    let bin = gst::Bin::new();
    let mut chain: Vec<gst::Element> = Vec::new();
    if let Some(tempo) = &tempo {
        chain.push(tempo.clone());
    }
    chain.push(convert.clone());
    chain.push(delay.clone());
    if let Some(dynamic) = &dynamic {
        chain.push(dynamic.clone());
    }
    chain.push(filter.clone());
    bin.add_many(chain.iter())
        .expect("the audio filter's elements are new");
    gst::Element::link_many(chain.iter()).expect("the audio filter links by itself");
    let sink_pad = chain
        .first()
        .and_then(|element| element.static_pad("sink"))
        .expect("an audio filter has a sink pad");
    let src_pad = chain
        .last()
        .and_then(|element| element.static_pad("src"))
        .expect("an audio filter has a src pad");
    bin.add_pad(&gst::GhostPad::with_target(&sink_pad).expect("a ghost sink pad"))
        .expect("the audio bin takes its sink pad");
    bin.add_pad(&gst::GhostPad::with_target(&src_pad).expect("a ghost src pad"))
        .expect("the audio bin takes its src pad");
    (
        Some(bin.upcast::<gst::Element>()),
        Some(filter),
        Some(delay),
        dynamic,
    )
}

/// Ask `vapostproc` for one packed AR24 modifier it advertises. The exact modifier is the
/// producer's, not ours; hard-coding the Mesa value would break on another driver.
fn dmabuf_caps(convert: &gst::Element) -> Option<gst::Caps> {
    let template = convert.static_pad("src")?.pad_template()?;
    let caps = template.caps();
    for wanted in ["P010:", "NV12:", "AR24:", "XR24:"] {
        for structure in caps.iter() {
            let Ok(formats) = structure.get::<gst::List>("drm-format") else {
                continue;
            };
            for value in formats.iter() {
                let Ok(format) = value.get::<&str>() else {
                    continue;
                };
                if format.starts_with(wanted) {
                    return Some(
                        gst::Caps::builder("video/x-raw")
                            .features(["memory:DMABuf"])
                            .field("format", "DMA_DRM")
                            .field("drm-format", format)
                            .build(),
                    );
                }
            }
        }
    }
    None
}

fn set_cpu_callbacks(
    sink: &AppSink,
    surface: NativeSurface,
    eos: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    frame_id: Arc<AtomicU64>,
    trace: Arc<Trace>,
    colour: Arc<Mutex<Option<ColourFacts>>>,
) {
    sink.set_callbacks(
        gstreamer_app::AppSinkCallbacks::builder()
            .new_sample(move |sink| {
                let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                let caps = sample.caps().ok_or(gst::FlowError::Error)?;
                let info = VideoInfo::from_caps(caps).map_err(|_| gst::FlowError::Error)?;
                let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;
                if info.format() != gstreamer_video::VideoFormat::Nv12 {
                    return Err(gst::FlowError::Error);
                }
                let (width, height) = (info.width() as usize, info.height() as usize);
                if width % 2 != 0 || height % 2 != 0 {
                    return Err(gst::FlowError::Error);
                }
                let y_stride = info.stride()[0] as usize;
                let uv_stride = info.stride()[1] as usize;
                let y_offset = info.offset()[0];
                let uv_offset = info.offset()[1];
                let mut y_plane = Vec::with_capacity(width * height);
                for row in 0..height {
                    let start = y_offset + row * y_stride;
                    let Some(line) = map.get(start..start + width) else {
                        return Err(gst::FlowError::Error);
                    };
                    y_plane.extend_from_slice(line);
                }
                let mut uv_plane = Vec::with_capacity(width * height / 2);
                for row in 0..height / 2 {
                    let start = uv_offset + row * uv_stride;
                    let Some(line) = map.get(start..start + width) else {
                        return Err(gst::FlowError::Error);
                    };
                    uv_plane.extend_from_slice(line);
                }
                if let Ok(mut slot) = colour.lock()
                    && slot.is_none()
                {
                    let mastering =
                        gstreamer_video::VideoMasteringDisplayInfo::from_caps(caps).ok();
                    let light = gstreamer_video::VideoContentLightLevel::from_caps(caps).ok();
                    *slot = Some(ColourFacts {
                        transfer: match info.colorimetry().transfer() {
                            gstreamer_video::VideoTransferFunction::Smpte2084 => "PQ",
                            gstreamer_video::VideoTransferFunction::AribStdB67 => "HLG",
                            _ => "BT.709",
                        },
                        matrix: match info.colorimetry().matrix() {
                            gstreamer_video::VideoColorMatrix::Bt2020 => "BT.2020",
                            gstreamer_video::VideoColorMatrix::Bt601 => "BT.601",
                            _ => "BT.709",
                        },
                        max_luminance: mastering.as_ref().map_or(0.0, |m| {
                            m.max_display_mastering_luminance() as f32 / 10_000.0
                        }),
                        min_luminance: mastering.as_ref().map_or(0.0, |m| {
                            m.min_display_mastering_luminance() as f32 / 10_000.0
                        }),
                        max_cll: light.as_ref().map_or(0, |l| l.max_content_light_level()),
                        max_fall: light
                            .as_ref()
                            .map_or(0, |l| l.max_frame_average_light_level()),
                    });
                }
                let colorimetry = info.colorimetry();
                let color = ColorInfo {
                    matrix: match colorimetry.matrix() {
                        gstreamer_video::VideoColorMatrix::Bt601 => Matrix::Bt601,
                        _ => Matrix::Bt709,
                    },
                    transfer: match colorimetry.transfer() {
                        gstreamer_video::VideoTransferFunction::Srgb => Transfer::Srgb,
                        _ => Transfer::Bt709,
                    },
                };
                let format = if matches!(
                    colorimetry.range(),
                    gstreamer_video::VideoColorRange::Range0_255
                ) {
                    Format::Nv12Full
                } else {
                    Format::Nv12Video
                };
                let frame = ByteFrame {
                    width: width as u32,
                    height: height as u32,
                    format,
                    color,
                    planes: [y_plane, uv_plane],
                };
                trace.tick();
                if let Err(message) =
                    surface.push_bytes(frame, frame_id.fetch_add(1, Ordering::Relaxed))
                {
                    if let Ok(mut slot) = error.lock() {
                        *slot = Some(message);
                    }
                    return Err(gst::FlowError::Error);
                }
                Ok(gst::FlowSuccess::Ok)
            })
            .eos(move |_| eos.store(true, Ordering::Relaxed))
            .build(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "needs a local media file; measures frame pacing for ten seconds"]
    fn a_local_file_holds_its_frame_pacing() {
        let media = std::env::var("RUNEFIN_TEST_MEDIA")
            .unwrap_or_else(|_| "/tmp/opencode/pace-1080p.mp4".into());
        let player = Player::start(&format!("file://{media}"), NativeSurface::new(), None)
            .expect("the pacing clip plays");
        std::thread::sleep(Duration::from_secs(10));
        let pacing = player.pacing();
        eprintln!(
            "pacing: {} frames, {} stalls, {} bursts, longest gap {} ms, path {}",
            pacing.frames,
            pacing.stalls,
            pacing.bursts,
            pacing.max_gap_ms,
            player.path_label()
        );
        assert!(pacing.frames > 60, "the sink rendered frames: {pacing:?}");
        player.stop();
    }

    #[test]
    #[ignore = "needs a generated local clip and a GStreamer runtime"]
    fn a_local_file_advances_the_clock() {
        let media = std::env::var("RUNEFIN_TEST_MEDIA")
            .unwrap_or_else(|_| "/tmp/opencode/runefin-test.ts".into());
        let player = Player::start(&format!("file://{media}"), NativeSurface::new(), None)
            .expect("the test clip plays");
        std::thread::sleep(std::time::Duration::from_secs(1));
        assert!(
            player.position_ms() > 0,
            "GStreamer did not advance the clock"
        );
        assert_eq!(
            player.surface().last_error(),
            None,
            "the NV12 frame failed surface validation"
        );
        player.seek_ms(2_000);
        std::thread::sleep(std::time::Duration::from_millis(500));
        assert!(
            player.position_ms() >= 1_500,
            "seek did not move the clock: {}",
            player.position_ms()
        );
        player.stop();
    }
}
