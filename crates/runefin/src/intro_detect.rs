//! Finding a series' intro: the span two episodes have in common at the head of each.
//!
//! This is the Intro Skipper plugin's job, done on the client from the audio it can decode. An
//! intro is the same music and title sequence at about the same offset in every episode, so two
//! episodes of a series, compared over their first minutes, agree there and nowhere else.
//!
//! The features are the subtitle sync's: a band-limited envelope (300–3400 Hz) and its modulation
//! over a second, so a score under dialogue does not read as speech or the reverse. The comparison
//! is a cross-correlation for the offset, then a run of frames along that offset whose local
//! correlation stays high — the run is the intro, and its length and strength are the confidence.
//!
//! No I/O: [`detect`] takes two decoded windows. [`crate::audio_probe`] supplies them.
#![allow(dead_code)]
//!
//! What it cannot do: a show with no shared intro (a cold open every week), an intro whose length
//! or music changes between episodes, and a series where the shared span is the *credits* — the
//! caller has to decide whether a span near the start is plausible before offering it.
//!
//! **Not offered in the app yet.** The synthetic tests pass, but against two real episodes of a
//! series on this server it finds no span: level envelopes cannot tell a title sequence from the
//! music under a scene when the two episodes' mixes differ. The next step is a spectral
//! fingerprint (a short FFT per frame, hashed), not a better envelope. Kept here — with its tests
//! and its live check — as the base for that, and deliberately not wired to a button that would
//! usually say "no intro found".

/// Analysis frame for the comparison. 200 ms is plenty for a title sequence and keeps six minutes
/// of two episodes at under 2,000 frames each.
pub const FRAME_MS: i64 = 200;
/// How far the two episodes' intros may sit apart, either way.
const MAX_SHIFT_MS: i64 = 25_000;
/// A run shorter than this is a coincidence, not a title sequence.
const MIN_RUN_MS: i64 = 10_000;
/// How well the frames inside a run must agree, frame by frame, to count.
const MIN_FRAME_CORRELATION: f32 = 0.5;
/// The whole span's mean correlation must clear this as well.
const MIN_CONFIDENCE: f32 = 0.55;

/// A span two episodes share, in the item time of the episode it was measured on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Intro {
    pub start_ms: i64,
    pub end_ms: i64,
    /// Mean frame correlation across the span, 0–1.
    pub confidence: f32,
}

/// Band-limited envelope of a window, one value per [`FRAME_MS`], modulated over a second and
/// normalised to zero mean and unit spread, so a loud episode and a quiet one compare.
pub(crate) fn features(samples: &[i16], rate: u32) -> Vec<f32> {
    if samples.is_empty() || rate == 0 {
        return Vec::new();
    }
    // 300–3400 Hz, as in the subtitle sync: a score is mostly below 300 Hz and would dominate.
    let mut high = Biquad::high_pass(rate, 300.0, 0.707);
    let mut low = Biquad::low_pass(rate, 3_400.0, 0.707);
    let filtered: Vec<f32> = samples
        .iter()
        .map(|s| low.run(high.run(f32::from(*s))))
        .collect();
    let per_frame = (rate as i64 * FRAME_MS / 1000).max(1) as usize;
    let levels: Vec<f32> = filtered
        .chunks_exact(per_frame)
        .map(|frame| {
            let energy: f32 = frame.iter().map(|s| s * s).sum::<f32>() / frame.len() as f32;
            (energy + 1e-9).sqrt().log10()
        })
        .collect();
    if levels.len() < 8 {
        return Vec::new();
    }
    // The envelope itself, not its modulation: an intro is music, and its shape over a second or
    // two is what two episodes have in common. (The subtitle sync subtracts a local mean because
    // there it is speech against a score; here the score is the signal.)
    let mean = levels.iter().sum::<f32>() / levels.len() as f32;
    let spread =
        (levels.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / levels.len() as f32).sqrt();
    if spread <= 1e-6 {
        return Vec::new();
    }
    // Clamped to ±3σ: a frame of digital silence is a dozen standard deviations below the mean,
    // and two aligned silences would otherwise dominate the product with a "correlation" of 5+.
    levels
        .iter()
        .map(|v| ((v - mean) / spread).clamp(-3.0, 3.0))
        .collect()
}

/// The best local agreement of two feature tracks at a shift: the rolling mean of their products
/// over a second, at its highest point. The mean over the *whole* window would drown a 30 s title
/// sequence in five minutes of different scenes, so the shift is chosen by where the two agree
/// most, not on average. `shift` is in frames: `a[t]` is compared with `b[t - shift]`.
fn best_local(a: &[f32], b: &[f32], shift: i64) -> f32 {
    let window = (1_000 / FRAME_MS) as usize; // a second
    let n = a.len() as i64;
    let from = shift.max(0);
    let to = (n + shift.min(0)).min(n);
    if to - from < window as i64 {
        return 0.0;
    }
    let mut sum = 0.0f32;
    let mut best = f32::MIN;
    let mut ring = std::collections::VecDeque::with_capacity(window);
    for t in from..to {
        let product = a[t as usize] * b[(t - shift) as usize];
        sum += product;
        ring.push_back(product);
        if ring.len() > window {
            sum -= ring.pop_front().unwrap_or(0.0);
        }
        if ring.len() == window {
            best = best.max(sum / window as f32);
        }
    }
    best.max(0.0)
}

