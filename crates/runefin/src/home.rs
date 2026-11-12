//! What the home screen shows, built from the server's own lists.
//!
//! `/UserItems/Resume` returns one row per *episode*, so a series watched across several days
//! appears several times. The design asks for one card per series, ordered by the server's own
//! recency, and hides entries untouched for 60 days. That merge and the date arithmetic live
//! here, away from the screen, so both are testable without a server.

use std::collections::HashSet;

use jellyfin_api::{Client, ImageKind, Item, LibraryView};

/// Resume entries older than this are hidden client-side (D2).
const STALE_DAYS: i64 = 60;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct HomeData {
    pub libraries: Vec<LibraryView>,
    pub resume: Vec<Item>,
    /// One next episode per series, from `/Shows/NextUp`.
    pub next_up: Vec<Item>,
    /// Live channels with a current program.
    pub on_now: Vec<Item>,
    /// Box sets, so collections are reachable without opening their library first.
    pub collections: Vec<Item>,
    /// Played films and episodes, newest first, deduplicated by series and never duplicating a
    /// Continue Watching card.
    pub recently_played: Vec<Item>,
    pub latest: Vec<LatestRail>,
}

impl HomeData {
    pub fn is_empty(&self) -> bool {
        self.resume.is_empty()
            && self.next_up.is_empty()
            && self.on_now.is_empty()
            && self.collections.is_empty()
            && self.recently_played.is_empty()
            && self.latest.iter().all(|rail| rail.items.is_empty())
    }
}

/// One "Recently added in <library>" rail.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LatestRail {
    pub library: String,
    pub library_id: String,
    pub items: Vec<Item>,
}

/// A finished home request: the screen renders either the data or the reason it is missing.
#[derive(Debug, Clone)]
pub enum HomeLoad {
    Ready(HomeData),
    Failed(String),
}

/// Fetch the rails that must be on screen together: libraries, Continue Watching and the
/// Recently Added rails. The requests after `libraries` run in parallel; Next Up is fetched by
/// [`load_next_up`] at the same time so a slow guide query cannot delay the first paint.
///
/// Libraries and Continue Watching are required; one library's Recently Added failing is not
/// worth hiding the rest of the screen, so that rail is skipped and the others stay.
pub fn load(client: &Client, now_days: i64) -> Result<HomeData, String> {
    // Continue Watching doesn't need the library list, so it travels beside the library call
    // instead of behind it: the two answered in series cost their sum on the way to the first
    // paint (164 + 170 ms against this server), and in parallel only the slower one.
    let t = crate::bench::timer();
    let (libraries, resume) = std::thread::scope(|scope| {
        let libraries_job = scope.spawn(|| {
            let t = crate::bench::timer();
            let answer = client.libraries().map_err(|e| e.user_message());
            crate::bench::lap("libraries", t);
            answer
        });
        let resume_job = scope.spawn(|| {
            let t = crate::bench::timer();
            let answer = client.resume(24).map_err(|e| e.user_message());
            crate::bench::lap("resume", t);
            answer
        });
        let libraries = libraries_job.join().expect("libraries worker");
        let resume = resume_job.join().expect("resume worker");
        Ok::<_, String>((libraries?, resume?))
    })?;
    // One Recently Added rail per video library. The server owns the order; every library the
    // reader can browse gets its own rail.
    let video: Vec<&LibraryView> = libraries
        .iter()
        .filter(|library| is_video_library(library))
        .collect();
    let latest = std::thread::scope(|scope| {
        let latest_jobs: Vec<_> = video
            .iter()
            .map(|library| {
                scope.spawn(move || {
                    let t = crate::bench::timer();
                    let answer = client
                        .latest(Some(&library.id), 12)
                        .ok()
                        .filter(|items| !items.is_empty())
                        .map(|items| LatestRail {
                            library: library.name.clone(),
                            library_id: library.id.clone(),
                            items,
                        });
                    crate::bench::lap("latest", t);
                    answer
                })
            })
            .collect();
        latest_jobs
            .into_iter()
            .filter_map(|job| job.join().expect("latest worker"))
            .collect::<Vec<LatestRail>>()
    });
    crate::bench::lap("home.jobs", t);
    Ok(HomeData {
        libraries,
        resume: merge_resume(resume.items, now_days),
        next_up: Vec::new(),
        on_now: Vec::new(),
        // Loaded by [`load_collections`] beside this call: enumerating box sets walks every
        // library on the server and would otherwise add ~40 ms to the first paint.
        collections: Vec::new(),
        recently_played: Vec::new(),
        latest,
    })
}

