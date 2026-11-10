//! The library screen: a virtualized grid or list over one paged `/Items` query.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use jellyfin_api::Item;
use rune::tok::{self, motion};

use crate::theme::{self, type_};
use rune::{Color, El, GrowthSpec, ScrollSpec, SurfaceFit, col, el, icon, row, text, text_input};

use crate::library::{GRID_COLUMNS, GRID_ROW_H, LIST_ROW_H, LibraryState, SortMode, ViewMode};
use crate::state::State;

use super::{CARD_GAP, CARD_H, CARD_W, card, placeholder, sidebar, subtitle_style};

pub(super) fn library_screen(s: &State, library: &LibraryState) -> El {
    let t = theme::colors();
    row()
        .key("library-screen")
        .w_full()
        .h_full()
        .fill(t.bg)
        .child(sidebar(s, &t))
        .child(el().w(1.0).h_full().fill(t.separator))
        .child(library_content(s, library, &t))
}

fn library_content(s: &State, library: &LibraryState, t: &tok::Theme) -> El {
    let header = col()
        .key("library-header")
        .w_full()
        .pt(24.0)
        .pb(12.0)
        .px(theme::gutter())
        .gap(10.0)
        .items_start()
        .child(back_button(t))
        .child(
            row()
                .w_full()
                .gap(12.0)
                .items_end()
                .child(text(library.name.clone()).font(type_::page()).color(t.text))
                .child(
                    text(count_label(library))
                        .key("library-count")
                        .font(type_::meta())
                        .color(t.text_secondary)
                        .mb(4.0),
                )
                .child(el().grow(1.0))
                .children({
                    // A TV library has no playable item on the page (it lists series); its
                    // shuffle still makes sense, and the server randomises the episodes.
                    let tv = library.collection_type.as_deref() == Some("tvshows");
                    (tv || library.items.iter().any(crate::state::is_playable)).then(|| {
                        row()
                            .gap(8.0)
                            .children((!tv).then(|| {
                                play_queue_button("▶ Play all", "library-play-all", t, false)
                            }))
                            .child(play_queue_button(
                                if tv {
                                    "⤨ Shuffle episodes"
                                } else {
                                    "⤨ Shuffle"
                                },
                                "library-shuffle",
                                t,
                                true,
                            ))
                    })
                }),
        )
        .children(library.notice.as_ref().map(|notice| {
            text(notice.clone())
                .key("library-notice")
                .font(type_::caption())
                .color(tok::accent(3, true))
        }));

    col()
        .key("library-content")
        .grow(1.0)
        .min_w(0.0)
        .min_h(0.0)
        .h_full()
        .fill(t.bg)
        .child(header)
        .child(library_toolbar(s, library, t))
        .children((s.library_selecting).then(|| bulk_bar(s, t)))
        .children((library.genres_open).then(|| genres_panel(library, t)))
        .children(library.filters_open.then(|| filters_panel(s, library, t)))
        .child(library_body(s, library, t))
}

/// What multi-select can do, shown only while it is on. Counts are live because selection is
/// in memory; the writes happen once, on the button.
fn bulk_bar(s: &State, t: &tok::Theme) -> El {
    row()
        .key("library-bulk-bar")
        .w_full()
        .px(theme::gutter())
        .pb(8.0)
        .gap(8.0)
        .items_center()
        .flex_wrap()
        .child(
            text(format!("{} selected", s.library_selected.len()))
                .key("library-selected-count")
                .font(type_::label())
                .color(t.text),
        )
        .child(el().grow(1.0))
        .child(toolbar_button(
            "Add to playlist",
            "bulk-playlist",
            false,
            t,
            |s: &mut State| {
                let ids: Vec<String> = s.library_selected.iter().cloned().collect();
                s.open_playlist_picker_for_ids(ids);
            },
        ))
        .child(toolbar_button(
            "Add to collection",
            "bulk-collection-add",
            false,
            t,
            |s: &mut State| {
                let ids: Vec<String> = s.library_selected.iter().cloned().collect();
                s.open_collection_picker_for_ids(ids);
            },
        ))
        .child(toolbar_button(
            "New collection",
            "bulk-collection",
            false,
            t,
            |s: &mut State| s.create_collection_from_selection(),
        ))
        .child(toolbar_button(
            "Mark watched",
            "bulk-watched",
            false,
            t,
            |s: &mut State| s.mark_selected_watched(),
        ))
        .child(toolbar_button(
            "Clear",
            "bulk-clear",
            false,
            t,
            |s: &mut State| s.clear_library_selection(),
        ))
        .child(toolbar_button(
            "Done",
            "bulk-done",
            false,
            t,
            |s: &mut State| s.toggle_library_selecting(),
        ))
}