/// Compare two decoded windows and return the intro they share, in `a`'s item time.
///
/// `a` and `b` start at `a_start_ms` and `_b_start_ms` of their episodes. Both should cover the
/// first several minutes; anything after the intros is different content and only adds noise.
pub fn detect(a: &[i16], a_start_ms: i64, b: &[i16], _b_start_ms: i64, rate: u32) -> Option<Intro> {
    let (fa, fb) = (features(a, rate), features(b, rate));
    if fa.len() < (MIN_RUN_MS / FRAME_MS) as usize || fb.len() < (MIN_RUN_MS / FRAME_MS) as usize {
        return None;
    }
    // The shift and the span are chosen together: for every offset the episodes could have, the
    // longest stretch where they keep agreeing. A single second of agreement is a coincidence —
    // repeated music, a sting — so length, not peak, is what wins.
    let (frames, start, length, confidence) = best_match(&fa, &fb)?;
    if length * FRAME_MS < MIN_RUN_MS || confidence < MIN_CONFIDENCE {
        return None;
    }
    // `start` indexes A's frames, so the span already sits where A has it.
    let start_ms = a_start_ms + start * FRAME_MS;
    let end_ms = start_ms + length * FRAME_MS;
    // The intro must sit at the head of the episode; a shared ending or a repeated scene is not
    // one, and a span that starts after five minutes is not a title sequence.
    if start_ms - a_start_ms > 5 * 60_000 {
        return None;
    }
    let _ = frames;
    Some(Intro {
        start_ms,
        end_ms,
        confidence,
    })
}

/// The best (shift in frames, start frame, length in frames, mean correlation) two feature tracks
/// offer, before the confidence gates. Exposed for the live check, which wants the candidate even
/// when the app would refuse to use it.
pub(crate) fn best_match(fa: &[f32], fb: &[f32]) -> Option<(i64, i64, i64, f32)> {
    let mut best: Option<(i64, i64, i64, f32)> = None;
    for shift in (-MAX_SHIFT_MS..=MAX_SHIFT_MS).step_by(FRAME_MS as usize) {
        let frames = shift / FRAME_MS;
        let Some((start, length, mean)) = longest_run(fa, fb, frames) else {
            continue;
        };
        if best.is_none_or(|(_, _, best_length, _)| length > best_length) {
            best = Some((frames, start, length, mean));
        }
    }
    best
}

/// The longest stretch at `shift` where the rolling one-second agreement stays at or above
/// [`MIN_FRAME_CORRELATION`]: start frame, length in frames, and the mean product inside.
fn longest_run(a: &[f32], b: &[f32], shift: i64) -> Option<(i64, i64, f32)> {
    let window = (1_000 / FRAME_MS) as usize;
    let n = a.len() as i64;
    let from = shift.max(0);
    let to = (n + shift.min(0)).min(n);
    if to - from < window as i64 {
        return None;
    }
    let mut sum = 0.0f32;
    let mut ring = std::collections::VecDeque::with_capacity(window);
    let mut run_start: Option<i64> = None;
    let mut best: Option<(i64, i64)> = None;
    for t in from..to {
        let product = a[t as usize] * b[(t - shift) as usize];
        sum += product;
        ring.push_back(product);
        if ring.len() > window {
            sum -= ring.pop_front().unwrap_or(0.0);
        }
        let mean = sum / ring.len() as f32;
        // Both episodes must actually have sound here: a stretch where both are below their own
        // average is silence agreeing with silence, not a title sequence.
        let audible = a[t as usize] > -0.8 && b[(t - shift) as usize] > -0.8;
        if ring.len() == window && mean >= MIN_FRAME_CORRELATION && audible {
            run_start.get_or_insert(t);
        } else if let Some(start) = run_start.take() {
            let length = t - start;
            if best.is_none_or(|(_, best_length)| length > best_length) {
                best = Some((start, length));
            }
        }
    }
    if let Some(start) = run_start {
        let length = to - start;
        if best.is_none_or(|(_, best_length)| length > best_length) {
            best = Some((start, length));
        }
    }
    let (start, length) = best?;
    let mut inside = 0.0f64;
    for u in start..(start + length) {
        inside += f64::from(a[u as usize]) * f64::from(b[(u - shift) as usize]);
    }
    Some((start, length, (inside / length as f64) as f32))
}

