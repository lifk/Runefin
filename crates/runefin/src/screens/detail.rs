//! The full-bleed detail page: backdrop, poster, one primary action, overview, season chips
//! and episode rows. The poster is a shared element with the card that opened it.

use jellyfin_api::Item;
use rune::tok::{self, motion};

use crate::theme::{self, type_};
use rune::{Color, El, ScrollSpec, SurfaceFit, col, el, row, text, text_input};

use crate::detail::{self, DetailState};
use crate::state::State;

use super::{hero_style, placeholder, subtitle_style};

const DETAIL_POSTER_W: f32 = 240.0;
const DETAIL_POSTER_H: f32 = 360.0;

pub(super) fn detail_screen(s: &State, detail: &DetailState) -> El {
    let t = theme::colors();
    let page = el()
        .key("detail-screen")
        .w_full()
        .h_full()
        .fill(t.bg)
        .child(backdrop(s, detail, &t))
        .child(
            el().absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .gradient(Color::TRANSPARENT, t.bg),
        )
        .child(el().absolute(0.0, 0.0).w_full().h_full().linear_gradient(
            std::f32::consts::FRAC_PI_2,
            [
                (0.0, t.bg.alpha(0.92)),
                (0.38, t.bg.alpha(0.78)),
                (0.62, t.bg.alpha(0.28)),
                (1.0, Color::TRANSPARENT),
            ],
        ))
        // A bright backdrop can sit behind the title even when the horizontal scrim is on the
        // wrong side of the split; keep the top third readable without hiding the artwork.
        .child(el().absolute(0.0, 0.0).w_full().h(360.0).linear_gradient(
            std::f32::consts::PI,
            [
                (0.0, t.bg.alpha(0.72)),
                (0.62, t.bg.alpha(0.24)),
                (1.0, Color::TRANSPARENT),
            ],
        ));

    let mut content = col()
        .key("detail-content")
        .w_full()
        .h_full()
        .padding(40.0)
        .gap(18.0)
        .child(
            el().key("detail-back")
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
                .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                .press_scale(0.97)
                .cursor(rune::Cursor::Pointer)
                .child(text("← Back").font(type_::label()).color(t.text))
                .on_press(|s: &mut State| s.back_from_detail()),
        );

    if detail.loading && detail.item.name.is_empty() {
        content = content.child(detail_skeleton(&t));
    } else if let Some(message) = &detail.error {
        content = content.child(detail_error(message, &t));
    } else {
        content = content.child(
            col()
                .key("detail-scroll")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .pb(if s.in_app_minimized { 124.0 } else { 0.0 })
                .gap(24.0)
                .child(hero(s, detail, &t))
                .children(next_up(s, detail, &t))
                .children(if detail.item.kind == "BoxSet" {
                    collection(s, detail, &t)
                } else {
                    series(s, detail, &t)
                })
                .children(collections_row(s, &t))
                .children(cast(s, detail, &t))
                .children(extras(s, &t))
                .children(similar(s, detail, &t)),
        );
    }
    let page = page.child(content);
    if s.preplay_open {
        page.child(preplay_sheet(s, detail, &t))
    } else {
        page
    }
}

/// Naming a new playlist. The item on this page is its first entry; typing and Enter both work,
/// and the field takes focus as soon as the modal opens.
/// What Play is about to do, before it does it. Every line is server data: version, container,
/// codecs, HDR, subtitle tracks and size. The decision is Direct play because this build asks
/// the server for the static stream; Phase 2 adds the transcode line.
fn preplay_sheet(s: &State, detail: &DetailState, t: &tok::Theme) -> El {
    let sources = &detail.item.media_sources;
    let selected = s
        .in_app_source
        .as_deref()
        .and_then(|id| sources.iter().find(|source| source.id == id))
        .or_else(|| sources.first());
    let mut panel = col()
        .key("preplay-sheet")
        .w(460.0)
        .padding(18.0)
        .gap(10.0)
        .radius(14.0)
        .child(
            text(format!("Play {}", detail.item.name))
                .font(type_::section())
                .color(t.text)
                .truncate(),
        );
    if sources.len() > 1 {
        let mut chips = row().w_full().gap(8.0).flex_wrap();
        for (at, source) in sources.iter().enumerate() {
            let on = selected.is_some_and(|selected| selected.id == source.id);
            let id = source.id.clone();
            chips = chips.child(
                el().key(format!("preplay-version-{}", source.id))
                    .role("button")
                    .h(28.0)
                    .px(10.0)
                    .radius(7.0)
                    .center()
                    .fill(if on {
                        tok::accent(0, true).alpha(0.24)
                    } else {
                        t.bg
                    })
                    .outline(
                        1.0,
                        0.0,
                        if on {
                            tok::accent(0, true)
                        } else {
                            t.card_border
                        },
                    )
                    .hover(t.hover)
                    .focusable(true)
                    .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                    .press_scale(0.97)
                    .cursor(rune::Cursor::Pointer)
                    .child(
                        text(
                            source
                                .name
                                .clone()
                                .unwrap_or_else(|| format!("Version {}", at + 1)),
                        )
                        .font(type_::caption())
                        .color(t.text),
                    )
                    .on_press(move |s: &mut State| s.set_preplay_source(id.clone())),
            );
        }
        panel = panel.child(chips);
    }
    if let Some(source) = selected {
        let video = source
            .media_streams
            .iter()
            .find(|stream| stream.kind == "Video");
        let audio = source
            .media_streams
            .iter()
            .find(|stream| stream.kind == "Audio");
        let subtitles = source
            .media_streams
            .iter()
            .filter(|stream| stream.kind == "Subtitle")
            .count();
        let video_label = video.map_or_else(
            || "—".to_string(),
            |video| {
                format!(
                    "{} · {}{}",
                    video.codec.as_deref().unwrap_or("?").to_uppercase(),
                    video
                        .height
                        .map_or_else(|| "?".to_string(), |height| format!("{height}p")),
                    video
                        .video_range
                        .as_deref()
                        .filter(|range| !range.eq_ignore_ascii_case("SDR"))
                        .map(|range| format!(" · {range}"))
                        .unwrap_or_default()
                )
            },
        );
        let audio_label = audio.map_or_else(
            || "—".to_string(),
            |audio| {
                format!(
                    "{} · {}{}",
                    audio.codec.as_deref().unwrap_or("?").to_uppercase(),
                    audio
                        .channels
                        .map_or_else(String::new, |channels| format!("{channels} ch · ")),
                    audio.language.as_deref().unwrap_or("")
                )
            },
        );
        let size = source.size.map_or_else(
            || "—".to_string(),
            |bytes| format!("{:.1} GB", bytes as f64 / 1_000_000_000.0),
        );
        let capped = s
            .playback_quality
            .max_bitrate()
            .is_some_and(|cap| source.bitrate.is_some_and(|bitrate| bitrate > cap));
        let decision = if capped {
            format!("Transcode · {}", s.playback_quality.label())
        } else {
            "Direct play".to_string()
        };
        let reason = if capped {
            "The chosen quality is below the file's bitrate."
        } else {
            "The file matches this device's profile; no server work."
        };
        let rows = [
            (
                "Container",
                source.container.clone().unwrap_or_else(|| "—".into()),
            ),
            ("Video", video_label),
            ("Audio", audio_label),
            ("Subtitles", format!("{subtitles} track(s)")),
            ("Size", size),
            ("Decision", decision),
            ("Why", reason.to_string()),
        ];
        for (label, value) in rows {
            panel = panel.child(
                row()
                    .w_full()
                    .gap(12.0)
                    .child(
                        text(label)
                            .font(type_::caption())
                            .color(t.text_secondary)
                            .w(100.0),
                    )
                    .child(
                        text(value)
                            .key(format!("preplay-{}", label.to_lowercase()))
                            .font(type_::label())
                            .color(t.text)
                            .grow(1.0)
                            .min_w(0.0)
                            .wrap(),
                    ),
            );
        }
    }
    panel = panel.child(
        row()
            .w_full()
            .gap(8.0)
            .items_center()
            .child(
                text(format!("Quality: {}", s.playback_quality.label()))
                    .key("preplay-quality")
                    .role("button")
                    .font(type_::caption())
                    .color(t.text_secondary)
                    .grow(1.0)
                    .cursor(rune::Cursor::Pointer)
                    .on_press(|s: &mut State| s.cycle_playback_quality()),
            )
            .child(
                el().key("preplay-cancel")
                    .role("button")
                    .h(34.0)
                    .px(14.0)
                    .radius(8.0)
                    .center()
                    .fill(t.bg)
                    .hover(t.hover)
                    .focusable(true)
                    .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                    .press_scale(0.97)
                    .cursor(rune::Cursor::Pointer)
                    .child(text("Cancel").font(type_::label()).color(t.text))
                    .on_press(|s: &mut State| s.close_preplay()),
            )
            .child(
                el().key("preplay-play")
                    .role("button")
                    .h(34.0)
                    .px(16.0)
                    .radius(8.0)
                    .center()
                    .fill(tok::accent(0, true))
                    .hover(t.hover)
                    .focusable(true)
                    .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                    .press_scale(0.97)
                    .cursor(rune::Cursor::Pointer)
                    .child(text("Play").font(type_::label()).color(t.bg))
                    .on_press(|s: &mut State| s.play_detail()),
            ),
    );
    el().key("preplay-layer")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .justify_center()
        .items_center()
        .fill(Color::BLACK.alpha(0.48))
        .on_press(|s: &mut State| s.close_preplay())
        .child(theme::elevation::frosted(panel, t))
}

