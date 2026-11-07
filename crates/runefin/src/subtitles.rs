//! SRT, WebVTT and ASS parsing for the player's own subtitle overlay.
//!
//! The custom GStreamer sink is a raw video sink, so `textoverlay` never runs: subtitles drawn
//! by GStreamer would be lost. The server serves external tracks as SRT (and converts embedded
//! ones), and the original ASS for tracks that are ASS. SRT is centred at the bottom; ASS keeps
//! its event alignment, `\pos` anchor and font size, which is what signs and titles need.

use std::collections::HashMap;

/// Where a cue sits on the picture, in the ASS numpad order (1 bottom-left … 9 top-right).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    BottomLeft,
    #[default]
    BottomCenter,
    BottomRight,
    MiddleLeft,
    MiddleCenter,
    MiddleRight,
    TopLeft,
    TopCenter,
    TopRight,
}

impl Align {
    /// ASS numbers its alignments like a numpad: 1–3 bottom, 4–6 middle, 7–9 top.
    pub fn from_ass(value: u8) -> Align {
        match value {
            1 => Align::BottomLeft,
            3 => Align::BottomRight,
            4 => Align::MiddleLeft,
            5 => Align::MiddleCenter,
            6 => Align::MiddleRight,
            7 => Align::TopLeft,
            8 => Align::TopCenter,
            9 => Align::TopRight,
            _ => Align::BottomCenter,
        }
    }

    /// The vertical band the cue belongs to: 0 top, 1 middle, 2 bottom.
    pub fn band(self) -> u8 {
        match self {
            Align::TopLeft | Align::TopCenter | Align::TopRight => 0,
            Align::MiddleLeft | Align::MiddleCenter | Align::MiddleRight => 1,
            Align::BottomLeft | Align::BottomCenter | Align::BottomRight => 2,
        }
    }

    /// The horizontal alignment: 0 start, 1 centre, 2 end.
    pub fn side(self) -> u8 {
        match self {
            Align::BottomLeft | Align::MiddleLeft | Align::TopLeft => 0,
            Align::BottomCenter | Align::MiddleCenter | Align::TopCenter => 1,
            Align::BottomRight | Align::MiddleRight | Align::TopRight => 2,
        }
    }
}

/// One timed subtitle, in milliseconds from the start of the item.
#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub align: Align,
    /// An explicit ASS `\pos` anchor, normalised to the script's play resolution.
    pub pos: Option<(f32, f32)>,
    /// Font size as a fraction of the script height; zero means "use the overlay's base size".
    pub font_percent: f32,
    /// Text colour as sRGB bytes, and opacity 0–255 (ASS stores transparency, not opacity).
    pub color: [u8; 3],
    /// True when a markup tag (`<font color>`) chose the colour, so the reader's preset must
    /// not overwrite it.
    pub color_explicit: bool,
    pub opacity: u8,
    pub bold: bool,
    pub italic: bool,
    /// ASS outline width as a fraction of the script height; zero means no outline.
    pub border_percent: f32,
    /// ASS drop-shadow distance as a fraction of the script height; zero means no shadow.
    pub shadow_percent: f32,
    pub border_color: [u8; 3],
    pub shadow_color: [u8; 3],
    pub shadow_opacity: u8,
    /// ASS `\fscx` / `\fscy` as factors. Rune scales uniformly, so the overlay uses the
    /// geometric mean; a pure horizontal stretch is approximated rather than reproduced.
    pub scale: (f32, f32),
    /// ASS `\frz` in degrees clockwise.
    pub rotation: f32,
}

impl Default for Cue {
    fn default() -> Self {
        Cue {
            start_ms: 0,
            end_ms: 0,
            text: String::new(),
            align: Align::BottomCenter,
            pos: None,
            font_percent: 0.0,
            color: [255, 255, 255],
            color_explicit: false,
            opacity: 255,
            bold: false,
            italic: false,
            border_percent: 0.0,
            shadow_percent: 0.0,
            border_color: [0, 0, 0],
            shadow_color: [0, 0, 0],
            shadow_opacity: 180,
            scale: (1.0, 1.0),
            rotation: 0.0,
        }
    }
}

