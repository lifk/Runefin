//! Poster fetching and the decoded-image map: part of `State`'s impl, kept out of the already
//! large state module.

use super::*;
use crate::net::ImagePriority;

impl State {
    /// Queue one fetch per card. Rune decodes on its own worker; this only gets the bytes.
    pub(crate) fn request_images(&mut self) {
        let mut requests = self.resume_images();
        if let Some(request) = self.home_backdrop_request() {
            requests.push(request);
        }
        // Every card in a horizontal rail, not just the first screenful: scrolling the rail
        // must not expose empty boxes until the reader clicks one.
        requests.extend(
            self.image_requests(
                self.home_data
                    .latest
                    .iter()
                    .flat_map(|rail| rail.items.iter()),
                ImagePriority::Low,
            ),
        );
        requests.extend(self.image_requests(self.home_data.next_up.iter(), ImagePriority::Low));
        requests.extend(self.image_requests(self.home_data.on_now.iter(), ImagePriority::Low));
        requests
            .extend(self.image_requests(self.home_data.recently_played.iter(), ImagePriority::Low));
        requests.extend(self.image_requests(self.home_data.collections.iter(), ImagePriority::Low));
        self.send_images(requests);
    }

    fn home_backdrop_request(&self) -> Option<(String, String, Option<String>, ImagePriority)> {
        self.image_cache.as_ref()?;
        let client = self.client()?;
        let item = self
            .hero_item()
            .filter(|item| item.image_tag(ImageKind::Backdrop).is_some())
            .or_else(|| {
                self.home_data
                    .latest
                    .iter()
                    .flat_map(|rail| rail.items.iter())
                    .find(|item| item.image_tag(ImageKind::Backdrop).is_some())
            })?;
        let tag = item.image_tag(ImageKind::Backdrop)?;
        let request = self.image_request(
            &client,
            &item.id,
            tag,
            ImageKind::Backdrop,
            HOME_BACKDROP_WIDTH,
        )?;
        Some((request.0, request.1, None, ImagePriority::Low))
    }

    fn resume_images(&self) -> Vec<(String, String, Option<String>, ImagePriority)> {
        let Some(client) = self.client() else {
            return Vec::new();
        };
        let mut requests = Vec::new();
        for item in self.home_data.resume.iter() {
            let request = if let Some(tag) = item.image_tag(ImageKind::Backdrop) {
                self.image_request(&client, &item.id, tag, ImageKind::Backdrop, 640)
            } else if let Some((id, tag)) = crate::home::poster(item) {
                self.image_request(&client, id, tag, ImageKind::Primary, POSTER_WIDTH)
            } else {
                None
            };
            if let Some(request) = request {
                requests.push((request.0, request.1, None, ImagePriority::High));
            }
        }
        requests
    }

    /// Only rows the virtual grid/list has mounted. Offscreen cards wait for the scroll.
    pub(crate) fn request_library_images_in(&mut self, rows: std::ops::Range<usize>) {
        let requests = match &self.library {
            Some(library) => {
                let (start, end) = match library.view {
                    ViewMode::Grid => (rows.start * library.columns, rows.end * library.columns),
                    ViewMode::List => (rows.start, rows.end),
                };
                let start = start.min(library.items.len());
                let end = end.min(library.items.len());
                self.image_requests(library.items[start..end].iter(), ImagePriority::High)
            }
            None => Vec::new(),
        };
        self.send_images(requests);
    }

    pub(super) fn request_search_images(&mut self) {
        let requests = self.image_requests(
            self.search
                .results
                .movies
                .iter()
                .take(6)
                .chain(self.search.results.shows.iter().take(6))
                .chain(self.search.results.episodes.iter().take(6)),
            ImagePriority::High,
        );
        self.send_images(requests);
    }

