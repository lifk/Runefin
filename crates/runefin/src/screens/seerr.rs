//! The optional Seerr companion: Discover, Requests and the request panel.
//!
//! It is one route with two tabs. The Requests list paints statuses from `/request` first and
//! fills titles/posters from a cached `/movie` or `/tv` summary, because the request row has
//! neither. A season pack is one download with many episode rows, so the size is shown once.

use rune::tok::{self, motion};
use rune::{El, ScrollSpec, SurfaceFit, col, el, row, text, text_input};

use crate::state::{
    DiscoverKind, DiscoverMode, DiscoverSort, NETWORKS, RequestsFilter, STUDIOS, SeerrTab, State,
};
use crate::theme::FocusVisual;
use crate::theme::{self, type_};

use super::{CARD_GAP, CARD_H, CARD_W, initial, poster_layers, sidebar};

pub(super) fn seerr_screen(s: &State) -> El {
    let t = theme::colors();
    let content = if !s.seerr_configured() {
        setup_card(s, &t)
    } else {
        match s.seerr.tab {
            SeerrTab::Discover => discover_body(s, &t),
            SeerrTab::Requests => requests_body(s, &t),
        }
    };
    row()
        .key("seerr-screen")
        .w_full()
        .h_full()
        .fill(t.bg)
        .child(sidebar(s, &t))
        .child(el().w(1.0).h_full().fill(t.separator))
        .child(
            col()
                .key("seerr-content")
                .grow(1.0)
                .min_w(0.0)
                .min_h(0.0)
                .h_full()
                .child(page_header(s, &t))
                .child(tabs(s, &t))
                .child(content),
        )
}

fn page_header(s: &State, t: &tok::Theme) -> El {
    let title = match s.seerr.tab {
        SeerrTab::Discover => "Discover",
        SeerrTab::Requests => "Requests",
    };
    let subtitle = match s.seerr.tab {
        SeerrTab::Discover => "New films and series to request",
        SeerrTab::Requests => "What Seerr is fetching, and what is already in the library",
    };
    row()
        .key("seerr-header")
        .w_full()
        .padding(theme::gutter())
        .pb(10.0)
        .gap(12.0)
        .items_center()
        .child(
            el().key("seerr-back")
                .role("button")
                .h(32.0)
                .px(12.0)
                .radius(8.0)
                .center()
                .fill(t.card)
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.97)
                .cursor(rune::Cursor::Pointer)
                .label("Back")
                .on_press(|s: &mut State| s.close_seerr())
                .child(text("← Back").font(type_::label()).color(t.text)),
        )
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(2.0)
                .child(
                    text(title)
                        .key("seerr-title")
                        .font(type_::page())
                        .color(t.text),
                )
                .child(
                    text(subtitle)
                        .key("seerr-subtitle")
                        .font(type_::meta())
                        .color(t.text_secondary),
                ),
        )
        .child(
            el().key("seerr-refresh")
                .role("button")
                .h(32.0)
                .px(12.0)
                .radius(8.0)
                .center()
                .fill(t.card)
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.97)
                .cursor(rune::Cursor::Pointer)
                .label("Refresh")
                .on_press(|s: &mut State| s.refresh_seerr())
                .child(text("Refresh").font(type_::label()).color(t.text)),
        )
}

fn tabs(s: &State, t: &tok::Theme) -> El {
    let requests = if let Some(counts) = &s.seerr.counts {
        if counts.active() > 0 {
            format!("Requests · {}", counts.active())
        } else {
            "Requests".to_string()
        }
    } else {
        "Requests".to_string()
    };
    row()
        .key("seerr-tabs")
        .w_full()
        .px(theme::gutter())
        .pb(8.0)
        .gap(8.0)
        .child(tab_button(
            "Discover",
            "seerr-tab-discover",
            s.seerr.tab == SeerrTab::Discover,
            t,
            |s: &mut State| s.set_seerr_tab(SeerrTab::Discover),
        ))
        .child(tab_button(
            &requests,
            "seerr-tab-requests",
            s.seerr.tab == SeerrTab::Requests,
            t,
            |s: &mut State| s.set_seerr_tab(SeerrTab::Requests),
        ))
}

fn tab_button(
    label: &str,
    key: &str,
    selected: bool,
    t: &tok::Theme,
    on: impl Fn(&mut State) + 'static,
) -> El {
    el().key(key)
        .role("tab")
        .h(32.0)
        .px(14.0)
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
        .focus_visual(2.0)
        .press_scale(0.97)
        .cursor(rune::Cursor::Pointer)
        .label(label.to_string())
        .on_press(on)
        .child(
            text(label.to_string())
                .font(type_::label())
                .color(if selected { t.bg } else { t.text }),
        )
}

fn setup_card(s: &State, t: &tok::Theme) -> El {
    let _ = s;
    col()
        .key("seerr-unconfigured")
        .w_full()
        .grow(1.0)
        .justify_center()
        .items_center()
        .gap(12.0)
        .padding(theme::gutter())
        .child(
            text("Seerr is not connected")
                .font(type_::section())
                .color(t.text),
        )
        .child(
            text(
                "Add the address and API key in Settings to browse Seerr's catalogue, request titles \
                 and follow their downloads. The rest of the app works without it.",
            )
            .font(type_::body())
            .color(t.text_secondary)
            .max_w(560.0),
        )
        .child(
            el().key("seerr-setup")
                .role("button")
                .h(38.0)
                .px(16.0)
                .radius(9.0)
                .center()
                .fill(tok::accent(0, true))
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.97)
                .cursor(rune::Cursor::Pointer)
                .on_press(|s: &mut State| s.open_settings())
                .child(text("Open Settings").font(type_::label()).color(t.bg)),
        )
}

