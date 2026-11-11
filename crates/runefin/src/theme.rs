//! Runefin's design tokens. Rune's `tok` remains the colour engine; this module owns the
//! app-level type, spacing, radius and surface choices so screens do not drift.

use rune::tok::{self, Theme};
use rune::{Color, El, Pixels, Shadow, el, text};

pub mod type_ {
    use rune::TextStyle;

    use super::text_scale;

    /// A route title: Library, Detail or Settings.
    pub const PAGE: TextStyle = TextStyle::new(32.0, 700.0).lh(38.0);
    /// A section title inside a page.
    pub const SECTION: TextStyle = TextStyle::new(20.0, 600.0).lh(26.0);
    /// A card title or a compact panel title.
    pub const TITLE: TextStyle = TextStyle::new(18.0, 600.0).lh(24.0);
    /// One-line card title.
    pub const CARD_TITLE: TextStyle = TextStyle::new(14.0, 600.0).lh(20.0);
    /// The default reading size.
    pub const BODY: TextStyle = TextStyle::new(16.0, 400.0).lh(23.0);
    /// One metadata line.
    pub const META: TextStyle = TextStyle::new(14.0, 400.0).lh(20.0);
    /// Button and control labels.
    pub const LABEL: TextStyle = TextStyle::new(14.0, 600.0).lh(20.0);
    /// Captions, badges and timecodes.
    pub const CAPTION: TextStyle = TextStyle::new(12.0, 500.0).lh(16.0);
    /// Episode and item descriptions.
    pub const OVERVIEW: TextStyle = TextStyle::new(15.0, 400.0).lh(23.0);

    /// The big screen raises only the small tokens: a 12 pt caption at 1080p is 18 px, below
    /// the ~24 px couch minimum; page and section titles are already large. The grid and
    /// poster geometry stay at 720 points.
    fn big(base: TextStyle, size: f32) -> TextStyle {
        if !super::is_big_screen() {
            return base;
        }
        TextStyle {
            size,
            line_height: if base.line_height > 0.0 {
                base.line_height * size / base.size
            } else {
                0.0
            },
            ..base
        }
    }

    /// Apply the reader's text-size preference to a base style. Line height scales with the
    /// size so leading keeps its ratio; zero stays zero (Rune's own 1.25 × size).
    pub fn scaled(base: TextStyle) -> TextStyle {
        let scale = text_scale();
        if (scale - 1.0).abs() < f32::EPSILON {
            return base;
        }
        TextStyle {
            size: base.size * scale,
            line_height: if base.line_height > 0.0 {
                base.line_height * scale
            } else {
                0.0
            },
            ..base
        }
    }

    pub fn page() -> TextStyle {
        scaled(PAGE)
    }
    pub fn section() -> TextStyle {
        scaled(SECTION)
    }
    pub fn title() -> TextStyle {
        scaled(TITLE)
    }
    pub fn card_title() -> TextStyle {
        scaled(big(CARD_TITLE, 16.0))
    }
    pub fn body() -> TextStyle {
        scaled(BODY)
    }
    pub fn meta() -> TextStyle {
        scaled(big(META, 16.0))
    }
    pub fn label() -> TextStyle {
        scaled(big(LABEL, 16.0))
    }
    pub fn caption() -> TextStyle {
        scaled(big(CAPTION, 16.0))
    }
    pub fn mono_caption() -> TextStyle {
        scaled(big(CAPTION, 16.0))
    }
    pub fn overview() -> TextStyle {
        scaled(OVERVIEW)
    }
}

/// The wordmark and the app mark, decoded once from the committed PNGs. Decoding is the same
/// `image` crate the placeholder pipeline uses, so the app adds no second decoder. A broken
/// asset draws nothing rather than panicking at startup.
fn embedded(bytes: &[u8]) -> Pixels {
    let pixels = Pixels::new();
    if let Ok(image) = ::image::load_from_memory(bytes) {
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        pixels.set(width, height, rgba.into_raw());
    }
    pixels
}

pub fn logo() -> &'static Pixels {
    static LOGO: std::sync::OnceLock<Pixels> = std::sync::OnceLock::new();
    LOGO.get_or_init(|| embedded(include_bytes!("../../../Assets/Logo_with_name.png")))
}

