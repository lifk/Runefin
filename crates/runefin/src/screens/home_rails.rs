//! The Home rails chooser: choose up to four libraries for their own Recently added rail.
//! A pin replaces the combined rail, so nothing repeats and only pinned libraries are queried.

use crate::theme::FocusVisual;
use crate::theme::{self, type_};
use rune::{Color, El, col, el, row, text};

use crate::state::State;

pub(super) fn home_rails_sheet(s: &State) -> El {
    let t = theme::colors();
    let libraries: Vec<&jellyfin_api::LibraryView> = s.sidebar_libraries().collect();
    let pins = s.home_pins();
    let all = pins.is_empty();
    let mut list = col().key("home-rails-list").w_full().gap(2.0);
    for library in &libraries {
        let position = pins.iter().position(|id| id == &library.id);
        let id = library.id.clone();
        list = list.child(
            row()
                .key(format!("home-rails-{}", library.id))
                .role("button")
                .label(format!(
                    "{} library{}",
                    library.name,
                    position.map_or(String::new(), |p| format!(", rail {}", p + 1))
                ))
                .help(if position.is_some() {
                    "Remove this library from Home"
                } else {
                    "Add this library to Home"
                })
                .w_full()
                .h(38.0)
                .px(9.0)
                .radius(7.0)
                .gap(10.0)
                .items_center()
                .fill(if position.is_some() {
                    Color::WHITE.alpha(0.04)
                } else {
                    Color::TRANSPARENT
                })
                .hover(t.hover)
                .motion(theme::motion::CONTROL)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.99)
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut State| s.toggle_home_library(id.clone()))
                .child(
                    el().w(16.0)
                        .h(16.0)
                        .radius(4.0)
                        .center()
                        .fill(if position.is_some() {
                            t.text
                        } else {
                            Color::TRANSPARENT
                        })
                        .outline(1.0, 0.0, Color::WHITE.alpha(0.24))
                        .children(position.map(|_| text("✓").font(type_::caption()).color(t.bg))),
                )
                .child(
                    text(library.name.clone())
                        .font(type_::body())
                        .color(t.text)
                        .grow(1.0),
                )
                .children(position.map(|p| {
                    el().w(20.0)
                        .h(20.0)
                        .radius(10.0)
                        .center()
                        .fill(Color::WHITE.alpha(0.14))
                        .child(
                            text(format!("{}", p + 1))
                                .font(type_::caption())
                                .color(t.text),
                        )
                })),
        );
    }
    let panel = col()
        .key("home-rails-sheet")
        .w(760.0)
        .max_h(600.0)
        .padding(18.0)
        .gap(10.0)
        .radius(14.0)
        .fill(t.card)
        .outline(1.0, 0.0, Color::WHITE.alpha(0.12))
        .shadow(8.0, 28.0, Color::BLACK.alpha(0.48))
        .child(
            row().w_full().items_center().gap(10.0)
                .child(
                    text("Home rails")
                        .font(type_::section())
                        .color(t.text)
                        .grow(1.0),
                )
                .child(
                    el().key("home-rails-close")
                        .role("button")
                        .label("Close")
                        .w(30.0)
                        .h(30.0)
                        .radius(7.0)
                        .center()
                        .fill(t.bg)
                        .hover(t.hover)
                        .focusable(true)
                        .focus_visual(2.0)
                        .press_scale(0.94)
                        .cursor(rune::Cursor::Pointer)
                        .child(text("✕").font(type_::label()).color(t.text_secondary))
                        .on_press(|s: &mut State| s.close_home_rails()),
                ),
        )
        .child(
            text("Up to four libraries get their own “Recently added in …” rail. The all-library rail is replaced, so nothing repeats.")
                .font(type_::meta())
                .color(t.text_secondary),
        )
        .child(
            row().key("home-rails-mode").gap(2.0)
                .child(
                    el().key("home-rails-all")
                        .role("button")
                        .label("All libraries")
                        .h(30.0)
                        .px(12.0)
                        .radius(7.0)
                        .center()
                        .fill(if all {
                            Color::WHITE.alpha(0.14)
                        } else {
                            Color::TRANSPARENT
                        })
                        .hover(t.hover)
                        .motion(theme::motion::CONTROL)
                        .focusable(true)
                        .focus_visual(2.0)
                        .child(text("All libraries").font(type_::label()).color(if all { t.text } else { t.text_secondary }))
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| s.reset_home_libraries()),
                )
                .child(
                    el().key("home-rails-chosen")
                        .role("button")
                        .label("Chosen libraries")
                        .h(30.0)
                        .px(12.0)
                        .radius(7.0)
                        .center()
                        .fill(if !all {
                            Color::WHITE.alpha(0.14)
                        } else {
                            Color::TRANSPARENT
                        })
                        .hover(t.hover)
                        .motion(theme::motion::CONTROL)
                        .focusable(true)
                        .focus_visual(2.0)
                        .child(text("Chosen libraries").font(type_::label()).color(if !all { t.text } else { t.text_secondary }))
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| {
                            if s.home_pins().is_empty() {
                                s.show_toast("Choose a library below.");
                            }
                        }),
                ),
        )
        .child(
            el().w_full()
                .min_h(0.0)
                .h((libraries.len() as f32 * 40.0).min(360.0))
                .scroll(rune::ScrollSpec {
                    stick_bottom: false,
                })
                .scroll_edge_fade(16.0)
                .child(list),
        )
        .child(
            row().w_full().items_center().gap(10.0).mt(4.0)
                .child(
                    text(format!("{} of 4 selected", pins.len()))
                        .font(type_::meta())
                        .color(t.text_secondary)
                        .grow(1.0),
                )
                .child(
                    el().key("home-rails-reset")
                        .role("button")
                        .label("Reset")
                        .h(32.0)
                        .px(12.0)
                        .radius(7.0)
                        .center()
                        .fill(t.bg)
                        .hover(t.hover)
                        .focusable(true)
                        .focus_visual(2.0)
                        .press_scale(0.96)
                        .cursor(rune::Cursor::Pointer)
                        .child(text("Reset").font(type_::label()).color(t.text_secondary))
                        .on_press(|s: &mut State| s.reset_home_libraries()),
                )
                .child(
                    el().key("home-rails-done")
                        .role("button")
                        .label("Done")
                        .h(32.0)
                        .px(16.0)
                        .radius(7.0)
                        .center()
                        .fill(t.text)
                        .hover(t.hover)
                        .focusable(true)
                        .focus_on_light()
                        .press_scale(0.96)
                        .cursor(rune::Cursor::Pointer)
                        .child(text("Done").font(type_::label()).color(t.bg))
                        .on_press(|s: &mut State| s.close_home_rails()),
                ),
        );
    el().key("home-rails-layer")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .fill(Color::BLACK.alpha(0.55))
        .center()
        .on_press(|s: &mut State| s.close_home_rails())
        .child(el().on_press(|_: &mut State| {}).child(panel))
}
