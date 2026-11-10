//! The search screen: one field, grouped rails, and cards that open detail.

use jellyfin_api::Item;
use rune::tok::{self, motion};

use crate::theme::{self, type_};
use rune::{El, ScrollSpec, col, el, row, text, text_input};

use crate::state::State;

use super::{CARD_GAP, card, sidebar};

pub(super) fn search_screen(s: &State) -> El {
    let t = theme::colors();
    let results = &s.search.results;
    let input = text_input(&s.search.query, type_::body())
        .key("search-input")
        .label("Search")
        .role("textbox")
        .single_line()
        .w_full()
        .on_edit(|s: &mut State| &mut s.search.query)
        .on_input_change(|s: &mut State, _| s.search_changed())
        .on_input_key(|s: &mut State, key: &rune::KeyEvent, _tx| {
            if key.key == rune::Key::Escape {
                s.close_search();
                true
            } else {
                false
            }
        })
        .color(t.text);
    let input_box = el()
        .key("search-box")
        .h(42.0)
        .w(640.0)
        .max_w(640.0)
        .min_w(0.0)
        .radius(10.0)
        .px(12.0)
        .center_y()
        .fill(t.card)
        .outline(
            if s.search.query.focused { 1.5 } else { 0.0 },
            0.0,
            tok::accent(0, true),
        )
        .child(input);

    let header = row()
        .key("search-header")
        .w_full()
        .gap(16.0)
        .child(text("Search").font(type_::page()).color(t.text))
        .child(input_box);

    let mut body = col()
        .key("search-results")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .pb(if s.in_app_minimized { 124.0 } else { 0.0 })
        .gap(28.0);

    if let Some(scope) = &s.search.scope {
        let name = s
            .home_data
            .libraries
            .iter()
            .find(|library| &library.id == scope)
            .map_or_else(
                || "this library".to_string(),
                |library| library.name.clone(),
            );
        body = body.child(
            row()
                .key("search-scope")
                .gap(8.0)
                .items_center()
                .child(
                    text(format!("In {name}"))
                        .font(type_::caption())
                        .color(t.text_secondary),
                )
                .child(
                    el().key("search-scope-clear")
                        .role("button")
                        .w(24.0)
                        .h(24.0)
                        .radius(6.0)
                        .center()
                        .fill(t.card)
                        .hover(t.hover)
                        .focusable(true)
                        .focus_ring(theme::focus_width(), 2.0, theme::ring())
                        .press_scale(0.94)
                        .child(text("×").font(type_::label()).color(t.text_secondary))
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| s.open_search(None)),
                ),
        );
    }

    if let Some(error) = &s.search.error {
        body = body.child(
            col()
                .gap(10.0)
                .child(
                    text(error.clone())
                        .key("search-error")
                        .font(type_::meta())
                        .color(tok::accent(2, true)),
                )
                .child(
                    el().key("search-retry")
                        .role("button")
                        .h(32.0)
                        .px(14.0)
                        .radius(8.0)
                        .center()
                        .fill(t.card)
                        .hover(t.hover)
                        .focusable(true)
                        .focus_ring(theme::focus_width(), 2.0, theme::ring())
                        .press_scale(0.97)
                        .child(text("Try again").font(type_::label()).color(t.text))
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| s.search_changed()),
                ),
        );
    } else if s.search.query.text.trim().chars().count() < 2 {
        let has_recents = !s.search.recents.is_empty();
        let terms: Vec<String> = if has_recents {
            s.search.recents.clone()
        } else {
            ["The Quiet Floor", "Nosferatu", "Service", "Harbour"]
                .into_iter()
                .map(str::to_string)
                .collect()
        };
        let idle = col()
            .key("search-idle")
            .gap(14.0)
            .items_start()
            .child(text("Find something to watch").font(type_::section()).color(t.text))
            .child(
                text("Search by title, actor, or genre. Ctrl+K searches everything; Ctrl+F stays in this library.")
                    .key("search-hint")
                    .font(type_::meta())
                    .color(t.text_secondary),
            )
            .child(
                text(if has_recents {
                    "Recent searches"
                } else {
                    "Try a search"
                })
                .font(type_::caption())
                .color(t.text_secondary),
            )
            .child(
                row()
                    .key("search-suggestions")
                    .gap(8.0)
                    .focus_group()
                    .children(terms.into_iter().map(|term| {
                        let key = format!(
                            "{}-{}",
                            if has_recents {
                                "search-recent"
                            } else {
                                "search-suggestion"
                            },
                            term
                        );
                        el().key(key)
                            .role("button")
                            .h(32.0)
                            .px(14.0)
                            .radius(8.0)
                            .center()
                            .fill(t.card)
                            .hover(t.hover)
                            .focusable(true)
                            .focus_ring(theme::focus_width(), 2.0, theme::ring())
                            .press_scale(0.97)
                            .child(text(term.clone()).font(type_::label()).color(t.text))
                            .cursor(rune::Cursor::Pointer)
                            .on_press(move |s: &mut State| s.search_for(&term))
                    })),
            );
        body = body.child(idle);
    } else if s.search.searching && results.is_empty() {
        body = body.child(
            text("Searching…")
                .key("search-status")
                .font(type_::meta())
                .color(t.text_secondary),
        );
    } else if results.is_empty() {
        let scope = s.search.scope.is_some();
        let mut empty = col()
            .key("search-empty-state")
            .gap(10.0)
            .items_start()
            .child(text("No matches").font(type_::section()).color(t.text))
            .child(
                text(format!(
                    "Nothing matched “{}”{}.",
                    s.search.query.text.trim(),
                    if scope { " in this library" } else { "" }
                ))
                .key("search-empty")
                .font(type_::meta())
                .color(t.text_secondary),
            )
            .child(
                row()
                    .gap(8.0)
                    .child(
                        el().key("search-clear")
                            .role("button")
                            .h(32.0)
                            .px(14.0)
                            .radius(8.0)
                            .center()
                            .fill(t.card)
                            .hover(t.hover)
                            .focusable(true)
                            .focus_ring(theme::focus_width(), 2.0, theme::ring())
                            .press_scale(0.97)
                            .child(text("Clear search").font(type_::label()).color(t.text))
                            .cursor(rune::Cursor::Pointer)
                            .on_press(|s: &mut State| s.clear_search()),
                    )
                    .children(scope.then(|| {
                        el().key("search-all-libraries")
                            .role("button")
                            .h(32.0)
                            .px(14.0)
                            .radius(8.0)
                            .center()
                            .fill(t.card)
                            .hover(t.hover)
                            .focusable(true)
                            .focus_ring(theme::focus_width(), 2.0, theme::ring())
                            .press_scale(0.97)
                            .child(
                                text("Search all libraries")
                                    .font(type_::label())
                                    .color(t.text),
                            )
                            .cursor(rune::Cursor::Pointer)
                            .on_press(|s: &mut State| s.open_search(None))
                    })),
            );
        if !scope {
            empty = empty.child(
                text("Try a shorter title, an actor, or a genre.")
                    .font(type_::meta())
                    .color(t.text_secondary),
            );
        }
        body = body.child(empty);
        if let Some(seerr) = seerr_section(s, &t) {
            body = body.child(seerr);
        }
    } else {
        body = body
            .child(rail("Movies", "search-movies", &results.movies, s, &t))
            .child(rail("Shows", "search-shows", &results.shows, s, &t))
            .child(rail(
                "Episodes",
                "search-episodes",
                &results.episodes,
                s,
                &t,
            ))
            .child(people(&results.people, &t));
        if let Some(seerr) = seerr_section(s, &t) {
            body = body.child(seerr);
        }
    }

    row()
        .key("search-screen")
        .w_full()
        .h_full()
        .fill(t.bg)
        .child(sidebar(s, &t))
        .child(el().w(1.0).h_full().fill(t.separator))
        .child(
            col()
                .key("search-content")
                .grow(1.0)
                .min_w(0.0)
                .min_h(0.0)
                .h_full()
                .padding(theme::gutter())
                .gap(22.0)
                .child(header)
                .children(search_hints(s, &t))
                .child(body),
        )
}