impl Cue {
    fn plain(start_ms: i64, end_ms: i64, text: String) -> Cue {
        Cue {
            start_ms,
            end_ms,
            text,
            align: Align::BottomCenter,
            pos: None,
            font_percent: 0.0,
            color: [255, 255, 255],
            color_explicit: false,
            opacity: 255,
            bold: false,
            italic: false,
            border_percent: 0.0,
            shadow_percent: 0.0,
            border_color: [0, 0, 0],
            shadow_color: [0, 0, 0],
            shadow_opacity: 180,
            scale: (1.0, 1.0),
            rotation: 0.0,
        }
    }
}

/// One row of `[V4+ Styles]`, reduced to what the overlay can draw.
#[derive(Debug, Clone, Copy)]
struct Style {
    size: f32,
    align: u8,
    color: [u8; 3],
    opacity: u8,
    bold: bool,
    italic: bool,
    outline: f32,
    shadow: f32,
    outline_color: [u8; 3],
    back_color: [u8; 3],
    back_opacity: u8,
    scale: (f32, f32),
    rotation: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            size: 40.0,
            align: 2,
            color: [255, 255, 255],
            opacity: 255,
            bold: false,
            italic: false,
            outline: 0.0,
            shadow: 0.0,
            outline_color: [0, 0, 0],
            back_color: [0, 0, 0],
            back_opacity: 180,
            scale: (1.0, 1.0),
            rotation: 0.0,
        }
    }
}

/// Parse an SRT or WebVTT file. Image subtitles never reach this: the server cannot convert them.
pub fn parse(bytes: &[u8]) -> Vec<Cue> {
    // Subtitle files are usually UTF-8, but old external tracks are often CP1252. Lossy
    // decoding keeps the English text rather than failing the whole track on one bad byte.
    parse_text(&String::from_utf8_lossy(bytes))
}

pub fn parse_text(raw: &str) -> Vec<Cue> {
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut cues = Vec::new();
    for block in normalized.split("\n\n") {
        let lines: Vec<&str> = block.lines().collect();
        // A cue may start with an identifier (SRT number, VTT cue name); the timing line is
        // never further down than that. Header blocks (WEBVTT, NOTE, STYLE, Region) have none.
        let Some(timing_at) = lines.iter().take(3).position(|line| line.contains("-->")) else {
            continue;
        };
        let Some((start_ms, end_ms)) = timing(lines[timing_at]) else {
            continue;
        };
        if end_ms <= start_ms {
            continue;
        }
        let plain = clean(&lines[timing_at + 1..].join("\n"));
        if plain.text.is_empty() {
            continue;
        }
        let mut cue = Cue::plain(start_ms, end_ms, plain.text);
        cue.align = plain.align;
        cue.bold = plain.bold;
        cue.italic = plain.italic;
        if let Some(color) = plain.color {
            cue.color = color;
            cue.color_explicit = true;
        }
        cues.push(cue);
    }
    cues.sort_by_key(|cue| cue.start_ms);
    cues
}

/// Parse an ASS/SSA script. The server hands embedded TEXT tracks back as SRT, so this is used
/// only for tracks the server serves as ASS; `parse_text` stays the fallback.
pub fn parse_ass(bytes: &[u8]) -> Vec<Cue> {
    parse_ass_text(&String::from_utf8_lossy(bytes))
}

#[derive(Debug, Clone, PartialEq)]
enum Section {
    None,
    Styles,
    Events,
}

