//! The search screen's state: the query field, the scoped search request and the grouped answer.

use rune::TextEdit;

use super::*;
use crate::search::{SearchLoad, SearchResults};

pub struct SearchState {
    pub query: TextEdit,
    pub scope: Option<String>,
    pub results: SearchResults,
    pub searching: bool,
    pub error: Option<String>,
    /// Successful query terms, newest first. This is session-local; no search history is
    /// written to the server or to disk.
    pub recents: Vec<String>,
    /// Rails the reader expanded into a grid, keyed by the rail's key.
    pub expanded: std::collections::HashSet<String>,
    /// `/Search/Hints` names under the field: people, genres and studios included.
    pub hints: Vec<jellyfin_api::SearchHint>,
    pub hints_loading: bool,
    /// The same query against Seerr's TMDB catalogue, so one search covers both.
    pub seerr: seerr_api::SearchResults,
    pub seerr_loading: bool,
    pub seerr_error: Option<String>,
}

impl SearchState {
    pub(super) fn new() -> SearchState {
        SearchState {
            query: TextEdit::new(""),
            scope: None,
            results: SearchResults::default(),
            searching: false,
            error: None,
            recents: Vec::new(),
            expanded: std::collections::HashSet::new(),
            hints: Vec::new(),
            hints_loading: false,
            seerr: seerr_api::SearchResults::default(),
            seerr_loading: false,
            seerr_error: None,
        }
    }
}

impl State {
    /// Open search from anywhere. `scope` is the library for `Ctrl+F`; `None` searches all.
    pub fn open_search(&mut self, scope: Option<String>) {
        self.stop_theme_media();
        if !matches!(self.route, Route::Search { .. }) {
            self.search_return = self.route.clone();
        }
        let changed_scope = self.search.scope != scope;
        self.search.scope = scope;
        self.search.query.focused = true;
        self.search.error = None;
        if changed_scope {
            self.search.results = SearchResults::default();
        }
        self.route = Route::Search {
            scope: self.search.scope.clone(),
        };
        if !self.search.query.text.trim().is_empty() {
            self.search_changed();
        }
    }

    pub fn search_for(&mut self, term: &str) {
        self.search.query = TextEdit::new(term);
        self.search.query.focused = true;
        self.search_changed();
    }

    pub fn clear_search(&mut self) {
        self.search_latest.cancel();
        self.search.query = TextEdit::new("");
        self.search.query.focused = true;
        self.search.results = SearchResults::default();
        self.search.expanded.clear();
        self.search.searching = false;
        self.search.error = None;
        self.search.seerr = seerr_api::SearchResults::default();
        self.search.seerr_loading = false;
        self.search.seerr_error = None;
        self.seerr_search_global.cancel();
    }

    pub fn close_search(&mut self) {
        self.search_latest.cancel();
        self.seerr_search_global.cancel();
        self.search.seerr = seerr_api::SearchResults::default();
        self.search.seerr_loading = false;
        self.search.seerr_error = None;
        self.search.query.focused = false;
        self.search.searching = false;
        self.route = std::mem::replace(&mut self.search_return, Route::Home);
        if matches!(self.route, Route::Home) {
            self.request_images();
        }
    }

    /// Called by the input on every edit; `Latest` supplies the 150 ms debounce and cancels the
    /// previous keystroke's request.
    pub fn search_changed(&mut self) {
        let term = self.search.query.text.trim().to_string();
        self.search.error = None;
        if term.chars().count() < 2 {
            self.search.results = SearchResults::default();
            self.search.expanded.clear();
            self.search.searching = false;
            self.search.hints.clear();
            self.search.hints_loading = false;
            self.search.seerr = seerr_api::SearchResults::default();
            self.search.seerr_loading = false;
            self.search.seerr_error = None;
            self.search_latest.cancel();
            self.search_hints.cancel();
            self.seerr_search_global.cancel();
            return;
        }
        let scope = self.search.scope.clone();
        let Some(client) = self.client() else {
            return;
        };
        self.search.searching = true;
        if let Some(hint_client) = self.client() {
            self.search.hints_loading = true;
            let hint_term = term.clone();
            self.search_hints
                .request(hint_term.clone(), move |hint_term| {
                    hint_client
                        .search_hints(&hint_term, 8)
                        .map_err(|error| error.user_message())
                });
        }
        if self.seerr_configured()
            && let Some(seerr) = self.seerr_client()
        {
            self.search.seerr_loading = true;
            self.search.seerr_error = None;
            let seerr_term = self.search.query.text.trim().to_string();
            self.seerr_search_global.request(seerr_term, move |term| {
                seerr.search(&term, 1).map_err(|error| error.user_message())
            });
        }
        self.search_latest.request(term, move |term| {
            match crate::search::load(&client, &term, scope.as_deref()) {
                Ok(results) => SearchLoad::Ready(results),
                Err(message) => SearchLoad::Failed(message),
            }
        });
    }

    /// One rail at a time is enough; expanding is an in-place alternative to another route.
    pub fn toggle_search_expanded(&mut self, key: &str) {
        if !self.search.expanded.remove(key) {
            self.search.expanded.insert(key.to_string());
        }
    }

    pub fn is_search_expanded(&self, key: &str) -> bool {
        self.search.expanded.contains(key)
    }

    pub(super) fn apply_search(&mut self, load: SearchLoad) {
        self.search.searching = false;
        match load {
            SearchLoad::Ready(results) => {
                self.search.results = results;
                self.search.expanded.clear();
                self.search.error = None;
                self.remember_search();
                self.request_search_images();
            }
            SearchLoad::Failed(message) => self.search.error = Some(message),
        }
    }

    fn remember_search(&mut self) {
        let term = self.search.query.text.trim().to_string();
        if term.chars().count() < 2 {
            return;
        }
        self.search
            .recents
            .retain(|old| !old.eq_ignore_ascii_case(&term));
        self.search.recents.insert(0, term);
        self.search.recents.truncate(6);
    }
}

impl State {
    pub(super) fn apply_search_seerr(&mut self, result: Result<seerr_api::SearchResults, String>) {
        self.search.seerr_loading = false;
        match result {
            Ok(results) => {
                self.search.seerr = results;
                self.search.seerr_error = None;
                self.request_seerr_images();
            }
            Err(message) => self.search.seerr_error = Some(message),
        }
    }
}