pub(super) fn playlist_create_modal(s: &State, item: &Item, t: &tok::Theme) -> El {
    let input = text_input(&s.playlist_new_name, type_::body())
        .key("playlist-name-input")
        .label("Playlist name")
        .placeholder("New playlist", t.text_secondary.alpha(0.55))
        .role("textbox")
        .single_line()
        .w_full()
        .on_edit(|s: &mut State| &mut s.playlist_new_name)
        .on_submit(|s: &mut State, _tx| s.create_playlist_with_detail());
    let panel = col()
        .key("playlist-create")
        .w(400.0)
        .padding(18.0)
        .gap(10.0)
        .radius(14.0)
        .fill(t.card)
        .outline(1.0, 0.0, t.card_border)
        .shadow(0.0, 18.0, Color::BLACK.alpha(0.5))
        .child(text("New playlist").font(type_::section()).color(t.text))
        .child(
            text(format!("Starts with {}", item.name))
                .font(type_::meta())
                .color(t.text_secondary)
                .truncate(),
        )
        .child(input)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .child(el().grow(1.0))
                .child(
                    el().key("playlist-create-cancel")
                        .role("button")
                        .h(34.0)
                        .px(14.0)
                        .radius(8.0)
                        .center()
                        .fill(t.bg)
                        .hover(t.hover)
                        .motion(motion::SNAPPY)
                        .focusable(true)
                        .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                        .press_scale(0.97)
                        .child(text("Cancel").font(type_::label()).color(t.text))
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| s.close_playlist_create()),
                )
                .child(
                    el().key("playlist-create-submit")
                        .role("button")
                        .h(34.0)
                        .px(16.0)
                        .radius(8.0)
                        .center()
                        .fill(tok::accent(0, true))
                        .hover(t.hover)
                        .motion(motion::SNAPPY)
                        .focusable(true)
                        .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                        .press_scale(0.97)
                        .child(text("Create").font(type_::label()).color(t.bg))
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| s.create_playlist_with_detail()),
                ),
        );
    el().key("playlist-create-layer")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .justify_center()
        .items_center()
        .fill(Color::BLACK.alpha(0.42))
        .on_press(|s: &mut State| s.close_playlist_create())
        .child(panel)
}

/// The "Add to playlist" picker. It lists what the sidebar already loaded, so opening it never
/// waits on the network; only the add itself is a request.
pub(super) fn playlist_picker(s: &State, item: &Item, t: &tok::Theme) -> El {
    let mut panel = col()
        .key("playlist-picker")
        .w(360.0)
        .padding(16.0)
        .gap(6.0)
        .radius(14.0)
        .fill(t.card)
        .outline(1.0, 0.0, t.card_border)
        .shadow(0.0, 18.0, Color::BLACK.alpha(0.5))
        .child(
            row()
                .w_full()
                .items_center()
                .gap(8.0)
                .child(
                    text("Add to a playlist")
                        .font(type_::section())
                        .color(t.text)
                        .grow(1.0),
                )
                .child(
                    el().key("playlist-picker-close")
                        .role("button")
                        .h(26.0)
                        .px(8.0)
                        .radius(13.0)
                        .center()
                        .fill(Color::TRANSPARENT)
                        .hover(t.hover)
                        .motion(motion::SNAPPY)
                        .focusable(true)
                        .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                        .press_scale(0.96)
                        .child(text("Esc").font(type_::caption()).color(t.text_secondary))
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| s.close_playlist_picker()),
                ),
        )
        .child(
            text(item.name.clone())
                .font(type_::meta())
                .color(t.text_secondary)
                .truncate(),
        );
    if s.playlists.is_empty() {
        panel = panel.child(
            text("No playlists yet. Create one in Jellyfin and it appears here.")
                .font(type_::meta())
                .color(t.text_secondary)
                .mt(4.0),
        );
    }
    for playlist in &s.playlists {
        let id = playlist.id.clone();
        let name = playlist.name.clone();
        panel = panel.child(
            row()
                .key(format!("playlist-pick-{}", playlist.id))
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
                .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                .press_scale(0.98)
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut State| {
                    s.add_detail_to_playlist(id.clone(), name.clone());
                })
                .child(
                    text("＋".to_string())
                        .font(type_::label())
                        .color(tok::accent(0, true)),
                )
                .child(
                    text(playlist.name.clone())
                        .font(type_::label())
                        .color(t.text)
                        .truncate(),
                ),
        );
    }
    panel = panel
        .child(el().w_full().h(1.0).fill(t.card_border).mt(4.0))
        .child(
            row()
                .key("playlist-new")
                .role("button")
                .w_full()
                .h(34.0)
                .px(10.0)
                .radius(8.0)
                .items_center()
                .gap(8.0)
                .fill(theme::FOCUS.alpha(0.10))
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                .press_scale(0.98)
                .cursor(rune::Cursor::Pointer)
                .on_press(|s: &mut State| s.open_playlist_create())
                .child(
                    text("＋".to_string())
                        .font(type_::label())
                        .color(tok::accent(0, true)),
                )
                .child(
                    text("New playlist".to_string())
                        .font(type_::label())
                        .color(t.text)
                        .truncate(),
                ),
        );
    el().key("playlist-picker-layer")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .justify_center()
        .items_center()
        .fill(Color::BLACK.alpha(0.42))
        .on_press(|s: &mut State| s.close_playlist_picker())
        .child(panel)
}

/// "Add to a collection": every box set the server knows, loaded when the picker opens. A
/// collection is shared state, so the label says so rather than implying a private shelf.
pub(super) fn collection_picker(s: &State, item: Option<&Item>, t: &tok::Theme) -> El {
    let mut panel = col()
        .key("collection-picker")
        .w(380.0)
        .padding(16.0)
        .gap(6.0)
        .radius(14.0)
        .fill(t.card)
        .outline(1.0, 0.0, t.card_border)
        .shadow(0.0, 18.0, Color::BLACK.alpha(0.5))
        .child(
            row()
                .w_full()
                .items_center()
                .gap(8.0)
                .child(
                    text("Add to a collection")
                        .font(type_::section())
                        .color(t.text)
                        .grow(1.0),
                )
                .child(
                    el().key("collection-picker-close")
                        .role("button")
                        .h(26.0)
                        .px(8.0)
                        .radius(13.0)
                        .center()
                        .fill(Color::TRANSPARENT)
                        .hover(t.hover)
                        .motion(motion::SNAPPY)
                        .focusable(true)
                        .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                        .press_scale(0.96)
                        .child(text("Esc").font(type_::caption()).color(t.text_secondary))
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| s.close_collection_picker()),
                ),
        )
        .child(
            text(item.map(|item| item.name.clone()).unwrap_or_else(|| {
                let count = s.collection_picker_ids.as_ref().map_or(0, Vec::len);
                format!("{count} selected")
            }))
            .font(type_::meta())
            .color(t.text_secondary)
            .truncate(),
        );
    if s.collection_choices_loading && s.collection_choices.is_empty() {
        panel = panel.child(
            text("Loading collections…")
                .font(type_::meta())
                .color(t.text_secondary)
                .mt(4.0),
        );
    } else if s.collection_choices.is_empty() {
        panel = panel.child(
            text("No collections yet. Create one in Jellyfin, then add items here.")
                .font(type_::meta())
                .color(t.text_secondary)
                .mt(4.0),
        );
    }
    for collection in &s.collection_choices {
        let id = collection.id.clone();
        let name = collection.name.clone();
        panel = panel.child(
            row()
                .key(format!("collection-pick-{}", collection.id))
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
                .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                .press_scale(0.98)
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut State| {
                    s.add_detail_to_collection(id.clone(), name.clone());
                })
                .child(
                    text("＋".to_string())
                        .font(type_::label())
                        .color(tok::accent(0, true)),
                )
                .child(
                    text(collection.name.clone())
                        .font(type_::label())
                        .color(t.text)
                        .truncate(),
                ),
        );
    }
    el().key("collection-picker-layer")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .justify_center()
        .items_center()
        .fill(Color::BLACK.alpha(0.42))
        .on_press(|s: &mut State| s.close_collection_picker())
        .child(panel)
}

