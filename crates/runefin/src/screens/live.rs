//! Live TV: the tuner's channels and what is on now. Clicking a channel opens it on the server
//! and hands the stream to the same in-app player, which switches to a LIVE OSD.

use jellyfin_api::Item;
use rune::tok::{self, motion};
use rune::{Color, El, SurfaceFit, col, el, icon, row, text};

use crate::state::State;
use crate::theme::FocusVisual;
use crate::theme::SurfaceStyle;
use crate::theme::{self, type_};

pub(super) fn live_tv_screen(s: &State) -> El {
    let t = theme::colors();
    let count = s.live.channels.len();
    let header = row()
        .w_full()
        .items_end()
        .gap(12.0)
        .child(
            col()
                .grow(1.0)
                .gap(4.0)
                .child(
                    text("Live TV")
                        .key("live-title")
                        .font(type_::page())
                        .color(t.text),
                )
                .child(
                    text(if count == 0 {
                        "Channels from your tuner".to_string()
                    } else {
                        format!("{} · what is on now", super::count_label(count, "channel"))
                    })
                    .key("live-subtitle")
                    .font(type_::body())
                    .color(t.text_secondary),
                ),
        )
        .child(button(
            if s.live.guide_hours >= 24 {
                "3 hours"
            } else {
                "Day"
            },
            "live-guide-window",
            s.live.guide_hours >= 24,
            &t,
            |s: &mut State| s.toggle_guide_window(),
        ))
        .child(button(
            if s.live.guide_open { "List" } else { "Guide" },
            "live-guide-toggle",
            s.live.guide_open,
            &t,
            |s: &mut State| s.toggle_live_guide(),
        ))
        .child(button(
            if s.live.favorites_only {
                "All channels"
            } else {
                "Favourites"
            },
            "live-favorites",
            s.live.favorites_only,
            &t,
            |s: &mut State| s.toggle_live_favorites_only(),
        ))
        .child(button(
            "Refresh",
            "live-refresh",
            false,
            &t,
            |s: &mut State| s.load_live_channels(),
        ));

    let body = if s.live.loading && s.live.channels.is_empty() {
        loading(&t)
    } else if let Some(error) = &s.live.error {
        let retry = button("Try again", "live-retry", false, &t, |s: &mut State| {
            s.load_live_channels()
        });
        col()
            .key("live-error")
            .w_full()
            .center()
            .gap(10.0)
            .child(
                text("Can't load channels")
                    .font(type_::section())
                    .color(t.text),
            )
            .child(
                text(error.clone())
                    .key("live-error-message")
                    .font(type_::meta())
                    .color(t.text_secondary),
            )
            .child(retry)
    } else if s.live.channels.is_empty() {
        col()
            .key("live-empty")
            .w_full()
            .center()
            .gap(10.0)
            .child(text("No channels").font(type_::section()).color(t.text))
            .child(
                text("The server has not reported any channels from its tuner.")
                    .font(type_::meta())
                    .color(t.text_secondary),
            )
    } else if s.live.guide_open {
        guide_grid(s, &t)
    } else {
        let mut list = col().key("live-channels").w_full().gap(10.0);
        for channel in s
            .live
            .channels
            .iter()
            .filter(|channel| !s.live.favorites_only || s.live.favorites.contains(&channel.id))
        {
            list = list.child(channel_row(s, channel, &t));
        }
        list
    };

    row()
        .key("live-tv-screen")
        .w_full()
        .h_full()
        .fill(t.bg)
        .child(super::sidebar(s, &t))
        .child(el().w(1.0).h_full().fill(t.separator))
        .child(
            col()
                .key("live-content")
                .grow(1.0)
                .min_w(0.0)
                .min_h(0.0)
                .h_full()
                .padding(theme::gutter())
                .gap(20.0)
                .child(header)
                .child(body),
        )
}

