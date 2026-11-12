//! Detail-page state: loading the item, its seasons and episodes, and returning to the card.

use super::*;

impl State {
    /// Open a detail page from a card, remembering where to return. A card for an episode opens
    /// its series, which is the page with seasons and the next-up row.
    pub fn open_detail(&mut self, id: String) {
        self.detail_match_id = None;
        self.open_detail_inner(id);
    }

    /// A card opened a detail page: its poster flies to the page's poster as one shared element
    /// (`El::matched`). The id names the card's rail too, because the same series can sit in two
    /// rails and Rune matches the last id in tree order.
    pub fn open_detail_from_card(&mut self, id: String, match_id: String) {
        self.open_detail_inner(id);
        self.detail_match_id = Some(match_id);
    }

    fn open_detail_inner(&mut self, id: String) {
        if matches!(self.route, Route::Detail { .. }) {
            // Opening a collection member from the collection page: remember the parent so
            // Back returns to it instead of skipping to the library.
            if let Some(parent) = self.detail.take() {
                let parent_return = std::mem::replace(&mut self.detail_return, Route::Home);
                self.detail_stack.push((parent, parent_return));
            }
        } else {
            self.detail_return = self.route.clone();
            self.detail_stack.clear();
        }
        self.capture_library_scroll();
        self.route = Route::Detail { id: id.clone() };
        self.remember_place();
        self.detail = Some(DetailState::loading(&id));
        self.detail_notice = None;
        self.detail_imdb = None;
        self.detail_imdb_for = None;
        self.detail_imdb_load.cancel();
        self.detail_extras.clear();
        self.detail_extras_for = None;
        self.detail_extras_load.cancel();
        self.detail_collections.clear();
        self.detail_collections_for = None;
        self.detail_collections_load.cancel();
        self.detail_collections.clear();
        self.detail_collections_for = None;
        self.detail_collections_load.cancel();
        self.stop_theme_media();
        self.detail_theme_songs.clear();
        self.detail_theme_videos.clear();
        self.detail_theme_for = None;
        self.detail_theme_load.cancel();
        self.preplay_open = false;
        self.close_collection_picker();
        self.detail_overview_open = false;
        self.detail_tech_open = false;
        self.detail_page.cancel();
        self.detail_episodes.cancel();
        self.seerr.series = None;
        self.seerr.series_error = None;
        self.seerr_series_for = None;
        self.seerr_series_load.cancel();
        self.load_detail(id);
    }

    pub fn retry_detail(&mut self) {
        if let Some(id) = self
            .detail
            .as_ref()
            .map(|detail| detail.item.id.clone())
            .filter(|id| !id.is_empty())
        {
            self.open_detail(id);
        }
    }

    /// Back from detail to the library or home screen, restoring the library's viewport.
    pub fn back_from_detail(&mut self) {
        self.capture_library_scroll();
        // Child-page theme and extras must not decorate the parent behind it.
        self.stop_theme_media();
        self.detail_theme_songs.clear();
        self.detail_theme_videos.clear();
        self.detail_theme_for = None;
        self.detail_theme_load.cancel();
        self.detail_extras.clear();
        self.detail_extras_for = None;
        self.detail_extras_load.cancel();
        self.detail_collections.clear();
        self.detail_collections_for = None;
        self.detail_collections_load.cancel();
        if let Some((parent, parent_return)) = self.detail_stack.pop() {
            let id = parent.item.id.clone();
            self.detail = Some(parent);
            self.detail_return = parent_return;
            self.route = Route::Detail { id };
            self.detail_page.cancel();
            self.detail_similar_load.cancel();
            self.detail_similar_for = None;
            self.detail_episodes.cancel();
            self.detail_notice = None;
            self.close_collection_picker();
            self.detail_overview_open = false;
            self.detail_tech_open = false;
            self.request_detail_images();
            self.request_imdb_rating();
            return;
        }
        let route = std::mem::replace(&mut self.detail_return, Route::Home);
        self.route = route.clone();
        self.detail = None;
        self.detail_page.cancel();
        self.detail_episodes.cancel();
        self.detail_notice = None;
        self.detail_overview_open = false;
        self.detail_tech_open = false;
        match route {
            Route::Library { id } => {
                if let Some(y) = self.library_scrolls.get(&id).copied() {
                    self.pending_scroll = Some(y);
                }
            }
            Route::Home => {
                self.library = None;
                self.request_images();
            }
            Route::Search { .. } => self.request_search_images(),
            _ => {}
        }
    }