pub fn parse_ass_text(raw: &str) -> Vec<Cue> {
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut section = Section::None;
    let mut play_res = (1280.0f32, 720.0f32);
    let mut styles: HashMap<String, Style> = HashMap::new();
    let mut style_format: Vec<String> = Vec::new();
    let mut event_format: Vec<String> = Vec::new();
    let mut cues = Vec::new();
    for line in normalized.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            section = match trimmed.to_ascii_lowercase().as_str() {
                "[v4+ styles]" | "[v4 styles]" | "[v4++ styles]" => Section::Styles,
                "[events]" => Section::Events,
                _ => Section::None,
            };
            continue;
        }
        match section {
            Section::None => {
                if let Some(value) = trimmed.strip_prefix("PlayResX:")
                    && let Ok(value) = value.trim().parse::<f32>()
                {
                    play_res.0 = value.max(1.0);
                } else if let Some(value) = trimmed.strip_prefix("PlayResY:")
                    && let Ok(value) = value.trim().parse::<f32>()
                {
                    play_res.1 = value.max(1.0);
                }
            }
            Section::Styles => {
                if let Some(format) = trimmed.strip_prefix("Format:") {
                    style_format = field_names(format);
                } else if let Some(style) = trimmed.strip_prefix("Style:")
                    && let Some((name, fields)) = named_fields(style, &style_format)
                {
                    let font_size = fields
                        .get("fontsize")
                        .and_then(|value| value.trim().parse::<f32>().ok())
                        .unwrap_or(40.0);
                    let align = fields
                        .get("alignment")
                        .and_then(|value| value.trim().parse::<u8>().ok())
                        .unwrap_or(2);
                    let (color, opacity) = fields
                        .get("primarycolour")
                        .and_then(|value| ass_colour(value))
                        .unwrap_or(([255, 255, 255], 255));
                    let bold = fields.get("bold").is_some_and(|value| value.trim() != "0");
                    let italic = fields
                        .get("italic")
                        .is_some_and(|value| value.trim() != "0");
                    let number = |key: &str, fallback: f32| {
                        fields
                            .get(key)
                            .and_then(|value| value.trim().parse::<f32>().ok())
                            .unwrap_or(fallback)
                    };
                    let (outline_color, _) = fields
                        .get("outlinecolour")
                        .and_then(|value| ass_colour(value))
                        .unwrap_or(([0, 0, 0], 255));
                    let (back_color, back_opacity) = fields
                        .get("backcolour")
                        .and_then(|value| ass_colour(value))
                        .unwrap_or(([0, 0, 0], 180));
                    styles.insert(
                        name,
                        Style {
                            size: font_size,
                            align,
                            color,
                            opacity,
                            bold,
                            italic,
                            outline: number("outline", 0.0).max(0.0),
                            shadow: number("shadow", 0.0).max(0.0),
                            outline_color,
                            back_color,
                            back_opacity,
                            scale: (
                                number("scalex", 100.0).max(0.0) / 100.0,
                                number("scaley", 100.0).max(0.0) / 100.0,
                            ),
                            rotation: number("angle", 0.0),
                        },
                    );
                }
            }
            Section::Events => {
                if let Some(format) = trimmed.strip_prefix("Format:") {
                    event_format = field_names(format);
                } else if let Some(dialogue) = trimmed.strip_prefix("Dialogue:")
                    && let Some(cue) = ass_dialogue(dialogue, &event_format, &styles, play_res)
                {
                    cues.push(cue);
                }
            }
        }
    }
    cues.sort_by_key(|cue| cue.start_ms);
    cues
}

fn field_names(format: &str) -> Vec<String> {
    format
        .split(',')
        .map(|name| name.trim().to_ascii_lowercase())
        .collect()
}

/// Split an ASS record by its Format field, keeping commas inside the last field (Text).
fn named_fields<'a>(
    record: &'a str,
    format: &[String],
) -> Option<(String, HashMap<String, &'a str>)> {
    if format.is_empty() {
        return None;
    }
    let mut parts = record.splitn(format.len(), ',');
    let mut map = HashMap::new();
    for name in format {
        let value = parts.next().unwrap_or_default();
        map.insert(name.clone(), value);
    }
    let name = map.get("name")?.trim().to_string();
    Some((name, map))
}