/// Box sets for the home rail, loaded beside the rest so a large library's box-set walk never
/// gates the first paint. A failure is a missing rail, not a broken Home.
pub fn load_collections(client: &Client) -> Vec<Item> {
    client.collections(12).unwrap_or_default()
}

/// The one-episode-per-series rail, loaded beside the rest so its slower query never gates the
/// first paint. A failure is a missing rail, not a broken Home.
pub fn load_next_up(client: &Client) -> Vec<Item> {
    client.next_up_all(12).unwrap_or_default()
}

/// Live channels that have a current program. A tuner-less server returns nothing.
pub fn load_on_now(client: &Client) -> Vec<Item> {
    client
        .live_channels(8)
        .unwrap_or_default()
        .into_iter()
        .filter(|channel| channel.current_program.is_some())
        .collect()
}

/// Recently played films and episodes, newest first. Loaded beside the first paint; a failure is
/// a missing rail, not a broken Home.
pub fn load_recently_played(client: &Client) -> Vec<Item> {
    client
        .items(&jellyfin_api::ItemQuery {
            include_types: vec!["Movie", "Episode"],
            recursive: true,
            played_only: true,
            sort_by: Some(jellyfin_api::SortBy::DatePlayed),
            descending: true,
            limit: 24,
            ..jellyfin_api::ItemQuery::default()
        })
        .map(|page| page.items)
        .unwrap_or_default()
}

/// Keep one card per series, newest first, and never repeat a Continue Watching entry. The
/// server returns every played episode, so this is the same merge rule as `merge_resume`.
pub fn merge_recently_played(items: Vec<Item>, resume: &[Item]) -> Vec<Item> {
    let resume_series: HashSet<&str> = resume
        .iter()
        .map(|item| item.series_id.as_deref().unwrap_or(item.id.as_str()))
        .collect();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        let key = item.series_id.as_deref().unwrap_or(item.id.as_str());
        if resume_series.contains(key) || !seen.insert(key.to_string()) {
            continue;
        }
        out.push(item);
        if out.len() == 12 {
            break;
        }
    }
    out
}

/// One card per series, newest first, stale entries dropped, unfinished only.
pub fn merge_resume(items: Vec<Item>, now_days: i64) -> Vec<Item> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        if item.user_data.as_ref().is_some_and(|u| !u.is_resumable()) {
            continue;
        }
        if let Some(days) = last_played_days(&item)
            && now_days.saturating_sub(days) > STALE_DAYS
        {
            continue;
        }
        let key = item.series_id.clone().unwrap_or_else(|| item.id.clone());
        if seen.insert(key) {
            out.push(item);
        }
    }
    out
}

/// The poster to show for a card: the series poster for an episode, the item's own otherwise.
pub fn poster(item: &Item) -> Option<(&str, &str)> {
    if let (Some(series), Some(tag)) = (&item.series_id, &item.series_primary_image_tag) {
        return Some((series, tag));
    }
    let tag = item.image_tag(ImageKind::Primary)?;
    Some((item.id.as_str(), tag))
}

/// True when the server added the item in the last 30 days. The date is the server's own
/// `DateCreated`; no local guess from a file timestamp.
pub fn is_new(item: &Item) -> bool {
    let Some(created) = item.date_created.as_deref().and_then(days_from_iso_date) else {
        return false;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs() as i64)
        / 86_400;
    now.saturating_sub(created) <= 30
}

/// The line under a Continue Watching card: `S2E04 · 18 min left`, or the year for a film with
/// no runtime left.
pub fn resume_subtitle(item: &Item) -> String {
    let left = remaining(item).map(|minutes| format!("{minutes} min left"));
    match (item.episode_label(), left) {
        (Some(episode), Some(left)) => format!("{episode} · {left}"),
        (Some(episode), None) => episode,
        (None, Some(left)) => left,
        (None, None) => subtitle(item),
    }
}