    fn load_detail(&mut self, id: String) {
        if !self.fetch_home {
            return;
        }
        let Some(client) = self.client() else {
            return;
        };
        self.detail_similar_for = Some(id.clone());
        self.detail_extras_for = Some(id.clone());
        self.detail_collections_for = Some(id.clone());
        let collections_client = client.clone();
        self.detail_collections_load.request(id.clone(), move |id| {
            collections_client
                .item_collections(&id)
                .map_err(|error| error.user_message())
        });
        self.detail_theme_for = Some(id.clone());
        let theme_client = client.clone();
        self.detail_theme_load.request(id.clone(), move |id| {
            theme_client
                .theme_media(&id)
                .map_err(|error| error.user_message())
        });
        let extras_client = client.clone();
        self.detail_extras_load.request(id.clone(), move |id| {
            extras_client
                .special_features(&id)
                .map_err(|error| error.user_message())
        });
        let similar_client = client.clone();
        self.detail_similar_load.request(id.clone(), move |id| {
            similar_client
                .similar(&id, 12)
                .map(|items| items.into_iter().take(12).collect())
                .map_err(|error| error.user_message())
        });
        self.detail_page
            .request(id, move |id| match crate::detail::load(&client, &id) {
                Ok(page) => page,
                Err(error) => DetailPage {
                    item: Item {
                        id,
                        ..Item::default()
                    },
                    seasons: Vec::new(),
                    episodes: Vec::new(),
                    next_up: None,
                    collection: Vec::new(),
                    local_trailers: Vec::new(),
                    error: Some(error),
                },
            });
    }

    pub(super) fn apply_detail_page(&mut self, page: DetailPage) {
        let Some(detail) = &mut self.detail else {
            return;
        };
        detail.loading = false;
        if let Some(error) = page.error {
            detail.error = Some(error);
            return;
        }
        detail.item = page.item;
        detail.seasons = page.seasons;
        detail.active_season = detail.seasons.first().map(|season| season.id.clone());
        detail.episodes = page.episodes;
        detail.next_up = page.next_up;
        detail.collection = page.collection;
        detail.local_trailers = page.local_trailers;
        detail.error = None;
        self.request_detail_images();
        self.request_imdb_rating();
        self.request_seerr_series();
    }

    /// IMDb comes from the public dataset; if that download fails and the reader configured an
    /// OMDb key, it is a fallback. TMDb and RT arrive with the item and never wait for either.
    fn request_imdb_rating(&mut self) {
        self.detail_imdb = None;
        self.detail_imdb_for = None;
        let Some(detail) = &self.detail else {
            return;
        };
        let Some(imdb) = detail.item.provider_ids.get("Imdb").cloned() else {
            return;
        };
        self.detail_imdb_for = Some(imdb.clone());
        if let Some(ratings) = &self.imdb_ratings {
            self.detail_imdb = ratings.rating(&imdb);
            return;
        }
        if self.imdb_ratings_loading {
            return;
        }
        let Some(client) = self.client() else {
            return;
        };
        self.imdb_ratings_loading = true;
        self.imdb_ratings_load
            .request((), move |()| crate::imdb::load(&client));
    }

    fn request_omdb_rating(&mut self) {
        let Some(imdb) = self.detail_imdb_for.clone() else {
            return;
        };
        let Some(key) = self
            .config
            .omdb_api_key
            .clone()
            .filter(|key| !key.trim().is_empty())
        else {
            return;
        };
        let Some(client) = self.client() else {
            return;
        };
        self.detail_imdb_load.request(imdb.clone(), move |id| {
            crate::ratings::imdb_rating(&client, &key, &id).map(|rating| (id, rating))
        });
    }

    pub(super) fn apply_imdb_ratings(&mut self, load: Result<crate::imdb::ImdbRatings, String>) {
        self.imdb_ratings_loading = false;
        match load {
            Ok(ratings) => {
                self.imdb_ratings = Some(ratings);
                if let Some(id) = self.detail_imdb_for.clone() {
                    self.detail_imdb = self.ratings_for(&id);
                }
            }
            Err(error) => {
                self.imdb_ratings_error = Some(error);
                self.request_omdb_rating();
            }
        }
    }