// ---- Discover ---------------------------------------------------------------------------

fn discover_body(s: &State, t: &tok::Theme) -> El {
    let query = s.seerr.query.text.trim().to_string();
    let search = query.chars().count() >= 2;
    let mut body = col()
        .key("seerr-discover")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .px(theme::gutter())
        .pb(if s.in_app_minimized { 124.0 } else { 32.0 })
        .gap(20.0)
        .child(search_box(s, t));

    if let Some(error) = &s.seerr.search_error
        && search
    {
        return body.child(message_card(error, "seerr-search-error", t));
    }

    if search {
        if s.seerr.searching && s.seerr.search.results.is_empty() {
            return body.child(loading_rows(t));
        }
        let movies: Vec<_> = s
            .seerr
            .search
            .results
            .iter()
            .filter(|item| item.is_movie())
            .collect();
        let shows: Vec<_> = s
            .seerr
            .search
            .results
            .iter()
            .filter(|item| item.is_tv())
            .collect();
        if movies.is_empty() && shows.is_empty() {
            return body.child(
                text("No films or series matched.")
                    .key("seerr-search-empty")
                    .font(type_::body())
                    .color(t.text_secondary),
            );
        }
        if !movies.is_empty() {
            body = body.child(card_section("Films", "seerr-search-movies", &movies, s, t));
        }
        if !shows.is_empty() {
            body = body.child(card_section("Series", "seerr-search-shows", &shows, s, t));
        }
        return body;
    }

    body = body.child(discover_mode_row(s, t));
    if s.seerr.discover_mode == DiscoverMode::Browse {
        return body.child(browse_filters(s, t)).child(browse_grid(s, t));
    }

    if let Some(error) = &s.seerr.discover_error {
        return body.child(message_card(error, "seerr-discover-error", t));
    }
    if s.seerr.discover_loading && s.seerr.discover.is_empty() {
        return body.child(loading_rows(t));
    }
    let discover = &s.seerr.discover;
    if discover.is_empty() {
        return body.child(
            text("Seerr's catalogue is empty. Try Search or refresh.")
                .key("seerr-discover-empty")
                .font(type_::body())
                .color(t.text_secondary),
        );
    }
    for (title, key, items) in [
        (
            "Trending this week",
            "seerr-row-trending",
            &discover.trending,
        ),
        ("Films", "seerr-row-movies", &discover.movies),
        ("Series", "seerr-row-tv", &discover.tv),
    ] {
        if !items.is_empty() {
            body = body.child(card_section(
                title,
                key,
                &items.iter().collect::<Vec<_>>(),
                s,
                t,
            ));
        }
    }
    body
}

fn discover_mode_row(s: &State, t: &tok::Theme) -> El {
    row()
        .key("seerr-discover-modes")
        .w_full()
        .gap(8.0)
        .items_center()
        .focus_group()
        .child(tab_button(
            "Explore",
            "seerr-mode-explore",
            s.seerr.discover_mode == DiscoverMode::Explore,
            t,
            |s: &mut State| s.set_seerr_discover_mode(DiscoverMode::Explore),
        ))
        .child(tab_button(
            "Browse",
            "seerr-mode-browse",
            s.seerr.discover_mode == DiscoverMode::Browse,
            t,
            |s: &mut State| s.set_seerr_discover_mode(DiscoverMode::Browse),
        ))
}

