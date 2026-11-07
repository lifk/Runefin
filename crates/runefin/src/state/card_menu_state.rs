//! The card context menu: right-click (or the Menu key) on any poster opens a short list of
//! things that item can do here. It is drawn in-app, because Rune's native popup menus are
//! macOS-only and this app ships on Linux first.

use super::*;

/// An open context menu: the item, and where the pointer asked for it in window points.
#[derive(Debug, Clone)]
pub struct CardMenu {
    pub item: Item,
    pub x: f32,
    pub y: f32,
}

impl State {
    /// Open the item menu and remember the card that opened it, so closing the menu can put
    /// keyboard or controller focus back on that card.
    pub fn open_card_menu_at(&mut self, item: Item, x: f32, y: f32, focus_key: Option<String>) {
        self.card_menu_focus = focus_key;
        self.card_menu = Some(CardMenu { item, x, y });
    }

    pub fn close_card_menu(&mut self) {
        self.card_menu = None;
    }

    /// Open the menu item's own page. An episode opens through its series, exactly like its
    /// poster does.
    pub fn card_menu_open_item(&mut self) {
        let Some(menu) = self.card_menu.take() else {
            return;
        };
        let id = menu
            .item
            .series_id
            .clone()
            .unwrap_or_else(|| menu.item.id.clone());
        self.open_detail(id);
    }

    pub fn card_menu_play(&mut self, from_start: bool) {
        let Some(menu) = self.card_menu.take() else {
            return;
        };
        if !is_playable(&menu.item) {
            let id = menu
                .item
                .series_id
                .clone()
                .unwrap_or_else(|| menu.item.id.clone());
            self.open_detail(id);
            return;
        }
        if from_start {
            self.play_in_app_at(menu.item, Some(0), false);
        } else {
            self.play_in_app(menu.item);
        }
    }

    /// Insert the item right after the one playing. With nothing playing this starts the item,
    /// which is the only useful meaning of "play next" then.
    pub fn card_menu_play_next(&mut self) {
        let Some(menu) = self.card_menu.take() else {
            return;
        };
        if !is_playable(&menu.item) {
            return;
        }
        if self.in_app.is_none() {
            self.play_in_app(menu.item);
            return;
        }
        if self.queue.is_empty()
            && let Some(session) = &self.in_app
        {
            self.queue.push(session.item.clone());
            self.queue_at = 0;
        }
        let at = (self.queue_at + 1).min(self.queue.len());
        self.queue.insert(at, menu.item);
        self.request_images();
    }

    pub fn card_menu_add_to_queue(&mut self) {
        let Some(menu) = self.card_menu.take() else {
            return;
        };
        if !is_playable(&menu.item) {
            return;
        }
        if self.in_app.is_none() {
            self.play_in_app(menu.item);
            return;
        }
        self.queue_add(menu.item);
    }

    /// Flip the favourite flag. The card updates at once; the server call is fire-and-forget,
    /// like every other watch-state write in the app.
    /// Like, dislike or clear. The card's own flag updates at once; the write is a worker call.
    pub fn card_menu_set_like(&mut self, like: Option<bool>) {
        let Some(menu) = self.card_menu.take() else {
            return;
        };
        if !is_playable(&menu.item) {
            return;
        }
        let id = menu.item.id.clone();
        if let Some(client) = self.client() {
            self.worker.set_rating(client, id.clone(), like);
        }
        self.update_item(&id, |item| {
            item.user_data
                .get_or_insert_with(jellyfin_api::UserData::default)
                .likes = like;
        });
        self.show_toast(match like {
            Some(true) => "Liked.",
            Some(false) => "Disliked.",
            None => "Rating cleared.",
        });
    }

    pub fn card_menu_toggle_favorite(&mut self) {
        let Some(menu) = self.card_menu.take() else {
            return;
        };
        let was = menu
            .item
            .user_data
            .as_ref()
            .is_some_and(|user| user.is_favorite);
        let favorite = !was;
        let id = menu.item.id.clone();
        if let Some(client) = self.client() {
            self.worker.set_favorite(client, id.clone(), favorite);
        }
        self.update_item(&id, |item| {
            item.user_data
                .get_or_insert_with(jellyfin_api::UserData::default)
                .is_favorite = favorite;
        });
        self.show_undo_toast(
            if favorite {
                "Added to favourites."
            } else {
                "Removed from favourites."
            },
            crate::state::UndoAction::Favorite {
                item_id: id,
                value: was,
            },
        );
    }

