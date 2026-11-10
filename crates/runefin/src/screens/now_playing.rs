//! What the app shows while mpv owns the screen: the poster, title, elapsed/remaining and the
//! two controls that matter. mpv itself draws the video in its own window on Wayland.

use rune::tok::{self, motion};

use crate::theme::{self, type_};
use rune::{El, SurfaceFit, col, el, row, text};

use crate::detail;
use crate::state::State;

use super::{hero_style, placeholder, subtitle_style};

pub(super) fn now_playing(s: &State) -> El {
    let t = theme::colors();
    let mut page = el().key("now-playing").w_full().h_full().fill(t.bg).child(
        el().absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .gradient(t.card, t.bg),
    );
    let content = col()
        .key("now-playing-content")
        .w_full()
        .max_w(980.0)
        .padding(32.0)
        .gap(24.0)
        .fill(t.card)
        .radius(16.0)
        .outline(1.0, 0.0, t.card_border)
        .items_start();

    let Some(session) = &s.playback else {
        let mut waiting = content
            .child(
                text("Launching mpv…")
                    .key("now-launching")
                    .font(hero_style())
                    .color(t.text),
            )
            .child(
                text("The player opens in its own window on Wayland.")
                    .font(subtitle_style())
                    .color(t.text_secondary),
            );
        if s.launching {
            waiting = waiting.child(control(
                "Cancel",
                "now-cancel",
                false,
                &t,
                |s: &mut State| s.stop_playback(),
            ));
        }
        if let Some(error) = &s.playback_error {
            waiting = waiting.child(
                text(error.clone())
                    .key("now-error")
                    .font(subtitle_style())
                    .color(tok::accent(2, true)),
            );
        }
        page = page.child(waiting);
        return page;
    };

    let poster = s
        .detail_image_key(&session.item)
        .and_then(|key| super::poster_layers(s, &key, SurfaceFit::Fill))
        .unwrap_or_else(|| placeholder(&session.item, &t));
    let poster = el()
        .key("now-poster")
        .w(220.0)
        .h(330.0)
        .radius(12.0)
        .clip_rounded(12.0)
        .fill(t.card)
        .child(poster);

    let title = session
        .item
        .series_name
        .clone()
        .unwrap_or_else(|| session.item.name.clone());
    let subtitle = session
        .item
        .episode_label()
        .map(|label| format!("{label} · {}", session.item.name))
        .unwrap_or_else(|| {
            session
                .item
                .production_year
                .map(|year| year.to_string())
                .unwrap_or_default()
        });
    let elapsed = detail::format_time(session.position_ms);
    let remaining = session.duration_ms.map_or_else(
        || "—".into(),
        |duration| {
            format!(
                "-{}",
                detail::format_time((duration - session.position_ms).max(0))
            )
        },
    );
    let progress = session.duration_ms.map_or(0.0, |duration| {
        if duration > 0 {
            (session.position_ms as f32 / duration as f32).clamp(0.0, 1.0)
        } else {
            0.0
        }
    });

    let mut info = col()
        .grow(1.0)
        .items_start()
        .gap(12.0)
        .child(
            text(title)
                .key("now-title")
                .font(type_::page())
                .color(t.text),
        )
        .child(
            text(subtitle)
                .key("now-subtitle")
                .font(subtitle_style())
                .color(t.text_secondary),
        )
        .child(
            row()
                .w(520.0)
                .gap(12.0)
                .child(
                    text(elapsed)
                        .key("now-elapsed")
                        .font(type_::mono_caption())
                        .color(t.text),
                )
                .child(
                    el().grow(1.0)
                        .h(4.0)
                        .radius(2.0)
                        .fill(t.control_track)
                        .child(
                            row()
                                .w_full()
                                .h_full()
                                .child(
                                    el().grow(progress.max(0.001))
                                        .h(4.0)
                                        .radius(2.0)
                                        .fill(tok::accent(0, true)),
                                )
                                .child(el().grow((1.0 - progress).max(0.001)).h(4.0)),
                        ),
                )
                .child(
                    text(remaining)
                        .key("now-remaining")
                        .font(type_::mono_caption())
                        .color(t.text_secondary),
                ),
        )
        .child(
            row()
                .gap(10.0)
                .child(control(
                    if session.paused { "Resume" } else { "Pause" },
                    "now-pause",
                    session.paused,
                    &t,
                    |s: &mut State| s.toggle_pause(),
                ))
                .child(control("Stop", "now-stop", false, &t, |s: &mut State| {
                    s.stop_playback()
                })),
        )
        .child(
            text("Playing in mpv's own window on Wayland.")
                .key("now-hint")
                .font(type_::mono_caption())
                .color(t.text_secondary),
        );
    if let Some(error) = &s.playback_error {
        info = info.child(
            text(error.clone())
                .key("now-error")
                .font(subtitle_style())
                .color(tok::accent(2, true)),
        );
    }

    page.child(
        row().w_full().h_full().center().padding(40.0).child(
            content.child(
                row()
                    .w_full()
                    .gap(36.0)
                    .items_start()
                    .child(poster)
                    .child(info),
            ),
        ),
    )
}

fn control(
    label: &str,
    key: &str,
    selected: bool,
    t: &tok::Theme,
    on: impl Fn(&mut State) + 'static,
) -> El {
    el().key(key)
        .role("button")
        .h(42.0)
        .px(18.0)
        .radius(10.0)
        .center()
        .fill(if selected { tok::accent(0, true) } else { t.bg })
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::ring())
        .press_scale(0.97)
        .child(
            text(label)
                .font(type_::label())
                .color(if selected { t.bg } else { t.text }),
        )
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut State| on(s))
}
