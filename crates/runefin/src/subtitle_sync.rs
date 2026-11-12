//! Aligning a subtitle track to a film's audio, the client-side half of what Bazarr does on the
//! server: build a voice-activity signal from a window of the file's audio, build a speech mask
//! from the cues, and find the shift that lines them up.
//!
//! No I/O here. [`align`] takes the decoded samples and the cue times and returns the offset a
//! reader would type into the delay control, or `None` when the window has nothing to align
//! against. The audio decode lives in [`crate::audio_probe`].
//!
//! The estimator is a constant offset only. A track that drifts (a 23.976/25 mismatch) needs a
//! rate fit over several windows, and claiming one without a fixture proving it would be worse
//! than leaving the delay where the reader put it.

/// Analysis frame: the plan's 20 ms grid, fine enough for a cue that starts on a cut and coarse
/// enough that a ten-minute window is 30,000 values.
pub const FRAME_MS: i64 = 20;

/// Where the cues and the audio line up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Alignment {
    /// How much later the cues should play, in milliseconds: add this to the reader's delay.
    pub lag_ms: i64,
    /// Pearson correlation at the peak, 0–1. Higher is a cleaner match.
    pub confidence: f32,
    /// The correlation one second to either side of the peak, for a report and for the tests.
    pub contrast: f32,
}

/// Lags searched either way. A subtitle can be off by more than a minute on a badly cut release,
/// but the delay control stops at ±30 s, so there is nothing to apply beyond that.
const MAX_LAG_MS: i64 = 30_000;
/// Coarse grid, then the fine grid around its peak.
const COARSE_MS: i64 = 100;
const FINE_MS: i64 = FRAME_MS;
/// Cue dilation either side: cue times are the text's, not the speech's, and trailing spaces or a
/// lead-in of a beat would otherwise pull the correlation off. Wide dilation fills the pauses
/// between lines and lets the peak drift, so it stays close to one typical cue padding.
const DILATE_MS: i64 = 40;
/// The overlap a lag must have with the window before its score means anything: half.
const MIN_OVERLAP: f32 = 0.5;
/// Below this correlation the answer is a guess. A wrong offset is worse than none. A track the
/// server considers in sync measured 0.216 on a real episode; a window of titles and music
/// measured 0.07 at its best, so this sits between them.
const MIN_CONFIDENCE: f32 = 0.2;
/// And it must stand clear of its neighbours: the same real match fell from 0.216 to 0.016 a
/// second either side, where a music-driven peak was a broad 0.23 against 0.19 nearby.
const MIN_CONTRAST: f32 = 0.05;

/// A second-order high-pass then low-pass (RBJ's cookbook), so the envelope follows the speech
/// band instead of the music under it. A film's score sits mostly below 300 Hz and is loud enough
/// to hide every line in a plain RMS: measured against one anime episode, band-limiting moved the
/// correlation peak from a spurious −27.8 s (music) to +0.1 s (the dialogue) on the same window.
fn speech_band(samples: &[i16], rate: u32) -> Vec<f32> {
    let mut high = Biquad::high_pass(rate, 300.0, 0.707);
    let mut low = Biquad::low_pass(rate, 3_400.0, 0.707);
    samples
        .iter()
        .map(|s| low.run(high.run(f32::from(*s))))
        .collect()
}

/// One biquad section, direct form I. Coefficients follow RBJ's audio cookbook.
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

/// One analysis frame's audio level: log energy above the window's noise floor, in a 0–1 range.
/// Silence reads 0, a voice at the floor + 30 dB reads about 1.
fn envelope(samples: &[i16], sample_rate: u32) -> Vec<f32> {
    let per_frame = (sample_rate as i64 * FRAME_MS / 1000).max(1) as usize;
    if samples.len() < per_frame {
        return Vec::new();
    }
    let samples = speech_band(samples, sample_rate);
    let mut levels = Vec::with_capacity(samples.len() / per_frame);
    let mut energy = Vec::with_capacity(levels.len());
    for frame in samples.chunks_exact(per_frame) {
        let sum: f64 = frame.iter().map(|s| f64::from(*s) * f64::from(*s)).sum();
        energy.push((sum / frame.len() as f64).sqrt());
    }
    // The floor is the quietest tenth: speech is sparse enough that the bottom of a film's
    // window is room tone, dialogue pauses or silence, and music lifts the floor uniformly.
    let mut sorted = energy.clone();
    sorted.sort_by(f64::total_cmp);
    let floor = sorted[sorted.len() / 10].max(1.0);
    for level in &energy {
        // 20*log10(level/floor), clamped to 0–30 dB and scaled to 1.
        let db = 20.0 * (level / floor).log10();
        levels.push(((db + 3.0) / 30.0).clamp(0.0, 1.0) as f32);
    }
    levels
}

