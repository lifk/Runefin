//! One item's detail page: the item, its seasons, and the episodes of the selected season.
//!
//! Movies and episodes need only one request; a series adds its seasons and the first season's
//! episodes. Season switches are their own small request so the page can show the item before
//! every episode has arrived. The `S2E04 · 48 min`-style labels and the resume time are here,
//! away from the screen, so they are testable without a server.

use jellyfin_api::{Client, Item, ItemQuery, SortBy};

#[derive(Debug, Clone)]
pub struct DetailPage {
    pub item: Item,
    pub seasons: Vec<Item>,
    pub episodes: Vec<Item>,
    pub next_up: Option<Item>,
    /// A BoxSet's members, in the order the server lists them.
    pub collection: Vec<Item>,
    /// Server-hosted trailers, played in the app.
    pub local_trailers: Vec<Item>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EpisodesPage {
    pub season: String,
    pub items: Vec<Item>,
    pub error: Option<String>,
}

/// How a box set's members are ordered on screen. The server's order is never destroyed; the
/// other modes sort a copy so switching back is exact.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CollectionSort {
    #[default]
    Server,
    Name,
    Year,
    Runtime,
}

impl CollectionSort {
    pub const ALL: [CollectionSort; 4] = [
        CollectionSort::Server,
        CollectionSort::Name,
        CollectionSort::Year,
        CollectionSort::Runtime,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CollectionSort::Server => "Server",
            CollectionSort::Name => "Name",
            CollectionSort::Year => "Year",
            CollectionSort::Runtime => "Runtime",
        }
    }

    /// A sorted copy. Year is newest first and runtime longest first; both fall back to the
    /// name so two items from the same year keep a stable order.
    pub fn apply(self, items: &[Item]) -> Vec<Item> {
        let mut out = items.to_vec();
        match self {
            CollectionSort::Server => {}
            CollectionSort::Name => out.sort_by_key(|item| item.name.to_lowercase()),
            CollectionSort::Year => out.sort_by(|a, b| {
                b.production_year
                    .unwrap_or(0)
                    .cmp(&a.production_year.unwrap_or(0))
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            }),
            CollectionSort::Runtime => out.sort_by(|a, b| {
                b.runtime_ms()
                    .unwrap_or(0)
                    .cmp(&a.runtime_ms().unwrap_or(0))
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            }),
        }
        out
    }
}

pub struct DetailState {
    pub item: Item,
    pub seasons: Vec<Item>,
    pub active_season: Option<String>,
    pub episodes: Vec<Item>,
    pub next_up: Option<Item>,
    pub collection: Vec<Item>,
    /// The order the collection grid is drawn in; the server's order is the default.
    pub collection_sort: CollectionSort,
    /// Server-hosted trailers; remote YouTube trailers live on `item.remote_trailers`.
    pub local_trailers: Vec<Item>,
    /// "More like this": what the server recommends after this item.
    pub similar: Vec<Item>,
    pub loading: bool,
    pub loading_episodes: bool,
    pub error: Option<String>,
}

impl DetailState {
    /// What the primary action starts. For a series that is the server's Next Up episode (the
    /// same one the row below the hero offers), then the first unplayed episode, then the first
    /// one. The button's label and the player both call this, so "Resume 18:41" can never
    /// disagree with the episode that actually opens.
    pub fn play_target(&self) -> Option<&Item> {
        if self.item.kind == "Series" {
            self.next_up
                .as_ref()
                .or_else(|| {
                    self.episodes
                        .iter()
                        .find(|episode| episode.user_data.as_ref().is_none_or(|user| !user.played))
                })
                .or_else(|| self.episodes.first())
        } else {
            Some(&self.item)
        }
    }

    pub fn loading(id: &str) -> DetailState {
        DetailState {
            item: Item {
                id: id.to_string(),
                ..Item::default()
            },
            seasons: Vec::new(),
            active_season: None,
            episodes: Vec::new(),
            next_up: None,
            collection: Vec::new(),
            collection_sort: CollectionSort::default(),
            local_trailers: Vec::new(),
            similar: Vec::new(),
            loading: true,
            loading_episodes: false,
            error: None,
        }
    }
}

