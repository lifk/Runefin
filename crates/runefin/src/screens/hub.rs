//! The Libraries hub: every video library, playlist and tool, one level below the rail.
//!
//! The rail only keeps five destinations, so this is the only place every library is listed.
//! A Recent row keeps the three last opened on the first screen; the All grid is six columns.

use crate::theme::FocusVisual;
use crate::theme::{self, type_};
use jellyfin_api::LibraryView;
use rune::{Color, El, ScrollSpec, col, el, icon, row, text, text_input};

use crate::state::{Route, State};

pub(super) fn libraries_screen(s: &State) -> El {
    let t = theme::colors();
    let query = s.hub_filter.text.trim().to_ascii_lowercase();
    let libraries: Vec<&LibraryView> = s
        .sidebar_libraries()
        .filter(|library| query.is_empty() || library.name.to_ascii_lowercase().contains(&query))
        .collect();
    let total_libraries = s.sidebar_libraries().count();
    let recent: Vec<&LibraryView> = {
        let picked: Vec<&LibraryView> = s
            .recent_libraries
            .iter()
            .filter_map(|id| libraries.iter().copied().find(|library| &library.id == id))
            .take(3)
            .collect();
        if picked.is_empty() {
            libraries.iter().copied().take(3).collect()
        } else {
            picked
        }
    };
    let mut body = col()
        .key("hub-content")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .scroll_edge_fade(28.0)
        .padding(32.0)
        .gap(18.0)
        .child(
            row()
                .key("hub-head")
                .w_full()
                .items_end()
                .gap(14.0)
                .child(
                    text("Libraries")
                        .key("hub-title")
                        .font(type_::page())
                        .color(t.text),
                )
                .child(
                    text(format!(
                        "{}{} libraries{}",
                        libraries.len(),
                        if query.is_empty() {
                            String::new()
                        } else {
                            format!(" of {total_libraries}")
                        },
                        if s.playlists.is_empty() {
                            String::new()
                        } else {
                            format!(" · {} playlists", s.playlists.len())
                        }
                    ))
                    .key("hub-sub")
                    .font(type_::meta())
                    .color(t.text_secondary),
                )
                .child(
                    el().key("hub-filter-box")
                        .h(34.0)
                        .w(260.0)
                        .radius(8.0)
                        .px(10.0)
                        .center_y()
                        .fill(t.bg)
                        .outline(
                            if s.hub_filter.focused { 1.5 } else { 1.0 },
                            0.0,
                            if s.hub_filter.focused {
                                crate::theme::accent()
                            } else {
                                t.card_border
                            },
                        )
                        .child(
                            text_input(&s.hub_filter, type_::meta())
                                .key("hub-filter")
                                .label("Filter libraries")
                                .placeholder("Filter libraries", t.text_secondary.alpha(0.55))
                                .role("textbox")
                                .single_line()
                                .w_full()
                                .on_edit(|s: &mut State| &mut s.hub_filter)
                                .on_input_key(
                                    |s: &mut State, key: &rune::KeyEvent, _tx: &mut rune::Tx| {
                                        if key.key == rune::Key::Enter {
                                            return s.keyboard_enter_open("hub-filter");
                                        }
                                        false
                                    },
                                )
                                .color(t.text),
                        ),
                ),
        );
    if !recent.is_empty() {
        body = body.child(section("Recent", t));
        let mut row_ = row().key("hub-recent").w_full().gap(12.0);
        for library in recent {
            row_ = row_.child(tile(library, "hub-recent", 360.0, 202.0, true, s, &t));
        }
        body = body.child(row_);
    }
    body = body.child(section("All libraries", t));
    if libraries.is_empty() {
        body = body.child(
            col()
                .key("hub-empty")
                .w_full()
                .gap(6.0)
                .child(
                    text(if query.is_empty() {
                        "No video libraries yet."
                    } else {
                        "No libraries match."
                    })
                    .font(type_::body())
                    .color(t.text),
                )
                .child(
                    text("Add one on the server, then refresh.")
                        .font(type_::meta())
                        .color(t.text_secondary),
                ),
        );
    } else {
        let mut grid = row().key("hub-libraries").w_full().flex_wrap().gap(12.0);
        for library in &libraries {
            grid = grid.child(tile(library, "hub-library", 173.0, 98.0, false, s, &t));
        }
        body = body.child(grid);
    }
    body = body.child(section("Tools", t));
    let mut tools = row().key("hub-tools").w_full().flex_wrap().gap(10.0);
    tools = tools
        .child(action(
            "Downloads",
            "hub-tool-downloads",
            &theme::icon::DOWNLOAD,
            &t,
            |s| s.open_downloads(),
        ))
        .child(action(
            "Remote",
            "hub-tool-remote",
            &theme::icon::REMOTE,
            &t,
            |s| s.open_remote(),
        ))
        .child(action(
            "Home rails",
            "hub-tool-home-rails",
            &theme::icon::LIBRARY,
            &t,
            |s| s.open_home_rails(),
        ));
    if s.big_screen {
        tools = tools.child(action(
            "Controls",
            "hub-tool-controls",
            &theme::icon::GAMEPAD,
            &t,
            |s| s.toggle_shortcuts(),
        ));
    }
    body = body.child(tools);
    if !s.playlists.is_empty() {
        body = body.child(section("Playlists", t));
        let mut list = row().key("hub-playlists").w_full().flex_wrap().gap(10.0);
        for playlist in &s.playlists {
            let id = playlist.id.clone();
            let name = playlist.name.clone();
            list = list.child(action(
                &playlist.name,
                &format!("hub-playlist-{}", playlist.id),
                &theme::icon::LIST,
                &t,
                move |s| s.open_playlist(id.clone(), name.clone()),
            ));
        }
        body = body.child(list);
    }
    let mut extras = row().key("hub-extras").w_full().flex_wrap().gap(10.0);
    extras = extras.child(action(
        "Watchlist",
        "hub-watchlist",
        &theme::icon::HEART,
        &t,
        |s| s.open_watchlist(),
    ));
    if !s.config.seerr_url.is_empty() {
        extras = extras.child(action(
            "Discover",
            "hub-seerr",
            &theme::icon::COMPASS,
            &t,
            |s| s.open_seerr(crate::state::SeerrTab::Discover),
        ));
    }
    body = body.child(extras);

    row()
        .key("hub-screen")
        .w_full()
        .h_full()
        .fill(t.bg)
        .child(super::sidebar(s, &t))
        .child(el().w(1.0).h_full().fill(t.separator))
        .child(body)
}