/// A biquad section, as in the subtitle sync.
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    fn high_pass(rate: u32, freq: f32, q: f32) -> Self {
        let (cos, alpha) = Self::terms(rate, freq, q);
        let a0 = 1.0 + alpha;
        Self {
            b0: (1.0 + cos) / 2.0 / a0,
            b1: -(1.0 + cos) / a0,
            b2: (1.0 + cos) / 2.0 / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha) / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn low_pass(rate: u32, freq: f32, q: f32) -> Self {
        let (cos, alpha) = Self::terms(rate, freq, q);
        let a0 = 1.0 + alpha;
        Self {
            b0: (1.0 - cos) / 2.0 / a0,
            b1: (1.0 - cos) / a0,
            b2: (1.0 - cos) / 2.0 / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha) / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn terms(rate: u32, freq: f32, q: f32) -> (f32, f32) {
        let w0 = std::f32::consts::TAU * freq / rate as f32;
        (w0.cos(), w0.sin() / (2.0 * q))
    }

    fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 8_000;

    /// A title sequence: eight notes on a fixed beat, so its level *changes* the way music does.
    /// A steady tone would be invisible to the modulation feature, which is what the matcher
    /// looks at.
    fn intro_audio(seconds: u32, _seed: f32) -> Vec<i16> {
        let notes = [330.0, 392.0, 440.0, 392.0, 494.0, 440.0, 392.0, 330.0];
        let rate = RATE;
        (0..rate * seconds)
            .map(|at| {
                let t = at as f32 / rate as f32;
                let beat = t / 0.75;
                let freq = notes[beat as usize % notes.len()];
                let gate = if beat.fract() < 0.7 { 1.0 } else { 0.1 };
                let melody = (t * freq * std::f32::consts::TAU).sin()
                    + 0.5 * (t * freq * 2.0 * std::f32::consts::TAU).sin();
                (melody * gate * 6_000.0) as i16
            })
            .collect()
    }

    /// Different content: bursts of "dialogue" at a pitch and a rhythm unique to the scene. The
    /// rhythm matters more than the pitch: the features compare timing, so two scenes built from
    /// the same syllable pattern would correlate as well as a shared intro does.
    fn scene_audio(seconds: u32, pitch: f32, period_ms: u32, duty: f32) -> Vec<i16> {
        let rate = RATE;
        (0..rate * seconds)
            .map(|at| {
                let t = at as f32 / rate as f32;
                let ms = (at as f32 / rate as f32 * 1000.0) as u32;
                if ((ms % period_ms) as f32) >= duty * period_ms as f32 {
                    return 0;
                }
                let voice = (t * pitch * std::f32::consts::TAU).sin()
                    + 0.4 * (t * pitch * 3.0 * std::f32::consts::TAU).sin();
                (voice * 4_000.0) as i16
            })
            .collect()
    }

    /// One episode: a scene, the shared title sequence, then a different scene.
    fn episode(intro_at: u32, scene_pitch: f32, period: u32, duty: f32, seconds: u32) -> Vec<i16> {
        let mut samples = scene_audio(intro_at.max(1), scene_pitch, period, duty);
        samples.extend(intro_audio(30, 0.0));
        samples.extend(scene_audio(
            seconds - intro_at - 30,
            scene_pitch + 40.0,
            period + 300,
            0.3,
        ));
        samples
    }

    #[test]
    fn a_shared_title_sequence_is_found_where_the_first_episode_has_it() {
        let a = episode(40, 220.0, 700, 0.6, 300);
        let b = episode(45, 260.0, 1_100, 0.35, 300);
        let found = detect(&a, 0, &b, 0, RATE).expect("the shared intro");
        assert!(
            (found.start_ms - 40_000).abs() < 1_000,
            "start {}",
            found.start_ms
        );
        assert!(
            (found.end_ms - 70_000).abs() < 2_500,
            "end {}",
            found.end_ms
        );
        assert!(found.confidence > 0.6, "confidence {}", found.confidence);
    }

    #[test]
    fn unrelated_windows_share_no_intro() {
        let a = scene_audio(300, 220.0, 700, 0.6);
        let b = scene_audio(300, 300.0, 1_100, 0.35);
        assert!(detect(&a, 0, &b, 0, RATE).is_none());
    }

    #[test]
    fn a_title_sequence_at_a_different_offset_still_lines_up() {
        // Episode B starts on a cold open ten seconds longer; the shared span is still found in A.
        let a = episode(40, 220.0, 700, 0.6, 300);
        let mut b = scene_audio(50, 260.0, 1_100, 0.35);
        b.extend(intro_audio(30, 0.0));
        b.extend(scene_audio(220, 300.0, 900, 0.4));
        let found = detect(&a, 0, &b, 0, RATE).expect("the shared intro");
        assert!(
            (found.start_ms - 40_000).abs() < 1_500,
            "start {}",
            found.start_ms
        );
    }

    #[test]
    fn a_span_too_short_to_be_an_intro_is_refused() {
        let mut a = scene_audio(300, 220.0, 700, 0.6);
        let mut b = scene_audio(300, 260.0, 1_100, 0.35);
        // Four seconds of shared audio: a sting, not a title sequence.
        let short = intro_audio(4, 0.0);
        a.splice(60_000..60_000 + short.len(), short.clone());
        b.splice(60_000..60_000 + short.len(), short);
        assert!(detect(&a, 0, &b, 0, RATE).is_none());
    }

    #[test]
    fn a_window_with_nothing_but_silence_has_no_answer() {
        let silent = vec![0i16; RATE as usize * 120];
        let voiced = intro_audio(120, 0.0);
        assert!(detect(&silent, 0, &voiced, 0, RATE).is_none());
    }
}
