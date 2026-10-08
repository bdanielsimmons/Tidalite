#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod api;
mod player;

use api::{cover_url, Api, Card, Kind, Page, Track};
use eframe::egui::{self, Align, Align2, Color32, FontId, Pos2, Rect, RichText, Rounding, Sense, Stroke, Vec2};
use player::{Cmd, Player};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ------------------------------------------------------- classic-skin palette
const APP_BG: Color32 = Color32::from_rgb(8, 8, 10);
const TRIM: Color32 = Color32::from_rgb(186, 184, 136);
const BEIGE: Color32 = Color32::from_rgb(201, 199, 160);
const BEIGE_LT: Color32 = Color32::from_rgb(226, 224, 188);
const BEIGE_DK: Color32 = Color32::from_rgb(160, 157, 118);
const LCD: Color32 = Color32::from_rgb(212, 210, 172);
const LCD_GHOST: Color32 = Color32::from_rgb(190, 188, 150);
const GROOVE: Color32 = Color32::from_rgb(120, 118, 84);
const SEL: Color32 = Color32::from_rgb(168, 165, 128);
const INK: Color32 = Color32::from_rgb(12, 12, 8);
const INK2: Color32 = Color32::from_rgb(70, 70, 50);
const RED: Color32 = Color32::from_rgb(150, 30, 20);
const BTN_FACE: Color32 = Color32::from_rgb(150, 146, 92);
const BTN_HI: Color32 = Color32::from_rgb(190, 186, 128);

const LIB_W: f32 = 430.0;
const NB: usize = 19;

// ------------------------------------------------------------------ messages
enum Msg {
    Code(String, String),
    LoggedIn,
    NeedLogin(String),
    Page(Page, bool),
    Err(String),
    Audio(u64, i64, Vec<u8>),
    PlayErr(u64, String),
    Prefetched(i64, Vec<u8>),
    Img(String, Option<egui::ColorImage>),
}

enum Action {
    Open(Card),
    Home,
    Library,
    Search(String),
    Back,
    Play(Vec<Track>, usize),
    PlayIndex(usize),
    PlayNext(Track),
    Enqueue(Track),
    ClearQueue,
    PlayBtn,
    PauseBtn,
    StopBtn,
    Toggle,
    Next,
    Prev,
    Seek(f32),
    Volume(f32),
    Shuffle,
    Repeat,
    ToggleLossless,
    ClearStatus,
    Logout,
    StartLogin,
}

#[derive(PartialEq, Clone, Copy)]
enum Auth {
    Checking,
    LoggedOut,
    LoggingIn,
    In,
}

#[derive(PartialEq, Clone, Copy)]
enum Repeat {
    Off,
    All,
    One,
}

// ------------------------------------------------------------- bitmap font
// Classic 5x7 pixel font, drawn as rectangles so it scales crisply.
fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        ' ' => [0, 0, 0, 0, 0, 0, 0],
        '.' => [0, 0, 0, 0, 0, 0b01100, 0b01100],
        ',' => [0, 0, 0, 0, 0b01100, 0b00100, 0b01000],
        ':' => [0, 0b01100, 0b01100, 0, 0b01100, 0b01100, 0],
        ';' => [0, 0b01100, 0b01100, 0, 0b01100, 0b00100, 0b01000],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        '_' => [0, 0, 0, 0, 0, 0, 0b11111],
        '(' => [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
        ')' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
        '[' => [0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110],
        ']' => [0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110],
        '/' => [0b00001, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b10000],
        '%' => [0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011],
        '+' => [0, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0],
        '=' => [0, 0, 0b11111, 0, 0b11111, 0, 0],
        '\'' => [0b00100, 0b00100, 0b01000, 0, 0, 0, 0],
        '"' => [0b01010, 0b01010, 0b01010, 0, 0, 0, 0],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0, 0b00100],
        '<' => [0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010],
        '>' => [0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000],
        '#' => [0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010],
        '&' => [0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101],
        '*' => [0, 0b00100, 0b10101, 0b01110, 0b10101, 0b00100, 0],
        '|' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        '@' => [0b01110, 0b10001, 0b10111, 0b10101, 0b10111, 0b10000, 0b01110],
        '$' => [0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100],
        _ => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0, 0b00100], // '?'
    }
}

fn fold(c: char) -> char {
    match c {
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
        'Ç' => 'C',
        'È' | 'É' | 'Ê' | 'Ë' => 'E',
        'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
        'Ñ' => 'N',
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => 'O',
        'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
        'Ý' => 'Y',
        '’' | '‘' => '\'',
        '“' | '”' => '"',
        '–' | '—' => '-',
        _ => c,
    }
}

fn ptext_w(text: &str, px: f32) -> f32 {
    let n = text.chars().flat_map(|c| c.to_uppercase()).count();
    if n == 0 {
        0.0
    } else {
        n as f32 * 6.0 * px - px
    }
}

/// Draw bitmap text. `anchor` is the left/center/right edge at the text's vertical center.
fn ptext(p: &egui::Painter, anchor: Pos2, h: Align, text: &str, px: f32, color: Color32) -> f32 {
    let w = ptext_w(text, px);
    let mut x = match h {
        Align::Min => anchor.x,
        Align::Center => anchor.x - w / 2.0,
        Align::Max => anchor.x - w,
    }
    .round();
    let y = (anchor.y - 3.5 * px).round();
    for c in text.chars().flat_map(|c| c.to_uppercase()) {
        let g = glyph(fold(c));
        for (ry, row) in g.iter().enumerate() {
            let mut cx = 0u32;
            while cx < 5 {
                if row & (0x10u8 >> cx) != 0 {
                    let start = cx;
                    while cx < 5 && row & (0x10u8 >> cx) != 0 {
                        cx += 1;
                    }
                    let r = Rect::from_min_size(
                        Pos2::new(x + start as f32 * px, y + ry as f32 * px),
                        Vec2::new((cx - start) as f32 * px, px),
                    );
                    p.rect_filled(r, Rounding::same(0.0), color);
                } else {
                    cx += 1;
                }
            }
        }
        x += 6.0 * px;
    }
    w
}

/// Scrolling bitmap text inside `rect` (static if it fits).
fn marquee(ui: &egui::Ui, rect: Rect, text: &str, px: f32, color: Color32) {
    let w = ptext_w(text, px);
    let clip = ui.painter().with_clip_rect(rect.shrink2(Vec2::new(2.0, 0.0)));
    let cy = rect.center().y;
    if w <= rect.width() - 8.0 {
        ptext(&clip, Pos2::new(rect.min.x + 5.0, cy), Align::Min, text, px, color);
    } else {
        let period = w + 8.0 * px;
        let t = ui.input(|i| i.time) as f32;
        let off = (t * 9.0 * px) % period;
        let x = rect.min.x + 5.0 - off;
        ptext(&clip, Pos2::new(x, cy), Align::Min, text, px, color);
        ptext(&clip, Pos2::new(x + period, cy), Align::Min, text, px, color);
    }
}