fn section(title: &str, t: rune::tok::Theme) -> El {
    text(title.to_string())
        .font(type_::row())
        .color(t.text)
        .mt(6.0)
}

fn tile(
    library: &LibraryView,
    prefix: &str,
    width: f32,
    height: f32,
    large: bool,
    s: &State,
    t: &rune::tok::Theme,
) -> El {
    let id = library.id.clone();
    let name = library.name.clone();
    let kind = library.collection_type.clone();
    let is_live = kind.as_deref() == Some("livetv");
    let glyph = match kind.as_deref() {
        Some("movies") => &theme::icon::FILM,
        Some("tvshows") => &theme::icon::TV,
        Some("boxsets") => &theme::icon::LAYERS,
        Some("livetv") => &theme::icon::ANTENNA,
        Some("playlists") => &theme::icon::LIST,
        _ if library.name.to_ascii_lowercase().contains("record") => &theme::icon::RECORD,
        _ => &theme::icon::LIBRARY,
    };
    let selected = matches!(&s.route, Route::Library { id } if id == &library.id)
        || (is_live && matches!(s.route, Route::LiveTv));
    let tile_key = format!("{prefix}-{}", library.id);
    let focused = s.focused_key.as_deref() == Some(tile_key.as_str());
    let image_key = s.library_image_key(library);
    el().key(tile_key)
        .role("button")
        .label(format!("{} library", library.name))
        .help(format!("Open {}", library.name))
        .w(width)
        .h(height)
        .radius(10.0)
        .clip_rounded(10.0)
        .fill(t.card)
        .gradient(t.card, t.bg)
        .children(
            super::poster_layers(s, &image_key, rune::SurfaceFit::Fill).map(|image| {
                el().absolute(0.0, 0.0)
                    .w_full()
                    .h_full()
                    .opacity(0.72)
                    .child(image)
            }),
        )
        .outline(
            if selected { 1.0 } else { 0.0 },
            0.0,
            Color::WHITE.alpha(0.14),
        )
        .hover(t.hover)
        .scale(if focused { 1.03 } else { 1.0 })
        .motion(theme::motion::HOVER)
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.98)
        .cursor(rune::Cursor::Pointer)
        .child(
            el().absolute(18.0, 14.0)
                .w(if large { 34.0 } else { 24.0 })
                .h(if large { 34.0 } else { 24.0 })
                .child(
                    icon(glyph, if large { 30.0 } else { 21.0 }).color(Color::WHITE.alpha(0.72)),
                ),
        )
        .child(
            col()
                .absolute(16.0, height - if large { 58.0 } else { 42.0 })
                .w(width - 30.0)
                .gap(2.0)
                .child(
                    text(library.name.clone())
                        .font(if large {
                            type_::section()
                        } else {
                            type_::card_title()
                        })
                        .color(t.text)
                        .max_lines(1),
                )
                .children(large.then(|| {
                    text(kind.clone().unwrap_or_else(|| "Library".into()))
                        .font(type_::caption())
                        .color(t.text_secondary)
                })),
        )
        .on_press(move |s: &mut State| {
            if is_live {
                s.open_live_tv();
            } else {
                s.open_library(id.clone(), name.clone(), kind.clone());
            }
        })
}

fn action(
    label: &str,
    key: &str,
    glyph: &'static rune::Icon,
    t: &rune::tok::Theme,
    on: impl Fn(&mut State) + 'static,
) -> El {
    el().key(key)
        .role("button")
        .label(label.to_string())
        .help(label.to_string())
        .h(44.0)
        .px(14.0)
        .radius(9.0)
        .gap(9.0)
        .center_y()
        .fill(t.card)
        .outline(1.0, 0.0, t.card_border)
        .hover(t.hover)
        .motion(theme::motion::CONTROL)
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.97)
        .cursor(rune::Cursor::Pointer)
        .child(icon(glyph, 18.0).color(t.text_secondary))
        .child(text(label.to_string()).font(type_::label()).color(t.text))
        .on_press(move |s: &mut State| on(s))
}