fn play_queue_button(label: &str, key: &str, t: &tok::Theme, shuffle: bool) -> El {
    el().key(key)
        .role("button")
        .h(36.0)
        .px(14.0)
        .radius(9.0)
        .center()
        .fill(if shuffle {
            t.card
        } else {
            tok::accent(0, true)
        })
        .gradient(
            if shuffle {
                Color::WHITE.alpha(0.075)
            } else {
                Color::WHITE.alpha(0.18)
            },
            Color::TRANSPARENT,
        )
        .outline(1.0, 0.0, t.card_border)
        .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
        .shadow(
            if shuffle { 2.0 } else { 4.0 },
            if shuffle { 6.0 } else { 12.0 },
            if shuffle {
                Color::BLACK.alpha(0.18)
            } else {
                tok::accent(0, true).alpha(0.35)
            },
        )
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_ring(theme::focus_width(), 2.0, theme::ring())
        .press_scale(0.97)
        .label(label)
        .help(if shuffle {
            "Shuffle this whole library"
        } else {
            "Play this whole library in order"
        })
        .child(
            text(label.to_string())
                .font(type_::label())
                .color(if shuffle { t.text } else { t.bg }),
        )
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut State| s.play_library_queue(shuffle))
}

fn back_button(t: &tok::Theme) -> El {
    el().key("library-back")
        .role("button")
        .h(34.0)
        .px(12.0)
        .radius(8.0)
        .center()
        .fill(t.card)
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_ring(theme::focus_width(), 2.0, theme::ring())
        .press_scale(0.97)
        .child(text("← Home").font(type_::label()).color(t.text))
        .cursor(rune::Cursor::Pointer)
        .on_press(|s: &mut State| s.go_home())
}