/// The item plus, for a series, its seasons and the first season's episodes.
pub fn load(client: &Client, id: &str) -> Result<DetailPage, String> {
    let item = client.item(id).map_err(|e| e.user_message())?;
    let mut seasons = Vec::new();
    let mut episodes = Vec::new();
    let mut next_up = None;
    let mut collection = Vec::new();
    if item.kind == "Series" {
        seasons = children(client, &item.id, "Season")?;
        if let Some(first) = seasons.first() {
            episodes = children(client, &first.id, "Episode")?;
        }
        next_up = client.next_up(&item.id, 1).unwrap_or(None);
    }
    if item.kind == "BoxSet" {
        collection = collection_children(client, &item.id)?;
    }
    // A missing trailer list is normal (most items have none) and must not fail the page.
    let local_trailers = client.local_trailers(&item.id).unwrap_or_default();
    Ok(DetailPage {
        item,
        seasons,
        episodes,
        next_up,
        collection,
        local_trailers,
        error: None,
    })
}

pub fn load_episodes(client: &Client, season: &str) -> EpisodesPage {
    match children(client, season, "Episode") {
        Ok(items) => EpisodesPage {
            season: season.to_string(),
            items,
            error: None,
        },
        Err(error) => EpisodesPage {
            season: season.to_string(),
            items: Vec::new(),
            error: Some(error),
        },
    }
}

/// A collection's direct members: movies or whole series, not their episodes.
fn collection_children(client: &Client, parent: &str) -> Result<Vec<Item>, String> {
    let mut query = ItemQuery::in_library(parent);
    query.recursive = false;
    query.include_types = vec!["Movie", "Series"];
    query.sort_by = Some(SortBy::SortName);
    query.limit = 300;
    client
        .items(&query)
        .map(|page| page.items)
        .map_err(|error| error.user_message())
}

fn children(client: &Client, parent: &str, kind: &'static str) -> Result<Vec<Item>, String> {
    let mut query = ItemQuery::in_library(parent);
    query.recursive = false;
    query.include_types = vec![kind];
    query.sort_by = Some(SortBy::SortName);
    query.limit = 300;
    query.extra_fields = Some("Chapters,Trickplay".into());
    let mut items = client.items(&query).map_err(|e| e.user_message())?.items;
    if kind == "Season" {
        items.sort_by_key(|item| (item.index_number.unwrap_or(0), item.name.clone()));
    } else {
        items.sort_by_key(|item| {
            (
                item.parent_index_number.unwrap_or(0),
                item.index_number.unwrap_or(0),
                item.name.clone(),
            )
        });
    }
    Ok(items)
}

