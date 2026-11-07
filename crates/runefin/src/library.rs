//! One library's paged grid: what to ask the server for, and the page the view shows while
//! the next one is already in flight.

use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use jellyfin_api::{Item, ItemQuery, SortBy};

/// The design's page size: 100 items per fetch, one page prefetched.
pub const PAGE_SIZE: u32 = 100;

/// Five 172 pt posters plus their gaps fit the 1280 pt window with the sidebar open.
pub const GRID_COLUMNS: usize = 5;

/// A grid row is a poster plus a title that reserves two lines and one metadata line, with a
/// little air before the next row: 264 + 6 + 40 + 6 + 20 = 336, plus 8.
pub const GRID_ROW_H: f32 = 344.0;

/// A list row is one 56 pt thumb plus breathing room.
pub const LIST_ROW_H: f32 = 76.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SortMode {
    Name,
    Added,
    Rating,
    Played,
    Release,
    Runtime,
    PlayCount,
    Random,
}

impl SortMode {
    pub fn label(self) -> &'static str {
        match self {
            SortMode::Name => "Name",
            SortMode::Added => "Added",
            SortMode::Rating => "Rating",
            SortMode::Played => "Played",
            SortMode::Release => "Release",
            SortMode::Runtime => "Runtime",
            SortMode::PlayCount => "Plays",
            SortMode::Random => "Random",
        }
    }

    pub fn sort_by(self) -> SortBy {
        match self {
            SortMode::Name => SortBy::SortName,
            SortMode::Added => SortBy::DateCreated,
            SortMode::Rating => SortBy::CommunityRating,
            SortMode::Played => SortBy::DatePlayed,
            SortMode::Release => SortBy::PremiereDate,
            SortMode::Runtime => SortBy::Runtime,
            SortMode::PlayCount => SortBy::PlayCount,
            SortMode::Random => SortBy::Random,
        }
    }

    pub fn descending(self) -> bool {
        !matches!(self, SortMode::Name | SortMode::Random)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewMode {
    Grid,
    List,
}

#[derive(Debug, Clone)]
pub struct LibraryPage {
    pub start_index: u32,
    pub items: Vec<Item>,
    pub total: i32,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LibraryRequest {
    pub query: ItemQuery,
    pub start: u32,
}

pub struct LibraryState {
    pub id: String,
    pub name: String,
    pub collection_type: Option<String>,
    pub items: Vec<Item>,
    pub total: i32,
    pub sort: SortMode,
    /// Sort direction. Each sort starts with its natural one (A–Z, newest, best rated) and the
    /// toolbar's arrow flips it.
    pub ascending: bool,
    pub view: ViewMode,
    /// Measured from the library body rather than fixed to the default window width.
    pub columns: usize,
    pub unwatched: bool,
    /// The user's favourites only; combines with `unwatched` as an AND filter.
    pub favorites: bool,
    /// The selected genre id and name.
    pub genre: Option<(String, String)>,
    /// Genres offered by this library, once fetched.
    pub genres: Vec<Item>,
    pub genres_loading: bool,
    pub genres_open: bool,
    /// The full filter and sort sheet. The toolbar keeps the two fastest filters visible.
    pub filters_open: bool,
    pub year: Option<i32>,
    pub min_rating: Option<u8>,
    pub audio_language: Option<String>,
    pub subtitle_language: Option<String>,
    pub has_subtitles: bool,
    pub only_4k: bool,
    pub missing_only: bool,
    pub unaired_only: bool,
    /// Type-ahead text, applied after a short idle beat.
    pub search: Option<String>,
    pub letter: Option<String>,
    pub loading: bool,
    /// A one-line result of an action on this page (a failed reorder, for example).
    pub notice: Option<String>,
    pub error: Option<String>,
    /// The first page requests the next one immediately; later pages wait for the scroll.
    pub prefetched: bool,
    /// The viewport position, stored as bits so the view's `on_scroll_offset` can update it.
    pub scroll: Arc<AtomicU32>,
}

impl LibraryState {
    pub fn new(id: String, name: String, collection_type: Option<String>) -> LibraryState {
        LibraryState {
            id,
            name,
            collection_type,
            items: Vec::new(),
            total: 0,
            sort: SortMode::Name,
            ascending: true,
            view: ViewMode::Grid,
            columns: GRID_COLUMNS,
            unwatched: false,
            favorites: false,
            genre: None,
            genres: Vec::new(),
            genres_loading: false,
            genres_open: false,
            filters_open: false,
            year: None,
            min_rating: None,
            audio_language: None,
            subtitle_language: None,
            has_subtitles: false,
            only_4k: false,
            missing_only: false,
            unaired_only: false,
            search: None,
            letter: None,
            loading: false,
            notice: None,
            error: None,
            prefetched: false,
            scroll: Arc::new(AtomicU32::new(0)),
        }
    }

    /// The same filters and order as the grid, but one page wide enough for the whole library.
    /// "Play all" uses it so a run is the library, not the hundred items loaded so far.
    pub fn queue_query(&self) -> ItemQuery {
        let mut query = self.query();
        query.start_index = 0;
        query.limit = if self.total > 0 {
            (self.total as u32).min(5_000)
        } else {
            PAGE_SIZE
        };
        query
    }

    pub fn query(&self) -> ItemQuery {
        if self.id == "__watchlist__" {
            // One list across every library. The server's favourite filter is the watchlist;
            // there is no second, private store to fall out of sync with it.
            let mut query = ItemQuery {
                recursive: true,
                favorites_only: true,
                limit: PAGE_SIZE,
                ..ItemQuery::default()
            };
            query.include_types = vec!["Movie", "Series", "Episode", "BoxSet"];
            query.sort_by = Some(self.sort.sort_by());
            query.descending = !self.ascending;
            query.extra_fields = Some("MediaSources,MediaStreams,DateCreated".into());
            query.unwatched_only = self.unwatched;
            query.genre_ids = self
                .genre
                .as_ref()
                .map(|(id, _)| id.clone())
                .into_iter()
                .collect();
            query.name_starts_with = self.letter.clone();
            query.years = self.year.into_iter().collect();
            query.min_community_rating = self.min_rating.map(f64::from);
            query.audio_languages = self.audio_language.clone().into_iter().collect();
            query.subtitle_languages = self.subtitle_language.clone().into_iter().collect();
            query.has_subtitles = self.has_subtitles;
            query.is_4k = self.only_4k;
            query.missing_only = self.missing_only;
            query.unaired_only = self.unaired_only;
            query.missing_only = self.missing_only;
            query.unaired_only = self.unaired_only;
            query.search = self.search.clone();
            return query;
        }
        let mut query = ItemQuery::in_library(self.id.clone());
        query.include_types = match self.collection_type.as_deref() {
            Some("movies") => vec!["Movie"],
            Some("tvshows") => vec!["Series"],
            Some("boxsets") => vec!["BoxSet"],
            _ => vec!["Movie", "Series", "Episode", "BoxSet"],
        };
        query.sort_by = Some(self.sort.sort_by());
        query.descending = !self.ascending;
        query.limit = PAGE_SIZE;
        query.unwatched_only = self.unwatched;
        query.favorites_only = self.favorites;
        query.genre_ids = self
            .genre
            .as_ref()
            .map(|(id, _)| id.clone())
            .into_iter()
            .collect();
        query.name_starts_with = self.letter.clone();
        query.years = self.year.into_iter().collect();
        query.min_community_rating = self.min_rating.map(f64::from);
        query.audio_languages = self.audio_language.clone().into_iter().collect();
        query.subtitle_languages = self.subtitle_language.clone().into_iter().collect();
        query.has_subtitles = self.has_subtitles;
        query.is_4k = self.only_4k;
        query.missing_only = self.missing_only;
        query.unaired_only = self.unaired_only;
        query.search = self.search.clone();
        // Capability badges need the streams; without this the server omits them and every
        // card would look SDR and subtitle-less.
        query.extra_fields = Some("MediaSources,MediaStreams,DateCreated".into());
        query
    }

    /// Changes only when the built rows' keys or contents may have changed; `virtual_rows`
    /// then skips calling the key function for all 20 000 rows on every commit.
    pub fn version(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.items.len().hash(&mut hasher);
        self.sort.hash(&mut hasher);
        self.view.hash(&mut hasher);
        self.columns.hash(&mut hasher);
        self.unwatched.hash(&mut hasher);
        self.favorites.hash(&mut hasher);
        self.genre.hash(&mut hasher);
        self.year.hash(&mut hasher);
        self.min_rating.hash(&mut hasher);
        self.audio_language.hash(&mut hasher);
        self.subtitle_language.hash(&mut hasher);
        self.has_subtitles.hash(&mut hasher);
        self.only_4k.hash(&mut hasher);
        self.missing_only.hash(&mut hasher);
        self.unaired_only.hash(&mut hasher);
        self.search.hash(&mut hasher);
        self.letter.hash(&mut hasher);
        hasher.finish()
    }

    /// Returns true when the measured width changes the row width.
    pub fn set_columns(&mut self, columns: usize) -> bool {
        let columns = columns.clamp(2, 12);
        if self.columns == columns {
            return false;
        }
        self.columns = columns;
        true
    }

    pub fn total_items(&self) -> usize {
        self.total.max(self.items.len() as i32) as usize
    }

    pub fn has_more(&self) -> bool {
        self.items.len() < self.total_items()
    }

    pub fn scroll_y(&self) -> f32 {
        f32::from_bits(self.scroll.load(Ordering::Relaxed))
    }

    pub fn set_scroll_y(&self, y: f32) {
        self.scroll.store(y.to_bits(), Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_query_names_the_library_types_sort_page_and_letter() {
        let mut library = LibraryState::new("lib".into(), "Movies".into(), Some("movies".into()));
        library.sort = SortMode::Added;
        library.ascending = !library.sort.descending();
        library.unwatched = true;
        library.favorites = true;
        library.genre = Some(("genre-drama".into(), "Drama".into()));
        library.letter = Some("S".into());
        let query = library.query().to_query();
        assert!(query.contains("parentId=lib"));
        assert!(query.contains("includeItemTypes=Movie"));
        assert!(query.contains("sortBy=DateCreated"));
        assert!(query.contains("sortOrder=Descending"));
        assert!(query.contains("limit=100"));
        assert!(query.contains("filters=IsUnplayed%2CIsFavorite"));
        assert!(query.contains("genreIds=genre-drama"));
        assert!(query.contains("nameStartsWith=S"));
    }

    #[test]
    fn a_tv_library_asks_for_series_and_a_bare_folder_asks_for_everything() {
        let tv = LibraryState::new("tv".into(), "Shows".into(), Some("tvshows".into()));
        assert_eq!(tv.query().include_types, vec!["Series"]);
        let folder = LibraryState::new("f".into(), "Files".into(), None);
        assert_eq!(
            folder.query().include_types,
            vec!["Movie", "Series", "Episode", "BoxSet"]
        );
    }

    #[test]
    fn the_sort_direction_is_the_librarys_choice_not_the_sorts() {
        let mut library = LibraryState::new("lib".into(), "Movies".into(), Some("movies".into()));
        assert!(library.ascending, "names start A–Z");
        assert!(
            !library.query().to_query().contains("sortOrder"),
            "ascending is the server's default and stays off the URL"
        );
        library.ascending = false;
        assert!(library.query().to_query().contains("sortOrder=Descending"));

        // Each sort starts from its natural direction, then the arrow flips from there.
        let mut added = LibraryState::new("lib".into(), "Movies".into(), None);
        added.sort = SortMode::Added;
        added.ascending = !added.sort.descending();
        assert!(!added.ascending);
        assert!(added.query().to_query().contains("sortOrder=Descending"));
    }

    #[test]
    fn the_queue_query_covers_the_library_not_one_page() {
        let mut library = LibraryState::new("lib".into(), "Movies".into(), Some("movies".into()));
        library.total = 974;
        library.unwatched = true;
        library.genre = Some(("genre-anime".into(), "Anime".into()));
        let query = library.queue_query().to_query();
        assert!(query.contains("limit=974"), "{query}");
        assert!(
            query.contains("startIndex=0") || !query.contains("startIndex"),
            "{query}"
        );
        assert!(query.contains("filters=IsUnplayed"));
        assert!(query.contains("genreIds=genre-anime"));
        // A huge library is capped rather than asking the server for a million rows.
        library.total = 1_000_000;
        assert!(library.queue_query().to_query().contains("limit=5000"));
    }

    #[test]
    fn favourites_alone_are_a_valid_filter_and_enter_the_version() {
        let mut library = LibraryState::new("lib".into(), "Movies".into(), None);
        let before = library.version();
        library.favorites = true;
        assert!(library.query().to_query().contains("filters=IsFavorite"));
        assert_ne!(library.version(), before);
    }

    #[test]
    fn the_version_changes_with_the_layout_inputs_not_the_scroll() {
        let mut library = LibraryState::new("lib".into(), "Movies".into(), None);
        let before = library.version();
        library.set_scroll_y(900.0);
        assert_eq!(library.version(), before);
        library.sort = SortMode::Rating;
        assert_ne!(library.version(), before);
    }

    #[test]
    fn the_scroll_round_trips_through_the_shared_cell() {
        let library = LibraryState::new("lib".into(), "Movies".into(), None);
        library.set_scroll_y(123.5);
        assert!((library.scroll_y() - 123.5).abs() < f32::EPSILON);
    }
}