fn library_toolbar(s: &State, library: &LibraryState, t: &tok::Theme) -> El {
    // A playlist is an ordered list: the server's order is the playlist's meaning, so sorting,
    // the unwatched/favourite filters and genres do not apply.
    let playlist = library.collection_type.as_deref() == Some("playlists");
    let mut bar = row()
        .key("library-toolbar")
        .focus_group()
        .w_full()
        .px(theme::gutter())
        .pb(12.0)
        .gap(8.0)
        // A narrow window gets a second line instead of half a button.
        .flex_wrap();
    if !playlist {
        bar = bar.child(
            el().key("library-search-box")
                .h(30.0)
                .w(180.0)
                .radius(8.0)
                .px(8.0)
                .center_y()
                .fill(t.card)
                .outline(
                    if s.library_search.focused { 1.5 } else { 1.0 },
                    0.0,
                    if s.library_search.focused {
                        tok::accent(0, true)
                    } else {
                        t.card_border
                    },
                )
                .box_shadow(rune::Shadow {
                    y: 0.0,
                    blur: 0.0,
                    spread: if s.library_search.focused { 3.0 } else { 0.0 },
                    color: tok::accent(0, true).alpha(if s.library_search.focused {
                        0.18
                    } else {
                        0.0
                    }),
                    inset: false,
                    ..rune::Shadow::default()
                })
                .child(
                    text_input(&s.library_search, type_::meta())
                        .key("library-search")
                        .label("Search this library")
                        .placeholder("Search this library", t.text_secondary.alpha(0.55))
                        .role("textbox")
                        .single_line()
                        .w_full()
                        .on_edit(|s: &mut State| &mut s.library_search)
                        .on_input_change(|s: &mut State, _| s.library_search_changed())
                        .color(t.text),
                ),
        );
        if library.search.is_some() {
            bar = bar.child(toolbar_button(
                "×",
                "library-search-clear",
                false,
                t,
                |s: &mut State| s.clear_library_search(),
            ));
        }
        for sort in [SortMode::Name, SortMode::Added, SortMode::Rating] {
            let key = format!("sort-{}", sort.label().to_ascii_lowercase());
            bar = bar.child(toolbar_button(
                sort.label(),
                &key,
                library.sort == sort,
                t,
                move |s: &mut State| s.set_sort(sort),
            ));
        }
        bar = bar.child(toolbar_button(
            if library.ascending {
                "↑ Asc"
            } else {
                "↓ Desc"
            },
            "sort-direction",
            false,
            t,
            |s: &mut State| s.toggle_sort_direction(),
        ));
    }
    if playlist {
        bar = bar.child(toolbar_button(
            if s.playlist_editing { "Done" } else { "Edit" },
            "playlist-edit",
            s.playlist_editing,
            t,
            |s: &mut State| s.toggle_playlist_editing(),
        ));
        if s.playlist_editing {
            bar = bar.child(
                text("Drag a poster to reorder")
                    .key("playlist-drag-hint")
                    .font(type_::caption())
                    .color(t.text_secondary),
            );
        }
    }
    bar = bar.child(el().grow(1.0));
    if !playlist {
        bar = bar
            .child(toolbar_button(
                "Unwatched",
                "filter-unwatched",
                library.unwatched,
                t,
                |s: &mut State| s.toggle_unwatched(),
            ))
            .child(toolbar_button(
                "Favorites",
                "filter-favorites",
                library.favorites,
                t,
                |s: &mut State| s.toggle_favorites(),
            ))
            .child(toolbar_button(
                "Genre",
                "library-genres",
                library.genre.is_some(),
                t,
                |s: &mut State| s.toggle_library_genres(),
            ))
            .child(toolbar_button(
                "Filters",
                "library-filters",
                library.filters_open,
                t,
                |s: &mut State| s.toggle_library_filters(),
            ))
            .child(toolbar_button(
                "Select",
                "library-select",
                s.library_selecting,
                t,
                |s: &mut State| s.toggle_library_selecting(),
            ));
    }
    bar.child(toolbar_button(
        "Grid",
        "view-grid",
        library.view == ViewMode::Grid,
        t,
        |s: &mut State| s.set_view(ViewMode::Grid),
    ))
    .child(toolbar_button(
        "List",
        "view-list",
        library.view == ViewMode::List,
        t,
        |s: &mut State| s.set_view(ViewMode::List),
    ))
}

