//! How loud a film is, so a quiet one and a loud one play at about the same level.
//!
//! Measured from a window of the item's own audio (see [`crate::audio_probe`]) and kept on disk,
//! because it is a property of the file: the server sends no loudness tags, and the plugins that
//! write them need the files themselves. The gain is applied to the player's volume, so the
//! reader's own volume, mute and the existing sound modes act on top of it.
//!
//! The measure is gated RMS, not EBU R128: a true LUFS meter needs K-weighting and true peak, and
//! what this is for — one film against the next — is a difference of fives of dB, not of two.
//! Frames under [`SILENCE_DBFS`] are excluded, so a film with long quiet stretches does not read
//! as a quiet film.

use std::path::PathBuf;

/// What a −23 LUFS EBU mix sits at as gated RMS. Films, series and rips come in around it; the
/// gain moves them to it.
pub const TARGET_DBFS: f32 = -23.0;
/// A wider correction means the window is not representative, so the gain is not applied whole.
pub const MAX_GAIN_DB: f32 = 12.0;
/// Frames quieter than this are silence for the measurement's purposes.
const SILENCE_DBFS: f32 = -60.0;

/// A window's level and the gain that brings it to the target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measurement {
    /// Gated RMS of the window, in dBFS (a negative number; −23 is typical).
    pub dbfs: f32,
    /// What to multiply the player's volume by, 1.0 meaning "leave it alone".
    pub gain: f32,
}

/// The measurement for a level already known — the median of several windows, say.
pub fn at_level(dbfs: f32) -> Measurement {
    let gain_db = (TARGET_DBFS - dbfs).clamp(-MAX_GAIN_DB, MAX_GAIN_DB);
    Measurement {
        dbfs,
        gain: 10f32.powf(gain_db / 20.0),
    }
}

/// Measure a window of mono PCM.
pub fn measure(samples: &[i16], sample_rate: u32) -> Option<Measurement> {
    let frame = (sample_rate as usize / 10).max(1); // 100 ms
    if samples.len() < sample_rate as usize / 2 {
        return None;
    }
    let mut sum = 0.0f64;
    let mut counted = 0u32;
    for chunk in samples.chunks_exact(frame) {
        let energy: f64 = chunk
            .iter()
            .map(|s| {
                let v = f64::from(*s) / 32_768.0;
                v * v
            })
            .sum::<f64>()
            / chunk.len() as f64;
        let level = 10.0 * energy.log10(); // dBFS of this frame
        if level.is_finite() && level > f64::from(SILENCE_DBFS) {
            sum += energy;
            counted += 1;
        }
    }
    if counted < 10 {
        // Under a second of anything above silence.
        return None;
    }
    let rms = (sum / f64::from(counted)).sqrt();
    if rms <= 0.0 || !rms.is_finite() {
        return None;
    }
    let dbfs = (20.0 * rms.log10()) as f32;
    Some(at_level(dbfs))
}

/// The cached measurements: one per item, rewritten as they are taken.
pub fn cache_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(base.join("runefin").join("loudness.json"))
}

/// Every measurement taken so far, by item id. A file that does not read is an empty cache.
pub fn load() -> std::collections::HashMap<String, Stored> {
    cache_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str::<Cache>(&text).ok())
        .filter(|cache| cache.v == VERSION)
        .map(|cache| cache.items)
        .unwrap_or_default()
}

/// Add one measurement and write the cache: it is small and written once per item ever measured.
pub fn store(items: &mut std::collections::HashMap<String, Stored>, item_id: &str, value: Stored) {
    items.insert(item_id.to_string(), value);
    let Some(path) = cache_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let cache = Cache {
        v: VERSION,
        items: items.clone(),
    };
    if let Ok(text) = serde_json::to_string(&cache) {
        // A write that fails leaves the old file; the next measurement tries again.
        let _ = std::fs::write(&path, text);
    }
}

const VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize)]
struct Cache {
    v: u32,
    items: std::collections::HashMap<String, Stored>,
}

/// One item's measurement, as it lives on disk.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Stored {
    /// Gated RMS, in dBFS.
    pub dbfs: f32,
    /// The multiplier to apply.
    pub gain: f32,
    /// Seconds since the epoch, for a report and for re-measuring later if that is ever wanted.
    pub saved: u64,
}

