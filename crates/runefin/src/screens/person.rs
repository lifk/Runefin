//! A person's filmography: the cast card's destination. The library grid is the server's own
//! credits; when Seerr is configured, the same person's TMDB credits that are not on the server
//! follow as a second rail, so a face can lead to something new to request.

use jellyfin_api::Item;
use rune::tok::{self, motion};
use rune::{El, ScrollSpec, SurfaceFit, col, el, row, text};

use crate::state::{PersonPage, State};
use crate::theme::{self, type_};

pub(super) fn person_screen(s: &State, person: &PersonPage) -> El {
    let t = theme::colors();
    let image = s
        .person_portrait_key()
        .and_then(|key| super::poster_layers(s, &key, SurfaceFit::Fill))
        .unwrap_or_else(|| {
            el().w_full().h_full().center().child(
                text(
                    person
                        .name
                        .chars()
                        .next()
                        .map_or_else(|| "?".to_string(), |c| c.to_uppercase().to_string()),
                )
                .font(type_::page())
                .color(t.text_secondary),
            )
        });
    let header = col().w_full().gap(10.0).child(
        row()
            .w_full()
            .gap(20.0)
            .items_center()
            .child(
                el().w(104.0)
                    .h(104.0)
                    .radius(52.0)
                    .clip_rounded(52.0)
                    .fill(t.card)
                    .child(image),
            )
            .child(
                col()
                    .grow(1.0)
                    .gap(4.0)
                    .child(
                        text(person.name.clone())
                            .key("person-name")
                            .font(type_::page())
                            .color(t.text)
                            .truncate(),
                    )
                    .child(
                        text(facts_line(person))
                            .key("person-count")
                            .font(type_::meta())
                            .color(t.text_secondary),
                    ),
            ),
    );
    let header = header.children(person.biography.clone().map(|biography| {
        text(biography)
            .key("person-biography")
            .font(type_::meta())
            .color(t.text_secondary)
            .max_lines(3)
    }));
    let body = body(s, person, &t);
    row()
        .key("person-screen")
        .w_full()
        .h_full()
        .fill(t.bg)
        .child(super::sidebar(s, &t))
        .child(el().w(1.0).h_full().fill(t.separator))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .min_h(0.0)
                .h_full()
                .padding(theme::gutter())
                .gap(22.0)
                .child(
                    el().key("person-back")
                        .role("button")
                        .h(32.0)
                        .w(92.0)
                        .px(12.0)
                        .radius(8.0)
                        .center()
                        .fill(t.card)
                        .hover(t.hover)
                        .motion(motion::SNAPPY)
                        .focusable(true)
                        .focus_ring(theme::focus_width(), 2.0, theme::FOCUS)
                        .press_scale(0.97)
                        .child(text("← Back").font(type_::label()).color(t.text))
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| s.back_from_person()),
                )
                .child(header)
                .child(
                    el().grow(1.0)
                        .min_h(0.0)
                        .w_full()
                        .scroll(ScrollSpec {
                            stick_bottom: false,
                        })
                        .child(body),
                ),
        )
}

fn facts_line(person: &PersonPage) -> String {
    let mut parts = Vec::new();
    if person.loading {
        parts.push("Loading credits…".to_string());
    } else if person.items.len() == 1 {
        parts.push("1 title in your library".to_string());
    } else {
        parts.push(format!("{} titles in your library", person.items.len()));
    }
    if let Some(birthday) = &person.birthday {
        parts.push(birthday.clone());
    }
    if let Some(place) = &person.place_of_birth {
        parts.push(place.clone());
    }
    parts.join(" · ")
}

fn body(s: &State, person: &PersonPage, t: &tok::Theme) -> El {
    if person.loading && person.items.is_empty() && person.seerr_credits.is_empty() {
        return grid_skeleton(t);
    }
    let mut body = col().key("person-body").w_full().gap(20.0);
    if let Some(error) = &person.error {
        body = body.child(error_card(error, t));
    } else if person.items.is_empty() {
        if !person.loading {
            body = body.child(quiet(
                "Not in your library",
                "No movies or series with this person are on the server.",
                t,
            ));
        }
    } else {
        if person.items.len() > 2 {
            body = body
                .child(
                    text("Known for")
                        .key("person-known-title")
                        .font(type_::section())
                        .color(t.text),
                )
                .child(
                    el().w_full().scroll_x().scroll_edge_fade(28.0).child(
                        row().gap(theme::CARD_GAP).focus_group().children(
                            person
                                .items
                                .iter()
                                .take(5)
                                .map(|item| super::card(item, "person-known", s, t)),
                        ),
                    ),
                );
        }
        body = body
            .child(section_heading("In your library", t))
            .child(grid(s, &person.items, t));
    }
    if person.seerr_loading && person.seerr_credits.is_empty() {
        body = body.child(quiet(
            "Looking on Seerr…",
            "Fetching the rest of this filmography.",
            t,
        ));
    } else if let Some(error) = &person.seerr_error {
        body = body.child(
            text(format!("Seerr: {error}"))
                .key("person-seerr-error")
                .font(type_::caption())
                .color(t.text_secondary),
        );
    } else {
        let credits = person.seerr_only();
        if !credits.is_empty() {
            body = body.child(seerr_rail(s, person, &credits, t));
        }
    }
    body
}

