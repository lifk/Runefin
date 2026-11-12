//! The in-app GStreamer player: the video surface is the page, and the OSD is a layer that
//! appears over it instead of shrinking it.

use rune::tok::{self, motion};

use crate::theme::FocusVisual;
use crate::theme::{self, type_};
use rune::{Color, Drag, El, Icon, Phase, SurfaceFit, col, el, icon, row, text, text_input};

use crate::detail::format_time;
use crate::state::State;

pub(super) fn launching_screen(_s: &State) -> El {
    let t = theme::colors();
    col()
        .key("in-app-launching")
        .w_full()
        .h_full()
        .fill(t.bg)
        .center()
        .gap(14.0)
        .child(
            text("Starting the in-app player…")
                .font(type_::title())
                .color(t.text),
        )
        .child(control("Cancel", "in-app-cancel", &t, |s: &mut State| {
            s.stop_in_app()
        }))
}

pub(super) fn in_app_screen(s: &State) -> El {
    let t = theme::colors();
    let Some(session) = &s.in_app else {
        return col().key("in-app-player");
    };
    let live = session.live;
    let title = session
        .item
        .series_name
        .clone()
        .unwrap_or_else(|| session.item.name.clone());
    let subtitle = if live {
        session
            .item
            .current_program
            .as_ref()
            .map(|program| format!("Now · {}", program.name))
            .unwrap_or_else(|| "Live".to_string())
    } else {
        session
            .item
            .episode_label()
            .map(|label| format!("{label} · {}", session.item.name))
            .unwrap_or_else(|| session.item.name.clone())
    };
    let display_ms = session.scrub_ms.unwrap_or(session.position_ms);
    let duration = session.duration_ms.unwrap_or(0).max(1);
    let progress = if live {
        0.0
    } else {
        (display_ms as f32 / duration as f32).clamp(0.0, 1.0)
    };
    let fit = if s.in_app_fill {
        SurfaceFit::Fill
    } else {
        SurfaceFit::Fit
    };

    let end_card = next_up_visible(s);
    let (window_w, _window_h) = if s.window_size.0 >= 1.0 {
        s.window_size
    } else if theme::is_big_screen() {
        (1280.0, 720.0)
    } else {
        (1280.0, 800.0)
    };
    let (video_w, video_h) = if end_card {
        (340.0, 191.0)
    } else {
        (window_w, _window_h)
    };
    let mut video = el()
        .key("in-app-video")
        .label("Video")
        .native_surface(session.player.surface(), fit)
        // A player hides the pointer with its chrome; any move brings both back.
        .cursor(if s.osd_visible {
            rune::Cursor::Default
        } else {
            rune::Cursor::Hidden
        })
        .on_pointer_move(|s: &mut State, _| s.show_in_app_osd())
        .on_press(|s: &mut State| s.toggle_in_app_pause());
    video = if end_card {
        video
            .absolute(window_w - video_w - 28.0, 136.0)
            .w(video_w)
            .h(video_h)
            .radius(10.0)
            .clip_rounded(10.0)
            .outline(1.0, 0.0, Color::WHITE.alpha(0.18))
            .shadow(0.0, 18.0, Color::BLACK.alpha(0.55))
    } else {
        video.absolute(0.0, 0.0).w_full().h_full()
    };
    let mut page = el()
        .key("in-app-player")
        .w_full()
        .h_full()
        .fill(Color::BLACK)
        .on_pointer_move(|s: &mut State, _| s.show_in_app_osd())
        .children(end_card.then(|| next_up_backdrop(s)).flatten())
        .child(video);
    if !s.osd_visible {
        // Subtitles stay on screen when the chrome hides: they belong to the film, not the OSD.
        // So does a stall indicator, which is the only thing telling the viewer why nothing moves.
        return page
            .children(subtitle_overlay(s))
            .children(buffering_overlay(session))
            .children(still_watching_card(s))
            .children(resume_conflict_card(s));
    }
    if end_card {
        return page
            .child(top_bar(&title, &subtitle, &t))
            .children(next_up_banner(s, &t))
            .children(buffering_overlay(session));
    }
    page = page
        .child(top_bar(&title, &subtitle, &t))
        .child(centre_cluster(s, session, &t))
        .child(bottom_osd(s, session, display_ms, duration, progress, &t));
    if s.player_gear_visible() {
        page = page.child(gear_panel(s, &t));
    } else if s.in_app_audio_open {
        page = page.child(audio_panel(s, &t));
    } else if s.in_app_goto_open {
        page = page.child(goto_panel(s, &t));
    } else if s.in_app_quality_open {
        page = page.child(quality_panel(s, session, &t));
    } else if s.in_app_sleep_open {
        page = page.child(sleep_panel(s, &t));
    } else if s.in_app_subtitles.open {
        page = page.child(subtitle_panel(s, &t));
    } else if s.in_app_chapters_open {
        page = page.child(chapter_panel(s, session, &t));
    } else if s.in_app_queue_open {
        page = page.child(queue_panel(s, &t));
    } else {
        page = page.children(next_up_banner(s, &t));
    }
    if s.in_app_stats {
        page = page.child(stats_panel(s, session));
    }
    // Drawn last so the bottom gradient does not grey the text; hidden while the menu is open.
    page.children(subtitle_overlay(s))
        .children(buffering_overlay(session))
        .children(still_watching_card(s))
        .children(resume_conflict_card(s))
}

/// The server and this device disagree about the resume point. The film already plays from
/// the server's position; this card is the one-second offer to use the newer local one.
fn resume_conflict_card(s: &State) -> Option<El> {
    let (local, server) = s.resume_conflict?;
    let t = theme::colors();
    Some(
        el().key("resume-conflict")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .justify_center()
            .items_center()
            .fill(Color::BLACK.alpha(0.62))
            .child(
                col()
                    .w(430.0)
                    .padding(22.0)
                    .gap(12.0)
                    .radius(14.0)
                    .fill(t.card)
                    .outline(1.0, 0.0, t.card_border)
                    .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
                    .shadow(6.0, 12.0, Color::BLACK.alpha(0.35))
                    .shadow(16.0, 48.0, Color::BLACK.alpha(0.45))
                    .child(
                        text("Where should this start?")
                            .key("resume-conflict-title")
                            .font(type_::section())
                            .color(t.text),
                    )
                    .child(
                        text("This device stopped more recently than the server's report.")
                            .font(type_::meta())
                            .color(t.text_secondary),
                    )
                    .child(
                        row()
                            .gap(10.0)
                            .child(
                                control(
                                    &format!("This device · {}", format_time(local)),
                                    "resume-conflict-local",
                                    &t,
                                    |s| s.resume_from_local(),
                                )
                                .fill(tok::accent(0, true)),
                            )
                            .child(
                                control(
                                    &format!("Server · {}", format_time(server)),
                                    "resume-conflict-server",
                                    &t,
                                    |s| s.resume_from_server(),
                                )
                                .fill(t.bg),
                            ),
                    ),
            ),
    )
}

/// A quiet pause after a long run. It sits above the player and below no menu: the only two
/// decisions are Continue and Stop.
fn still_watching_card(s: &State) -> Option<El> {
    if !s.still_watching_open {
        return None;
    }
    let t = theme::colors();
    let detail = if s.still_watching_next.is_some() {
        "The next episode is ready."
    } else {
        "You've been watching for a while."
    };
    Some(
        el().key("still-watching")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .justify_center()
            .items_center()
            .fill(Color::BLACK.alpha(0.72))
            .child(
                col()
                    .w(420.0)
                    .padding(24.0)
                    .gap(14.0)
                    .radius(14.0)
                    .fill(t.card)
                    .outline(1.0, 0.0, t.card_border)
                    .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
                    .shadow(6.0, 12.0, Color::BLACK.alpha(0.35))
                    .shadow(16.0, 48.0, Color::BLACK.alpha(0.45))
                    .items_center()
                    .child(
                        text("Still watching?")
                            .key("still-watching-title")
                            .font(type_::section())
                            .color(t.text),
                    )
                    .child(text(detail).font(type_::meta()).color(t.text_secondary))
                    .child(
                        text(format!("Pausing in {} s", s.still_watching_countdown))
                            .key("still-watching-countdown")
                            .font(type_::mono_caption())
                            .color(t.text_secondary),
                    )
                    .child(
                        row()
                            .gap(10.0)
                            .child(
                                control("Keep watching", "still-watching-continue", &t, |s| {
                                    s.still_watching_continue()
                                })
                                .fill(tok::accent(0, true)),
                            )
                            .child(
                                control("Stop", "still-watching-stop", &t, |s| {
                                    s.still_watching_stop()
                                })
                                .fill(t.bg),
                            ),
                    ),
            ),
    )
}

fn goto_panel(s: &State, t: &tok::Theme) -> El {
    el().key("in-app-goto-layer")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .justify_center()
        .items_center()
        .fill(Color::BLACK.alpha(0.45))
        .on_press(|s: &mut State| s.in_app_goto_open = false)
        .child(
            col()
                .key("in-app-goto-panel")
                .w(300.0)
                .padding(16.0)
                .gap(10.0)
                .radius(12.0)
                .focus_group()
                .fill(t.card)
                .outline(1.0, 0.0, t.card_border)
                .child(text("Go to time").font(type_::section()).color(t.text))
                .child(
                    text_input(&s.in_app_goto, type_::body())
                        .key("in-app-goto-input")
                        .label("Time")
                        .placeholder("mm:ss or h:mm:ss", t.text_secondary.alpha(0.55))
                        .role("textbox")
                        .single_line()
                        .w_full()
                        .on_edit(|s: &mut State| &mut s.in_app_goto)
                        .on_submit(|s: &mut State, _tx| s.submit_in_app_goto())
                        .on_input_key(|s: &mut State, key: &rune::KeyEvent, tx: &mut rune::Tx| {
                            // A single-line field has no use for Down; the remote needs it to
                            // reach the Go button.
                            if s.big_screen && key.key == rune::Key::Down {
                                s.note_focus("in-app-goto-submit".into());
                                tx.focus_key("in-app-goto-submit".to_string());
                                return true;
                            }
                            false
                        })
                        .color(t.text),
                )
                .child(
                    row()
                        .gap(8.0)
                        .child(el().grow(1.0))
                        .child(
                            el().key("in-app-goto-cancel")
                                .role("button")
                                .h(32.0)
                                .px(12.0)
                                .radius(8.0)
                                .center()
                                .fill(t.bg)
                                .hover(t.hover)
                                .focusable(true)
                                .focus_visual(2.0)
                                .cursor(rune::Cursor::Pointer)
                                .child(text("Cancel").font(type_::label()).color(t.text))
                                .on_press(|s: &mut State| s.in_app_goto_open = false),
                        )
                        .child(
                            el().key("in-app-goto-submit")
                                .role("button")
                                .h(32.0)
                                .px(14.0)
                                .radius(8.0)
                                .center()
                                .fill(tok::accent(0, true))
                                .hover(t.hover)
                                .focusable(true)
                                .focus_visual(2.0)
                                .cursor(rune::Cursor::Pointer)
                                .child(text("Go").font(type_::label()).color(t.bg))
                                .on_press(|s: &mut State| s.submit_in_app_goto()),
                        ),
                ),
        )
}