pub fn app_icon() -> &'static Pixels {
    static ICON: std::sync::OnceLock<Pixels> = std::sync::OnceLock::new();
    ICON.get_or_init(|| embedded(include_bytes!("../../../Assets/Icon.png")))
}

static TEXT_SCALE: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(1.0f32.to_bits());

pub fn set_text_scale(scale: f32) {
    TEXT_SCALE.store(
        scale.clamp(0.85, 1.35).to_bits(),
        std::sync::atomic::Ordering::Relaxed,
    );
}

pub fn text_scale() -> f32 {
    f32::from_bits(TEXT_SCALE.load(std::sync::atomic::Ordering::Relaxed))
}

pub const CARD_W: f32 = 176.0;
pub const CARD_H: f32 = 264.0;
pub const CARD_GAP: f32 = 16.0;
pub const RAIL_GAP: f32 = 40.0;
pub const GUTTER: f32 = 40.0;
pub const SIDEBAR_W: f32 = 224.0;

/// Visible over both themes; white disappears on a light surface.
pub const FOCUS: Color = Color {
    r: 0.0,
    g: 0.20,
    b: 0.80,
    a: 1.0,
};

/// A 2 pt ring is 3 px at 1080p. The neutral colour carries the visibility; a 4 pt solid blue
/// read as a selection box over artwork.
pub fn focus_width() -> f32 {
    2.0
}

/// The focus ring colour. A TV shows a thin neutral ring; the solid blue reads as a selection
/// box over artwork. Desktop keeps the accent.
pub fn ring() -> Color {
    if is_big_screen() {
        colors().text.alpha(0.85)
    } else {
        FOCUS
    }
}

/// The focus treatment on a control. Desktop keeps the accent ring; big screen uses the same
/// wash the pointer gets, with no ring: a ring around a card or a sidebar row reads as a
/// selection box, not as focus.
pub trait FocusVisual {
    fn focus_visual(self, radius: f32) -> El;
    /// A lifted card shows focus by its scale alone; desktop keeps its ring.
    fn focus_lift_only(self, radius: f32) -> El;
}

impl FocusVisual for El {
    fn focus_visual(self, radius: f32) -> El {
        if is_big_screen() {
            // Without `no_focus_ring`, Rune falls back to its blue default ring.
            self.focus_fill(colors().hover).no_focus_ring()
        } else {
            self.focus_ring(focus_width(), radius, ring())
        }
    }

    fn focus_lift_only(self, radius: f32) -> El {
        if is_big_screen() {
            self.no_focus_ring()
        } else {
            self.focus_ring(focus_width(), radius, ring())
        }
    }
}

/// The lit-card details that keep a flat fill from reading as a printed rectangle.
pub trait SurfaceStyle {
    /// A 3.5 % white sheen from the top edge, under the content.
    fn surface_sheen(self) -> El;
    /// A 1 px inset white top edge, the contact light on a raised card.
    fn inset_edge(self) -> El;
}

impl SurfaceStyle for El {
    fn surface_sheen(self) -> El {
        self.gradient(Color::WHITE.alpha(0.035), Color::TRANSPARENT)
    }

    fn inset_edge(self) -> El {
        self.box_shadow(Shadow {
            y: 1.0,
            blur: 0.0,
            spread: 0.0,
            color: Color::WHITE.alpha(0.05),
            inset: true,
            ..Shadow::default()
        })
    }
}

/// The focus halo, drawn at the window root so a rail's scroll clip cannot cut it. Four
/// blurred bars form a frame outside the focused poster; the centre stays clear, so the card's
/// artwork is never washed out.
pub fn focus_glow(rect: [f32; 4]) -> El {
    let (x, y, w, h) = (rect[0], rect[1], rect[2], rect[3]);
    let glow = Color::WHITE.alpha(0.24);
    let bar = |left: f32, top: f32, width: f32, height: f32| {
        el().absolute(left, top)
            .w(width)
            .h(height)
            .radius(4.0)
            .fill(glow)
            .blur(10.0)
    };
    el().key("focus-glow")
        .absolute(x - 6.0, y - 6.0)
        .w(w + 12.0)
        .h(h + 12.0)
        .child(bar(0.0, 0.0, w + 12.0, 6.0))
        .child(bar(0.0, h + 6.0, w + 12.0, 6.0))
        .child(bar(0.0, 6.0, 6.0, h))
        .child(bar(w + 6.0, 6.0, 6.0, h))
        .no_focus_ring()
}