fn backdrop(s: &State, detail: &DetailState, _t: &tok::Theme) -> El {
    let image = s.detail_backdrop_key(&detail.item).and_then(|key| {
        super::poster_layers(s, &key, SurfaceFit::Fill).map(|layers| (key, layers))
    });
    let (top, bottom) = theme::palette(&detail.item.id);
    let mut layer = match image {
        Some((_, layers)) => el().absolute(0.0, 0.0).w_full().h_full().child(layers),
        None => el()
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .fill(top)
            .linear_gradient(std::f32::consts::PI, [(0.0, top), (1.0, bottom)]),
    };
    // A muted theme video is decoration over the still backdrop; it never takes input.
    if s.theme_video_on
        && let Some(player) = &s.theme_video
    {
        layer = layer.child(
            el().key("detail-theme-video-layer")
                .absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .opacity(0.45)
                .native_surface(player.surface(), SurfaceFit::Fill),
        );
    }
    layer
}

/// One row per server media source. The label is composed from facts the server sent, never
/// from the filename, so a 4K remux and a 1080p copy are distinguishable without guessing.
fn version_row(s: &State, detail: &DetailState, t: &tok::Theme) -> Option<El> {
    if detail.item.media_sources.len() < 2 {
        return None;
    }
    let mut chips = row().w_full().gap(8.0).flex_wrap();
    for source in &detail.item.media_sources {
        let video = source
            .media_streams
            .iter()
            .find(|stream| stream.kind == "Video");
        let label = source.name.clone().unwrap_or_else(|| {
            let resolution = video
                .and_then(|video| video.height)
                .map_or_else(String::new, |height| format!("{height}p"));
            let range = video
                .and_then(|video| video.video_range.clone())
                .filter(|range| !range.eq_ignore_ascii_case("SDR"))
                .unwrap_or_default();
            let bitrate = source
                .bitrate
                .map(|bits| format!(" · {:.0} Mbps", bits as f64 / 1_000_000.0))
                .unwrap_or_default();
            format!(
                "{resolution}{}{bitrate}",
                if range.is_empty() {
                    String::new()
                } else {
                    format!(" · {range}")
                }
            )
        });
        let selected = s.in_app_source.as_deref() == Some(source.id.as_str());
        let id = source.id.clone();
        chips = chips.child(
            el().key(format!("detail-version-{}", source.id))
                .role("button")
                .h(30.0)
                .px(10.0)
                .radius(7.0)
                .center()
                .fill(if selected {
                    tok::accent(0, true).alpha(0.24)
                } else {
                    t.card
                })
                .outline(
                    1.0,
                    0.0,
                    if selected {
                        tok::accent(0, true)
                    } else {
                        t.card_border
                    },
                )
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                .press_scale(0.97)
                .cursor(rune::Cursor::Pointer)
                .child(text(label).font(type_::caption()).color(t.text))
                .on_press(move |s: &mut State| s.play_detail_source(id.clone())),
        );
    }
    Some(
        row()
            .w_full()
            .items_center()
            .gap(10.0)
            .child(
                text("Version")
                    .font(type_::caption())
                    .color(t.text_secondary),
            )
            .child(chips.grow(1.0)),
    )
}