fn ass_dialogue(
    record: &str,
    format: &[String],
    styles: &HashMap<String, Style>,
    play_res: (f32, f32),
) -> Option<Cue> {
    if format.is_empty() {
        return None;
    }
    let mut parts = record.splitn(format.len(), ',');
    let mut map: HashMap<&str, &str> = HashMap::new();
    for name in format {
        let value = parts.next().unwrap_or_default();
        map.insert(name.as_str(), value);
    }
    let start = time_ms(map.get("start")?.trim())?;
    let end = time_ms(map.get("end")?.trim())?;
    if end <= start {
        return None;
    }
    let style_name = map.get("style").copied().unwrap_or("Default").trim();
    let style = styles.get(style_name).copied().unwrap_or_default();
    let raw_text = map.get("text").copied().unwrap_or_default();
    let inline = ass_text(raw_text);
    if inline.text.is_empty() {
        return None;
    }
    let font_size = inline.font_size.unwrap_or(style.size);
    let align = Align::from_ass(inline.align.unwrap_or(style.align));
    let pos = inline.pos.map(|(x, y)| {
        (
            (x / play_res.0).clamp(0.0, 1.0),
            (y / play_res.1).clamp(0.0, 1.0),
        )
    });
    Some(Cue {
        start_ms: start,
        end_ms: end,
        text: inline.text,
        align,
        pos,
        font_percent: if play_res.1 > 0.0 {
            (font_size / play_res.1).clamp(0.0, 0.5)
        } else {
            0.0
        },
        color: inline.color.unwrap_or(style.color),
        color_explicit: true,
        opacity: style.opacity,
        bold: inline.bold.unwrap_or(style.bold),
        italic: inline.italic.unwrap_or(style.italic),
        border_percent: if play_res.1 > 0.0 {
            (inline.border.unwrap_or(style.outline) / play_res.1).clamp(0.0, 0.2)
        } else {
            0.0
        },
        shadow_percent: if play_res.1 > 0.0 {
            (inline.shadow.unwrap_or(style.shadow) / play_res.1).clamp(0.0, 0.2)
        } else {
            0.0
        },
        border_color: inline.border_color.unwrap_or(style.outline_color),
        shadow_color: inline.shadow_color.unwrap_or(style.back_color),
        shadow_opacity: inline.shadow_opacity.unwrap_or(style.back_opacity),
        scale: (
            inline.scale.0.unwrap_or(style.scale.0),
            inline.scale.1.unwrap_or(style.scale.1),
        ),
        rotation: inline.rotation.unwrap_or(style.rotation),
    })
}

struct InlineText {
    text: String,
    align: Option<u8>,
    pos: Option<(f32, f32)>,
    font_size: Option<f32>,
    color: Option<[u8; 3]>,
    bold: Option<bool>,
    italic: Option<bool>,
    border: Option<f32>,
    shadow: Option<f32>,
    border_color: Option<[u8; 3]>,
    shadow_color: Option<[u8; 3]>,
    shadow_opacity: Option<u8>,
    scale: (Option<f32>, Option<f32>),
    rotation: Option<f32>,
}