/// The same query against Seerr's catalogue, so one search covers the server and what could be
/// requested. Availability chips tell the two apart; a card opens the Seerr detail.
fn seerr_section(s: &State, t: &tok::Theme) -> Option<El> {
    if !s.seerr_configured() {
        return None;
    }
    if s.search.seerr_loading && s.search.seerr.results.is_empty() {
        return Some(
            text("Searching Seerr…")
                .key("search-seerr-loading")
                .font(type_::meta())
                .color(t.text_secondary),
        );
    }
    if let Some(error) = &s.search.seerr_error {
        return Some(
            text(format!("Seerr: {error}"))
                .key("search-seerr-error")
                .font(type_::meta())
                .color(t.text_secondary),
        );
    }
    let items: Vec<&seerr_api::DiscoverItem> = s
        .search
        .seerr
        .results
        .iter()
        .filter(|item| item.is_movie() || item.is_tv())
        .take(20)
        .collect();
    if items.is_empty() {
        return None;
    }
    Some(
        col()
            .key("search-seerr")
            .w_full()
            .gap(12.0)
            .child(
                text("On Seerr")
                    .key("search-seerr-title")
                    .font(type_::section())
                    .color(t.text),
            )
            .child(
                el().w_full().scroll_x().scroll_edge_fade(28.0).child(
                    row().gap(CARD_GAP).focus_group().children(
                        items
                            .into_iter()
                            .map(|item| super::seerr::discover_card(item, s, t)),
                    ),
                ),
            ),
    )
}