/// The envelope's own modulation: how far each frame sits above or below the level around it,
/// over a second. Speech comes in syllables, so a voice modulates at 2–8 Hz even when it is quiet;
/// a score or a room tone does not. Measured on a real film, this turned a −0.5 s/−0.0 s spread of
/// peaks into a consistent −0.1…−0.3 s with sharper contrast.
fn modulation(envelope: &[f32]) -> Vec<f32> {
    let half = (1_000 / FRAME_MS) as usize / 2;
    let mut out = Vec::with_capacity(envelope.len());
    for (at, value) in envelope.iter().enumerate() {
        let from = at.saturating_sub(half);
        let to = (at + half + 1).min(envelope.len());
        let mean: f32 = envelope[from..to].iter().sum::<f32>() / (to - from) as f32;
        out.push((value - mean).abs());
    }
    out
}

/// Speech mask from the cues, one value per analysis frame, dilated by [`DILATE_MS`].
fn cue_mask(cues: &[(i64, i64)], start_ms: i64, frames: usize) -> Vec<f32> {
    let mut mask = vec![0.0f32; frames];
    let frame = FRAME_MS;
    let dilate = (DILATE_MS / frame) as usize;
    for &(from, to) in cues {
        let first = ((from - start_ms).max(0) / frame) as usize;
        let last = ((to - start_ms).max(0) / frame) as usize;
        let first = first.saturating_sub(dilate);
        let last = (last + dilate).min(frames.saturating_sub(1));
        if frames == 0 || first >= frames || first > last {
            continue;
        }
        for value in &mut mask[first..=last] {
            *value = 1.0;
        }
    }
    mask
}

/// Pearson correlation of the cue mask against the audio envelope, shifted by `lag_ms`.
///
/// `lag_ms` is how much later the cues should play: a positive lag compares audio frame `t` with
/// cue frame `t - lag`. Lags whose overlap is under [`MIN_OVERLAP`] score 0.
fn score(audio: &[f32], cues: &[f32], lag_ms: i64) -> f32 {
    let shift = lag_ms / FRAME_MS;
    let n = audio.len() as i64;
    let from = shift.max(0);
    let to = (n + shift.min(0)).min(n);
    let count = to - from;
    if count <= 0 || (count as f32) < MIN_OVERLAP * n as f32 {
        return 0.0;
    }
    let (mut sa, mut sc) = (0.0f64, 0.0f64);
    let (mut saa, mut scc, mut sac) = (0.0f64, 0.0f64, 0.0f64);
    for t in from..to {
        let a = f64::from(audio[t as usize]);
        let c = f64::from(cues[(t - shift) as usize]);
        sa += a;
        sc += c;
        saa += a * a;
        scc += c;
        sac += a * c;
    }
    let count = count as f64;
    let (ma, mc) = (sa / count, sc / count);
    let (va, vc) = (saa / count - ma * ma, scc / count - mc * mc);
    if va <= 1e-9 || vc <= 1e-9 {
        return 0.0;
    }
    ((sac / count - ma * mc) / (va.sqrt() * vc.sqrt())) as f32
}

/// Find the shift that best lines `cues` up with `samples`.
///
/// `samples` are mono 8-bit-safe PCM at `sample_rate`, starting at `start_ms` of the item; `cues`
/// are in item time. The result is the value to add to the reader's delay.
pub fn align(
    samples: &[i16],
    sample_rate: u32,
    start_ms: i64,
    cues: &[(i64, i64)],
) -> Option<Alignment> {
    let (found, mean) = search(samples, sample_rate, start_ms, cues)?;
    if found.confidence < MIN_CONFIDENCE
        || found.confidence < mean * 1.5
        || found.contrast < MIN_CONTRAST
    {
        return None;
    }
    Some(found)
}