/// Strip ASS override blocks, keeping the three overrides the overlay can honour: alignment,
/// position and font size. `\N`/`\n` become line breaks, `\h` a hard space.
fn ass_text(raw: &str) -> InlineText {
    let mut out = String::with_capacity(raw.len());
    let mut align = None;
    let mut pos = None;
    let mut font_size = None;
    let mut color = None;
    let mut bold = None;
    let mut italic = None;
    let mut border = None;
    let mut shadow = None;
    let mut border_color = None;
    let mut shadow_color = None;
    let mut shadow_opacity = None;
    let mut scale_x = None;
    let mut scale_y = None;
    let mut rotation = None;
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                let mut block = String::new();
                for tag in chars.by_ref() {
                    if tag == '}' {
                        break;
                    }
                    block.push(tag);
                }
                let mut tags = block.as_str();
                while let Some(at) = tags.find('\\') {
                    tags = &tags[at + 1..];
                    let end = tags.find('\\').unwrap_or(tags.len());
                    let tag = &tags[..end];
                    if let Some(value) = tag.strip_prefix("an") {
                        if let Ok(value) = value.trim().parse::<u8>() {
                            align = Some(value.clamp(1, 9));
                        }
                    } else if let Some(value) = tag.strip_prefix("pos") {
                        if let Some((x, y)) = parse_pair(value) {
                            pos = Some((x, y));
                        }
                    } else if let Some(value) = tag.strip_prefix("fscx") {
                        if let Ok(value) = value.trim().parse::<f32>() {
                            scale_x = Some((value / 100.0).clamp(0.0, 8.0));
                        }
                    } else if let Some(value) = tag.strip_prefix("fscy") {
                        if let Ok(value) = value.trim().parse::<f32>() {
                            scale_y = Some((value / 100.0).clamp(0.0, 8.0));
                        }
                    } else if let Some(value) = tag.strip_prefix("fs") {
                        // `\fscx`/`\fscy` were handled above; only a plain number is a size.
                        if let Ok(value) = value.trim().parse::<f32>() {
                            font_size = Some(value);
                        }
                    } else if let Some(value) = tag.strip_prefix("bord") {
                        if let Ok(value) = value.trim().parse::<f32>() {
                            border = Some(value.max(0.0));
                        }
                    } else if let Some(value) = tag.strip_prefix("shad") {
                        let first = value.split(',').next().unwrap_or(value);
                        if let Ok(value) = first.trim().parse::<f32>() {
                            shadow = Some(value.max(0.0));
                        }
                    } else if let Some(value) =
                        tag.strip_prefix("frz").or_else(|| tag.strip_prefix("fr"))
                    {
                        if let Ok(value) = value.trim().parse::<f32>() {
                            rotation = Some(value);
                        }
                    } else if let Some(value) = tag.strip_prefix("3c") {
                        if let Some((rgb, _)) = ass_colour(value) {
                            border_color = Some(rgb);
                        }
                    } else if let Some(value) = tag.strip_prefix("4c") {
                        if let Some((rgb, opacity)) = ass_colour(value) {
                            shadow_color = Some(rgb);
                            shadow_opacity = Some(opacity);
                        }
                    } else if let Some(value) =
                        tag.strip_prefix("1c").or_else(|| tag.strip_prefix("c"))
                    {
                        if let Some((rgb, _)) = ass_colour(value) {
                            color = Some(rgb);
                        }
                    } else if let Some(value) = tag.strip_prefix('b') {
                        // `\bord` also starts with b; only a bare 0/1 is a weight switch.
                        match value.trim() {
                            "0" => bold = Some(false),
                            "1" => bold = Some(true),
                            _ => {}
                        }
                    } else if let Some(value) = tag.strip_prefix('i') {
                        match value.trim() {
                            "0" => italic = Some(false),
                            "1" => italic = Some(true),
                            _ => {}
                        }
                    }
                }
            }
            '\\' => match chars.next() {
                Some('N' | 'n') => out.push('\n'),
                Some('h') => out.push(' '),
                // Any other escape is a formatting command in a context we ignore.
                Some(_) | None => {}
            },
            '<' => {
                // HTML-ish tags appear in some converted tracks.
                for tag in chars.by_ref() {
                    if tag == '>' {
                        break;
                    }
                }
            }
            other => out.push(other),
        }
    }
    let text = out
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    InlineText {
        text,
        align,
        pos,
        font_size,
        color,
        bold,
        italic,
        border,
        shadow,
        border_color,
        shadow_color,
        shadow_opacity,
        scale: (scale_x, scale_y),
        rotation,
    }
}

/// `&HAABBGGRR` (or `&HBBGGRR`) to sRGB bytes and opacity. ASS stores transparency, so the
/// opacity is the inverse of the alpha byte.
fn ass_colour(value: &str) -> Option<([u8; 3], u8)> {
    let digits = value.trim().trim_start_matches("&H").trim_end_matches('&');
    let number = u32::from_str_radix(digits, 16).ok()?;
    let alpha = if digits.len() > 6 {
        ((number >> 24) & 0xFF) as u8
    } else {
        0
    };
    Some((
        [
            (number & 0xFF) as u8,
            ((number >> 8) & 0xFF) as u8,
            ((number >> 16) & 0xFF) as u8,
        ],
        255 - alpha,
    ))
}