fn browse_filters(s: &State, t: &tok::Theme) -> El {
    let discover = &s.seerr.discover;
    let filters = &discover.filters;
    let mut rows = col().key("seerr-browse-filters").w_full().gap(8.0);

    let mut kind = row().gap(6.0).items_center().focus_group();
    kind = kind
        .child(tab_button(
            DiscoverKind::Movies.label(),
            "seerr-browse-films",
            filters.kind == DiscoverKind::Movies,
            t,
            |s: &mut State| s.set_discover_kind(DiscoverKind::Movies),
        ))
        .child(tab_button(
            DiscoverKind::Series.label(),
            "seerr-browse-series",
            filters.kind == DiscoverKind::Series,
            t,
            |s: &mut State| s.set_discover_kind(DiscoverKind::Series),
        ))
        .child(el().w(14.0))
        .child(tab_button(
            DiscoverSort::Popular.label(),
            "seerr-browse-popular",
            filters.sort == DiscoverSort::Popular,
            t,
            |s: &mut State| s.set_discover_sort(DiscoverSort::Popular),
        ))
        .child(tab_button(
            DiscoverSort::Rating.label(),
            "seerr-browse-rating",
            filters.sort == DiscoverSort::Rating,
            t,
            |s: &mut State| s.set_discover_sort(DiscoverSort::Rating),
        ))
        .child(tab_button(
            DiscoverSort::Newest.label(),
            "seerr-browse-newest",
            filters.sort == DiscoverSort::Newest,
            t,
            |s: &mut State| s.set_discover_sort(DiscoverSort::Newest),
        ));
    rows = rows.child(kind);

    let mut decades = row().gap(6.0).items_center().focus_group();
    decades = decades.child(tab_button(
        "Any year",
        "seerr-browse-year-any",
        filters.decade.is_none(),
        t,
        |s: &mut State| s.set_discover_decade(None),
    ));
    for (label, year, key) in [
        ("2020s", 2020, "seerr-browse-year-2020"),
        ("2010s", 2010, "seerr-browse-year-2010"),
        ("2000s", 2000, "seerr-browse-year-2000"),
        ("1990s", 1990, "seerr-browse-year-1990"),
        ("Older", 0, "seerr-browse-year-older"),
    ] {
        decades = decades.child(tab_button(
            label,
            key,
            filters.decade == Some(year),
            t,
            move |s: &mut State| s.set_discover_decade(Some(year)),
        ));
    }
    rows = rows.child(decades);

    let mut genres = row().gap(6.0).focus_group();
    genres = genres.child(tab_button(
        "All genres",
        "seerr-browse-genre-all",
        filters.genre.is_none(),
        t,
        |s: &mut State| s.set_discover_genre(None),
    ));
    for genre in discover.genres() {
        let Some(name) = genre.name.as_deref() else {
            continue;
        };
        let id = genre.id;
        genres = genres.child(tab_button(
            name,
            &format!("seerr-browse-genre-{}", genre.id),
            filters.genre == Some(id),
            t,
            move |s: &mut State| s.set_discover_genre(Some(id)),
        ));
    }
    rows = rows.child(
        el().w_full()
            .scroll_x()
            .scroll_edge_fade(28.0)
            .child(genres),
    );

    let hubs = match filters.kind {
        DiscoverKind::Movies => STUDIOS,
        DiscoverKind::Series => NETWORKS,
    };
    let mut hub_row = row().gap(6.0).focus_group();
    hub_row = hub_row.child(tab_button(
        match filters.kind {
            DiscoverKind::Movies => "All studios",
            DiscoverKind::Series => "All networks",
        },
        "seerr-browse-hub-all",
        filters.hub.is_none(),
        t,
        |s: &mut State| s.set_discover_hub(None),
    ));
    for (name, id) in hubs {
        let id = *id;
        hub_row = hub_row.child(tab_button(
            name,
            &format!("seerr-browse-hub-{id}"),
            filters.hub == Some(id),
            t,
            move |s: &mut State| s.set_discover_hub(Some(id)),
        ));
    }
    rows = rows.child(
        el().w_full()
            .scroll_x()
            .scroll_edge_fade(28.0)
            .child(hub_row),
    );
    rows
}

fn browse_grid(s: &State, t: &tok::Theme) -> El {
    let discover = &s.seerr.discover;
    if let Some(error) = &discover.browse_error {
        return message_card(error, "seerr-browse-error", t);
    }
    if discover.browse.is_empty() {
        if discover.browse_loading {
            return loading_rows(t);
        }
        return text("Nothing matched those filters.")
            .key("seerr-browse-empty")
            .font(type_::body())
            .color(t.text_secondary);
    }
    let mut grid = col().key("seerr-browse-grid").w_full().gap(CARD_GAP);
    for chunk in discover.browse.chunks(5) {
        grid = grid.child(
            row()
                .gap(CARD_GAP)
                .items_start()
                .focus_group()
                .children(chunk.iter().map(|item| discover_card(item, s, t))),
        );
    }
    if discover.browse_loading {
        grid = grid.child(loading_rows(t));
    } else if discover.browse_pages > 0 && discover.browse_page < discover.browse_pages {
        grid = grid.child(
            el().key("seerr-browse-more")
                .role("button")
                .w(160.0)
                .h(36.0)
                .px(16.0)
                .radius(9.0)
                .center()
                .fill(t.card)
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.97)
                .cursor(rune::Cursor::Pointer)
                .label("Load more")
                .on_press(|s: &mut State| s.load_more_seerr_browse())
                .child(text("Load more").font(type_::label()).color(t.text)),
        );
    }
    grid
}

fn search_box(s: &State, t: &tok::Theme) -> El {
    let input = text_input(&s.seerr.query, type_::body())
        .key("seerr-search")
        .label("Search Seerr")
        .placeholder("Search films and series", t.text_secondary.alpha(0.55))
        .role("textbox")
        .single_line()
        .w_full()
        .on_edit(|s: &mut State| &mut s.seerr.query)
        .on_input_change(|s: &mut State, _| s.seerr_search_changed())
        .color(t.text);
    el().key("seerr-search-box")
        .h(42.0)
        .w_full()
        .max_w(640.0)
        .radius(10.0)
        .px(12.0)
        .center_y()
        .fill(t.card)
        .outline(
            if s.seerr.query.focused { 1.5 } else { 1.0 },
            0.0,
            if s.seerr.query.focused {
                tok::accent(0, true)
            } else {
                t.card_border
            },
        )
        .child(input)
}

fn card_section(
    title: &str,
    key: &str,
    items: &[&seerr_api::DiscoverItem],
    s: &State,
    t: &tok::Theme,
) -> El {
    col()
        .key(key)
        .w_full()
        .gap(12.0)
        .child(text(title).font(type_::section()).color(t.text))
        .child(
            el().w_full().scroll_x().scroll_edge_fade(28.0).child(
                row().gap(CARD_GAP).focus_group().children(
                    items
                        .iter()
                        .take(20)
                        .copied()
                        .map(|item| discover_card(item, s, t)),
                ),
            ),
        )
}