/// Give a big-screen control the pointer's neutral wash while it holds keyboard focus. Return
/// activation comes from `rune::set_return_activates`, not from a highlight colour.
pub fn tv_focus(el: El) -> El {
    if is_big_screen() {
        // The same neutral wash the pointer gets on the desktop, not an accent slab: focus and
        // selection must not look alike. No ring, and none of Rune's blue default either.
        el.focus_fill(colors().hover).no_focus_ring()
    } else {
        el
    }
}

static HIGH_CONTRAST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_high_contrast(on: bool) {
    HIGH_CONTRAST.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn is_high_contrast() -> bool {
    HIGH_CONTRAST.load(std::sync::atomic::Ordering::Relaxed)
}

static BIG_SCREEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The 10-foot presentation flag. The layout itself is scaled by Rune's design height; this
/// only changes the safe margin and lets screens drop pointer-only affordances.
pub fn set_big_screen(on: bool) {
    BIG_SCREEN.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn is_big_screen() -> bool {
    BIG_SCREEN.load(std::sync::atomic::Ordering::Relaxed)
}

/// The page gutter. A TV keeps 5 % of the width clear for overscan; the desk layout uses 40 pt.
pub fn gutter() -> f32 {
    if is_big_screen() { 64.0 } else { GUTTER }
}

static LIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_light(light: bool) {
    LIGHT.store(light, std::sync::atomic::Ordering::Relaxed);
}

pub fn is_light() -> bool {
    LIGHT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Text that sits over artwork: one offset copy of the glyphs behind the fill reads as a
/// shadow without the hard rectangle a box shadow draws around a text element. Rune has no
/// glyph-shadow primitive, so the shadow is the same run, blurred, black and slightly lower.
pub fn readable_text(value: impl Into<String>, style: rune::TextStyle, color: Color) -> El {
    shadowed(value.into(), style, color, None)
}

/// The same shadow for a clamped paragraph, used by the detail overview.
pub fn readable_lines(
    value: impl Into<String>,
    style: rune::TextStyle,
    color: Color,
    lines: usize,
) -> El {
    shadowed(value.into(), style, color, Some(lines))
}

fn shadowed(value: String, style: rune::TextStyle, color: Color, lines: Option<usize>) -> El {
    // On a light surface the text is dark, so the dark halo that separates white text
    // from a poster turns into a smudge. Use a light halo there, and keep the black one
    // for the dark theme's white text.
    let shadow_color = if is_light() {
        Color::WHITE.alpha(0.70)
    } else {
        Color::BLACK.alpha(0.80)
    };
    let mut shadow = text(value.clone()).font(style).color(shadow_color);
    let mut main = text(value).font(style).color(color);
    let mut layer = el().items_start();
    if let Some(lines) = lines {
        shadow = shadow.max_lines(lines).w_full();
        main = main.max_lines(lines).w_full();
        layer = layer.w_full();
    }
    layer
        .child(el().absolute(0.0, 1.5).w_full().blur(2.0).child(shadow))
        .child(main)
}

/// A deterministic two-stop wash for an item without artwork. The seed is the item id, so a
/// card keeps the same colour between frames, and the stops stay low-chroma so white text and
/// the letter mark remain readable.
pub fn palette(seed: &str) -> (Color, Color) {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut hasher);
    let hue = (hasher.finish() % 360) as f32;
    if is_light() {
        (
            rune::color::oklch(0.92, 0.045, hue),
            rune::color::oklch(0.82, 0.04, (hue + 38.0) % 360.0),
        )
    } else {
        (
            rune::color::oklch(0.30, 0.06, hue),
            rune::color::oklch(0.18, 0.045, (hue + 38.0) % 360.0),
        )
    }
}

pub fn colors() -> Theme {
    let mut theme = tok::theme(!is_light());
    let cool = tok::accent(0, true);
    theme.bg = theme.bg.mix(cool, 0.028);
    theme.card = theme.card.mix(cool, 0.038);
    theme.separator = theme.separator.mix(cool, 0.04);
    // Rune's dark default is a 1.6 % white wash, which is invisible on a card. A 10 % wash is
    // enough for a button to answer the pointer without turning into a second fill.
    theme.hover = Color::WHITE.alpha(0.10);
    theme.press = Color::WHITE.alpha(0.16);
    if is_high_contrast() {
        theme.bg = theme.bg.mix(Color::BLACK, 0.55);
        theme.card = theme.card.mix(Color::BLACK, 0.45);
        theme.text = if is_light() {
            Color::BLACK
        } else {
            Color::WHITE
        };
        theme.text_secondary = if is_light() {
            Color::BLACK.alpha(0.82)
        } else {
            Color::WHITE.alpha(0.88)
        };
        theme.card_border = if is_light() {
            Color::BLACK.alpha(0.28)
        } else {
            Color::WHITE.alpha(0.32)
        };
        theme.separator = theme.card_border.alpha(0.62);
        theme.hover = if is_light() {
            Color::BLACK.alpha(0.12)
        } else {
            Color::WHITE.alpha(0.16)
        };
        theme.press = theme.hover.alpha(0.24);
    }
    theme
}

/// Elevation and material for surfaces that float over artwork.
pub mod elevation {
    use rune::{Color, El, Shadow};

    use super::Theme;

    /// Contact, mid and ambient layers, plus a 1 px inset top edge: a panel keeps an edge on a
    /// bright frame without a black halo. These are the CSS-shaped shadows `El::box_shadow`
    /// stacks (negative spread on the wide layers, as in modern elevation tokens).
    pub fn floating_layers() -> [Shadow; 4] {
        [
            Shadow {
                y: 1.0,
                blur: 2.5,
                spread: 0.0,
                color: Color::BLACK.alpha(0.35),
                ..Shadow::default()
            },
            Shadow {
                y: 6.0,
                blur: 12.0,
                spread: -2.0,
                color: Color::BLACK.alpha(0.38),
                ..Shadow::default()
            },
            Shadow {
                y: 16.0,
                blur: 48.0,
                spread: -6.0,
                color: Color::BLACK.alpha(0.45),
                ..Shadow::default()
            },
            Shadow {
                y: 1.0,
                blur: 0.0,
                spread: 0.0,
                color: Color::WHITE.alpha(0.06),
                inset: true,
                ..Shadow::default()
            },
        ]
    }

    /// [`floating_layers`] on an element. The first four shadows spring, so a panel that
    /// resizes keeps its depth while it moves.
    pub fn floating(mut el: El) -> El {
        for shadow in floating_layers() {
            el = el.box_shadow(shadow);
        }
        el
    }

    /// A panel over video: frost what is behind it, tint the frost, then a hairline and
    /// elevation. `El::backdrop_blur` blurs the frame already drawn; four elements may blur in
    /// one frame, so frost the open panel and the control bar, not every menu.
    pub fn frosted(mut el: El, t: &Theme) -> El {
        el = el
            .backdrop_blur(16.0)
            .fill(t.card.alpha(0.52))
            // A sheen from the top edge reads as glass even when the frame behind is dark;
            // without it the tint on a night scene looks like an opaque panel.
            .gradient(Color::WHITE.alpha(0.06), Color::TRANSPARENT)
            .outline(1.0, 0.0, Color::WHITE.alpha(0.12));
        floating(el)
    }
}

/// State is not accent: availability and request statuses get their own inks, so a blue chip
/// always means "selected" or "actionable".
pub mod status {
    use rune::color::oklch;
    use rune::{Color, El, el, row, text};

    use super::{is_light, type_};

    pub fn success() -> Color {
        tone(0.76, 0.12, 152.0, 0.52)
    }
    pub fn warning() -> Color {
        tone(0.82, 0.11, 78.0, 0.56)
    }
    pub fn danger() -> Color {
        tone(0.72, 0.16, 25.0, 0.55)
    }
    pub fn info() -> Color {
        tone(0.76, 0.07, 245.0, 0.50)
    }
    pub fn neutral() -> Color {
        super::colors().text_secondary
    }

    fn tone(dark_l: f32, chroma: f32, hue: f32, light_l: f32) -> Color {
        oklch(if is_light() { light_l } else { dark_l }, chroma, hue)
    }

    /// A state label inline in a row: a tinted fill with the tone at full strength.
    pub fn pill(label: &str, tone: Color) -> El {
        el().h(20.0)
            .px(7.0)
            .radius(6.0)
            .center()
            .fill(tone.alpha(0.14))
            .child(text(label.to_owned()).font(type_::caption()).color(tone))
    }

    /// The same state over artwork: an opaque chip with a tone dot, so a bright poster cannot
    /// wash the label out.
    pub fn overlay(label: &str, tone: Color) -> El {
        row()
            .h(22.0)
            .px(8.0)
            .gap(5.0)
            .radius(6.0)
            .center_y()
            .fill(Color::BLACK.alpha(0.62))
            .child(el().size(6.0, 6.0).radius(3.0).fill(tone))
            .child(
                text(label.to_owned())
                    .font(type_::caption())
                    .color(Color::WHITE),
            )
    }
}

pub mod icon {
    use rune::Icon;

    pub static HOME: Icon = Icon {
        name: "runefin-home",
        view: 20.0,
        body: r#"<path d="M3 9.5L10 3l7 6.5V17h-5v-4H8v4H3z" stroke="currentColor" stroke-width="1.6" fill="none" stroke-linejoin="round"/>"#,
    };
    pub static SEARCH: Icon = Icon {
        name: "runefin-search",
        view: 20.0,
        body: r#"<circle cx="8.5" cy="8.5" r="5" stroke="currentColor" stroke-width="1.6" fill="none"/><path d="M12.2 12.2L17 17" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>"#,
    };
    pub static REMOTE: Icon = Icon {
        name: "runefin-remote",
        view: 20.0,
        body: r#"<rect x="4" y="2.5" width="12" height="15" rx="3" stroke="currentColor" stroke-width="1.6" fill="none"/><circle cx="10" cy="7" r="1.6" fill="currentColor"/><path d="M7 12h6M7 15h6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>"#,
    };
    /// A gamepad outline for the controller controls sheet.
    pub static GAMEPAD: Icon = Icon {
        name: "runefin-gamepad",
        view: 20.0,
        body: r#"<path d="M6.2 6h7.6a4 4 0 0 1 3.8 2.8l.9 3.2a2.3 2.3 0 0 1-4 2l-1.3-1.5H6.8L5.5 14a2.3 2.3 0 0 1-4-2l.9-3.2A4 4 0 0 1 6.2 6z" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linejoin="round"/><path d="M6.5 8.6v2.2M5.4 9.7h2.2" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><circle cx="13.6" cy="9.1" r=".9" fill="currentColor"/><circle cx="14.8" cy="10.7" r=".9" fill="currentColor"/>"#,
    };
    pub static SETTINGS: Icon = Icon {
        name: "runefin-settings",
        view: 20.0,
        body: r#"<circle cx="10" cy="10" r="2.5" stroke="currentColor" stroke-width="1.6" fill="none"/><path d="M10 2.5v2M10 15.5v2M2.5 10h2M15.5 10h2M4.7 4.7l1.4 1.4M13.9 13.9l1.4 1.4M15.3 4.7l-1.4 1.4M6.1 13.9l-1.4 1.4" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>"#,
    };
    pub static PLAY: Icon = Icon {
        name: "runefin-play",
        view: 20.0,
        body: r#"<path d="M6 4.5l10 5.5-10 5.5z" fill="currentColor"/>"#,
    };
    pub static PAUSE: Icon = Icon {
        name: "runefin-pause",
        view: 20.0,
        body: r#"<rect x="5" y="4" width="3.5" height="12" rx="1" fill="currentColor"/><rect x="11.5" y="4" width="3.5" height="12" rx="1" fill="currentColor"/>"#,
    };
    pub static SEEK_BACK: Icon = Icon {
        name: "runefin-seek-back",
        view: 20.0,
        body: r#"<path d="M10 5L4 10l6 5z" fill="currentColor"/><path d="M16 5l-6 5 6 5z" fill="currentColor"/>"#,
    };
    pub static SEEK_FORWARD: Icon = Icon {
        name: "runefin-seek-forward",
        view: 20.0,
        body: r#"<path d="M10 5l6 5-6 5z" fill="currentColor"/><path d="M4 5l6 5-6 5z" fill="currentColor"/>"#,
    };
    /// Episode stepping: a bar against the edge, then a triangle. Not the double chevron of a
    /// ten-second seek, which is a different action.
    pub static SKIP_PREV: Icon = Icon {
        name: "runefin-skip-prev",
        view: 20.0,
        body: r#"<rect x="4" y="5" width="2.4" height="10" rx="0.8" fill="currentColor"/><path d="M16 5l-7 5 7 5z" fill="currentColor"/>"#,
    };
    pub static SKIP_NEXT: Icon = Icon {
        name: "runefin-skip-next",
        view: 20.0,
        body: r#"<path d="M4 5l7 5-7 5z" fill="currentColor"/><rect x="13.6" y="5" width="2.4" height="10" rx="0.8" fill="currentColor"/>"#,
    };
    pub static VOLUME_HIGH: Icon = Icon {
        name: "runefin-volume-high",
        view: 20.0,
        body: r#"<path d="M3 8h3l4-3v10l-4-3H3z" fill="currentColor"/><path d="M13 7.5a3.5 3.5 0 0 1 0 5M15 5.5a6 6 0 0 1 0 9" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round"/>"#,
    };
    /// Four corners opening outward; fullscreen.
    pub static EXPAND: Icon = Icon {
        name: "runefin-expand",
        view: 20.0,
        body: r#"<path d="M8 4H4v4M12 4h4v4M8 16H4v-4M16 16h-4v-4" stroke="currentColor" stroke-width="1.6" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"#,
    };
    /// Four corners closing inward; windowed.
    pub static COLLAPSE: Icon = Icon {
        name: "runefin-collapse",
        view: 20.0,
        body: r#"<path d="M4 8h4V4M16 8h-4V4M4 12h4v4M16 12h-4v4" stroke="currentColor" stroke-width="1.6" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"#,
    };
    pub static MUTE: Icon = Icon {
        name: "runefin-mute",
        view: 20.0,
        body: r#"<path d="M3 8h3l4-3v10l-4-3H3z" fill="currentColor"/><path d="M13 8l4 4M17 8l-4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>"#,
    };
    /// Settings for the picture, not the app: three sliders. The sidebar's gear stays the app's
    /// settings; a player needs a different shape at a glance.
    pub static TUNE: Icon = Icon {
        name: "runefin-tune",
        view: 20.0,
        body: r#"<path d="M3 6h8M15 6h2M3 14h3M10 14h7" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/><circle cx="13" cy="6" r="1.8" stroke="currentColor" stroke-width="1.6" fill="none"/><circle cx="8" cy="14" r="1.8" stroke="currentColor" stroke-width="1.6" fill="none"/>"#,
    };
    pub static STOP: Icon = Icon {
        name: "runefin-stop",
        view: 20.0,
        body: r#"<rect x="5" y="5" width="10" height="10" rx="1.5" fill="currentColor"/>"#,
    };
    pub static CHEVRON_LEFT: Icon = Icon {
        name: "runefin-chevron-left",
        view: 20.0,
        body: r#"<path d="M12.5 5L7.5 10l5 5" stroke="currentColor" stroke-width="1.6" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"#,
    };
    pub static CHEVRON_RIGHT: Icon = Icon {
        name: "runefin-chevron-right",
        view: 20.0,
        body: r#"<path d="M7.5 5l5 5-5 5" stroke="currentColor" stroke-width="1.6" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"#,
    };
    pub static HEART: Icon = Icon {
        name: "runefin-heart",
        view: 20.0,
        body: r#"<path d="M10 16.5S3.5 12.7 3.5 7.9A3.4 3.4 0 0 1 10 6a3.4 3.4 0 0 1 6.5 1.9c0 4.8-6.5 8.6-6.5 8.6z" stroke="currentColor" stroke-width="1.6" fill="none" stroke-linejoin="round"/>"#,
    };
    pub static FILM: Icon = Icon {
        name: "runefin-film",
        view: 20.0,
        body: r#"<rect x="2.5" y="3.5" width="15" height="13" rx="2" stroke="currentColor" stroke-width="1.6" fill="none"/><path d="M6 3.5v13M14 3.5v13" stroke="currentColor" stroke-width="1.6"/>"#,
    };
    pub static TV: Icon = Icon {
        name: "runefin-tv",
        view: 20.0,
        body: r#"<rect x="2.5" y="4" width="15" height="10" rx="2" stroke="currentColor" stroke-width="1.6" fill="none"/><path d="M7 17h6M10 14v3" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>"#,
    };
    pub static LAYERS: Icon = Icon {
        name: "runefin-layers",
        view: 20.0,
        body: r#"<path d="M10 3l7 4-7 4-7-4z" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" fill="none"/><path d="M3 11l7 4 7-4M3 15l7 4 7-4" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" fill="none"/>"#,
    };
    pub static ANTENNA: Icon = Icon {
        name: "runefin-antenna",
        view: 20.0,
        body: r#"<circle cx="10" cy="9" r="1.5" stroke="currentColor" stroke-width="1.6" fill="none"/><path d="M10 10.5V17M7 13.5h6M5.5 6.5a7 7 0 0 1 9 0M3 4a10.5 10.5 0 0 1 14 0" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" fill="none"/>"#,
    };
    pub static LIST: Icon = Icon {
        name: "runefin-list",
        view: 20.0,
        body: r#"<circle cx="3.5" cy="5.5" r="1" fill="currentColor"/><circle cx="3.5" cy="10" r="1" fill="currentColor"/><circle cx="3.5" cy="14.5" r="1" fill="currentColor"/><path d="M7 5.5h10M7 10h10M7 14.5h7" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>"#,
    };
    pub static RECORD: Icon = Icon {
        name: "runefin-record",
        view: 20.0,
        body: r#"<circle cx="10" cy="10" r="6" stroke="currentColor" stroke-width="1.6" fill="none"/><circle cx="10" cy="10" r="2" fill="currentColor"/>"#,
    };
    pub static LIBRARY: Icon = Icon {
        name: "runefin-library",
        view: 20.0,
        body: r#"<path d="M3 5.5h5l1.5 2H17v9H3z" stroke="currentColor" stroke-width="1.6" fill="none" stroke-linejoin="round"/>"#,
    };
    pub static COMPASS: Icon = Icon {
        name: "runefin-compass",
        view: 20.0,
        body: r#"<circle cx="10" cy="10" r="7" stroke="currentColor" stroke-width="1.6" fill="none"/><path d="M13 7l-1.8 4.2L7 13l1.8-4.2z" stroke="currentColor" stroke-width="1.4" fill="none" stroke-linejoin="round"/>"#,
    };
    pub static BELL: Icon = Icon {
        name: "runefin-bell",
        view: 20.0,
        body: r#"<path d="M10 3.6a4 4 0 0 1 4 4v2.3l1.5 2.6H4.5L6 9.9V7.6a4 4 0 0 1 4-4zM8.4 15a1.6 1.6 0 0 0 3.2 0" stroke="currentColor" stroke-width="1.6" fill="none" stroke-linejoin="round"/>"#,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_size_preference_scales_size_and_leading_together() {
        set_text_scale(1.0);
        let base = type_::body();
        set_text_scale(1.15);
        let large = type_::body();
        assert!((large.size - base.size * 1.15).abs() < 0.01);
        assert!((large.line_height - base.line_height * 1.15).abs() < 0.01);
        assert_eq!(type_::card_title().size, type_::CARD_TITLE.size * 1.15);
        set_text_scale(1.0);
        assert_eq!(type_::label().size, type_::LABEL.size);
    }
}
