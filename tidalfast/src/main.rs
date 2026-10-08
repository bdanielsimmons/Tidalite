#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod api;
mod decode;
mod font;
mod player;

use api::{cover_url, Api, Card, Kind, Page, Track};
use eframe::egui::{self, Align, Color32, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
use font::{fit, ptext, ptext_fit, snap, spx, text_w, thick, wrap};
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
const SEL: Color32 = Color32::from_rgb(184, 181, 142);
const INK: Color32 = Color32::from_rgb(12, 12, 8);
const INK2: Color32 = Color32::from_rgb(70, 70, 50);
const DIM: Color32 = Color32::from_rgb(120, 118, 88);
const RED: Color32 = Color32::from_rgb(150, 30, 20);
const BTN_FACE: Color32 = Color32::from_rgb(150, 146, 92);
const BTN_HI: Color32 = Color32::from_rgb(190, 186, 128);
const ROW_ALT: Color32 = Color32::from_rgb(203, 201, 163);
const ROW_SEL: Color32 = Color32::from_rgb(70, 70, 50);

const LIB_W: f32 = 430.0;
const NB: usize = 19;
const BTN_H: f32 = 26.0;
const ROW_H: f32 = 24.0;
const HEAD_H: f32 = 18.0;

// ------------------------------------------------------------------ messages
enum Msg {
    Code(String, String),
    LoggedIn,
    NeedLogin(String),
    Page(Page, bool),
    MoreTracks(u64, Vec<Track>),
    Err(String),
    Audio(u64, i64, Vec<u8>),
    PlayErr(u64, String),
    Prefetched(i64, Vec<u8>),
    Img(String, Option<egui::ColorImage>),
    Lyrics(i64, Result<(Vec<(f32, String)>, bool), String>),
}

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Tracks,
    Lists,
    Albums,
    Artists,
}

enum Action {
    Open(Card),
    Home,
    Library,
    Search(String),
    Back,
    Tab(Tab),
    Play(Vec<Track>, usize),
    PlayShuffled(Vec<Track>),
    PlayIndex(usize),
    PlayNext(Track),
    Enqueue(Track),
    Remove(usize),
    ClearQueue,
    PlayBtn,
    PauseBtn,
    StopBtn,
    Toggle,
    Next,
    Prev,
    Seek(f32),
    SeekRel(f32),
    Volume(f32),
    Shuffle,
    Repeat,
    ToggleLossless,
    ClearStatus,
    CopyLog,
    ToggleArt,
    ToggleGray,
    ToggleLyrics,
    ToggleFullscreen,
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

#[derive(PartialEq, Clone, Copy)]
enum RowState {
    Normal,
    Playing,
    Played,
}

// ---------------------------------------------------------------- random
struct Rng(u64);

impl Rng {
    fn new() -> Rng {
        let n = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
        Rng(n ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

fn shuffle_vec<T>(v: &mut Vec<T>) {
    let mut r = Rng::new();
    for i in (1..v.len()).rev() {
        let j = (r.next() % (i as u64 + 1)) as usize;
        v.swap(i, j);
    }
}

/// `tracks` reordered randomly, with tracks[first] moved to the front.
fn shuffled_from(tracks: &[Track], first: usize) -> Vec<Track> {
    let mut rest: Vec<Track> = tracks.to_vec();
    if rest.is_empty() {
        return rest;
    }
    let f = rest.remove(first.min(rest.len() - 1));
    shuffle_vec(&mut rest);
    let mut v = Vec::with_capacity(rest.len() + 1);
    v.push(f);
    v.extend(rest);
    v
}

// --------------------------------------------------------------- marquee
/// Scrolling bitmap text inside `rect` (static if it fits).
fn marquee(ui: &egui::Ui, rect: Rect, text: &str, px: f32, color: Color32) {
    let px = spx(px);
    let w = text_w(text, px);
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
                fill_rect(p, Rect::from_min_size(min, Vec2::splat(px)), color);
            }
        }
    }
}

// ------------------------------------------------------------ skin drawing
/// Filled rectangle, snapped to whole physical pixels (crisp edges, never thinner than 1 px).
fn fill_rect(p: &egui::Painter, r: Rect, c: Color32) {
    if r.width() <= 0.0 || r.height() <= 0.0 {
        return;
    }
    let e = thick(1.0);
    let min = Pos2::new(snap(r.min.x), snap(r.min.y));
    let mut max = Pos2::new(snap(r.max.x), snap(r.max.y));
    if max.x - min.x < e {
        max.x = min.x + e;
    }
    if max.y - min.y < e {
        max.y = min.y + e;
    }
    p.rect_filled(Rect::from_min_max(min, max), Rounding::same(0.0), c);
}

/// One-pixel-style outline drawn inside `r`.
fn outline(p: &egui::Painter, r: Rect, t: f32, c: Color32) {
    fill_rect(p, Rect::from_min_size(r.min, Vec2::new(r.width(), t)), c);
    fill_rect(p, Rect::from_min_size(Pos2::new(r.min.x, r.max.y - t), Vec2::new(r.width(), t)), c);
    fill_rect(p, Rect::from_min_size(r.min, Vec2::new(t, r.height())), c);
    fill_rect(p, Rect::from_min_size(Pos2::new(r.max.x - t, r.min.y), Vec2::new(t, r.height())), c);
}

/// Sunken panel (LCD windows, groove).
fn inset(p: &egui::Painter, r: Rect, fill: Color32) {
    let t1 = thick(1.5);
    let t2 = thick(1.0);
    fill_rect(p, r, fill);
    fill_rect(p, Rect::from_min_size(r.min, Vec2::new(r.width(), t1)), INK);
    fill_rect(p, Rect::from_min_size(r.min, Vec2::new(t1, r.height())), INK);
    fill_rect(p, Rect::from_min_size(Pos2::new(r.min.x, r.max.y - t2), Vec2::new(r.width(), t2)), BEIGE_LT);
    fill_rect(p, Rect::from_min_size(Pos2::new(r.max.x - t2, r.min.y), Vec2::new(t2, r.height())), BEIGE_LT);
}

/// Raised bevelled button face.
fn raised_h(p: &egui::Painter, r: Rect, down: bool, hover: bool) {
    let (hi, lo) = if down { (BEIGE_DK, BEIGE_LT) } else { (BEIGE_LT, BEIGE_DK) };
    let face = if down {
        SEL
    } else if hover {
        Color32::from_rgb(213, 211, 174)
    } else {
        BEIGE
    };
    let t = thick(1.0);
    fill_rect(p, r, face);
    let i = r.shrink(t);
    fill_rect(p, Rect::from_min_size(i.min, Vec2::new(i.width(), t)), hi);
    fill_rect(p, Rect::from_min_size(i.min, Vec2::new(t, i.height())), hi);
    fill_rect(p, Rect::from_min_size(Pos2::new(i.min.x, i.max.y - t), Vec2::new(i.width(), t)), lo);
    fill_rect(p, Rect::from_min_size(Pos2::new(i.max.x - t, i.min.y), Vec2::new(t, i.height())), lo);
    outline(p, r, t, INK);
}

fn raised(p: &egui::Painter, r: Rect, down: bool) {
    raised_h(p, r, down, false);
}

/// Black rounded window frame with beige trim, title tab and beige body at `inner`.
fn window_deco(ui: &egui::Ui, inner: Rect, title: &str) {
    let p = ui.painter();
    let outer = Rect::from_min_max(inner.min - Vec2::new(14.0, 30.0), inner.max + Vec2::new(14.0, 14.0));
    p.rect_filled(outer, Rounding::same(18.0), Color32::BLACK);
    p.rect_stroke(outer.shrink(2.5), Rounding::same(16.0), Stroke::new(1.5, TRIM));
    p.rect_stroke(outer.shrink(5.5), Rounding::same(13.0), Stroke::new(1.0, Color32::from_rgb(70, 69, 50)));
    // title tab
    let label = format!("-[ {} ]-", title);
    let cy = outer.min.y + 12.0;
    let tw = (text_w(&label, 2.0) + 30.0).min(outer.width() - 120.0).max(60.0);
    let tab = Rect::from_center_size(Pos2::new(outer.center().x, cy), Vec2::new(tw, 22.0));
    p.rect_filled(tab.expand(3.0), Rounding::same(6.0), Color32::BLACK);
    fill_rect(p, Rect::from_min_max(Pos2::new(outer.min.x + 26.0, cy - 1.0), Pos2::new(tab.min.x - 6.0, cy + 1.0)), TRIM);
    fill_rect(p, Rect::from_min_max(Pos2::new(tab.max.x + 6.0, cy - 1.0), Pos2::new(outer.max.x - 26.0, cy + 1.0)), TRIM);
    ptext_fit(p, tab.center(), Align::Center, &label, 2.0, tw - 12.0, TRIM);
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
    let px = px.floor().max(1.0);
    let o = Pos2::new(snap(center.x - 3.5 * px), snap(center.y - 3.5 * px + off));
    pixmap(p, o, px, icon, Color32::from_rgb(34, 32, 16));
    resp
}

fn retro_btn_w(ui: &mut egui::Ui, text: &str, w: f32, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::click());
    let down = resp.is_pointer_button_down_on() || active;
    raised_h(ui.painter(), rect, down, resp.hovered());
    let dy = if down { 1.0 } else { 0.0 };
    ptext_fit(ui.painter(), rect.center() + Vec2::new(0.0, dy), Align::Center, text, 2.0, w - 10.0, INK);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Button sized to its label.
fn retro_btn(ui: &mut egui::Ui, text: &str, active: bool) -> egui::Response {
    let w = (text_w(text, 2.0) + 26.0).max(46.0);
    retro_btn_w(ui, text, w, active)
}

fn section_header(ui: &mut egui::Ui, text: &str) {
    ui.add_space(6.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
    fill_rect(ui.painter(), rect, INK);
    ptext(ui.painter(), Pos2::new(rect.min.x + 8.0, rect.center().y), Align::Min, text, 2.0, TRIM);
}

/// Thin sub-heading bar used inside the queue.
fn mini_header(ui: &mut egui::Ui, text: &str) {
    let w = ui.available_width();
    let probe = Rect::from_min_size(ui.cursor().min, Vec2::new(w, HEAD_H));
    if !ui.is_rect_visible(probe) {
        ui.allocate_space(Vec2::new(w, HEAD_H));
        return;
    }
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, HEAD_H), Sense::hover());
    fill_rect(ui.painter(), rect, GROOVE);
    ptext(ui.painter(), Pos2::new(rect.min.x + 8.0, rect.center().y), Align::Min, text, 1.0, BEIGE_LT);
}

