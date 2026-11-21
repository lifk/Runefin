//! The Downloads page: what is on this device, what is on its way, and the controls for both.
//!
//! Every row is drawn from the manifest, so the page reads with no server at all; only the
//! posters need one, and a row without art is still a row you can play.

use rune::tok;

use crate::downloads::{self, Entry, State as EntryState};
use crate::theme::FocusVisual;
use crate::theme::{self, type_};
use rune::{El, ScrollSpec, SurfaceFit, col, el, row, text};

use crate::state::State;

use super::sidebar;

pub(super) fn downloads_screen(s: &State) -> El {
    let t = theme::colors();
    row()
        .key("downloads-screen")
        .w_full()
        .h_full()
        .fill(t.bg)
        .child(sidebar(s, &t))
        .child(el().w(1.0).h_full().fill(t.separator))
        .child(downloads_content(s, &t))
}

fn downloads_content(s: &State, t: &tok::Theme) -> El {
    let entries = &s.downloads.entries;
    let summary = if entries.is_empty() {
        "Nothing on this device yet".to_string()
    } else {
        let done = entries.iter().filter(|entry| entry.state.is_done()).count();
        format!(
            "{} on this device · {} of {}",
            super::count_label(done, "item"),
            downloads::size(s.downloads_used()),
            format_gb(s.downloads_budget()),
        )
    };
    let header = col()
        .key("downloads-header")
        .w_full()
        .pt(28.0)
        .pb(6.0)
        .px(theme::gutter())
        .gap(6.0)
        .items_start()
        .child(text("Downloads").font(type_::page()).color(t.text))
        .child(
            text(summary)
                .key("downloads-summary")
                .font(type_::meta())
                .color(t.text_secondary),
        );

    let mut body = col()
        .key("downloads-list")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .px(theme::gutter())
        .pb(if s.in_app_minimized { 124.0 } else { 28.0 })
        .gap(10.0);

    if let Some(error) = &s.downloads.error {
        let message = format!(
            "Downloads are unavailable: {error}. Pick another folder in Settings, or free some space."
        );
        body = body.child(
            col()
                .key("downloads-error")
                .w_full()
                .padding(16.0)
                .gap(8.0)
                .radius(12.0)
                .fill(t.card)
                .outline(1.0, 0.0, theme::status::danger().alpha(0.45))
                .child(
                    text("The download folder cannot be used")
                        .font(type_::section())
                        .color(t.text),
                )
                .child(text(message).font(type_::meta()).color(t.text_secondary)),
        );
    }

    if entries.is_empty() && s.downloads.error.is_none() {
        body = body.child(
            col()
                .key("downloads-empty")
                .w_full()
                .padding(20.0)
                .gap(8.0)
                .radius(12.0)
                .fill(t.card.alpha(0.55))
                .child(text("No downloads yet").font(type_::section()).color(t.text))
                .child(
                    text("Use Download on a film or an episode and it stays on this device, ready to play offline.")
                        .font(type_::meta())
                        .color(t.text_secondary),
                )
                .child(
                    text(format!("Folder: {}", s.downloads.root))
                        .key("downloads-folder")
                        .font(type_::mono_caption())
                        .color(t.text_secondary.alpha(0.8)),
                ),
        );
    }

    body = body.children(entries.iter().map(|entry| row_for(s, entry, t)));

    if !entries.is_empty() {
        body = body.child(
            text(format!(
                "Folder: {}{}",
                s.downloads.root,
                s.downloads
                    .free
                    .map(|free| format!(" · {} free", downloads::size(free)))
                    .unwrap_or_default()
            ))
            .key("downloads-folder")
            .font(type_::mono_caption())
            .color(t.text_secondary.alpha(0.8))
            .mt(6.0),
        );
    }

    col()
        .key("downloads-content")
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .child(header)
        .child(body)
}

/// `20 GB`, `7.5 GB` — a budget line that reads like the size labels beside it.
pub(super) fn format_gb(gb: f64) -> String {
    if (gb - gb.round()).abs() < 0.05 {
        format!("{gb:.0} GB")
    } else {
        format!("{gb:.1} GB")
    }
}