fn channel_row(s: &State, channel: &jellyfin_api::Item, t: &tok::Theme) -> El {
    let image = s
        .image_key(channel)
        .and_then(|key| super::poster_layers(s, &key, SurfaceFit::Fit))
        .unwrap_or_else(|| {
            el().w_full().h_full().center().child(
                text(
                    channel
                        .name
                        .chars()
                        .next()
                        .map_or_else(|| "?".to_string(), |c| c.to_uppercase().to_string()),
                )
                .font(type_::section())
                .color(t.text_secondary),
            )
        });
    let number = channel
        .channel_number
        .clone()
        .unwrap_or_else(|| "—".to_string());
    let now_at = now_seconds();
    let current = channel.current_program.as_ref();
    let now = current
        .map(|program| {
            let left = program
                .end_date
                .as_deref()
                .and_then(jellyfin_api::iso_seconds)
                .map(|end| ((end - now_at).max(0) + 59) / 60);
            match left {
                Some(minutes) => format!("Now · {} · {minutes}m left", program.name),
                None => format!("Now · {}", program.name),
            }
        })
        .unwrap_or_else(|| "No guide data".to_string());
    let progress = current.and_then(|program| {
        let start = program
            .start_date
            .as_deref()
            .and_then(jellyfin_api::iso_seconds)?;
        let end = program
            .end_date
            .as_deref()
            .and_then(jellyfin_api::iso_seconds)?;
        (end > start).then_some(((now_at - start) as f32 / (end - start) as f32).clamp(0.0, 1.0))
    });
    let next_program = s.live.next.get(&channel.id).cloned();
    let next = next_program.as_ref().map(|program| {
        let in_minutes = program
            .start_date
            .as_deref()
            .and_then(jellyfin_api::iso_seconds)
            .map(|start| ((start - now_at).max(0) + 59) / 60);
        match in_minutes {
            Some(minutes) if minutes > 0 => {
                format!("Next · {} · in {minutes}m", program.name)
            }
            _ => format!("Next · {}", program.name),
        }
    });
    let reminder_set = next_program
        .as_ref()
        .is_some_and(|program| s.has_live_reminder(&program.id));
    let id = channel.id.clone();
    row()
        .key(format!("live-channel-{}", channel.id))
        .role("button")
        .w_full()
        .h(108.0)
        .padding(12.0)
        .gap(16.0)
        .items_center()
        .radius(12.0)
        .fill(t.card)
        .gradient(Color::WHITE.alpha(0.045), Color::TRANSPARENT)
        .inset_edge()
        .outline(1.0, 0.0, t.card_border)
        .shadow(1.0, 2.0, Color::BLACK.alpha(0.25))
        .shadow(6.0, 18.0, Color::BLACK.alpha(0.16))
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.99)
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut State| {
            if let Some(channel) = s
                .live
                .channels
                .iter()
                .find(|channel| channel.id == id)
                .cloned()
            {
                s.play_live_channel(channel);
            }
        })
        .child(
            el().w(76.0)
                .h(84.0)
                .radius(8.0)
                .clip_rounded(8.0)
                .fill(Color::WHITE.alpha(0.05))
                .child(image),
        )
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(4.0)
                .child(
                    row()
                        .gap(8.0)
                        .items_center()
                        .child(
                            el().h(22.0)
                                .px(8.0)
                                .radius(6.0)
                                .center()
                                .fill(tok::accent(2, true).alpha(0.16))
                                .child(
                                    text("● LIVE")
                                        .font(type_::caption())
                                        .color(tok::accent(2, true)),
                                ),
                        )
                        .child(
                            text(format!("#{number}"))
                                .font(type_::mono_caption())
                                .color(t.text_secondary),
                        ),
                )
                .child(
                    text(channel.name.clone())
                        .font(type_::title())
                        .color(t.text)
                        .truncate(),
                )
                .child(
                    text(now)
                        .key(format!("live-program-{}", channel.id))
                        .font(type_::meta())
                        .color(t.text_secondary)
                        .truncate(),
                )
                .children(progress.map(|progress| {
                    row()
                        .w_full()
                        .h(3.0)
                        .radius(1.5)
                        .fill(t.control_track)
                        .child(
                            el().grow(progress.max(0.001))
                                .h(3.0)
                                .radius(1.5)
                                .fill(tok::accent(0, true)),
                        )
                        .child(el().grow((1.0 - progress).max(0.001)).h(3.0))
                }))
                .children(next.map(|next| {
                    text(next)
                        .key(format!("live-next-{}", channel.id))
                        .font(type_::caption())
                        .color(t.text_secondary.alpha(0.72))
                        .truncate()
                })),
        )
        .children(next_program.as_ref().map(|_| {
            let channel_id = channel.id.clone();
            el().key(format!("live-remind-{}", channel.id))
                .role("button")
                .w(30.0)
                .h(30.0)
                .radius(15.0)
                .center()
                .fill(if reminder_set {
                    tok::accent(3, true).alpha(0.22)
                } else {
                    t.bg
                })
                .outline(
                    1.0,
                    0.0,
                    if reminder_set {
                        tok::accent(3, true).alpha(0.65)
                    } else {
                        t.card_border
                    },
                )
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.94)
                .label(if reminder_set {
                    "Remove reminder"
                } else {
                    "Remind me"
                })
                .help(if reminder_set {
                    "Remove the reminder"
                } else {
                    "Notify me when this program starts"
                })
                .cursor(rune::Cursor::Pointer)
                .child(icon(&theme::icon::BELL, 14.0).color(if reminder_set {
                    tok::accent(3, true)
                } else {
                    t.text_secondary
                }))
                .on_press(move |s: &mut State| s.toggle_next_live_reminder(&channel_id))
        }))
        .child(
            el().key(format!("live-favorite-{}", channel.id))
                .role("button")
                .w(30.0)
                .h(30.0)
                .radius(15.0)
                .center()
                .fill(if s.live.favorites.contains(&channel.id) {
                    tok::accent(0, true).alpha(0.22)
                } else {
                    t.bg
                })
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.94)
                .cursor(rune::Cursor::Pointer)
                .child(
                    text(if s.live.favorites.contains(&channel.id) {
                        "★"
                    } else {
                        "☆"
                    })
                    .font(type_::label())
                    .color(t.text),
                )
                .on_press({
                    let id = channel.id.clone();
                    move |s: &mut State| s.toggle_live_favorite(&id)
                }),
        )
        .child(
            el().h(34.0)
                .px(14.0)
                .radius(8.0)
                .center()
                .fill(tok::accent(0, true))
                .gradient(Color::WHITE.alpha(0.18), Color::TRANSPARENT)
                .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
                .shadow(4.0, 12.0, tok::accent(0, true).alpha(0.35))
                .child(text("Watch").font(type_::label()).color(t.bg)),
        )
}