pub(super) fn discover_card(item: &seerr_api::DiscoverItem, s: &State, t: &tok::Theme) -> El {
    let key = s.seerr_item_poster_key(item);
    let poster = key
        .as_deref()
        .and_then(|key| poster_layers(s, key, SurfaceFit::Fill))
        .unwrap_or_else(|| placeholder(item.title(), t));
    let media_type = item.media_type.clone();
    let tmdb_id = item.id;
    let state = item
        .media_info
        .as_ref()
        .map(|info| info.media_status())
        .unwrap_or(seerr_api::MediaStatus::Unknown);
    col()
        .key(format!("seerr-card-{}-{}", item.media_type, item.id))
        .role("button")
        .w(CARD_W)
        .gap(6.0)
        .hover_group()
        .focusable(true)
        .focus_visual(4.0)
        .press_scale(0.985)
        .motion(motion::SNAPPY)
        .label(item.title().to_string())
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut State| s.open_seerr_detail(media_type.clone(), tmdb_id))
        .child(
            el().w(CARD_W)
                .h(CARD_H)
                .radius(10.0)
                .clip_rounded(10.0)
                .fill(t.card)
                .outline(1.0, 0.0, t.card_border)
                .shadow(10.0, 24.0, t.shadow_soft)
                .child(poster)
                .children(
                    (state != seerr_api::MediaStatus::Unknown)
                        .then(|| el().absolute(8.0, 8.0).child(status_chip(state, t))),
                ),
        )
        .child(
            text(item.title().to_string())
                .font(type_::card_title())
                .color(t.text)
                .max_lines(2)
                .truncate(),
        )
        .child(
            text(item.year().unwrap_or("").to_string())
                .font(type_::meta())
                .color(t.text_secondary),
        )
}

// ---- Requests ---------------------------------------------------------------------------

fn requests_body(s: &State, t: &tok::Theme) -> El {
    let mut body = col()
        .key("seerr-requests")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .px(theme::gutter())
        .pb(if s.in_app_minimized { 124.0 } else { 32.0 })
        .gap(12.0)
        .child(filter_row(s, t));

    if let Some(error) = &s.seerr.requests_error
        && s.seerr.requests.is_empty()
    {
        return body.child(message_card(error, "seerr-requests-error", t));
    }
    if s.seerr.loading && s.seerr.requests.is_empty() {
        return body.child(loading_rows(t));
    }
    if s.seerr.requests.is_empty() {
        return body.child(
            text("No active requests. Find something on Discover.")
                .key("seerr-requests-empty")
                .font(type_::body())
                .color(t.text_secondary),
        );
    }
    if let Some(error) = &s.seerr.requests_error {
        body = body.child(message_card(error, "seerr-requests-banner", t));
    }
    if let Some(counts) = &s.seerr.counts {
        let total = match s.seerr.filter {
            RequestsFilter::Active => counts.active(),
            RequestsFilter::All => counts.total,
        };
        if total > s.seerr.requests.len() as i64 {
            body = body.child(
                text(format!(
                    "Showing {} of {total} requests.",
                    s.seerr.requests.len()
                ))
                .key("seerr-requests-truncated")
                .font(type_::caption())
                .color(t.text_secondary),
            );
        }
    }
    if let Some(at) = s.seerr.last_loaded
        && at.elapsed().as_secs() >= 10
    {
        body = body.child(
            text(format!("Updated {} s ago", at.elapsed().as_secs()))
                .key("seerr-requests-stale")
                .font(type_::caption())
                .color(t.text_secondary),
        );
    }
    for request in &s.seerr.requests {
        body = body.child(request_row(request, s, t));
    }
    body
}

fn filter_row(s: &State, t: &tok::Theme) -> El {
    let summary_status = if s.seerr.summaries_loading {
        "Loading titles…".to_string()
    } else if let Some(error) = &s.seerr.summaries_error {
        error.clone()
    } else {
        String::new()
    };
    row()
        .key("seerr-filter")
        .w_full()
        .gap(8.0)
        .items_center()
        .child(filter_button(
            "Active",
            "seerr-filter-active",
            s.seerr.filter == RequestsFilter::Active,
            t,
            |s: &mut State| s.set_seerr_filter(RequestsFilter::Active),
        ))
        .child(filter_button(
            "All",
            "seerr-filter-all",
            s.seerr.filter == RequestsFilter::All,
            t,
            |s: &mut State| s.set_seerr_filter(RequestsFilter::All),
        ))
        .child(el().grow(1.0))
        .child(
            text(summary_status)
                .key("seerr-summaries-status")
                .font(type_::caption())
                .color(t.text_secondary),
        )
}

fn filter_button(
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
            t.card
        } else {
            rune::Color::TRANSPARENT
        })
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.97)
        .cursor(rune::Cursor::Pointer)
        .label(label.to_string())
        .on_press(on)
        .child(
            text(label.to_string())
                .font(type_::caption())
                .color(if selected { t.text } else { t.text_secondary }),
        )
}