fn row_for(s: &State, entry: &Entry, t: &tok::Theme) -> El {
    let id = entry.item.id.clone();
    let state = entry.state;
    // The row's title is the item's own name; an episode's series is in the line under it.
    let title = entry.item.name.clone();
    let line = entry.line();
    let size = if state.is_done() {
        downloads::size(entry.size)
    } else if let Some(progress) = entry.progress() {
        format!(
            "{} of {} · {}%",
            downloads::size(entry.done),
            downloads::size(entry.size),
            (progress * 100.0).round() as i64
        )
    } else {
        downloads::size(entry.done)
    };
    let detail = format!("{line} · {size}");

    let mut row = row()
        .key(format!("download-{id}"))
        .w_full()
        .padding(12.0)
        .gap(14.0)
        .items_center()
        .radius(12.0)
        .fill(t.card.alpha(0.55))
        .outline(1.0, 0.0, t.card_border)
        .child(thumb(s, entry, t))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(5.0)
                .items_start()
                .child(
                    text(title)
                        .key(format!("download-title-{id}"))
                        .font(type_::section())
                        .color(t.text)
                        .truncate(),
                )
                .child(
                    text(detail)
                        .key(format!("download-detail-{id}"))
                        .font(type_::meta())
                        .color(t.text_secondary)
                        .truncate(),
                )
                .children(
                    matches!(state, EntryState::Downloading | EntryState::Paused).then(|| {
                        el().w(260.0)
                            .h(3.0)
                            .mt(2.0)
                            .radius(1.5)
                            .fill(t.control_track)
                            .child(
                                el().w(260.0).h(3.0).radius(1.5).child(
                                    el().grow(entry.progress().unwrap_or(0.0).max(0.001))
                                        .h(3.0)
                                        .radius(1.5)
                                        .fill(tok::accent(0, true)),
                                ),
                            )
                    }),
                )
                .children(entry.error.as_ref().map(|message| {
                    text(message.clone())
                        .key(format!("download-error-{id}"))
                        .font(type_::caption())
                        .color(theme::status::danger())
                        .truncate()
                })),
        )
        .child(
            text(state.label())
                .key(format!("download-state-{id}"))
                .font(type_::caption())
                .color(state_colour(state, t))
                .w(112.0),
        );

    if state.is_watchable() {
        let play = format!("download-play-{id}");
        row = row.child(action("Play", play, true, t, {
            let id = id.clone();
            move |s: &mut State| s.play_download(&id)
        }));
    }
    match state {
        EntryState::Downloading => {
            let key = format!("download-pause-{id}");
            row = row.child(action("Pause", key, false, t, {
                let id = id.clone();
                move |s: &mut State| s.pause_download(&id)
            }));
        }
        EntryState::Paused | EntryState::Failed | EntryState::Queued => {
            let key = if state == EntryState::Paused {
                format!("download-resume-{id}")
            } else {
                format!("download-retry-{id}")
            };
            row = row.child(action(
                if state == EntryState::Paused {
                    "Resume"
                } else {
                    "Retry"
                },
                key,
                true,
                t,
                {
                    let id = id.clone();
                    move |s: &mut State| s.resume_download(&id)
                },
            ));
        }
        EntryState::Done => {}
    }
    let keep_key = format!("download-keep-{id}");
    row = row.child(action(
        if entry.kept { "Kept" } else { "Keep" },
        keep_key,
        false,
        t,
        {
            let id = id.clone();
            move |s: &mut State| s.keep_download(&id)
        },
    ));
    let remove = format!("download-remove-{id}");
    row.child(action("Remove", remove, false, t, move |s: &mut State| {
        s.remove_download(&id)
    }))
}

fn state_colour(state: EntryState, t: &tok::Theme) -> rune::Color {
    match state {
        EntryState::Done => theme::status::success(),
        EntryState::Failed => theme::status::danger(),
        EntryState::Downloading => tok::accent(0, true),
        _ => t.text_secondary,
    }
}

fn thumb(s: &State, entry: &Entry, t: &tok::Theme) -> El {
    let (top, bottom) = theme::palette(&entry.item.id);
    let mut art = el()
        .key(format!("download-poster-{}", entry.item.id))
        .w(56.0)
        .h(84.0)
        .radius(6.0)
        .clip_rounded(6.0)
        .fill(top)
        .linear_gradient(std::f32::consts::PI, [(0.0, top), (1.0, bottom)])
        .center()
        .child(
            text(super::initial(&entry.item.name))
                .font(rune::TextStyle::new(18.0, 650.0))
                .color(t.text_secondary),
        );
    if let Some((id, tag)) = crate::home::poster(&entry.item) {
        let key = crate::image::key(
            &s.config.server_url,
            id,
            jellyfin_api::ImageKind::Primary.as_str(),
            Some(tag),
            120,
        );
        if let Some(layers) = super::poster_layers(s, &key, SurfaceFit::Fit) {
            art = art.child(layers);
        }
    }
    art
}

fn action(
    label: &str,
    key: String,
    primary: bool,
    t: &tok::Theme,
    on_press: impl Fn(&mut State) + 'static,
) -> El {
    el().key(key)
        .role("button")
        .label(label.to_string())
        .h(32.0)
        .px(14.0)
        .radius(8.0)
        .center()
        .fill(if primary {
            t.card
        } else {
            rune::Color::TRANSPARENT
        })
        .outline(1.0, 0.0, t.card_border)
        .hover(t.hover)
        .motion(crate::theme::motion::CONTROL)
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.97)
        .child(
            text(label.to_string())
                .font(type_::label())
                .color(if primary { t.text } else { t.text_secondary }),
        )
        .cursor(rune::Cursor::Pointer)
        .on_press(on_press)
}
