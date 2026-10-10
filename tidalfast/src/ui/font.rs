//! Pixel font engine: a hand-made 5x7 bitmap font (upper + lower case, proportional spacing),
//! packed into a small texture so every character is a single crisp textured quad.

use eframe::egui::{self, Align, Color32, Pos2, Rect, Vec2};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::OnceLock;

// ------------------------------------------------------------ modern (smooth) text
static MODERN: AtomicBool = AtomicBool::new(false);
static CTX: OnceLock<egui::Context> = OnceLock::new();

/// Modern skins draw text with a smooth proportional font instead of the pixel font.
pub fn set_modern(on: bool) {
    MODERN.store(on, Ordering::Relaxed);
}

fn modern() -> bool {
    MODERN.load(Ordering::Relaxed) && CTX.get().is_some()
}

fn mfont(px: f32) -> egui::FontId {
    egui::FontId::proportional((px * 7.4 * ui_scale()).max(9.0))
}

// ------------------------------------------------------------ panel scale
thread_local! {
    static SCALE: std::cell::Cell<f32> = const { std::cell::Cell::new(1.0) };
}

/// How much bigger (or smaller) text and controls are drawn right now: 1 normally; a panel that scales its
/// contents with its size (the practice panel) sets it while it draws (see `with_scale`).
pub fn ui_scale() -> f32 {
    SCALE.with(|s| s.get())
}

/// Draw `f` with everything (text, buttons, icons) `s` times its usual size.
pub fn with_scale<R>(s: f32, f: impl FnOnce() -> R) -> R {
    let was = SCALE.with(|c| c.replace(s));
    let r = f();
    SCALE.with(|c| c.set(was));
    r
}

fn mw(text: &str, px: f32) -> f32 {
    match CTX.get() {
        Some(c) => c.fonts(|f| f.layout_no_wrap(text.to_string(), mfont(px), Color32::WHITE).size().x),
        None => text.chars().count() as f32 * px * 4.0,
    }
}

// ------------------------------------------------------------ screen scale
static PPP_BITS: AtomicU32 = AtomicU32::new(0x3F80_0000); // 1.0f32

pub fn set_ppp(v: f32) {
    PPP_BITS.store(v.max(0.5).to_bits(), Ordering::Relaxed);
}

pub fn ppp() -> f32 {
    f32::from_bits(PPP_BITS.load(Ordering::Relaxed))
}

/// Snap a coordinate to a whole physical pixel.
pub fn snap(v: f32) -> f32 {
    let p = ppp();
    (v * p).round() / p
}

/// A length of about `k` points, but never thinner than one physical pixel and always whole pixels.
pub fn thick(k: f32) -> f32 {
    let p = ppp();
    (k * p).round().max(1.0) / p
}

/// Pixel size of one font dot: always a whole number of physical pixels (and as scaled as the panel drawing it).
pub fn spx(px: f32) -> f32 {
    thick(px * ui_scale())
}

/// Width of pixel-font text whose dots are already `d` wide.
fn pw(text: &str, d: f32) -> f32 {
    let n: f32 = text.chars().map(adv).sum();
    if n == 0.0 {
        0.0
    } else {
        n * d - d
    }
}

// ------------------------------------------------------------------ glyphs
fn g7(a: [u8; 7]) -> [u8; 9] {
    [a[0], a[1], a[2], a[3], a[4], a[5], a[6], 0, 0]
}