/// A horizontally scrolling chip rail of the library's genres. It appears only when the Genre
/// button asks for it, so the normal toolbar stays compact at 1024.
/// The full filter and sort sheet. It is inline, not a modal, so the grid stays visible while
/// a filter is chosen; every control re-reads the server's first page.
fn filters_panel(s: &State, library: &LibraryState, t: &tok::Theme) -> El {
    let max_height = (s.window_size.1 - 240.0).clamp(220.0, 520.0);
    let mut panel = col()
        .key("library-filters-panel")
        .w_full()
        .max_h(max_height)
        .px(theme::gutter())
        .pb(10.0)
        .gap(10.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .scroll_edge_fade(16.0);
    let mut filters = row().w_full().gap(8.0).flex_wrap();
    filters = filters
        .child(toolbar_button(
            "Unwatched",
            "filters-unwatched",
            library.unwatched,
            t,
            |s: &mut State| s.toggle_unwatched(),
        ))
        .child(toolbar_button(
            "Favourites",
            "filters-favorites",
            library.favorites,
            t,
            |s: &mut State| s.toggle_favorites(),
        ));
    if library.unwatched || library.favorites || library.genre.is_some() || library.letter.is_some()
    {
        filters = filters.child(toolbar_button(
            "Clear filters",
            "filters-clear",
            false,
            t,
            |s: &mut State| s.clear_library_filters(),
        ));
    }
    panel = panel
        .child(
            text("Filters")
                .font(type_::section())
                .color(t.text_secondary),
        )
        .child(filters)
        .child(
            text("Sort")
                .font(type_::section())
                .color(t.text_secondary)
                .mt(4.0),
        );
    let mut sorts = row().w_full().gap(8.0).flex_wrap();
    for sort in [
        crate::library::SortMode::Name,
        crate::library::SortMode::Added,
        crate::library::SortMode::Rating,
        crate::library::SortMode::Played,
        crate::library::SortMode::Release,
        crate::library::SortMode::Runtime,
        crate::library::SortMode::PlayCount,
        crate::library::SortMode::Random,
    ] {
        sorts = sorts.child(toolbar_button(
            sort.label(),
            &format!("sort-{}", sort.label().to_lowercase()),
            library.sort == sort,
            t,
            move |s: &mut State| s.set_sort(sort),
        ));
    }
    sorts = sorts.child(toolbar_button(
        if library.ascending {
            "Ascending"
        } else {
            "Descending"
        },
        "sort-direction",
        false,
        t,
        |s: &mut State| s.toggle_sort_direction(),
    ));
    panel = panel.child(sorts);
    let mut views = row().w_full().gap(8.0).flex_wrap().items_center();
    views = views.child(
        text("Saved views")
            .font(type_::caption())
            .color(t.text_secondary),
    );
    for (index, view) in s.saved_views.iter().enumerate() {
        let label = view.label();
        views = views
            .child(toolbar_button(
                &label,
                &format!("saved-view-{index}"),
                false,
                t,
                move |s: &mut State| s.apply_library_view(index),
            ))
            .child(toolbar_button(
                "×",
                &format!("saved-view-delete-{index}"),
                false,
                t,
                move |s: &mut State| s.delete_library_view(index),
            ));
    }
    views = views.child(toolbar_button(
        "+ Save view",
        "saved-view-save",
        false,
        t,
        |s: &mut State| s.save_library_view(),
    ));
    panel = panel.child(views);
    let mut deep = row().w_full().gap(8.0).flex_wrap().items_center();
    deep = deep
        .child(text("Year").font(type_::caption()).color(t.text_secondary))
        .child(toolbar_button(
            "−",
            "filter-year-down",
            false,
            t,
            |s: &mut State| s.adjust_library_year(-1),
        ))
        .child(
            text(
                library
                    .year
                    .map_or_else(|| "Any".to_string(), |year| year.to_string()),
            )
            .key("filter-year-value")
            .font(type_::label())
            .color(t.text)
            .w(46.0)
            .center(),
        )
        .child(toolbar_button(
            "+",
            "filter-year-up",
            false,
            t,
            |s: &mut State| s.adjust_library_year(1),
        ))
        .child(toolbar_button(
            &library.min_rating.map_or_else(
                || "Rating: any".to_string(),
                |rating| format!("Rating {rating}+"),
            ),
            "filter-rating",
            library.min_rating.is_some(),
            t,
            |s: &mut State| s.cycle_library_rating(),
        ))
        .child(toolbar_button(
            &library
                .audio_language
                .as_ref()
                .map_or_else(|| "Audio: any".to_string(), |code| format!("Audio {code}")),
            "filter-audio-language",
            library.audio_language.is_some(),
            t,
            |s: &mut State| s.cycle_library_audio_language(),
        ))
        .child(toolbar_button(
            &library
                .subtitle_language
                .as_ref()
                .map_or_else(|| "Subs: any".to_string(), |code| format!("Subs {code}")),
            "filter-subtitle-language",
            library.subtitle_language.is_some(),
            t,
            |s: &mut State| s.cycle_library_subtitle_language(),
        ))
        .child(toolbar_button(
            "Has subtitles",
            "filter-has-subtitles",
            library.has_subtitles,
            t,
            |s: &mut State| s.toggle_library_subtitles(),
        ))
        .child(toolbar_button(
            "4K only",
            "filter-4k",
            library.only_4k,
            t,
            |s: &mut State| s.toggle_library_4k(),
        ))
        .child(toolbar_button(
            "Missing",
            "filter-missing",
            library.missing_only,
            t,
            |s: &mut State| s.toggle_library_missing(),
        ))
        .child(toolbar_button(
            "Unaired",
            "filter-unaired",
            library.unaired_only,
            t,
            |s: &mut State| s.toggle_library_unaired(),
        ));
    panel = panel.child(deep);
    if !library.genres.is_empty() {
        let mut genres = row().w_full().gap(8.0).scroll_x().scroll_edge_fade(16.0);
        for genre in &library.genres {
            let id = genre.id.clone();
            let name = genre.name.clone();
            let selected = library
                .genre
                .as_ref()
                .is_some_and(|(at, _)| at == &genre.id);
            let label = name.clone();
            genres = genres.child(
                el().key(format!("filter-genre-{}", genre.id))
                    .role("button")
                    .h(30.0)
                    .px(12.0)
                    .radius(15.0)
                    .center()
                    .fill(if selected {
                        tok::accent(0, true).alpha(0.24)
                    } else {
                        t.card
                    })
                    .outline(
                        1.0,
                        0.0,
                        if selected {
                            tok::accent(0, true)
                        } else {
                            t.card_border
                        },
                    )
                    .hover(t.hover)
                    .motion(motion::SNAPPY)
                    .focusable(true)
                    .focus_ring(theme::focus_width(), 2.0, theme::ring())
                    .press_scale(0.97)
                    .cursor(rune::Cursor::Pointer)
                    .child(text(label).font(type_::caption()).color(t.text))
                    .on_press(move |s: &mut State| {
                        s.select_library_genre(Some((id.clone(), name.clone())))
                    }),
            );
        }
        panel = panel.child(genres);
    }
    if !s
        .library
        .as_ref()
        .is_some_and(|library| library.genres_loading)
    {
        return panel;
    }
    panel.child(
        text("Loading genres…")
            .font(type_::caption())
            .color(t.text_secondary),
    )
}

fn genres_panel(library: &LibraryState, t: &tok::Theme) -> El {
    let mut rail = row()
        .key("library-genres-panel")
        .w_full()
        .h(36.0)
        .px(theme::gutter())
        .gap(8.0)
        .scroll_x()
        .scroll_edge_fade(16.0);
    rail = rail.child(genre_chip(
        "All",
        "genre-all",
        library.genre.is_none(),
        t,
        |s: &mut State| s.select_library_genre(None),
    ));
    if library.genres_loading && library.genres.is_empty() {
        rail = rail.child(
            text("Loading genres…")
                .key("library-genres-loading")
                .font(type_::caption())
                .color(t.text_secondary),
        );
    }
    for genre in &library.genres {
        let id = genre.id.clone();
        let name = genre.name.clone();
        let selected = library
            .genre
            .as_ref()
            .is_some_and(|(chosen, _)| chosen == &genre.id);
        rail = rail.child(genre_chip(
            &genre.name,
            &format!("genre-{}", genre.id),
            selected,
            t,
            move |s: &mut State| s.select_library_genre(Some((id.clone(), name.clone()))),
        ));
    }
    rail
}

fn genre_chip(
    label: &str,
    key: &str,
    selected: bool,
    t: &tok::Theme,
    on: impl Fn(&mut State) + 'static,
) -> El {
    el().key(key)
        .role("button")
        .h(28.0)
        .px(10.0)
        .radius(7.0)
        .center()
        .fill(if selected {
            tok::accent(0, true)
        } else {
            t.card
        })
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_ring(theme::focus_width(), 2.0, theme::ring())
        .press_scale(0.96)
        .child(
            text(label.to_string())
                .font(type_::caption())
                .color(if selected { t.bg } else { t.text }),
        )
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut State| on(s))
}