/// [`align`] without the confidence gate: the best shift it found, and the mean score it beat.
/// For the ignored live test and for anyone tuning the thresholds — a report wants the peak even
/// when the app would refuse to apply it.
pub(crate) fn search(
    samples: &[i16],
    sample_rate: u32,
    start_ms: i64,
    cues: &[(i64, i64)],
) -> Option<(Alignment, f32)> {
    let audio = modulation(&envelope(samples, sample_rate));
    if audio.len() < 300 {
        // Under about six seconds there is nothing to correlate.
        return None;
    }
    let cues = cue_mask(cues, start_ms, audio.len());
    let covered = cues.iter().filter(|v| **v > 0.0).count();
    if covered < audio.len() / 100 {
        // A window with almost no cues (or none at all) cannot say anything about a shift.
        return None;
    }
    let mut best = (0.0f32, 0);
    let mut sum = 0.0f64;
    let mut evaluated = 0u32;
    let mut lag = -MAX_LAG_MS;
    while lag <= MAX_LAG_MS {
        let value = score(&audio, &cues, lag);
        sum += f64::from(value.max(0.0));
        evaluated += 1;
        if value > best.0 {
            best = (value, lag);
        }
        lag += COARSE_MS;
    }
    let mean = (sum / f64::from(evaluated)) as f32;
    // Refine on the fine grid around the coarse peak, so a 100 ms step never reaches the delay.
    let mut peak = best;
    for lag in (best.1 - COARSE_MS..=best.1 + COARSE_MS).step_by(FINE_MS as usize) {
        let value = score(&audio, &cues, lag);
        if value > peak.0 {
            peak = (value, lag);
        }
    }
    // One second either side: a real match falls off, a plate of noise does not.
    let contrast =
        peak.0 - score(&audio, &cues, peak.1 - 1_000).max(score(&audio, &cues, peak.1 + 1_000));
    Some((
        Alignment {
            lag_ms: peak.1,
            confidence: peak.0,
            contrast,
        },
        mean,
    ))
}