fn loading(t: &tok::Theme) -> El {
    let mut list = col().key("live-loading").w_full().gap(10.0);
    for _ in 0..4 {
        list = list.child(
            row()
                .w_full()
                .h(92.0)
                .padding(14.0)
                .gap(16.0)
                .items_center()
                .radius(12.0)
                .fill(t.card)
                .child(el().w(72.0).h(64.0).radius(8.0).fill(t.control_track))
                .child(
                    col()
                        .grow(1.0)
                        .gap(8.0)
                        .child(super::skeleton_bar(220.0, t))
                        .child(super::skeleton_bar(150.0, t)),
                ),
        );
    }
    list
}

fn button(
    label: &'static str,
    key: &'static str,
    selected: bool,
    t: &tok::Theme,
    on: impl Fn(&mut State) + 'static,
) -> El {
    el().key(key)
        .role("button")
        .h(32.0)
        .px(12.0)
        .radius(8.0)
        .center()
        .fill(if selected {
            tok::accent(0, true).alpha(0.20)
        } else {
            t.card
        })
        .outline(
            1.0,
            0.0,
            if selected {
                tok::accent(0, true).alpha(0.65)
            } else {
                t.card_border
            },
        )
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.97)
        .child(text(label).font(type_::label()).color(t.text))
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut State| on(s))
}

/// 15-minute slices over three hours. Program blocks carry one grow unit per slice, so every
/// row and the time header line up without measuring a single pixel.
const SLICES: usize = 12;
const CHANNEL_LABEL_W: f32 = 168.0;

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn guide_grid(s: &State, t: &tok::Theme) -> El {
    let now = now_seconds();
    let window = s.live.guide_hours.max(1) * 3_600;
    let slice_seconds = window / SLICES as i64;
    let marker = |index: usize| {
        if index == 0 {
            "Now".to_string()
        } else {
            let minutes = index as i64 * slice_seconds / 60;
            format!("+{minutes}m")
        }
    };
    let header = row()
        .key("live-guide-times")
        .w_full()
        .items_center()
        .child(el().w(CHANNEL_LABEL_W))
        .child(
            row()
                .grow(1.0)
                .min_w(0.0)
                .children((0..SLICES).map(|index| {
                    el().grow(2.0).child(
                        text(marker(index))
                            .font(type_::mono_caption())
                            .color(t.text_secondary),
                    )
                })),
        );
    let mut grid = col().key("live-guide").w_full().gap(8.0).child(header);
    for channel in s
        .live
        .channels
        .iter()
        .filter(|channel| !s.live.favorites_only || s.live.favorites.contains(&channel.id))
    {
        grid = grid.child(guide_row(s, channel, now, slice_seconds, t));
    }
    col()
        .key("live-guide-scroll")
        .w_full()
        .gap(10.0)
        .child(grid)
}