    /// Flip the watched flag. Marking something watched removes its card from Continue Watching
    /// immediately, which is what the viewer just asked for; the next home load agrees.
    pub fn card_menu_toggle_played(&mut self) {
        let Some(menu) = self.card_menu.take() else {
            return;
        };
        let was = menu.item.user_data.as_ref().is_some_and(|user| user.played);
        let played = !was;
        let id = menu.item.id.clone();
        let resume = played
            .then(|| {
                self.home_data
                    .resume
                    .iter()
                    .find(|item| item.id == id)
                    .cloned()
            })
            .flatten();
        if let Some(client) = self.client() {
            self.worker.set_played(client, id.clone(), played);
        }
        self.update_item(&id, |item| {
            item.user_data
                .get_or_insert_with(jellyfin_api::UserData::default)
                .played = played;
        });
        if played {
            self.home_data.resume.retain(|item| item.id != id);
            self.forget_checkpoint(&id);
        }
        self.show_undo_toast(
            if played {
                "Marked watched."
            } else {
                "Marked unwatched."
            },
            crate::state::UndoAction::Played {
                item_id: id,
                value: was,
                resume,
            },
        );
    }

    /// Apply a change to every in-memory copy of an item: rails, library page, detail, queue and
    /// the playing session. Without this a card would look stale until the next reload.
    pub(crate) fn update_item(&mut self, id: &str, mut change: impl FnMut(&mut Item)) {
        for item in self
            .home_data
            .resume
            .iter_mut()
            .chain(self.home_data.next_up.iter_mut())
            .chain(self.home_data.on_now.iter_mut())
            .chain(self.home_data.recently_played.iter_mut())
            .chain(self.home_data.collections.iter_mut())
            .chain(
                self.home_data
                    .latest
                    .iter_mut()
                    .flat_map(|rail| rail.items.iter_mut()),
            )
            .chain(
                self.library
                    .iter_mut()
                    .flat_map(|library| library.items.iter_mut()),
            )
            .chain(self.queue.iter_mut())
            .chain(self.in_app.iter_mut().map(|session| &mut session.item))
        {
            if item.id == id {
                change(item);
            }
        }
        if let Some(detail) = &mut self.detail {
            if detail.item.id == id {
                change(&mut detail.item);
            }
            for episode in detail.episodes.iter_mut() {
                if episode.id == id {
                    change(episode);
                }
            }
            for member in detail.collection.iter_mut() {
                if member.id == id {
                    change(member);
                }
            }
        }
    }

    /// Clear the resume position (and with it the Continue Watching card) without touching the
    /// watched flag. The local card goes at once; the server is told in the background.
    pub fn card_menu_remove_from_resume(&mut self) {
        let Some(menu) = self.card_menu.take() else {
            return;
        };
        self.remove_from_resume(&menu.item.id);
    }

    /// The card's remove button and the item menu both land here. The server write is the same
    /// `UserData` update the menu always used; the local rails react immediately.
    pub fn remove_from_resume(&mut self, id: &str) {
        let item = self
            .home_data
            .resume
            .iter()
            .find(|item| item.id == id)
            .cloned();
        let position_ticks = item
            .as_ref()
            .and_then(|item| item.user_data.as_ref())
            .map_or(0, |user| user.playback_position_ticks);
        let percentage = item
            .as_ref()
            .and_then(|item| item.user_data.as_ref())
            .and_then(|user| user.played_percentage);
        self.update_item(id, |item| {
            if let Some(user) = &mut item.user_data {
                user.playback_position_ticks = 0;
                user.played_percentage = None;
            }
        });
        self.home_data.resume.retain(|item| item.id != id);
        self.forget_checkpoint(id);
        if let Some(client) = self.client() {
            self.worker.set_resume_position(client, id.to_string(), 0);
        }
        match item {
            Some(item) => self.show_undo_toast(
                "Removed from Continue Watching.",
                crate::state::UndoAction::Resume {
                    item,
                    position_ticks,
                    percentage,
                },
            ),
            None => self.show_toast("Removed from Continue Watching."),
        }
    }