// --------------------------------------------------------------- pixel icons
const PREV: [&str; 7] = ["#.....#", "#....##", "#...###", "#..####", "#...###", "#....##", "#.....#"];
const NEXT: [&str; 7] = ["#.....#", "##....#", "###...#", "####..#", "###...#", "##....#", "#.....#"];
const PLAY: [&str; 7] = ["##.....", "####...", "######.", "#######", "######.", "####...", "##....."];
const PAUSE: [&str; 7] = ["###.###", "###.###", "###.###", "###.###", "###.###", "###.###", "###.###"];
const STOP: [&str; 7] = [".......", ".#####.", ".#####.", ".#####.", ".#####.", ".#####.", "......."];
const PLAY_S: [&str; 5] = ["#....", "##...", "###..", "##...", "#...."];
const PAUSE_S: [&str; 5] = ["##.##", "##.##", "##.##", "##.##", "##.##"];
const STOP_S: [&str; 5] = ["#####", "#####", "#####", "#####", "#####"];

fn pixmap(p: &egui::Painter, origin: Pos2, px: f32, rows: &[&str], color: Color32) {
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            if ch == '#' {
                let min = origin + Vec2::new(x as f32 * px, y as f32 * px);
                p.rect_filled(Rect::from_min_size(min, Vec2::splat(px)), Rounding::same(0.0), color);
            }
        }
    }
}

// ------------------------------------------------------------ skin drawing
fn fill_rect(p: &egui::Painter, r: Rect, c: Color32) {
    p.rect_filled(r, Rounding::same(0.0), c);
}

/// Sunken panel (LCD windows, groove).
fn inset(p: &egui::Painter, r: Rect, fill: Color32) {
    fill_rect(p, r, fill);
    fill_rect(p, Rect::from_min_size(r.min, Vec2::new(r.width(), 1.5)), INK);
    fill_rect(p, Rect::from_min_size(r.min, Vec2::new(1.5, r.height())), INK);
    fill_rect(p, Rect::from_min_size(Pos2::new(r.min.x, r.max.y - 1.0), Vec2::new(r.width(), 1.0)), BEIGE_LT);
    fill_rect(p, Rect::from_min_size(Pos2::new(r.max.x - 1.0, r.min.y), Vec2::new(1.0, r.height())), BEIGE_LT);
}

/// Raised bevelled button face.
fn raised(p: &egui::Painter, r: Rect, down: bool) {
    let (hi, lo) = if down { (BEIGE_DK, BEIGE_LT) } else { (BEIGE_LT, BEIGE_DK) };
    fill_rect(p, r, if down { SEL } else { BEIGE });
    fill_rect(p, Rect::from_min_size(r.min, Vec2::new(r.width(), 1.5)), hi);
    fill_rect(p, Rect::from_min_size(r.min, Vec2::new(1.5, r.height())), hi);
    fill_rect(p, Rect::from_min_size(Pos2::new(r.min.x, r.max.y - 1.5), Vec2::new(r.width(), 1.5)), lo);
    fill_rect(p, Rect::from_min_size(Pos2::new(r.max.x - 1.5, r.min.y), Vec2::new(1.5, r.height())), lo);
    p.rect_stroke(r, Rounding::same(0.0), Stroke::new(1.0, INK));
}

/// Black rounded window frame with beige trim, title tab and beige body at `inner`.
fn window_deco(ui: &egui::Ui, inner: Rect, title: &str) {
    let p = ui.painter();
    let outer = Rect::from_min_max(inner.min - Vec2::new(14.0, 30.0), inner.max + Vec2::new(14.0, 14.0));
    p.rect_filled(outer, Rounding::same(18.0), Color32::BLACK);
    p.rect_stroke(outer.shrink(2.5), Rounding::same(16.0), Stroke::new(1.5, TRIM));
    p.rect_stroke(outer.shrink(5.5), Rounding::same(13.0), Stroke::new(1.0, Color32::from_rgb(70, 69, 50)));
    // title tab
    let cy = outer.min.y + 12.0;
    let tw = 210.0f32.min(outer.width() - 120.0);
    let tab = Rect::from_center_size(Pos2::new(outer.center().x, cy), Vec2::new(tw, 22.0));
    p.rect_filled(tab.expand(3.0), Rounding::same(6.0), Color32::BLACK);
    fill_rect(p, Rect::from_min_max(Pos2::new(outer.min.x + 26.0, cy - 1.0), Pos2::new(tab.min.x - 6.0, cy + 1.0)), TRIM);
    fill_rect(p, Rect::from_min_max(Pos2::new(tab.max.x + 6.0, cy - 1.0), Pos2::new(outer.max.x - 26.0, cy + 1.0)), TRIM);
    ptext(p, tab.center(), Align::Center, &format!("-[ {} ]-", title), 2.0, TRIM);
    // body
    p.rect_filled(inner.expand(2.0), Rounding::same(3.0), INK);
    p.rect_filled(inner, Rounding::same(2.0), BEIGE);
}

const SEGS: [(f32, f32, f32, f32); 7] = [
    (1.0, 0.0, 6.0, 2.0),  // a
    (6.0, 1.0, 2.0, 6.0),  // b
    (6.0, 7.0, 2.0, 6.0),  // c
    (1.0, 12.0, 6.0, 2.0), // d
    (0.0, 7.0, 2.0, 6.0),  // e
    (0.0, 1.0, 2.0, 6.0),  // f
    (1.0, 6.0, 6.0, 2.0),  // g
];

fn seg_mask(d: u32) -> [u8; 7] {
    match d {
        0 => [1, 1, 1, 1, 1, 1, 0],
        1 => [0, 1, 1, 0, 0, 0, 0],
        2 => [1, 1, 0, 1, 1, 0, 1],
        3 => [1, 1, 1, 1, 0, 0, 1],
        4 => [0, 1, 1, 0, 0, 1, 1],
        5 => [1, 0, 1, 1, 0, 1, 1],
        6 => [1, 0, 1, 1, 1, 1, 1],
        7 => [1, 1, 1, 0, 0, 0, 0],
        8 => [1, 1, 1, 1, 1, 1, 1],
        _ => [1, 1, 1, 1, 0, 1, 1],
    }
}

/// Seven-segment LCD digit with faint unlit segments.
fn seg_digit(p: &egui::Painter, o: Pos2, k: f32, d: u32, lit: bool) {
    let m = seg_mask(d);
    for (i, (x, y, w, h)) in SEGS.iter().enumerate() {
        let r = Rect::from_min_size(o + Vec2::new(x * k, y * k), Vec2::new(w * k, h * k));
        fill_rect(p, r, LCD_GHOST);
        if lit && m[i] == 1 {
            fill_rect(p, r, INK);
        }
    }
}

