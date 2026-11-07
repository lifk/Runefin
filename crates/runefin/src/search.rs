//! Search results grouped the way Jellyfin's own UI groups them. The server does the matching;
//! this only splits one `/Items?searchTerm=` answer into Movies, Shows, Episodes and People.

use jellyfin_api::{Client, Item, ItemQuery};

#[derive(Debug, Clone, Default)]
pub struct SearchResults {
    pub movies: Vec<Item>,
    pub shows: Vec<Item>,
    pub episodes: Vec<Item>,
    pub people: Vec<Item>,
}

impl SearchResults {
    pub fn is_empty(&self) -> bool {
        self.movies.is_empty()
            && self.shows.is_empty()
            && self.episodes.is_empty()
            && self.people.is_empty()
    }

    fn add(&mut self, item: Item) {
        match item.kind.as_str() {
            "Movie" => self.movies.push(item),
            "Series" | "BoxSet" => self.shows.push(item),
            "Episode" => self.episodes.push(item),
            "Person" => self.people.push(item),
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
pub enum SearchLoad {
    Ready(SearchResults),
    Failed(String),
}

/// `scope` is a library id for `Ctrl+F`; `None` searches the whole server.
pub fn load(client: &Client, term: &str, scope: Option<&str>) -> Result<SearchResults, String> {
    let mut query = ItemQuery::search(term);
    query.limit = 80;
    if let Some(scope) = scope {
        query.parent_id = Some(scope.to_string());
    }
    let items = client
        .items(&query)
        .map_err(|error| error.user_message())?
        .items;
    let mut results = SearchResults::default();
    for item in items {
        results.add(item);
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_land_in_their_own_group() {
        let mut results = SearchResults::default();
        for (id, kind) in [
            ("m", "Movie"),
            ("s", "Series"),
            ("b", "BoxSet"),
            ("e", "Episode"),
            ("p", "Person"),
            ("x", "Studio"),
        ] {
            results.add(Item {
                id: id.into(),
                kind: kind.into(),
                ..Item::default()
            });
        }
        assert_eq!(results.movies.len(), 1);
        assert_eq!(results.shows.len(), 2);
        assert_eq!(results.episodes.len(), 1);
        assert_eq!(results.people.len(), 1);
        assert!(!results.is_empty());
    }
}