fn request_row(request: &seerr_api::MediaRequest, s: &State, t: &tok::Theme) -> El {
    let tmdb_id = request.media.tmdb_id.unwrap_or_default();
    let summary = s.seerr.summary(&request.media_type, tmdb_id);
    let title = summary
        .map(|summary| summary.title.clone())
        .unwrap_or_else(|| format!("TMDB {tmdb_id}"));
    let year = summary
        .and_then(|summary| summary.year.clone())
        .unwrap_or_default();
    let poster_key = s.seerr_summary_poster_key(&request.media_type, tmdb_id);
    let poster = poster_key
        .as_deref()
        .and_then(|key| poster_layers(s, key, SurfaceFit::Fill))
        .unwrap_or_else(|| placeholder(&title, t));
    let requester = request
        .requested_by
        .as_ref()
        .map(seerr_api::User::display_name)
        .unwrap_or("");
    let request_status = request.request_status();
    let media_status = request.media_status();
    let progress = request.progress_label();
    let fraction = request
        .progress_fraction()
        .filter(|fraction| *fraction > 0.0 && *fraction < 1.0);
    let item_id = request
        .media
        .jellyfin_media_id
        .clone()
        .or_else(|| s.seerr.library_id(&request.media_type, tmdb_id));
    let request_id = request.id;
    let media_type = request.media_type.clone();

    let mut details = col().grow(1.0).min_w(0.0).gap(5.0).child(
        row()
            .w_full()
            .gap(8.0)
            .items_center()
            .child(
                text(title)
                    .key(format!("seerr-title-{request_id}"))
                    .font(type_::title())
                    .color(t.text)
                    .max_lines(1)
                    .truncate(),
            )
            .children(
                (!year.is_empty()).then(|| text(year).font(type_::meta()).color(t.text_secondary)),
            )
            .child(type_chip(request.media_type.as_str(), t)),
    );
    details = details.child(
        row()
            .w_full()
            .gap(6.0)
            .items_center()
            .children(
                (!matches!(request_status, seerr_api::RequestStatus::Unknown)).then(|| {
                    theme::status::pill(request_status.label(), request_tone(request_status))
                }),
            )
            .children(
                (!matches!(media_status, seerr_api::MediaStatus::Unknown)).then(|| {
                    theme::status::pill(media_label(media_status), media_tone(media_status))
                }),
            )
            .children((!requester.is_empty()).then(|| {
                text(requester.to_string())
                    .font(type_::caption())
                    .color(t.text_secondary)
            })),
    );
    details = details.child(
        text(progress)
            .key(format!("seerr-progress-{request_id}"))
            .font(type_::meta())
            .color(t.text_secondary),
    );
    if let Some(fraction) = fraction {
        details = details.child(
            row()
                .key(format!("seerr-progress-bar-{request_id}"))
                .w_full()
                .h(5.0)
                .radius(3.0)
                .fill(t.card)
                .child(
                    el().grow(fraction.max(0.001))
                        .h_full()
                        .radius(3.0)
                        .fill(tok::accent(0, true))
                        .shadow(0.0, 4.0, tok::accent(0, true).alpha(0.30))
                        .child(
                            el().absolute(0.0, 0.0)
                                .w_full()
                                .h(1.0)
                                .fill(rune::Color::WHITE.alpha(0.35)),
                        ),
                ),
        );
    }
    for (index, group) in download_groups(request).into_iter().enumerate() {
        details = details.child(
            text(group)
                .key(format!("seerr-download-{request_id}-{index}"))
                .font(type_::caption())
                .color(t.text_secondary)
                .max_lines(2)
                .truncate(),
        );
    }

    row()
        .key(format!("seerr-request-{request_id}"))
        .w_full()
        .padding(12.0)
        .gap(14.0)
        .items_start()
        .fill(t.card)
        .radius(12.0)
        .outline(1.0, 0.0, t.card_border)
        .child(
            el().w(82.0)
                .h(123.0)
                .radius(8.0)
                .clip_rounded(8.0)
                .fill(t.bg)
                .child(poster),
        )
        .child(details)
        .children(item_id.map(|id| {
            el().key(format!("seerr-open-{request_id}"))
                .role("button")
                .h(32.0)
                .px(10.0)
                .radius(8.0)
                .center()
                .fill(t.bg)
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.97)
                .cursor(rune::Cursor::Pointer)
                .label("Open in Jellyfin")
                .on_press(move |s: &mut State| s.seerr_open_in_jellyfin(id.clone()))
                .child(
                    text("Open in Jellyfin")
                        .font(type_::caption())
                        .color(t.text),
                )
        }))
        .role("button")
        .focusable(true)
        .focus_visual(2.0)
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut State| s.open_seerr_detail(media_type.clone(), tmdb_id))
}

/// One line per unique download. A season pack repeats its size on every episode row, so the
/// episode count is shown instead of the size repeated.
fn download_groups(request: &seerr_api::MediaRequest) -> Vec<String> {
    let mut groups: Vec<(String, usize, u64, String, String)> = Vec::new();
    for download in &request.media.download_status {
        let key = if download.download_id.is_empty() {
            format!("{}:{}", download.title, download.size)
        } else {
            download.download_id.clone()
        };
        if let Some(group) = groups.iter_mut().find(|group| group.0 == key) {
            group.1 += download.episode.iter().count().max(1);
        } else {
            groups.push((
                key,
                download.episode.iter().count().max(1),
                download.size,
                download.status.clone(),
                download.title.clone(),
            ));
        }
    }
    groups
        .into_iter()
        .take(4)
        .map(|(_, episodes, size, status, title)| {
            if episodes > 1 {
                format!(
                    "{title} · {episodes} episodes · {:.1} GB · {status}",
                    size as f64 / 1_000_000_000.0
                )
            } else {
                format!("{title} · {status}")
            }
        })
        .collect()
}

fn type_chip(media_type: &str, t: &tok::Theme) -> El {
    let label = if media_type == "tv" { "Series" } else { "Film" };
    chip(label, t.bg, t.text_secondary)
}

fn request_tone(status: seerr_api::RequestStatus) -> rune::Color {
    use seerr_api::RequestStatus::*;
    match status {
        Pending | Approved => theme::status::info(),
        Completed => theme::status::success(),
        Declined | Failed => theme::status::danger(),
        Unknown => theme::status::neutral(),
    }
}