/// Minutes left in the item, rounded up, when the server gives both a runtime and a position.
pub fn remaining(item: &Item) -> Option<i64> {
    let runtime = item.runtime_ms()?;
    let position = item.user_data.as_ref()?.position_ms()?;
    let left = runtime.saturating_sub(position);
    (left > 0).then(|| (left + 59_999) / 60_000)
}

/// The line under a card's title: `S2E04 · Overtime`, or the year for a film.
pub fn subtitle(item: &Item) -> String {
    if let Some(episode) = item.episode_label() {
        return format!("{episode} · {}", item.name);
    }
    item.production_year
        .map(|year| year.to_string())
        .unwrap_or_default()
}

/// `0.0..=1.0` watched share for the progress bar; the server sends the percentage for resume
/// rows and we fall back to position / runtime when it does not.
pub fn progress(item: &Item) -> f32 {
    let Some(user) = &item.user_data else {
        return 0.0;
    };
    let percent = user
        .played_percentage
        .or_else(|| {
            let position = user.position_ms()? as f64;
            let runtime = item.runtime_ms()? as f64;
            (runtime > 0.0).then_some(position / runtime * 100.0)
        })
        .unwrap_or(0.0);
    (percent / 100.0).clamp(0.0, 1.0) as f32
}

/// Whether an item is finished. A series counts as watched when the server says nothing in it
/// is unplayed; an episode or film when it is marked played. This is what the check badge on a
/// card means, and it is the same rule Jellyfin's own clients use.
pub fn is_watched(item: &Item) -> bool {
    let Some(user) = &item.user_data else {
        return false;
    };
    user.played || (item.kind == "Series" && user.unplayed_item_count == Some(0))
}

/// Today as days since the Unix epoch, the unit the stale check compares in.
pub fn now_days() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64 / 86_400)
        .unwrap_or(0)
}

fn is_video_library(library: &LibraryView) -> bool {
    !matches!(
        library.collection_type.as_deref(),
        Some("music") | Some("livetv") | Some("playlists")
    )
}

fn last_played_days(item: &Item) -> Option<i64> {
    let date = item.user_data.as_ref()?.last_played_date.as_deref()?;
    days_from_iso_date(date)
}