    fn ratings_for(&self, imdb: &str) -> Option<f64> {
        self.imdb_ratings
            .as_ref()
            .and_then(|ratings| ratings.rating(imdb))
    }

    pub(super) fn apply_imdb_rating(&mut self, load: Result<(String, Option<f64>), String>) {
        match load {
            Ok((id, rating)) if self.detail_imdb_for.as_deref() == Some(&id) => {
                self.detail_imdb = rating;
            }
            Ok(_) => {}
            Err(_) => self.detail_imdb = None,
        }
    }

    pub fn toggle_detail_tech(&mut self) {
        self.detail_tech_open = !self.detail_tech_open;
    }

    pub fn select_season(&mut self, season: String) {
        let changed = match &mut self.detail {
            Some(detail) if detail.active_season.as_deref() != Some(season.as_str()) => {
                detail.active_season = Some(season.clone());
                detail.episodes.clear();
                detail.loading_episodes = true;
                true
            }
            _ => false,
        };
        if changed {
            self.load_episodes(season);
        }
    }

    fn load_episodes(&mut self, season: String) {
        if !self.fetch_home {
            return;
        }
        let Some(client) = self.client() else {
            return;
        };
        self.detail_episodes.request(season, move |season| {
            crate::detail::load_episodes(&client, &season)
        });
    }

    pub(super) fn apply_episodes_page(&mut self, page: EpisodesPage) {
        let Some(detail) = &mut self.detail else {
            return;
        };
        if detail.active_season.as_deref() != Some(page.season.as_str()) {
            return;
        }
        detail.loading_episodes = false;
        if let Some(error) = page.error {
            detail.error = Some(error);
        } else {
            detail.episodes = page.items;
            detail.error = None;
        }
    }

    /// Play an episode row directly from the series page.
    pub fn play_episode(&mut self, item: Item) {
        self.play_in_app(item);
    }

    pub fn toggle_detail_overview(&mut self) {
        self.detail_overview_open = !self.detail_overview_open;
    }

    pub fn toggle_favorite(&mut self) {
        let Some(client) = self.client() else {
            return;
        };
        let Some(detail) = &mut self.detail else {
            return;
        };
        let Some(user) = &mut detail.item.user_data else {
            return;
        };
        let was = user.is_favorite;
        user.is_favorite = !was;
        let favorite = user.is_favorite;
        let item_id = detail.item.id.clone();
        self.worker.set_favorite(client, item_id.clone(), favorite);
        self.show_undo_toast(
            if favorite {
                "Added to favourites."
            } else {
                "Removed from favourites."
            },
            crate::state::UndoAction::Favorite {
                item_id,
                value: was,
            },
        );
    }

    /// Open the playlist picker for one item. The list is already in memory from the sidebar
    /// load; opening it never starts a request. It works from a detail page or a card menu.
    pub fn open_playlist_picker_for(&mut self, item: Item) {
        self.playlist_picker_ids = Some(vec![item.id.clone()]);
        self.playlist_picker_item = Some(item);
        self.playlist_picker_open = true;
    }

    /// A bulk selection uses the same picker and add path.
    pub fn open_playlist_picker_for_ids(&mut self, ids: Vec<String>) {
        if ids.is_empty() {
            return;
        }
        self.playlist_picker_item = None;
        self.playlist_picker_ids = Some(ids);
        self.playlist_picker_open = true;
    }

    /// The detail page's Playlist button: open for this item, or close if it is already open.
    pub fn toggle_playlist_picker(&mut self) {
        if self.playlist_picker_open {
            self.close_playlist_picker();
            return;
        }
        if let Some(item) = self.detail.as_ref().map(|detail| detail.item.clone()) {
            self.open_playlist_picker_for(item);
        }
    }

    pub fn close_playlist_picker(&mut self) {
        self.playlist_picker_open = false;
        self.playlist_picker_item = None;
        self.playlist_picker_ids = None;
    }

    /// Reorder the visible collection. The server's order stays in `detail.collection`.
    pub fn set_collection_sort(&mut self, sort: crate::detail::CollectionSort) {
        if let Some(detail) = &mut self.detail {
            detail.collection_sort = sort;
        }
    }

    /// The item the collection picker will add: the card that opened it, or the detail item.
    fn collection_target(&self) -> Option<Item> {
        self.collection_picker_item.clone().or_else(|| {
            self.detail
                .as_ref()
                .map(|detail| detail.item.clone())
                .filter(|item| !item.id.is_empty())
        })
    }