fn hero(s: &State, detail: &DetailState, t: &tok::Theme) -> El {
    let narrow = s.window_size.0 >= 1.0 && s.window_size.0 < 1000.0;
    let (poster_w, poster_h) = if narrow {
        (140.0, 210.0)
    } else if detail.item.kind == "BoxSet" {
        // A collection's cover is the hero, not the page: leave room for the first row of
        // members (and their titles) above the fold at 800 points.
        (180.0, 270.0)
    } else {
        (DETAIL_POSTER_W, DETAIL_POSTER_H)
    };
    let poster = s
        .detail_image_key(&detail.item)
        .and_then(|key| super::poster_layers(s, &key, SurfaceFit::Fill))
        .unwrap_or_else(|| placeholder(&detail.item, t));
    let poster = el()
        .key("detail-poster")
        .w(poster_w)
        .h(poster_h)
        .radius(12.0)
        .clip_rounded(12.0)
        .fill(t.card)
        .matched(format!("poster-{}", detail.item.id))
        .child(poster);

    let title = s
        .detail_logo_key(&detail.item)
        .and_then(|key| super::poster_layers(s, &key, SurfaceFit::Fit))
        .map_or_else(
            || {
                theme::readable_text(detail.item.name.clone(), type_::page(), t.text)
                    .key("detail-title")
            },
            |logo| {
                el().key("detail-title")
                    .role("img")
                    .label(detail.item.name.clone())
                    .w(340.0)
                    .h(104.0)
                    .child(logo)
            },
        );
    let metadata = theme::readable_text(
        detail::metadata(&detail.item),
        type_::meta(),
        t.text_secondary,
    )
    .key("detail-meta");
    let extended = detail::extended_metadata(&detail.item);
    let is_boxset = detail.item.kind == "BoxSet";
    let primary = el()
        .key("detail-play")
        .role("button")
        .h(36.0)
        .min_w(150.0)
        .px(18.0)
        .radius(9.0)
        .center()
        .fill(tok::accent(0, true))
        .gradient(Color::WHITE.alpha(0.18), Color::TRANSPARENT)
        .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
        .shadow(4.0, 12.0, tok::accent(0, true).alpha(0.35))
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
        .press_scale(0.97)
        .child(
            text(detail::primary_label(detail))
                .font(type_::label())
                .color(t.bg),
        )
        .cursor(rune::Cursor::Pointer)
        .on_press(|s: &mut State| s.play_detail());
    let favorite = detail
        .item
        .user_data
        .as_ref()
        .is_some_and(|user| user.is_favorite);
    let favorite = el()
        .key("detail-favorite")
        .role("button")
        .h(36.0)
        .px(16.0)
        .radius(9.0)
        .center()
        .fill(t.card)
        .outline(
            1.0,
            0.0,
            if favorite {
                tok::accent(0, true).alpha(0.70)
            } else {
                t.card_border
            },
        )
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
        .press_scale(0.97)
        .label(if favorite {
            "Remove favourite"
        } else {
            "Add favourite"
        })
        .child(
            text(if favorite { "Favorited" } else { "Favorite" })
                .font(type_::label())
                .color(if favorite {
                    tok::accent(0, true)
                } else {
                    t.text
                }),
        )
        .cursor(rune::Cursor::Pointer)
        .on_press(|s: &mut State| s.toggle_favorite());
    let played = detail
        .item
        .user_data
        .as_ref()
        .is_some_and(|user| user.played);
    let played = el()
        .key("detail-played")
        .role("button")
        .h(36.0)
        .px(16.0)
        .radius(9.0)
        .center()
        .fill(t.card)
        .gradient(Color::WHITE.alpha(0.075), Color::TRANSPARENT)
        .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
        .shadow(2.0, 6.0, Color::BLACK.alpha(0.18))
        .outline(1.0, 0.0, t.card_border)
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
        .press_scale(0.97)
        .label(if played {
            "Mark unwatched"
        } else {
            "Mark watched"
        })
        .child(
            text(if played { "Unwatched" } else { "Watched" })
                .font(type_::label())
                .color(t.text),
        )
        .cursor(rune::Cursor::Pointer)
        .on_press(|s: &mut State| s.toggle_played());

    let mut info = col()
        .grow(1.0)
        .min_w(0.0)
        .max_w(760.0)
        .items_start()
        .gap(14.0)
        .child(title)
        .child(metadata)
        .children((!extended.is_empty()).then(|| {
            theme::readable_text(extended, type_::meta(), t.text_secondary)
                .key("detail-extended-meta")
        }))
        .children(ratings(s, detail, t))
        .children(detail::technical_line(&detail.item).map(|line| {
            row()
                .gap(10.0)
                .items_center()
                .child(
                    theme::readable_text(line, type_::mono_caption(), t.text_secondary)
                        .key("detail-technical")
                        .grow(1.0),
                )
                .child(
                    el().key("detail-tech-toggle")
                        .role("button")
                        .h(24.0)
                        .px(8.0)
                        .radius(6.0)
                        .center()
                        .fill(t.card)
                        .hover(t.hover)
                        .motion(motion::SNAPPY)
                        .focusable(true)
                        .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                        .press_scale(0.96)
                        .child(
                            text(if s.detail_tech_open {
                                "Less"
                            } else {
                                "Details"
                            })
                            .font(type_::caption())
                            .color(t.text_secondary),
                        )
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| s.toggle_detail_tech()),
                )
        }))
        .children((s.detail_tech_open).then(|| technical(&detail.item, t)))
        .child({
            // Buttons wrap instead of clipping when the window is narrow.
            let mut actions = row().gap(10.0).flex_wrap();
            // A collection has no stream of its own; its members do.
            if !is_boxset {
                actions = actions.child(primary);
                if detail.item.media_sources.len() > 1 {
                    actions = actions.child(
                        el().key("detail-play-options")
                            .role("button")
                            .label("Playback options")
                            .h(36.0)
                            .w(34.0)
                            .radius(9.0)
                            .center()
                            .fill(t.card)
                            .gradient(Color::WHITE.alpha(0.075), Color::TRANSPARENT)
                            .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
                            .shadow(2.0, 6.0, Color::BLACK.alpha(0.18))
                            .outline(1.0, 0.0, t.card_border)
                            .hover(t.hover)
                            .motion(motion::SNAPPY)
                            .focusable(true)
                            .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                            .press_scale(0.97)
                            .cursor(rune::Cursor::Pointer)
                            .child(text("▾").font(type_::label()).color(t.text))
                            .on_press(|s: &mut State| s.open_preplay()),
                    );
                }
                // Start over belongs to the item the primary action opens, not the series
                // aggregate: a series carries no position of its own even when its Next Up
                // episode is 18 minutes in.
                if detail
                    .play_target()
                    .and_then(|item| item.user_data.as_ref())
                    .and_then(|user| user.position_ms())
                    .is_some_and(|position| position > 0)
                {
                    actions = actions.child(
                        el().key("detail-restart")
                            .role("button")
                            .h(36.0)
                            .px(16.0)
                            .radius(9.0)
                            .center()
                            .fill(t.card)
                            .gradient(Color::WHITE.alpha(0.075), Color::TRANSPARENT)
                            .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
                            .shadow(2.0, 6.0, Color::BLACK.alpha(0.18))
                            .outline(1.0, 0.0, t.card_border)
                            .hover(t.hover)
                            .motion(motion::SNAPPY)
                            .focusable(true)
                            .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                            .press_scale(0.97)
                            .label("Play from the beginning")
                            .child(text("Start over").font(type_::label()).color(t.text))
                            .cursor(rune::Cursor::Pointer)
                            .on_press(|s: &mut State| s.play_detail_from_start()),
                    );
                }
                if !s.detail_theme_songs.is_empty() {
                    actions = actions.child(
                        el().key("detail-theme-audio")
                            .role("button")
                            .label("Toggle theme song")
                            .h(36.0)
                            .px(16.0)
                            .radius(9.0)
                            .center()
                            .fill(if s.theme_audio.is_some() {
                                tok::accent(0, true).alpha(0.22)
                            } else {
                                t.card
                            })
                            .outline(1.0, 0.0, t.card_border)
                            .hover(t.hover)
                            .motion(motion::SNAPPY)
                            .focusable(true)
                            .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                            .press_scale(0.97)
                            .cursor(rune::Cursor::Pointer)
                            .child(
                                text(if s.theme_audio.is_some() {
                                    "♪ Theme on"
                                } else {
                                    "♪ Theme"
                                })
                                .font(type_::label())
                                .color(t.text),
                            )
                            .on_press(|s: &mut State| s.toggle_theme_audio()),
                    );
                }
                if !s.detail_theme_videos.is_empty() {
                    actions = actions.child(
                        el().key("detail-theme-video")
                            .role("button")
                            .label("Toggle theme video")
                            .h(36.0)
                            .px(16.0)
                            .radius(9.0)
                            .center()
                            .fill(if s.theme_video_on {
                                tok::accent(0, true).alpha(0.22)
                            } else {
                                t.card
                            })
                            .outline(1.0, 0.0, t.card_border)
                            .hover(t.hover)
                            .motion(motion::SNAPPY)
                            .focusable(true)
                            .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                            .press_scale(0.97)
                            .cursor(rune::Cursor::Pointer)
                            .child(
                                text(if s.theme_video_on {
                                    "Theme video on"
                                } else {
                                    "Theme video"
                                })
                                .font(type_::label())
                                .color(t.text),
                            )
                            .on_press(|s: &mut State| s.toggle_theme_video()),
                    );
                }
                if !detail.local_trailers.is_empty() || !detail.item.remote_trailers.is_empty()
                {
                    actions = actions.child(
                        el().key("detail-trailer")
                            .role("button")
                            .label("Watch trailer")
                            .h(36.0)
                            .px(16.0)
                            .radius(9.0)
                            .center()
                            .fill(t.card)
                            .gradient(Color::WHITE.alpha(0.075), Color::TRANSPARENT)
                            .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
                            .shadow(2.0, 6.0, Color::BLACK.alpha(0.18))
                            .outline(1.0, 0.0, t.card_border)
                            .hover(t.hover)
                            .motion(motion::SNAPPY)
                            .focusable(true)
                            .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                            .press_scale(0.97)
                            .help("A local trailer plays here; a provider trailer opens in your browser")
                            .cursor(rune::Cursor::Pointer)
                            .on_press(|s: &mut State| s.play_detail_trailer())
                            .child(
                                text(if s.trailer_loading {
                                    "Loading trailer…"
                                } else {
                                    "Trailer"
                                })
                                .font(type_::label())
                                .color(t.text),
                            ),
                    );
                }
                if detail.item.kind == "Series"
                    && let Some((tmdb, tv)) = &s.seerr.series
                    && detail
                        .item
                        .provider_ids
                        .get("Tmdb")
                        .and_then(|id| id.parse::<u64>().ok())
                        == Some(*tmdb)
                {
                    let present: std::collections::HashSet<i64> = detail
                        .seasons
                        .iter()
                        .filter_map(|season| season.index_number.map(i64::from))
                        .collect();
                    let missing = crate::state::missing_seasons(&present, tv);
                    if !missing.is_empty() {
                        let label = if missing.len() == 1 {
                            format!("Request season {}", missing[0])
                        } else {
                            format!("Request {} seasons", missing.len())
                        };
                        actions = actions.child(
                            el().key("detail-seerr-request")
                                .role("button")
                                .label(label.clone())
                                .h(36.0)
                                .px(16.0)
                                .radius(9.0)
                                .center()
                                .fill(t.card)
                                .gradient(Color::WHITE.alpha(0.075), Color::TRANSPARENT)
                                .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
                                .shadow(2.0, 6.0, Color::BLACK.alpha(0.18))
                                .outline(1.0, 0.0, t.card_border)
                                .hover(t.hover)
                                .motion(motion::SNAPPY)
                                .focusable(true)
                                .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                                .press_scale(0.97)
                                .cursor(rune::Cursor::Pointer)
                                .on_press(move |s: &mut State| {
                                    s.request_seerr_missing_seasons(missing.clone())
                                })
                                .child(text(label).font(type_::label()).color(t.text)),
                        );
                    }
                }
            } else if !detail.collection.is_empty() {
                actions = actions
                    .child(queue_button(
                        "▶ Play all",
                        "detail-play-all",
                        t,
                        true,
                        |s| s.play_collection_queue(false),
                    ))
                    .child(queue_button(
                        "⤨ Shuffle",
                        "detail-shuffle",
                        t,
                        false,
                        |s| s.play_collection_queue(true),
                    ));
            }
            actions = actions.child(favorite).child(played);
            if !is_boxset && crate::state::is_playable(&detail.item) {
                actions = actions.child(
                    el().key("detail-playlist")
                        .role("button")
                        .h(36.0)
                        .px(16.0)
                        .radius(9.0)
                        .center()
                        .fill(t.card)
                        .gradient(Color::WHITE.alpha(0.075), Color::TRANSPARENT)
                        .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
                        .shadow(2.0, 6.0, Color::BLACK.alpha(0.18))
                        .outline(1.0, 0.0, t.card_border)
                        .hover(t.hover)
                        .motion(motion::SNAPPY)
                        .focusable(true)
                        .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                        .press_scale(0.97)
                        .label("Add to a playlist")
                        .child(text("Playlist").font(type_::label()).color(t.text))
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| s.toggle_playlist_picker()),
                );
                actions = actions.child(
                    el().key("detail-collection-add")
                        .role("button")
                        .h(36.0)
                        .px(16.0)
                        .radius(9.0)
                        .center()
                        .fill(t.card)
                        .gradient(Color::WHITE.alpha(0.075), Color::TRANSPARENT)
                        .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
                        .shadow(2.0, 6.0, Color::BLACK.alpha(0.18))
                        .outline(1.0, 0.0, t.card_border)
                        .hover(t.hover)
                        .motion(motion::SNAPPY)
                        .focusable(true)
                        .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                        .press_scale(0.97)
                        .label("Add to a collection")
                        .child(text("Collection").font(type_::label()).color(t.text))
                        .cursor(rune::Cursor::Pointer)
                        .on_press(|s: &mut State| s.toggle_collection_picker()),
                );
            }
            actions
        });
    info = info.children(version_row(s, detail, t));
    if let Some(notice) = &s.detail_notice {
        info = info.child(
            text(notice.clone())
                .key("detail-notice")
                .font(type_::mono_caption())
                .color(tok::accent(3, true)),
        );
    }
    if let Some(error) = &s.playback_error {
        info = info.child(
            text(error.clone())
                .key("detail-playback-error")
                .font(type_::mono_caption())
                .color(tok::accent(2, true)),
        );
    }
    info = info
        .child(overview(s, detail, t))
        .child(row().gap(10.0).child(season_chips(detail, t)));

    // A narrow window stacks the poster above the text instead of letting the hero's
    // minimum widths push the overview and modals past the right edge.
    if narrow {
        col()
            .key("detail-hero")
            .w_full()
            .gap(20.0)
            .items_start()
            .child(poster)
            .child(info)
    } else {
        row()
            .key("detail-hero")
            .w_full()
            .min_w(0.0)
            .gap(28.0)
            .items_start()
            .child(poster)
            .child(info)
    }
}