fn toolbar_button(
    label: &str,
    key: &str,
    selected: bool,
    t: &tok::Theme,
    on: impl Fn(&mut State) + 'static,
) -> El {
    el().key(key)
        .role("button")
        .h(30.0)
        .px(10.0)
        .radius(8.0)
        .center()
        .fill(if selected {
            tok::accent(0, true)
        } else {
            t.card
        })
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_ring(theme::focus_width(), 2.0, theme::ring())
        .press_scale(0.96)
        .child(
            text(label)
                .font(type_::label())
                .color(if selected { t.bg } else { t.text }),
        )
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut State| on(s))
}

fn library_body(s: &State, library: &LibraryState, t: &tok::Theme) -> El {
    if library.items.is_empty() {
        return if let Some(message) = &library.error {
            library_error(message, t)
        } else if library.loading {
            library_skeleton(t)
        } else {
            library_empty(library, t)
        };
    }

    let scroll = Arc::clone(&library.scroll);
    // A column, not a row: a row stretches its child to the viewport height, so the virtual
    // rows never report a taller content extent and the wheel has nothing to scroll.
    let viewport = col()
        .key("library-scroll")
        .grow(1.0)
        .min_h(0.0)
        .h_full()
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .on_far_from_bottom(600.0, |s: &mut State, far| {
            if !far {
                s.load_library_page(false);
            }
        })
        .on_scroll_offset(move |at| {
            scroll.store(at.offset[1].to_bits(), Ordering::Relaxed);
        })
        .child(if library.view == ViewMode::Grid {
            library_grid(s, t)
        } else {
            library_list(s, t)
        });

    let mut body = row()
        .key("library-body")
        .w_full()
        .grow(1.0)
        .min_w(0.0)
        .min_h(0.0)
        .px(theme::gutter())
        .pb(if s.in_app_minimized { 116.0 } else { 20.0 })
        .gap(12.0)
        .on_layout_tx(|s: &mut State, rect: [f32; 4], tx: &mut rune::Tx| {
            // Do not add the gap here: the measured rect includes the body padding, and adding
            // one more gap let a third card claim a column it could not fit.
            let columns = ((rect[2] / (CARD_W + CARD_GAP)).floor() as usize).clamp(2, 12);
            if let Some(library) = &mut s.library
                && library.set_columns(columns)
            {
                tx.settle();
            }
        })
        .child(viewport);
    if library.sort == SortMode::Name && library.total_items() >= 100 {
        body = body.child(letter_rail(library, t));
    }
    body
}