    pub fn open_collection_picker_for(&mut self, item: Item) {
        self.collection_picker_ids = Some(vec![item.id.clone()]);
        self.collection_picker_item = Some(item);
        self.collection_picker_open = true;
        self.load_collection_choices();
    }

    /// A bulk selection uses the same picker and add path.
    pub fn open_collection_picker_for_ids(&mut self, ids: Vec<String>) {
        if ids.is_empty() {
            return;
        }
        self.collection_picker_item = None;
        self.collection_picker_ids = Some(ids);
        self.collection_picker_open = true;
        self.load_collection_choices();
    }

    pub fn toggle_collection_picker(&mut self) {
        if self.collection_picker_open {
            self.close_collection_picker();
            return;
        }
        if let Some(item) = self.detail.as_ref().map(|detail| detail.item.clone()) {
            self.open_collection_picker_for(item);
        }
    }

    pub fn close_collection_picker(&mut self) {
        self.collection_picker_open = false;
        self.collection_picker_item = None;
        self.collection_picker_ids = None;
    }

    /// The picker lists every box set the server knows, not only the twelve on Home.
    fn load_collection_choices(&mut self) {
        if self.collection_choices_loading {
            return;
        }
        let Some(client) = self.client() else {
            return;
        };
        self.collection_choices_loading = true;
        self.collection_choices_load.request((), move |()| {
            client
                .collections(200)
                .map_err(|error| error.user_message())
        });
    }

    /// Add the item(s) to a box set. The server owns membership; the picker closes and a toast
    /// names the collection, so the action has a visible result.
    pub fn add_detail_to_collection(&mut self, collection_id: String, collection_name: String) {
        let Some(client) = self.client() else {
            self.show_toast("Your session expired. Sign in again.");
            return;
        };
        let ids = self.collection_picker_ids.clone().unwrap_or_else(|| {
            self.collection_target()
                .map(|item| vec![item.id])
                .unwrap_or_default()
        });
        if ids.is_empty() {
            self.close_collection_picker();
            return;
        }
        self.collection_picker_open = false;
        self.library_selected.clear();
        self.collection_add_pending = Some((collection_id.clone(), collection_name.clone()));
        self.collection_add_load
            .request(collection_id.clone(), move |collection_id| {
                client
                    .add_to_collection(&collection_id, &ids)
                    .map_err(|error| error.user_message())
            });
        self.show_toast(format!("Adding to {collection_name}…"));
    }

    pub(crate) fn apply_collection_add(&mut self, load: Result<(), String>) {
        match load {
            Ok(()) => {
                if let Some((_, name)) = self.collection_add_pending.take() {
                    self.show_toast(format!("Added to {name}."));
                } else {
                    self.show_toast("Added to the collection.");
                }
            }
            Err(message) => {
                self.collection_add_pending = None;
                self.show_toast(format!("Could not add to the collection: {message}"));
            }
        }
    }

    /// The "New playlist" prompt. It starts empty and focused, and the item on this page is the
    /// first entry, so the flow is picker → name → done.
    pub fn open_playlist_create(&mut self) {
        self.playlist_picker_open = false;
        self.playlist_create_open = true;
        self.playlist_new_name = TextEdit::new("");
        self.playlist_new_name.focused = true;
    }

    pub fn close_playlist_create(&mut self) {
        self.playlist_create_open = false;
    }

    pub fn create_playlist_with_detail(&mut self) {
        let name = self.playlist_new_name.text.trim().to_string();
        if name.is_empty() {
            self.detail_notice = Some("Give the playlist a name first.".into());
            return;
        }
        let Some(client) = self.client() else {
            self.detail_notice = Some("Your session expired. Sign in again.".into());
            return;
        };
        let Some(item) = self.playlist_target() else {
            return;
        };
        if !crate::state::is_playable(&item) {
            self.show_toast("This page is a container; open one of its members first.");
            return;
        }
        self.playlist_create_open = false;
        let ids = vec![item.id.clone()];
        self.playlist_create_load
            .request(name.clone(), move |name| {
                let id = client
                    .create_playlist(&name, &ids)
                    .map_err(|error| error.user_message())?;
                Ok(Item {
                    id,
                    name,
                    kind: "Playlist".into(),
                    ..Item::default()
                })
            });
        self.show_toast("Creating the playlist…");
    }