/// Round metal transport button with a pixel icon.
fn round_btn(ui: &egui::Ui, center: Pos2, radius: f32, icon: &[&str], px: f32, id: &str) -> egui::Response {
    let rect = Rect::from_center_size(center, Vec2::splat(radius * 2.0));
    let resp = ui
        .interact(rect, ui.id().with(id), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let down = resp.is_pointer_button_down_on();
    let off = if down { 1.5 } else { 0.0 };
    let p = ui.painter();
    p.circle_filled(center, radius, Color32::BLACK);
    p.circle_filled(center + Vec2::new(0.0, off), radius - 1.5, if resp.hovered() { BTN_HI } else { BTN_FACE });
    p.circle_filled(
        center + Vec2::new(-radius * 0.18, -radius * 0.28 + off),
        radius * 0.5,
        Color32::from_rgba_unmultiplied(255, 255, 225, 55),
    );
    let o = center - Vec2::new(3.5 * px, 3.5 * px) + Vec2::new(0.0, off);
    pixmap(p, o, px, icon, Color32::from_rgb(34, 32, 16));
    resp
}

fn retro_btn(ui: &mut egui::Ui, text: &str, w: f32, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::click());
    let down = resp.is_pointer_button_down_on() || active;
    raised(ui.painter(), rect, down);
    let dy = if down { 1.0 } else { 0.0 };
    ptext(ui.painter(), rect.center() + Vec2::new(0.0, dy), Align::Center, text, 2.0, INK);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn section_header(ui: &mut egui::Ui, text: &str) {
    ui.add_space(6.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
    fill_rect(ui.painter(), rect, INK);
    ptext(ui.painter(), Pos2::new(rect.min.x + 8.0, rect.center().y), Align::Min, text, 2.0, TRIM);
}

fn list_row(ui: &mut egui::Ui, num: Option<usize>, left: &str, right: &str, playing: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        fill_rect(p, rect, SEL);
    }
    let color = if playing { INK } else { INK2 };
    let font = FontId::proportional(15.0);
    let cy = rect.center().y;
    let rr = p.text(Pos2::new(rect.max.x - 8.0, cy), Align2::RIGHT_CENTER, right, font.clone(), color);
    let x0 = rect.min.x + 8.0;
    let mut tx = x0;
    if let Some(n) = num {
        if playing {
            pixmap(p, Pos2::new(x0, cy - 5.0), 2.0, &PLAY_S[..], INK);
        } else {
            p.text(Pos2::new(x0, cy), Align2::LEFT_CENTER, format!("{}.", n), font.clone(), color);
        }
        tx = x0 + 34.0;
    }
    let clip = Rect::from_min_max(Pos2::new(tx, rect.min.y), Pos2::new(rr.min.x - 12.0, rect.max.y));
    p.with_clip_rect(clip).text(Pos2::new(tx, cy), Align2::LEFT_CENTER, left, font, color);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

// -------------------------------------------------------------------- misc
fn fmt_time(s: f32) -> String {
    let s = s.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

fn fmt_long(s: f32) -> String {
    let s = s.max(0.0) as u32;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

fn rand_u64() -> u64 {
    let n = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
    let mut x = n ^ 0x9E37_79B9_7F4A_7C15;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}

fn goertzel(x: &[f32], f: f32, rate: f32) -> f32 {
    let w = 2.0 * std::f32::consts::PI * f / rate;
    let c = 2.0 * w.cos();
    let (mut s1, mut s2) = (0.0f32, 0.0f32);
    for &v in x {
        let s0 = v + c * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    let p = s1 * s1 + s2 * s2 - c * s1 * s2;
    p.max(0.0).sqrt() * 2.0 / x.len() as f32
}

// -------------------------------------------------------------------- images
struct Images {
    map: HashMap<String, Option<egui::TextureHandle>>,
    requested: HashSet<String>,
    req: Sender<String>,
}

impl Images {
    fn get(&mut self, url: &str) -> Option<egui::TextureId> {
        if url.is_empty() {
            return None;
        }
        if let Some(Some(h)) = self.map.get(url) {
            return Some(h.id());
        }
        if self.requested.insert(url.to_string()) {
            let _ = self.req.send(url.to_string());
        }
        None
    }
}

fn spawn_image_pool(api: Api, tx: Sender<Msg>, ctx: egui::Context) -> Sender<String> {
    let (req_tx, req_rx) = channel::<String>();
    let req_rx = Arc::new(Mutex::new(req_rx));
    for _ in 0..4 {
        let rx = req_rx.clone();
        let tx = tx.clone();
        let api = api.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || loop {
            let url = match rx.lock().unwrap().recv() {
                Ok(u) => u,
                Err(_) => break,
            };
            let img = api.fetch(&url).ok().and_then(|b| image::load_from_memory(&b).ok()).map(|im| {
                let rgba = im.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw())
            });
            let _ = tx.send(Msg::Img(url, img));
            ctx.request_repaint();
        });
    }
    req_tx
}

fn paint_art(ui: &egui::Ui, images: &mut Images, url: &str, rect: Rect, round: f32) {
    ui.painter().rect_filled(rect, Rounding::same(round), INK2);
    if let Some(id) = images.get(url) {
        egui::Image::new(egui::load::SizedTexture::new(id, rect.size()))
            .rounding(Rounding::same(round))
            .paint_at(ui, rect);
    }
}

// ------------------------------------------------------------------ page view
fn page_view(ui: &mut egui::Ui, images: &mut Images, page: &Page, playing_id: Option<i64>, acts: &mut Vec<Action>) {
    ui.add_space(4.0);
    let has_header = !page.image.is_empty() || !page.subtitle.is_empty();
    if has_header {
        ui.horizontal(|ui| {
            let (cr, _) = ui.allocate_exact_size(Vec2::splat(104.0), Sense::hover());
            inset(ui.painter(), cr, INK);
            paint_art(ui, images, &page.image, cr.shrink(2.0), 0.0);
            ui.vertical(|ui| {
                ui.label(RichText::new(&page.title).size(20.0).strong().color(INK));
                ui.label(RichText::new(&page.subtitle).size(14.0).color(INK2));
                ui.add_space(6.0);
                if !page.tracks.is_empty() {
                    ui.horizontal(|ui| {
                        if retro_btn(ui, "PLAY", 72.0, false).clicked() {
                            acts.push(Action::Play(page.tracks.clone(), 0));
                        }
                        if retro_btn(ui, "SHUFFLE", 92.0, false).clicked() {
                            let mut v = page.tracks.clone();
                            let n = v.len();
                            for i in (1..n).rev() {
                                let j = (rand_u64().wrapping_add(i as u64 * 7919) % (i as u64 + 1)) as usize;
                                v.swap(i, j);
                            }
                            acts.push(Action::Play(v, 0));
                        }
                    });
                }
            });
        });
    } else {
        ui.label(RichText::new(&page.title).size(22.0).strong().color(INK));
    }

    let rows = |ui: &mut egui::Ui, acts: &mut Vec<Action>| {
        for (title, cards) in &page.rows {
            section_header(ui, title);
            for c in cards {
                let tag = match c.kind {
                    Kind::Album => "ALBUM",
                    Kind::Playlist => "LIST",
                    Kind::Artist => "ARTIST",
                    Kind::Mix => "MIX",
                };
                let r = list_row(ui, None, &format!("[{}]  {}", tag, c.title), &c.subtitle, false);
                if r.clicked() {
                    acts.push(Action::Open(c.clone()));
                }
            }
        }
    };
    let tracks = |ui: &mut egui::Ui, acts: &mut Vec<Action>| {
        if page.tracks.is_empty() {
            return;
        }
        if page.rows_first {
            section_header(ui, "LIKED SONGS");
        } else {
            ui.add_space(8.0);
        }
        for (i, t) in page.tracks.iter().enumerate() {
            let playing = playing_id == Some(t.id);
            let r = list_row(ui, Some(i + 1), &format!("{} - {}", t.artist, t.title), &fmt_time(t.duration), playing);
            if r.clicked() {
                acts.push(Action::Play(page.tracks.clone(), i));
            }
            let tc = t.clone();
            r.context_menu(|ui| {
                if ui.button("Play next").clicked() {
                    acts.push(Action::PlayNext(tc.clone()));
                    ui.close_menu();
                }
                if ui.button("Add to queue").clicked() {
                    acts.push(Action::Enqueue(tc.clone()));
                    ui.close_menu();
                }
            });
        }
    };
    if page.rows_first {
        rows(ui, acts);
        tracks(ui, acts);
    } else {
        tracks(ui, acts);
        rows(ui, acts);
    }
    ui.add_space(20.0);
}

// ----------------------------------------------------------------------- app
struct App {
    api: Api,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    ctx: egui::Context,
    images: Images,

    auth: Auth,
    login_code: Option<(String, String)>,
    login_err: String,

    page: Option<Arc<Page>>,
    back: Vec<Arc<Page>>,
    serial: u64,
    loading: bool,
    search: String,
    status: String,
    status_err: bool,

    queue: Arc<Vec<Track>>,
    cur: Option<usize>,
    play_gen: u64,
    buffering: bool,
    paused: bool,
    stopped: bool,
    prefer_lossless: bool,
    kbps: u32,
    prefetched: Option<(i64, Vec<u8>)>,
    prefetching: i64,
    player: Player,
    volume: f32,
    shuffle: bool,
    repeat: Repeat,
    seek_drag: Option<f32>,
    bands: [f32; NB],
    peaks: [f32; NB],
    show_log: bool,
    audio_err_shown: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> App {
        setup_style(&cc.egui_ctx);
        let api = Api::new();
        let (tx, rx) = channel();
        let ctx = cc.egui_ctx.clone();
        let req = spawn_image_pool(api.clone(), tx.clone(), ctx.clone());
        let mut app = App {
            api,
            tx,
            rx,
            ctx,
            images: Images { map: HashMap::new(), requested: HashSet::new(), req },
            auth: Auth::LoggedOut,
            login_code: None,
            login_err: String::new(),
            page: None,
            back: Vec::new(),
            serial: 0,
            loading: false,
            search: String::new(),
            status: String::new(),
            status_err: false,
            queue: Arc::new(Vec::new()),
            cur: None,
            play_gen: 0,
            buffering: false,
            paused: false,
            stopped: true,
            prefer_lossless: true,
            kbps: 0,
            prefetched: None,
            prefetching: 0,
            player: Player::new(),
            volume: 0.8,
            shuffle: false,
            repeat: Repeat::Off,
            seek_drag: None,
            bands: [0.0; NB],
            peaks: [0.0; NB],
            show_log: false,
            audio_err_shown: false,
        };
        crate::api::log(&format!("tidalfast started (token saved: {})", app.api.has_token()));
        if app.api.has_token() {
            app.auth = Auth::Checking;
            let (api, tx, ctx) = (app.api.clone(), app.tx.clone(), app.ctx.clone());
            std::thread::spawn(move || {
                match api.refresh_info() {
                    Ok(()) => {
                        let _ = tx.send(Msg::LoggedIn);
                    }
                    Err(e) => {
                        let _ = tx.send(Msg::NeedLogin(e));
                    }
                }
                ctx.request_repaint();
            });
        }
        app
    }

    fn set_err(&mut self, e: String) {
        api::log(&format!("ERROR: {}", e));
        self.status = e;
        self.status_err = true;
    }

    fn set_note(&mut self, s: &str) {
        self.status = s.to_string();
        self.status_err = false;
    }

    // ------------------------------------------------------------- loading
    fn load(&mut self, push: bool, f: impl FnOnce(&Api) -> Result<Page, String> + Send + 'static) {
        self.loading = true;
        let (api, tx, ctx) = (self.api.clone(), self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let _ = match f(&api) {
                Ok(p) => tx.send(Msg::Page(p, push)),
                Err(e) => tx.send(Msg::Err(e)),
            };
            ctx.request_repaint();
        });
    }

    fn after_login(&mut self) {
        self.auth = Auth::In;
        self.login_code = None;
        self.back.clear();
        crate::api::log("logged in; loading library");
        self.load(false, |a| a.library());
    }

    fn start_login(&mut self) {
        self.auth = Auth::LoggingIn;
        self.login_err.clear();
        let (api, tx, ctx) = (self.api.clone(), self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            match api.start_login() {
                Ok(dc) => {
                    let _ = webbrowser::open(&dc.url);
                    let _ = tx.send(Msg::Code(dc.user_code.clone(), dc.url.clone()));
                    ctx.request_repaint();
                    match api.finish_login(&dc) {
                        Ok(()) => {
                            let _ = api.refresh_info();
                            let _ = tx.send(Msg::LoggedIn);
                        }
                        Err(e) => {
                            let _ = tx.send(Msg::NeedLogin(e));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(Msg::NeedLogin(e));
                }
            }
            ctx.request_repaint();
        });
    }

    fn drain(&mut self, ctx: &egui::Context) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Code(c, u) => self.login_code = Some((c, u)),
                Msg::LoggedIn => self.after_login(),
                Msg::NeedLogin(e) => {
                    self.auth = Auth::LoggedOut;
                    self.login_err = e;
                }
                Msg::Page(p, push) => {
                    crate::api::log(&format!("page '{}': {} rows, {} tracks", p.title, p.rows.len(), p.tracks.len()));
                    if push {
                        if let Some(old) = self.page.take() {
                            self.back.push(old);
                        }
                    }
                    self.page = Some(Arc::new(p));
                    self.serial += 1;
                    self.loading = false;
                }
                Msg::Err(e) => {
                    self.loading = false;
                    self.set_err(e);
                }
                Msg::Audio(g, id, bytes) => {
                    if g == self.play_gen {
                        let n = bytes.len();
                        crate::api::log(&format!("downloaded {} bytes for track {}", n, id));
                        self.player.send(Cmd::Play(bytes));
                        self.set_kbps(n);
                        self.buffering = false;
                        self.paused = false;
                        self.stopped = false;
                        self.prefetch_next(id);
                    }
                }
                Msg::PlayErr(g, e) => {
                    if g == self.play_gen {
                        self.buffering = false;
                        self.stopped = true;
                        self.set_err(format!("CAN'T PLAY: {}", e));
                    }
                }
                Msg::Prefetched(id, b) => self.prefetched = Some((id, b)),
                Msg::Img(url, img) => {
                    let tex = img.map(|ci| ctx.load_texture(&url, ci, egui::TextureOptions::LINEAR));
                    self.images.map.insert(url, tex);
                }
            }
        }
    }

    // ------------------------------------------------------------ playback
    fn set_kbps(&mut self, bytes: usize) {
        let dur = self.cur_track().map(|t| t.duration).unwrap_or(0.0);
        self.kbps = if dur > 1.0 { (bytes as f32 * 8.0 / 1000.0 / dur) as u32 } else { 0 };
    }

    fn play_index(&mut self, i: usize) {
        let Some(t) = self.queue.get(i).cloned() else { return };
        self.cur = Some(i);
        self.play_gen += 1;
        let g = self.play_gen;
        self.player.send(Cmd::Stop);
        self.status.clear();
        self.stopped = false;
        self.paused = false;
        self.audio_err_shown = false;
        crate::api::log(&format!("play: '{}' - '{}' (lossless: {})", t.artist, t.title, self.prefer_lossless));

        if let Some((id, bytes)) = self.prefetched.take() {
            if id == t.id {
                let n = bytes.len();
                self.player.send(Cmd::Play(bytes));
                self.set_kbps(n);
                self.buffering = false;
                self.prefetch_next(id);
                return;
            }
        }
        self.buffering = true;
        let lossless = self.prefer_lossless;
        let (api, tx, ctx) = (self.api.clone(), self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let r = api.stream_url(t.id, lossless).and_then(|u| api.fetch(&u));
            let _ = match r {
                Ok(b) => tx.send(Msg::Audio(g, t.id, b)),
                Err(e) => tx.send(Msg::PlayErr(g, e)),
            };
            ctx.request_repaint();
        });
    }

    fn next_index(&self) -> Option<usize> {
        let cur = self.cur?;
        let n = self.queue.len();
        if n == 0 {
            return None;
        }
        if self.shuffle && n > 1 {
            let mut j = (rand_u64() % n as u64) as usize;
            if j == cur {
                j = (j + 1) % n;
            }
            return Some(j);
        }
        if cur + 1 < n {
            Some(cur + 1)
        } else if self.repeat == Repeat::All {
            Some(0)
        } else {
            None
        }
    }

    fn prefetch_next(&mut self, _playing_id: i64) {
        if self.shuffle {
            return;
        }
        let Some(cur) = self.cur else { return };
        let Some(next) = self.queue.get(cur + 1) else { return };
        if self.prefetching == next.id || self.prefetched.as_ref().map(|p| p.0) == Some(next.id) {
            return;
        }
        self.prefetching = next.id;
        let id = next.id;
        let lossless = self.prefer_lossless;
        let (api, tx, ctx) = (self.api.clone(), self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            if let Ok(b) = api.stream_url(id, lossless).and_then(|u| api.fetch(&u)) {
                let _ = tx.send(Msg::Prefetched(id, b));
                ctx.request_repaint();
            }
        });
    }

    fn pos(&self) -> f32 {
        self.player.shared.lock().unwrap().pos
    }

    fn cur_track(&self) -> Option<Track> {
        self.cur.and_then(|i| self.queue.get(i).cloned())
    }

    fn update_bands(&mut self) {
        let active = self.cur.is_some() && !self.paused && !self.stopped && !self.buffering;
        let (samples, rate) = if active {
            let v = self.player.viz.lock().unwrap();
            let n = v.samples.len().min(1024);
            (v.samples[v.samples.len() - n..].to_vec(), v.rate as f32)
        } else {
            (Vec::new(), 44100.0)
        };
        let n = samples.len();
        let mut target = [0.0f32; NB];
        if n >= 512 && rate > 0.0 {
            let denom = (n - 1) as f32;
            let xs: Vec<f32> = samples
                .iter()
                .enumerate()
                .map(|(i, v)| v * (0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / denom).cos()))
                .collect();
            for i in 0..NB {
                let f = 60.0 * (12000.0f32 / 60.0).powf(i as f32 / (NB - 1) as f32);
                if f >= rate / 2.0 {
                    continue;
                }
                let amp = goertzel(&xs, f, rate) * 2.0;
                let db = 20.0 * (amp + 1e-6).log10();
                target[i] = ((db + 64.0 + i as f32 * 1.3) / 54.0).clamp(0.0, 1.0);
            }
        }
        for i in 0..NB {
            let old = self.bands[i];
            self.bands[i] = if target[i] > old { target[i] } else { (old - 0.06).max(target[i]) };
            self.peaks[i] = self.bands[i].max(self.peaks[i] - 0.012);
        }
    }

    fn apply(&mut self, a: Action) {
        match a {
            Action::StartLogin => self.start_login(),
            Action::Home => {
                self.back.clear();
                self.load(false, |a| a.home());
            }
            Action::Library => {
                self.back.clear();
                self.load(false, |a| a.library());
            }
            Action::Search(q) => {
                if !q.trim().is_empty() {
                    self.load(true, move |a| a.search(q.trim()));
                }
            }
            Action::Open(c) => self.load(true, move |a| a.open(&c)),
            Action::Back => {
                if let Some(p) = self.back.pop() {
                    self.page = Some(p);
                    self.serial += 1;
                }
            }
            Action::Play(tracks, i) => {
                self.queue = Arc::new(tracks);
                self.play_index(i);
            }
            Action::PlayIndex(i) => self.play_index(i),
            Action::PlayNext(t) => {
                let at = self.cur.map(|c| c + 1).unwrap_or(0).min(self.queue.len());
                Arc::make_mut(&mut self.queue).insert(at, t);
                if self.cur.is_none() {
                    self.play_index(0);
                }
            }
            Action::Enqueue(t) => {
                Arc::make_mut(&mut self.queue).push(t);
                if self.cur.is_none() {
                    self.play_index(0);
                }
            }
            Action::ClearQueue => {
                if let Some(t) = self.cur_track() {
                    self.queue = Arc::new(vec![t]);
                    self.cur = Some(0);
                } else {
                    self.queue = Arc::new(Vec::new());
                }
            }
            Action::PlayBtn => {
                if self.cur.is_none() {
                    if !self.queue.is_empty() {
                        self.play_index(0);
                    }
                } else if self.stopped {
                    if let Some(c) = self.cur {
                        self.play_index(c);
                    }
                } else if self.paused {
                    self.player.send(Cmd::Resume);
                    self.paused = false;
                }
            }
            Action::PauseBtn => {
                if self.cur.is_some() && !self.stopped {
                    if self.paused {
                        self.player.send(Cmd::Resume);
                        self.paused = false;
                    } else {
                        self.player.send(Cmd::Pause);
                        self.paused = true;
                    }
                }
            }
            Action::StopBtn => {
                self.play_gen += 1;
                self.player.send(Cmd::Stop);
                self.stopped = true;
                self.paused = false;
                self.buffering = false;
            }
            Action::Toggle => {
                if self.cur.is_none() || self.stopped {
                    self.apply(Action::PlayBtn);
                } else {
                    self.apply(Action::PauseBtn);
                }
            }
            Action::Next => {
                if let Some(j) = self.next_index() {
                    self.play_index(j);
                }
            }
            Action::Prev => {
                if self.pos() > 3.0 {
                    self.player.send(Cmd::Seek(0.0));
                } else if let Some(c) = self.cur {
                    self.play_index(c.saturating_sub(1));
                }
            }
            Action::Seek(t) => self.player.send(Cmd::Seek(t)),
            Action::Volume(v) => {
                self.volume = v;
                self.player.send(Cmd::Volume(v * v));
            }
            Action::Shuffle => self.shuffle = !self.shuffle,
            Action::Repeat => {
                self.repeat = match self.repeat {
                    Repeat::Off => Repeat::All,
                    Repeat::All => Repeat::One,
                    Repeat::One => Repeat::Off,
                }
            }
            Action::ToggleLossless => {
                self.prefer_lossless = !self.prefer_lossless;
                self.prefetched = None;
                self.prefetching = 0;
                let msg = if self.prefer_lossless { "HIFI LOSSLESS - NEXT TRACK" } else { "320K AAC - NEXT TRACK" };
                self.set_note(msg);
            }
            Action::ClearStatus => self.status.clear(),
            Action::Logout => {
                self.player.send(Cmd::Stop);
                self.api.logout();
                self.auth = Auth::LoggedOut;
                self.page = None;
                self.back.clear();
                self.queue = Arc::new(Vec::new());
                self.cur = None;
                self.stopped = true;
            }
        }
    }

    // ---------------------------------------------------------------- views
    fn login_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        let full = ui.max_rect();
        let inner = Rect::from_center_size(full.center(), Vec2::new(460.0, 260.0));
        window_deco(ui, inner, "TIDALFAST");
        let p = ui.painter();
        let c = inner.center();
        ptext(p, c + Vec2::new(0.0, -80.0), Align::Center, "TIDALFAST", 6.0, INK);
        ptext(p, c + Vec2::new(0.0, -42.0), Align::Center, "TIDAL, NATIVE AND FAST", 2.0, INK2);
        match self.auth {
            Auth::Checking => {
                ptext(p, c + Vec2::new(0.0, 20.0), Align::Center, "CHECKING SESSION...", 2.0, INK);
            }
            Auth::LoggingIn => match &self.login_code {
                Some((code, url)) => {
                    ptext(p, c + Vec2::new(0.0, -6.0), Align::Center, "APPROVE THE LOGIN IN YOUR BROWSER", 2.0, INK);
                    ptext(p, c + Vec2::new(0.0, 32.0), Align::Center, code, 5.0, INK);
                    let b = Rect::from_center_size(c + Vec2::new(0.0, 82.0), Vec2::new(300.0, 28.0));
                    let r = ui.interact(b, ui.id().with("relink"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    raised(ui.painter(), b, r.is_pointer_button_down_on());
                    ptext(ui.painter(), b.center(), Align::Center, "OPEN LINK AGAIN", 2.0, INK);
                    if r.clicked() {
                        let _ = webbrowser::open(url);
                    }
                }
                None => {
                    ptext(p, c + Vec2::new(0.0, 20.0), Align::Center, "CONTACTING TIDAL...", 2.0, INK);
                }
            },
            _ => {
                let b = Rect::from_center_size(c + Vec2::new(0.0, 28.0), Vec2::new(300.0, 40.0));
                let r = ui.interact(b, ui.id().with("login"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                raised(ui.painter(), b, r.is_pointer_button_down_on());
                ptext(ui.painter(), b.center(), Align::Center, "LOG IN WITH TIDAL", 3.0, INK);
                if r.clicked() {
                    acts.push(Action::StartLogin);
                }
                if !self.login_err.is_empty() {
                    let er = Rect::from_center_size(c + Vec2::new(0.0, 90.0), Vec2::new(420.0, 20.0));
                    marquee(ui, er, &self.login_err, 2.0, RED);
                }
            }
        }
    }

    fn library_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        window_deco(ui, ui.max_rect(), "LIBRARY");
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 64.0).max(80.0);
            let r = ui.add_sized(
                [w, 24.0],
                egui::TextEdit::singleline(&mut self.search)
                    .font(FontId::proportional(15.0))
                    .hint_text("Search Tidal..."),
            );
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                acts.push(Action::Search(self.search.clone()));
            }
            if retro_btn(ui, "GO", 52.0, false).clicked() {
                acts.push(Action::Search(self.search.clone()));
            }
        });
        ui.horizontal(|ui| {
            if retro_btn(ui, "HOME", 66.0, false).clicked() {
                acts.push(Action::Home);
            }
            if retro_btn(ui, "LIBRARY", 90.0, false).clicked() {
                acts.push(Action::Library);
            }
            if !self.back.is_empty() && retro_btn(ui, "< BACK", 76.0, false).clicked() {
                acts.push(Action::Back);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if retro_btn(ui, "LOG OUT", 84.0, false).clicked() {
                    acts.push(Action::Logout);
                }
                if retro_btn(ui, "LOG", 52.0, self.show_log).clicked() {
                    self.show_log = !self.show_log;
                }
            });
        });
        if self.status_err && !self.status.is_empty() {
            ui.add(egui::Label::new(RichText::new(self.status.clone()).size(13.0).color(RED)));
        }
        if self.show_log {
            let mut text = crate::api::log_text();
            egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut text)
                        .font(FontId::monospace(12.0))
                        .desired_width(f32::INFINITY),
                );
            });
            return;
        }
        ui.add_space(4.0);
        let page = self.page.clone();
        let serial = self.serial;
        let loading = self.loading;
        let playing_id = self.cur_track().map(|t| t.id);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.push_id(serial, |ui| {
                if loading {
                    ui.add_space(4.0);
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
                    ptext(ui.painter(), Pos2::new(rect.min.x + 4.0, rect.center().y), Align::Min, "LOADING...", 2.0, INK2);
                }
                if let Some(pg) = &page {
                    page_view(ui, &mut self.images, pg, playing_id, acts);
                }
            });
        });
    }

    fn player_window(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        let inner = ui.max_rect();
        window_deco(ui, inner, "TIDALFAST");
        let s = (inner.width() / 300.0).min(inner.height() / 126.0).max(0.5);
        let ox = inner.min.x + (inner.width() - 300.0 * s) / 2.0;
        let oy = inner.min.y + (inner.height() - 126.0 * s) / 2.0;
        let rc = move |x: f32, y: f32, w: f32, h: f32| {
            Rect::from_min_size(Pos2::new(ox + x * s, oy + y * s), Vec2::new(w * s, h * s))
        };
        let px_big = (1.5 * s).round().max(1.0);
        let px_sm = (1.0 * s).round().max(1.0);
        let now_t = ui.input(|i| i.time);
        let track = self.cur_track();
        let pos = self.pos();
        let dur = track.as_ref().map(|t| t.duration).unwrap_or(0.0);
        let shown = self.seek_drag.unwrap_or(pos);
        let rate = self.player.shared.lock().unwrap().rate;
        let active = self.cur.is_some() && !self.stopped;
        let p = ui.painter();

        // ---- LCD time
        let lcd = rc(6.0, 6.0, 100.0, 40.0);
        inset(p, lcd, LCD);
        let icon: &[&str] = if !active {
            &STOP_S[..]
        } else if self.paused {
            &PAUSE_S[..]
        } else {
            &PLAY_S[..]
        };
        pixmap(p, Pos2::new(ox + 10.0 * s, oy + 11.0 * s), (0.9 * s).max(1.0), icon, INK);
        let blink = !(active && self.paused) || (now_t * 2.0) as i64 % 2 == 0;
        let e = if active { shown.max(0.0) as u32 } else { 0 };
        let (mm, ss) = ((e / 60).min(99), e % 60);
        let digits = [mm / 10, mm % 10, ss / 10, ss % 10];
        let xs = [22.0f32, 38.0, 62.0, 78.0];
        for i in 0..4 {
            seg_digit(p, Pos2::new(ox + xs[i] * s, oy + 14.0 * s), 1.5 * s, digits[i], blink);
        }
        fill_rect(p, rc(54.0, 20.0, 3.0, 3.0), INK);
        fill_rect(p, rc(54.0, 29.0, 3.0, 3.0), INK);

        // ---- spectrum
        let sp = rc(6.0, 50.0, 100.0, 28.0);
        inset(p, sp, LCD);
        for i in 0..NB {
            let x = 8.0 + i as f32 * 5.0;
            let h = ((self.bands[i] * 22.0) / 2.0).floor() * 2.0;
            if h > 0.0 {
                fill_rect(p, rc(x, 75.0 - h, 4.0, h), INK);
            }
            let ph = ((self.peaks[i] * 22.0) / 2.0).floor() * 2.0;
            if ph > 0.0 {
                fill_rect(p, rc(x, 75.0 - ph - 2.0, 4.0, 1.0), INK2);
            }
        }

        // ---- title + status
        let tb = rc(112.0, 6.0, 182.0, 14.0);
        inset(p, tb, LCD);
        let title_txt = match &track {
            Some(t) => format!("{} - {}  ({})", t.artist, t.title, fmt_time(t.duration)),
            None => "TIDALFAST - TIDAL, NATIVE AND FAST".to_string(),
        };
        marquee(ui, tb, &title_txt, px_big, INK);

        let sb = rc(112.0, 22.0, 182.0, 10.0);
        inset(p, sb, LCD);
        let (stxt, scol): (String, Color32) = if !self.status.is_empty() {
            (self.status.clone(), if self.status_err { RED } else { INK })
        } else if self.buffering {
            ("BUFFERING...".to_string(), INK2)
        } else if self.loading {
            ("LOADING...".to_string(), INK2)
        } else {
            (String::new(), INK2)
        };
        marquee(ui, sb, &stxt, px_sm, scol);
        let sr = ui.interact(sb, ui.id().with("status"), Sense::click());
        if sr.clicked() && !self.status.is_empty() {
            acts.push(Action::ClearStatus);
        }

        // ---- kbps / khz / quality
        let kb = rc(112.0, 34.0, 50.0, 12.0);
        inset(p, kb, LCD);
        let kb_txt = if active && self.kbps > 0 { format!("{} KBPS", self.kbps) } else { "--- KBPS".to_string() };
        ptext(p, kb.center(), Align::Center, &kb_txt, px_sm, INK);
        let kh = rc(166.0, 34.0, 40.0, 12.0);
        inset(p, kh, LCD);
        let kh_txt = if active && rate > 0 { format!("{} KHZ", rate / 1000) } else { "-- KHZ".to_string() };
        ptext(p, kh.center(), Align::Center, &kh_txt, px_sm, INK);
        let qb = rc(210.0, 34.0, 40.0, 12.0);
        let qr = ui
            .interact(qb, ui.id().with("quality"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Click to switch between lossless and 320k AAC");
        raised(p, qb, qr.is_pointer_button_down_on());
        ptext(p, qb.center(), Align::Center, if self.prefer_lossless { "HIFI" } else { "320K" }, px_sm, INK);
        if qr.clicked() {
            acts.push(Action::ToggleLossless);
        }

        // ---- cover art
        let cv = rc(258.0, 34.0, 36.0, 36.0);
        inset(p, cv, INK);
        if let Some(t) = &track {
            paint_art(ui, &mut self.images, &cover_url(&t.cover, 160), cv.shrink(2.0), 0.0);
        }

        // ---- volume
        let vg = rc(112.0, 54.0, 138.0, 7.0);
        for i in 0..28 {
            let h = 2.0 + i as f32 * 0.14;
            fill_rect(p, rc(113.0 + i as f32 * 4.85, 52.0 - h, 1.0, h), INK2);
        }
        inset(p, vg, GROOVE);
        let thumb_v = 9.0 * s;
        let vx = vg.min.x + (vg.width() - thumb_v) * self.volume;
        fill_rect(p, Rect::from_min_max(vg.min + Vec2::new(1.5, 1.5), Pos2::new(vx + thumb_v / 2.0, vg.max.y - 1.0)), INK2);
        let vthumb = Rect::from_min_size(Pos2::new(vx, oy + 50.0 * s), Vec2::new(thumb_v, 15.0 * s));
        raised(p, vthumb, false);
        let vr = ui.interact(rc(112.0, 48.0, 138.0, 18.0), ui.id().with("vol"), Sense::click_and_drag());
        if vr.dragged() || vr.clicked() {
            if let Some(pp) = vr.interact_pointer_pos() {
                let v = ((pp.x - vg.min.x - thumb_v / 2.0) / (vg.width() - thumb_v)).clamp(0.0, 1.0);
                acts.push(Action::Volume(v));
            }
        }
        ptext(p, Pos2::new(ox + 112.0 * s, oy + 71.0 * s), Align::Min, "VOLUME", px_sm, INK2);
        ptext(p, Pos2::new(ox + 250.0 * s, oy + 71.0 * s), Align::Max, &format!("{}%", (self.volume * 100.0).round() as u32), px_sm, INK2);

        // ---- seek bar
        let sk = rc(6.0, 84.0, 288.0, 9.0);
        let thumb_w = 22.0 * s;
        let skr = ui.interact(rc(6.0, 80.0, 288.0, 17.0), ui.id().with("seek"), Sense::click_and_drag());
        if active && dur > 0.0 {
            if skr.dragged() || skr.clicked() {
                if let Some(pp) = skr.interact_pointer_pos() {
                    let f = ((pp.x - sk.min.x - thumb_w / 2.0) / (sk.width() - thumb_w)).clamp(0.0, 1.0);
                    self.seek_drag = Some(f * dur);
                }
            }
            if skr.drag_stopped() || skr.clicked() {
                if let Some(t) = self.seek_drag.take() {
                    acts.push(Action::Seek(t));
                }
            }
        }
        let frac = if active && dur > 0.0 { (shown / dur).clamp(0.0, 1.0) } else { 0.0 };
        inset(p, sk, GROOVE);
        let tx = sk.min.x + (sk.width() - thumb_w) * frac;
        fill_rect(p, Rect::from_min_max(sk.min + Vec2::new(1.5, 1.5), Pos2::new(tx + thumb_w / 2.0, sk.max.y - 1.0)), INK2);
        raised(p, Rect::from_min_size(Pos2::new(tx, sk.min.y - 1.0 * s), Vec2::new(thumb_w, sk.height() + 2.0 * s)), false);

        // ---- transport buttons
        let cy = oy + 110.0 * s;
        let r = 9.5 * s;
        let px = (1.3 * s).max(1.0);
        if round_btn(ui, Pos2::new(ox + 20.0 * s, cy), r, &PREV[..], px, "b_prev").clicked() {
            acts.push(Action::Prev);
        }
        if round_btn(ui, Pos2::new(ox + 44.0 * s, cy), r, &PLAY[..], px, "b_play").clicked() {
            acts.push(Action::PlayBtn);
        }
        if round_btn(ui, Pos2::new(ox + 68.0 * s, cy), r, &PAUSE[..], px, "b_pause").clicked() {
            acts.push(Action::PauseBtn);
        }
        if round_btn(ui, Pos2::new(ox + 92.0 * s, cy), r, &STOP[..], px, "b_stop").clicked() {
            acts.push(Action::StopBtn);
        }
        if round_btn(ui, Pos2::new(ox + 116.0 * s, cy), r, &NEXT[..], px, "b_next").clicked() {
            acts.push(Action::Next);
        }

        // ---- shuffle / repeat
        let sh = rc(160.0, 102.0, 62.0, 16.0);
        let shr = ui.interact(sh, ui.id().with("shuf"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        raised(p, sh, self.shuffle || shr.is_pointer_button_down_on());
        fill_rect(p, Rect::from_center_size(Pos2::new(sh.min.x + 7.0 * s, sh.center().y), Vec2::splat(3.0 * s)), if self.shuffle { RED } else { BEIGE_DK });
        ptext(p, sh.center() + Vec2::new(3.0 * s, 0.0), Align::Center, "SHUFFLE", px_sm, INK);
        if shr.clicked() {
            acts.push(Action::Shuffle);
        }
        let rp = rc(228.0, 102.0, 66.0, 16.0);
        let rpr = ui.interact(rp, ui.id().with("rep"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        raised(p, rp, self.repeat != Repeat::Off || rpr.is_pointer_button_down_on());
        fill_rect(p, Rect::from_center_size(Pos2::new(rp.min.x + 7.0 * s, rp.center().y), Vec2::splat(3.0 * s)), if self.repeat != Repeat::Off { RED } else { BEIGE_DK });
        let rp_txt = match self.repeat {
            Repeat::Off => "REPEAT",
            Repeat::All => "REPEAT ALL",
            Repeat::One => "REPEAT 1",
        };
        ptext(p, rp.center() + Vec2::new(3.0 * s, 0.0), Align::Center, rp_txt, px_sm, INK);
        if rpr.clicked() {
            acts.push(Action::Repeat);
        }
    }

    fn playlist_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        window_deco(ui, ui.max_rect(), "PLAYLIST");
        let queue = self.queue.clone();
        let cur = self.cur;
        let list_h = (ui.available_height() - 40.0).max(40.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(list_h).show(ui, |ui| {
            for (i, t) in queue.iter().enumerate() {
                let r = list_row(ui, Some(i + 1), &format!("{} - {}", t.artist, t.title), &fmt_time(t.duration), cur == Some(i));
                if r.clicked() {
                    acts.push(Action::PlayIndex(i));
                }
            }
            if queue.is_empty() {
                ui.add_space(6.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
                ptext(ui.painter(), Pos2::new(rect.min.x + 8.0, rect.center().y), Align::Min, "PLAYLIST EMPTY - PICK SOMETHING TO PLAY", 2.0, INK2);
            }
        });
        ui.add_space(6.0);
        let total: f32 = queue.iter().map(|t| t.duration).sum();
        let before: f32 = queue.iter().take(cur.unwrap_or(0)).map(|t| t.duration).sum();
        let elapsed = if self.cur.is_some() && !self.stopped { before + self.pos() } else { before };
        ui.horizontal(|ui| {
            if retro_btn(ui, "CLEAR", 72.0, false).clicked() {
                acts.push(Action::ClearQueue);
            }
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(260.0), 24.0), Sense::hover());
            inset(ui.painter(), rect, LCD);
            ptext(
                ui.painter(),
                rect.center(),
                Align::Center,
                &format!("{}/{}   {} TRACKS", fmt_long(elapsed), fmt_long(total), queue.len()),
                2.0,
                INK,
            );
        });
    }
}

// ------------------------------------------------------------------- update
fn panel_frame() -> egui::Frame {
    egui::Frame::none()
        .fill(APP_BG)
        .inner_margin(egui::Margin { left: 14.0, right: 14.0, top: 30.0, bottom: 14.0 })
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain(ctx);
        let mut acts: Vec<Action> = Vec::new();

        // Player events: errors and end-of-track
        {
            let (ended, err) = {
                let mut g = self.player.shared.lock().unwrap();
                let e = g.ended;
                g.ended = false;
                (e, g.error.take())
            };
            if let Some(e) = err {
                self.buffering = false;
                if e.starts_with("DECODE") && self.prefer_lossless {
                    api::log(&format!("lossless decode failed ({}), retrying as AAC", e));
                    self.prefer_lossless = false;
                    self.prefetched = None;
                    self.prefetching = 0;
                    if let Some(c) = self.cur {
                        self.play_index(c);
                    }
                    self.set_note("LOSSLESS FAILED - USING 320K AAC");
                } else {
                    self.stopped = true;
                    self.set_err(e);
                }
            }
            if ended && !self.buffering && !self.stopped {
                if self.repeat == Repeat::One {
                    if let Some(c) = self.cur {
                        self.play_index(c);
                    }
                } else if let Some(j) = self.next_index() {
                    self.play_index(j);
                } else {
                    self.stopped = true;
                }
            }
        }
        let dead = self.player.shared.lock().unwrap().dead.clone();
        if let Some(d) = dead {
            if !self.audio_err_shown && self.cur.is_some() && !self.stopped {
                self.audio_err_shown = true;
                self.stopped = true;
                self.buffering = false;
                self.set_err(d);
            }
        }
        self.update_bands();

        if self.auth != Auth::In {
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(APP_BG))
                .show(ctx, |ui| self.login_ui(ui, &mut acts));
        } else {
            if ctx.input(|i| i.key_pressed(egui::Key::Space)) && !ctx.wants_keyboard_input() {
                acts.push(Action::Toggle);
            }
            let screen = ctx.screen_rect();
            let right_w = (screen.width() - LIB_W).max(420.0);
            let inner_w = right_w - 28.0;
            let max_inner_h = (screen.height() * 0.5 - 44.0).max(120.0);
            let s = (inner_w / 300.0).min(max_inner_h / 126.0).clamp(0.8, 3.0);
            let player_h = 126.0 * s + 44.0;

            egui::SidePanel::left("library")
                .exact_width(LIB_W)
                .frame(panel_frame())
                .show(ctx, |ui| self.library_ui(ui, &mut acts));
            egui::TopBottomPanel::top("player")
                .exact_height(player_h)
                .frame(panel_frame())
                .show(ctx, |ui| self.player_window(ui, &mut acts));
            egui::CentralPanel::default()
                .frame(panel_frame())
                .show(ctx, |ui| self.playlist_ui(ui, &mut acts));
        }

        for a in acts {
            self.apply(a);
        }
        let busy = self.cur.is_some() && !self.stopped && !self.paused;
        ctx.request_repaint_after(Duration::from_millis(if busy { 33 } else { 250 }));
    }
}

fn setup_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::light();
    v.panel_fill = APP_BG;
    v.window_fill = BEIGE;
    v.extreme_bg_color = LCD;
    v.faint_bg_color = BEIGE_DK;
    v.hyperlink_color = INK;
    v.selection.bg_fill = SEL;
    v.selection.stroke = Stroke::new(1.0, INK);
    v.widgets.noninteractive.bg_stroke = Stroke::NONE;
    v.widgets.noninteractive.bg_fill = BEIGE;
    v.widgets.noninteractive.weak_bg_fill = BEIGE;
    v.widgets.inactive.bg_fill = BEIGE_DK;
    v.widgets.inactive.weak_bg_fill = BEIGE_DK;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, INK);
    v.widgets.hovered.bg_fill = SEL;
    v.widgets.hovered.weak_bg_fill = SEL;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, INK);
    v.widgets.active.bg_fill = BEIGE_DK;
    v.widgets.active.weak_bg_fill = BEIGE_DK;
    v.widgets.active.bg_stroke = Stroke::new(1.0, INK);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.fg_stroke = Stroke::new(1.0, INK);
        w.rounding = Rounding::same(0.0);
    }
    ctx.set_visuals(v);

    let mut s = (*ctx.style()).clone();
    s.spacing.item_spacing = Vec2::new(6.0, 4.0);
    s.spacing.button_padding = Vec2::new(8.0, 3.0);
    s.spacing.scroll.bar_width = 10.0;
    ctx.set_style(s);
}

fn main() -> eframe::Result<()> {
    std::panic::set_hook(Box::new(|info| {
        let loc = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "?".to_string()
        };
        crate::api::log(&format!("PANIC at {}: {}", loc, msg));
    }));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("tidalfast")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native("tidalfast", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
