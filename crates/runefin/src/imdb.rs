//! IMDb scores without an API key: IMDb publishes a daily `title.ratings.tsv.gz` (about 8.7 MB)
//! with every rated title. The first detail page that has an IMDb id downloads it once, builds a
//! sorted 6-byte index (`tt` number + rating × 10), and every later lookup is a binary search.
//! TMDb and Rotten Tomatoes come from Jellyfin itself and need none of this.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use flate2::read::GzDecoder;
use jellyfin_api::Client;

const URL: &str = "https://datasets.imdbws.com/title.ratings.tsv.gz";

pub struct ImdbRatings {
    /// Big-endian `u32` IMDb number followed by a big-endian `u16` rating × 10, 6 bytes each.
    data: Vec<u8>,
}

impl ImdbRatings {
    /// A score for `tt…`, or `None` when the title is unrated or absent.
    pub fn rating(&self, imdb_id: &str) -> Option<f64> {
        let key = imdb_id.strip_prefix("tt")?.parse::<u32>().ok()?;
        let count = self.data.len() / 6;
        let (mut low, mut high) = (0usize, count);
        while low < high {
            let mid = (low + high) / 2;
            let at = mid * 6;
            let found = u32::from_be_bytes([
                self.data[at],
                self.data[at + 1],
                self.data[at + 2],
                self.data[at + 3],
            ]);
            match found.cmp(&key) {
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid,
                std::cmp::Ordering::Equal => {
                    let rating = u16::from_be_bytes([self.data[at + 4], self.data[at + 5]]);
                    return Some(f64::from(rating) / 10.0);
                }
            }
        }
        None
    }

    fn from_records(mut records: Vec<(u32, u16)>) -> ImdbRatings {
        records.sort_unstable_by_key(|(id, _)| *id);
        let mut data = Vec::with_capacity(records.len() * 6);
        for (id, rating) in records {
            data.extend_from_slice(&id.to_be_bytes());
            data.extend_from_slice(&rating.to_be_bytes());
        }
        ImdbRatings { data }
    }
}

/// Load the cached index, or download and build it. Runs on a worker: the UI shows TMDb/RT
/// immediately and adds IMDb when this returns.
pub fn load(client: &Client) -> Result<ImdbRatings, String> {
    let bin = cache_path();
    let cached = bin.as_ref().and_then(|bin| {
        let data = std::fs::read(bin).ok()?;
        (!data.is_empty() && data.len().is_multiple_of(6)).then_some(data)
    });
    let fresh = bin.as_ref().is_some_and(|bin| {
        std::fs::metadata(bin)
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age < Duration::from_secs(30 * 24 * 60 * 60))
    });
    if let Some(data) = cached.as_ref().filter(|_| fresh) {
        return Ok(ImdbRatings { data: data.clone() });
    }
    let gz = match client.fetch_bytes(URL) {
        Ok(bytes) => bytes,
        Err(error) => {
            // A stale index is better than no score when the dataset host is unreachable.
            if let Some(data) = cached {
                return Ok(ImdbRatings { data });
            }
            return Err(error.user_message());
        }
    };
    let decoder = GzDecoder::new(std::io::Cursor::new(gz));
    let mut reader = BufReader::new(decoder);
    let mut line = String::new();
    let mut records = Vec::with_capacity(1_800_000);
    let mut first = true;
    while reader
        .read_line(&mut line)
        .map_err(|error| format!("Could not read the IMDb dataset: {error}"))?
        > 0
    {
        if first {
            first = false;
            line.clear();
            continue;
        }
        let trimmed = line.trim_end();
        let mut fields = trimmed.split('\t');
        let Some(id) = fields.next().and_then(|id| id.strip_prefix("tt")) else {
            line.clear();
            continue;
        };
        let (Some(rating), Some(_votes)) = (fields.next(), fields.next()) else {
            line.clear();
            continue;
        };
        if let (Ok(id), Ok(rating)) = (id.parse::<u32>(), rating.parse::<f32>()) {
            records.push((id, (rating * 10.0).round() as u16));
        }
        line.clear();
    }
    let ratings = ImdbRatings::from_records(records);
    if ratings.data.is_empty() {
        return Err("The IMDb dataset contained no ratings.".into());
    }
    if let Some(bin) = &bin
        && let Some(dir) = bin.parent()
    {
        let _ = std::fs::create_dir_all(dir);
        let temporary = bin.with_extension("part");
        if let Ok(mut file) = std::fs::File::create(&temporary)
            && file.write_all(&ratings.data).is_ok()
        {
            let _ = std::fs::rename(&temporary, bin);
        }
    }
    Ok(ratings)
}

fn cache_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(base.join("runefin").join("imdb-ratings.bin"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_index_finds_a_rating_and_reports_missing_titles() {
        let ratings = ImdbRatings::from_records(vec![(289_043, 75), (1_520_000, 86), (17_136, 85)]);
        assert_eq!(ratings.rating("tt0289043"), Some(7.5));
        assert_eq!(ratings.rating("tt1520000"), Some(8.6));
        assert_eq!(ratings.rating("tt0017136"), Some(8.5));
        assert_eq!(ratings.rating("tt0000000"), None);
        assert_eq!(ratings.rating("not-an-id"), None);
    }

    #[test]
    #[ignore = "downloads IMDb's 8.7 MB dataset once"]
    fn the_live_dataset_has_a_known_rating() {
        let config = crate::config::Config::load();
        assert!(config.has_session(), "sign in once before this test");
        let client = Client::new(&config.server_url, config.device_id.clone())
            .with_session(config.token.clone(), config.user_id.clone());
        let ratings = load(&client).expect("the dataset");
        assert_eq!(ratings.rating("tt0289043"), Some(7.5));
        eprintln!("imdb: tt0289043 -> {:?}", ratings.rating("tt0289043"));
    }
}