/// Quality is a negotiation, not a slider: the label says exactly which method is active and
/// the Why? lines name the reason the server is converting (or that it is not).
fn quality_panel(s: &State, session: &crate::state::InAppSession, t: &tok::Theme) -> El {
    let mut options = col().gap(2.0);
    for quality in [
        crate::config::PlaybackQuality::Auto,
        crate::config::PlaybackQuality::Original,
        crate::config::PlaybackQuality::Mbps20,
        crate::config::PlaybackQuality::Mbps12,
        crate::config::PlaybackQuality::Mbps8,
        crate::config::PlaybackQuality::Mbps4,
        crate::config::PlaybackQuality::Mbps2,
    ] {
        options = options.child(choice_row(
            quality.label(),
            &format!(
                "quality-{}",
                quality.label().to_lowercase().replace(' ', "-")
            ),
            s.playback_quality == quality,
            t,
            move |s: &mut State| s.apply_in_app_quality(quality),
        ));
    }
    let method = match session.play_method {
        "Transcode" => "Transcode",
        "DirectStream" => "Direct stream",
        _ => "Direct play",
    };
    let reason = session
        .play_reason
        .as_deref()
        .unwrap_or(match session.play_method {
            "Transcode" => "The server is converting this file for the chosen quality or codec.",
            "DirectStream" => "Video is copied; another stream is converted.",
            _ => "The file matches the device profile; no server work.",
        });
    el().key("in-app-quality-layer")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .justify_end()
        .items_end()
        .pb(150.0)
        .pr(24.0)
        .on_press(|s: &mut State| s.in_app_quality_open = false)
        .child(
            col()
                .key("in-app-quality-panel")
                .w(320.0)
                .padding(12.0)
                .gap(4.0)
                .radius(12.0)
                .focus_group()
                .fill(t.card)
                .outline(1.0, 0.0, t.card_border)
                .shadow(0.0, 18.0, Color::BLACK.alpha(0.5))
                .child(
                    text("Quality")
                        .font(type_::section())
                        .color(t.text)
                        .padding(4.0),
                )
                .child(options)
                .child(el().h(6.0))
                .child(
                    text(format!("{method} · {}", s.playback_quality.label()))
                        .key("in-app-quality-method")
                        .font(type_::label())
                        .color(tok::accent(0, true))
                        .padding(4.0),
                )
                .child(
                    text(reason)
                        .key("in-app-why")
                        .font(type_::caption())
                        .color(t.text_secondary)
                        .max_lines(3)
                        .padding(4.0),
                ),
        )
}

fn sleep_panel(s: &State, t: &tok::Theme) -> El {
    let mut options = col().gap(2.0);
    for (label, key, minutes) in [
        ("Off", "sleep-off", None),
        ("10 minutes", "sleep-10", Some(10)),
        ("20 minutes", "sleep-20", Some(20)),
        ("30 minutes", "sleep-30", Some(30)),
        ("45 minutes", "sleep-45", Some(45)),
        ("60 minutes", "sleep-60", Some(60)),
        ("End of episode", "sleep-end", None),
    ] {
        let on = if key == "sleep-end" {
            s.sleep_end_of_episode
        } else {
            s.sleep_deadline.is_some() && minutes.is_some()
        };
        options = options.child(
            el().key(key)
                .role("button")
                .w_full()
                .h(30.0)
                .px(8.0)
                .radius(6.0)
                .center_y()
                .fill(if on {
                    tok::accent(0, true).alpha(0.22)
                } else {
                    Color::TRANSPARENT
                })
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.98)
                .child(text(label).font(type_::label()).color(t.text))
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut State| {
                    if key == "sleep-off" {
                        s.cancel_sleep_timer();
                    } else {
                        s.set_sleep_timer(minutes);
                    }
                }),
        );
    }
    el().key("in-app-sleep-panel")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .justify_end()
        .items_end()
        .pb(150.0)
        .pr(24.0)
        .on_press(|s: &mut State| s.in_app_sleep_open = false)
        .child(
            col()
                .w(190.0)
                .padding(6.0)
                .gap(2.0)
                .radius(10.0)
                .focus_group()
                .fill(t.card)
                .outline(1.0, 0.0, t.card_border)
                .shadow(0.0, 16.0, Color::BLACK.alpha(0.45))
                .child(
                    text("Sleep timer")
                        .font(type_::caption())
                        .color(t.text_secondary)
                        .padding(8.0),
                )
                .child(options),
        )
}

/// The source is behind: show the percentage instead of a frozen picture with no explanation.
/// It sits above every panel and ignores the OSD timer, because a stall is not idle chrome.
fn buffering_overlay(session: &crate::state::InAppSession) -> Option<El> {
    let percent = session.buffering?;
    Some(
        col()
            .key("in-app-buffering")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .justify_center()
            .items_center()
            .child(
                row()
                    .gap(12.0)
                    .padding(14.0)
                    .px(20.0)
                    .radius(14.0)
                    .items_center()
                    .fill(Color::BLACK.alpha(0.68))
                    .outline(1.0, 0.0, Color::WHITE.alpha(0.14))
                    .shadow(0.0, 16.0, Color::BLACK.alpha(0.5))
                    .child(
                        text("Buffering…".to_string())
                            .font(type_::label())
                            .color(Color::WHITE),
                    )
                    .child(
                        text(format!("{percent}%"))
                            .key("in-app-buffering-percent")
                            .font(type_::mono_caption())
                            .color(Color::WHITE.alpha(0.62)),
                    ),
            ),
    )
}

/// The current cue, drawn by Rune over the video. GStreamer's own textoverlay never runs on
/// the custom sink, so this is the only path that can show subtitles.
fn subtitle_overlay(s: &State) -> Option<El> {
    // The track panel and the scrub preview both need the lower third; a cue from the old
    // position would only sit under them.
    if s.in_app_subtitles.open || s.in_app_audio_open || s.in_app_hover.is_some() {
        return None;
    }
    let cues = s.current_in_app_cues();
    let has_image = s
        .in_app_subtitles
        .image
        .as_ref()
        .is_some_and(|image| !image.bitmaps.is_empty());
    if cues.is_empty() && !has_image {
        return None;
    }
    let (window_w, window_h) = if s.window_size.0 >= 1.0 {
        s.window_size
    } else {
        (1280.0, 800.0)
    };
    let size = s.in_app_subtitles.size;
    let bottom = if next_up_visible(s) {
        252.0
    } else if s.osd_visible {
        172.0
    } else {
        54.0
    };
    let top = if s.osd_visible { 130.0 } else { 36.0 };
    if let Some(image) = &s.in_app_subtitles.image
        && !image.bitmaps.is_empty()
    {
        let (canvas_w, canvas_h) = image.screen();
        let (left, top, width, height) = video_content_rect(s);
        let sx = width / canvas_w.max(1) as f32;
        let sy = height / canvas_h.max(1) as f32;
        // PGS authors put dialogue near the bottom edge, exactly where our transport lives.
        // Lift bottom-half cues while the OSD is up, as the text overlay already does; signs in
        // the top half stay anchored to the picture.
        let lift = if next_up_visible(s) {
            220.0
        } else if s.osd_visible {
            130.0
        } else {
            0.0
        };
        let mut layer = el()
            .key("in-app-subtitles")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full();
        for (at, bitmap) in image.bitmaps.iter().enumerate() {
            let key = if at == 0 {
                "in-app-subtitle-image".to_string()
            } else {
                format!("in-app-subtitle-image-{at}")
            };
            layer = layer.child(
                el().key(key)
                    .absolute(
                        left + bitmap.x * sx,
                        top + bitmap.y * sy
                            - if top + bitmap.y * sy > height * 0.55 {
                                lift
                            } else {
                                0.0
                            },
                    )
                    .w(bitmap.width * sx)
                    .h(bitmap.height * sy)
                    .pixels_fit(&bitmap.pixels, SurfaceFit::Fill),
            );
        }
        return Some(layer);
    }
    let mut layer = el()
        .key("in-app-subtitles")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full();
    let mut keyed = 0usize;
    for band in [0u8, 1, 2] {
        let in_band: Vec<&crate::subtitles::Cue> = cues
            .iter()
            .copied()
            .filter(|cue| cue.pos.is_none())
            .filter(|cue| {
                // Plain SRT/VTT cues follow the reader's position setting; ASS keeps the
                // alignment its author chose, including signs near the top.
                let own = if cue.font_percent == 0.0
                    && cue.align == crate::subtitles::Align::BottomCenter
                {
                    // A plain cue with no explicit `{\anN}` follows the reader's setting.
                    s.subtitle_position.band()
                } else {
                    cue.align.band()
                };
                own == band
            })
            .collect();
        if in_band.is_empty() {
            continue;
        }
        let mut column = col().w_full().gap(6.0).px(56.0);
        column = match band {
            0 => column.absolute(0.0, 0.0).h_full().justify_start().pt(top),
            1 => column.absolute(0.0, 0.0).h_full().justify_center(),
            _ => column.absolute(0.0, 0.0).h_full().justify_end().pb(bottom),
        };
        for cue in in_band {
            let key = if keyed == 0 {
                "in-app-subtitle-box".to_string()
            } else {
                format!("in-app-subtitle-box-{keyed}")
            };
            keyed += 1;
            let mut row = row().w_full();
            row = match cue.align.side() {
                0 => row.justify_start(),
                1 => row.justify_center(),
                _ => row.justify_end(),
            };
            column = column.child(row.child(cue_box(s, cue, size, window_h, &key)));
        }
        layer = layer.child(column);
    }
    // Explicitly positioned cues (ASS `\pos`) are signs: they sit where the script says.
    for cue in cues.iter().filter(|cue| cue.pos.is_some()) {
        let Some((fx, fy)) = cue.pos else {
            continue;
        };
        let key = if keyed == 0 {
            "in-app-subtitle-box".to_string()
        } else {
            format!("in-app-subtitle-box-{keyed}")
        };
        keyed += 1;
        let x = (fx * window_w).clamp(8.0, (window_w - 40.0).max(8.0));
        let y = (fy * window_h).clamp(8.0, (window_h - 40.0).max(8.0));
        layer = layer.child(
            el().absolute(x, y)
                .child(cue_box(s, cue, size, window_h, &key)),
        );
    }
    Some(layer)
}

/// The video's rectangle inside the window, so PGS coordinates land on the picture rather than
/// the letterbox bars. The player uses the same fit/fill choice as the native surface.
fn video_content_rect(s: &State) -> (f32, f32, f32, f32) {
    let (window_w, window_h) = if s.window_size.0 >= 1.0 {
        s.window_size
    } else {
        (1280.0, 800.0)
    };
    let Some((_, video_w, video_h, _)) = s.in_app_video else {
        return (0.0, 0.0, window_w, window_h);
    };
    let (video_w, video_h) = (video_w.max(1) as f32, video_h.max(1) as f32);
    let scale = if s.in_app_fill {
        (window_w / video_w).max(window_h / video_h)
    } else {
        (window_w / video_w).min(window_h / video_h)
    };
    let width = video_w * scale;
    let height = video_h * scale;
    (
        (window_w - width) / 2.0,
        (window_h - height) / 2.0,
        width,
        height,
    )
}