fn media_tone(status: seerr_api::MediaStatus) -> rune::Color {
    use seerr_api::MediaStatus::*;
    match status {
        Available => theme::status::success(),
        PartiallyAvailable | Processing => theme::status::warning(),
        Pending => theme::status::info(),
        Blocklisted | Deleted => theme::status::danger(),
        Unknown => theme::status::neutral(),
    }
}

fn media_label(status: seerr_api::MediaStatus) -> &'static str {
    match status {
        seerr_api::MediaStatus::PartiallyAvailable => "Partial",
        seerr_api::MediaStatus::Deleted => "Removed",
        other => other.label(),
    }
}

fn status_chip(status: seerr_api::MediaStatus, _t: &tok::Theme) -> El {
    if matches!(status, seerr_api::MediaStatus::Unknown) {
        return el();
    }
    theme::status::overlay(media_label(status), media_tone(status))
}

fn chip(label: &str, fill: rune::Color, color: rune::Color) -> El {
    el().h(20.0)
        .px(6.0)
        .radius(5.0)
        .center()
        .fill(fill)
        .child(text(label.to_string()).font(type_::caption()).color(color))
}

fn placeholder(title: &str, t: &tok::Theme) -> El {
    el().w_full().h_full().fill(t.card).center().child(
        text(initial(title))
            .font(type_::title())
            .color(t.text_secondary),
    )
}

fn message_card(message: &str, key: &str, t: &tok::Theme) -> El {
    row()
        .key(key)
        .w_full()
        .padding(14.0)
        .radius(10.0)
        .fill(t.card)
        .outline(1.0, 0.0, t.card_border)
        .child(text(message.to_string()).font(type_::body()).color(t.text))
}

fn loading_rows(t: &tok::Theme) -> El {
    let mut body = col().key("seerr-loading").w_full().gap(10.0);
    for _ in 0..4 {
        body = body.child(el().w_full().h(70.0).radius(10.0).fill(t.card).outline(
            1.0,
            0.0,
            t.card_border,
        ));
    }
    body
}

// ---- Detail and request panel -----------------------------------------------------------

pub(super) fn seerr_detail_screen(s: &State) -> El {
    let t = theme::colors();
    row()
        .key("seerr-detail-screen")
        .w_full()
        .h_full()
        .fill(t.bg)
        .child(sidebar(s, &t))
        .child(el().w(1.0).h_full().fill(t.separator))
        .child(
            col()
                .key("seerr-detail-content")
                .grow(1.0)
                .min_w(0.0)
                .min_h(0.0)
                .h_full()
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(detail_header(s, &t))
                .child(detail_body(s, &t)),
        )
}

fn detail_header(s: &State, t: &tok::Theme) -> El {
    let title = s
        .seerr
        .detail
        .as_ref()
        .and_then(|detail| detail.details.as_ref())
        .map(seerr_api::Details::title)
        .unwrap_or("Seerr");
    let back = match s.seerr.detail_tab {
        SeerrTab::Discover => "← Discover",
        SeerrTab::Requests => "← Requests",
    };
    row()
        .key("seerr-detail-header")
        .w_full()
        .padding(theme::gutter())
        .pb(8.0)
        .gap(12.0)
        .items_center()
        .child(
            el().key("seerr-detail-back")
                .role("button")
                .h(32.0)
                .px(12.0)
                .radius(8.0)
                .center()
                .fill(t.card)
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.97)
                .cursor(rune::Cursor::Pointer)
                .label("Back")
                .on_press(|s: &mut State| s.back_from_seerr_detail())
                .child(text(back).font(type_::label()).color(t.text)),
        )
        .child(
            text(title.to_string())
                .key("seerr-detail-title")
                .font(type_::page())
                .color(t.text),
        )
        .child(el().grow(1.0))
        .child(
            text("Seerr")
                .key("seerr-detail-service")
                .font(type_::caption())
                .color(t.text_secondary),
        )
}