/// 5 columns (bit 4 = leftmost) x 9 rows. Caps/digits use rows 0-6, lowercase x-height is rows 2-6,
/// descenders go down to row 8.
pub fn glyph(c: char) -> [u8; 9] {
    match c {
        'A' => g7([0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
        'B' => g7([0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110]),
        'C' => g7([0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110]),
        'D' => g7([0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110]),
        'E' => g7([0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111]),
        'F' => g7([0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000]),
        'G' => g7([0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111]),
        'H' => g7([0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
        'I' => g7([0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
        'J' => g7([0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100]),
        'K' => g7([0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001]),
        'L' => g7([0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111]),
        'M' => g7([0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001]),
        'N' => g7([0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001]),
        'O' => g7([0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
        'P' => g7([0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000]),
        'Q' => g7([0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101]),
        'R' => g7([0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001]),
        'S' => g7([0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110]),
        'T' => g7([0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]),
        'U' => g7([0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
        'V' => g7([0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100]),
        'W' => g7([0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010]),
        'X' => g7([0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001]),
        'Y' => g7([0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100]),
        'Z' => g7([0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111]),
        '0' => g7([0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110]),
        '1' => g7([0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
        '2' => g7([0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111]),
        '3' => g7([0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110]),
        '4' => g7([0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010]),
        '5' => g7([0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110]),
        '6' => g7([0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110]),
        '7' => g7([0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000]),
        '8' => g7([0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110]),
        '9' => g7([0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100]),
        ' ' => g7([0, 0, 0, 0, 0, 0, 0]),
        '.' => g7([0, 0, 0, 0, 0, 0b01100, 0b01100]),
        ',' => g7([0, 0, 0, 0, 0b01100, 0b00100, 0b01000]),
        ':' => g7([0, 0b01100, 0b01100, 0, 0b01100, 0b01100, 0]),
        ';' => g7([0, 0b01100, 0b01100, 0, 0b01100, 0b00100, 0b01000]),
        '-' => g7([0, 0, 0, 0b11111, 0, 0, 0]),
        '_' => g7([0, 0, 0, 0, 0, 0, 0b11111]),
        '(' => g7([0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010]),
        ')' => g7([0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000]),
        '[' => g7([0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110]),
        ']' => g7([0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110]),
        '/' => g7([0b00001, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b10000]),
        '%' => g7([0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011]),
        '+' => g7([0, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0]),
        '=' => g7([0, 0, 0b11111, 0, 0b11111, 0, 0]),
        '\'' => g7([0b00100, 0b00100, 0b01000, 0, 0, 0, 0]),
        '"' => g7([0b01010, 0b01010, 0b01010, 0, 0, 0, 0]),
        '!' => g7([0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0, 0b00100]),
        '<' => g7([0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010]),
        '>' => g7([0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000]),
        '#' => g7([0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010]),
        '&' => g7([0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101]),
        '*' => g7([0, 0b00100, 0b10101, 0b01110, 0b10101, 0b00100, 0]),
        '|' => g7([0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]),
        '@' => g7([0b01110, 0b10001, 0b10111, 0b10101, 0b10111, 0b10000, 0b01110]),
        '$' => g7([0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100]),
        'a' => [0, 0, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111, 0, 0],
        'b' => [0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b11110, 0, 0],
        'c' => [0, 0, 0b01110, 0b10001, 0b10000, 0b10001, 0b01110, 0, 0],
        'd' => [0b00001, 0b00001, 0b01111, 0b10001, 0b10001, 0b10001, 0b01111, 0, 0],
        'e' => [0, 0, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110, 0, 0],
        'f' => [0b00110, 0b01000, 0b01000, 0b11110, 0b01000, 0b01000, 0b01000, 0, 0],
        'g' => [0, 0, 0b01111, 0b10001, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
        'h' => [0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001, 0, 0],
        'i' => [0b00100, 0, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110, 0, 0],
        'j' => [0b00010, 0, 0b00110, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'k' => [0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0, 0],
        'l' => [0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110, 0, 0],
        'm' => [0, 0, 0b11010, 0b10101, 0b10101, 0b10101, 0b10101, 0, 0],
        'n' => [0, 0, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001, 0, 0],
        'o' => [0, 0, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110, 0, 0],
        'p' => [0, 0, 0b11110, 0b10001, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000],
        'q' => [0, 0, 0b01111, 0b10001, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001],
        'r' => [0, 0, 0b10110, 0b11001, 0b10000, 0b10000, 0b10000, 0, 0],
        's' => [0, 0, 0b01111, 0b10000, 0b01110, 0b00001, 0b11110, 0, 0],
        't' => [0, 0b01000, 0b11110, 0b01000, 0b01000, 0b01001, 0b00110, 0, 0],
        'u' => [0, 0, 0b10001, 0b10001, 0b10001, 0b10011, 0b01101, 0, 0],
        'v' => [0, 0, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100, 0, 0],
        'w' => [0, 0, 0b10001, 0b10001, 0b10101, 0b10101, 0b01010, 0, 0],
        'x' => [0, 0, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0, 0],
        'y' => [0, 0, 0b10001, 0b10001, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
        'z' => [0, 0, 0b11111, 0b00010, 0b00100, 0b01000, 0b11111, 0, 0],
        '~' => g7([0, 0, 0b01000, 0b10101, 0b00010, 0, 0]),
        '^' => g7([0b00100, 0b01010, 0b10001, 0, 0, 0, 0]),
        '`' => g7([0b01000, 0b00100, 0, 0, 0, 0, 0]),
        '\\' => g7([0b10000, 0b10000, 0b01000, 0b00100, 0b00010, 0b00001, 0b00001]),
        '{' => g7([0b00110, 0b00100, 0b00100, 0b01000, 0b00100, 0b00100, 0b00110]),
        '}' => g7([0b01100, 0b00100, 0b00100, 0b00010, 0b00100, 0b00100, 0b01100]),
        _ => g7([0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0, 0b00100]), // '?'
    }
}

/// Map any character to something the font has (accents folded, the rest becomes '?').
pub fn norm(c: char) -> char {
    let c = match c {
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
        'Ç' => 'C',
        'È' | 'É' | 'Ê' | 'Ë' => 'E',
        'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
        'Ñ' => 'N',
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => 'O',
        'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
        'Ý' => 'Y',
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ý' | 'ÿ' => 'y',
        'ß' => 's',
        '’' | '‘' | '´' => '\'',
        '“' | '”' => '"',
        '–' | '—' => '-',
        '…' | '·' | '•' => '.',
        '\u{a0}' => ' ',
        _ => c,
    };
    if (' '..='~').contains(&c) {
        c
    } else {
        '?'
    }
}

const FIRST: u32 = 32;
const COUNT: usize = 95;

/// (left column, width) of the inked part of each glyph; digits are tabular (full width).
fn metrics() -> &'static [(u8, u8); COUNT] {
    static M: OnceLock<[(u8, u8); COUNT]> = OnceLock::new();
    M.get_or_init(|| {
        let mut m = [(0u8, 3u8); COUNT];
        for i in 0..COUNT {
            let c = char::from_u32(FIRST + i as u32).unwrap_or('?');
            let g = glyph(c);
            let all = g.iter().fold(0u8, |a, r| a | r);
            if all == 0 {
                m[i] = (0, 3);
            } else if c.is_ascii_digit() {
                m[i] = (0, 5);
            } else {
                let mut l = 0u8;
                while all & (0x10u8 >> l) == 0 {
                    l += 1;
                }
                let mut r = 4u8;
                while all & (0x10u8 >> r) == 0 {
                    r -= 1;
                }
                m[i] = (l, r - l + 1);
            }
        }
        m
    })
}

fn met(c: char) -> (u8, u8) {
    metrics()[(norm(c) as u32 - FIRST) as usize]
}

/// Advance of one character in font dots (ink width + 1 dot gap).
fn adv(c: char) -> f32 {
    met(c).1 as f32 + 1.0
}

/// With a Winamp skin on, all text takes the skin's playlist font (drawn the Winamp way); Tidalite's own pixel
/// size `px` maps to about this many points.
fn skin_size(px: f32) -> f32 {
    // never below 12 points, so small labels stay readable
    (px * 8.0).max(12.0) * ui_scale()
}

fn skinned() -> bool {
    crate::winamp_ui::text_override()
}

/// Text width for the smooth-font looks (the modern skins, or a Winamp skin's playlist font).
fn sw(text: &str, px: f32) -> f32 {
    if skinned() {
        crate::winamp_ui::sans_width(text, skin_size(px))
    } else {
        mw(text, px)
    }
}

pub fn text_w(text: &str, px: f32) -> f32 {
    if text.contains(DRAWN) {
        return drawn_split(text).iter().map(|s| s.map_or_else(|c| drawn_w(c, px), |s| text_w(s, px))).sum();
    }
    if skinned() || modern() {
        return sw(text, px);
    }
    pw(text, spx(px))
}

// ------------------------------------------------------------------ atlas
const CW: usize = 8;
const CH: usize = 10;
const COLS: usize = 16;
const ROWS: usize = 6;
const TW: usize = COLS * CW;
const TH: usize = ROWS * CH;

static TEX: OnceLock<egui::TextureId> = OnceLock::new();

/// Build the glyph atlas. Keep the returned handle alive for as long as the app runs.
pub fn init(ctx: &egui::Context) -> egui::TextureHandle {
    let mut img = egui::ColorImage::new([TW, TH], Color32::TRANSPARENT);
    for i in 0..COUNT {
        let c = char::from_u32(FIRST + i as u32).unwrap_or('?');
        let g = glyph(c);
        let (ox, oy) = ((i % COLS) * CW, (i / COLS) * CH);
        for (ry, row) in g.iter().enumerate() {
            for cx in 0..5usize {
                if row & (0x10u8 >> cx) != 0 {
                    img.pixels[(oy + ry) * TW + ox + cx] = Color32::WHITE;
                }
            }
        }
    }
    let h = ctx.load_texture("tidalite-font", img, egui::TextureOptions::NEAREST);
    let _ = TEX.set(h.id());
    let _ = CTX.set(ctx.clone());
    h
}

// ------------------------------------------------------------------ drawing
fn draw_char(p: &egui::Painter, tex: egui::TextureId, c: char, x: f32, y: f32, px: f32, col: Color32) {
    let c = norm(c);
    let i = (c as u32 - FIRST) as usize;
    let (l, w) = metrics()[i];
    if w == 0 || c == ' ' {
        return;
    }
    let (ox, oy) = ((i % COLS) * CW + l as usize, (i / COLS) * CH);
    let uv = Rect::from_min_max(
        Pos2::new(ox as f32 / TW as f32, oy as f32 / TH as f32),
        Pos2::new((ox + w as usize) as f32 / TW as f32, (oy + 9) as f32 / TH as f32),
    );
    let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w as f32 * px, 9.0 * px));
    p.image(tex, r, uv, col);
}

/// Draw bitmap text. `anchor` is the left/center/right edge at the vertical center of the capitals.
/// Returns the text width.
pub fn ptext(p: &egui::Painter, anchor: Pos2, h: Align, text: &str, px: f32, color: Color32) -> f32 {
    if text.contains(DRAWN) {
        return ptext_drawn(p, anchor, h, text, px, color);
    }
    if skinned() {
        return crate::winamp_ui::sans_text(p, anchor, h, text, skin_size(px), f32::INFINITY, color);
    }
    if modern() {
        let w = mw(text, px);
        let al = match h {
            Align::Min => egui::Align2::LEFT_CENTER,
            Align::Center => egui::Align2::CENTER_CENTER,
            Align::Max => egui::Align2::RIGHT_CENTER,
        };
        // egui's default face is thin; a second pass half a pixel over gives it the weight small text needs
        p.text(Pos2::new(anchor.x + 0.45, anchor.y + 0.5), al, text, mfont(px), color);
        p.text(Pos2::new(anchor.x, anchor.y + 0.5), al, text, mfont(px), color);
        return w;
    }
    draw_dots(p, anchor, h, text, spx(px), color)
}

/// Pixel-font text whose dots are already `px` wide.
fn draw_dots(p: &egui::Painter, anchor: Pos2, h: Align, text: &str, px: f32, color: Color32) -> f32 {
    let w = pw(text, px);
    let Some(tex) = TEX.get().copied() else { return w };
    let mut x = snap(match h {
        Align::Min => anchor.x,
        Align::Center => anchor.x - w / 2.0,
        Align::Max => anchor.x - w,
    });
    let y = snap(anchor.y - 3.5 * px);
    let clip = p.clip_rect();
    for c in text.chars() {
        let a = adv(c) * px;
        // skip what is entirely outside the clip rectangle (long lists, scrolling text)
        if x + a >= clip.min.x && x <= clip.max.x {
            draw_char(p, tex, c, x, y, px, color);
        }
        x += a;
    }
    w
}

// ------------------------------------------------------------------ drawn symbols
// The chord symbols no font draws well (a triangle for major 7, a small circle for diminished) are drawn as shapes,
// sized to the capitals of whatever font is on.
const DRAWN: [char; 2] = ['\u{0394}', '\u{00b0}'];

/// Text cut into runs of plain text (Ok) and drawn symbols (Err).
fn drawn_split(text: &str) -> Vec<Result<&str, char>> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, c) in text.char_indices() {
        if DRAWN.contains(&c) {
            if i > start {
                out.push(Ok(&text[start..i]));
            }
            out.push(Err(c));
            start = i + c.len_utf8();
        }
    }
    if start < text.len() {
        out.push(Ok(&text[start..]));
    }
    out
}

/// The height of the capitals, as drawn.
fn cap_h(px: f32) -> f32 {
    if skinned() || modern() {
        text_w("H", px) * 1.15
    } else {
        7.0 * spx(px)
    }
}

fn drawn_w(c: char, px: f32) -> f32 {
    let h = cap_h(px);
    if c == '\u{00b0}' {
        h * 0.6
    } else {
        h * 1.05 + h * 0.2
    }
}

fn ptext_drawn(p: &egui::Painter, anchor: Pos2, h: Align, text: &str, px: f32, color: Color32) -> f32 {
    let w = text_w(text, px);
    let mut x = match h {
        Align::Min => anchor.x,
        Align::Center => anchor.x - w / 2.0,
        Align::Max => anchor.x - w,
    };
    let ch = cap_h(px);
    let line = (ch * 0.14).max(1.2);
    for run in drawn_split(text) {
        match run {
            Ok(s) => x += ptext(p, Pos2::new(x, anchor.y), Align::Min, s, px, color),
            Err('\u{00b0}') => {
                // a small ring at the top of the capitals
                let r = ch * 0.2;
                p.circle_stroke(Pos2::new(x + ch * 0.3, anchor.y - ch * 0.5 + r), r, egui::Stroke::new(line * 0.8, color));
                x += drawn_w('\u{00b0}', px);
            }
            Err(c) => {
                // an upright triangle, as tall as the capitals
                let (l, r, top, bot) = (x + ch * 0.1, x + ch * 1.15, anchor.y - ch * 0.5, anchor.y + ch * 0.5);
                let pts = vec![Pos2::new((l + r) / 2.0, top), Pos2::new(r, bot), Pos2::new(l, bot)];
                p.add(egui::Shape::closed_line(pts, egui::Stroke::new(line, color)));
                x += drawn_w(c, px);
            }
        }
    }
    w
}

/// Like `ptext`, but shrinks the dot size until the text fits `max_w`.
pub fn ptext_fit(p: &egui::Painter, anchor: Pos2, h: Align, text: &str, px: f32, max_w: f32, color: Color32) {
    if text.contains(DRAWN) {
        let mut k = px;
        while k > px * 0.5 && text_w(text, k) > max_w {
            k -= 0.05;
        }
        ptext(p, anchor, h, text, k, color);
        return;
    }
    if skinned() {
        crate::winamp_ui::sans_text(p, anchor, h, text, skin_size(px), max_w, color);
        return;
    }
    if modern() {
        let mut k = px;
        while k > px * 0.55 && mw(text, k) > max_w {
            k -= 0.05;
        }
        ptext(p, anchor, h, text, k, color);
        return;
    }
    let mut k = spx(px);
    let step = thick(1.0);
    while k > thick(1.0) && pw(text, k) > max_w {
        k -= step;
    }
    draw_dots(p, anchor, h, text, k, color);
}

/// Shorten with "..." so the text fits `max_w`.
pub fn fit(text: &str, px: f32, max_w: f32) -> String {
    if text_w(text, px) <= max_w {
        return text.to_string();
    }
    if skinned() || modern() {
        let chars: Vec<char> = text.chars().collect();
        let (mut lo, mut hi) = (0usize, chars.len());
        while lo < hi {
            let mid = (lo + hi + 1) / 2;
            let t: String = chars[..mid].iter().collect::<String>().trim_end().to_string() + "...";
            if sw(&t, px) <= max_w {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        let t: String = chars[..lo].iter().collect();
        return format!("{}...", t.trim_end());
    }
    let px = spx(px);
    let limit = max_w - pw("...", px) - px;
    let mut acc = 0.0f32;
    let mut end = 0usize;
    for (i, c) in text.char_indices() {
        let a = adv(c) * px;
        if acc + a - px > limit {
            break;
        }
        acc += a;
        end = i + c.len_utf8();
    }
    format!("{}...", text[..end].trim_end())
}

/// Greedy word wrap.
pub fn wrap(text: &str, px: f32, max_w: f32) -> Vec<String> {
    if skinned() || modern() {
        let mut out: Vec<String> = Vec::new();
        for raw in text.split('\n') {
            let mut line = String::new();
            for word in raw.split(' ') {
                let t = if line.is_empty() { word.to_string() } else { format!("{} {}", line, word) };
                if sw(&t, px) > max_w && !line.is_empty() {
                    out.push(std::mem::take(&mut line));
                    line = word.to_string();
                } else {
                    line = t;
                }
            }
            out.push(line);
        }
        return out;
    }
    let px = spx(px);
    let mut out: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let mut line = String::new();
        let mut w = 0.0f32;
        for c in raw.chars() {
            let a = adv(c) * px;
            if w + a - px > max_w && !line.is_empty() {
                match line.rfind(' ') {
                    Some(i) if i > 0 => {
                        let rest = line[i + 1..].to_string();
                        line.truncate(i);
                        out.push(line);
                        line = rest;
                    }
                    _ => out.push(std::mem::take(&mut line)),
                }
                w = line.chars().map(adv).sum::<f32>() * px;
            }
            line.push(c);
            w += a;
        }
        out.push(line);
    }
    out
}