/// One cue's plate and lines. ASS sizes are a fraction of the script height, so they scale with
/// the window; SRT uses the reader's size setting against a 30 pt base.
fn cue_box(s: &State, cue: &crate::subtitles::Cue, size: f32, window_h: f32, key: &str) -> El {
    let px = if cue.font_percent > 0.0 {
        (cue.font_percent * window_h * size).clamp(14.0, 64.0)
    } else {
        30.0 * size
    };
    // Plain cues use the reader's face and colour; an ASS cue keeps the authored colour and
    // weight, because signs and karaoke lose their meaning under a blanket override.
    let ass = cue.font_percent > 0.0;
    let bold = cue.bold || (!ass && s.subtitle_style.bold());
    let italic = cue.italic || (!ass && s.subtitle_style.italic());
    let color_authored = ass || cue.color_explicit;
    let family = match s.subtitle_font {
        crate::config::SubtitleFont::Sans => rune::Family::Sans,
        // A CSS-style list, tried in order before the sans slot: a minimal install may carry any
        // one of these, and fontconfig resolves the generic name on the way out.
        crate::config::SubtitleFont::Serif => {
            rune::Family::Named("DejaVu Serif, Liberation Serif, Noto Serif, FreeSerif, serif")
        }
        crate::config::SubtitleFont::Mono => rune::Family::SystemMono,
    };
    let mut style = rune::TextStyle::new(px, if bold { 700.0 } else { 600.0 })
        .lh(px * 1.24)
        .family(family);
    if italic {
        style = style.italic();
    }
    let rgb = if color_authored {
        cue.color
    } else {
        s.subtitle_color.rgb()
    };
    let color = rune::Color {
        r: rune::color::srgb_decode(rgb[0] as f32 / 255.0),
        g: rune::color::srgb_decode(rgb[1] as f32 / 255.0),
        b: rune::color::srgb_decode(rgb[2] as f32 / 255.0),
        a: (cue.opacity as f32 / 255.0) * s.subtitle_opacity,
    };
    let outline = if ass && cue.border_percent > 0.0 {
        (cue.border_percent * window_h * size).clamp(0.0, 12.0)
    } else {
        s.subtitle_outline.width()
    };
    // ASS carries its own outline colour and a drop shadow; Rune text has no stroke, so both
    // are offset copies of the box behind the glyph fill. The reader's preset keeps black.
    let outline_color = if ass && cue.border_percent > 0.0 {
        rune::Color {
            r: rune::color::srgb_decode(cue.border_color[0] as f32 / 255.0),
            g: rune::color::srgb_decode(cue.border_color[1] as f32 / 255.0),
            b: rune::color::srgb_decode(cue.border_color[2] as f32 / 255.0),
            a: 1.0,
        }
    } else {
        Color::BLACK
    };
    let shadow = if ass && cue.shadow_percent > 0.0 {
        (cue.shadow_percent * window_h * size).clamp(0.0, 12.0)
    } else {
        2.0
    };
    let shadow_color = if ass && cue.shadow_percent > 0.0 {
        rune::Color {
            r: rune::color::srgb_decode(cue.shadow_color[0] as f32 / 255.0),
            g: rune::color::srgb_decode(cue.shadow_color[1] as f32 / 255.0),
            b: rune::color::srgb_decode(cue.shadow_color[2] as f32 / 255.0),
            a: (cue.shadow_opacity as f32 / 255.0) * s.subtitle_opacity,
        }
    } else {
        Color::BLACK.alpha(0.85)
    };
    let mut lines = col()
        .key(key)
        .items_center()
        .gap(2.0)
        .px(18.0)
        .py(8.0)
        .radius(10.0)
        .fill(Color::BLACK.alpha(s.subtitle_background.alpha() * s.subtitle_background_opacity));
    for line in cue.text.lines() {
        // A box shadow draws the element's rectangle, which behind text is a bar, not an
        // outline. Duplicate glyph runs at the stroke offsets give a real outline; the cost is
        // bounded to the two or three lines on screen.
        let mut stack = el().items_start();
        if shadow > 0.0 {
            stack = stack.child(
                el().absolute(if ass { shadow } else { 0.0 }, shadow)
                    .child(text(line.to_string()).font(style).color(shadow_color)),
            );
        }
        if outline > 0.0 {
            // A ring of copies, not four corners: at 3 pt the gaps between eight directions
            // show as stripes through a thick ASS outline. A second inner ring fills them.
            let steps = 16;
            for step in 0..steps {
                let angle = step as f32 * std::f32::consts::TAU / steps as f32;
                let (x, y) = (angle.cos() * outline, angle.sin() * outline);
                stack = stack.child(
                    el().absolute(x, y)
                        .child(text(line.to_string()).font(style).color(outline_color)),
                );
            }
            if outline > 2.2 {
                for step in 0..8 {
                    let angle = step as f32 * std::f32::consts::TAU / 8.0;
                    let (x, y) = (angle.cos() * outline * 0.5, angle.sin() * outline * 0.5);
                    stack = stack.child(
                        el().absolute(x, y)
                            .child(text(line.to_string()).font(style).color(outline_color)),
                    );
                }
            }
        }
        lines = lines.child(stack.child(text(line.to_string()).font(style).color(color)));
    }
    // ASS rotates and scales the whole cue about its centre. Rune scales uniformly, so a
    // non-uniform `scx`/`scy` pair keeps its area rather than its exact aspect.
    let scale = (cue.scale.0 * cue.scale.1)
        .max(0.01)
        .sqrt()
        .clamp(0.25, 4.0);
    if (scale - 1.0).abs() > 0.001 || cue.rotation.abs() > 0.001 {
        lines = lines
            .origin(0.5, 0.5)
            .scale(scale)
            .rotate(cue.rotation.to_radians());
    }
    lines
}

/// Track list plus the two controls readers actually ask for. It sits above the transport,
/// anchored right so it never covers the play button.
fn next_up_visible(s: &State) -> bool {
    s.osd_visible
        && s.in_app_hover.is_none()
        && s.next_up_eligible()
        && !s.player_panel_open()
        && !s.player_gear_visible()
}

/// The next episode's still behind the end card, dimmed enough that the card and the corner
/// video keep their contrast.
fn next_up_backdrop(s: &State) -> Option<El> {
    let next = s.in_app_next.as_ref()?;
    let key = s.next_episode_image_key(next)?;
    let image = s.images.get(&key)?;
    Some(
        el().absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .child(
                el().w_full()
                    .h_full()
                    .pixels_fit(image.pixels(), SurfaceFit::Fill)
                    .opacity(0.32),
            )
            .child(el().absolute(0.0, 0.0).w_full().h_full().linear_gradient(
                std::f32::consts::PI,
                [
                    (0.0, Color::BLACK.alpha(0.35)),
                    (0.55, Color::BLACK.alpha(0.72)),
                    (1.0, Color::BLACK),
                ],
            )),
    )
}

/// The last-30-seconds offer: the next episode's still, label, title and three decisions. It is
/// a card over a dimmed still, while the ending episode keeps playing in the corner.
fn next_up_banner(s: &State, t: &tok::Theme) -> Option<El> {
    if !next_up_visible(s) {
        return None;
    }
    let next = s.in_app_next.as_ref()?;
    let session = s.in_app.as_ref()?;
    let duration = session.duration_ms.unwrap_or(0);
    let remaining = (duration - session.position_ms).max(0);
    let status = if session.paused {
        "Paused".to_string()
    } else if remaining <= 1_000 {
        "Playing now".to_string()
    } else {
        format!("Playing in 0:{:02}", (remaining + 999) / 1000)
    };
    let still = s
        .next_episode_image_key(next)
        .and_then(|key| s.images.get(&key));
    let thumb = if let Some(image) = still {
        el().w(300.0)
            .h(169.0)
            .radius(10.0)
            .clip_rounded(10.0)
            .pixels_fit(image.pixels(), SurfaceFit::Fill)
    } else {
        el().w(300.0)
            .h(169.0)
            .radius(10.0)
            .fill(Color::WHITE.alpha(0.07))
    };
    let label = next
        .episode_label()
        .map(|label| format!("UP NEXT · {label}"))
        .unwrap_or_else(|| "UP NEXT".to_string());
    let runtime = next
        .runtime_ms()
        .map(|ms| format!("{} min", (ms / 60_000).max(1)))
        .unwrap_or_default();
    let banner = row()
        .key("in-app-next")
        .w(860.0)
        .h(230.0)
        .padding(18.0)
        .gap(18.0)
        .items_start()
        .radius(16.0)
        .fill(Color::BLACK.alpha(0.84))
        .outline(1.0, 0.0, Color::WHITE.alpha(0.14))
        .shadow(0.0, 24.0, Color::BLACK.alpha(0.55))
        .child(
            col().w(300.0).gap(8.0).child(thumb).child(
                text(status)
                    .font(type_::label())
                    .color(Color::WHITE.alpha(0.72)),
            ),
        )
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(7.0)
                .child(
                    row()
                        .w_full()
                        .gap(10.0)
                        .child(
                            text(label)
                                .font(type_::caption())
                                .color(tok::accent(0, true)),
                        )
                        .child(el().grow(1.0))
                        .children((!runtime.is_empty()).then(|| {
                            text(runtime)
                                .font(type_::caption())
                                .color(Color::WHITE.alpha(0.55))
                        })),
                )
                .child(
                    text(next.name.clone())
                        .font(type_::title())
                        .color(Color::WHITE)
                        .max_lines(2)
                        .truncate(),
                )
                .children(next.overview.clone().map(|overview| {
                    text(overview)
                        .font(type_::overview())
                        .color(Color::WHITE.alpha(0.72))
                        .max_lines(2)
                }))
                .child(el().grow(1.0))
                .child(
                    row()
                        .gap(8.0)
                        .child(accent_control(
                            "Play now",
                            "in-app-next-play",
                            t,
                            |s: &mut State| s.play_in_app_next(),
                        ))
                        .child(control(
                            "Watch credits",
                            "in-app-next-credits",
                            t,
                            |s: &mut State| s.dismiss_in_app_next(),
                        ))
                        .child(control(
                            "Dismiss",
                            "in-app-next-dismiss",
                            t,
                            |s: &mut State| s.dismiss_in_app_next(),
                        )),
                ),
        );
    Some(
        col()
            .key("in-app-next-layer")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .justify_end()
            .items_start()
            .px(64.0)
            .pb(64.0)
            .child(banner),
    )
}

fn accent_control(
    label: &str,
    key: &str,
    _t: &tok::Theme,
    on: impl Fn(&mut State) + 'static,
) -> El {
    el().key(key)
        .role("button")
        .h(36.0)
        .px(14.0)
        .radius(9.0)
        .center()
        .fill(tok::accent(0, true))
        .gradient(Color::WHITE.alpha(0.18), Color::TRANSPARENT)
        .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
        .shadow(4.0, 12.0, tok::accent(0, true).alpha(0.35))
        .hover(Color::WHITE.alpha(0.18))
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.96)
        .cursor(rune::Cursor::Pointer)
        .child(text(label).font(type_::label()).color(Color::WHITE))
        .on_press(move |s: &mut State| on(s))
}

/// The audio track picker. Language titles come from MediaSources; the selected dot comes
/// from playbin's own `current-audio`, so it stays true even if GStreamer auto-picked one.
fn audio_panel(s: &State, t: &tok::Theme) -> El {
    let current = s
        .in_app
        .as_ref()
        .map(|session| session.player.audio_tracks().1)
        .unwrap_or(-1);
    let mut panel = col()
        .key("in-app-audio-panel")
        .transition(rune::Transition::offset(0.0, 8.0).combined(rune::Transition::opacity()))
        .w(360.0)
        .padding(16.0)
        .gap(6.0)
        .radius(14.0)
        .focus_group()
        .child(text("Audio").font(type_::section()).color(t.text));
    if s.in_app_audio_tracks.is_empty() {
        panel = panel.child(
            text("This item has one audio track.")
                .font(type_::meta())
                .color(t.text_secondary),
        );
    }
    for track in &s.in_app_audio_tracks {
        let ordinal = track.ordinal;
        panel = panel.child(choice_row(
            &accessibility_tag(&track.display_title, "audio"),
            &format!("audio-track-{ordinal}"),
            ordinal == current,
            t,
            move |s: &mut State| s.select_in_app_audio(ordinal),
        ));
    }
    panel = panel
        .child(el().h(6.0))
        .child(choice_row(
            "Downmix to stereo",
            "audio-downmix",
            s.downmix_stereo,
            t,
            |s: &mut State| s.toggle_downmix(),
        ))
        .child(choice_row(
            &format!("Sound: {}", s.audio_mode.label()),
            "audio-mode",
            s.audio_mode != crate::config::AudioMode::Off,
            t,
            |s: &mut State| s.cycle_audio_mode(),
        ))
        .child(
            row()
                .w_full()
                .h(34.0)
                .items_center()
                .gap(8.0)
                .child(
                    text("Audio delay")
                        .font(type_::label())
                        .color(t.text)
                        .grow(1.0),
                )
                .child(queue_control("−", "audio-delay-down".into(), t, |s| {
                    s.adjust_audio_delay(-25)
                }))
                .child(
                    text(format!("{} ms", s.audio_delay_ms))
                        .key("audio-delay-value")
                        .font(type_::mono_caption())
                        .color(t.text_secondary)
                        .w(64.0)
                        .center(),
                )
                .child(queue_control("+", "audio-delay-up".into(), t, |s| {
                    s.adjust_audio_delay(25)
                })),
        );
    el().key("in-app-audio-layer")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .justify_end()
        .items_end()
        .pb(150.0)
        .pr(24.0)
        .on_press(|s: &mut State| s.close_in_app_audio_menu())
        .child(theme::elevation::frosted(panel, t))
}