fn library_grid(s: &State, t: &tok::Theme) -> El {
    let Some(library) = &s.library else {
        return col();
    };
    let columns = library.columns;
    let rows = library.items.len().div_ceil(columns);
    let theme = *t;
    col()
        .key("library-grid")
        .w_full()
        .growth(GrowthSpec::default())
        .virtual_rows_version(library.version())
        .virtual_rows(
            rows,
            GRID_ROW_H,
            |_s: &State, i| format!("library-row-{i}").into(),
            move |s: &State, i| {
                let Some(library) = &s.library else {
                    return row();
                };
                let start = i * columns;
                row()
                    .key(format!("library-grid-row-{i}"))
                    .w_full()
                    .h(GRID_ROW_H)
                    .gap(CARD_GAP)
                    .items_start()
                    .children(
                        library
                            .items
                            .iter()
                            .skip(start)
                            .take(columns)
                            .map(|item| card(item, "library", s, &theme)),
                    )
            },
        )
        .on_visible_rows(|s: &mut State, range| {
            s.request_library_images_in(range);
            false
        })
}

fn library_list(s: &State, t: &tok::Theme) -> El {
    let Some(library) = &s.library else {
        return col();
    };
    let theme = *t;
    col()
        .key("library-list")
        .w_full()
        .growth(GrowthSpec::default())
        .virtual_rows_version(library.version())
        .virtual_rows(
            library.items.len(),
            LIST_ROW_H,
            |s: &State, i| {
                s.library
                    .as_ref()
                    .and_then(|library| library.items.get(i))
                    .map_or_else(
                        || format!("library-row-{i}"),
                        |item| format!("library-item-{}", item.id),
                    )
                    .into()
            },
            move |s: &State, i| {
                let Some(item) = s.library.as_ref().and_then(|library| library.items.get(i)) else {
                    return el();
                };
                library_list_row(item, s, &theme)
            },
        )
        .on_visible_rows(|s: &mut State, range| {
            s.request_library_images_in(range);
            false
        })
}