fn parse_pair(value: &str) -> Option<(f32, f32)> {
    let inner = value.trim().strip_prefix('(')?.strip_suffix(')')?;
    let (x, y) = inner.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

/// Every cue covering `ms`, in start order. ASS can place a sign and a line at once.
pub fn cues_at(cues: &[Cue], ms: i64) -> Vec<&Cue> {
    let end = cues.partition_point(|cue| cue.start_ms <= ms);
    cues[..end]
        .iter()
        .filter(|cue| cue.end_ms > ms)
        .take(8)
        .collect()
}

/// The last cue that covers `ms`. Sorted starts make this a binary search over the prefix.
#[cfg(test)]
pub fn cue_at(cues: &[Cue], ms: i64) -> Option<&Cue> {
    let end = cues.partition_point(|cue| cue.start_ms <= ms);
    cues[..end].iter().rev().find(|cue| cue.end_ms > ms)
}

fn timing(line: &str) -> Option<(i64, i64)> {
    let (start, rest) = line.split_once("-->")?;
    let start = time_ms(start)?;
    // VTT allows cue settings after the end time (`align:start position:10%`).
    let end = rest.split_whitespace().next().and_then(time_ms)?;
    Some((start, end))
}

fn time_ms(value: &str) -> Option<i64> {
    let value = value.trim().replace(',', ".");
    let parts: Vec<&str> = value.split(':').collect();
    let (hours, minutes, seconds) = match parts.as_slice() {
        [h, m, s] => (
            h.parse::<i64>().ok()?,
            m.parse::<i64>().ok()?,
            s.parse::<f64>().ok()?,
        ),
        [m, s] => (0, m.parse::<i64>().ok()?, s.parse::<f64>().ok()?),
        _ => return None,
    };
    Some(hours * 3_600_000 + minutes * 60_000 + (seconds * 1000.0).round() as i64)
}

/// Drop SRT/HTML tags and ASS override blocks, and turn their line breaks into real ones.
struct PlainText {
    text: String,
    align: Align,
    color: Option<[u8; 3]>,
    bold: bool,
    italic: bool,
}

/// Strip SRT/VTT markup while keeping the facts the overlay can honour: line breaks, a cue
/// alignment (`{\an8}`), inline bold/italic, and a font colour. A cue is one styled block, so
/// a tag anywhere marks the whole cue; mixed-colour signs can only come from ASS.
fn clean(text: &str) -> PlainText {
    let mut out = String::with_capacity(text.len());
    let mut align = Align::BottomCenter;
    let mut color = None;
    let mut bold = false;
    let mut italic = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '<' => {
                let mut tag = String::new();
                for c in chars.by_ref() {
                    if c == '>' {
                        break;
                    }
                    tag.push(c);
                }
                let tag = tag.trim().to_ascii_lowercase();
                if tag == "b" || tag == "strong" {
                    bold = true;
                } else if tag == "i" || tag == "em" {
                    italic = true;
                } else if let Some(rest) = tag.strip_prefix("font color=") {
                    let value = rest.trim_matches(['"', '\'']);
                    if let Some(rgb) = parse_color(value) {
                        color = Some(rgb);
                    }
                } else if let Some(class) = tag.strip_prefix("c.")
                    && let Some(rgb) = color_name(class)
                {
                    color = Some(rgb);
                }
            }
            '{' => {
                let mut block = String::new();
                for tag in chars.by_ref() {
                    if tag == '}' {
                        break;
                    }
                    block.push(tag);
                }
                // SRT files sometimes carry the same `{\anN}` alignment tag as ASS.
                let mut rest = block.as_str();
                while let Some(at) = rest.find("\\an") {
                    let digits = &rest[at + 3..];
                    if let Some(value) = digits.chars().next().and_then(|c| c.to_digit(10))
                        && (1..=9).contains(&value)
                    {
                        align = Align::from_ass(value as u8);
                    }
                    rest = &digits[digits.len().min(1)..];
                }
            }
            '\\' => match chars.next() {
                // `\N` and `\n` are ASS line breaks; `\h` is a hard space.
                Some('N' | 'n') => out.push('\n'),
                Some('h') => out.push(' '),
                Some(other) => out.push(other),
                None => {}
            },
            '&' => {
                let rest: String = chars.clone().take(6).collect();
                if let Some((entity, replacement)) = [
                    ("amp;", "&"),
                    ("lt;", "<"),
                    ("gt;", ">"),
                    ("nbsp;", " "),
                    ("quot;", "\""),
                ]
                .into_iter()
                .find(|(entity, _)| rest.starts_with(entity))
                {
                    out.push_str(replacement);
                    for _ in 0..entity.chars().count() {
                        chars.next();
                    }
                } else {
                    out.push('&');
                }
            }
            other => out.push(other),
        }
    }
    PlainText {
        text: out
            .lines()
            .map(str::trim)
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string(),
        align,
        color,
        bold,
        italic,
    }
}

fn parse_color(value: &str) -> Option<[u8; 3]> {
    let hex = value
        .strip_prefix('#')
        .or_else(|| value.strip_prefix("0x"))?;
    match hex.len() {
        3 => Some([
            u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?,
            u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?,
            u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?,
        ]),
        6 => Some([
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
        ]),
        _ => None,
    }
}