    pub(super) fn apply_playlist_create(&mut self, load: Result<Item, String>) {
        match load {
            Ok(playlist) => {
                self.show_toast(format!("Created {}.", playlist.name));
                self.playlists.push(playlist);
                self.playlists.sort_by(|a, b| {
                    a.name
                        .to_ascii_lowercase()
                        .cmp(&b.name.to_ascii_lowercase())
                });
            }
            Err(message) => self.show_toast(format!("Could not create it: {message}")),
        }
    }

    /// The item the picker was opened for: a card menu's item, or the detail page's.
    fn playlist_target(&self) -> Option<Item> {
        self.playlist_picker_item.clone().or_else(|| {
            self.detail
                .as_ref()
                .map(|detail| detail.item.clone())
                .filter(|item| !item.id.is_empty())
        })
    }

    /// Append the picked item to a playlist. The server appends; the picker closes and a toast
    /// says where it went, so the action has a visible result on any screen.
    pub fn add_detail_to_playlist(&mut self, playlist_id: String, playlist_name: String) {
        let Some(client) = self.client() else {
            self.show_toast("Your session expired. Sign in again.");
            return;
        };
        let Some(item) = self.playlist_target() else {
            return;
        };
        if !crate::state::is_playable(&item) {
            self.show_toast("This page is a container; add one of its members instead.");
            self.playlist_picker_open = false;
            return;
        }
        self.playlist_picker_open = false;
        let ids = vec![item.id.clone()];
        self.playlist_add_pending =
            Some((playlist_id.clone(), item.id.clone(), playlist_name.clone()));
        self.playlist_add_load
            .request(playlist_id.clone(), move |playlist_id| {
                client
                    .add_to_playlist(&playlist_id, &ids)
                    .map_err(|error| error.user_message())
            });
        self.show_toast(format!("Adding to {playlist_name}…"));
    }

    pub(crate) fn apply_playlist_add(&mut self, load: Result<(), String>) {
        match load {
            Ok(()) => {
                if let Some((playlist_id, item_id, name)) = self.playlist_add_pending.take() {
                    self.show_undo_toast(
                        format!("Added to {name}."),
                        crate::state::UndoAction::PlaylistAdd {
                            playlist_id,
                            item_id,
                        },
                    );
                } else {
                    self.show_toast("Added to the playlist.");
                }
            }
            Err(message) => {
                self.playlist_add_pending = None;
                self.show_toast(format!("Could not add to the playlist: {message}"));
            }
        }
    }

    /// Mark every episode currently loaded for this season watched. One server write per
    /// episode, all off-thread; the rows update at once.
    pub fn mark_detail_season_watched(&mut self) {
        let Some(client) = self.client() else {
            return;
        };
        let Some(detail) = &mut self.detail else {
            return;
        };
        if detail.episodes.is_empty() {
            return;
        }
        let ids: Vec<String> = detail
            .episodes
            .iter()
            .filter(|episode| !episode.user_data.as_ref().is_some_and(|user| user.played))
            .map(|episode| episode.id.clone())
            .collect();
        if ids.is_empty() {
            self.show_toast("This season is already watched.");
            return;
        }
        for id in &ids {
            self.worker.set_played(client.clone(), id.clone(), true);
        }
        for episode in &mut detail.episodes {
            episode
                .user_data
                .get_or_insert_with(jellyfin_api::UserData::default)
                .played = true;
        }
        self.show_toast(format!("Marked {} episode(s) watched.", ids.len()));
    }

    pub fn toggle_played(&mut self) {
        let Some(client) = self.client() else {
            return;
        };
        let Some(detail) = &mut self.detail else {
            return;
        };
        let Some(user) = &mut detail.item.user_data else {
            return;
        };
        let was = user.played;
        user.played = !was;
        let played = user.played;
        let item_id = detail.item.id.clone();
        if played {
            self.forget_checkpoint(&item_id);
        }
        self.worker.set_played(client, item_id.clone(), played);
        self.show_undo_toast(
            if played {
                "Marked watched."
            } else {
                "Marked unwatched."
            },
            crate::state::UndoAction::Played {
                item_id,
                value: was,
                resume: None,
            },
        );
    }