/// An accessibility tag the track title earns: audio description or SDH. The server names
/// these in the display title; we never infer them from the language.
fn accessibility_tag(title: &str, kind: &str) -> String {
    let lower = title.to_ascii_lowercase();
    let tag = match kind {
        "audio" if lower.contains("descriptive") || lower.contains("audio description") => "AD",
        "subtitle"
            if lower.contains("sdh")
                || lower.contains("hearing impaired")
                || lower.contains("hard of hearing") =>
        {
            "SDH"
        }
        _ => return title.to_string(),
    };
    if lower.contains(tag.to_ascii_lowercase().as_str()) {
        title.to_string()
    } else {
        format!("{title} · {tag}")
    }
}

/// Language chips offered by the online lookup. Order is by how often they are wanted, not
/// alphabetically; the codes are what the providers expect.
const SUBTITLE_LANGUAGES: [(&str, &str); 6] = [
    ("eng", "English"),
    ("spa", "Español"),
    ("fre", "Français"),
    ("ger", "Deutsch"),
    ("jpn", "日本語"),
    ("por", "Português"),
];

/// The chapter list. It is the same information the seek bar's ticks carry, made readable and
/// clickable: one row per chapter, the current one marked, the time on the right.
fn chapter_panel(s: &State, session: &crate::state::InAppSession, t: &tok::Theme) -> El {
    let chapters = &session.item.chapters;
    let current = chapters
        .iter()
        .rposition(|chapter| chapter.start_ms() <= session.position_ms)
        .unwrap_or(0);
    let mut panel = col()
        .key("in-app-chapter-panel")
        .w(368.0)
        .padding(14.0)
        .gap(4.0)
        .radius(14.0)
        .focus_group()
        .child(text("Chapters").font(type_::section()).color(t.text));
    for (at, chapter) in chapters.iter().take(8).enumerate() {
        let selected = at == current;
        panel = panel.child(
            row()
                .key(format!("in-app-chapter-{at}"))
                .role("button")
                .w_full()
                .h(40.0)
                .px(8.0)
                .radius(8.0)
                .items_center()
                .gap(8.0)
                .fill(if selected {
                    theme::FOCUS.alpha(0.14)
                } else {
                    Color::TRANSPARENT
                })
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.99)
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut State| s.seek_in_app_chapter(at))
                .child(
                    text(if selected { "▶" } else { "" })
                        .w(12.0)
                        .font(type_::caption())
                        .color(tok::accent(0, true)),
                )
                .children(s.chapter_image_key(&session.item, at).map(|key| {
                    let thumb = s.images.get(&key);
                    el().key(format!("in-app-chapter-thumb-{at}"))
                        .w(46.0)
                        .h(26.0)
                        .radius(4.0)
                        .clip_rounded(4.0)
                        .fill(t.bg)
                        .children(thumb.map(|image| {
                            el().w_full()
                                .h_full()
                                .pixels_fit(image.pixels(), SurfaceFit::Fill)
                        }))
                }))
                .child(
                    text(chapter.name.clone())
                        .font(type_::label())
                        .color(if selected { t.text } else { t.text_secondary })
                        .truncate(),
                )
                .child(el().grow(1.0))
                .child(
                    text(format_time(chapter.start_ms()))
                        .font(type_::mono_caption())
                        .color(t.text_secondary),
                ),
        );
    }
    if chapters.len() > 8 {
        panel = panel.child(
            text(format!("and {} more", chapters.len() - 8))
                .font(type_::caption())
                .color(t.text_secondary),
        );
    }
    el().key("in-app-chapter-layer")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .justify_start()
        .items_end()
        .pb(150.0)
        .pl(24.0)
        .on_press(|s: &mut State| s.close_in_app_chapters())
        .child(theme::elevation::frosted(panel, t))
}

/// The run's items, opened from the `2 of 4` chip. It is the queue made visible: what EOS and
/// `N`/`P` will follow, and a way to jump anywhere in it.
fn queue_control(
    label: &str,
    key: String,
    t: &tok::Theme,
    on: impl Fn(&mut State) + 'static,
) -> El {
    el().key(key)
        .role("button")
        .w(26.0)
        .h(26.0)
        .radius(6.0)
        .center()
        .fill(t.bg)
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.94)
        .child(
            text(label.to_string())
                .font(type_::caption())
                .color(t.text_secondary),
        )
        .cursor(rune::Cursor::Pointer)
        .on_press(on)
}

fn queue_panel(s: &State, t: &tok::Theme) -> El {
    let mut panel = col()
        .key("in-app-queue-panel")
        .transition(rune::Transition::offset(0.0, 8.0).combined(rune::Transition::opacity()))
        .w(360.0)
        .padding(14.0)
        .gap(4.0)
        .radius(14.0)
        .focus_group()
        .child(
            row()
                .w_full()
                .items_center()
                .child(
                    text("Up next")
                        .font(type_::section())
                        .color(t.text)
                        .grow(1.0),
                )
                .child(
                    text(format!("{} of {}", s.queue_at + 1, s.queue.len()))
                        .font(type_::mono_caption())
                        .color(t.text_secondary),
                ),
        );
    for (at, item) in s.queue.iter().take(12).enumerate() {
        let selected = at == s.queue_at;
        let label = item
            .episode_label()
            .map(|label| format!("{label} · {}", item.name))
            .unwrap_or_else(|| item.name.clone());
        panel = panel.child(
            row()
                .key(format!("in-app-queue-{at}"))
                .role("button")
                .w_full()
                .h(32.0)
                .px(8.0)
                .radius(8.0)
                .items_center()
                .gap(8.0)
                .fill(if selected {
                    theme::FOCUS.alpha(0.14)
                } else {
                    Color::TRANSPARENT
                })
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.99)
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut State| s.play_queue_at(at))
                .child(
                    text(if selected { "▶" } else { "" })
                        .w(12.0)
                        .font(type_::caption())
                        .color(tok::accent(0, true)),
                )
                .child(
                    text(label)
                        .font(type_::label())
                        .color(if selected { t.text } else { t.text_secondary })
                        .truncate(),
                )
                .child(el().grow(1.0))
                .child(queue_control(
                    "↑",
                    format!("queue-up-{at}"),
                    t,
                    move |s| s.queue_move(at, -1),
                ))
                .child(queue_control(
                    "↓",
                    format!("queue-down-{at}"),
                    t,
                    move |s| s.queue_move(at, 1),
                ))
                .child(queue_control(
                    "×",
                    format!("queue-remove-{at}"),
                    t,
                    move |s| s.queue_remove(at),
                )),
        );
    }
    if s.queue.len() > 12 {
        panel = panel.child(
            text(format!("and {} more", s.queue.len() - 12))
                .font(type_::caption())
                .color(t.text_secondary),
        );
    }
    if !s.queue.is_empty() {
        panel = panel.child(
            control("Save as playlist", "queue-save-playlist", t, |s| {
                s.save_queue_as_playlist()
            })
            .help("Save this run as a playlist"),
        );
    }
    el().key("in-app-queue-layer")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .justify_start()
        .items_end()
        .pb(150.0)
        .pl(24.0)
        .on_press(|s: &mut State| s.close_in_app_queue())
        .child(theme::elevation::frosted(panel, t))
}

fn subtitle_panel(s: &State, t: &tok::Theme) -> El {
    // The online lookup replaces the track list inside the same panel, so opening it keeps the
    // player's other chrome still and Esc steps back one level at a time.
    let panel = if s.in_app_subtitle_search.open {
        subtitle_search_panel(s, t)
    } else {
        subtitle_tracks_panel(s, t)
    };
    // The panel is a full-size layer so a click misses the video behind it.
    el().key("in-app-subtitle-layer")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .justify_end()
        .items_end()
        .pb(150.0)
        .pr(24.0)
        .on_press(|s: &mut State| s.close_in_app_subtitles_menu())
        .child(theme::elevation::frosted(panel, t))
}

fn subtitle_tracks_panel(s: &State, t: &tok::Theme) -> El {
    let subs = &s.in_app_subtitles;
    let mut panel = col()
        .key("in-app-subtitle-panel")
        .w(360.0)
        .padding(16.0)
        .gap(6.0)
        .radius(14.0)
        .focus_group();
    panel = panel.child(
        row()
            .w_full()
            .items_center()
            .child(text("Subtitles").font(type_::section()).color(t.text))
            .child(el().grow(1.0))
            .children((subs.tracks.len() > 6).then(|| {
                text(format!("{} tracks", subs.tracks.len()))
                    .key("subtitle-track-count")
                    .font(type_::caption())
                    .color(t.text_secondary)
            })),
    );

    // The track list is the only part that grows with the file; the delay, size and search
    // controls stay put under it. A film can carry twenty tracks, so the list scrolls inside a
    // fixed window instead of pushing the panel past the top of the screen.
    let rows = subs.tracks.len() + 1;
    let content_h = rows as f32 * 40.0 + 8.0;
    let mut tracks = col()
        .key("subtitle-track-list")
        .w_full()
        .h(content_h)
        .gap(6.0);
    tracks = tracks.child(choice_row(
        "Off",
        "subtitle-track-off",
        subs.selected.is_none(),
        t,
        |s: &mut State| s.select_in_app_subtitle(None),
    ));
    if subs.tracks.is_empty() {
        tracks = tracks.child(
            text(if subs.error.is_some() {
                "This item has no text subtitles."
            } else {
                "Looking for subtitle tracks…"
            })
            .font(type_::meta())
            .color(t.text_secondary),
        );
    }
    for (at, track) in subs.tracks.iter().enumerate() {
        if !track.is_text {
            if crate::image_subtitles::ImageCodec::for_codec(track.codec.as_deref()).is_some() {
                let label = format!(
                    "{} · {}",
                    track.display_title,
                    track
                        .codec
                        .as_deref()
                        .unwrap_or("image")
                        .to_ascii_uppercase()
                );
                tracks = tracks.child(choice_row(
                    &label,
                    &format!("subtitle-track-{at}"),
                    subs.selected == Some(at),
                    t,
                    move |s: &mut State| s.select_in_app_subtitle(Some(at)),
                ));
                continue;
            }
            // An unsupported bitmap format explains itself and offers the online text search.
            tracks = tracks.child(
                row()
                    .key(format!("subtitle-track-{at}"))
                    .role("button")
                    .w_full()
                    .h(34.0)
                    .px(10.0)
                    .radius(8.0)
                    .items_center()
                    .gap(8.0)
                    .fill(Color::TRANSPARENT)
                    .hover(t.hover)
                    .motion(motion::SNAPPY)
                    .focusable(true)
                    .focus_visual(2.0)
                    .press_scale(0.98)
                    .cursor(rune::Cursor::Pointer)
                    .on_press(|s: &mut State| s.open_in_app_subtitle_search())
                    .child(
                        text("▣".to_string())
                            .font(type_::label())
                            .color(t.text_secondary),
                    )
                    .child(
                        text(track.display_title.clone())
                            .font(type_::label())
                            .color(t.text_secondary)
                            .truncate(),
                    )
                    .child(el().grow(1.0))
                    .child(
                        text("Image · find text".to_string())
                            .key(format!("subtitle-image-{at}"))
                            .font(type_::caption())
                            .color(tok::accent(0, true)),
                    ),
            );
            continue;
        }
        let label = accessibility_tag(&track.display_title, "subtitle");
        tracks = tracks.child(choice_row(
            &label,
            &format!("subtitle-track-{at}"),
            subs.selected == Some(at),
            t,
            move |s: &mut State| s.select_in_app_subtitle(Some(at)),
        ));
    }
    if subs.image_loading {
        tracks = tracks.child(
            text("Preparing image subtitles…")
                .key("subtitle-image-loading")
                .font(type_::caption())
                .color(t.text_secondary),
        );
    }
    if let Some(error) = &subs.image_error {
        tracks = tracks.child(
            text(error.clone())
                .key("subtitle-image-error")
                .font(type_::caption())
                .color(tok::accent(3, true)),
        );
    }
    if let Some(error) = &subs.error
        && !subs.tracks.is_empty()
    {
        tracks = tracks.child(
            text(error.clone())
                .font(type_::caption())
                .color(tok::accent(3, true)),
        );
    }
    // A scroll's content fills a set viewport height unless it sets its own; the list gives
    // itself the full content height so the viewport has something to scroll.
    panel = panel.child(
        el().w_full()
            .h(content_h.min(330.0))
            .scroll(rune::ScrollSpec {
                stick_bottom: false,
            })
            .scroll_edge_fade(16.0)
            .child(tracks),
    );
    panel = panel
        .child(el().w_full().h(1.0).fill(t.separator).mt(6.0))
        .child(stepper_row(
            "Delay",
            &format!("{:+.1} s", subs.delay_ms as f32 / 1000.0),
            "subtitle-delay-down",
            "subtitle-delay-up",
            t,
            |s: &mut State| s.adjust_in_app_subtitle_delay(-100),
            |s: &mut State| s.adjust_in_app_subtitle_delay(100),
        ))
        .child(stepper_row(
            "Size",
            &format!("{}%", (subs.size * 100.0).round() as i32),
            "subtitle-size-down",
            "subtitle-size-up",
            t,
            |s: &mut State| s.adjust_in_app_subtitle_size(-0.1),
            |s: &mut State| s.adjust_in_app_subtitle_size(0.1),
        ));
    // Aligning the cues to the film's own sound. Hidden for a bitmap track (its cues are
    // pictures the server times) and for anything live, where there is no file to listen to.
    let live = s.in_app.as_ref().is_some_and(|session| session.live);
    if !live && !subs.cues.is_empty() && subs.image.is_none() {
        let status = if subs.syncing {
            let seconds = s
                .subtitle_sync_progress
                .load(std::sync::atomic::Ordering::Relaxed);
            format!("Listening… {seconds} s")
        } else {
            subs.sync_note
                .clone()
                .unwrap_or_else(|| "Match the cues to the voices".into())
        };
        panel = panel.child(
            row()
                .key("subtitle-sync")
                .role("button")
                .w_full()
                .h(36.0)
                .px(10.0)
                .radius(8.0)
                .items_center()
                .gap(8.0)
                .fill(Color::TRANSPARENT)
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.98)
                .label("Sync subtitles to the audio")
                .help("Listen to the film and move the cues to match")
                .cursor(rune::Cursor::Pointer)
                .on_press(|s: &mut State| s.sync_in_app_subtitles())
                .child(
                    text("≋".to_string())
                        .font(type_::label())
                        .color(tok::accent(0, true)),
                )
                .child(
                    text("Sync to audio".to_string())
                        .font(type_::label())
                        .color(t.text),
                )
                .child(el().grow(1.0))
                .child(
                    text(status)
                        .key("subtitle-sync-status")
                        .font(type_::caption())
                        .color(if subs.sync_note.is_some() && !subs.syncing {
                            t.text
                        } else {
                            t.text_secondary
                        })
                        .truncate(),
                ),
        );
    }
    panel = panel
        .child(el().w_full().h(1.0).fill(t.separator).mt(6.0))
        .child(
            row()
                .key("subtitle-search-open")
                .role("button")
                .w_full()
                .h(36.0)
                .px(10.0)
                .radius(8.0)
                .items_center()
                .gap(8.0)
                .fill(theme::FOCUS.alpha(0.10))
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.98)
                .help("Download a subtitle from the server's providers")
                .cursor(rune::Cursor::Pointer)
                .on_press(|s: &mut State| s.open_in_app_subtitle_search())
                .child(
                    text("＋".to_string())
                        .font(type_::label())
                        .color(tok::accent(0, true)),
                )
                .child(
                    text("Search online…".to_string())
                        .font(type_::label())
                        .color(t.text)
                        .truncate(),
                ),
        );
    panel
}

