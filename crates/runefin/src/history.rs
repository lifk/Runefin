//! Local watch history: an append-only JSONL file, no database and no server call.
//!
//! One line is one completed play. It is private to this machine; the Settings page reads the
//! last entries and the totals. Writes happen on a worker thread so a film ending never waits
//! on the disk.

use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct HistoryEntry {
    pub item_id: String,
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub series_name: Option<String>,
    #[serde(default)]
    pub year: Option<i32>,
    /// How long this play actually advanced, in milliseconds.
    pub played_ms: i64,
    /// Unix seconds when it ended.
    pub at: i64,
}

pub fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(base.join("runefin").join("history.jsonl"))
}

/// Newest last, capped. A malformed line is skipped rather than hiding the history.
pub fn load(limit: usize) -> Vec<HistoryEntry> {
    let Some(path) = path() else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut entries: Vec<HistoryEntry> = text
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    if entries.len() > limit.max(1) {
        entries.drain(..entries.len() - limit.max(1));
    }
    entries
}

pub fn append(entry: HistoryEntry) {
    std::thread::spawn(move || {
        let Some(path) = path() else {
            return;
        };
        let Some(parent) = path.parent() else {
            return;
        };
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
        let Ok(line) = serde_json::to_string(&entry) else {
            return;
        };
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(file, "{line}");
        }
    });
}

/// Letterboxd's import format: `Title,Year,WatchedDate`. Quotes are doubled.
pub fn export_csv(entries: &[HistoryEntry]) {
    let mut text = String::from("Title,Year,WatchedDate\n");
    for entry in entries {
        let title = entry.name.replace('"', "\"\"");
        let year = entry.year.map_or_else(String::new, |year| year.to_string());
        let date = entry.at.div_euclid(86_400);
        let (year_of_date, month, day) = civil_from_days(date);
        text.push_str(&format!(
            "\"{title}\",{year},{year_of_date:04}-{month:02}-{day:02}\n"
        ));
    }
    std::thread::spawn(move || {
        let Some(base) = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        else {
            return;
        };
        let directory = base.join("runefin");
        let _ = std::fs::create_dir_all(&directory);
        let _ = std::fs::write(directory.join("letterboxd.csv"), text);
    });
}

/// Howard Hinnant's civil-from-days, for the CSV date.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn clear() {
    std::thread::spawn(move || {
        if let Some(path) = path() {
            let _ = std::fs::remove_file(path);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_export_date_uses_the_unix_epoch() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_000), (2024, 10, 4));
    }

    #[test]
    fn an_entry_round_trips_as_one_line() {
        let entry = HistoryEntry {
            item_id: "dune-2".into(),
            name: "Dune: Part Two".into(),
            kind: "Movie".into(),
            series_name: None,
            year: Some(2024),
            played_ms: 7_200_000,
            at: 1_700_000_000,
        };
        let line = serde_json::to_string(&entry).expect("serialises");
        assert!(!line.contains('\n'));
        let back: HistoryEntry = serde_json::from_str(&line).expect("parses");
        assert_eq!(back.item_id, "dune-2");
        assert_eq!(back.played_ms, 7_200_000);
    }
}