    pub(super) fn request_detail_images(&mut self) {
        let Some(detail) = &self.detail else {
            return;
        };
        let Some(client) = self.client() else {
            return;
        };
        let mut requests = Vec::new();
        let priority = crate::net::ImagePriority::High;
        if let Some((id, tag)) = crate::home::poster(&detail.item)
            && let Some(request) =
                self.image_request(&client, id, tag, ImageKind::Primary, DETAIL_POSTER_WIDTH)
        {
            let fallback = detail.item.image_tag(ImageKind::Backdrop).map(|tag| {
                client.image_url(
                    &detail.item.id,
                    ImageKind::Backdrop,
                    DETAIL_POSTER_WIDTH,
                    Some(tag),
                    None,
                )
            });
            requests.push((request.0, request.1, fallback, priority));
        }
        if let Some(tag) = detail.item.image_tag(ImageKind::Backdrop)
            && let Some(request) = self.image_request(
                &client,
                &detail.item.id,
                tag,
                ImageKind::Backdrop,
                BACKDROP_WIDTH,
            )
        {
            requests.push((request.0, request.1, None, priority));
        }
        if let Some(tag) = detail.item.image_tag(ImageKind::Logo)
            && let Some(request) =
                self.image_request(&client, &detail.item.id, tag, ImageKind::Logo, 520)
        {
            requests.push((request.0, request.1, None, priority));
        }
        if let Some(detail) = &self.detail {
            // The same width the card draws from (`image_key`), or the fetched bytes would be
            // cached under a key no card ever looks up.
            for item in detail.similar.iter().take(12) {
                if let Some(tag) = item.image_tag(ImageKind::Primary)
                    && let Some(request) =
                        self.image_request(&client, &item.id, tag, ImageKind::Primary, POSTER_WIDTH)
                {
                    // This page is what the reader is looking at; the home rails that asked
                    // for their own art are behind it now, so a Low here would wait behind
                    // dozens of stale fetches.
                    requests.push((request.0, request.1, None, crate::net::ImagePriority::High));
                }
            }
            for member in detail.collection.iter() {
                if let Some(tag) = member.image_tag(ImageKind::Primary)
                    && let Some(request) = self.image_request(
                        &client,
                        &member.id,
                        tag,
                        ImageKind::Primary,
                        POSTER_WIDTH,
                    )
                {
                    requests.push((request.0, request.1, None, crate::net::ImagePriority::High));
                }
            }
            for episode in detail.episodes.iter() {
                if let Some(tag) = episode.image_tag(ImageKind::Primary)
                    && let Some(request) =
                        self.image_request(&client, &episode.id, tag, ImageKind::Primary, 320)
                {
                    requests.push((request.0, request.1, None, crate::net::ImagePriority::High));
                }
            }
            if let Some(next) = &detail.next_up
                && let Some(tag) = next.image_tag(ImageKind::Primary)
                && let Some(request) =
                    self.image_request(&client, &next.id, tag, ImageKind::Primary, 320)
            {
                requests.push((request.0, request.1, None, crate::net::ImagePriority::High));
            }
            for person in detail.item.people.iter().take(12) {
                if let Some(tag) = person.primary_image_tag.as_deref()
                    && let Some(request) =
                        self.image_request(&client, &person.id, tag, ImageKind::Primary, 160)
                {
                    requests.push((request.0, request.1, None, crate::net::ImagePriority::High));
                }
            }
        }
        self.send_images(requests);
    }

    /// The cache key for a detail poster, and for the backdrop behind it.
    pub fn detail_image_key(&self, item: &Item) -> Option<String> {
        let (id, tag) = crate::home::poster(item)?;
        Some(crate::image::key(
            &self.config.server_url,
            id,
            "Primary",
            Some(tag),
            DETAIL_POSTER_WIDTH,
        ))
    }

    pub fn detail_backdrop_key(&self, item: &Item) -> Option<String> {
        let tag = item.image_tag(ImageKind::Backdrop)?;
        Some(crate::image::key(
            &self.config.server_url,
            &item.id,
            "Backdrop",
            Some(tag),
            BACKDROP_WIDTH,
        ))
    }