fn technical(item: &Item, t: &tok::Theme) -> El {
    let facts = detail::technical_facts(item);
    if facts.is_empty() {
        return col();
    }
    let mut card = col()
        .key("detail-tech")
        .w_full()
        .max_w(620.0)
        .padding(12.0)
        .gap(6.0)
        .radius(10.0)
        .fill(t.card)
        .outline(1.0, 0.0, t.card_border);
    for (label, value) in facts {
        card = card.child(
            row()
                .gap(12.0)
                .items_start()
                .child(
                    text(label)
                        .w(80.0)
                        .font(type_::caption())
                        .color(t.text_secondary),
                )
                .child(text(value).font(type_::mono_caption()).color(t.text)),
        );
    }
    card
}

fn overview(s: &State, detail: &DetailState, t: &tok::Theme) -> El {
    let Some(overview) = detail.item.overview.as_deref().filter(|o| !o.is_empty()) else {
        return col();
    };
    let long = overview.chars().count() > 420;
    let lines = if s.detail_overview_open { 24 } else { 4 };
    let mut block = col().max_w(720.0).gap(6.0).child(
        theme::readable_lines(overview.to_string(), type_::overview(), t.text, lines)
            .key("detail-overview"),
    );
    if long {
        block = block.child(
            el().key("detail-overview-toggle")
                .role("button")
                .h(28.0)
                .px(8.0)
                .radius(6.0)
                .center()
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                .press_scale(0.96)
                .child(
                    text(if s.detail_overview_open {
                        "Less"
                    } else {
                        "More"
                    })
                    .font(type_::label())
                    .color(t.text_secondary),
                )
                .cursor(rune::Cursor::Pointer)
                .on_press(|s: &mut State| s.toggle_detail_overview()),
        );
    }
    block
}

/// The three scores readers compare: IMDb (OMDb when configured), TMDb and Rotten Tomatoes.
/// The server carries the last two; an absent IMDb key leaves a link to the title instead.
fn ratings(s: &State, detail: &DetailState, t: &tok::Theme) -> Option<El> {
    let item = &detail.item;
    let mut row = row().key("detail-ratings").w_full().gap(8.0).flex_wrap();
    let mut any = false;
    if let Some(imdb) = s.detail_imdb {
        row = row.child(rating_badge(
            "IMDb",
            format!("{imdb:.1}"),
            [245, 197, 24],
            "detail-rating-imdb",
            t,
        ));
        any = true;
    } else if let Some(imdb) = item.provider_ids.get("Imdb") {
        let id = imdb.clone();
        row = row.child(
            el().key("detail-imdb-link")
                .role("button")
                .label("Open this title on IMDb")
                .h(24.0)
                .px(8.0)
                .radius(12.0)
                .items_center()
                .gap(4.0)
                .fill(brand([245, 197, 24]).alpha(0.12))
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                .press_scale(0.97)
                .cursor(rune::Cursor::Pointer)
                .on_press(move |_s: &mut State| {
                    let url = format!("https://www.imdb.com/title/{id}/");
                    std::thread::spawn(move || {
                        let _ = State::open_url(&url);
                    });
                })
                .child(
                    text("IMDb")
                        .font(type_::caption())
                        .color(brand([245, 197, 24])),
                )
                .child(
                    text("↗".to_string())
                        .font(type_::caption())
                        .color(brand([245, 197, 24])),
                ),
        );
        any = true;
    }
    if let Some(tmdb) = item.community_rating.filter(|rating| *rating > 0.0) {
        row = row.child(rating_badge(
            "TMDb",
            format!("{tmdb:.1}"),
            [1, 180, 228],
            "detail-rating-tmdb",
            t,
        ));
        any = true;
    }
    if let Some(rt) = item.critic_rating.filter(|rating| *rating > 0.0) {
        row = row.child(rating_badge(
            "RT",
            format!("{rt:.0}%"),
            [250, 48, 72],
            "detail-rating-rt",
            t,
        ));
        any = true;
    }
    for (provider, label, host, rgb, key) in [
        (
            "Tmdb",
            "TMDb",
            "https://www.themoviedb.org/movie/",
            [1, 180, 228],
            "detail-tmdb-link",
        ),
        (
            "Tvdb",
            "TVDB",
            "https://thetvdb.com/?id=",
            [40, 160, 120],
            "detail-tvdb-link",
        ),
    ] {
        if let Some(id) = item.provider_ids.get(provider) {
            let id = id.clone();
            row = row.child(provider_link(label, host, key, rgb, t, id));
            any = true;
        }
    }
    any.then_some(row)
}

