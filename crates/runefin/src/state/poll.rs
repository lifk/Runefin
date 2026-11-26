//! The frame tick, in four contiguous phases. They live here rather than in `state.rs` so the
//! order is visible in one file and the state module stays navigable.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::*;

impl State {
    /// `poll` asks this every frame; it returns true only when something visible changed.
    pub fn poll(&mut self) -> bool {
        // Four contiguous passes, in the order the app has always run them. Ingest pulls in what
        // arrived, Derive turns answers into state, React runs timers and follow-ups, and Persist
        // flushes what the frame observed. Keeping the boundaries contiguous preserves the exact
        // order the app had before the split.
        let mut changed = self.poll_ingest();
        changed |= self.poll_derive();
        changed |= self.poll_react();
        changed |= self.poll_persist();
        changed
    }

    /// Phase 1 — Ingest: route changes, boot, deep links, worker answers, tray events and the
    /// first batch of network results. This is where the frame's inputs become state.
    fn poll_ingest(&mut self) -> bool {
        // A route change makes the previous screen's images evictable; the new screen's
        // request pass repopulates the protected set.
        if self.route != self.last_route {
            self.current_images.clear();
            self.last_route = self.route.clone();
            if self.keyboard.open {
                self.keyboard_reset();
            }
        }
        let mut booted = false;
        if self.fetch_home && !self.home_started && self.is_signed_in() {
            self.home_started = true;
            self.load_home();
            booted = true;
        }
        // A link that launched the app waits for sign-in; one that arrived while signed in
        // applies on this frame either way.
        let mut deep_linked = false;
        if self.fetch_home
            && self.is_signed_in()
            && let Some(link) = self.pending_deep_link.take()
        {
            self.apply_deep_link(link);
            deep_linked = true;
        }
        if let Some(item) = self.deep_link_load.take() {
            self.apply_deep_link_item(item);
            deep_linked = true;
        }
        let mut downloads_changed = false;
        if let Some(result) = self.downloads.save.take() {
            self.apply_download_saved(result);
            downloads_changed = true;
        }
        if let Some(result) = self.downloads.commit.take() {
            self.apply_download_commit(result);
            downloads_changed = true;
        }
        if let Some(free) = self.downloads.free_load.take() {
            *self.downloads.free = free;
            downloads_changed = true;
        }
        if let Some(result) = self.downloads.reload.take() {
            self.apply_download_reload(result);
            downloads_changed = true;
        }
        let trickplay_changed = self.poll_trickplay();
        // With the player's chrome up, warm the tile under the playhead before a pointer reaches
        // the timeline. A prefetched tile is already current; a different one waits for a hover.
        if self.in_app.is_some()
            && self.osd_visible
            && self.in_app_hover.is_none()
            && self.in_app_trickplay_key.is_none()
            && let Some(session) = &self.in_app
        {
            let (item, ms) = (session.item.clone(), session.position_ms);
            self.ensure_trickplay_tile(&item, ms);
        }
        let mut revealed = false;
        self.image_fades.retain(|_, started| {
            if started.elapsed() >= Duration::from_millis(120) {
                revealed = true;
                false
            } else {
                true
            }
        });
        let messages = self.worker.drain();
        let mut changed = booted
            || deep_linked
            || downloads_changed
            || trickplay_changed
            || revealed
            || !messages.is_empty();
        let tray_events: Vec<rune::TrayEvent> = self
            .tray_events
            .lock()
            .map(|mut queue| queue.drain(..).collect())
            .unwrap_or_default();
        for event in tray_events {
            match event {
                rune::TrayEvent::Menu(id) => {
                    changed |= match id.as_str() {
                        "playpause" if self.in_app.is_some() => {
                            self.toggle_in_app_pause();
                            true
                        }
                        "stop" if self.in_app.is_some() => {
                            self.stop_in_app();
                            true
                        }
                        "next" if self.in_app.is_some() => {
                            self.play_in_app_next_episode();
                            true
                        }
                        "previous" if self.in_app.is_some() => {
                            self.play_in_app_previous();
                            true
                        }
                        "show" => {
                            self.open_requested = true;
                            true
                        }
                        "quit" => {
                            self.quit_requested = true;
                            true
                        }
                        _ => false,
                    };
                }
                rune::TrayEvent::Activate { .. } => {
                    // A click on the icon means "bring the app forward", the one thing a tray
                    // icon must do. It was ignored.
                    self.open_requested = true;
                    changed = true;
                }
                _ => {}
            }
        }
        for message in messages {
            self.apply(message);
        }
        changed
    }