    pub fn detail_logo_key(&self, item: &Item) -> Option<String> {
        let tag = item.image_tag(ImageKind::Logo)?;
        Some(crate::image::key(
            &self.config.server_url,
            &item.id,
            "Logo",
            Some(tag),
            520,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jellyfin_api::ItemQuery;
    use std::time::{Duration, Instant};

    /// End to end against the live server: create a playlist from a real movie's page, see it
    /// in the sidebar list, then delete it again. Ignored by default because it writes.
    #[test]
    fn similar_posters_are_fetched_at_the_size_the_cards_look_up() {
        let mut config = crate::config::Config::fresh();
        config.server_url = "http://127.0.0.1:9".into();
        config.user_id = "probe".into();
        config.token = "probe".into();
        let mut state = State::from_config(config);
        let mut detail = crate::demo::movie_detail();
        for item in &mut detail.similar {
            item.image_tags = Some(std::collections::HashMap::from([(
                "Primary".to_string(),
                format!("tag-{}", item.id),
            )]));
        }
        let first = detail.similar[0].clone();
        state.detail = Some(detail);
        state.request_detail_images();
        let drawn = state.image_key(&first).expect("the card has a key");
        assert!(
            state.requested_images.contains(&drawn),
            "the request is queued under the key the card looks up"
        );
    }

    #[test]
    #[ignore = "needs a signed-in config; creates and deletes a playlist"]
    fn a_playlist_created_from_a_detail_page_is_listed_and_can_be_deleted() {
        let config = crate::config::Config::load();
        assert!(config.has_session(), "sign in once before this test");
        let client = Client::new(&config.server_url, config.device_id.clone())
            .with_session(config.token.clone(), config.user_id.clone());
        let movie = client
            .items(&ItemQuery {
                recursive: true,
                include_types: vec!["Movie"],
                limit: 1,
                ..ItemQuery::default()
            })
            .expect("a movie")
            .items
            .into_iter()
            .next()
            .expect("one movie on the server");
        let mut state = State::from_config(config);
        let waker: rune::Waker = std::sync::Arc::new(|| {});
        state.setup(waker);
        let mut detail = crate::demo::movie_detail();
        detail.item = movie.clone();
        detail.collection.clear();
        state.detail = Some(detail);
        state.open_playlist_create();
        state.playlist_new_name = TextEdit::new("Runefin probe");
        state.create_playlist_with_detail();
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline
            && !state
                .playlists
                .iter()
                .any(|playlist| playlist.name == "Runefin probe")
        {
            state.poll();
            std::thread::sleep(Duration::from_millis(50));
        }
        let created = state
            .playlists
            .iter()
            .find(|playlist| playlist.name == "Runefin probe")
            .cloned();
        eprintln!(
            "created playlist: {:?} from {}",
            created.as_ref().map(|playlist| playlist.id.clone()),
            movie.name
        );
        assert!(created.is_some(), "the new playlist reaches the sidebar");
        let created = created.expect("created");
        assert!(
            client
                .playlist_items(&created.id)
                .expect("the playlist's items")
                .iter()
                .any(|item| item.id == movie.id),
            "the page's item is the first entry"
        );
        client.delete_playlist(&created.id).expect("delete");
        assert!(
            client.delete_playlist(&created.id).is_err(),
            "gone for good"
        );
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;
    use jellyfin_api::ItemQuery;
    use std::time::{Duration, Instant};

    /// The picker's add path, live: create an empty playlist, add a movie through the same
    /// state method the picker row calls, then read the server and delete the playlist.
    #[test]
    #[ignore = "needs a signed-in config; creates and deletes a playlist"]
    fn adding_from_the_picker_reaches_the_server() {
        let config = crate::config::Config::load();
        assert!(config.has_session(), "sign in once before this test");
        let client = Client::new(&config.server_url, config.device_id.clone())
            .with_session(config.token.clone(), config.user_id.clone());
        let movie = client
            .items(&ItemQuery {
                recursive: true,
                include_types: vec!["Movie"],
                limit: 1,
                ..ItemQuery::default()
            })
            .expect("a movie")
            .items
            .into_iter()
            .next()
            .expect("one movie on the server");
        let playlist = client
            .create_playlist("Runefin picker probe", &[])
            .expect("create");
        let mut state = State::from_config(config);
        let waker: rune::Waker = std::sync::Arc::new(|| {});
        state.setup(waker);
        state.open_playlist_picker_for(movie.clone());
        state.add_detail_to_playlist(playlist.clone(), "Runefin picker probe".into());
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline
            && state
                .toast
                .as_deref()
                .is_none_or(|toast| !toast.contains("Added"))
        {
            state.poll();
            std::thread::sleep(Duration::from_millis(50));
        }
        let items = client.playlist_items(&playlist).unwrap_or_default();
        let added = items.iter().any(|item| item.id == movie.id);
        eprintln!(
            "picker add: {} entries, contains {:?}: {added}",
            items.len(),
            movie.name
        );
        client.delete_playlist(&playlist).expect("delete");
        assert!(added, "the picker's add reached the playlist");
        assert!(
            state
                .toast
                .as_deref()
                .is_some_and(|toast| toast.contains("Added")),
            "the toast confirms it"
        );
    }

    /// The hero paints from `/Items/{id}`; recommendations arrive beside it. This proves the
    /// two are separate requests and that the rail fills without a reload.
    #[test]
    #[ignore = "needs a signed-in live server"]
    fn a_live_movie_page_streams_its_recommendations() {
        let config = crate::config::Config::load();
        assert!(config.has_session(), "sign in once before this test");
        let client = Client::new(&config.server_url, config.device_id.clone())
            .with_session(config.token.clone(), config.user_id.clone());
        let movie = client
            .items(&ItemQuery {
                recursive: true,
                include_types: vec!["Movie"],
                limit: 1,
                ..ItemQuery::default()
            })
            .expect("a movie")
            .items
            .into_iter()
            .next()
            .expect("one movie on the server");
        let mut state = State::from_config(config);
        let waker: rune::Waker = std::sync::Arc::new(|| {});
        state.setup(waker);
        let started = Instant::now();
        state.open_detail(movie.id.clone());
        let deadline = started + Duration::from_secs(10);
        while Instant::now() < deadline
            && state
                .detail
                .as_ref()
                .is_none_or(|detail| detail.similar.is_empty())
        {
            state.poll();
            std::thread::sleep(Duration::from_millis(50));
        }
        let detail = state.detail.as_ref().expect("a detail page");
        assert!(!detail.item.id.is_empty());
        assert!(
            !detail.similar.is_empty(),
            "the server recommends something after {:?}",
            movie.name
        );
        eprintln!(
            "detail: {:?} painted, {} recommendations by {:?}",
            detail.item.name,
            detail.similar.len(),
            started.elapsed()
        );
    }
}

#[cfg(test)]
mod playlist_removal_tests {
    use super::*;
    use jellyfin_api::ItemQuery;
    use std::time::{Duration, Instant};

    /// Live: make a two-entry playlist, open it as the playlist page, remove one entry through
    /// the same method the card's × calls, then delete the playlist again.
    #[test]
    #[ignore = "needs a signed-in config; creates, edits and deletes a playlist"]
    fn an_entry_can_be_removed_from_a_playlist_page() {
        let config = crate::config::Config::load();
        assert!(config.has_session(), "sign in once before this test");
        let client = Client::new(&config.server_url, config.device_id.clone())
            .with_session(config.token.clone(), config.user_id.clone());
        let movies = client
            .items(&ItemQuery {
                recursive: true,
                include_types: vec!["Movie"],
                limit: 2,
                ..ItemQuery::default()
            })
            .expect("movies")
            .items;
        assert!(movies.len() >= 2, "need two movies");
        let ids: Vec<String> = movies.iter().take(2).map(|item| item.id.clone()).collect();
        let playlist = client
            .create_playlist("Runefin probe", &ids)
            .expect("create");
        let mut state = State::from_config(config);
        let waker: rune::Waker = std::sync::Arc::new(|| {});
        state.setup(waker);
        state.open_playlist(playlist.clone(), "Runefin probe".into());
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline
            && state
                .library
                .as_ref()
                .is_none_or(|library| library.items.len() < 2)
        {
            state.poll();
            std::thread::sleep(Duration::from_millis(50));
        }
        let entry = state
            .library
            .as_ref()
            .and_then(|library| library.items.first())
            .and_then(|item| item.playlist_item_id.clone())
            .expect("the first entry has a playlist entry id");
        eprintln!("removing entry {entry} from {playlist}");
        state.remove_playlist_entry(entry);
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline
            && state
                .library
                .as_ref()
                .is_none_or(|library| library.items.len() != 1)
        {
            state.poll();
            std::thread::sleep(Duration::from_millis(50));
        }
        let left = state
            .library
            .as_ref()
            .map(|library| library.items.len())
            .unwrap_or(0);
        let on_server = client.playlist_items(&playlist).map(|items| items.len());
        client.delete_playlist(&playlist).expect("delete");
        assert_eq!(left, 1, "one entry left after the removal");
        assert_eq!(on_server.expect("read"), 1, "and the server agrees");
    }
}