fn provider_link(
    label: &str,
    host: &str,
    key: &str,
    rgb: [u8; 3],
    t: &tok::Theme,
    id: String,
) -> El {
    let colour = brand(rgb);
    let url = format!("{host}{id}");
    el().key(key)
        .role("button")
        .label(format!("Open this title on {label}"))
        .h(24.0)
        .px(8.0)
        .radius(12.0)
        .items_center()
        .gap(4.0)
        .fill(colour.alpha(0.12))
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
        .press_scale(0.97)
        .cursor(rune::Cursor::Pointer)
        .on_press(move |_s: &mut State| {
            let url = url.clone();
            std::thread::spawn(move || {
                let _ = State::open_url(&url);
            });
        })
        .child(text(label.to_string()).font(type_::caption()).color(colour))
        .child(text("↗".to_string()).font(type_::caption()).color(colour))
}

fn rating_badge(label: &str, value: String, rgb: [u8; 3], key: &str, t: &tok::Theme) -> El {
    let colour = brand(rgb);
    el().key(key)
        .h(24.0)
        .px(8.0)
        .radius(12.0)
        .items_center()
        .gap(6.0)
        .fill(colour.alpha(0.13))
        .child(text(label.to_string()).font(type_::caption()).color(colour))
        .child(text(value).font(type_::caption()).color(t.text))
}

fn brand(rgb: [u8; 3]) -> rune::Color {
    rune::Color {
        r: rune::color::srgb_decode(rgb[0] as f32 / 255.0),
        g: rune::color::srgb_decode(rgb[1] as f32 / 255.0),
        b: rune::color::srgb_decode(rgb[2] as f32 / 255.0),
        a: 1.0,
    }
}

/// A collection's members, five poster cards per line. The cards open the member's own
/// detail, which is how a collection becomes a browse path rather than a dead end.
fn collection(s: &State, detail: &DetailState, t: &tok::Theme) -> Option<El> {
    if detail.collection.is_empty() {
        return None;
    }
    let members = detail.collection_sort.apply(&detail.collection);
    let watched = members
        .iter()
        .filter(|item| crate::home::is_watched(item))
        .count();
    let total = members.len();
    // The sort lives on the same line as the heading when it fits. A second line pushed the
    // first row of posters (and their titles) below an 800-point window.
    let mut sort = row()
        .key("detail-collection-sort")
        .items_center()
        .gap(6.0)
        .child(
            text("Sort")
                .font(type_::caption())
                .color(t.text_secondary.alpha(0.75)),
        );
    for mode in crate::detail::CollectionSort::ALL {
        let selected = detail.collection_sort == mode;
        sort = sort.child(
            el().key(format!("collection-sort-{}", mode.label().to_lowercase()))
                .role("button")
                .h(26.0)
                .px(9.0)
                .radius(13.0)
                .center()
                .fill(if selected {
                    tok::accent(0, true).alpha(0.22)
                } else {
                    t.card
                })
                .outline(
                    1.0,
                    0.0,
                    if selected {
                        tok::accent(0, true)
                    } else {
                        t.card_border
                    },
                )
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                .press_scale(0.96)
                .cursor(rune::Cursor::Pointer)
                .child(
                    text(mode.label())
                        .font(type_::caption())
                        .color(if selected { t.text } else { t.text_secondary }),
                )
                .on_press(move |s: &mut State| s.set_collection_sort(mode)),
        );
    }
    let mut grid = col().key("detail-collection").w_full().gap(14.0).child(
        row()
            .w_full()
            .items_center()
            .gap(12.0)
            .flex_wrap()
            .child(
                text("In this collection")
                    .key("detail-collection-title")
                    .font(type_::section())
                    .color(t.text_secondary)
                    .grow(1.0),
            )
            .child(sort)
            .child(
                text(format!("{watched} of {total} watched"))
                    .key("detail-collection-progress")
                    .font(type_::mono_caption())
                    .color(t.text_secondary),
            )
            .children((watched < total).then(|| {
                queue_button("▶ Continue", "detail-collection-continue", t, true, |s| {
                    s.play_collection_queue(false)
                })
            })),
    );
    for chunk in members.chunks(5) {
        let mut line = row().w_full().gap(16.0).items_start();
        for item in chunk {
            // The same key `request_detail_images` fetches for a collection member; a different
            // width would always miss the cache and leave the member as initials.
            let image = s
                .image_key(item)
                .and_then(|key| super::poster_layers(s, &key, SurfaceFit::Fill))
                .unwrap_or_else(|| placeholder(item, t));
            let id = item.id.clone();
            let year = item
                .production_year
                .map(|year| year.to_string())
                .unwrap_or_default();
            line = line.child(
                col()
                    .key(format!("collection-{}", item.id))
                    .role("button")
                    .w(176.0)
                    .gap(6.0)
                    .hover(t.hover)
                    .motion(motion::SNAPPY)
                    .focusable(true)
                    .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                    .press_scale(0.98)
                    .cursor(rune::Cursor::Pointer)
                    .on_press(move |s: &mut State| s.open_detail(id.clone()))
                    .child(
                        el().w(176.0)
                            .h(264.0)
                            .radius(10.0)
                            .clip_rounded(10.0)
                            .fill(t.card)
                            .child(image),
                    )
                    .child(
                        text(item.name.clone())
                            .font(type_::card_title())
                            .color(t.text)
                            .truncate(),
                    )
                    .child(text(year).font(type_::caption()).color(t.text_secondary)),
            );
        }
        grid = grid.child(line);
    }
    Some(grid)
}

/// "More like this": the server's own recommendation list, as a rail of the same cards the
/// rest of the app uses. The section is absent when the server has nothing to offer, rather
/// than an empty heading.
/// Collections an item belongs to. The server owns membership; this is a set of links, not an
/// editor.
fn collections_row(s: &State, t: &tok::Theme) -> Option<El> {
    if s.detail_collections.is_empty() {
        return None;
    }
    let mut chips = row()
        .key("detail-collections")
        .w_full()
        .gap(6.0)
        .flex_wrap();
    for collection in &s.detail_collections {
        let id = collection.id.clone();
        chips = chips.child(
            el().key(format!("detail-collection-{}", collection.id))
                .role("button")
                .h(30.0)
                .px(10.0)
                .radius(15.0)
                .center()
                .fill(t.card)
                .outline(1.0, 0.0, t.card_border)
                .hover(t.hover)
                .focusable(true)
                .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                .cursor(rune::Cursor::Pointer)
                .child(
                    text(format!("Part of {}", collection.name))
                        .font(type_::caption())
                        .color(t.text_secondary),
                )
                .on_press(move |s: &mut State| s.open_detail(id.clone())),
        );
    }
    Some(
        row()
            .w_full()
            .items_center()
            .gap(8.0)
            .child(
                text("Collections")
                    .font(type_::caption())
                    .color(t.text_secondary),
            )
            .child(chips),
    )
}

/// Special features grouped by their server `ExtraType`. Clicking one plays it in the in-app
/// player without reporting, like a trailer; it never enters the queue.
fn extras(s: &State, t: &tok::Theme) -> Option<El> {
    if s.detail_extras.is_empty() {
        return None;
    }
    let mut groups: Vec<(String, Vec<&Item>)> = Vec::new();
    for item in &s.detail_extras {
        let label = match item.extra_type.as_deref().unwrap_or("Other") {
            "BehindTheScenes" => "Behind the scenes",
            "DeletedScene" => "Deleted scenes",
            "Interview" => "Interviews",
            "Featurette" => "Featurettes",
            "Short" => "Shorts",
            "Clip" => "Clips",
            "Trailer" => "Trailers",
            other => other,
        }
        .to_string();
        if let Some((_, items)) = groups.iter_mut().find(|(name, _)| *name == label) {
            items.push(item);
        } else {
            groups.push((label, vec![item]));
        }
    }
    let mut section = col().key("detail-extras").w_full().gap(10.0).child(
        theme::readable_text("Extras", type_::section(), t.text_secondary)
            .key("detail-extras-title"),
    );
    for (label, items) in groups {
        let mut rail = row().gap(crate::theme::CARD_GAP).focus_group();
        for item in items {
            rail = rail.child(extra_card(s, item, t));
        }
        section = section
            .child(text(label).font(type_::caption()).color(t.text_secondary))
            .child(
                el().w_full()
                    .min_w(0.0)
                    .scroll_x()
                    .scroll_edge_fade(24.0)
                    .child(rail),
            );
    }
    Some(section)
}