/// One list row. Text is only built (and drawn) when the row is on screen.
fn list_row(
    ui: &mut egui::Ui,
    idx: usize,
    num: Option<usize>,
    left: impl FnOnce() -> String,
    right: impl FnOnce() -> String,
    state: RowState,
) -> Option<egui::Response> {
    let w = ui.available_width();
    let probe = Rect::from_min_size(ui.cursor().min, Vec2::new(w, ROW_H));
    if !ui.is_rect_visible(probe) {
        ui.allocate_space(Vec2::new(w, ROW_H));
        return None;
    }
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, ROW_H), Sense::click());
    let p = ui.painter();
    let bg = if state == RowState::Playing {
        Some(ROW_SEL)
    } else if resp.hovered() {
        Some(SEL)
    } else if idx % 2 == 1 {
        Some(ROW_ALT)
    } else {
        None
    };
    if let Some(c) = bg {
        fill_rect(p, rect, c);
    }
    let (fg, fg2) = match state {
        RowState::Playing => (BEIGE_LT, BEIGE_LT),
        RowState::Played => (DIM, DIM),
        RowState::Normal => (INK, INK2),
    };
    let cy = rect.center().y;
    let rtxt = right();
    let rw = ptext(p, Pos2::new(rect.max.x - 10.0, cy), Align::Max, &rtxt, 2.0, fg2);
    let x0 = rect.min.x + 8.0;
    let mut tx = x0;
    if let Some(n) = num {
        if state == RowState::Playing {
            pixmap(p, Pos2::new(x0 + 2.0, cy - 5.0), 2.0, &PLAY_S[..], BEIGE_LT);
        } else {
            ptext_fit(p, Pos2::new(x0 + 34.0, cy), Align::Max, &n.to_string(), 2.0, 34.0, fg2);
        }
        tx = x0 + 44.0;
    }
    let avail = (rect.max.x - 10.0 - rw - 16.0) - tx;
    let ltxt = fit(&left(), 2.0, avail.max(20.0));
    ptext(p, Pos2::new(tx, cy), Align::Min, &ltxt, 2.0, fg);
    Some(resp.on_hover_cursor(egui::CursorIcon::PointingHand))
}

/// Retro-styled entry for right-click menus. Returns true when clicked.
fn menu_item(ui: &mut egui::Ui, text: &str) -> bool {
    let w = (text_w(text, 2.0) + 28.0).max(150.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::click());
    if resp.hovered() {
        fill_rect(ui.painter(), rect, SEL);
    }
    ptext(ui.painter(), Pos2::new(rect.min.x + 10.0, rect.center().y), Align::Min, text, 2.0, INK);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// Single heading line, shortened with "..." if it doesn't fit.
fn title_line(ui: &mut egui::Ui, text: &str, px: f32, col: Color32) {
    let h = 7.0 * spx(px) + 10.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
    let t = fit(text, px, rect.width() - 4.0);
    ptext(ui.painter(), Pos2::new(rect.min.x + 2.0, rect.center().y), Align::Min, &t, px, col);
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

fn app_icon() -> egui::IconData {
    let n = 32usize;
    let mut rgba = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let dx = x as f32 + 0.5 - 16.0;
            let dy = y as f32 + 0.5 - 16.0;
            let d = (dx * dx + dy * dy).sqrt();
            let mut c: Option<[u8; 3]> = None;
            if d <= 15.5 {
                c = Some([8, 8, 10]);
            }
            if d <= 14.0 {
                c = Some([186, 184, 136]);
            }
            if d <= 12.5 {
                c = Some([150, 146, 92]);
            }
            if d <= 9.0 && dy < -3.0 && dx < 0.0 && d > 6.0 {
                c = Some([190, 186, 128]);
            }
            let fx = x as f32 + 0.5;
            if fx >= 12.0 && fx <= 23.0 && dy.abs() <= (23.5 - fx) * 0.78 {
                c = Some([34, 32, 16]);
            }
            if let Some(c) = c {
                let i = (y * n + x) * 4;
                rgba[i] = c[0];
                rgba[i + 1] = c[1];
                rgba[i + 2] = c[2];
                rgba[i + 3] = 255;
            }
        }
    }
    egui::IconData { rgba, width: n as u32, height: n as u32 }
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
            let (gray, real) = match url.strip_prefix("gray:") {
                Some(u) => (true, u.to_string()),
                None => (false, url.clone()),
            };
            let img = api.fetch(&real).ok().and_then(|b| image::load_from_memory(&b).ok()).map(|im| {
                let im = if gray { im.grayscale() } else { im };
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
    paint_art_g(ui, images, url, rect, round, false);
}

/// Texture for a cover, optionally the black-and-white version (falls back to colour while it loads).
fn art_tex(images: &mut Images, url: &str, gray: bool) -> Option<egui::TextureId> {
    if gray {
        if let Some(id) = images.get(&format!("gray:{}", url)) {
            return Some(id);
        }
    }
    images.get(url)
}

fn paint_art_g(ui: &egui::Ui, images: &mut Images, url: &str, rect: Rect, round: f32, gray: bool) {
    ui.painter().rect_filled(rect, Rounding::same(round), INK2);
    if let Some(id) = art_tex(images, url, gray) {
        egui::Image::new(egui::load::SizedTexture::new(id, rect.size()))
            .rounding(Rounding::same(round))
            .paint_at(ui, rect);
    }
}

// ------------------------------------------------------------------ page view
fn tracks_list(ui: &mut egui::Ui, tracks: &[Track], playing_id: Option<i64>, acts: &mut Vec<Action>) {
    for (i, t) in tracks.iter().enumerate() {
        let state = if playing_id == Some(t.id) { RowState::Playing } else { RowState::Normal };
        let r = list_row(
            ui,
            i,
            Some(i + 1),
            || format!("{} - {}", t.artist, t.title),
            || fmt_time(t.duration),
            state,
        );
        if let Some(r) = r {
            if r.clicked() {
                acts.push(Action::Play(tracks.to_vec(), i));
            }
            r.context_menu(|ui| {
                if menu_item(ui, "Play next") {
                    acts.push(Action::PlayNext(t.clone()));
                    ui.close_menu();
                }
                if menu_item(ui, "Add to queue") {
                    acts.push(Action::Enqueue(t.clone()));
                    ui.close_menu();
                }
            });
        }
    }
}

fn cards_list(ui: &mut egui::Ui, cards: &[Card], tags: bool, acts: &mut Vec<Action>) {
    for (i, c) in cards.iter().enumerate() {
        let r = list_row(
            ui,
            i,
            None,
            || {
                if tags {
                    let tag = match c.kind {
                        Kind::Album => "ALBUM",
                        Kind::Playlist => "LIST",
                        Kind::Artist => "ARTIST",
                        Kind::Mix => "MIX",
                    };
                    format!("[{}]  {}", tag, c.title)
                } else {
                    c.title.clone()
                }
            },
            || c.subtitle.clone(),
            RowState::Normal,
        );
        if let Some(r) = r {
            if r.clicked() {
                acts.push(Action::Open(c.clone()));
            }
        }
    }
}

fn play_buttons(ui: &mut egui::Ui, tracks: &[Track], acts: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        if retro_btn(ui, "PLAY", false).clicked() {
            acts.push(Action::Play(tracks.to_vec(), 0));
        }
        if retro_btn(ui, "SHUFFLE", false).clicked() {
            acts.push(Action::PlayShuffled(tracks.to_vec()));
        }
    });
}