/// `/Search/Hints` under the field: names the result rails do not show as rows, such as
/// people, genres and studios. A chip fills the field rather than opening a route, so the
/// reader stays in one search.
fn search_hints(s: &State, t: &tok::Theme) -> Option<El> {
    if s.search.query.text.trim().chars().count() < 2 || s.search.hints.is_empty() {
        return None;
    }
    let mut chips = row().key("search-hints").w_full().gap(6.0).flex_wrap();
    for hint in s.search.hints.iter().take(8) {
        let name = hint.name.clone();
        let label = if hint.kind.is_empty() {
            hint.name.clone()
        } else {
            format!("{} · {}", hint.name, hint.kind)
        };
        chips = chips.child(
            el().key(format!("search-hint-{}", hint.name))
                .role("button")
                .h(28.0)
                .px(10.0)
                .radius(14.0)
                .center()
                .fill(t.card)
                .outline(1.0, 0.0, t.card_border)
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_ring(theme::focus_width(), 2.0, theme::ring())
                .press_scale(0.97)
                .cursor(rune::Cursor::Pointer)
                .child(text(label).font(type_::caption()).color(t.text_secondary))
                .on_press(move |s: &mut State| s.search_for(&name)),
        );
    }
    Some(chips)
}

fn rail(title: &str, key: &str, items: &[Item], s: &State, t: &tok::Theme) -> El {
    if items.is_empty() {
        return col();
    }
    let expanded = s.is_search_expanded(key);
    let mut header = row()
        .w_full()
        .items_end()
        .gap(12.0)
        .child(
            text(title)
                .key(format!("{key}-title"))
                .font(type_::section())
                .color(t.text_secondary)
                .grow(1.0),
        )
        .child(
            el().key(format!("{key}-count")).child(
                text(if items.len() == 1 {
                    "1 result".to_string()
                } else {
                    super::count_label(items.len(), "result")
                })
                .font(type_::caption())
                .color(t.text_secondary),
            ),
        );
    if items.len() > 6 {
        let rail_key = key.to_string();
        header = header.child(
            el().key(format!("{key}-all"))
                .role("button")
                .h(26.0)
                .px(8.0)
                .radius(6.0)
                .center()
                .fill(t.bg)
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_ring(theme::focus_width(), 2.0, theme::ring())
                .press_scale(0.96)
                .child(
                    text(if expanded { "Show less" } else { "See all" })
                        .font(type_::label())
                        .color(t.text_secondary),
                )
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut State| s.toggle_search_expanded(&rail_key)),
        );
    }
    let body = if expanded {
        // Five 176 pt cards fit the content column at 1280; rows are explicit because Rune
        // has no wrap layout and a horizontal rail of forty results is not a grid.
        let mut grid = col()
            .key(format!("{key}-grid"))
            .w_full()
            .gap(CARD_GAP)
            .items_start();
        for chunk in items.chunks(5) {
            grid = grid.child(
                row()
                    .gap(CARD_GAP)
                    .items_start()
                    .children(chunk.iter().map(|item| card(item, key, s, t))),
            );
        }
        grid
    } else {
        el().w_full()
            .min_w(0.0)
            .scroll_x()
            .scroll_edge_fade(24.0)
            .child(
                row()
                    .gap(CARD_GAP)
                    .focus_group()
                    .children(items.iter().take(6).map(|item| card(item, key, s, t))),
            )
    };
    col()
        .key(key)
        .w_full()
        .min_w(0.0)
        .gap(12.0)
        .child(header)
        .child(body)
}

fn people(items: &[Item], t: &tok::Theme) -> El {
    if items.is_empty() {
        return col();
    }
    col()
        .key("search-people")
        .w_full()
        .gap(8.0)
        .child(
            text("People")
                .key("search-people-title")
                .font(type_::section())
                .color(t.text_secondary),
        )
        .child(row().gap(10.0).children(items.iter().take(6).map(|item| {
            let id = item.id.clone();
            let name = item.name.clone();
            let image = item
                .image_tag(jellyfin_api::ImageKind::Primary)
                .map(str::to_string);
            el().key(format!("search-person-{}", item.id))
                .role("button")
                .h(28.0)
                .px(10.0)
                .radius(7.0)
                .center()
                .fill(t.card)
                .hover(t.hover)
                .press_scale(0.96)
                .focusable(true)
                .focus_ring(theme::focus_width(), 2.0, theme::ring())
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut State| {
                    s.open_person(&jellyfin_api::Person {
                        id: id.clone(),
                        name: name.clone(),
                        primary_image_tag: image.clone(),
                        ..jellyfin_api::Person::default()
                    });
                })
                .child(text(item.name.clone()).font(type_::label()).color(t.text))
        })))
}