/// One confident window is an answer; two that disagree by more than a beat are not. A wrong
/// offset is worse than none, so disagreement reads as "no clear match".
pub fn settle(found: Vec<Alignment>) -> Option<Alignment> {
    let best = *found
        .iter()
        .max_by(|a, b| a.confidence.total_cmp(&b.confidence))?;
    if found
        .iter()
        .any(|other| (other.lag_ms - best.lag_ms).abs() > 800)
    {
        return None;
    }
    Some(best)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 8_000;

    /// Speech-like bursts: a 220 Hz tone with a slow amplitude wobble, so the envelope is not a
    /// flat line, in the given item-time intervals.
    fn talking(seconds: i64, bursts: &[(i64, i64)]) -> Vec<i16> {
        let rate = f64::from(RATE);
        let mut samples = vec![0i16; (seconds * RATE as i64) as usize];
        for &(from, to) in bursts {
            let first = ((from * RATE as i64 / 1000).max(0) as usize).min(samples.len());
            let last = ((to * RATE as i64 / 1000) as usize).min(samples.len());
            if first >= last {
                continue;
            }
            for (offset, sample) in samples[first..last].iter_mut().enumerate() {
                let t = (first + offset) as f64 / rate;
                // A voice, not a hum: a fundamental and two harmonics inside the speech band,
                // with a syllable-rate wobble, so band-limiting keeps it.
                let voice = (t * 330.0 * std::f64::consts::TAU).sin()
                    + 0.6 * (t * 990.0 * std::f64::consts::TAU).sin()
                    + 0.3 * (t * 1_980.0 * std::f64::consts::TAU).sin();
                let wobble = 1.0 + 0.35 * (t * 30.0).sin();
                *sample = (voice * wobble * 3_000.0) as i16;
            }
        }
        samples
    }

    /// Cue intervals for the same bursts, shifted by `shift_ms`.
    fn cues_for(bursts: &[(i64, i64)], shift_ms: i64) -> Vec<(i64, i64)> {
        bursts
            .iter()
            .map(|(from, to)| (from + shift_ms, to + shift_ms))
            .collect()
    }

    /// Sixty seconds of speech with irregular pauses (a comb of equal gaps would correlate just
    /// as well one second off, which is what the contrast measure exists to reject).
    fn dialogue() -> Vec<(i64, i64)> {
        let gaps = [900, 1_400, 700, 2_100, 1_100, 1_800, 600, 1_300, 2_400, 800];
        let lengths = [520, 780, 430, 1_100, 610, 900, 380, 700, 1_200, 460];
        let mut at = 3_000;
        let mut out = Vec::new();
        for i in 0..58 {
            let length = lengths[i % lengths.len()];
            out.push((at, at + length));
            at += length + gaps[i % gaps.len()];
            if at > 58_000 {
                break;
            }
        }
        out
    }

    #[test]
    fn an_offset_is_recovered_from_a_clean_window() {
        let bursts = dialogue();
        let samples = talking(60, &bursts);
        // `early` is how much the cues run ahead of the speech (negative: they run late); the
        // correction the reader needs is +early.
        for early in [-2_400, -900, 0, 700, 1_800] {
            let cues = cues_for(&bursts, -early);
            let found = align(&samples, RATE, 0, &cues)
                .unwrap_or_else(|| panic!("no alignment for a {early} ms offset"));
            assert!(
                (found.lag_ms - early).abs() <= FRAME_MS * 3,
                "cues {early} ms early: expected a {early} ms correction, got {}",
                found.lag_ms
            );
            assert!(found.confidence > 0.3, "confidence {}", found.confidence);
        }
    }

    #[test]
    fn a_window_starting_late_keeps_item_time() {
        // The probe starts at 120 s of the item; the cues and the audio keep item time.
        let bursts: Vec<(i64, i64)> = dialogue()
            .into_iter()
            .map(|(from, to)| (from + 120_000, to + 120_000))
            .collect();
        let samples = talking(
            61,
            &bursts
                .iter()
                .map(|(from, to)| (*from - 120_000, *to - 120_000))
                .collect::<Vec<_>>(),
        );
        // Cues that run 1.3 s late need a -1.3 s correction.
        let cues = cues_for(&bursts, 1_300);
        let found = align(&samples, RATE, 120_000, &cues).expect("alignment");
        assert!(
            (found.lag_ms + 1_300).abs() <= FRAME_MS * 3,
            "got {}",
            found.lag_ms
        );
    }

    #[test]
    fn silence_or_music_has_no_answer() {
        let silent = vec![0i16; (60 * RATE as i64) as usize];
        let cues = cues_for(&dialogue(), 0);
        assert!(align(&silent, RATE, 0, &cues).is_none(), "silence aligned");

        // A flat tone everywhere: every lag correlates as well as every other.
        let hum: Vec<i16> = (0..(60 * RATE as i64) as usize)
            .map(|i| {
                ((i as f64 / RATE as f64 * 100.0 * std::f64::consts::TAU).sin() * 6_000.0) as i16
            })
            .collect();
        assert!(align(&hum, RATE, 0, &cues).is_none(), "a hum aligned");
    }

    #[test]
    fn a_window_without_cues_has_no_answer() {
        let samples = talking(30, &dialogue());
        assert!(align(&samples, RATE, 0, &[]).is_none());
        // Cues that all sit outside the window are the same as none.
        let outside = vec![(300_000, 301_000)];
        assert!(align(&samples, RATE, 0, &outside).is_none());
    }

    #[test]
    fn the_envelope_sees_speech_above_the_floor() {
        let samples = talking(10, &[(2_000, 4_000)]);
        let levels = envelope(&samples, RATE);
        assert_eq!(levels.len(), 10_000 / FRAME_MS as usize);
        assert!(levels[150] > 0.2, "speech frame reads {}", levels[150]);
        assert!(levels[450] < 0.1, "silence frame reads {}", levels[450]);
    }
}