    pub(super) fn image_requests<'a>(
        &self,
        items: impl Iterator<Item = &'a Item>,
        priority: ImagePriority,
    ) -> Vec<(String, String, Option<String>, ImagePriority)> {
        if self.image_cache.is_none() {
            return Vec::new();
        }
        let Some(client) = self.client() else {
            return Vec::new();
        };
        let mut requests = Vec::new();
        for item in items {
            let Some((id, tag)) = crate::home::poster(item) else {
                continue;
            };
            if let Some(request) =
                self.image_request(&client, id, tag, ImageKind::Primary, POSTER_WIDTH)
            {
                // A few items on the live server answer the Primary image with a 0-byte 200;
                // their Backdrop is valid, so a card can still show art instead of a letter.
                let fallback = item.image_tag(ImageKind::Backdrop).map(|tag| {
                    client.image_url(&item.id, ImageKind::Backdrop, POSTER_WIDTH, Some(tag), None)
                });
                requests.push((request.0, request.1, fallback, priority));
            }
        }
        requests
    }

    /// The next episode's still, for the up-next card. A 16:9 still at 480 px covers the
    /// card's thumbnail at 2× without pulling a full backdrop.
    pub(super) fn request_next_episode_image(&mut self, item: &Item) {
        let Some(client) = self.client() else {
            return;
        };
        let Some(tag) = item.image_tag(ImageKind::Primary) else {
            return;
        };
        if let Some((key, url)) =
            self.image_request(&client, &item.id, tag, ImageKind::Primary, 480)
        {
            self.send_images(vec![(key, url, None, ImagePriority::High)]);
        }
    }

    /// Chapter stills for the chapter list. They are small, low priority and capped, so a long
    /// film cannot flood the image worker before the poster/backdrop work is done.
    pub(super) fn request_chapter_images(&mut self, item: &Item) {
        let Some(client) = self.client() else {
            return;
        };
        if self.image_cache.is_none() {
            return;
        }
        let mut requests = Vec::new();
        for (at, chapter) in item.chapters.iter().take(16).enumerate() {
            let Some(tag) = chapter.image_tag.as_deref() else {
                continue;
            };
            let Some(key) = self.chapter_image_key(item, at) else {
                continue;
            };
            if self.requested_images.contains(&key) {
                continue;
            }
            let Some(url) = client.chapter_image_url(&item.id, at as u32, tag, 160) else {
                continue;
            };
            requests.push((key, url, None, ImagePriority::Low));
        }
        self.send_images(requests);
    }

    /// The cached still's key, for the screen side of the up-next card.
    pub fn next_episode_image_key(&self, item: &Item) -> Option<String> {
        Some(crate::image::key(
            &self.config.server_url,
            &item.id,
            "Primary",
            Some(item.image_tag(ImageKind::Primary)?),
            480,
        ))
    }

    /// Fetch only the trickplay tile under the pointer. A tile is a 10×10 sprite, so a preview
    /// never asks the server for a second thumbnail endpoint.
    pub(crate) fn ensure_trickplay_tile(&mut self, item: &Item, ms: i64) {
        let Some(client) = self.client() else {
            return;
        };
        let Some((width, info)) = item.trickplay_manifest() else {
            return;
        };
        let Some(key) = self.trickplay_key(item, ms) else {
            return;
        };
        if self.images.contains_key(&key) || self.requested_images.contains(&key) {
            return;
        }
        // A 640-wide tile is a 6400×3600 sprite. Keep only the tile under the pointer so
        // scrubbing a long film cannot accumulate hundreds of megabytes of decoded frames.
        if let Some(previous) = self.in_app_trickplay_key.replace(key.clone())
            && previous != key
        {
            self.images.remove(&previous);
            self.placeholders.remove(&previous);
            self.image_fades.remove(&previous);
            self.image_order.retain(|old| old != &previous);
            // Forgetting this let a tile evicted for memory never load again on a second pass.
            self.requested_images.remove(&previous);
        }
        let tile = info.tile_of(info.thumbnail_at(ms));
        if let Some(url) = client.trickplay_url(&item.id, width, tile) {
            self.send_images(vec![(key, url, None, ImagePriority::High)]);
        }
    }

    pub(super) fn image_request(
        &self,
        client: &Client,
        id: &str,
        tag: &str,
        kind: ImageKind,
        width: u32,
    ) -> Option<(String, String)> {
        let key = crate::image::key(&self.config.server_url, id, kind.as_str(), Some(tag), width);
        if self.requested_images.contains(&key) {
            return None;
        }
        Some((key, client.image_url(id, kind, width, Some(tag), None)))
    }

    pub(super) fn send_images(
        &mut self,
        requests: Vec<(String, String, Option<String>, ImagePriority)>,
    ) {
        if requests.is_empty() {
            return;
        }
        let Some(client) = self.client() else {
            return;
        };
        for (key, url, fallback, priority) in requests {
            if self.requested_images.insert(key.clone()) {
                self.current_images.insert(key.clone());
                if let Some(fallback) = fallback {
                    self.worker.request_image_with_fallback(
                        client.clone(),
                        key,
                        url,
                        fallback,
                        priority,
                    );
                } else {
                    self.worker
                        .request_image(client.clone(), key, url, priority);
                }
            }
        }
    }
}
