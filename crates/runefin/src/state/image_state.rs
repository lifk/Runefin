//! Poster fetching and the decoded-image map: part of `State`'s impl, kept out of the already
//! large state module.

use super::*;
use crate::net::ImagePriority;

impl State {
    /// Queue one fetch per card. Rune decodes on its own worker; this only gets the bytes.
    pub(crate) fn request_images(&mut self) {
        // This pass is the current screen: replacing the set drops images from the route we
        // left and pins exactly the keys this screen draws.
        self.current_images.clear();
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
            self.image_width(HOME_BACKDROP_WIDTH),
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
                self.image_request(
                    &client,
                    &item.id,
                    tag,
                    ImageKind::Backdrop,
                    self.image_width(640),
                )
            } else if let Some((id, tag, kind)) = crate::home::poster_source(item) {
                self.image_request(&client, id, tag, kind, self.image_width(POSTER_WIDTH))
            } else {
                None
            };
            if let Some(request) = request {
                // A resume's backdrop can be a 0-byte 200 or an HTML error; keep a real
                // poster/still as the fallback so the card is not left as a letter.
                let fallback = crate::home::poster_source(item).and_then(|(id, tag, kind)| {
                    (kind != ImageKind::Backdrop)
                        .then(|| client.image_url(id, kind, self.image_width(640), Some(tag), None))
                });
                requests.push((request.0, request.1, fallback, ImagePriority::High));
            }
        }
        requests
    }

    /// Only rows the virtual grid/list has mounted. Offscreen cards wait for the scroll.
    pub(crate) fn request_library_images_in(&mut self, rows: std::ops::Range<usize>) {
        self.library_image_rows = rows.clone();
        // The mounted rows are the current screen; rows scrolled away are evictable.
        self.current_images.clear();
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
        self.current_images.clear();
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
            let Some((id, tag, kind)) = crate::home::poster_source(item) else {
                continue;
            };
            if let Some(request) =
                self.image_request(&client, id, tag, kind, self.image_width(POSTER_WIDTH))
            {
                // A few items on the live server answer the chosen image with a 0-byte 200;
                // their Backdrop is valid, so a card can still show art instead of a letter.
                let fallback = (kind != ImageKind::Backdrop)
                    .then(|| item.image_tag(ImageKind::Backdrop))
                    .flatten()
                    .map(|tag| {
                        client.image_url(
                            &item.id,
                            ImageKind::Backdrop,
                            self.image_width(POSTER_WIDTH),
                            Some(tag),
                            None,
                        )
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
        let Some((id, tag, kind)) = crate::home::own_image_source(item) else {
            return;
        };
        if let Some((key, url)) = self.image_request(&client, id, tag, kind, self.image_width(480))
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
            let Some(url) =
                client.chapter_image_url(&item.id, at as u32, tag, self.image_width(160))
            else {
                continue;
            };
            requests.push((key, url, None, ImagePriority::Low));
        }
        self.send_images(requests);
    }

    /// The cached still's key, for the screen side of the up-next card.
    pub fn next_episode_image_key(&self, item: &Item) -> Option<String> {
        let (id, tag, kind) = crate::home::own_image_source(item)?;
        Some(crate::image::key(
            &self.config.server_url,
            id,
            kind.as_str(),
            Some(tag),
            self.image_width(480),
        ))
    }

    /// Fetch the trickplay tile under the pointer, at the size this preview will draw. A tile
    /// is a whole sprite (10×10 or 5×5), so a preview never asks for a second thumbnail endpoint.
    ///
    /// The first tile starts at once (a prefetch usually made it unnecessary). A tile change
    /// waits a short beat so a fast drag cannot queue a sprite for every 16 minutes crossed; the
    /// panel shows its loading card with the right time until the resting tile decodes.
    pub(crate) fn ensure_trickplay_tile(&mut self, item: &Item, ms: i64) {
        let min_width = self.in_app_trickplay_min_width();
        let Some((width, info)) = item.trickplay_manifest_for(min_width) else {
            return;
        };
        let Some(key) = self.trickplay_key_for(item, ms, min_width) else {
            return;
        };
        if self.in_app_trickplay_key.as_deref() == Some(key.as_str()) {
            self.trickplay_due = None;
            // A failed fetch clears its key from `requested_images`; the next resting pointer
            // retries instead of leaving a permanently blank preview.
            if !self.images.contains_key(&key) && !self.requested_images.contains(&key) {
                self.fetch_trickplay_tile(item, ms, width, info, key);
            }
            return;
        }
        if self.images.contains_key(&key) || self.requested_images.contains(&key) {
            // A prefetch or an earlier request already has this tile: adopt it now and drop
            // the old sprite, which nothing will draw once the key moves.
            if let Some(previous) = self.in_app_trickplay_key.replace(key)
                && !previous.is_empty()
            {
                self.forget_image(&previous);
            }
            self.trickplay_due = None;
            return;
        }
        if self.in_app_trickplay_key.is_none() {
            self.fetch_trickplay_tile(item, ms, width, info, key);
        } else {
            self.trickplay_due = Some((
                item.clone(),
                ms,
                Instant::now() + Duration::from_millis(140),
            ));
        }
    }

    /// Whether a debounced tile is waiting; the host keeps a wake scheduled while it is.
    pub(crate) fn trickplay_pending(&self) -> bool {
        self.trickplay_due.is_some()
    }

    /// The debounced fetch `poll` runs once the pointer has rested in a new tile.
    pub(crate) fn poll_trickplay(&mut self) -> bool {
        let Some((item, ms, due)) = self.trickplay_due.as_ref() else {
            return false;
        };
        if Instant::now() < *due {
            return false;
        }
        let (item, ms) = (item.clone(), *ms);
        self.trickplay_due = None;
        let min_width = self.in_app_trickplay_min_width();
        if let Some((width, info)) = item.trickplay_manifest_for(min_width)
            && let Some(key) = self.trickplay_key_for(&item, ms, min_width)
        {
            self.fetch_trickplay_tile(&item, ms, width, info, key);
        }
        true
    }

    fn fetch_trickplay_tile(
        &mut self,
        item: &Item,
        ms: i64,
        width: u32,
        info: &jellyfin_api::TrickplayInfo,
        key: String,
    ) {
        let Some(client) = self.client() else {
            return;
        };
        // One sprite is tens of megabytes decoded. Keep only the tile under the pointer so a
        // long scrub cannot accumulate a film's worth of frames.
        if let Some(previous) = self.in_app_trickplay_key.replace(key.clone())
            && previous != key
        {
            self.forget_image(&previous);
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
        // Always return the key, even when it is already requested: the screen pass needs it
        // to re-mark the image as current, or a route revisit leaves the whole screen
        // unprotected and the next trim evicts it into letter placeholders.
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
            self.current_images.insert(key.clone());
            if self.requested_images.insert(key.clone()) {
                if let Some(fallback) = fallback {
                    self.worker.request_image_with_fallback(
                        client.clone(),
                        key,
                        url,
                        fallback,
                        priority,
                    );
                } else if key.contains("-Trickplay-") {
                    self.worker
                        .request_trickplay_image(client.clone(), key, url, priority);
                } else {
                    self.worker
                        .request_image(client.clone(), key, url, priority);
                }
            }
        }
    }

    /// The width to ask for art the layout draws at `base` logical points. At 1x this keeps
    /// the old pixel budget; the window factor multiplies it up to 2x. A 4K screen draws a
    /// 176 pt card at 528 physical pixels, so 720 px still covers it; asking for 1080 made each
    /// decoded poster 8.9 MB and the 256 MB cache held only 28, which is why a 4K Home or
    /// Detail page evicted its own visible art. The cap keeps the decoded set within a sane
    /// memory budget without making the card soft.
    pub(crate) fn image_width(&self, base: u32) -> u32 {
        let cap = base.saturating_mul(2).min(3840);
        ((base as f32 * self.image_scale).round() as u32).clamp(base, cap)
    }

    /// Adopt the native window's physical/logical factor. The width is part of every image
    /// cache key, so a new factor makes the old bytes undrawable: drop them and ask the
    /// current screen again. Runs from the window-geometry sample, never from a handler.
    pub(crate) fn set_image_scale(&mut self, scale: f32) -> bool {
        let scale = scale.clamp(1.0, 4.0);
        if (scale - self.image_scale).abs() < 0.02 {
            return false;
        }
        self.image_scale = scale;
        if self.images.is_empty() && self.requested_images.is_empty() {
            return true;
        }
        self.images.clear();
        self.placeholders.clear();
        self.image_fades.clear();
        self.image_order.clear();
        self.requested_images.clear();
        self.current_images.clear();
        self.request_current_images();
        true
    }

    /// Refetch the visible screen after the scale changed. Every loader is a no-op when its
    /// screen has no data, so this cannot start work for a route that is not showing.
    fn request_current_images(&mut self) {
        match self.route.clone() {
            Route::Home => self.request_images(),
            Route::Library { .. } => {
                let rows = self.library_image_rows.clone();
                if !rows.is_empty() {
                    self.request_library_images_in(rows);
                }
            }
            Route::Detail { .. } => self.request_detail_images(),
            Route::Person { .. } | Route::SeerrPerson { .. } => self.request_person_images(),
            Route::Search { .. } => self.request_search_images(),
            Route::Downloads => self.request_download_images(),
            Route::LiveTv => self.request_live_images(),
            Route::Seerr | Route::SeerrDetail { .. } => self.request_seerr_images(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_widths_follow_the_window_scale_up_to_twice_the_budget() {
        let mut s = State::from_config(crate::config::Config::fresh());
        assert_eq!(s.image_width(360), 360, "1x keeps the desk budget");
        s.set_image_scale(1.5);
        assert_eq!(s.image_width(360), 540, "1080p big screen draws 1.5x");
        s.set_image_scale(3.0);
        assert_eq!(
            s.image_width(360),
            720,
            "4K keeps 2x, not 3x: 720 still covers a 528 px card and a 3x poster is 8.9 MB decoded"
        );
        assert_eq!(s.image_width(1280), 2560, "the Home backdrop caps at 2x");
        assert_eq!(s.image_width(1600), 3200, "the detail backdrop caps at 2x");
    }
}