/// The date part of Jellyfin's ISO 8601 `LastPlayedDate`, as days since the epoch. The server
/// sends UTC; time-of-day precision does not change a 60-day decision.
pub(crate) fn days_from_iso_date(date: &str) -> Option<i64> {
    let mut parts = date.get(..10)?.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: i64 = parts.next()?.parse().ok()?;
    let day: i64 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Howard Hinnant's civil-date conversion; no calendar crate and no local-time ambiguity.
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let shifted_month = (month + 9) % 12;
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * 146_097 + day_of_era - 719_468)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jellyfin_api::UserData;

    fn item(id: &str, series: Option<&str>, last_played: Option<&str>) -> Item {
        Item {
            id: id.into(),
            name: id.into(),
            series_id: series.map(Into::into),
            user_data: Some(UserData {
                playback_position_ticks: 10_000 * 60_000,
                last_played_date: last_played.map(Into::into),
                ..UserData::default()
            }),
            ..Item::default()
        }
    }

    const NOW: i64 = 20_000;

    #[test]
    fn watched_covers_a_played_item_and_a_series_with_nothing_left() {
        let mut played = item("e1", None, None);
        played.user_data = Some(UserData {
            played: true,
            ..UserData::default()
        });
        assert!(is_watched(&played));

        let mut finished_series = item("s1", None, None);
        finished_series.kind = "Series".into();
        finished_series.user_data = Some(UserData {
            unplayed_item_count: Some(0),
            ..UserData::default()
        });
        assert!(is_watched(&finished_series));

        let mut running = item("s2", None, None);
        running.kind = "Series".into();
        running.user_data = Some(UserData {
            unplayed_item_count: Some(3),
            ..UserData::default()
        });
        assert!(!is_watched(&running));
        assert!(!is_watched(&item("e2", None, None)));
    }

    #[test]
    fn a_series_watched_across_episodes_gets_one_card_newest_first() {
        let merged = merge_resume(
            vec![
                item("e3", Some("show"), Some("2026-11-01T20:00:00Z")),
                item("e2", Some("show"), Some("2026-10-30T20:00:00Z")),
                item("film", None, Some("2026-10-20T20:00:00Z")),
            ],
            NOW,
        );
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].id, "e3");
        assert_eq!(merged[1].id, "film");
    }

    #[test]
    fn a_resume_entry_untouched_for_more_than_sixty_days_is_hidden() {
        let stale = days_from_iso_date("2026-08-01").expect("a valid ISO date");
        let now = stale + STALE_DAYS + 1;
        assert!(merge_resume(vec![item("old", None, Some("2026-08-01"))], now).is_empty());
        assert_eq!(
            merge_resume(
                vec![item("edge", None, Some("2026-08-01"))],
                stale + STALE_DAYS
            )
            .len(),
            1
        );
    }

    #[test]
    fn an_unfinished_item_without_a_date_is_kept() {
        let merged = merge_resume(vec![item("new", None, None)], NOW);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn a_finished_item_never_reaches_continue_watching() {
        let mut finished = item("done", None, Some("2026-11-01T20:00:00Z"));
        finished.user_data = Some(UserData {
            playback_position_ticks: 1,
            played: true,
            ..UserData::default()
        });
        assert!(merge_resume(vec![finished], NOW).is_empty());
    }

    #[test]
    fn recently_played_dedupes_by_series_and_excludes_continue_watching() {
        let mut resume_item = item("cw-e1", Some("show-a"), None);
        resume_item.series_id = Some("show-a".into());
        let latest = item("e2", Some("show-a"), None);
        let second = item("e3", Some("show-b"), None);
        let older = item("e4", Some("show-b"), None);
        let merged = merge_recently_played(
            vec![latest, second, older, item("film", None, None)],
            &[resume_item],
        );
        assert_eq!(
            merged
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["e3", "film"]
        );
    }

    #[test]
    fn a_resume_card_says_how_much_is_left() {
        let mut i = item("ep", None, None);
        i.run_time_ticks = Some(10_000 * 30 * 60_000);
        i.user_data
            .as_mut()
            .expect("the fixture has user data")
            .playback_position_ticks = 10_000 * 12 * 60_000;
        i.parent_index_number = Some(2);
        i.index_number = Some(4);
        assert_eq!(remaining(&i), Some(18));
        assert_eq!(resume_subtitle(&i), "S2E04 · 18 min left");
    }

    #[test]
    fn the_iso_date_parser_handles_the_epoch_and_leap_years() {
        assert_eq!(days_from_iso_date("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(days_from_iso_date("2024-02-29T12:00:00Z"), Some(19_782));
        assert_eq!(days_from_iso_date("not a date"), None);
    }

    #[test]
    fn episodes_prefer_the_series_poster_and_movies_use_their_own() {
        let mut episode = item("e1", Some("show"), None);
        episode.series_primary_image_tag = Some("series-tag".into());
        assert_eq!(poster(&episode), Some(("show", "series-tag")));

        let mut film = Item {
            id: "film".into(),
            image_tags: Some([("Primary".to_string(), "film-tag".to_string())].into()),
            ..Item::default()
        };
        assert_eq!(poster(&film), Some(("film", "film-tag")));
        film.image_tags = None;
        assert_eq!(poster(&film), None);
    }

    #[test]
    fn progress_prefers_the_server_percentage_and_falls_back_to_position() {
        let mut i = item("ep", None, None);
        i.run_time_ticks = Some(10_000 * 120_000);
        i.user_data
            .as_mut()
            .expect("the fixture has user data")
            .played_percentage = Some(42.0);
        assert!((progress(&i) - 0.42).abs() < 1e-5);
        i.user_data
            .as_mut()
            .expect("the fixture has user data")
            .played_percentage = None;
        assert!((progress(&i) - 0.5).abs() < 1e-5);
    }
}