/// The online subtitle lookup: language chips, then the provider results with the facts that
/// decide between them (downloads, rating, exact-file match, forced/SDH, format).
fn subtitle_search_panel(s: &State, t: &tok::Theme) -> El {
    let search = &s.in_app_subtitle_search;
    let language_label = SUBTITLE_LANGUAGES
        .iter()
        .find(|(code, _)| *code == search.language)
        .map(|(_, label)| *label)
        .unwrap_or(search.language.as_str());
    let mut panel = col()
        .key("in-app-subtitle-search")
        .w(400.0)
        .padding(16.0)
        .gap(6.0)
        .radius(14.0)
        .focus_group()
        .child(
            row()
                .key("subtitle-search-close")
                .role("button")
                .h(26.0)
                .px(8.0)
                .radius(13.0)
                .items_center()
                .fill(Color::TRANSPARENT)
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(0.96)
                .cursor(rune::Cursor::Pointer)
                .on_press(|s: &mut State| s.close_in_app_subtitle_search())
                .child(
                    text("← Back".to_string())
                        .font(type_::meta())
                        .color(t.text_secondary),
                ),
        )
        .child(
            text("Find subtitles".to_string())
                .font(type_::section())
                .color(t.text),
        )
        .child(
            text(format!("Results for {language_label}"))
                .font(type_::meta())
                .color(t.text_secondary),
        );
    // Two rows of three: six chips do not fit one row at this width, and a clipped language
    // button is worse than a second line.
    let mut chip_rows = col().w_full().gap(6.0);
    for group in SUBTITLE_LANGUAGES.chunks(3) {
        let mut chips = row().w_full().gap(6.0);
        for (code, label) in group {
            let code = *code;
            let label = *label;
            let selected = code == search.language;
            chips = chips.child(
                el().key(format!("subtitle-lang-{code}"))
                    .role("button")
                    .h(28.0)
                    .px(10.0)
                    .radius(14.0)
                    .center()
                    .fill(if selected {
                        theme::FOCUS
                    } else {
                        Color::TRANSPARENT
                    })
                    .outline(1.0, 0.0, if selected { theme::FOCUS } else { t.separator })
                    .hover(t.hover)
                    .motion(motion::SNAPPY)
                    .focusable(true)
                    .focus_visual(2.0)
                    .press_scale(0.96)
                    .cursor(rune::Cursor::Pointer)
                    .on_press(move |s: &mut State| {
                        if s.in_app_subtitle_search.language != code {
                            s.search_in_app_subtitles(code.to_string());
                        }
                    })
                    .child(
                        text(label.to_string())
                            .font(type_::caption())
                            .color(if selected {
                                Color::BLACK
                            } else {
                                t.text_secondary
                            }),
                    ),
            );
        }
        chip_rows = chip_rows.child(chips);
    }
    panel = panel.child(chip_rows);
    if search.loading {
        panel = panel.child(
            text("Searching the subtitle providers…")
                .font(type_::meta())
                .color(t.text_secondary)
                .mt(4.0),
        );
    } else if let Some(error) = &search.error {
        panel = panel
            .child(
                text(error.clone())
                    .font(type_::meta())
                    .color(tok::accent(3, true)),
            )
            .child(
                el().key("subtitle-search-retry")
                    .role("button")
                    .h(30.0)
                    .px(12.0)
                    .radius(15.0)
                    .center()
                    .fill(theme::FOCUS.alpha(0.14))
                    .hover(t.hover)
                    .motion(motion::SNAPPY)
                    .focusable(true)
                    .focus_visual(2.0)
                    .press_scale(0.97)
                    .cursor(rune::Cursor::Pointer)
                    .on_press(|s: &mut State| {
                        let language = s.in_app_subtitle_search.language.clone();
                        s.search_in_app_subtitles(language);
                    })
                    .child(
                        text("Try again".to_string())
                            .font(type_::caption())
                            .color(t.text),
                    ),
            );
    } else if search.results.is_empty() {
        panel = panel.child(
            text(format!(
                "No {language_label} subtitles found for this item."
            ))
            .font(type_::meta())
            .color(t.text_secondary)
            .mt(4.0),
        );
    }
    for (at, result) in search.results.iter().take(6).enumerate() {
        let downloading = search
            .downloading
            .as_deref()
            .is_some_and(|id| id == result.id);
        let id = result.id.clone();
        let mut item = col()
            .key(format!("subtitle-result-{at}"))
            .role("button")
            .w_full()
            .padding(9.0)
            .gap(1.0)
            .radius(8.0)
            .fill(Color::TRANSPARENT)
            .hover(t.hover)
            .motion(motion::SNAPPY)
            .focusable(true)
            .focus_visual(2.0)
            .press_scale(0.99)
            .cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut State| s.download_in_app_subtitle(id.clone()))
            .child(
                text(result.name.clone())
                    .font(type_::label())
                    .color(t.text)
                    .truncate(),
            )
            .child(
                text(result.detail_line())
                    .font(type_::caption())
                    .color(t.text_secondary)
                    .truncate(),
            );
        if let Some(credit) = result.credit_line() {
            item = item.child(
                text(credit)
                    .font(type_::caption())
                    .color(t.text_secondary.alpha(0.7))
                    .truncate(),
            );
        }
        if downloading {
            item = item.child(
                text("Downloading…".to_string())
                    .font(type_::caption())
                    .color(tok::accent(0, true)),
            );
        }
        panel = panel.child(item);
    }
    if search.results.len() > 6 {
        panel = panel.child(
            text(format!(
                "{} more available; refine the language.",
                search.results.len() - 6
            ))
            .font(type_::caption())
            .color(t.text_secondary),
        );
    }
    panel
}

fn choice_row(
    label: &str,
    key: &str,
    selected: bool,
    t: &tok::Theme,
    on_press: impl Fn(&mut State) + 'static,
) -> El {
    row()
        .key(key)
        .role("button")
        .w_full()
        .h(34.0)
        .px(10.0)
        .radius(8.0)
        .items_center()
        .gap(8.0)
        .fill(if selected {
            theme::FOCUS.alpha(0.12)
        } else {
            Color::TRANSPARENT
        })
        .hover(t.hover)
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.98)
        .cursor(rune::Cursor::Pointer)
        .on_press(on_press)
        .child(
            text(if selected { "●" } else { "○" })
                .font(type_::label())
                .color(if selected {
                    tok::accent(0, true)
                } else {
                    t.text_secondary
                }),
        )
        .child(
            text(label.to_string())
                .font(type_::label())
                .color(t.text)
                .truncate(),
        )
}

fn stepper_row(
    label: &str,
    value: &str,
    down_key: &str,
    up_key: &str,
    t: &tok::Theme,
    down: impl Fn(&mut State) + 'static,
    up: impl Fn(&mut State) + 'static,
) -> El {
    row()
        .w_full()
        .h(34.0)
        .items_center()
        .gap(8.0)
        .child(
            text(label.to_string())
                .font(type_::label())
                .color(t.text_secondary),
        )
        .child(text(value.to_string()).font(type_::label()).color(t.text))
        .child(el().grow(1.0))
        .child(control("−", down_key, t, down))
        .child(control("+", up_key, t, up))
}

/// A weighted rail above the seek bar. Each chapter start gets a small tick; the weights keep
/// the ticks at their real positions without needing to know the measured width.
fn chapter_marks(session: &crate::state::InAppSession, _t: &tok::Theme) -> El {
    let duration = session.duration_ms.unwrap_or(0).max(1);
    if session.item.chapters.len() < 2 {
        return row().w_full().h(5.0);
    }
    let mut rail = row().key("in-app-chapter-marks").w_full().h(5.0);
    let mut last = 0;
    for chapter in &session.item.chapters {
        let start = chapter.start_ms().clamp(0, duration);
        let span = (start - last).max(1);
        rail = rail.child(
            el().grow(span as f32)
                .h(5.0)
                .child(el().w(2.0).h(5.0).fill(Color::WHITE.alpha(0.62))),
        );
        last = start;
    }
    rail.child(el().grow((duration - last).max(1) as f32).h(5.0))
}

/// The A–B range as a second, quieter band under the segment marks.
fn loop_marks(s: &State, session: &crate::state::InAppSession) -> Option<El> {
    let (a, b) = (s.loop_a?, s.loop_b?);
    let duration = session.duration_ms.unwrap_or(0).max(1);
    let (a, b) = (a.clamp(0, duration), b.clamp(0, duration));
    if b <= a {
        return None;
    }
    let mut rail = row().key("in-app-loop-marks").w_full().h(2.0);
    if a > 0 {
        rail = rail.child(el().grow(a as f32).h(2.0));
    }
    rail = rail
        .child(
            el().grow((b - a).max(1) as f32)
                .h(2.0)
                .radius(1.0)
                .fill(tok::accent(1, true).alpha(0.9)),
        )
        .child(el().grow((duration - b).max(1) as f32).h(2.0));
    Some(rail)
}