    /// The Undo button on a toast. The server is told the opposite change in the background; the
    /// in-memory rails are restored first so the screen feels immediate.
    pub fn undo_toast(&mut self) {
        let Some(undo) = self.toast_undo.take() else {
            return;
        };
        self.toast = None;
        self.toast_until = None;
        match undo {
            crate::state::UndoAction::Favorite { item_id, value } => {
                if let Some(client) = self.client() {
                    self.worker.set_favorite(client, item_id.clone(), value);
                }
                self.update_item(&item_id, |item| {
                    item.user_data
                        .get_or_insert_with(jellyfin_api::UserData::default)
                        .is_favorite = value;
                });
            }
            crate::state::UndoAction::Played {
                item_id,
                value,
                resume,
            } => {
                if let Some(client) = self.client() {
                    self.worker.set_played(client, item_id.clone(), value);
                }
                self.update_item(&item_id, |item| {
                    let user = item
                        .user_data
                        .get_or_insert_with(jellyfin_api::UserData::default);
                    user.played = value;
                    if let Some(restore) = &resume
                        && let Some(old) = &restore.user_data
                    {
                        user.playback_position_ticks = old.playback_position_ticks;
                        user.played_percentage = old.played_percentage;
                    }
                });
                if let Some(resume) = resume {
                    self.home_data.resume.retain(|item| item.id != item_id);
                    self.home_data.resume.insert(0, resume);
                }
            }
            crate::state::UndoAction::Resume {
                item,
                position_ticks,
                percentage,
            } => {
                if let Some(client) = self.client() {
                    self.worker
                        .set_resume_position(client, item.id.clone(), position_ticks);
                }
                self.update_item(&item.id, |live| {
                    if let Some(user) = &mut live.user_data {
                        user.playback_position_ticks = position_ticks;
                        user.played_percentage = percentage;
                    }
                });
                let mut restored = item;
                if let Some(user) = &mut restored.user_data {
                    user.playback_position_ticks = position_ticks;
                    user.played_percentage = percentage;
                }
                self.home_data.resume.retain(|item| item.id != restored.id);
                self.home_data.resume.insert(0, restored);
            }
            crate::state::UndoAction::Seek { position_ms } => {
                if let Some(current) = self.in_app.as_ref().map(|session| session.position_ms) {
                    self.seek_in_app(position_ms.saturating_sub(current));
                }
            }
            crate::state::UndoAction::PlaylistRemove {
                playlist_id,
                item,
                index,
                ..
            } => {
                if let Some(client) = self.client() {
                    self.worker
                        .add_to_playlist(client, playlist_id, vec![item.id.clone()]);
                }
                if let Some(library) = &mut self.library {
                    let at = index.min(library.items.len());
                    library.items.insert(at, item);
                    library.total += 1;
                }
                self.show_toast("Put the entry back.");
            }
            crate::state::UndoAction::NotInterested { item_id } => {
                self.hidden_home.remove(&item_id);
            }
            crate::state::UndoAction::PlaylistAdd {
                playlist_id,
                item_id,
            } => {
                if let Some(client) = self.client() {
                    self.worker.undo_playlist_add(client, playlist_id, item_id);
                }
                self.show_toast("Removed the added entry.");
            }
            crate::state::UndoAction::PlaylistMove {
                playlist_id,
                entry_id,
                from,
                to,
            } => {
                if let Some(library) = &mut self.library
                    && from < library.items.len()
                    && to < library.items.len()
                {
                    let moved = library.items.remove(to);
                    library.items.insert(from, moved);
                }
                if let Some(client) = self.client() {
                    self.worker
                        .move_playlist_entry(client, playlist_id, entry_id, from);
                }
            }
        }
    }

    /// "Play on…": ask the server which other sessions can take an item, then show them.
    pub fn card_menu_cast(&mut self) {
        let Some(menu) = self.card_menu.take() else {
            return;
        };
        if !is_playable(&menu.item) {
            return;
        }
        self.transfer_after_cast = false;
        self.cast_item = Some(menu.item);
        self.cast_picker_open = true;
        self.cast_loading = true;
        let Some(client) = self.client() else {
            self.show_toast("Your session expired. Sign in again.");
            return;
        };
        self.cast_sessions_load.request((), move |()| {
            client.sessions().map_err(|error| error.user_message())
        });
    }

    pub fn close_cast_picker(&mut self) {
        self.cast_picker_open = false;
        self.cast_item = None;
        self.cast_sessions.clear();
        self.cast_loading = false;
        self.transfer_after_cast = false;
    }

    /// Send the waiting item to another session. The server relays it; the toast says so.
    pub fn cast_to_session(&mut self, session_id: String, device_name: String) {
        self.cast_picker_open = false;
        let Some(item) = self.cast_item.take() else {
            return;
        };
        let Some(client) = self.client() else {
            self.show_toast("Your session expired. Sign in again.");
            return;
        };
        let ids = vec![item.id.clone()];
        let position = item
            .user_data
            .as_ref()
            .and_then(|user| user.position_ms())
            .unwrap_or(0)
            * jellyfin_api::TICKS_PER_MS;
        self.cast_send_load.request((), move |()| {
            client
                .play_on_session_at(&session_id, &ids, position)
                .map_err(|error| error.user_message())
        });
        self.show_toast(format!("Sending to {device_name}…"));
    }

    /// Open the playlist picker for the menu's item. The item is a stream, so a Series entry
    /// offers nothing here; the row is hidden for containers in the first place.
    pub fn card_menu_add_to_playlist(&mut self) {
        let Some(menu) = self.card_menu.take() else {
            return;
        };
        if !is_playable(&menu.item) {
            return;
        }
        self.open_playlist_picker_for(menu.item);
    }

    /// Open the collection picker for the menu's item.
    pub fn card_menu_add_to_collection(&mut self) {
        let Some(menu) = self.card_menu.take() else {
            return;
        };
        if !is_playable(&menu.item) {
            return;
        }
        self.open_collection_picker_for(menu.item);
    }

    /// Only episodes carry a series id; for anything else this is a no-op rather than a jump to
    /// the item the menu already offered to open.
    pub fn card_menu_go_to_series(&mut self) {
        let Some(menu) = self.card_menu.take() else {
            return;
        };
        let Some(series) = menu.item.series_id.clone() else {
            return;
        };
        self.open_detail(series);
    }
}