fn color_name(name: &str) -> Option<[u8; 3]> {
    Some(match name.trim() {
        "white" => [255, 255, 255],
        "yellow" => [255, 226, 84],
        "cyan" => [128, 235, 255],
        "green" => [150, 245, 160],
        "magenta" | "pink" => [255, 170, 205],
        "red" => [255, 90, 90],
        "blue" => [120, 160, 255],
        "black" => [0, 0, 0],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srt_parses_comma_times_and_multiline_cues() {
        let srt = "\u{feff}1\r\n00:00:01,876 --> 00:00:05,422\r\nFirst line\r\nSecond line\r\n\r\n\
                   2\r\n00:00:06,000 --> 00:00:08,500\r\n<i>Italic</i> and {\\an8}placed\r\n";
        let cues = parse_text(srt);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].start_ms, 1_876);
        assert_eq!(cues[0].end_ms, 5_422);
        assert_eq!(cues[0].text, "First line\nSecond line");
        assert_eq!(cues[0].align, Align::BottomCenter);
        assert_eq!(cues[1].text, "Italic and placed");
    }

    #[test]
    fn srt_inline_tags_keep_bold_italic_colour_and_alignment() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\n{\\an8}<i>Italic</i> and <b>bold</b> <font color=\"#ff0000\">red</font>\n";
        let cues = parse_text(srt);
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].align, Align::TopCenter);
        assert!(cues[0].italic);
        assert!(cues[0].bold);
        assert_eq!(cues[0].color, [255, 0, 0]);
        assert!(cues[0].color_explicit);
        assert_eq!(cues[0].text, "Italic and bold red");
    }

    #[test]
    fn vtt_class_colours_and_short_hex_are_understood() {
        let vtt = "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\n<c.yellow>Warm</c>\n\n\
                   00:00:02.000 --> 00:00:03.000\n<font color=\"#0f0\">Green</font>\n";
        let cues = parse_text(vtt);
        assert_eq!(cues[0].color, [255, 226, 84]);
        assert!(cues[0].color_explicit);
        assert_eq!(cues[1].color, [0, 255, 0]);
    }

    #[test]
    fn vtt_skips_its_header_and_cue_settings() {
        let vtt = "WEBVTT\n\nRegion: id:bottom\n\nintro\n00:01.000 --> 00:03.500 align:center\nHello\n\n\
                   NOTE a comment\n\n00:00:04.000 --> 00:00:05.000\n<c.yellow>Bye</c>\n";
        let cues = parse_text(vtt);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].start_ms, 1_000);
        assert_eq!(cues[0].end_ms, 3_500);
        assert_eq!(cues[0].text, "Hello");
        assert_eq!(cues[1].text, "Bye");
    }

    #[test]
    fn ass_escapes_become_real_line_breaks() {
        let srt = "1\n00:00:10,000 --> 00:00:12,000\n{\\an8}Top\\Nline\\hand\\hspace\n";
        let cues = parse_text(srt);
        assert_eq!(cues[0].text, "Top\nline and space");
    }

    #[test]
    fn a_cue_is_found_at_the_right_millisecond() {
        let cues = parse_text(
            "1\n00:00:01,000 --> 00:00:03,000\nOne\n\n2\n00:00:03,000 --> 00:00:04,000\nTwo\n",
        );
        assert_eq!(cue_at(&cues, 500).map(|c| c.text.as_str()), None);
        assert_eq!(cue_at(&cues, 1_000).map(|c| c.text.as_str()), Some("One"));
        assert_eq!(cue_at(&cues, 2_999).map(|c| c.text.as_str()), Some("One"));
        assert_eq!(cue_at(&cues, 3_000).map(|c| c.text.as_str()), Some("Two"));
        assert_eq!(cue_at(&cues, 4_000).map(|c| c.text.as_str()), None);
    }

    #[test]
    fn garbage_blocks_are_ignored_not_fatal() {
        let cues = parse_text(
            "not a subtitle\n\n1\nbad timing\nText\n\n2\n00:00:01,000 --> 00:00:02,000\nGood\n",
        );
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "Good");
    }

    const ASS: &str = r#"[Script Info]