/// Server-published intro/recap/credits ranges as coloured bands. They sit between the chapter
/// ticks and the timeline, so a reader can see what the skip action will do before pressing it.
fn segment_marks(s: &State, session: &crate::state::InAppSession, _t: &tok::Theme) -> El {
    let duration = session.duration_ms.unwrap_or(0).max(1);
    let mut segments: Vec<&jellyfin_api::MediaSegment> = s
        .in_app_segments
        .iter()
        .filter(|segment| {
            matches!(
                segment.kind.as_str(),
                "Intro" | "Recap" | "Outro" | "Preview"
            )
        })
        .collect();
    segments.sort_by_key(|segment| segment.start_ms());
    if segments.is_empty() {
        return row().w_full().h(4.0);
    }
    let mut rail = row().key("in-app-segment-marks").w_full().h(4.0);
    let mut last = 0;
    let mut seen = std::collections::HashMap::new();
    for segment in segments {
        let start = segment.start_ms().clamp(0, duration);
        let end = segment.end_ms().clamp(start + 1, duration);
        if start < last {
            continue;
        }
        if start > last {
            rail = rail.child(el().grow((start - last) as f32).h(4.0));
        }
        let kind = segment.kind.to_ascii_lowercase();
        let count = seen.entry(kind.clone()).or_insert(0usize);
        let key = if *count == 0 {
            format!("in-app-seek-segment-{kind}")
        } else {
            format!("in-app-seek-segment-{kind}-{count}")
        };
        *count += 1;
        let color = match kind.as_str() {
            "intro" => tok::accent(0, true).alpha(0.85),
            "outro" => tok::accent(2, true).alpha(0.85),
            "recap" => Color::WHITE.alpha(0.55),
            _ => Color::WHITE.alpha(0.32),
        };
        let label = match kind.as_str() {
            "intro" => "Intro",
            "outro" => "Credits",
            "recap" => "Recap",
            _ => "Preview",
        };
        let start_at = segment.start_ms();
        rail = rail.child(
            el().key(key)
                .role("button")
                .grow((end - start).max(1) as f32)
                .h(4.0)
                .radius(2.0)
                .fill(color)
                .focusable(true)
                .focus_visual(2.0)
                .press_scale(1.0)
                .cursor(rune::Cursor::Pointer)
                .label(format!("{label} segment"))
                .help(format!(
                    "{label} · {}–{}",
                    format_time(start),
                    format_time(end)
                ))
                .on_press(move |s: &mut State| s.seek_in_app_to(start_at)),
        );
        last = end;
    }
    rail.child(el().grow((duration - last).max(1) as f32).h(4.0))
}

/// The preview above the timeline: the trickplay frame under the pointer when the server has
/// one, and a plain time card when it does not. Both use the same box, so the panel never
/// changes size under the pointer. The panel is inert: moving onto it must not move the time.
fn seek_preview(s: &State, session: &crate::state::InAppSession, _t: &tok::Theme) -> Option<El> {
    let (x, ms) = s.in_app_hover?;
    let chapter = session
        .item
        .chapter_at(ms)
        .map(|chapter| chapter.name.clone());
    let frame = s.trickplay_key(&session.item, ms).and_then(|key| {
        let image = s.images.get(&key)?;
        let (_, info) = session.item.trickplay_manifest()?;
        let tile_width = info.tile_width.max(1);
        let tile_height = info.tile_height.max(1);
        let thumb_width = info.width.max(1);
        let thumb_height = info.height.max(1);
        let thumb = info.thumbnail_at(ms);
        let (column, row) = info.cell_of(thumb);
        let scale = 240.0 / thumb_width as f32;
        let sprite_width = thumb_width as f32 * tile_width as f32 * scale;
        let sprite_height = thumb_height as f32 * tile_height as f32 * scale;
        Some(
            el().absolute(
                -(column as f32) * thumb_width as f32 * scale,
                -(row as f32) * thumb_height as f32 * scale,
            )
            .w(sprite_width)
            .h(sprite_height)
            .pixels_fit(image.pixels(), SurfaceFit::Fill),
        )
    });
    // A 300-wide panel keeps 30 pt of margin around the 240×135 frame; 150 is its half.
    let left = (x - 150.0).clamp(0.0, (s.in_app_seek_width - 300.0).max(0.0));
    let window = el()
        .absolute(30.0, 30.0)
        .w(240.0)
        .h(135.0)
        .radius(8.0)
        .clip_rounded(8.0)
        .fill(Color::BLACK)
        .outline(1.0, 0.0, Color::WHITE.alpha(0.30))
        .shadow(0.0, 12.0, Color::BLACK.alpha(0.6));
    let window = match frame {
        Some(sprite) => window.child(sprite).child(
            el().absolute(0.0, 107.0)
                .w_full()
                .h(28.0)
                .fill(Color::BLACK.alpha(0.66))
                .center()
                .child(
                    text(format_time(ms))
                        .key("in-app-trickplay-time")
                        .font(type_::mono_caption())
                        .color(Color::WHITE),
                ),
        ),
        None => window.fill(Color::WHITE.alpha(0.06)).child(
            col()
                .w_full()
                .h_full()
                .center()
                .gap(4.0)
                .child(
                    text(format_time(ms))
                        .key("in-app-trickplay-time")
                        .font(type_::title())
                        .color(Color::WHITE.alpha(0.92)),
                )
                .child(
                    text("No preview for this video")
                        .font(type_::mono_caption())
                        .color(Color::WHITE.alpha(0.55)),
                ),
        ),
    };
    Some(
        el().key("in-app-trickplay")
            .absolute(left, -186.0)
            .w(300.0)
            .h(174.0)
            .inert()
            .children(chapter.map(|name| {
                el().absolute(0.0, 0.0)
                    .w_full()
                    .h(22.0)
                    .radius(11.0)
                    .center()
                    .fill(Color::BLACK.alpha(0.78))
                    .outline(1.0, 0.0, Color::WHITE.alpha(0.14))
                    .child(
                        text(name)
                            .key("in-app-trickplay-chapter")
                            .font(type_::mono_caption())
                            .color(Color::WHITE.alpha(0.88))
                            .truncate(),
                    )
            }))
            .child(window),
    )
}

fn top_bar(title: &str, subtitle: &str, t: &tok::Theme) -> El {
    el().key("in-app-top")
        .absolute(0.0, 0.0)
        .w_full()
        .h(120.0)
        .linear_gradient(
            std::f32::consts::PI,
            [(0.0, Color::BLACK.alpha(0.78)), (1.0, Color::TRANSPARENT)],
        )
        .padding(20.0)
        .child(
            row()
                .w_full()
                .gap(14.0)
                .items_center()
                .child(
                    control("← Back", "in-app-stop", t, |s: &mut State| {
                        s.stop_in_app()
                    })
                    .fill(Color::BLACK.alpha(0.34)),
                )
                .child(
                    col()
                        .grow(1.0)
                        .min_w(0.0)
                        .gap(2.0)
                        .child(
                            text(title.to_string())
                                .font(type_::title())
                                .color(Color::WHITE)
                                .truncate(),
                        )
                        .child(
                            text(subtitle.to_string())
                                .font(type_::mono_caption())
                                .color(Color::WHITE.alpha(0.70))
                                .truncate(),
                        ),
                ),
        )
}

/// The stream's real facts, opened from the settings sheet. It is diagnostic, not decoration:
/// codec/resolution/range, the active audio and subtitle tracks, speed, position and frame count.
fn stats_panel(s: &State, session: &crate::state::InAppSession) -> El {
    let video = s
        .in_app_video
        .as_ref()
        .map(|(codec, width, height, range)| {
            format!(
                "{} · {width}×{height}{}",
                codec.to_uppercase(),
                range
                    .as_ref()
                    .filter(|range| !range.eq_ignore_ascii_case("SDR"))
                    .map(|range| format!(" · {range}"))
                    .unwrap_or_default()
            )
        })
        .unwrap_or_else(|| "—".to_string());
    let audio = s
        .in_app_audio_tracks
        .iter()
        .find(|track| track.ordinal == session.player.audio_tracks().1)
        .map(|track| track.display_title.clone())
        .unwrap_or_else(|| "—".to_string());
    let subtitles = s
        .in_app_subtitles
        .selected
        .and_then(|at| s.in_app_subtitles.tracks.get(at))
        .map(|track| {
            if s.in_app_subtitles.delay_ms == 0 {
                track.display_title.clone()
            } else {
                format!(
                    "{} · {:+.1} s",
                    track.display_title,
                    s.in_app_subtitles.delay_ms as f32 / 1000.0
                )
            }
        })
        .unwrap_or_else(|| "Off".to_string());
    let position = if let Some(duration) = session.duration_ms {
        format!(
            "{} / {}",
            format_time(session.position_ms),
            format_time(duration)
        )
    } else {
        format!("{} / LIVE", format_time(session.position_ms))
    };
    let rows = [
        ("Sink", session.player.path_label()),
        (
            "Container",
            s.in_app_container.clone().unwrap_or_else(|| "—".into()),
        ),
        ("Video", video),
        ("Audio", audio),
        ("Subtitles", subtitles),
        (
            "Speed",
            format!(
                "{}× · pitch-corrected{}",
                s.in_app_speed,
                if s.downmix_stereo { " · stereo" } else { "" }
            ),
        ),
        (
            "Audio mode",
            format!("{} · {} ms delay", s.audio_mode.label(), s.audio_delay_ms),
        ),
        (
            "Loop",
            match (s.loop_a, s.loop_b) {
                (Some(a), Some(b)) => format!("{}–{}", format_time(a), format_time(b)),
                (Some(a), None) => format!("A at {}", format_time(a)),
                _ => "Off".to_string(),
            },
        ),
        (
            "Method",
            if session.live {
                "Live TV".to_string()
            } else if !session.report {
                "Local / trailer".to_string()
            } else {
                match session.play_method {
                    "Transcode" => "Transcode".to_string(),
                    "DirectStream" => "Direct stream".to_string(),
                    _ => "Direct play".to_string(),
                }
            },
        ),
        (
            "Pipeline",
            format!(
                "{} · {}",
                session.player.path_label(),
                if session.paused { "paused" } else { "playing" }
            ),
        ),
        (
            "Colour",
            session.player.colour_facts().map_or_else(
                || "—".to_string(),
                |facts| {
                    let mut parts = vec![format!("{} · {}", facts.transfer, facts.matrix)];
                    if facts.max_luminance > 0.0 {
                        parts.push(format!(
                            "{:.0}/{:.4} nits",
                            facts.max_luminance, facts.min_luminance
                        ));
                    }
                    if facts.max_cll > 0 {
                        parts.push(format!("CLL {}", facts.max_cll));
                    }
                    parts.join(" · ")
                },
            ),
        ),
        ("Last request", format!("{} ms", crate::net::last_job_ms())),
        ("Position", position),
        ("Frames", session.player.frames().to_string()),
        (
            "Cache",
            format!(
                "{} images · {} queued",
                s.images.len(),
                s.requested_images.len()
            ),
        ),
        ("Queue", format!("{} of {}", s.queue_at + 1, s.queue.len())),
        ("Server", s.config.server_url.clone()),
        (
            "Last error",
            s.playback_error
                .clone()
                .or_else(|| s.detail_notice.clone())
                .unwrap_or_else(|| "—".into()),
        ),
    ];
    let mut panel = col()
        .key("in-app-stats")
        .w(340.0)
        .padding(14.0)
        .gap(6.0)
        .radius(12.0)
        .fill(Color::BLACK.alpha(0.82))
        .outline(1.0, 0.0, Color::WHITE.alpha(0.14))
        .shadow(0.0, 14.0, Color::BLACK.alpha(0.45));
    for (label, value) in rows {
        panel = panel.child(
            row()
                .w_full()
                .gap(10.0)
                .child(
                    text(label)
                        .w(76.0)
                        .font(type_::mono_caption())
                        .color(Color::WHITE.alpha(0.55)),
                )
                .child(
                    text(value)
                        .font(type_::mono_caption())
                        .color(Color::WHITE.alpha(0.92))
                        .truncate(),
                ),
        );
    }
    col()
        .key("in-app-stats-layer")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .items_end()
        .pt(76.0)
        .pr(20.0)
        .child(panel)
}