/// A short technical line for the hero: `4K · HDR10 · HEVC · 5.1 · 42.1 GB`.
pub fn technical_line(item: &Item) -> Option<String> {
    if item.is_folder() {
        return None;
    }
    let source = item.primary_media_source()?;
    let mut parts = Vec::new();
    if let Some(video) = item.video_stream() {
        if let (Some(width), Some(height)) = (video.width, video.height) {
            parts.push(resolution_label(width, height));
        }
        if let Some(range) = video.video_range.as_deref()
            && !range.eq_ignore_ascii_case("SDR")
            && !range.eq_ignore_ascii_case("Unknown")
        {
            parts.push(range.to_string());
        }
        if let Some(codec) = &video.codec {
            parts.push(codec.to_uppercase());
        }
    }
    if let Some(audio) = item.audio_stream()
        && let Some(channels) = audio.channels
        && channels > 2
    {
        parts.push(channels_label(channels));
    }
    if let Some(size) = source.size {
        parts.push(format_bytes(size));
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// The rows behind the hero's "Details" toggle.
pub fn technical_facts(item: &Item) -> Vec<(String, String)> {
    let Some(source) = item.primary_media_source() else {
        return Vec::new();
    };
    let mut facts = Vec::new();
    facts.push((
        "Container".into(),
        source
            .container
            .as_deref()
            .map(str::to_uppercase)
            .unwrap_or_else(|| "—".into()),
    ));
    if let Some(video) = item.video_stream() {
        let mut bits = Vec::new();
        if let Some(codec) = &video.codec {
            bits.push(codec.to_uppercase());
        }
        if let (Some(width), Some(height)) = (video.width, video.height) {
            bits.push(format!("{width}×{height}"));
        }
        if let Some(range) = &video.video_range {
            bits.push(range.clone());
        }
        if let Some(profile) = &video.profile {
            bits.push(profile.clone());
        }
        facts.push(("Video".into(), bits.join(" · ")));
    }
    if let Some(audio) = item.audio_stream() {
        let mut bits = Vec::new();
        if let Some(codec) = &audio.codec {
            bits.push(codec.to_uppercase());
        }
        if let Some(channels) = audio.channels {
            bits.push(channels_label(channels));
        }
        if let Some(language) = &audio.language {
            bits.push(language.to_uppercase());
        }
        facts.push(("Audio".into(), bits.join(" · ")));
    }
    if let Some(size) = source.size {
        facts.push(("Size".into(), format_bytes(size)));
    }
    if let Some(bitrate) = source.bitrate {
        facts.push((
            "Bitrate".into(),
            format!("{:.1} Mbps", bitrate as f64 / 1_000_000.0),
        ));
    }
    facts
}

fn resolution_label(width: i32, height: i32) -> String {
    if width >= 3_800 || height >= 2_000 {
        "4K".into()
    } else if width >= 1_900 || height >= 1_000 {
        "1080p".into()
    } else if width >= 1_200 || height >= 700 {
        "720p".into()
    } else {
        format!("{width}×{height}")
    }
}

fn channels_label(channels: i32) -> String {
    match channels {
        8 => "7.1".into(),
        6 => "5.1".into(),
        2 => "Stereo".into(),
        1 => "Mono".into(),
        other => format!("{other} ch"),
    }
}

fn format_bytes(bytes: i64) -> String {
    let gb = bytes as f64 / 1_000_000_000.0;
    if gb >= 1.0 {
        format!("{gb:.1} GB")
    } else {
        format!("{:.0} MB", bytes as f64 / 1_000_000.0)
    }
}

/// The one primary action's label, carrying the resume position of the item it will start.
pub fn primary_label(detail: &DetailState) -> String {
    let Some(item) = detail.play_target() else {
        return "Play".into();
    };
    label_for(item)
}

fn label_for(item: &Item) -> String {
    let Some(user) = &item.user_data else {
        return "Play".into();
    };
    if user.played {
        return "Play again".into();
    }
    match user.position_ms() {
        Some(position) if position > 0 => {
            let context = item
                .episode_label()
                .map(|label| format!("{label} · "))
                .unwrap_or_default();
            format!("Resume {context}{}", format_time(position))
        }
        _ => "Play".into(),
    }
}

/// `2025 · TV-14 · 48 min · Drama, Mystery` — one muted line under the title.
/// The extra collector facts the hero keeps out of its one-line metadata. Empty means the
/// server gave none; the caller then draws no line at all.
pub fn extended_metadata(item: &Item) -> String {
    let mut parts = Vec::new();
    if let Some(original) = &item.original_title
        && !original.eq_ignore_ascii_case(&item.name)
    {
        parts.push(original.clone());
    }
    if let Some(date) = &item.premiere_date {
        parts.push(format!("Released {}", date.get(..10).unwrap_or(date)));
    }
    if let Some(status) = &item.status {
        parts.push(status.clone());
    }
    if let Some(studio) = item.studios.first() {
        parts.push(studio.name.clone());
    }
    if !item.tags.is_empty() {
        parts.push(
            item.tags
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    parts.join(" · ")
}

pub fn metadata(item: &Item) -> String {
    let mut parts = Vec::new();
    if let Some(year) = item.production_year {
        parts.push(year.to_string());
    }
    if let Some(rating) = &item.official_rating {
        parts.push(rating.clone());
    }
    if let Some(minutes) = item.runtime_ms().map(|ms| ms / 60_000)
        && minutes > 0
    {
        parts.push(format!("{minutes} min"));
    }
    if let Some(genres) = &item.genres
        && !genres.is_empty()
    {
        parts.push(
            genres
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    parts.join(" · ")
}

/// `42:13` under an hour, `1:02:13` over it.
pub fn format_time(ms: i64) -> String {
    let seconds = (ms / 1000).max(0);
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jellyfin_api::UserData;

    fn item() -> Item {
        Item {
            id: "item".into(),
            name: "The Item".into(),
            production_year: Some(2025),
            official_rating: Some("TV-14".into()),
            run_time_ticks: Some(10_000 * 60_000 * 48),
            genres: Some(vec!["Drama".into(), "Mystery".into(), "Thriller".into()]),
            user_data: Some(UserData {
                playback_position_ticks: 10_000 * 2_533_000,
                ..UserData::default()
            }),
            ..Item::default()
        }
    }

    #[test]
    fn the_primary_button_carries_the_resume_position() {
        assert_eq!(label_for(&item()), "Resume 42:13");
        let mut played = item();
        played.user_data = Some(UserData {
            played: true,
            ..UserData::default()
        });
        assert_eq!(label_for(&played), "Play again");
        played.user_data = None;
        assert_eq!(label_for(&played), "Play");
    }

    #[test]
    fn a_series_primary_button_resumes_the_episode_it_will_play() {
        let mut detail = DetailState::loading("frieren");
        detail.item.kind = "Series".into();
        let mut first = item();
        first.id = "s1e1".into();
        first.user_data = Some(UserData {
            playback_position_ticks: 10_000 * 60_000 * 7,
            ..UserData::default()
        });
        detail.episodes.push(first);
        assert_eq!(primary_label(&detail), "Resume 7:00");
        detail.episodes[0].user_data = Some(UserData {
            played: true,
            ..UserData::default()
        });
        let mut second = item();
        second.id = "s1e2".into();
        second.user_data = Some(UserData {
            playback_position_ticks: 10_000 * 60_000 * 18 + 10_000 * 41_000,
            ..UserData::default()
        });
        detail.episodes.push(second);
        assert_eq!(primary_label(&detail), "Resume 18:41");
        detail.episodes[1].user_data = None;
        assert_eq!(
            primary_label(&detail),
            "Play",
            "a fresh episode is not resumed"
        );
    }

    #[test]
    fn the_metadata_line_keeps_the_year_rating_runtime_and_three_genres() {
        assert_eq!(
            metadata(&item()),
            "2025 · TV-14 · 48 min · Drama, Mystery, Thriller"
        );
    }

    #[test]
    fn a_collection_can_be_sorted_without_losing_the_server_order() {
        let mut a = item();
        a.id = "a".into();
        a.name = "Zulu".into();
        a.production_year = Some(1999);
        a.run_time_ticks = Some(10_000 * 60_000 * 90);
        let mut b = item();
        b.id = "b".into();
        b.name = "Alpha".into();
        b.production_year = Some(2024);
        b.run_time_ticks = Some(10_000 * 60_000 * 30);
        let items = vec![a, b];
        let ids = |sorted: Vec<Item>| sorted.into_iter().map(|item| item.id).collect::<Vec<_>>();
        assert_eq!(ids(CollectionSort::Server.apply(&items)), ["a", "b"]);
        assert_eq!(ids(CollectionSort::Name.apply(&items)), ["b", "a"]);
        assert_eq!(ids(CollectionSort::Year.apply(&items)), ["b", "a"]);
        assert_eq!(ids(CollectionSort::Runtime.apply(&items)), ["a", "b"]);
        assert_eq!(
            items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"],
            "sorting a copy never rewrites the server's order"
        );
    }

    #[test]
    fn the_time_formatter_switches_to_hours_only_when_it_needs_them() {
        assert_eq!(format_time(42 * 60_000 + 13_000), "42:13");
        assert_eq!(format_time(3_733_000), "1:02:13");
    }
}