fn extra_card(s: &State, item: &Item, t: &tok::Theme) -> El {
    let play_item = item.clone();
    let key = s.image_key(item);
    let image = key
        .as_deref()
        .and_then(|key| super::poster_layers(s, key, SurfaceFit::Fill))
        .unwrap_or_else(|| super::placeholder(item, t));
    col()
        .key(format!("detail-extra-{}", item.id))
        .role("button")
        .w(180.0)
        .gap(6.0)
        .focusable(true)
        .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
        .press_scale(0.98)
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut State| s.play_extra(play_item.clone()))
        .child(
            el().w(180.0)
                .h(101.0)
                .radius(8.0)
                .clip_rounded(8.0)
                .fill(t.card)
                .child(image),
        )
        .child(
            text(item.name.clone())
                .font(type_::card_title())
                .color(t.text)
                .truncate(),
        )
        .child(
            text(
                item.runtime_ms()
                    .map(|ms| format!("{} min", (ms / 60_000).max(1)))
                    .unwrap_or_default(),
            )
            .font(type_::mono_caption())
            .color(t.text_secondary),
        )
}

fn similar(s: &State, detail: &DetailState, t: &tok::Theme) -> Option<El> {
    if detail.similar.is_empty() {
        return None;
    }
    Some(
        col()
            .key("detail-similar")
            .w_full()
            .gap(10.0)
            .child(
                theme::readable_text("More like this", type_::section(), t.text_secondary)
                    .key("detail-similar-title"),
            )
            .child(
                el().key("detail-similar-scroll")
                    .w_full()
                    .min_w(0.0)
                    .scroll_x()
                    .scroll_edge_fade(28.0)
                    .child(row().gap(crate::theme::CARD_GAP).focus_group().children({
                        // Duplicate card keys made Rune rebuild/reorder the rail on state
                        // changes. Keep the server order but one card per series.
                        let mut seen = std::collections::HashSet::new();
                        let items: Vec<&Item> = detail
                            .similar
                            .iter()
                            .filter(|item| {
                                seen.insert(
                                    item.series_id
                                        .as_deref()
                                        .unwrap_or(item.id.as_str())
                                        .to_string(),
                                )
                            })
                            .take(12)
                            .collect();
                        items
                            .into_iter()
                            .map(|item| super::card(item, "similar", s, t))
                    })),
            ),
    )
}

fn queue_button(
    label: &str,
    key: &str,
    t: &tok::Theme,
    primary: bool,
    on: impl Fn(&mut State) + 'static,
) -> El {
    el().key(key)
        .role("button")
        .h(36.0)
        .px(18.0)
        .radius(9.0)
        .center()
        .fill(if primary {
            tok::accent(0, true)
        } else {
            t.card
        })
        .gradient(
            if primary {
                Color::WHITE.alpha(0.18)
            } else {
                Color::WHITE.alpha(0.075)
            },
            Color::TRANSPARENT,
        )
        .outline(1.0, 0.0, t.card_border)
        .shadow(1.0, 2.0, Color::BLACK.alpha(0.30))
        .shadow(
            if primary { 4.0 } else { 2.0 },
            if primary { 12.0 } else { 6.0 },
            if primary {
                tok::accent(0, true).alpha(0.35)
            } else {
                Color::BLACK.alpha(0.18)
            },
        )
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .focusable(true)
        .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
        .press_scale(0.97)
        .label(label)
        .child(
            text(label.to_string())
                .font(type_::label())
                .color(if primary { t.bg } else { t.text }),
        )
        .cursor(rune::Cursor::Pointer)
        .on_press(on)
}

fn season_chips(detail: &DetailState, t: &tok::Theme) -> El {
    if detail.seasons.is_empty() {
        return row();
    }
    let mut chips = row().key("detail-seasons").gap(8.0);
    for season in &detail.seasons {
        let selected = detail.active_season.as_deref() == Some(season.id.as_str());
        let id = season.id.clone();
        chips = chips.child(
            el().key(format!("season-{}", season.id))
                .role("button")
                .h(30.0)
                .px(12.0)
                .radius(8.0)
                .center()
                // The active season is the one the episode list belongs to: a raised chip with
                // an accent hairline, not a second filled primary on the page.
                .fill(if selected {
                    tok::accent(0, true).alpha(0.20)
                } else {
                    Color::TRANSPARENT
                })
                .outline(
                    1.0,
                    0.0,
                    if selected {
                        tok::accent(0, true).alpha(0.70)
                    } else {
                        t.card_border
                    },
                )
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                .press_scale(0.96)
                .child(
                    text(season.name.clone())
                        .font(type_::label())
                        .color(if selected { t.text } else { t.text_secondary }),
                )
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut State| s.select_season(id.clone())),
        );
    }
    chips
}

fn series(s: &State, detail: &DetailState, t: &tok::Theme) -> Option<El> {
    if detail.seasons.is_empty() && detail.episodes.is_empty() {
        return None;
    }
    let mut section = col().key("detail-series").w_full().gap(12.0);
    if !detail.loading_episodes && !detail.episodes.is_empty() {
        let season_name = detail
            .active_season
            .as_ref()
            .and_then(|id| detail.seasons.iter().find(|season| &season.id == id))
            .map(|season| season.name.clone())
            .unwrap_or_else(|| "Episodes".to_string());
        section = section.child(
            row()
                .key("detail-episodes-header")
                .w_full()
                .items_center()
                .gap(8.0)
                .child(
                    text(season_name)
                        .font(type_::section())
                        .color(t.text_secondary)
                        .grow(1.0),
                )
                .child(queue_button(
                    "▶ Play season",
                    "detail-play-season",
                    t,
                    false,
                    |s| s.play_season_queue(false),
                ))
                .child(queue_button(
                    "⤨ Shuffle",
                    "detail-shuffle-season",
                    t,
                    false,
                    |s| s.play_season_queue(true),
                ))
                .child(queue_button(
                    "✓ Season",
                    "detail-season-watched",
                    t,
                    false,
                    |s| s.mark_detail_season_watched(),
                )),
        );
    }
    if detail.loading_episodes {
        section = section.child(
            text("Loading episodes…")
                .key("detail-episodes-loading")
                .font(subtitle_style())
                .color(t.text_secondary),
        );
    } else {
        section = section.child(
            col().key("detail-episodes").w_full().gap(8.0).children(
                detail
                    .episodes
                    .iter()
                    .map(|episode| episode_row(episode, s, t)),
            ),
        );
    }
    Some(section)
}

/// The next episode the server recommends. It is one row, not another rail: a still, the
/// episode identity, the resume position and a single action.
fn next_up(s: &State, detail: &DetailState, t: &tok::Theme) -> Option<El> {
    let item = detail.next_up.as_ref()?;
    let guarded = s.spoiler_guard && !crate::home::is_watched(item);
    let image = s
        .episode_image_key(item)
        .and_then(|key| super::poster_layers(s, &key, SurfaceFit::Fill))
        .unwrap_or_else(|| placeholder(item, t));
    let label = item
        .episode_label()
        .map(|label| format!("{label} · {}", item.name))
        .unwrap_or_else(|| item.name.clone());
    let resume = "Play next".to_string();
    let episode = item.clone();
    Some(
        col()
            .key("detail-next-up")
            .w_full()
            .gap(10.0)
            .child(
                text("Next up")
                    .key("detail-next-up-title")
                    .font(type_::section())
                    .color(t.text_secondary),
            )
            .child(
                row()
                    .key("detail-next-up-card")
                    .w_full()
                    .padding(10.0)
                    .gap(16.0)
                    .items_center()
                    .radius(12.0)
                    .fill(t.card)
                    .outline(1.0, 0.0, t.card_border)
                    .hover(t.hover)
                    .motion(motion::SNAPPY)
                    .role("button")
                    .focusable(true)
                    .focus_ring(theme::focus_width(), 2.0, theme::FOCUS)
                    .press_scale(0.995)
                    .cursor(rune::Cursor::Pointer)
                    .on_press(move |s: &mut State| s.play_episode(episode.clone()))
                    .child(
                        el().w(256.0)
                            .h(144.0)
                            .radius(8.0)
                            .clip_rounded(8.0)
                            .fill(t.bg)
                            .child(
                                el().w_full()
                                    .h_full()
                                    .blur(if guarded { 12.0 } else { 0.0 })
                                    .child(image),
                            )
                            .children(guarded.then(|| {
                                el().key("detail-next-up-guard")
                                    .absolute(0.0, 0.0)
                                    .w_full()
                                    .h_full()
                                    .center()
                                    .child(
                                        text("Spoiler hidden")
                                            .font(type_::caption())
                                            .color(Color::WHITE),
                                    )
                            })),
                    )
                    .child(
                        col()
                            .grow(1.0)
                            .min_w(0.0)
                            .gap(5.0)
                            .child(
                                text(label)
                                    .font(type_::body())
                                    .color(t.text)
                                    .max_lines(2)
                                    .truncate(),
                            )
                            .child(
                                text(if guarded {
                                    "Spoiler hidden — play to reveal.".to_string()
                                } else {
                                    item.overview.clone().unwrap_or_default()
                                })
                                .font(type_::meta())
                                .color(t.text_secondary)
                                .max_lines(2)
                                .truncate(),
                            ),
                    )
                    .child(
                        el().w(132.0)
                            .h(38.0)
                            .radius(9.0)
                            .center()
                            .fill(t.bg)
                            .outline(1.0, 0.0, t.card_border)
                            .child(text(resume).font(type_::label()).color(t.text)),
                    ),
            ),
    )
}