/// The LIVE marker and what is on. It replaces the timeline for a tuner stream, which has no
/// duration and no published trickplay.
fn live_status(s: &State, session: &crate::state::InAppSession, t: &tok::Theme) -> El {
    let channel = s
        .live_channel_index()
        .map(|at| format!("CH {} · {}", at + 1, session.item.name))
        .unwrap_or_else(|| session.item.name.clone());
    row()
        .key("in-app-live")
        .w_full()
        .h(24.0)
        .items_center()
        .gap(8.0)
        .child(
            el().w(8.0)
                .h(8.0)
                .radius(4.0)
                .fill(tok::accent(2, true))
                .shadow(0.0, 6.0, tok::accent(2, true).alpha(0.55)),
        )
        .child(
            text("LIVE")
                .font(type_::label())
                .color(tok::accent(2, true)),
        )
        .child(
            text(channel)
                .key("in-app-live-channel")
                .font(type_::mono_caption())
                .color(Color::WHITE.alpha(0.72))
                .truncate(),
        )
        .child(
            text(
                session
                    .item
                    .current_program
                    .as_ref()
                    .map(|program| program.name.clone())
                    .unwrap_or_default(),
            )
            .font(type_::mono_caption())
            .color(Color::WHITE.alpha(0.52))
            .truncate(),
        )
        .children(s.live.next.get(&session.item.id).map(|program| {
            text(format!("Next: {}", program.name))
                .key("in-app-live-next-program")
                .font(type_::mono_caption())
                .color(Color::WHITE.alpha(0.40))
                .truncate()
        }))
        .child(el().grow(1.0))
        .child(icon_control(
            &theme::icon::SKIP_PREV,
            "Previous channel",
            "in-app-live-prev",
            t,
            |s: &mut State| s.zap_live(-1),
        ))
        .child(icon_control(
            &theme::icon::SKIP_NEXT,
            "Next channel",
            "in-app-live-next",
            t,
            |s: &mut State| s.zap_live(1),
        ))
}

fn volume_slider(s: &State, t: &tok::Theme) -> El {
    let fraction = s.in_app_volume.clamp(0.0, 1.0) as f32;
    let width = 86.0;
    // A TV has no pointer to reveal the slider: the focused state is the knob itself. A wash
    // over the whole 86×24 box read as a grey pill stuck to the bar, so big screen keeps the
    // hover wash off and grows the knob instead.
    let focused =
        theme::is_big_screen() && s.focused_key.as_deref() == Some("in-app-volume-slider");
    let knob = if focused { 14.0 } else { 10.0 };
    let track = if focused { 5.0 } else { 4.0 };
    let knob_x = (width * fraction - knob / 2.0).clamp(0.0, width - knob);
    let mut slider = el()
        .key("in-app-volume-slider")
        .role("slider")
        .label(format!("Volume {}%", (s.in_app_volume * 100.0).round()))
        .range_value(s.in_app_volume, 0.0, 1.0)
        .range_step(0.05)
        .w(width)
        .h(24.0)
        .justify_center()
        // `el()` lays its children out as a row, so `justify_center` only centres horizontally.
        // Without `items_center` the 4 pt track sat at the top of the 24 pt box while the knob
        // stayed at the vertical centre, and the slider read as tilted.
        .items_center()
        .focusable(true)
        .hover(t.hover)
        .cursor(rune::Cursor::Grab)
        .on_drag(|s: &mut State, drag: &mut Drag, _tx| {
            if matches!(drag.phase, Phase::Down | Phase::Move) && drag.w > 0.0 && !drag.cancelled()
            {
                let fraction = (drag.x / drag.w).clamp(0.0, 1.0);
                s.set_in_app_volume(f64::from(fraction));
            }
        })
        .child(
            row()
                .w_full()
                .h(track)
                .radius(2.5)
                .items_center()
                .child(
                    el().w(width * fraction)
                        .h(track)
                        .radius(2.5)
                        .fill(tok::accent(0, true)),
                )
                .child(
                    el().grow(1.0)
                        .h(track)
                        .radius(2.5)
                        .fill(Color::WHITE.alpha(0.24)),
                ),
        )
        .child(
            el().absolute(knob_x, (24.0 - knob) / 2.0)
                .w(knob)
                .h(knob)
                .radius(knob / 2.0)
                .fill(Color::WHITE)
                .outline(if focused { 2.0 } else { 0.0 }, 0.0, tok::accent(0, true))
                .shadow(
                    0.0,
                    if focused { 6.0 } else { 2.0 },
                    if focused {
                        tok::accent(0, true).alpha(0.55)
                    } else {
                        Color::BLACK.alpha(0.45)
                    },
                ),
        )
        .help("Volume");
    slider = if theme::is_big_screen() {
        slider.no_focus_ring()
    } else {
        slider.focus_ring(theme::focus_width(), 2.0, theme::ring())
    };
    slider
}

fn seek_bar(s: &State, session: &crate::state::InAppSession, progress: f32, t: &tok::Theme) -> El {
    let big = theme::is_big_screen();
    let hovering = s.in_app_hover.is_some() || big;
    // Focus on a TV is shown by the bar itself: a taller rule and a ringed knob. A translucent
    // wash across the whole width read as a grey band over the picture.
    let focused = big && s.focused_key.as_deref() == Some("in-app-seek");
    // Resting, the bar is a quiet 4 pt rule. Under the pointer, or on a TV with no pointer,
    // it stays thick and keeps the knob visible.
    let track = if focused {
        10.0
    } else if big || hovering {
        8.0
    } else {
        4.0
    };
    let knob = if focused {
        20.0
    } else if big {
        16.0
    } else {
        14.0
    };
    let knob_x = (s.in_app_seek_width * progress - knob / 2.0)
        .clamp(0.0, (s.in_app_seek_width - knob).max(0.0));
    let knob_y = 24.0 - track / 2.0 - knob / 2.0;
    let duration_ms = session.duration_ms.unwrap_or(0).max(0);
    let position_ms = session
        .scrub_ms
        .unwrap_or(session.position_ms)
        .clamp(0, duration_ms.max(0));
    let bar = el()
        .key("in-app-seek")
        .role("slider")
        .label("Seek")
        .a11y_value(format!(
            "{} of {}",
            format_time(position_ms),
            format_time(duration_ms)
        ))
        .range_value(
            position_ms as f64 / 1000.0,
            0.0,
            duration_ms as f64 / 1000.0,
        )
        .range_step(10.0)
        .w_full()
        .h(24.0)
        .focusable(true)
        .cursor(rune::Cursor::Grab)
        .on_layout_tx(|s: &mut State, rect: [f32; 4], tx: &mut rune::Tx| {
            // The knob is placed from the last measured width. Without a settle the first
            // OSD frame drew it at zero, at the left edge.
            if (s.in_app_seek_width - rect[2]).abs() > 0.5 {
                s.in_app_seek_width = rect[2];
                tx.settle();
            }
        })
        .on_pointer_move_local(|s: &mut State, event: rune::PointerEvent, _tx| {
            s.hover_in_app_seek(event.x);
        })
        .on_hover_change(|s: &mut State, inside: bool, _tx| {
            if !inside {
                s.leave_in_app_hover();
            }
        })
        .on_drag(|s: &mut State, drag: &mut Drag, _tx| {
            let fraction = if drag.w > 0.0 {
                (drag.x / drag.w).clamp(0.0, 1.0)
            } else {
                0.0
            };
            match drag.phase {
                Phase::Down | Phase::Move => {
                    s.hover_in_app_seek(drag.x);
                    s.preview_in_app_scrub(fraction);
                }
                Phase::Up => {
                    s.clear_in_app_hover();
                    s.finish_in_app_scrub(fraction, drag.cancelled());
                }
            }
        })
        .child(
            col()
                .w_full()
                .h(24.0)
                .justify_end()
                .gap(2.0)
                .child(chapter_marks(session, t))
                .child(segment_marks(s, session, t))
                .children(loop_marks(s, session))
                .child(
                    row()
                        .key("in-app-seek-track")
                        .w_full()
                        .h(track)
                        .radius(track / 2.0)
                        .items_center()
                        .child(
                            el().grow(progress.max(0.001))
                                .h(track)
                                .radius(track / 2.0)
                                .fill(tok::accent(0, true)),
                        )
                        .child(
                            el().grow((1.0 - progress).max(0.001))
                                .h(track)
                                .radius(track / 2.0)
                                .fill(Color::WHITE.alpha(if focused {
                                    0.62
                                } else if hovering {
                                    0.42
                                } else {
                                    0.24
                                })),
                        ),
                ),
        )
        .children(hovering.then(|| {
            el().key("in-app-seek-knob")
                .absolute(knob_x, knob_y)
                .w(knob)
                .h(knob)
                .radius(knob / 2.0)
                .fill(Color::WHITE)
                .outline(if focused { 2.5 } else { 0.0 }, 0.0, tok::accent(0, true))
                .shadow(
                    0.0,
                    if focused { 8.0 } else { 3.0 },
                    if focused {
                        tok::accent(0, true).alpha(0.55)
                    } else {
                        Color::BLACK.alpha(0.55)
                    },
                )
        }))
        .children(seek_preview(s, session, t));
    if big {
        bar.no_focus_ring()
    } else {
        bar.focus_ring(theme::focus_width(), 12.0, theme::ring())
    }
}

/// The clock, the run's position and the three controls that belong to the picture rather than
/// to a menu: volume, mute and the settings sheet.
fn timeline_row(
    s: &State,
    session: &crate::state::InAppSession,
    display_ms: i64,
    duration: i64,
    t: &tok::Theme,
) -> El {
    let live = session.live;
    let mute_label = if session.player.is_muted() {
        "Unmute"
    } else {
        "Mute"
    };
    let queue = (s.queue.len() > 1).then(|| {
        control(
            &format!("{} of {}", s.queue_at + 1, s.queue.len()),
            "in-app-queue",
            t,
            |s: &mut State| s.toggle_in_app_queue(),
        )
        .help("This run's items")
    });
    row()
        .key("in-app-timeline-row")
        .w_full()
        .items_center()
        .gap(8.0)
        .child(
            text(format_time(display_ms))
                .key("in-app-elapsed")
                .font(type_::mono_caption())
                .color(Color::WHITE),
        )
        .children((!live).then(|| {
            text("·")
                .font(type_::mono_caption())
                .color(Color::WHITE.alpha(0.38))
        }))
        .children((!live).then(|| {
            text(format!("-{}", format_time((duration - display_ms).max(0))))
                .key("in-app-remaining")
                .font(type_::mono_caption())
                .color(Color::WHITE.alpha(0.64))
        }))
        .children(queue)
        .children((s.in_app_subtitles.delay_ms != 0).then(|| {
            text(format!(
                "{:+.1} s",
                s.in_app_subtitles.delay_ms as f32 / 1000.0
            ))
            .key("in-app-subtitle-delay")
            .font(type_::mono_caption())
            .color(Color::WHITE.alpha(0.72))
        }))
        .child(el().grow(1.0))
        .child(volume_slider(s, t))
        .child(icon_control(
            if session.player.is_muted() {
                &theme::icon::MUTE
            } else {
                &theme::icon::VOLUME_HIGH
            },
            mute_label,
            "in-app-mute",
            t,
            |s: &mut State| s.toggle_in_app_mute(),
        ))
        .child(icon_control(
            if s.fullscreen {
                &theme::icon::COLLAPSE
            } else {
                &theme::icon::EXPAND
            },
            if s.fullscreen {
                "Windowed"
            } else {
                "Fullscreen"
            },
            "in-app-fullscreen",
            t,
            |s: &mut State| s.toggle_fullscreen(),
        ))
        .child(icon_control(
            &theme::icon::TUNE,
            "Player settings",
            "in-app-gear",
            t,
            |s: &mut State| s.toggle_in_app_gear(),
        ))
}