fn page_view(
    ui: &mut egui::Ui,
    images: &mut Images,
    page: &Page,
    playing_id: Option<i64>,
    tab: Tab,
    acts: &mut Vec<Action>,
) {
    ui.add_space(6.0);

    // ---- "Your Library": one tab at a time, MY TRACKS first
    if page.library {
        match tab {
            Tab::Tracks => {
                title_line(ui, "MY TRACKS", 3.0, INK);
                let sub = if page.total > page.tracks.len() {
                    format!("{} of {} songs loaded...", page.tracks.len(), page.total)
                } else {
                    format!("{} songs", page.tracks.len())
                };
                title_line(ui, &sub, 2.0, INK2);
                if !page.tracks.is_empty() {
                    ui.add_space(4.0);
                    play_buttons(ui, &page.tracks, acts);
                }
                ui.add_space(8.0);
                if page.tracks.is_empty() {
                    title_line(ui, "No liked songs found.", 2.0, INK2);
                }
                tracks_list(ui, &page.tracks, playing_id, acts);
            }
            t => {
                let (key, name) = match t {
                    Tab::Lists => ("Playlists", "PLAYLISTS"),
                    Tab::Albums => ("Albums", "ALBUMS"),
                    _ => ("Artists", "ARTISTS"),
                };
                title_line(ui, name, 3.0, INK);
                ui.add_space(6.0);
                let mut any = false;
                for (title, cards) in &page.rows {
                    if title == key {
                        any = true;
                        title_line(ui, &format!("{} items", cards.len()), 2.0, INK2);
                        ui.add_space(4.0);
                        cards_list(ui, cards, false, acts);
                    }
                }
                if !any {
                    title_line(ui, "Nothing here yet.", 2.0, INK2);
                }
            }
        }
        ui.add_space(20.0);
        return;
    }

    // ---- album / playlist / artist / search / home
    let has_header = !page.image.is_empty() || !page.subtitle.is_empty();
    if has_header {
        ui.horizontal(|ui| {
            let (cr, _) = ui.allocate_exact_size(Vec2::splat(104.0), Sense::hover());
            inset(ui.painter(), cr, INK);
            paint_art(ui, images, &page.image, cr.shrink(2.0), 0.0);
            ui.vertical(|ui| {
                title_line(ui, &page.title, 3.0, INK);
                title_line(ui, &page.subtitle, 2.0, INK2);
                ui.add_space(6.0);
                if !page.tracks.is_empty() {
                    play_buttons(ui, &page.tracks, acts);
                }
            });
        });
    } else {
        title_line(ui, &page.title, 3.0, INK);
    }
    ui.add_space(6.0);

    if page.tracks.is_empty() && page.rows.is_empty() {
        title_line(ui, "Nothing to show.", 2.0, INK2);
    }
    for (title, cards) in &page.rows {
        section_header(ui, title);
        cards_list(ui, cards, true, acts);
    }
    if !page.tracks.is_empty() {
        if !page.rows.is_empty() {
            section_header(ui, "SONGS");
        }
        tracks_list(ui, &page.tracks, playing_id, acts);
    }
    ui.add_space(20.0);
}

fn push_tracks(p: &mut Arc<Page>, v: &[Track]) {
    let pg = Arc::make_mut(p);
    let have: HashSet<i64> = pg.tracks.iter().map(|t| t.id).collect();
    pg.tracks.extend(v.iter().filter(|t| !have.contains(&t.id)).cloned());
}

// -------------------------------------------------------------------- lyrics
struct Lyrics {
    id: i64,
    /// 0 = loading, 1 = ready, 2 = none available
    status: u8,
    lines: Vec<(f32, String)>,
    synced: bool,
}

// ----------------------------------------------------------------------- app
struct App {
    api: Api,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    ctx: egui::Context,
    images: Images,
    _font_tex: egui::TextureHandle,

    auth: Auth,
    login_code: Option<(String, String)>,
    login_err: String,

    page: Option<Arc<Page>>,
    back: Vec<Arc<Page>>,
    serial: u64,
    lib_gen: u64,
    lib_tab: Tab,
    loading: bool,
    search: String,
    search_cur: usize,
    search_focus: bool,
    status: String,
    status_err: bool,

    queue: Arc<Vec<Track>>,
    /// Original (un-shuffled) order, kept while shuffle is on so it can be restored.
    orig_queue: Option<Vec<Track>>,
    cur: Option<usize>,
    q_scroll: bool,
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

    art_view: bool,
    art_gray: bool,
    show_lyrics: bool,
    fullscreen: bool,
    art_tilt: Vec2,
    lyrics: Option<Lyrics>,
    lyric_scroll: f32,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> App {
        font::set_ppp(cc.egui_ctx.pixels_per_point());
        let font_tex = font::init(&cc.egui_ctx);
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
            _font_tex: font_tex,
            auth: Auth::LoggedOut,
            login_code: None,
            login_err: String::new(),
            page: None,
            back: Vec::new(),
            serial: 0,
            lib_gen: 0,
            lib_tab: Tab::Tracks,
            loading: false,
            search: String::new(),
            search_cur: 0,
            search_focus: false,
            status: String::new(),
            status_err: false,
            queue: Arc::new(Vec::new()),
            orig_queue: None,
            cur: None,
            q_scroll: false,
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
            art_view: false,
            art_gray: false,
            show_lyrics: true,
            fullscreen: false,
            art_tilt: Vec2::ZERO,
            lyrics: None,
            lyric_scroll: 0.0,
        };
        crate::api::log(&format!("tidalite started (token saved: {})", app.api.has_token()));
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