fn library_list_row(item: &Item, s: &State, t: &tok::Theme) -> El {
    let mut facts: Vec<String> = Vec::new();
    // For a film the subtitle is already the year; for an episode it is `S2E04 · name`, so
    // the year still belongs in the facts line.
    if item.episode_label().is_some()
        && let Some(year) = item.production_year
    {
        facts.push(year.to_string());
    }
    if let Some(rating) = &item.official_rating {
        facts.push(rating.clone());
    }
    if let Some(minutes) = item.runtime_ms().map(|ms| ms / 60_000).filter(|m| *m > 0) {
        facts.push(format!("{minutes} min"));
    }
    if let Some(date) = item.date_created.as_deref().and_then(|date| date.get(..10)) {
        facts.push(format!("Added {date}"));
    }
    if let Some(plays) = item
        .user_data
        .as_ref()
        .map(|user| user.play_count)
        .filter(|plays| *plays > 0)
    {
        facts.push(format!(
            "{plays} {}",
            if plays == 1 { "play" } else { "plays" }
        ));
    }
    if let Some(size) = item.media_sources.first().and_then(|source| source.size) {
        facts.push(format!("{:.1} GB", size as f64 / 1_000_000_000.0));
    }
    if let Some(video) = item
        .media_sources
        .iter()
        .flat_map(|source| source.media_streams.iter())
        .find(|stream| stream.kind == "Video")
    {
        if video.height.is_some_and(|height| height >= 2000) {
            facts.push("4K".into());
        }
        if video
            .video_range
            .as_deref()
            .is_some_and(|range| !range.eq_ignore_ascii_case("SDR"))
        {
            facts.push("HDR".into());
        }
    }
    if item.media_sources.iter().any(|source| {
        source
            .media_streams
            .iter()
            .any(|stream| stream.kind == "Subtitle")
    }) {
        facts.push("CC".into());
    }
    if crate::home::is_watched(item) {
        facts.push("Watched".into());
    }
    let facts = facts.join(" · ");
    let poster = s
        .image_key(item)
        .and_then(|key| super::poster_layers(s, &key, SurfaceFit::Fill))
        .unwrap_or_else(|| placeholder(item, t));
    let watched = crate::home::progress(item);
    theme::tv_focus(
        row()
            .key(format!("library-item-{}", item.id))
            .w_full()
            .h(LIST_ROW_H - 8.0)
            .padding(6.0)
            .gap(12.0)
            .radius(8.0)
            .hover(t.hover)
            .motion(motion::SNAPPY)
            .role("button")
            .focusable(true)
            .focus_ring(theme::focus_width(), 2.0, theme::ring())
            .press_scale(0.995)
            .cursor(rune::Cursor::Pointer)
            .on_press({
                let id = item.id.clone();
                move |s: &mut State| s.open_detail(id.clone())
            })
            .child(
                el().w(40.0)
                    .h(56.0)
                    .radius(5.0)
                    .clip_rounded(5.0)
                    .fill(t.card)
                    .child(poster),
            )
            .child(
                col()
                    .grow(1.0)
                    .gap(4.0)
                    .child(
                        text(item.name.clone())
                            .font(type_::body())
                            .color(t.text)
                            .truncate(),
                    )
                    .child(
                        row()
                            .gap(10.0)
                            .child(
                                text(crate::home::subtitle(item))
                                    .font(type_::mono_caption())
                                    .color(t.text_secondary),
                            )
                            .child(
                                el().w(80.0).h(3.0).radius(1.5).fill(t.control_track).child(
                                    el().w(80.0 * watched)
                                        .h(3.0)
                                        .radius(1.5)
                                        .fill(tok::accent(0, true)),
                                ),
                            ),
                    )
                    .children((!facts.is_empty()).then(|| {
                        text(facts)
                            .font(type_::mono_caption())
                            .color(t.text_secondary)
                            .truncate()
                    })),
            )
            .children(crate::home::is_watched(item).then(|| {
                el().key(format!("played-{}", item.id))
                    .w(20.0)
                    .h(20.0)
                    .radius(10.0)
                    .center()
                    .fill(tok::accent(0, true))
                    .child(text("✓").font(type_::caption()).color(t.bg))
            }))
            .child(icon(&theme::icon::CHEVRON_RIGHT, 14.0).color(t.text_secondary.alpha(0.45))),
    )
}