ScriptType: v4.00+
PlayResX: 1280
PlayResY: 720

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Dialogue,Constantia,53,&H00FFFFFF,&H0000FFFF,&H00000000,&H9B000000,0,0,0,0,100,100,0,0,1,2.5,1,2,15,15,20,0
Style: title,LeiSho,46,&H00DAD7D6,&H000000FF,&H00000000,&H00000000,0,0,0,0,85,100,0,0,1,0,0,8,10,10,10,1
Style: bold,Const,50,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,-1,1,0,0,100,100,0,0,1,2,1,2,15,15,20,0

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:02:06.93,0:02:10.02,Dialogue,,0000,0000,0000,,Smash the final glass pane
Dialogue: 0,0:11:37.67,0:11:40.67,title,,0000,0000,0000,,{\an3\fscx80\pos(535,697)}Episode {\fscx40}Twenty-Six
Dialogue: 0,0:14:30.84,0:14:33.43,bold,,0000,0000,0000,,{\shad0\bord0\fs50\an4\c&H231E13&\b0}Umitsuki\NApartments
Comment: 0,0:00:00.00,0:00:00.00,Dialogue,,0000,0000,0000,,
"#;

    #[test]
    fn ass_keeps_alignment_position_and_font_size() {
        let cues = parse_ass_text(ASS);
        assert_eq!(cues.len(), 3);
        assert_eq!(cues[0].text, "Smash the final glass pane");
        assert_eq!(cues[0].align, Align::BottomCenter);
        assert!((cues[0].font_percent - 53.0 / 720.0).abs() < 0.001);
        assert_eq!(cues[0].color, [255, 255, 255]);
        assert_eq!(cues[0].opacity, 255);

        // `\an3` overrides the style's top-centre alignment and `\pos` anchors it.
        assert_eq!(cues[1].align, Align::BottomRight);
        assert_eq!(cues[1].text, "Episode Twenty-Six");
        assert_eq!(cues[1].color, [214, 215, 218], "the title style's grey");
        let (x, y) = cues[1].pos.expect("an explicit position");
        assert!((x - 535.0 / 1280.0).abs() < 0.001);
        assert!((y - 697.0 / 720.0).abs() < 0.001);

        // `\fs` overrides the style size and `\N` is a line break.
        assert_eq!(cues[2].align, Align::MiddleLeft);
        assert_eq!(cues[2].text, "Umitsuki\nApartments");
        assert!((cues[2].font_percent - 50.0 / 720.0).abs() < 0.001);
        assert_eq!(cues[2].color, [19, 30, 35], "the inline \\c override");
        assert!(!cues[2].bold, "the style is bold but \\b0 wins");
    }

    #[test]
    fn ass_outline_shadow_scale_and_rotation_are_read() {
        let script = r#"[Script Info]
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Signs,Arial,60,&H00FFFFFF,&H000000FF,&H0000FFFF,&H80000000,0,0,0,0,120,90,0,6,1,4,2,8,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:04.00,Signs,,0000,0000,0000,,{\frz12\3c&HFF0000&}A sign
Dialogue: 0,0:00:05.00,0:00:08.00,Signs,,0000,0000,0000,,{\bord0\shad0\frz0\fscx100\fscy100}Plain
"#;
        let cues = parse_ass_text(script);
        assert_eq!(cues.len(), 2);
        assert!((cues[0].border_percent - 4.0 / 1080.0).abs() < 0.0001);
        assert!((cues[0].shadow_percent - 2.0 / 1080.0).abs() < 0.0001);
        assert_eq!(
            cues[0].border_color,
            [0, 0, 255],
            "\\3c overrides the outline"
        );
        assert_eq!(cues[0].shadow_color, [0, 0, 0]);
        assert_eq!(
            cues[0].shadow_opacity, 127,
            "the style's back alpha becomes opacity"
        );
        assert_eq!(cues[0].scale, (1.2, 0.9));
        assert_eq!(cues[0].rotation, 12.0);
        // The second event switches every one of those off, which is what signs use.
        assert_eq!(cues[1].border_percent, 0.0);
        assert_eq!(cues[1].shadow_percent, 0.0);
        assert_eq!(cues[1].scale, (1.0, 1.0));
        assert_eq!(cues[1].rotation, 0.0);
    }

    #[test]
    fn two_ass_events_can_cover_the_same_instant() {
        let cues = parse_ass_text(ASS);
        let active = cues_at(&cues, 11 * 60_000 + 38_000);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].text, "Episode Twenty-Six");
        assert!(cues_at(&cues, 1_000).is_empty());
    }
}
