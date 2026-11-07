//! One person's filmography: the server's `personIds` credit query, opened from a cast card or
//! a search result, plus the same person's TMDB credits from Seerr. Back restores the page it
//! came from, including its nested state.

use std::collections::HashSet;

use super::*;

pub struct PersonPage {
    /// The server's person id. Empty when only Seerr knows the person.
    pub id: String,
    pub name: String,
    pub image_tag: Option<String>,
    /// The TMDB profile path, when Seerr supplied the person.
    pub profile_path: Option<String>,
    pub tmdb_id: Option<u64>,
    pub biography: Option<String>,
    pub birthday: Option<String>,
    pub place_of_birth: Option<String>,
    pub items: Vec<Item>,
    /// TMDB ids of the library titles, so a Seerr credit already on the server can be skipped.
    pub library_tmdb: HashSet<u64>,
    pub seerr_credits: Vec<seerr_api::Credit>,
    pub loading: bool,
    pub error: Option<String>,
    pub seerr_loading: bool,
    pub seerr_error: Option<String>,
}

impl PersonPage {
    fn empty(name: String) -> Self {
        Self {
            id: String::new(),
            name,
            image_tag: None,
            profile_path: None,
            tmdb_id: None,
            biography: None,
            birthday: None,
            place_of_birth: None,
            items: Vec::new(),
            library_tmdb: HashSet::new(),
            seerr_credits: Vec::new(),
            loading: true,
            error: None,
            seerr_loading: false,
            seerr_error: None,
        }
    }

    /// The credits Seerr knows that are not already on the server, newest first.
    pub fn seerr_only(&self) -> Vec<&seerr_api::Credit> {
        let mut credits: Vec<&seerr_api::Credit> = self
            .seerr_credits
            .iter()
            .filter(|credit| !self.library_tmdb.contains(&credit.id))
            .filter(|credit| credit.is_movie() || credit.is_tv())
            .collect();
        // The best-rated credits first: a TMDB filmography includes talk shows and
        // self-appearances, and a pure date sort puts those in front of the films.
        // TMDB can list one series once per character; the rail shows a title once.
        let mut seen = HashSet::new();
        credits.retain(|credit| {
            seen.insert((credit.media_type.clone().unwrap_or_default(), credit.id))
        });
        credits.sort_by(|a, b| {
            b.vote_average
                .unwrap_or(0.0)
                .total_cmp(&a.vote_average.unwrap_or(0.0))
                .then_with(|| b.year().unwrap_or("").cmp(a.year().unwrap_or("")))
                .then_with(|| a.title().cmp(b.title()))
        });
        credits
    }
}

pub(super) struct PersonLoad {
    pub(super) meta: Item,
    pub(super) items: Vec<Item>,
}

pub(super) struct PersonBundle {
    pub(super) person: seerr_api::PersonDetails,
    pub(super) credits: seerr_api::PersonCredits,
    pub(super) jellyfin: Option<PersonLoad>,
}

fn tmdb_id_of(item: &Item) -> Option<u64> {
    item.provider_ids.get("Tmdb").and_then(|id| id.parse().ok())
}

impl State {
    /// Open a person from a detail page or search. Opening from a detail parks that page so Back
    /// can restore it; opening from anywhere else returns to that route.
    pub fn open_person(&mut self, person: &jellyfin_api::Person) {
        self.person_return = self.route.clone();
        self.person_detail = if matches!(self.route, Route::Detail { .. }) {
            self.detail.take()
        } else {
            None
        };
        self.route = Route::Person {
            id: person.id.clone(),
        };
        let mut page = PersonPage::empty(person.name.clone());
        page.id = person.id.clone();
        page.image_tag = person.primary_image_tag.clone();
        self.person = Some(page);
        self.load_person(person.id.clone());
    }

    /// Open a person from a Seerr cast rail. The TMDB id is the only identity we start with; the
    /// server's person is found by name and checked against the same TMDB id.
    pub fn open_seerr_person(&mut self, tmdb_id: u64, name: String, profile_path: Option<String>) {
        self.person_return = self.route.clone();
        self.person_detail = None;
        self.route = Route::SeerrPerson { tmdb_id };
        let mut page = PersonPage::empty(name.clone());
        page.profile_path = profile_path;
        page.tmdb_id = Some(tmdb_id);
        self.person = Some(page);
        self.load_seerr_person(tmdb_id, name, true);
    }