fn section_heading(label: &str, t: &tok::Theme) -> El {
    text(label.to_string()).font(type_::section()).color(t.text)
}

fn seerr_rail(
    s: &State,
    person: &PersonPage,
    credits: &[&seerr_api::Credit],
    t: &tok::Theme,
) -> El {
    let mut rail = row().gap(theme::CARD_GAP).focus_group();
    for credit in credits.iter().take(24) {
        rail = rail.child(credit_card(s, credit, t));
    }
    col()
        .key("person-seerr")
        .w_full()
        .gap(12.0)
        .child(
            text(format!("More with {} on Seerr", person.name))
                .key("person-seerr-title")
                .font(type_::section())
                .color(t.text),
        )
        .child(el().w_full().scroll_x().scroll_edge_fade(28.0).child(rail))
}

fn credit_card(s: &State, credit: &seerr_api::Credit, t: &tok::Theme) -> El {
    let art = s
        .credit_poster_key(credit)
        .and_then(|key| super::poster_layers(s, &key, SurfaceFit::Fill))
        .unwrap_or_else(|| {
            el().w_full().h_full().center().child(
                text(
                    credit
                        .title()
                        .chars()
                        .next()
                        .map_or_else(|| "?".to_string(), |c| c.to_uppercase().to_string()),
                )
                .font(type_::page())
                .color(t.text_secondary),
            )
        });
    let media = credit.media_type.clone().unwrap_or_default();
    let tmdb_id = credit.id;
    let facts = match (credit.year(), credit.character.as_deref()) {
        (Some(year), Some(character)) => format!("{year} · {character}"),
        (Some(year), None) => year.to_string(),
        (None, Some(character)) => character.to_string(),
        (None, None) => String::new(),
    };
    col()
        .key(format!("person-seerr-{}", credit.id))
        .role("button")
        .w(theme::CARD_W)
        .gap(6.0)
        .press_scale(0.98)
        .hover(t.hover)
        .focusable(true)
        .focus_ring(theme::focus_width(), 2.0, theme::FOCUS)
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut State| s.open_seerr_detail(media.clone(), tmdb_id))
        .child(
            el().w_full()
                .h(theme::CARD_H)
                .radius(10.0)
                .clip_rounded(10.0)
                .fill(t.card)
                .child(art),
        )
        .child(
            text(credit.title().to_string())
                .font(type_::card_title())
                .color(t.text)
                .max_lines(1)
                .truncate(),
        )
        .child(
            text(facts)
                .font(type_::caption())
                .color(t.text_secondary)
                .max_lines(1)
                .truncate(),
        )
}

fn quiet(title: &str, detail: &str, t: &tok::Theme) -> El {
    col()
        .w_full()
        .gap(6.0)
        .child(text(title).font(type_::section()).color(t.text))
        .child(text(detail).font(type_::meta()).color(t.text_secondary))
}

fn error_card(error: &str, t: &tok::Theme) -> El {
    col()
        .key("person-error")
        .w_full()
        .gap(10.0)
        .child(
            text("Can't load credits")
                .font(type_::section())
                .color(t.text),
        )
        .child(
            text(error.to_string())
                .key("person-error-message")
                .font(type_::meta())
                .color(t.text_secondary),
        )
        .child(
            el().key("person-retry")
                .role("button")
                .h(32.0)
                .px(12.0)
                .radius(8.0)
                .center()
                .fill(t.card)
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .press_scale(0.97)
                .child(text("Try again").font(type_::label()).color(t.text))
                .cursor(rune::Cursor::Pointer)
                .on_press(|s: &mut State| s.retry_person()),
        )
}

fn grid(s: &State, items: &[Item], t: &tok::Theme) -> El {
    let mut grid = col().key("person-credits").w_full().gap(theme::CARD_GAP);
    for chunk in items.chunks(5) {
        grid = grid.child(
            row()
                .gap(theme::CARD_GAP)
                .items_start()
                .children(chunk.iter().map(|item| super::card(item, "person", s, t))),
        );
    }
    grid
}

fn grid_skeleton(t: &tok::Theme) -> El {
    let mut grid = col().key("person-loading").w_full().gap(theme::CARD_GAP);
    for _ in 0..2 {
        grid = grid.child(row().gap(theme::CARD_GAP).children((0..5).map(|_| {
            el().w(theme::CARD_W)
                .h(theme::CARD_H)
                .radius(10.0)
                .fill(t.card)
        })));
    }
    grid
}