    /// Fetch the rest of the liked songs in the background (the first 100 arrive with the page).
    fn spawn_more_tracks(&self, start: usize, total: usize, gen: u64) {
        let (api, tx, ctx) = (self.api.clone(), self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let mut off = start;
            while off < total {
                match api.fav_tracks(off) {
                    Ok((t, _total, raw)) => {
                        if raw == 0 {
                            break;
                        }
                        off += raw;
                        let _ = tx.send(Msg::MoreTracks(gen, t));
                        ctx.request_repaint();
                    }
                    Err(e) => {
                        crate::api::log(&format!("liked songs: stopped at {} ({})", off, e));
                        break;
                    }
                }
            }
        });
    }

    fn fetch_lyrics(&mut self, id: i64) {
        self.lyrics = Some(Lyrics { id, status: 0, lines: Vec::new(), synced: false });
        self.lyric_scroll = 0.0;
        let (api, tx, ctx) = (self.api.clone(), self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let r = api.lyrics(id);
            let _ = tx.send(Msg::Lyrics(id, r));
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
                    crate::api::log(&format!(
                        "page '{}': {} rows, {} tracks (of {})",
                        p.title,
                        p.rows.len(),
                        p.tracks.len(),
                        p.total
                    ));
                    if p.library {
                        self.lib_gen += 1;
                        if p.total > p.tracks.len() {
                            self.spawn_more_tracks(p.tracks.len(), p.total, self.lib_gen);
                        }
                    }
                    if push {
                        if let Some(old) = self.page.take() {
                            self.back.push(old);
                        }
                    }
                    self.page = Some(Arc::new(p));
                    self.serial += 1;
                    self.loading = false;
                }
                Msg::MoreTracks(g, v) => {
                    if g == self.lib_gen {
                        let mut done = false;
                        if let Some(p) = self.page.as_mut() {
                            if p.library {
                                push_tracks(p, &v);
                                done = true;
                            }
                        }
                        if !done {
                            for b in self.back.iter_mut() {
                                if b.library {
                                    push_tracks(b, &v);
                                    break;
                                }
                            }
                        }
                    }
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
                Msg::Lyrics(id, r) => {
                    if let Some(l) = self.lyrics.as_mut() {
                        if l.id == id {
                            match r {
                                Ok((lines, synced)) => {
                                    l.lines = lines;
                                    l.synced = synced;
                                    l.status = 1;
                                }
                                Err(e) => {
                                    crate::api::log(&format!("lyrics: none for {} ({})", id, e));
                                    l.status = 2;
                                }
                            }
                            self.lyric_scroll = 0.0;
                        }
                    }
                }
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
        self.q_scroll = true;
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
        if cur + 1 < n {
            Some(cur + 1)
        } else if self.repeat == Repeat::All {
            Some(0)
        } else {
            None
        }
    }

    fn next_track(&self) -> Option<Track> {
        if self.repeat == Repeat::One {
            return self.cur_track();
        }
        let cur = self.cur?;
        self.queue.get(cur + 1).cloned().or_else(|| {
            if self.repeat == Repeat::All {
                self.queue.first().cloned()
            } else {
                None
            }
        })
    }

    fn prefetch_next(&mut self, _playing_id: i64) {
        if self.repeat == Repeat::One {
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

    /// The queue order changed: forget the old prefetch and fetch the new "next" track.
    fn refresh_prefetch(&mut self) {
        self.prefetched = None;
        self.prefetching = 0;
        if self.cur.is_some() && !self.stopped && !self.buffering {
            self.prefetch_next(0);
        }
    }

    fn pos(&self) -> f32 {
        self.player.shared.lock().unwrap().pos
    }

    fn cur_track(&self) -> Option<Track> {
        self.cur.and_then(|i| self.queue.get(i).cloned())
    }

    fn set_shuffle(&mut self, on: bool) {
        if on == self.shuffle {
            return;
        }
        self.shuffle = on;
        if on {
            let orig: Vec<Track> = (*self.queue).clone();
            let new = match self.cur {
                Some(c) if c < orig.len() => {
                    self.cur = Some(0);
                    shuffled_from(&orig, c)
                }
                _ => {
                    let mut v = orig.clone();
                    shuffle_vec(&mut v);
                    v
                }
            };
            self.orig_queue = Some(orig);
            self.queue = Arc::new(new);
        } else if let Some(orig) = self.orig_queue.take() {
            let cur_id = self.cur_track().map(|t| t.id);
            let pos = cur_id.and_then(|id| orig.iter().position(|t| t.id == id));
            self.queue = Arc::new(orig);
            if self.cur.is_some() {
                self.cur = Some(pos.unwrap_or(0));
            }
        }
        self.q_scroll = true;
        self.refresh_prefetch();
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
            Action::Tab(t) => {
                self.lib_tab = t;
                self.serial += 1; // scroll back to the top
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
                if tracks.is_empty() {
                    return;
                }
                if self.shuffle {
                    let v = shuffled_from(&tracks, i);
                    self.orig_queue = Some(tracks);
                    self.queue = Arc::new(v);
                    self.play_index(0);
                } else {
                    self.orig_queue = None;
                    self.queue = Arc::new(tracks);
                    self.play_index(i);
                }
            }
            Action::PlayShuffled(tracks) => {
                if tracks.is_empty() {
                    return;
                }
                let mut v = tracks.clone();
                shuffle_vec(&mut v);
                self.shuffle = true;
                self.orig_queue = Some(tracks);
                self.queue = Arc::new(v);
                self.play_index(0);
            }
            Action::PlayIndex(i) => self.play_index(i),
            Action::PlayNext(t) => {
                let at = self.cur.map(|c| c + 1).unwrap_or(0).min(self.queue.len());
                // keep the un-shuffled copy in step (right after the current track)
                let cur_id = self.cur_track().map(|c| c.id);
                if let Some(o) = self.orig_queue.as_mut() {
                    let p = cur_id.and_then(|id| o.iter().position(|x| x.id == id)).map(|p| p + 1).unwrap_or(o.len());
                    o.insert(p.min(o.len()), t.clone());
                }
                Arc::make_mut(&mut self.queue).insert(at, t);
                if self.cur.is_none() {
                    self.play_index(0);
                } else {
                    self.refresh_prefetch();
                }
            }
            Action::Enqueue(t) => {
                if let Some(o) = self.orig_queue.as_mut() {
                    o.push(t.clone());
                }
                Arc::make_mut(&mut self.queue).push(t);
                if self.cur.is_none() {
                    self.play_index(0);
                }
            }
            Action::Remove(i) => {
                if i >= self.queue.len() || Some(i) == self.cur {
                    return;
                }
                let t = Arc::make_mut(&mut self.queue).remove(i);
                if let Some(c) = self.cur {
                    if i < c {
                        self.cur = Some(c - 1);
                    }
                }
                if let Some(o) = self.orig_queue.as_mut() {
                    if let Some(p) = o.iter().position(|x| x.id == t.id) {
                        o.remove(p);
                    }
                }
                self.refresh_prefetch();
            }
            Action::ClearQueue => {
                if let Some(t) = self.cur_track() {
                    self.queue = Arc::new(vec![t]);
                    self.cur = Some(0);
                } else {
                    self.queue = Arc::new(Vec::new());
                }
                self.orig_queue = if self.shuffle { Some((*self.queue).clone()) } else { None };
                self.prefetched = None;
                self.prefetching = 0;
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
            Action::SeekRel(d) => {
                if self.cur.is_some() && !self.stopped {
                    let dur = self.cur_track().map(|t| t.duration).unwrap_or(0.0);
                    let t = (self.pos() + d).clamp(0.0, (dur - 1.0).max(0.0));
                    self.player.send(Cmd::Seek(t));
                }
            }
            Action::Volume(v) => {
                self.volume = v;
                self.player.send(Cmd::Volume(v * v));
            }
            Action::Shuffle => {
                let on = !self.shuffle;
                self.set_shuffle(on);
                self.set_note(if on { "SHUFFLE ON - QUEUE REORDERED" } else { "SHUFFLE OFF - ORIGINAL ORDER" });
            }
            Action::Repeat => {
                self.repeat = match self.repeat {
                    Repeat::Off => Repeat::All,
                    Repeat::All => Repeat::One,
                    Repeat::One => Repeat::Off,
                };
                self.refresh_prefetch();
            }
            Action::ToggleLossless => {
                self.prefer_lossless = !self.prefer_lossless;
                self.prefetched = None;
                self.prefetching = 0;
                let msg = if self.prefer_lossless { "HIFI LOSSLESS - NEXT TRACK" } else { "320K AAC - NEXT TRACK" };
                self.set_note(msg);
            }
            Action::ClearStatus => self.status.clear(),
            Action::CopyLog => {
                let text = crate::api::log_text();
                self.ctx.output_mut(|o| o.copied_text = text);
                self.set_note("LOG COPIED TO CLIPBOARD");
            }
            Action::ToggleArt => self.art_view = !self.art_view,
            Action::ToggleGray => self.art_gray = !self.art_gray,
            Action::ToggleLyrics => self.show_lyrics = !self.show_lyrics,
            Action::ToggleFullscreen => {
                self.fullscreen = !self.fullscreen;
                self.ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
            }
            Action::Logout => {
                self.art_view = false;
                self.player.send(Cmd::Stop);
                self.api.logout();
                self.auth = Auth::LoggedOut;
                self.page = None;
                self.back.clear();
                self.queue = Arc::new(Vec::new());
                self.orig_queue = None;
                self.cur = None;
                self.stopped = true;
                self.shuffle = false;
            }
        }
    }

    // ---------------------------------------------------------------- views
    fn login_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        let full = ui.max_rect();
        let inner = Rect::from_center_size(full.center(), Vec2::new(480.0, 270.0));
        window_deco(ui, inner, "TIDALITE");
        let p = ui.painter();
        let c = inner.center();
        ptext(p, c + Vec2::new(0.0, -84.0), Align::Center, "TIDALITE", 7.0, INK);
        ptext(p, c + Vec2::new(0.0, -40.0), Align::Center, "A retro player for Tidal", 2.0, INK2);
        match self.auth {
            Auth::Checking => {
                ptext(p, c + Vec2::new(0.0, 24.0), Align::Center, "Checking session...", 2.0, INK);
            }
            Auth::LoggingIn => match &self.login_code {
                Some((code, url)) => {
                    ptext(p, c + Vec2::new(0.0, -6.0), Align::Center, "Approve the login in your browser", 2.0, INK);
                    ptext(p, c + Vec2::new(0.0, 34.0), Align::Center, code, 5.0, INK);
                    let b = Rect::from_center_size(c + Vec2::new(0.0, 86.0), Vec2::new(300.0, 30.0));
                    let r = ui.interact(b, ui.id().with("relink"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    raised_h(ui.painter(), b, r.is_pointer_button_down_on(), r.hovered());
                    ptext(ui.painter(), b.center(), Align::Center, "Open link again", 2.0, INK);
                    if r.clicked() {
                        let _ = webbrowser::open(url);
                    }
                }
                None => {
                    ptext(p, c + Vec2::new(0.0, 24.0), Align::Center, "Contacting Tidal...", 2.0, INK);
                }
            },
            _ => {
                let b = Rect::from_center_size(c + Vec2::new(0.0, 30.0), Vec2::new(320.0, 44.0));
                let r = ui.interact(b, ui.id().with("login"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                raised_h(ui.painter(), b, r.is_pointer_button_down_on(), r.hovered());
                ptext(ui.painter(), b.center(), Align::Center, "LOG IN WITH TIDAL", 3.0, INK);
                if r.clicked() {
                    acts.push(Action::StartLogin);
                }
                if !self.login_err.is_empty() {
                    let er = Rect::from_center_size(c + Vec2::new(0.0, 96.0), Vec2::new(430.0, 22.0));
                    inset(ui.painter(), er, LCD);
                    marquee(ui, er, &self.login_err, 2.0, RED);
                }
            }
        }
    }

    /// Single-line text field drawn in the pixel font.
    fn search_input(&mut self, ui: &mut egui::Ui, rect: Rect, acts: &mut Vec<Action>) {
        let resp = ui.interact(rect, ui.id().with("search_box"), Sense::click());
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
        if resp.clicked() {
            self.search_focus = true;
            self.search_cur = self.search.chars().count();
        } else if self.search_focus && ui.input(|i| i.pointer.primary_pressed()) && !resp.hovered() {
            self.search_focus = false;
        }

        if self.search_focus {
            let events = ui.input(|i| i.events.clone());
            let mut chars: Vec<char> = self.search.chars().collect();
            let mut cur = self.search_cur.min(chars.len());
            let mut go = false;
            for ev in events {
                match ev {
                    egui::Event::Text(t) | egui::Event::Paste(t) => {
                        for c in t.chars() {
                            if !c.is_control() {
                                chars.insert(cur, c);
                                cur += 1;
                            }
                        }
                    }
                    egui::Event::Key { key, pressed: true, modifiers, .. } => match key {
                        egui::Key::Backspace => {
                            if modifiers.ctrl {
                                while cur > 0 && chars[cur - 1] == ' ' {
                                    chars.remove(cur - 1);
                                    cur -= 1;
                                }
                                while cur > 0 && chars[cur - 1] != ' ' {
                                    chars.remove(cur - 1);
                                    cur -= 1;
                                }
                            } else if cur > 0 {
                                chars.remove(cur - 1);
                                cur -= 1;
                            }
                        }
                        egui::Key::Delete => {
                            if cur < chars.len() {
                                chars.remove(cur);
                            }
                        }
                        egui::Key::ArrowLeft => cur = cur.saturating_sub(1),
                        egui::Key::ArrowRight => cur = (cur + 1).min(chars.len()),
                        egui::Key::Home => cur = 0,
                        egui::Key::End => cur = chars.len(),
                        egui::Key::Enter => go = true,
                        egui::Key::Escape => self.search_focus = false,
                        _ => {}
                    },
                    _ => {}
                }
            }
            self.search = chars.into_iter().collect();
            self.search_cur = cur;
            if go {
                acts.push(Action::Search(self.search.clone()));
            }
        }

        let px = 2.0;
        let p = ui.painter();
        inset(p, rect, LCD);
        let inner = rect.shrink2(Vec2::new(8.0, 2.0));
        let cp = p.with_clip_rect(inner);
        let cy = rect.center().y;
        let prefix: String = self.search.chars().take(self.search_cur).collect();
        let caret_x = if prefix.is_empty() { 0.0 } else { text_w(&prefix, px) + spx(px) };
        let off = (caret_x - (inner.width() - 6.0)).max(0.0);
        if self.search.is_empty() {
            ptext(&cp, Pos2::new(inner.min.x, cy), Align::Min, "Search Tidal...", px, DIM);
        } else {
            ptext(&cp, Pos2::new(inner.min.x - off, cy), Align::Min, &self.search, px, INK);
        }
        let t = ui.input(|i| i.time);
        if self.search_focus && (t * 2.0) as i64 % 2 == 0 {
            let r = Rect::from_min_size(Pos2::new(inner.min.x - off + caret_x, cy - 8.0), Vec2::new(spx(2.0), 16.0));
            fill_rect(&cp, r, INK);
        }
    }

    fn log_view(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            if retro_btn(ui, "COPY LOG", false).clicked() {
                acts.push(Action::CopyLog);
            }
            if retro_btn(ui, "CLOSE", false).clicked() {
                self.show_log = false;
            }
        });
        ui.add_space(4.0);
        let w = ui.available_width() - 24.0;
        let text = crate::api::log_text();
        let mut lines: Vec<(String, bool)> = Vec::new();
        for l in text.lines() {
            let bad = l.contains("ERROR") || l.contains("PANIC") || l.contains("DECODE") || l.contains("failed");
            for piece in wrap(l, 2.0, w) {
                lines.push((piece, bad));
            }
        }
        let rh = 20.0;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show_rows(ui, rh, lines.len(), |ui, range| {
                for i in range {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), rh), Sense::hover());
                    let (txt, bad) = &lines[i];
                    ptext(ui.painter(), Pos2::new(rect.min.x + 6.0, rect.center().y), Align::Min, txt, 2.0, if *bad { RED } else { INK });
                }
            });
    }

    fn library_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        window_deco(ui, ui.max_rect(), "LIBRARY");
        ui.add_space(2.0);

        // search row
        ui.horizontal(|ui| {
            let go_w = 54.0;
            let w = (ui.available_width() - go_w - ui.spacing().item_spacing.x).max(80.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::hover());
            self.search_input(ui, rect, acts);
            if retro_btn_w(ui, "GO", go_w, false).clicked() {
                acts.push(Action::Search(self.search.clone()));
            }
        });

        // navigation row
        ui.horizontal(|ui| {
            if retro_btn(ui, "HOME", false).clicked() {
                acts.push(Action::Home);
            }
            if retro_btn(ui, "LIBRARY", false).clicked() {
                acts.push(Action::Library);
            }
            if !self.back.is_empty() && retro_btn(ui, "< BACK", false).clicked() {
                acts.push(Action::Back);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if retro_btn(ui, "LOG OUT", false).clicked() {
                    acts.push(Action::Logout);
                }
                if retro_btn(ui, "LOG", self.show_log).clicked() {
                    self.show_log = !self.show_log;
                }
            });
        });

        // library tabs
        let is_lib = self.page.as_ref().map(|p| p.library).unwrap_or(false);
        if is_lib && !self.show_log {
            ui.horizontal(|ui| {
                for (t, label) in [(Tab::Tracks, "MY TRACKS"), (Tab::Lists, "LISTS"), (Tab::Albums, "ALBUMS"), (Tab::Artists, "ARTISTS")] {
                    if retro_btn(ui, label, self.lib_tab == t).clicked() {
                        acts.push(Action::Tab(t));
                    }
                }
            });
        }

        // persistent error line
        if self.status_err && !self.status.is_empty() {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::click());
            inset(ui.painter(), rect, LCD);
            marquee(ui, rect, &self.status, 2.0, RED);
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                acts.push(Action::ClearStatus);
            }
        }
        ui.add_space(4.0);

        // list well
        let avail = ui.available_rect_before_wrap();
        if avail.height() < 30.0 {
            return;
        }
        inset(ui.painter(), avail, LCD);
        let area = avail.shrink(thick(3.0));
        let page = self.page.clone();
        let serial = self.serial;
        let loading = self.loading;
        let tab = self.lib_tab;
        let playing_id = self.cur_track().map(|t| t.id);
        let show_log = self.show_log;
        ui.allocate_ui_at_rect(area, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
            if show_log {
                self.log_view(ui, acts);
                return;
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.push_id(serial, |ui| {
                    if loading {
                        ui.add_space(6.0);
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
                        ptext(ui.painter(), Pos2::new(rect.min.x + 8.0, rect.center().y), Align::Min, "LOADING...", 2.0, INK2);
                    }
                    if let Some(pg) = &page {
                        page_view(ui, &mut self.images, pg, playing_id, tab, acts);
                    }
                });
            });
        });
    }

    fn player_window(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        let inner = ui.max_rect();
        window_deco(ui, inner, "TIDALITE");
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
        let next = self.next_track();
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
        pixmap(p, Pos2::new(snap(ox + 10.0 * s), snap(oy + 11.0 * s)), (0.9 * s).round().max(1.0), icon, INK);
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
            None => "Tidalite - a retro player for Tidal".to_string(),
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
        } else if let (true, Some(n)) = (active, &next) {
            (format!("NEXT: {} - {}", n.artist, n.title), INK2)
        } else if active {
            ("END OF QUEUE".to_string(), INK2)
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
        ptext_fit(p, kb.center(), Align::Center, &kb_txt, px_sm, kb.width() - 6.0, INK);
        let kh = rc(166.0, 34.0, 40.0, 12.0);
        inset(p, kh, LCD);
        let kh_txt = if active && rate > 0 { format!("{} KHZ", rate / 1000) } else { "-- KHZ".to_string() };
        ptext_fit(p, kh.center(), Align::Center, &kh_txt, px_sm, kh.width() - 6.0, INK);
        let qb = rc(210.0, 34.0, 40.0, 12.0);
        let qr = ui
            .interact(qb, ui.id().with("quality"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        raised_h(p, qb, qr.is_pointer_button_down_on(), qr.hovered());
        ptext_fit(p, qb.center(), Align::Center, if self.prefer_lossless { "HIFI" } else { "320K" }, px_sm, qb.width() - 6.0, INK);
        if qr.clicked() {
            acts.push(Action::ToggleLossless);
        }

        // ---- cover art
        let cv = rc(258.0, 34.0, 36.0, 36.0);
        inset(p, cv, INK);
        if let Some(t) = &track {
            paint_art_g(ui, &mut self.images, &cover_url(&t.cover, 160), cv.shrink(2.0), 0.0, self.art_gray);
        }
        let cvr = ui.interact(cv, ui.id().with("cover"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        if cvr.clicked() {
            acts.push(Action::ToggleArt);
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

        // ---- shuffle / repeat (labels are fitted into their buttons)
        let sh = rc(136.0, 102.0, 74.0, 16.0);
        let shr = ui.interact(sh, ui.id().with("shuf"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        raised_h(p, sh, self.shuffle || shr.is_pointer_button_down_on(), shr.hovered() && !self.shuffle);
        let led = Rect::from_center_size(Pos2::new(sh.min.x + 7.0 * s, sh.center().y), Vec2::splat(3.0 * s));
        fill_rect(p, led, if self.shuffle { RED } else { BEIGE_DK });
        let st = Rect::from_min_max(Pos2::new(sh.min.x + 12.0 * s, sh.min.y), sh.max);
        ptext_fit(p, st.center(), Align::Center, "SHUFFLE", px_sm, st.width() - 6.0, INK);
        if shr.clicked() {
            acts.push(Action::Shuffle);
        }
        let rp = rc(214.0, 102.0, 80.0, 16.0);
        let rpr = ui.interact(rp, ui.id().with("rep"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        raised_h(p, rp, self.repeat != Repeat::Off || rpr.is_pointer_button_down_on(), rpr.hovered() && self.repeat == Repeat::Off);
        let led = Rect::from_center_size(Pos2::new(rp.min.x + 7.0 * s, rp.center().y), Vec2::splat(3.0 * s));
        fill_rect(p, led, if self.repeat != Repeat::Off { RED } else { BEIGE_DK });
        let rp_txt = match self.repeat {
            Repeat::Off => "REPEAT",
            Repeat::All => "REPEAT ALL",
            Repeat::One => "REPEAT 1",
        };
        let rt = Rect::from_min_max(Pos2::new(rp.min.x + 12.0 * s, rp.min.y), rp.max);
        ptext_fit(p, rt.center(), Align::Center, rp_txt, px_sm, rt.width() - 6.0, INK);
        if rpr.clicked() {
            acts.push(Action::Repeat);
        }
    }

    /// Full-window cover viewer: tilts toward the mouse, optional B&W, lyrics, fullscreen.
    fn art_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        let full = ui.max_rect();
        let p = ui.painter().clone();
        let t = ui.input(|i| i.time) as f32;
        let dt = ui.input(|i| i.stable_dt).min(0.05);
        let hover = ui.input(|i| i.pointer.hover_pos());
        let track = self.cur_track();
        let pos = self.pos();
        let dur = track.as_ref().map(|t| t.duration).unwrap_or(0.0);
        let active = self.cur.is_some() && !self.stopped;

        // ---- layout
        let bar_h = 96.0;
        let area = Rect::from_min_max(
            full.min + Vec2::new(28.0, 28.0),
            Pos2::new(full.max.x - 28.0, full.max.y - bar_h - 12.0),
        );
        let lyrics_on = self.show_lyrics && area.width() > 640.0;
        let art_zone = if lyrics_on {
            Rect::from_min_max(area.min, Pos2::new(area.min.x + area.width() * 0.5 - 10.0, area.max.y))
        } else {
            area
        };
        let caption_h = 64.0;
        let a = (art_zone.width().min(art_zone.height() - caption_h) * 0.82).max(120.0);
        let center = Pos2::new(art_zone.center().x, art_zone.min.y + (art_zone.height() - caption_h) / 2.0);

        // ---- ambient glow: the cover, huge and faint, behind everything
        let url = track.as_ref().map(|t| cover_url(&t.cover, 640)).unwrap_or_default();
        let tex = art_tex(&mut self.images, &url, self.art_gray);
        if let Some(id) = tex {
            let side = full.width().max(full.height()) * 1.15;
            let g = Rect::from_center_size(full.center(), Vec2::splat(side));
            p.with_clip_rect(full).image(
                id,
                g,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::from_white_alpha(34),
            );
        }

        // ---- tilt follows the mouse (gentle idle sway when it is elsewhere)
        let target = match hover {
            Some(pp) if full.contains(pp) => Vec2::new(
                ((pp.x - center.x) / (full.width() * 0.5)).clamp(-1.0, 1.0),
                ((pp.y - center.y) / (full.height() * 0.5)).clamp(-1.0, 1.0),
            ),
            _ => Vec2::new((t * 0.6).sin() * 0.3, (t * 0.45).cos() * 0.25),
        };
        let k = 1.0 - (-dt * 9.0).exp();
        self.art_tilt += (target - self.art_tilt) * k;
        let tilt = self.art_tilt;
        let (sth, cth) = (tilt.x * 0.33).sin_cos();
        let (sph, cph) = (-tilt.y * 0.33).sin_cos();
        let dist = a * 3.2;
        let xf = move |x: f32, y: f32| -> Pos2 {
            let x1 = x * cth;
            let z1 = -x * sth;
            let y1 = y * cph - z1 * sph;
            let z2 = y * sph + z1 * cph;
            let sc = dist / (dist - z2);
            Pos2::new(center.x + x1 * sc, center.y + y1 * sc)
        };
        let corners = |h: f32| [xf(-h, -h), xf(h, -h), xf(h, h), xf(-h, h)];
        let h = a / 2.0;

        // shadow, frame, cover
        let off = Vec2::new(-tilt.x * 22.0, -tilt.y * 12.0 + 20.0);
        let shadow: Vec<Pos2> = corners(h + 6.0).iter().map(|c| *c + off).collect();
        p.add(egui::Shape::convex_polygon(shadow, Color32::from_black_alpha(120), Stroke::NONE));
        p.add(egui::Shape::convex_polygon(corners(h + 9.0).to_vec(), Color32::BLACK, Stroke::new(2.0, TRIM)));
        let cs = corners(h);
        match tex {
            Some(id) => {
                let mut mesh = egui::Mesh::with_texture(id);
                let uvs = [Pos2::new(0.0, 0.0), Pos2::new(1.0, 0.0), Pos2::new(1.0, 1.0), Pos2::new(0.0, 1.0)];
                for (c, uv) in cs.iter().zip(uvs.iter()) {
                    mesh.vertices.push(egui::epaint::Vertex { pos: *c, uv: *uv, color: Color32::WHITE });
                }
                mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
                p.add(egui::Shape::mesh(mesh));
            }
            None => {
                p.add(egui::Shape::convex_polygon(cs.to_vec(), INK2, Stroke::NONE));
            }
        }

        // ---- caption
        if let Some(tr) = &track {
            let cx = art_zone.center().x;
            let y0 = art_zone.max.y - caption_h + 14.0;
            let w = art_zone.width() - 20.0;
            ptext_fit(&p, Pos2::new(cx, y0), Align::Center, &tr.title, 3.0, w, BEIGE_LT);
            ptext_fit(&p, Pos2::new(cx, y0 + 30.0), Align::Center, &format!("{} - {}", tr.artist, tr.album), 2.0, w, TRIM);
        } else {
            ptext(&p, art_zone.center(), Align::Center, "Nothing playing", 3.0, TRIM);
        }

        // ---- lyrics
        if lyrics_on {
            let ly = Rect::from_min_max(Pos2::new(area.min.x + area.width() * 0.5 + 10.0, area.min.y), area.max);
            p.rect_filled(ly, Rounding::same(10.0), Color32::from_black_alpha(150));
            p.rect_stroke(ly, Rounding::same(10.0), Stroke::new(1.5, TRIM));
            let cp = p.with_clip_rect(ly.shrink(4.0));
            let lh = 38.0;
            let cyl = ly.center().y;
            let wheel = ui.input(|i| i.raw_scroll_delta.y);
            let over = hover.map(|pp| ly.contains(pp)).unwrap_or(false);
            let mut scroll = self.lyric_scroll;
            let mut msg: Option<&str> = None;
            match &self.lyrics {
                None => msg = Some("Lyrics appear here"),
                Some(l) => {
                    let same = track.as_ref().map(|t| t.id) == Some(l.id);
                    if !same || l.status == 0 {
                        msg = Some("Loading lyrics...");
                    } else if l.status == 2 {
                        msg = Some("No lyrics for this track");
                    } else {
                        let n = l.lines.len();
                        let idx: Option<usize> = if l.synced {
                            l.lines.iter().rposition(|(tt, _)| *tt <= pos + 0.25)
                        } else {
                            None
                        };
                        if l.synced {
                            let tgt = idx.unwrap_or(0) as f32;
                            scroll += (tgt - scroll) * (1.0 - (-dt * 7.0).exp());
                        } else if over {
                            scroll = (scroll - wheel / lh).clamp(0.0, (n.max(1) - 1) as f32);
                        }
                        for (i, (_, txt)) in l.lines.iter().enumerate() {
                            if txt.is_empty() {
                                continue;
                            }
                            let d = i as f32 - scroll;
                            let y = cyl + d * lh;
                            if y < ly.min.y - lh || y > ly.max.y + lh {
                                continue;
                            }
                            let fade = (1.0 - d.abs() / 6.0).clamp(0.15, 1.0);
                            let (px, col) = if idx == Some(i) { (3.0, BEIGE_LT) } else { (2.0, TRIM.gamma_multiply(fade)) };
                            ptext_fit(&cp, Pos2::new(ly.center().x, y), Align::Center, txt, px, ly.width() - 40.0, col);
                        }
                    }
                }
            }
            if let Some(m) = msg {
                ptext_fit(&cp, ly.center(), Align::Center, m, 2.0, ly.width() - 30.0, TRIM);
            }
            self.lyric_scroll = scroll;
        }

        // ---- seek bar
        let sb = Rect::from_min_size(Pos2::new(full.min.x + 28.0, full.max.y - bar_h), Vec2::new(full.width() - 56.0, 12.0));
        let sr = ui.interact(sb, ui.id().with("art_seek"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        if active && dur > 0.0 && sr.clicked() {
            if let Some(pp) = sr.interact_pointer_pos() {
                acts.push(Action::Seek(((pp.x - sb.min.x) / sb.width()).clamp(0.0, 1.0) * dur));
            }
        }
        inset(&p, sb, GROOVE);
        let frac = if active && dur > 0.0 { (pos / dur).clamp(0.0, 1.0) } else { 0.0 };
        fill_rect(
            &p,
            Rect::from_min_max(sb.min + Vec2::new(2.0, 2.0), Pos2::new(sb.min.x + 2.0 + (sb.width() - 4.0) * frac, sb.max.y - 2.0)),
            BEIGE_LT,
        );
        ptext(&p, Pos2::new(sb.min.x, sb.max.y + 14.0), Align::Min, &fmt_time(if active { pos } else { 0.0 }), 2.0, TRIM);
        ptext(&p, Pos2::new(sb.max.x, sb.max.y + 14.0), Align::Max, &fmt_time(dur), 2.0, TRIM);

        // ---- buttons
        let row = Rect::from_min_size(Pos2::new(full.min.x + 28.0, full.max.y - 52.0), Vec2::new(full.width() - 56.0, BTN_H));
        let (gray, lyr, fs) = (self.art_gray, self.show_lyrics, self.fullscreen);
        let playing = active && !self.paused;
        ui.allocate_ui_at_rect(row, |ui| {
            ui.horizontal(|ui| {
                if retro_btn(ui, "PREV", false).clicked() {
                    acts.push(Action::Prev);
                }
                if retro_btn(ui, if playing { "PAUSE" } else { "PLAY" }, false).clicked() {
                    acts.push(Action::Toggle);
                }
                if retro_btn(ui, "NEXT", false).clicked() {
                    acts.push(Action::Next);
                }
                ui.add_space(14.0);
                if retro_btn(ui, if gray { "B&W" } else { "COLOR" }, gray).clicked() {
                    acts.push(Action::ToggleGray);
                }
                if retro_btn(ui, "LYRICS", lyr).clicked() {
                    acts.push(Action::ToggleLyrics);
                }
                if retro_btn(ui, if fs { "WINDOWED" } else { "FULLSCREEN" }, false).clicked() {
                    acts.push(Action::ToggleFullscreen);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if retro_btn(ui, "CLOSE", false).clicked() {
                        acts.push(Action::ToggleArt);
                    }
                });
            });
        });
        ptext(
            &p,
            Pos2::new(full.max.x - 30.0, full.max.y - 14.0),
            Align::Max,
            "Esc close    F fullscreen    G b&w    L lyrics    Space play/pause",
            1.0,
            DIM,
        );
    }

    fn playlist_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        let title = if self.shuffle { "QUEUE - SHUFFLED" } else { "QUEUE" };
        window_deco(ui, ui.max_rect(), title);
        let queue = self.queue.clone();
        let cur = self.cur;
        let full = ui.available_rect_before_wrap();
        let foot_h = 36.0;
        if full.height() < foot_h + 40.0 {
            return;
        }

        // ---- list well
        let well = Rect::from_min_max(full.min, Pos2::new(full.max.x, full.max.y - foot_h));
        inset(ui.painter(), well, LCD);
        let area = well.shrink(thick(3.0));

        enum QItem {
            Head(&'static str),
            Row(usize),
        }
        let mut items: Vec<QItem> = Vec::with_capacity(queue.len() + 3);
        for i in 0..queue.len() {
            match cur {
                Some(c) => {
                    if i == 0 && c > 0 {
                        items.push(QItem::Head("PLAYED"));
                    }
                    if i == c {
                        items.push(QItem::Head("NOW PLAYING"));
                    }
                    if i == c + 1 {
                        items.push(QItem::Head("UP NEXT"));
                    }
                }
                None => {
                    if i == 0 {
                        items.push(QItem::Head("UP NEXT"));
                    }
                }
            }
            items.push(QItem::Row(i));
        }
        let mut y_cur: Option<f32> = None;
        {
            let mut y = 0.0f32;
            for it in &items {
                match it {
                    QItem::Head(_) => y += HEAD_H,
                    QItem::Row(i) => {
                        if Some(*i) == cur {
                            y_cur = Some(y);
                        }
                        y += ROW_H;
                    }
                }
            }
        }

        ui.allocate_ui_at_rect(area, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
            if queue.is_empty() {
                ui.add_space(10.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
                ptext_fit(
                    ui.painter(),
                    Pos2::new(rect.min.x + 10.0, rect.center().y),
                    Align::Min,
                    "Queue is empty - pick something to play",
                    2.0,
                    rect.width() - 20.0,
                    INK2,
                );
                return;
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let top = ui.cursor().min;
                if self.q_scroll {
                    if let Some(yc) = y_cur {
                        let r = Rect::from_min_size(Pos2::new(top.x, top.y + yc), Vec2::new(10.0, ROW_H));
                        ui.scroll_to_rect(r, Some(Align::Center));
                    }
                    self.q_scroll = false;
                }
                for it in &items {
                    match it {
                        QItem::Head(t) => mini_header(ui, t),
                        QItem::Row(i) => {
                            let i = *i;
                            let t = &queue[i];
                            let state = match cur {
                                Some(c) if c == i => RowState::Playing,
                                Some(c) if i < c => RowState::Played,
                                _ => RowState::Normal,
                            };
                            let r = list_row(
                                ui,
                                i,
                                Some(i + 1),
                                || format!("{} - {}", t.artist, t.title),
                                || fmt_time(t.duration),
                                state,
                            );
                            if let Some(r) = r {
                                if r.clicked() {
                                    acts.push(Action::PlayIndex(i));
                                }
                                r.context_menu(|ui| {
                                    if menu_item(ui, "Play now") {
                                        acts.push(Action::PlayIndex(i));
                                        ui.close_menu();
                                    }
                                    if cur != Some(i) && menu_item(ui, "Remove from queue") {
                                        acts.push(Action::Remove(i));
                                        ui.close_menu();
                                    }
                                });
                            }
                        }
                    }
                }
            });
        });

        // ---- footer: clear + time/track counter
        let total: f32 = queue.iter().map(|t| t.duration).sum();
        let before: f32 = queue.iter().take(cur.unwrap_or(0)).map(|t| t.duration).sum();
        let elapsed = if self.cur.is_some() && !self.stopped { before + self.pos() } else { before };
        let foot = Rect::from_min_max(Pos2::new(full.min.x, full.max.y - foot_h + 8.0), full.max);
        ui.allocate_ui_at_rect(foot, |ui| {
            ui.horizontal(|ui| {
                if retro_btn(ui, "CLEAR", false).clicked() {
                    acts.push(Action::ClearQueue);
                }
                if retro_btn(ui, "ART", false).clicked() {
                    acts.push(Action::ToggleArt);
                }
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(340.0), BTN_H), Sense::hover());
                inset(ui.painter(), rect, LCD);
                let n = queue.len();
                ptext_fit(
                    ui.painter(),
                    rect.center(),
                    Align::Center,
                    &format!("{} / {}    {} {}", fmt_long(elapsed), fmt_long(total), n, if n == 1 { "track" } else { "tracks" }),
                    2.0,
                    rect.width() - 16.0,
                    INK,
                );
            });
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
        font::set_ppp(ctx.pixels_per_point());
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
            // Keyboard: Space = play/pause, arrows = seek, classic Winamp keys Z X C V B,
            // A = art viewer, G = black & white, L = lyrics, F / F11 = fullscreen, Esc = back.
            if !self.search_focus {
                let kp = |k: egui::Key| ctx.input(|i| i.key_pressed(k));
                if kp(egui::Key::Space) {
                    acts.push(Action::Toggle);
                }
                if kp(egui::Key::ArrowLeft) {
                    acts.push(Action::SeekRel(-5.0));
                }
                if kp(egui::Key::ArrowRight) {
                    acts.push(Action::SeekRel(5.0));
                }
                if kp(egui::Key::Z) {
                    acts.push(Action::Prev);
                }
                if kp(egui::Key::X) {
                    acts.push(Action::PlayBtn);
                }
                if kp(egui::Key::C) {
                    acts.push(Action::PauseBtn);
                }
                if kp(egui::Key::V) {
                    acts.push(Action::StopBtn);
                }
                if kp(egui::Key::B) {
                    acts.push(Action::Next);
                }
                if kp(egui::Key::A) {
                    acts.push(Action::ToggleArt);
                }
                if kp(egui::Key::G) {
                    acts.push(Action::ToggleGray);
                }
                if kp(egui::Key::L) {
                    acts.push(Action::ToggleLyrics);
                }
                if kp(egui::Key::F) && self.art_view {
                    acts.push(Action::ToggleFullscreen);
                }
                if kp(egui::Key::Escape) {
                    if self.fullscreen {
                        acts.push(Action::ToggleFullscreen);
                    } else if self.art_view {
                        acts.push(Action::ToggleArt);
                    }
                }
            }
            if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
                acts.push(Action::ToggleFullscreen);
            }

            // lyrics are fetched on demand, only while the viewer is open
            if self.art_view && self.show_lyrics {
                if let Some(t) = self.cur_track() {
                    if self.lyrics.as_ref().map(|l| l.id) != Some(t.id) {
                        self.fetch_lyrics(t.id);
                    }
                }
            }

            if self.art_view {
                egui::CentralPanel::default()
                    .frame(egui::Frame::none().fill(Color32::from_rgb(6, 6, 8)))
                    .show(ctx, |ui| self.art_ui(ui, &mut acts));
            } else {
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
        }

        for a in acts {
            self.apply(a);
        }
        let busy = self.cur.is_some() && !self.stopped && !self.paused;
        ctx.request_repaint_after(Duration::from_millis(if self.art_view { 16 } else if busy || self.search_focus { 33 } else { 250 }));
    }
}

fn setup_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::light();
    v.panel_fill = APP_BG;
    v.window_fill = BEIGE;
    v.window_stroke = Stroke::new(1.0, INK);
    v.window_rounding = Rounding::same(0.0);
    v.menu_rounding = Rounding::same(0.0);
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
    v.widgets.hovered.bg_fill = GROOVE;
    v.widgets.hovered.weak_bg_fill = GROOVE;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, INK);
    v.widgets.active.bg_fill = INK2;
    v.widgets.active.weak_bg_fill = INK2;
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
    s.spacing.scroll = egui::style::ScrollStyle::solid();
    s.spacing.scroll.bar_width = 12.0;
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
            .with_title("Tidalite")
            .with_icon(Arc::new(app_icon()))
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native("Tidalite", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