fn detail_body(s: &State, t: &tok::Theme) -> El {
    let Some(detail) = &s.seerr.detail else {
        return col().key("seerr-detail-missing").child(
            text("Choose a title on Discover.")
                .font(type_::body())
                .color(t.text_secondary),
        );
    };
    if detail.loading {
        return col()
            .key("seerr-detail-loading")
            .padding(theme::gutter())
            .child(loading_rows(t));
    }
    if let Some(error) = &detail.error {
        return col()
            .key("seerr-detail-error")
            .padding(theme::gutter())
            .gap(10.0)
            .child(message_card(error, "seerr-detail-message", t))
            .child(
                el().key("seerr-detail-retry")
                    .role("button")
                    .h(32.0)
                    .px(12.0)
                    .radius(8.0)
                    .center()
                    .fill(t.card)
                    .hover(t.hover)
                    .focusable(true)
                    .focus_visual(2.0)
                    .press_scale(0.97)
                    .cursor(rune::Cursor::Pointer)
                    .on_press(|s: &mut State| {
                        if let Some(detail) = &s.seerr.detail {
                            s.open_seerr_detail(detail.media_type.clone(), detail.tmdb_id);
                        }
                    })
                    .child(text("Try again").font(type_::label()).color(t.text)),
            );
    }
    let Some(details) = &detail.details else {
        return col()
            .key("seerr-detail-empty")
            .padding(theme::gutter())
            .child(
                text("Seerr returned no details.")
                    .font(type_::body())
                    .color(t.text_secondary),
            );
    };
    let poster_key = s.seerr_detail_poster_key();
    let poster = poster_key
        .as_deref()
        .and_then(|key| poster_layers(s, key, SurfaceFit::Fill))
        .unwrap_or_else(|| placeholder(details.title(), t));
    let backdrop = s
        .seerr_detail_backdrop_key()
        .as_deref()
        .and_then(|key| poster_layers(s, key, SurfaceFit::Fill));
    let media_status = details
        .media_info()
        .map(|info| info.media_status())
        .unwrap_or(seerr_api::MediaStatus::Unknown);
    let overview = details.overview().unwrap_or("No overview yet.");

    let mut info = col().grow(1.0).min_w(0.0).gap(10.0).child(
        row()
            .w_full()
            .gap(12.0)
            .items_center()
            .child(type_chip(&detail.media_type, t))
            .children(details.year().map(|year| {
                text(year.to_string())
                    .font(type_::meta())
                    .color(t.text_secondary)
            }))
            .child(status_chip(media_status, t)),
    );
    if let seerr_api::Details::Movie(movie) = details
        && let Some(runtime) = movie.runtime
    {
        info = info.child(
            text(format!("{runtime} min"))
                .font(type_::meta())
                .color(t.text_secondary),
        );
    }
    let genres: Vec<&str> = details
        .genres()
        .iter()
        .filter_map(|genre| genre.name.as_deref())
        .collect();
    if !genres.is_empty() {
        info = info.child(
            text(genres.join(" · "))
                .key("seerr-detail-genres")
                .font(type_::meta())
                .color(t.text_secondary),
        );
    }
    if let Some(tagline) = details.tagline() {
        info = info.child(
            text(tagline.to_string())
                .key("seerr-detail-tagline")
                .font(type_::meta())
                .color(t.text_secondary)
                .max_lines(2),
        );
    }
    info = info
        .child(
            text(overview.to_string())
                .key("seerr-detail-overview")
                .font(type_::overview())
                .color(t.text)
                .max_lines(6),
        )
        .child(detail_action(s, details, media_status, t));
    if let seerr_api::Details::Tv(tv) = details
        && !tv.seasons.is_empty()
    {
        info = info.child(season_picker(s, tv, t));
    }

    let mut page = col()
        .key("seerr-detail-body")
        .w_full()
        .padding(theme::gutter())
        .gap(18.0);
    if let Some(backdrop) = backdrop {
        page = page.child(
            el().key("seerr-detail-backdrop")
                .w_full()
                .h(220.0)
                .radius(14.0)
                .clip_rounded(14.0)
                .fill(t.card)
                .child(backdrop)
                .child(el().absolute(0.0, 0.0).w_full().h_full().linear_gradient(
                    std::f32::consts::PI,
                    [(0.0, t.bg.alpha(0.10)), (1.0, t.bg.alpha(0.92))],
                )),
        );
    }
    let page = page.child(
        row()
            .w_full()
            .gap(22.0)
            .items_start()
            .child(
                el().w(210.0)
                    .h(315.0)
                    .radius(12.0)
                    .clip_rounded(12.0)
                    .fill(t.card)
                    .outline(1.0, 0.0, t.card_border)
                    .shadow(12.0, 28.0, t.shadow_soft)
                    .child(poster),
            )
            .child(info),
    );
    match cast_rail(s, details, t) {
        Some(cast) => page.child(cast),
        None => page,
    }
}

/// The billed cast, each face opening the person page that joins the library and Seerr.
fn cast_rail(s: &State, details: &seerr_api::Details, t: &tok::Theme) -> Option<El> {
    let cast: Vec<&seerr_api::CastMember> = details
        .credits()
        .cast
        .iter()
        .filter(|member| member.name.is_some())
        .take(12)
        .collect();
    if cast.is_empty() {
        return None;
    }
    let mut rail = row().gap(14.0).focus_group();
    for member in cast {
        let name = member.name.clone().unwrap_or_default();
        let character = member.character.clone().unwrap_or_default();
        let profile = member.profile_path.clone();
        let portrait = profile
            .as_deref()
            .map(|path| s.seerr_profile_key(member.id, path, 160))
            .and_then(|key| poster_layers(s, &key, SurfaceFit::Fill))
            .unwrap_or_else(|| {
                el().w_full().h_full().center().child(
                    text(
                        name.chars()
                            .next()
                            .map_or_else(|| "?".to_string(), |c| c.to_uppercase().to_string()),
                    )
                    .font(type_::title())
                    .color(t.text_secondary),
                )
            });
        let click_name = name.clone();
        let id = member.id;
        rail = rail.child(
            col()
                .key(format!("seerr-cast-{}", id))
                .role("button")
                .w(112.0)
                .gap(6.0)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.98)
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut State| {
                    s.open_seerr_person(id, click_name.clone(), profile.clone())
                })
                .child(
                    el().w(96.0)
                        .h(96.0)
                        .radius(48.0)
                        .clip_rounded(48.0)
                        .fill(t.card)
                        .child(portrait),
                )
                .child(
                    text(name)
                        .key(format!("seerr-cast-name-{}", id))
                        .font(type_::card_title())
                        .color(t.text)
                        .max_lines(1)
                        .truncate(),
                )
                .child(
                    text(character)
                        .key(format!("seerr-cast-character-{}", id))
                        .font(type_::caption())
                        .color(t.text_secondary)
                        .max_lines(1)
                        .truncate(),
                ),
        );
    }
    Some(
        col()
            .key("seerr-detail-cast")
            .w_full()
            .gap(12.0)
            .child(text("Cast").font(type_::section()).color(t.text))
            .child(el().w_full().scroll_x().scroll_edge_fade(28.0).child(rail)),
    )
}