    pub fn load_person(&mut self, id: String) {
        let client = self.client();
        let Some(page) = &mut self.person else {
            return;
        };
        page.loading = true;
        page.error = None;
        let Some(client) = client else {
            page.loading = false;
            page.error = Some("Your session expired. Sign in again.".into());
            return;
        };
        self.request_person_portrait();
        self.person_latest.request(id, move |id| {
            let meta = client.item(&id).map_err(|error| error.user_message())?;
            let items = client
                .person_items(&id, 120)
                .map_err(|error| error.user_message())?;
            Ok(PersonLoad { meta, items })
        });
    }

    /// The Seerr half. `resolve_library` is true when the page started from a TMDB id and has to
    /// find the server's person by name; a page opened from a cast card already has it.
    fn load_seerr_person(&mut self, tmdb_id: u64, name: String, resolve_library: bool) {
        let Some(seerr) = self.seerr_client() else {
            if let Some(page) = &mut self.person {
                page.seerr_error = Some("Seerr is not configured.".into());
            }
            return;
        };
        let jellyfin = self.client();
        let Some(page) = &mut self.person else {
            return;
        };
        page.seerr_loading = true;
        page.seerr_error = None;
        self.seerr_person_load.request(
            (tmdb_id, name.clone(), resolve_library),
            move |(tmdb_id, name, resolve_library)| {
                let person = seerr
                    .person(tmdb_id)
                    .map_err(|error| error.user_message())?;
                let credits = seerr
                    .person_credits(tmdb_id)
                    .map_err(|error| error.user_message())?;
                let jellyfin = if resolve_library {
                    jellyfin.and_then(|client| {
                        let found = client.find_person(&name).ok().flatten()?;
                        if tmdb_id_of(&found) != Some(tmdb_id) {
                            return None;
                        }
                        let items = client.person_items(&found.id, 120).ok()?;
                        Some(PersonLoad { meta: found, items })
                    })
                } else {
                    None
                };
                Ok(PersonBundle {
                    person,
                    credits,
                    jellyfin,
                })
            },
        );
    }

    /// The cast rail cached a 160 px portrait; the page header uses 240.
    fn request_person_portrait(&mut self) {
        let Some(client) = self.client() else {
            return;
        };
        let portrait = self.person.as_ref().and_then(|person| {
            person
                .image_tag
                .as_ref()
                .map(|tag| (person.id.clone(), tag.clone()))
        });
        if let Some((id, tag)) = portrait
            && let Some((key, url)) =
                self.image_request(&client, &id, &tag, jellyfin_api::ImageKind::Primary, 240)
        {
            self.send_images(vec![(key, url, None, crate::net::ImagePriority::High)]);
        }
    }

    pub fn retry_person(&mut self) {
        let Some(page) = &self.person else {
            return;
        };
        if page.id.is_empty() {
            if let Some(tmdb) = page.tmdb_id {
                let name = page.name.clone();
                self.load_seerr_person(tmdb, name, true);
            }
        } else {
            let id = page.id.clone();
            self.load_person(id);
        }
    }

    pub(super) fn apply_person(&mut self, load: Result<PersonLoad, String>) {
        let Some(page) = &mut self.person else {
            return;
        };
        page.loading = false;
        let mut refresh_portrait = false;
        match load {
            Ok(load) => {
                page.items = load.items;
                if page.name.is_empty() {
                    page.name = load.meta.name.clone();
                }
                if page.image_tag.is_none() {
                    page.image_tag = load
                        .meta
                        .image_tags
                        .as_ref()
                        .and_then(|tags| tags.get("Primary").cloned());
                    refresh_portrait = true;
                }
                page.tmdb_id = tmdb_id_of(&load.meta);
                page.biography = load.meta.overview.clone();
                page.error = None;
                page.library_tmdb = page.items.iter().filter_map(tmdb_id_of).collect();
                let tmdb = page.tmdb_id;
                let name = page.name.clone();
                let requests = self.image_requests(
                    self.person.iter().flat_map(|p| p.items.iter()),
                    crate::net::ImagePriority::High,
                );
                self.send_images(requests);
                if let Some(tmdb) = tmdb
                    && self.seerr_configured()
                {
                    self.load_seerr_person(tmdb, name, false);
                }
            }
            Err(message) => page.error = Some(message),
        }
        if refresh_portrait {
            self.request_person_portrait();
        }
    }