    /// Phase 2 — Derive: the rest of the network answers, home/library merges, timers and the
    /// small derived reads that follow from them.
    fn poll_derive(&mut self) -> bool {
        let mut changed = false;
        self.trim_image_cache();
        if let Some(watching) = self.home_watching.take() {
            changed = true;
            match watching {
                Ok(items) => self.apply_watching(items),
                Err(message) => self.home_failed(message),
            }
        }
        if let Some(rails) = self.home_rails.take() {
            changed = true;
            match rails {
                Ok(rails) => self.apply_rails(rails),
                Err(message) => self.home_failed(message),
            }
            // The slowest home query (337 ms against this server), and its rail is the last one:
            // asking for it after the rails land keeps it out of the first paint's way.
            self.load_recently_played();
            self.load_upcoming();
        }
        if let Some((items, shuffle)) = self.queue_load.take() {
            if !items.is_empty() {
                let at = if shuffle {
                    0
                } else {
                    crate::state::in_app_state::first_unplayed(&items)
                };
                self.play_queue(items, at, shuffle);
            }
            changed = true;
        }
        if let Some(added) = self.playlist_add_load.take() {
            self.apply_playlist_add(added);
            changed = true;
        }
        if let Some(choices) = self.collection_choices_load.take() {
            self.collection_choices_loading = false;
            self.collection_choices = choices.unwrap_or_default();
            changed = true;
        }
        if let Some(added) = self.collection_add_load.take() {
            self.apply_collection_add(added);
            changed = true;
        }
        if let Some(created) = self.playlist_create_load.take() {
            self.apply_playlist_create(created);
            changed = true;
        }
        if let Some(removed) = self.playlist_remove_load.take() {
            self.apply_playlist_remove(removed);
            changed = true;
        }
        if let Some(playlists) = self.playlists_load.take()
            && self.is_signed_in()
        {
            self.playlists = playlists.unwrap_or_default();
            changed = true;
        }
        if let Some(measured) = self.loudness_load.take()
            && let (Some(item), Some(measured)) = (self.loudness_for.take(), measured)
        {
            crate::loudness::store(
                &mut self.loudness_cache,
                &item,
                crate::loudness::Stored::from(measured),
            );
            if self.loudness
                && self
                    .in_app
                    .as_ref()
                    .is_some_and(|session| session.item_id == item)
            {
                self.apply_loudness();
            }
            changed = true;
        }
        if let Some(result) = self.subtitle_sync.take() {
            self.apply_subtitle_sync(result);
            changed = true;
        }
        if self.in_app_subtitles.syncing {
            // The panel shows how much of the window is decoded; one frame when it moves.
            let progress = self
                .subtitle_sync_progress
                .load(std::sync::atomic::Ordering::Relaxed);
            if progress != self.subtitle_sync_seen {
                self.subtitle_sync_seen = progress;
                changed = true;
            }
        }
        if let Some(next_up) = self.home_next_up.take() {
            if self.is_signed_in() {
                self.home_data.next_up = next_up;
                self.request_images();
                self.save_home_snapshot();
            }
            changed = true;
        }
        if let Some(upcoming) = self.home_upcoming.take() {
            if self.is_signed_in() {
                self.home_data.upcoming = upcoming;
                self.request_images();
                self.save_home_snapshot();
            }
            changed = true;
        }
        if let Some(on_now) = self.home_on_now.take() {
            if self.is_signed_in() {
                self.home_data.on_now = on_now;
                self.request_images();
                self.save_home_snapshot();
            }
            changed = true;
        }
        if let Some(played) = self.home_recently_played.take() {
            if self.is_signed_in() {
                self.home_data.recently_played =
                    crate::home::merge_recently_played(played, &self.home_data.resume);
                self.request_images();
                self.save_home_snapshot();
            }
            changed = true;
        }
        if let Some(collections) = self.home_collections.take() {
            if self.is_signed_in() {
                self.home_data.collections = collections;
                self.request_images();
                self.save_home_snapshot();
            }
            changed = true;
        }
        if let Some(page) = self.library_page.take() {
            self.apply_library_page(page);
            changed = true;
        }
        if let Some((id, item)) = self.hover_preview_load.take()
            && let Some(preview) = &mut self.hover_preview
            && preview.item_id == id
        {
            preview.item = item;
            changed = true;
        }
        if let Some(facets) = self.library_facets.take() {
            if let Some(library) = &mut self.library {
                library.studios = facets.studios;
                library.tags = facets.tags;
                library.facets_loading = false;
            }
            changed = true;
        }
        if let Some(counts) = self.library_counts.take() {
            if let Some(library) = &mut self.library {
                library.unwatched_total = Some(counts.unwatched);
                library.missing_total = Some(counts.missing);
                library.unaired_total = Some(counts.unaired);
            }
            changed = true;
        }
        if let Some(genres) = self.library_genres.take() {
            self.apply_library_genres(genres);
            changed = true;
        }
        if self.last_reminder_check.elapsed() >= Duration::from_secs(1) {
            self.last_reminder_check = Instant::now();
            if self.poll_live_reminders() {
                changed = true;
            }
        }
        if self
            .toast
            .until
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            *self.toast.message = None;
            *self.toast.undo = None;
            *self.toast.until = None;
            changed = true;
        }
        if let Some(sessions) = self.cast_sessions_load.take() {
            self.cast_loading = false;
            match sessions {
                Ok(sessions) => self.cast_sessions = sessions,
                Err(message) => {
                    self.cast_sessions.clear();
                    self.show_toast(message);
                }
            }
            changed = true;
        }
        if let Some(started) = self.theme_audio_load.take() {
            let wanted = self.theme_audio_starting;
            self.theme_audio_starting = false;
            match started {
                Ok(player) if !wanted => player.stop(),
                Ok(player) => {
                    if let Some(old) = self.theme_audio.replace(player) {
                        old.stop();
                    }
                    self.theme_audio_title = self
                        .detail_theme_songs
                        .first()
                        .map(|item| item.name.clone());
                }
                Err(message) => self.show_toast(format!("Theme song: {message}")),
            }
            changed = true;
        }
        if let Some(started) = self.theme_video_load.take() {
            let wanted = self.theme_video_starting;
            self.theme_video_starting = false;
            match started {
                Ok(player) if wanted && !self.reduce_motion => {
                    if let Some(old) = self.theme_video.replace(player) {
                        old.stop();
                    }
                    if let Some(player) = &self.theme_video {
                        player.toggle_mute();
                    }
                    self.theme_video_on = true;
                }
                Ok(player) => player.stop(),
                Err(message) => self.show_toast(format!("Theme video: {message}")),
            }
            changed = true;
        }
        if let Some(sent) = self.cast_send_load.take() {
            match sent {
                Ok(()) => {
                    if self.transfer_after_cast {
                        self.transfer_after_cast = false;
                        self.stop_in_app();
                        self.show_toast("Continuing on the other device.");
                    } else {
                        self.show_toast("Sent to the other device.");
                    }
                }
                Err(message) => {
                    self.transfer_after_cast = false;
                    self.show_toast(format!("Could not send it: {message}"));
                }
            }
            changed = true;
        }
        if let Some(servers) = self.discovery_load.take() {
            self.discovered = servers;
            self.discovering = false;
            changed = true;
        }
        if let Some(hints) = self.search_hints.take() {
            self.search.hints_loading = false;
            self.search.hints = hints.unwrap_or_default();
            changed = true;
        }
        if let Some(page) = self.detail_page.take() {
            self.apply_detail_page(page);
            changed = true;
        }
        if let Some(theme) = self.detail_theme_load.take() {
            let keep = self.detail.as_ref().is_some_and(|detail| {
                self.detail_theme_for.as_deref() == Some(detail.item.id.as_str())
            });
            if keep {
                match theme {
                    Ok(theme) => {
                        self.detail_theme_songs = theme
                            .theme_songs_result
                            .map(|result| result.items)
                            .unwrap_or_default();
                        self.detail_theme_videos = theme
                            .theme_videos_result
                            .map(|result| result.items)
                            .unwrap_or_default();
                    }
                    Err(_) => {
                        self.detail_theme_songs.clear();
                        self.detail_theme_videos.clear();
                    }
                }
            }
            changed = true;
        }
        if let Some(collections) = self.detail_collections_load.take() {
            let keep = self.detail.as_ref().is_some_and(|detail| {
                self.detail_collections_for.as_deref() == Some(detail.item.id.as_str())
            });
            if keep {
                self.detail_collections = collections.unwrap_or_default();
            }
            changed = true;
        }
        if let Some(extras) = self.detail_extras_load.take() {
            let keep = self.detail.as_ref().is_some_and(|detail| {
                self.detail_extras_for.as_deref() == Some(detail.item.id.as_str())
            });
            if keep {
                self.detail_extras = extras.unwrap_or_default();
            }
            changed = true;
        }
        if let Some(rating) = self.detail_imdb_load.take() {
            self.apply_imdb_rating(rating);
            changed = true;
        }
        if let Some(trailer) = self.trailer_load.take() {
            self.apply_trailer_download(trailer);
            changed = true;
        }
        if let Some(ratings) = self.imdb_ratings_load.take() {
            self.apply_imdb_ratings(ratings);
            changed = true;
        }
        if let Some(similar) = self.detail_similar_load.take() {
            let belongs = self.detail_similar_for.take().is_some_and(|id| {
                self.detail
                    .as_ref()
                    .is_some_and(|detail| detail.item.id == id)
            });
            if belongs
                && self.is_signed_in()
                && let Some(detail) = &mut self.detail
            {
                detail.similar = similar.unwrap_or_default();
                self.request_detail_images();
            }
            changed = true;
        }
        if let Some(load) = self.person_latest.take() {
            self.apply_person(load);
            changed = true;
        }
        if let Some(load) = self.seerr_person_load.take() {
            self.apply_seerr_person(load);
            changed = true;
        }
        if let Some(page) = self.detail_episodes.take() {
            self.apply_episodes_page(page);
            changed = true;
        }
        if let Some(started) = self.playback_start.take() {
            self.apply_playback_start(started);
            changed = true;
        }
        if let Some(result) = self.seerr_search_global.take() {
            self.apply_search_seerr(result);
            changed = true;
        }
        if let Some(search) = self.search_latest.take() {
            self.apply_search(search);
            changed = true;
        }
        if let Some(probe) = self.seerr_probe.take() {
            self.apply_seerr_probe(probe);
            changed = true;
        }
        changed
    }

    /// Phase 3 — React: the Seerr/Quick/player state machines, live and in-app results, and the
    /// follow-ups that start from them.
    fn poll_react(&mut self) -> bool {
        let mut changed = false;
        if self.poll_seerr() {
            changed = true;
        }
        if let Some(remote) = self.remote_latest.take() {
            self.apply_remote(remote);
            changed = true;
        }
        if let Some(channels) = self.live_latest.take() {
            self.apply_live_channels(channels);
            changed = true;
        }
        if let Some(started) = self.quick_start.take() {
            self.apply_quick_start(started);
            changed = true;
        }
        if let Some(polled) = self.quick_poll.take() {
            self.apply_quick_poll(polled);
            changed = true;
        }
        if let Some(auth) = self.quick_auth.take() {
            self.apply_quick_auth(auth);
            changed = true;
        }
        if self.poll_quick() {
            changed = true;
        }
        if let Some(opened) = self.live_open.take() {
            self.apply_live_open(opened);
            changed = true;
        }
        // Closing a tuner is fire-and-forget for a stop, but a zap is waiting for the close to
        // land before it opens the next stream: the server counts tuners, not channels.
        if self.live_close.take().is_some()
            && let Some(channel) = self.pending_zap.take()
        {
            self.play_live_channel(channel);
            changed = true;
        }
        if let Some(started) = self.in_app_start.take() {
            self.apply_in_app_start(started);
            changed = true;
        }
        if let Some(segments) = self.in_app_segments_load.take() {
            self.apply_in_app_segments(segments);
            changed = true;
        }
        if let Some(streams) = self.in_app_streams_load.take() {
            self.apply_in_app_playback_streams(streams);
            changed = true;
        }
        if let Some(found) = self.in_app_subtitle_search_load.take() {
            self.apply_in_app_subtitle_search(found);
            changed = true;
        }
        if let Some(downloaded) = self.in_app_subtitle_download_load.take() {
            self.apply_in_app_subtitle_download(downloaded);
            changed = true;
        }
        if let Some(cues) = self.in_app_subtitle_load.take() {
            self.apply_in_app_subtitle_cues(cues);
            changed = true;
        }
        if let Some(image) = self.in_app_image_load.take() {
            self.apply_in_app_image_subtitle_style(image);
            changed = true;
        }
        if let Some(neighbours) = self.in_app_neighbours_load.take() {
            self.apply_in_app_neighbours(neighbours);
            changed = true;
        }
        if self.poll_in_app() {
            changed = true;
        }
        if self.sync_image_subtitles() {
            changed = true;
        }
        if self.home_loaded && self.in_app.is_none() && self.restore_pending_place() {
            changed = true;
        }
        changed
    }

    /// Phase 4 — Persist: the frame's external observers (image cache, tray tooltip, MPRIS).
    fn poll_persist(&mut self) -> bool {
        let mut changed = false;
        if self.image_dirty.swap(false, Ordering::Relaxed) {
            changed = true;
        }
        let tooltip = self
            .in_app
            .as_ref()
            .map(|session| session.item.name.clone())
            .unwrap_or_else(|| "Runefin".into());
        if tooltip != self.tray_tooltip
            && let Some(tray) = &self.tray
        {
            tray.set_tooltip(tooltip.clone());
            self.tray_tooltip = tooltip;
        }
        if self.sync_mpris() {
            changed = true;
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_toast_write_goes_through_its_tracked_cell() {
        let mut state = State::from_config(crate::config::Config::fresh());
        let before = rune::Tracked::version(&state.toast.message);
        state.show_toast("Saved");
        assert!(
            rune::Tracked::version(&state.toast.message) > before,
            "writing the toast bumps its cell"
        );
        assert_eq!(state.toast.message.as_deref(), Some("Saved"));
    }
}