impl From<Measurement> for Stored {
    fn from(value: Measurement) -> Self {
        Stored {
            dbfs: value.dbfs,
            gain: value.gain,
            saved: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `RUNEFIN_PROBE_WAV=<wav>`: what this module reads from a file, next to an ffmpeg
    /// measurement of the same file, for calibrating against a real mix.
    #[test]
    #[ignore = "needs RUNEFIN_PROBE_WAV=<wav>"]
    fn a_wav_file_reads_at_a_known_level() {
        let path = std::env::var("RUNEFIN_PROBE_WAV").expect("RUNEFIN_PROBE_WAV");
        let bytes = std::fs::read(path).expect("wav");
        // Minimal RIFF walk: find `data`, assume 16-bit mono at 8 kHz, as the probe writes it.
        let at = bytes
            .windows(4)
            .position(|w| w == b"data")
            .expect("a data chunk")
            + 8;
        let rate = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
        let channels = u16::from_le_bytes([bytes[22], bytes[23]]);
        let samples: Vec<i16> = bytes[at..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| i16::from_le_bytes(*pair))
            .collect();
        assert_eq!(channels, 1, "the probe writes mono");
        let measured = measure(&samples, rate).expect("a level");
        eprintln!(
            "wav: {:.1} dBFS at {} Hz, {} samples, gain {:+.1} dB",
            measured.dbfs,
            rate,
            samples.len(),
            20.0 * measured.gain.log10()
        );
    }

    /// A sine whose RMS is the given dBFS, 8 kHz mono — a sine's RMS is its peak over √2, so the
    /// peak is scaled to match.
    fn tone(dbfs: f32, seconds: u32) -> Vec<i16> {
        let rate = 8_000u32;
        let amplitude = 32_767.0 * 10f32.powf(dbfs / 20.0) * std::f32::consts::SQRT_2;
        (0..rate * seconds)
            .map(|at| {
                let t = at as f32 / rate as f32;
                (amplitude * (t * 440.0 * std::f32::consts::TAU).sin()) as i16
            })
            .collect()
    }

    #[test]
    fn louder_and_quieter_windows_meet_at_the_target() {
        let loud = measure(&tone(-18.0, 5), 8_000).expect("a loud window");
        let quiet = measure(&tone(-30.0, 5), 8_000).expect("a quiet window");
        assert!((loud.dbfs + 18.0).abs() < 1.0, "measured {}", loud.dbfs);
        assert!((quiet.dbfs + 30.0).abs() < 1.0, "measured {}", quiet.dbfs);
        // Both end up at about the target, so the gain differs by the 12 dB between them.
        let loud_gain = 20.0 * loud.gain.log10();
        let quiet_gain = 20.0 * quiet.gain.log10();
        assert!(
            (quiet_gain - loud_gain - 12.0).abs() < 1.0,
            "{loud_gain} {quiet_gain}"
        );
        assert!((loud.dbfs + loud_gain - TARGET_DBFS).abs() < 1.0);
        assert!((quiet.dbfs + quiet_gain - TARGET_DBFS).abs() < 1.0);
    }

    #[test]
    fn the_gain_is_clamped_and_silence_has_none() {
        let very_quiet = measure(&tone(-50.0, 5), 8_000).expect("measured");
        let gain_db = 20.0 * very_quiet.gain.log10();
        assert!((gain_db - MAX_GAIN_DB).abs() < 0.1, "{gain_db}");
        assert!(
            measure(&vec![0i16; 8_000 * 5], 8_000).is_none(),
            "silence measured"
        );
        assert!(
            measure(&[0i16; 100], 8_000).is_none(),
            "a hundred samples measured"
        );
    }

    #[test]
    fn a_quiet_passage_does_not_make_a_quiet_film() {
        // Ten seconds at −18 dBFS with ten seconds of silence after it: the silence is gated out,
        // so the film still reads as loud.
        let mut samples = tone(-18.0, 10);
        samples.extend(std::iter::repeat_n(0i16, 8_000 * 10));
        let mixed = measure(&samples, 8_000).expect("measured");
        assert!((mixed.dbfs + 18.0).abs() < 1.0, "measured {}", mixed.dbfs);
    }

    #[test]
    fn the_cache_round_trips_through_disk() {
        let path = std::env::temp_dir().join(format!(
            "runefin-loudness-{}-{}.json",
            std::process::id(),
            line!()
        ));
        let cache = Cache {
            v: VERSION,
            items: std::collections::HashMap::from([(
                "item-1".to_string(),
                Stored {
                    dbfs: -20.0,
                    gain: 0.5,
                    saved: 1,
                },
            )]),
        };
        std::fs::write(&path, serde_json::to_string(&cache).expect("json")).expect("written");
        let read: Cache =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("parsed");
        assert_eq!(read.v, VERSION);
        assert_eq!(read.items["item-1"].gain, 0.5);
        let mut wrong = read;
        wrong.v = VERSION + 1;
        std::fs::write(&path, serde_json::to_string(&wrong).expect("json")).expect("written");
        let stale: Option<Cache> =
            serde_json::from_str::<Cache>(&std::fs::read_to_string(&path).expect("read"))
                .ok()
                .filter(|cache| cache.v == VERSION);
        assert!(stale.is_none(), "a foreign version was accepted");
        let _ = std::fs::remove_file(&path);
    }
}