    pub(super) fn apply_seerr_person(&mut self, load: Result<PersonBundle, String>) {
        let Some(page) = &mut self.person else {
            return;
        };
        page.seerr_loading = false;
        match load {
            Ok(bundle) => {
                if page.name.is_empty() {
                    page.name = bundle.person.name.clone().unwrap_or_default();
                }
                if page.biography.is_none() {
                    page.biography = bundle.person.biography.clone();
                }
                page.birthday = bundle.person.birthday.clone();
                page.place_of_birth = bundle.person.place_of_birth.clone();
                if page.profile_path.is_none() {
                    page.profile_path = bundle.person.profile_path.clone();
                }
                page.seerr_credits = bundle.credits.all().cloned().collect();
                if let Some(load) = bundle.jellyfin {
                    page.id = load.meta.id.clone();
                    page.image_tag = load
                        .meta
                        .image_tags
                        .as_ref()
                        .and_then(|tags| tags.get("Primary").cloned());
                    page.tmdb_id = tmdb_id_of(&load.meta).or(page.tmdb_id);
                    page.items = load.items;
                    page.library_tmdb = page.items.iter().filter_map(tmdb_id_of).collect();
                }
                page.seerr_error = None;
                let requests = self.image_requests(
                    self.person.iter().flat_map(|p| p.items.iter()),
                    crate::net::ImagePriority::High,
                );
                self.send_images(requests);
                self.request_person_images();
            }
            Err(message) => page.seerr_error = Some(message),
        }
    }

    /// The portrait the page should show: the server's image when it has one, otherwise the
    /// TMDB profile.
    pub fn person_portrait_key(&self) -> Option<String> {
        let page = self.person.as_ref()?;
        if let Some(tag) = &page.image_tag {
            return Some(crate::image::key(
                &self.config.server_url,
                &page.id,
                "Primary",
                Some(tag),
                240,
            ));
        }
        let tmdb = page.tmdb_id?;
        let path = page.profile_path.as_deref()?;
        Some(self.seerr_profile_key(tmdb, path, 240))
    }

    /// The poster of one Seerr credit, for the "more on Seerr" rail.
    pub fn credit_poster_key(&self, credit: &seerr_api::Credit) -> Option<String> {
        let path = credit.poster_path.as_deref()?;
        let media = credit.media_type.as_deref().unwrap_or("movie");
        Some(self.seerr_poster_key(media, credit.id, path, 342))
    }

    /// The TMDB profile and the poster of every Seerr credit on the page.
    fn request_person_images(&mut self) {
        if self.image_cache.is_none() {
            return;
        }
        let Some(page) = &self.person else {
            return;
        };
        let mut requests = Vec::new();
        if let (Some(tmdb), Some(path)) = (page.tmdb_id, page.profile_path.as_deref()) {
            let key = self.seerr_image_key("person", tmdb, "Profile", path, 240);
            requests.push((
                key,
                Self::tmdb_url(240, path),
                crate::net::ImagePriority::High,
            ));
        }
        for credit in &page.seerr_credits {
            if let Some(path) = &credit.poster_path {
                let media = credit.media_type.as_deref().unwrap_or("movie");
                let key = self.seerr_image_key(media, credit.id, "Poster", path, 342);
                requests.push((
                    key,
                    Self::tmdb_url(342, path),
                    crate::net::ImagePriority::Low,
                ));
            }
        }
        for (key, url, priority) in requests {
            if self.requested_images.insert(key.clone()) {
                self.worker.request_public_image(key, url, priority);
            }
        }
    }

    /// Close the filmography. A credit opened from it returns here first; a second Back then
    /// restores the detail page that opened the person.
    pub fn back_from_person(&mut self) {
        self.person_latest.cancel();
        self.seerr_person_load.cancel();
        self.person = None;
        let route = std::mem::replace(&mut self.person_return, Route::Home);
        if matches!(route, Route::Detail { .. })
            && let Some(detail) = self.person_detail.take()
        {
            self.route = route;
            self.detail = Some(detail);
            self.request_detail_images();
            return;
        }
        self.route = route;
        match &self.route {
            Route::Home => self.request_images(),
            Route::Search { .. } => self.request_search_images(),
            Route::Library { .. } => {
                if let Some(library) = &self.library
                    && let Some(y) = self.library_scrolls.get(&library.id).copied()
                {
                    self.pending_scroll = Some(y);
                }
            }
            _ => {}
        }
    }
}