fn detail_action(
    s: &State,
    details: &seerr_api::Details,
    media_status: seerr_api::MediaStatus,
    t: &tok::Theme,
) -> El {
    let Some(detail) = &s.seerr.detail else {
        return el();
    };
    // Seerr's own link is the fast path; a partially downloaded series can have episodes on the
    // server while its media row still says Processing, so the library lookup fills the gap.
    let library_id = details
        .media_info()
        .and_then(|info| info.jellyfin_media_id.clone())
        .or_else(|| s.seerr.library_id(&detail.media_type, detail.tmdb_id));
    let mut actions = row().key("seerr-detail-actions").gap(8.0).items_center();
    if let Some(id) = library_id.clone() {
        actions = actions.child(
            el().key("seerr-detail-open")
                .role("button")
                .h(38.0)
                .px(16.0)
                .radius(9.0)
                .center()
                .fill(t.card)
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.97)
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut State| s.seerr_open_in_jellyfin(id.clone()))
                .child(text("Open in Jellyfin").font(type_::label()).color(t.text)),
        );
    }
    if media_status == seerr_api::MediaStatus::Available {
        if library_id.is_none() {
            actions = actions.child(
                text("In your library")
                    .key("seerr-detail-available")
                    .font(type_::label())
                    .color(t.text_secondary),
            );
        }
        return actions;
    }
    if matches!(
        media_status,
        seerr_api::MediaStatus::Processing
            | seerr_api::MediaStatus::Pending
            | seerr_api::MediaStatus::Blocklisted
    ) {
        if library_id.is_none() {
            let message = match media_status {
                seerr_api::MediaStatus::Processing => "Seerr is fetching this title.",
                seerr_api::MediaStatus::Pending => "Waiting for approval.",
                _ => "This title is not requestable.",
            };
            actions = actions.child(
                text(message)
                    .key("seerr-detail-status")
                    .font(type_::label())
                    .color(t.text_secondary),
            );
        }
        return actions;
    }
    if matches!(details, seerr_api::Details::Tv(_)) && detail.selected_seasons.is_empty() {
        return actions.child(
            text("Choose at least one season")
                .key("seerr-detail-no-seasons")
                .font(type_::label())
                .color(t.text_secondary),
        );
    }
    if detail.requesting {
        return actions.child(
            text("Requesting…")
                .key("seerr-detail-requesting")
                .font(type_::label())
                .color(t.text_secondary),
        );
    }
    actions.child(
        el().key("seerr-detail-request")
            .role("button")
            .h(38.0)
            .px(16.0)
            .radius(9.0)
            .center()
            .fill(tok::accent(0, true))
            .hover(t.hover)
            .motion(motion::SNAPPY)
            .focusable(true)
            .focus_visual(2.0)
            .press_scale(0.97)
            .cursor(rune::Cursor::Pointer)
            .on_press(|s: &mut State| s.request_seerr_selected())
            .child(
                text(match details {
                    seerr_api::Details::Movie(_) => "Request film",
                    seerr_api::Details::Tv(_) => "Request seasons",
                })
                .font(type_::label())
                .color(t.bg),
            ),
    )
}

fn season_picker(s: &State, tv: &seerr_api::TvDetails, t: &tok::Theme) -> El {
    let selected = s
        .seerr
        .detail
        .as_ref()
        .map(|detail| &detail.selected_seasons);
    let mut list = col().key("seerr-detail-seasons").w_full().gap(4.0).child(
        row()
            .w_full()
            .gap(8.0)
            .items_center()
            .child(text("Seasons").font(type_::label()).color(t.text_secondary))
            .child(el().grow(1.0))
            .child(
                el().key("seerr-season-all")
                    .role("button")
                    .h(26.0)
                    .px(8.0)
                    .radius(6.0)
                    .center()
                    .fill(t.card)
                    .hover(t.hover)
                    .motion(motion::SNAPPY)
                    .focusable(true)
                    .focus_visual(2.0)
                    .press_scale(0.97)
                    .cursor(rune::Cursor::Pointer)
                    .on_press(|s: &mut State| s.select_all_seerr_seasons())
                    .child(text("All").font(type_::caption()).color(t.text)),
            ),
    );
    for season in tv
        .seasons
        .iter()
        .filter(|season| season.season_number > 0 && season.episode_count > 0)
    {
        let number = season.season_number;
        let on = selected.is_some_and(|selected| selected.contains(&number));
        list = list.child(
            el().key(format!("seerr-season-{number}"))
                .role("checkbox")
                .h(28.0)
                .px(8.0)
                .radius(6.0)
                .gap(8.0)
                .center_y()
                .fill(if on { t.card } else { rune::Color::TRANSPARENT })
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.98)
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut State| s.toggle_seerr_season(number))
                .child(
                    text(if on { "✓" } else { "○" })
                        .font(type_::caption())
                        .color(if on { t.text } else { t.text_secondary }),
                )
                .child(
                    text(format!(
                        "Season {} · {} episodes",
                        season.season_number, season.episode_count
                    ))
                    .font(type_::caption())
                    .color(t.text_secondary),
                ),
        );
    }
    list
}