struct Span<'a> {
    program: Option<&'a Item>,
    slices: u32,
}

fn spans<'a>(programs: &'a [Item], now: i64, slice_seconds: i64) -> Vec<Span<'a>> {
    let mut spans: Vec<Span<'a>> = Vec::new();
    for index in 0..SLICES {
        let middle = now + index as i64 * slice_seconds + slice_seconds / 2;
        let program = programs.iter().find(|program| {
            let (Some(start), Some(end)) = (
                program
                    .start_date
                    .as_deref()
                    .and_then(jellyfin_api::iso_seconds),
                program
                    .end_date
                    .as_deref()
                    .and_then(jellyfin_api::iso_seconds),
            ) else {
                return false;
            };
            start <= middle && middle < end
        });
        let same = spans
            .last()
            .and_then(|span| span.program)
            .map(|last| last.id.as_str())
            == program.map(|program| program.id.as_str());
        if same {
            if let Some(last) = spans.last_mut() {
                last.slices += 1;
            }
        } else {
            spans.push(Span { program, slices: 1 });
        }
    }
    spans
}

fn guide_row(s: &State, channel: &Item, now: i64, slice_seconds: i64, t: &tok::Theme) -> El {
    let programs = s
        .live
        .guide
        .get(&channel.id)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let number = channel
        .channel_number
        .clone()
        .unwrap_or_else(|| "—".to_string());
    let id = channel.id.clone();
    let mut timeline = row().grow(1.0).min_w(0.0).h(58.0).gap(2.0);
    let guide_spans = spans(programs, now, slice_seconds);
    if guide_spans.is_empty() {
        timeline = timeline.child(
            el().grow(SLICES as f32)
                .min_w(0.0)
                .h(58.0)
                .radius(6.0)
                .fill(Color::WHITE.alpha(0.025))
                .center()
                .child(
                    text("No guide data")
                        .font(type_::caption())
                        .color(t.text_secondary.alpha(0.6)),
                ),
        );
    }
    for span in guide_spans {
        let weight = span.slices as f32;
        let block = match span.program {
            Some(program) => {
                let current = program
                    .start_date
                    .as_deref()
                    .and_then(jellyfin_api::iso_seconds)
                    .is_some_and(|start| start <= now)
                    && program
                        .end_date
                        .as_deref()
                        .and_then(jellyfin_api::iso_seconds)
                        .is_some_and(|end| now < end);
                el().grow(weight)
                    .min_w(0.0)
                    .h(58.0)
                    .px(7.0)
                    .radius(6.0)
                    .fill(if current {
                        tok::accent(0, true).alpha(0.20)
                    } else {
                        t.card
                    })
                    .outline(
                        1.0,
                        0.0,
                        if current {
                            tok::accent(0, true).alpha(0.55)
                        } else {
                            t.card_border
                        },
                    )
                    .child(
                        text(program.name.clone())
                            .font(type_::caption())
                            .color(t.text)
                            .max_lines(2)
                            .truncate(),
                    )
            }
            None => el()
                .grow(weight)
                .min_w(0.0)
                .h(58.0)
                .radius(6.0)
                .fill(Color::WHITE.alpha(0.025)),
        };
        timeline = timeline.child(block);
    }
    row()
        .key(format!("live-guide-row-{}", channel.id))
        .role("button")
        .w_full()
        .gap(12.0)
        .items_center()
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.995)
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut State| {
            if let Some(channel) = s
                .live
                .channels
                .iter()
                .find(|channel| channel.id == id)
                .cloned()
            {
                s.play_live_channel(channel);
            }
        })
        .child(
            col()
                .w(CHANNEL_LABEL_W)
                .gap(2.0)
                .child(
                    text(format!("#{number}"))
                        .font(type_::mono_caption())
                        .color(t.text_secondary),
                )
                .child(
                    text(channel.name.clone())
                        .font(type_::label())
                        .color(t.text)
                        .truncate(),
                ),
        )
        .child(timeline)
}