/// The people the server returned with the item. This is deliberately a quiet rail, not a
/// second hero: a face, a name and the role the server supplied.
fn cast(s: &State, detail: &DetailState, t: &tok::Theme) -> Option<El> {
    if detail.item.people.is_empty() {
        return None;
    }
    let mut rail = row()
        .key("detail-cast-rail")
        .w_full()
        .min_w(0.0)
        .gap(16.0)
        .scroll_x()
        .scroll_edge_fade(20.0);
    for person in detail.item.people.iter().take(16) {
        let role = person
            .role
            .clone()
            .unwrap_or_else(|| person.kind.clone().unwrap_or_else(|| "Cast".to_string()));
        let image = s
            .person_image_key(person)
            .and_then(|key| super::poster_layers(s, &key, SurfaceFit::Fill))
            .unwrap_or_else(|| {
                el().w_full().h_full().fill(t.card).center().child(
                    text(
                        person
                            .name
                            .chars()
                            .next()
                            .map_or_else(|| "?".to_string(), |c| c.to_uppercase().to_string()),
                    )
                    .font(type_::section())
                    .color(t.text_secondary),
                )
            });
        let person_id = person.id.clone();
        let person_for_page = person.clone();
        rail = rail.child(
            col()
                .key(format!("detail-cast-{person_id}"))
                .role("button")
                .w(96.0)
                .gap(6.0)
                .items_center()
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_ring(theme::focus_width(), 2.0, theme::FOCUS)
                .press_scale(0.98)
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut State| s.open_person(&person_for_page))
                .child(
                    el().w(76.0)
                        .h(76.0)
                        .radius(38.0)
                        .clip_rounded(38.0)
                        .fill(t.card)
                        .child(image),
                )
                .child(
                    text(person.name.clone())
                        .font(type_::caption())
                        .color(t.text)
                        .max_lines(1)
                        .truncate(),
                )
                .child(
                    text(role)
                        .font(type_::mono_caption())
                        .color(t.text_secondary)
                        .max_lines(1)
                        .truncate(),
                ),
        );
    }
    Some(
        col()
            .key("detail-cast")
            .w_full()
            .gap(12.0)
            .child(
                theme::readable_text("Cast", type_::section(), t.text_secondary)
                    .key("detail-cast-title"),
            )
            .child(rail),
    )
}

fn episode_row(item: &Item, s: &State, t: &tok::Theme) -> El {
    let label = item.episode_label().unwrap_or_else(|| item.name.clone());
    let mut runtime = item
        .runtime_ms()
        .map(|ms| format!("{} min", (ms / 60_000).max(1)))
        .unwrap_or_default();
    if let Some(date) = item
        .premiere_date
        .as_deref()
        .and_then(|date| date.get(..10))
    {
        runtime = if runtime.is_empty() {
            format!("Airs {date}")
        } else {
            format!("{runtime} · {date}")
        };
    }
    let watched = crate::home::progress(item);
    row()
        .key(format!("detail-episode-{}", item.id))
        .w_full()
        .padding(10.0)
        .gap(12.0)
        .radius(8.0)
        .fill(t.card)
        .hover(t.hover)
        .motion(motion::SNAPPY)
        .role("button")
        .focusable(true)
        .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
        .press_scale(0.995)
        .cursor(rune::Cursor::Pointer)
        .on_press({
            let episode = item.clone();
            move |s: &mut State| s.play_episode(episode.clone())
        })
        .on_secondary_press({
            let episode = item.clone();
            let focus_key = format!("detail-episode-{}", item.id);
            move |s: &mut State, press: rune::PointerPress, _tx: &mut rune::Tx| {
                s.open_card_menu_at(episode.clone(), press.x, press.y, Some(focus_key.clone()));
            }
        })
        .children(crate::home::is_watched(item).then(|| {
            el().key(format!("played-{}", item.id))
                .w(20.0)
                .h(20.0)
                .radius(10.0)
                .center()
                .fill(tok::accent(0, true))
                .child(text("✓").font(type_::caption()).color(t.bg))
        }))
        .child(
            el().w(160.0)
                .h(90.0)
                .radius(6.0)
                .clip_rounded(6.0)
                .fill(t.bg)
                .child(
                    s.episode_image_key(item)
                        .and_then(|key| super::poster_layers(s, &key, SurfaceFit::Fill))
                        .unwrap_or_else(|| placeholder(item, t)),
                ),
        )
        .child(
            text(label)
                .font(type_::mono_caption())
                .color(t.text_secondary)
                .w(52.0),
        )
        .child(
            col()
                .grow(1.0)
                .gap(4.0)
                .child(
                    text(item.name.clone())
                        .font(type_::body())
                        .color(t.text)
                        .truncate(),
                )
                .child(
                    text(runtime)
                        .font(type_::mono_caption())
                        .color(t.text_secondary),
                ),
        )
        .child(
            el().w(80.0).h(3.0).radius(1.5).fill(t.control_track).child(
                el().w(80.0 * watched)
                    .h(3.0)
                    .radius(1.5)
                    .fill(tok::accent(0, true)),
            ),
        )
}

fn detail_skeleton(t: &tok::Theme) -> El {
    col()
        .key("detail-loading")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(24.0)
        .child(
            text("Loading…")
                .font(subtitle_style())
                .color(t.text_secondary),
        )
        .child(
            row()
                .w_full()
                .gap(28.0)
                .items_start()
                .child(
                    el().w(DETAIL_POSTER_W)
                        .h(DETAIL_POSTER_H)
                        .radius(12.0)
                        .fill(t.card),
                )
                .child(
                    col()
                        .grow(1.0)
                        .gap(14.0)
                        .child(el().w(360.0).h(34.0).radius(6.0).fill(t.card))
                        .child(el().w(260.0).h(12.0).radius(6.0).fill(t.card)),
                ),
        )
}

fn detail_error(message: &str, t: &tok::Theme) -> El {
    col()
        .key("detail-error")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .center()
        .gap(10.0)
        .child(
            text("Couldn't open this item")
                .font(hero_style())
                .color(t.text),
        )
        .child(
            text(message.to_string())
                .key("detail-error-message")
                .font(subtitle_style())
                .color(tok::accent(2, true)),
        )
        .child(
            el().key("detail-retry")
                .role("button")
                .h(34.0)
                .px(14.0)
                .radius(8.0)
                .center()
                .fill(t.card)
                .hover(t.hover)
                .motion(motion::SNAPPY)
                .focusable(true)
                .focus_ring(crate::theme::focus_width(), 2.0, crate::theme::FOCUS)
                .press_scale(0.97)
                .child(text("Try again").font(type_::label()).color(t.text))
                .cursor(rune::Cursor::Pointer)
                .on_press(|s: &mut State| s.retry_detail()),
        )
}