fn letter_rail(library: &LibraryState, t: &tok::Theme) -> El {
    let mut rail = col().key("letter-rail").w(28.0).gap(2.0).pt(2.0);
    for letter in 'A'..='Z' {
        let selected = library
            .letter
            .as_deref()
            .is_some_and(|chosen| chosen.starts_with(letter));
        rail = rail.child(
            el().key(format!("letter-{letter}"))
                .role("button")
                .w(28.0)
                .h(24.0)
                .radius(5.0)
                .center()
                .fill(if selected { tok::accent(0, true) } else { t.bg })
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_ring(theme::focus_width(), 1.0, theme::ring())
                .press_scale(0.94)
                .child(
                    text(letter.to_string())
                        .font(type_::mono_caption())
                        .color(if selected { t.bg } else { t.text_secondary }),
                )
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut State| s.toggle_letter(letter)),
        );
    }
    rail
}

fn library_skeleton(t: &tok::Theme) -> El {
    col()
        .key("library-loading")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .px(theme::gutter())
        .gap(20.0)
        .child(
            text("Loading the library…")
                .key("library-loading-label")
                .font(subtitle_style())
                .color(t.text_secondary),
        )
        .children((0..2).map(|_| {
            row().gap(CARD_GAP).children(
                (0..GRID_COLUMNS).map(|_| el().w(CARD_W).h(CARD_H).radius(10.0).fill(t.card)),
            )
        }))
}

fn library_empty(library: &LibraryState, t: &tok::Theme) -> El {
    let filtered = library.unwatched
        || library.favorites
        || library.genre.is_some()
        || library.letter.is_some()
        || library.year.is_some()
        || library.min_rating.is_some()
        || library.audio_language.is_some()
        || library.subtitle_language.is_some()
        || library.has_subtitles
        || library.only_4k
        || library.missing_only
        || library.unaired_only;
    let action = if filtered {
        toolbar_button(
            "Show all",
            "library-clear-filter",
            false,
            t,
            |s: &mut State| s.clear_library_filters(),
        )
    } else {
        toolbar_button("Refresh", "library-retry", false, t, |s: &mut State| {
            s.load_library_page(true)
        })
    };
    col()
        .key("library-empty")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .center()
        .gap(10.0)
        .padding(theme::gutter())
        .child(text("No items here").font(type_::page()).color(t.text))
        .child(
            text(if library.genre.is_some() {
                "Nothing in this genre here."
            } else if library.favorites && library.unwatched {
                "Nothing unwatched in your favourites."
            } else if library.favorites {
                "You have not marked anything here as a favourite."
            } else if library.unwatched {
                "Nothing unwatched in this library."
            } else if library.letter.is_some() {
                "Nothing starts with that letter."
            } else {
                "This library has nothing to show yet."
            })
            .font(subtitle_style())
            .color(t.text_secondary),
        )
        .child(action)
}

fn library_error(message: &str, t: &tok::Theme) -> El {
    col()
        .key("library-error")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .center()
        .gap(10.0)
        .padding(theme::gutter())
        .child(
            text("Couldn't load this library")
                .font(type_::page())
                .color(t.text),
        )
        .child(
            text(message.to_string())
                .key("library-error-message")
                .font(subtitle_style())
                .color(tok::accent(2, true)),
        )
        .child(toolbar_button(
            "Try again",
            "library-retry",
            false,
            t,
            |s: &mut State| s.load_library_page(true),
        ))
}

fn count_label(library: &LibraryState) -> String {
    let filtered = [
        library.unwatched,
        library.favorites,
        library.genre.is_some(),
        library.year.is_some(),
        library.min_rating.is_some(),
        library.audio_language.is_some(),
        library.subtitle_language.is_some(),
        library.has_subtitles,
        library.only_4k,
        library.missing_only,
        library.unaired_only,
        library.letter.is_some(),
        library.search.is_some(),
    ]
    .into_iter()
    .filter(|filter| *filter)
    .count();
    let total = library.total_items();
    let label = if library.loading && total == 0 {
        "Loading…".to_string()
    } else if total == 0 {
        "No items".to_string()
    } else if library.items.len() < total {
        format!(
            "{} of {}",
            library.items.len(),
            super::count_label(total, "item")
        )
    } else {
        super::count_label(total, "item")
    };
    if filtered > 0 {
        format!("{label} · {filtered} filter(s)")
    } else {
        label
    }
}