/// The skip offer, on its own line above the timeline. It appears five seconds before a segment
/// and leaves with it, so the transport never crowds and the volume is never covered.
fn skip_pill(s: &State, t: &tok::Theme) -> Option<El> {
    if s.skip_mode != crate::config::SkipMode::Ask {
        return None;
    }
    let segment = s.offered_in_app_segment()?;
    let label = match segment.kind.as_str() {
        "Intro" => "Skip intro",
        "Outro" => "Skip credits",
        "Recap" => "Skip recap",
        _ => "Skip preview",
    };
    let end = segment.end_ms();
    Some(row().w_full().justify_end().child(accent_control(
        label,
        "in-app-skip-segment",
        t,
        move |s: &mut State| s.skip_in_app_segment(end),
    )))
}

/// What a seek button does, in the reader's configured step. The label and the action read the
/// same value, so a 30-second step can never be announced as ten.
fn seek_label(step_ms: i64, forward: bool) -> String {
    let step = step_ms / 1000;
    if forward {
        format!("Forward {step} seconds")
    } else {
        format!("Back {step} seconds")
    }
}

/// The three things a viewer reaches for without opening anything, centred on the picture.
fn centre_cluster(s: &State, session: &crate::state::InAppSession, t: &tok::Theme) -> El {
    let live = session.live;
    let primary_glyph = if session.paused {
        &theme::icon::PLAY
    } else {
        &theme::icon::PAUSE
    };
    let primary_label = if session.paused { "Play" } else { "Pause" };
    row()
        .key("in-app-transport")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .justify_center()
        .items_center()
        .gap(16.0)
        .children((!live).then(|| {
            icon_control(
                &theme::icon::SEEK_BACK,
                &seek_label(s.seek_step_ms, false),
                "in-app-back-10",
                t,
                |s: &mut State| s.seek_in_app_step(false),
            )
        }))
        .child(transport_primary(primary_glyph, primary_label, t))
        .children((!live).then(|| {
            icon_control(
                &theme::icon::SEEK_FORWARD,
                &seek_label(s.seek_step_ms, true),
                "in-app-forward-10",
                t,
                |s: &mut State| s.seek_in_app_step(true),
            )
        }))
}

fn bottom_osd(
    s: &State,
    session: &crate::state::InAppSession,
    display_ms: i64,
    duration: i64,
    progress: f32,
    t: &tok::Theme,
) -> El {
    let live = session.live;
    col()
        .key("in-app-controls")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .justify_end()
        .linear_gradient(
            std::f32::consts::PI,
            [
                (0.0, Color::TRANSPARENT),
                (0.46, Color::TRANSPARENT),
                (1.0, Color::BLACK.alpha(0.94)),
            ],
        )
        .padding(24.0)
        .gap(10.0)
        .children((!live).then(|| skip_pill(s, t)).flatten())
        .children((!live).then(|| seek_bar(s, session, progress, t)))
        .children(live.then(|| live_status(s, session, t)))
        .child(timeline_row(s, session, display_ms, duration, t))
}

fn gear_heading(label: &str, t: &tok::Theme) -> El {
    text(label)
        .font(type_::caption())
        .color(t.text_secondary)
        .w_full()
        .padding(2.0)
}

fn gear_row(
    label: &str,
    value: &str,
    key: &str,
    t: &tok::Theme,
    on: impl Fn(&mut State) + 'static,
) -> El {
    row()
        .key(key)
        .role("button")
        .w(168.0)
        .h(27.0)
        .px(7.0)
        .radius(6.0)
        .items_center()
        .gap(8.0)
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_visual(2.0)
        .cursor(rune::Cursor::Pointer)
        .press_scale(0.99)
        .on_press(on)
        .child(
            text(label)
                .font(type_::label())
                .color(t.text)
                .grow(1.0)
                .min_w(0.0)
                .truncate(),
        )
        .child(
            text(value.to_string())
                .font(type_::caption())
                .color(t.text_secondary)
                .max_w(76.0)
                .truncate(),
        )
}

/// The settings sheet. Every control that left the bar lives here, grouped, with its current
/// value on the right so the state stays readable without opening anything.
fn gear_panel(s: &State, t: &tok::Theme) -> El {
    let Some(session) = s.in_app.as_ref() else {
        return col().key("in-app-gear-menu");
    };
    let audio_count = s.in_app_audio_tracks.len();
    let audio_current = session.player.audio_tracks().1;
    let audio_value = if audio_count > 1 {
        let at = usize::try_from(audio_current)
            .unwrap_or(0)
            .min(audio_count - 1);
        format!("{}/{}", at + 1, audio_count)
    } else {
        format!("{}/{}", audio_current + 1, audio_count.max(1))
    };
    let text_count = s
        .in_app_subtitles
        .tracks
        .iter()
        .filter(|track| track.is_text)
        .count();
    let subs_value = if s.in_app_subtitles.image.is_some() {
        "Image".to_string()
    } else if s.in_app_subtitles.image_loading {
        "Preparing…".to_string()
    } else if s.in_app_subtitles.image_error.is_some() {
        "Failed".to_string()
    } else {
        match s.in_app_subtitles.selected {
            None => "Off".to_string(),
            Some(at) => {
                let ordinal = s
                    .in_app_subtitles
                    .tracks
                    .iter()
                    .take(at + 1)
                    .filter(|track| track.is_text)
                    .count();
                format!("{}/{}", ordinal.max(1), text_count.max(1))
            }
        }
    };
    let speed = format!("{}×", s.in_app_speed);
    let fit = if s.in_app_fill { "Fill" } else { "Fit" };
    let sleep = if s.sleep_end_of_episode {
        "Episode".to_string()
    } else if let Some(deadline) = s.sleep_deadline {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        format!("{:02}:{:02}", left.as_secs() / 60, left.as_secs() % 60)
    } else {
        "Off".to_string()
    };
    let loop_label = match (s.loop_a, s.loop_b) {
        (None, _) => "Off",
        (Some(_), None) => "A set",
        (Some(_), Some(_)) => "On",
    };
    // The full path (with P010/PQ) is in the statistics overlay; the row keeps the short form
    // so it cannot push past the sheet's edge.
    let sink = if session.player.path_label().starts_with("DMA-BUF") {
        "DMA-BUF".to_string()
    } else {
        "CPU".to_string()
    };

    let rows = row()
        .w_full()
        .flex_wrap()
        .focus_group()
        .gap(3.0)
        .child(gear_heading("Tracks", t))
        .child(gear_row(
            "Subtitles",
            &subs_value,
            "in-app-subtitles",
            t,
            |s: &mut State| s.toggle_in_app_subtitles_menu(),
        ))
        .child(gear_row("Audio", &audio_value, "in-app-audio", t, |s| {
            s.toggle_in_app_audio_menu()
        }))
        .child(gear_heading("Playback", t))
        .child(gear_row(
            "Quality",
            s.playback_quality.label(),
            "in-app-quality",
            t,
            |s: &mut State| s.toggle_quality_menu(),
        ))
        .child(gear_row("Speed", &speed, "in-app-speed", t, |s| {
            s.shift_in_app_speed(true)
        }))
        .child(gear_row("Fit / Fill", fit, "in-app-fit", t, |s| {
            s.toggle_in_app_fit()
        }))
        .child(gear_heading("Timers", t))
        .child(gear_row("Sleep", &sleep, "in-app-sleep", t, |s| {
            s.toggle_sleep_menu()
        }))
        .child(gear_row("A–B loop", loop_label, "in-app-loop", t, |s| {
            s.cycle_loop()
        }))
        .child(gear_heading("Navigate", t))
        .children((session.item.chapters.len() >= 2).then(|| {
            gear_row("Chapters", "", "in-app-chapters", t, |s| {
                s.toggle_in_app_chapters()
            })
        }))
        .child(gear_row("Go to time", "", "in-app-goto", t, |s| {
            s.toggle_in_app_goto()
        }))
        .children(
            (s.queue_previous_available() || s.in_app_prev.is_some()).then(|| {
                gear_row("Previous episode", "P", "in-app-prev-episode", t, |s| {
                    s.play_in_app_previous()
                })
            }),
        )
        .children(
            (s.queue_next_available() || s.in_app_next.is_some()).then(|| {
                gear_row("Next episode", "N", "in-app-next-episode", t, |s| {
                    s.play_in_app_next_episode()
                })
            }),
        )
        .child(gear_heading("This device", t))
        .child(gear_row("Continue on…", "", "in-app-transfer", t, |s| {
            s.open_in_app_transfer()
        }))
        .child(gear_row(
            "Statistics",
            &sink,
            "in-app-stats-toggle",
            t,
            |s| {
                s.close_player_gear();
                s.toggle_in_app_stats();
            },
        ))
        .child(gear_row("Minimize", "", "in-app-minimize", t, |s| {
            s.close_player_gear();
            s.minimize_in_app();
        }));

    let menu = col()
        .key("in-app-gear-menu")
        .transition(rune::Transition::offset(0.0, 8.0).combined(rune::Transition::opacity()))
        .w(368.0)
        .padding(10.0)
        .gap(6.0)
        .radius(12.0)
        .child(
            text("Player")
                .font(type_::section())
                .color(t.text)
                .padding(2.0),
        )
        .child(rows)
        .child(
            text(if s.big_screen {
                "A pause · ←/→ seek · B close"
            } else {
                "Space pause · ←/→ seek · Esc close"
            })
            .key("in-app-gear-hint")
            .font(type_::caption())
            .color(t.text_secondary)
            .padding(4.0),
        );

    el().key("in-app-gear-layer")
        .role("button")
        .label("Close player settings")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .justify_end()
        .items_end()
        .pb(96.0)
        .pr(24.0)
        .on_press(|s: &mut State| s.close_player_gear())
        .child(theme::elevation::frosted(menu, t))
}

fn transport_primary(glyph: &'static Icon, label: &str, t: &tok::Theme) -> El {
    el().key("in-app-pause")
        .role("button")
        .w(48.0)
        .h(48.0)
        .radius(24.0)
        .center()
        .fill(tok::accent(0, true))
        .shadow(0.0, 12.0, tok::accent(0, true).alpha(0.35))
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_visual(2.0)
        .press_scale(0.94)
        .label(label)
        .help(label)
        .child(icon(glyph, 18.0).color(t.bg))
        .cursor(rune::Cursor::Pointer)
        .on_press(|s: &mut State| s.toggle_in_app_pause())
}

fn icon_control(
    glyph: &'static Icon,
    label: &str,
    key: &str,
    t: &tok::Theme,
    on: impl Fn(&mut State) + 'static,
) -> El {
    theme::tv_focus(
        el().key(key)
            .role("button")
            .w(40.0)
            .h(40.0)
            .radius(10.0)
            .center()
            .fill(Color::BLACK.alpha(0.32))
            .outline(1.0, 0.0, Color::WHITE.alpha(0.14))
            .hover(t.hover)
            .motion(motion::SNAPPY)
            .focusable(true)
            .focus_visual(2.0)
            .press_scale(0.94)
            .label(label)
            .help(label)
            .child(icon(glyph, 17.0).color(Color::WHITE.alpha(0.92)))
            .cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut State| on(s)),
    )
}

fn control(label: &str, key: &str, t: &tok::Theme, on: impl Fn(&mut State) + 'static) -> El {
    theme::tv_focus(
        el().key(key)
            .role("button")
            .h(36.0)
            .px(12.0)
            .radius(9.0)
            .center()
            .fill(Color::BLACK.alpha(0.34))
            .gradient(Color::WHITE.alpha(0.07), Color::TRANSPARENT)
            .outline(1.0, 0.0, Color::WHITE.alpha(0.14))
            .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
            .hover(t.hover)
            .motion(motion::SNAPPY)
            .focusable(true)
            .focus_visual(2.0)
            .press_scale(0.96)
            .child(text(label).font(type_::label()).color(Color::WHITE))
            .cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut State| on(s)),
    )
}

#[cfg(test)]
mod tests {
    use super::seek_label;

    #[test]
    fn the_seek_labels_follow_the_configured_step() {
        assert_eq!(seek_label(10_000, false), "Back 10 seconds");
        assert_eq!(seek_label(30_000, true), "Forward 30 seconds");
    }
}
