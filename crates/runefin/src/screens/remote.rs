//! Remote control: the other Jellyfin sessions, with pause/resume and stop.

use jellyfin_api::SessionInfo;
use rune::tok::{self, motion};

use crate::theme::{self, type_};
use rune::{El, ScrollSpec, col, el, row, text};

use crate::detail::format_time;
use crate::state::State;

use super::{sidebar, subtitle_style};

pub(super) fn remote_screen(s: &State) -> El {
    let t = theme::colors();
    let mut content = col()
        .key("remote-content")
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .max_w(1000.0)
        .padding(40.0)
        .pb(if s.in_app_minimized { 124.0 } else { 40.0 })
        .gap(18.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(
            row()
                .w_full()
                .gap(16.0)
                .child(text("Remote control").font(type_::page()).color(t.text))
                .child(el().grow(1.0))
                .child(
                    el().key("remote-refresh")
                        .role("button")
                        .h(32.0)
                        .px(12.0)
                        .radius(8.0)
                        .center()
                        .fill(t.card)
                        .hover(t.hover)
                        .motion(motion::SNAPPY)
                        .focusable(true)
                        .focus_ring(theme::focus_width(), 2.0, theme::FOCUS)
                        .press_scale(0.97)
                        .child(text("Refresh").font(type_::label()).color(t.text))
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| s.load_remote()),
                ),
        )
        .child(
            text("Control another Jellyfin session on your account.")
                .font(type_::meta())
                .color(t.text_secondary),
        );

    if let Some(error) = &s.remote.error {
        content = content.child(
            text(error.clone())
                .key("remote-error")
                .font(subtitle_style())
                .color(tok::accent(2, true)),
        );
    }

    let sessions: Vec<&SessionInfo> = s
        .remote
        .sessions
        .iter()
        .filter(|session| session.device_id != s.config.device_id)
        .collect();
    if sessions.is_empty() {
        content = content.child(
            text(if s.remote.loading {
                "Looking for sessions…"
            } else {
                "No other sessions are active."
            })
            .key("remote-empty")
            .font(subtitle_style())
            .color(t.text_secondary),
        );
    } else {
        content = content.child(
            col()
                .key("remote-sessions")
                .w_full()
                .gap(8.0)
                .children(sessions.iter().map(|session| session_row(session, &t))),
        );
    }

    row()
        .key("remote-screen")
        .w_full()
        .h_full()
        .fill(t.bg)
        .child(sidebar(s, &t))
        .child(el().w(1.0).h_full().fill(t.separator))
        .child(content)
}

fn session_row(session: &SessionInfo, t: &tok::Theme) -> El {
    let title = session
        .now_playing_item
        .as_ref()
        .map_or_else(|| "Idle".to_string(), |item| item.name.clone());
    let meta = format!(
        "{} · {} · {}",
        session.device_name, session.client, session.user_name
    );
    let position = session
        .play_state
        .as_ref()
        .map(|state| format_time(state.position_ticks / 10_000))
        .unwrap_or_else(|| "—".into());
    let paused = session
        .play_state
        .as_ref()
        .is_some_and(|state| state.is_paused);
    let id = session.id.clone();
    let toggle_id = id.clone();
    let stop_id = id.clone();
    let seek_id = id.clone();
    let seek_ticks = session
        .play_state
        .as_ref()
        .map_or(0, |state| state.position_ticks + 300_000_000);
    let mut controls = row().gap(8.0).focus_group();
    if session.play_state.is_some() {
        controls = controls
            .child(
                el().key(format!("remote-pause-{}", session.id))
                    .role("button")
                    .h(30.0)
                    .px(10.0)
                    .radius(7.0)
                    .center()
                    .fill(if paused { tok::accent(0, true) } else { t.bg })
                    .hover(t.hover)
                    .focusable(true)
                    .focus_ring(theme::focus_width(), 2.0, theme::FOCUS)
                    .press_scale(0.97)
                    .child(
                        text(if paused { "Resume" } else { "Pause" })
                            .font(type_::label())
                            .color(if paused { t.bg } else { t.text }),
                    )
                    .cursor(rune::Cursor::Pointer)
                    .on_press(move |s: &mut State| {
                        s.remote_command(
                            toggle_id.clone(),
                            if paused { "Unpause" } else { "Pause" },
                        )
                    }),
            )
            .child(
                el().key(format!("remote-seek-{}", session.id))
                    .role("button")
                    .h(30.0)
                    .px(10.0)
                    .radius(7.0)
                    .center()
                    .fill(t.bg)
                    .hover(t.hover)
                    .focusable(true)
                    .focus_ring(theme::focus_width(), 2.0, theme::FOCUS)
                    .press_scale(0.97)
                    .child(text("+30s").font(type_::label()).color(t.text))
                    .cursor(rune::Cursor::Pointer)
                    .on_press(move |s: &mut State| s.remote_seek(seek_id.clone(), seek_ticks)),
            )
            .child(
                el().key(format!("remote-stop-{}", session.id))
                    .role("button")
                    .h(30.0)
                    .px(10.0)
                    .radius(7.0)
                    .center()
                    .fill(t.bg)
                    .hover(t.hover)
                    .focusable(true)
                    .focus_ring(theme::focus_width(), 2.0, theme::FOCUS)
                    .press_scale(0.97)
                    .child(text("Stop").font(type_::label()).color(t.text))
                    .cursor(rune::Cursor::Pointer)
                    .on_press(move |s: &mut State| s.remote_command(stop_id.clone(), "Stop")),
            );
    }

    row()
        .key(format!("remote-session-{id}"))
        .w_full()
        .padding(12.0)
        .gap(14.0)
        .radius(10.0)
        .fill(t.card)
        .child(
            col()
                .grow(1.0)
                .gap(4.0)
                .child(text(title).font(type_::title()).color(t.text))
                .child(
                    text(meta)
                        .font(type_::mono_caption())
                        .color(t.text_secondary),
                ),
        )
        .child(
            text(position)
                .font(type_::mono_caption())
                .color(t.text_secondary),
        )
        .child(controls)
}
