#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod api;
mod band;
mod cache;
mod chart;
mod decode;
mod extras;
mod font;
mod icons;
mod media;
mod player;
mod skin;
mod sources;
mod stems;
mod store;
mod tools;
mod tuning;
mod update;
mod vicon;
mod views;
mod viz;

use api::{cover_url, Api, Card, Kind, Page, Track};
use eframe::egui::{self, Align, Color32, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
use font::{fit, ptext, ptext_fit, snap, spx, text_w, thick, wrap};
use icons::*;
use player::{Cmd, Player};
use skin::*;
use sources::Src;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use viz::viz_draw;

const EQ_PRESETS: [(&str, [f32; 10]); 4] = [
    ("FLAT", [0.0; 10]),
    ("BASS", [6.0, 5.0, 4.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    ("TREBLE", [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 5.0, 6.0]),
    ("ROCK", [4.0, 3.0, 2.0, 0.0, -1.0, -1.0, 1.0, 3.0, 4.0, 5.0]),
];

/// Shown in the player and the log so you can tell which build you are running.
const VERSION: &str = if PRACTICE { "v9 STUDIO" } else { "v9" };
/// Built with `--no-default-features` the practice tools (files, tunes, diary, loops, timer) are left out.
const PRACTICE: bool = cfg!(feature = "practice");

const LIB_W: f32 = 430.0;
#[allow(dead_code)]
const NB: usize = 19;
/// Points in the waveform visualizer.
const WAVE_N: usize = 128;
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
    Media(u8),
    LikeFailed(i64, bool, String),
    /// files / folders chosen or dropped: the audio found (and the folders to watch)
    Added(Vec<store::Ext>, Vec<String>),
    /// a rescan of the watched folders
    Scanned(Vec<store::Ext>),
    YtAdded(Result<store::Ext, String>),
    YtTool(Result<PathBuf, String>),
    Sc(Result<Vec<store::Ext>, String>),
    ScSets(Result<Vec<(String, String)>, String>),
    UpdateFound(Result<Option<update::Info>, String>),
    Tuning(i64, Option<(i32, f32)>),
    UpdateStaged(Result<(), String>),
    YtList(String, Result<Vec<store::Ext>, String>),
    ScMeta(store::Ext),
    Wave(i64, Option<Vec<u8>>),
    Exported(Result<String, String>),
    /// LOOK UP result: tune name, who wrote it, other versions found on Tidal
    Lookup(String, Result<String, String>, Vec<(Track, u32)>),
    Chart(String, Result<(String, String, String), (String, Vec<String>)>),
    Live(String, Result<(String, String, String), (String, Vec<String>)>),
    Beats(i64, Option<(f32, f32)>),
    /// stem separation finished for this track id
    Stems(i64, Result<(), String>),
    /// the stem tool (runtime + model) finished downloading
    StemTool(Result<(), String>),
}

/// Which list the library shows.
#[derive(PartialEq, Clone, Copy)]
enum Sec {
    Tidal,
    Files,
    Yt,
    Sc,
    Tunes,
    Diary,
    Lists,
}

/// Which text field has the keyboard (0 = none) and where its caret is.
#[derive(Default)]
struct Ed {
    id: u32,
    cur: usize,
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
    ToggleSpec,
    ToggleLyrics,
    ToggleFullscreen,
    ToggleLike(Track),
    MoveQueue(usize, usize),
    Skin,
    SkinSet(usize),
    Sleep,
    ToggleMini,
    TogglePin,
    ToggleEq,
    TogglePractice,
    SetAAt(f32),
    SetBAt(f32),
    NudgeA(f32),
    NudgeB(f32),
    LoopToggle,
    LoopClear,
    Speed(u32),
    ToggleCacheView,
    ClearCache,
    OpenCache,
    ToggleKeep,
    Logout,
    StartLogin,
    // ---- sources
    Section(Sec),
    AddFiles,
    AddFolder,
    Rescan,
    ForgetFolder(usize),
    AddYt,
    ScGo,
    ScSets,
    ScOpenSet(String),
    CopyLink(String),
    SaveList(u8, String),
    ApplyUpdate,
    FindTuning,
    PlaylistPick(Track),
    PlaylistAdd(usize, Track),
    PlaylistNew(Option<Track>),
    PlaylistFromQueue,
    PlaylistRemove(usize, usize),
    PlaylistMove(usize, usize, i32),
    PlaylistDelete(usize),
    PlaylistOpen(Option<usize>),
    RemoveList(usize),
    YtKeep(i64),
    LookStyle,
    ScBrowser,
    ScKeep(i64),
    Offline(bool),
    GetYtDlp,
    RemoveExt(i64),
    // ---- tunes
    PickTune(Track),
    AddToTune(usize, Track),
    NewTuneFrom(Track),
    NewTune,
    OpenTune(Option<usize>),
    RemoveTune(usize),
    Status(usize, u8),
    PlayTune(usize, bool),
    AddCurrentToTune(usize),
    LookUp(usize),
    // ---- practice
    SaveSection,
    GoSection(usize),
    DeleteSection(usize),
    ToggleFocus,
    ToggleMore,
    RemoveVersion(usize, usize),
    Pomo,
    TimerPanel,
    FindChart(usize, Option<String>),
    LiveChart(String, Option<String>),
    MetroPanel,
    PlayAlong(String),
    SaveLive,
    Opt(extras::Opt),
    SetKnob(extras::Knob, String),
    ToggleNumerals,
    Knob(extras::Knob, i32),
    StemGet,
    StemSplit,
    StemCancel,
    StemToggle(usize),
    StemSolo(usize),
    StemAll,
    StemClear,
    Continue,
    Trainer,
    TapTempo,
    CountIn,
    Chan,
    Transpose(i32),
    Export,
    // ---- diary / chart
    SaveEntry,
    RightTab(u8),
    ImportIreal(Option<usize>),
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

/// A button showing a pixel icon, `w` wide. `col` tints it; `on` draws it pressed.
fn icon_btn_w(ui: &mut egui::Ui, icon: &[&str], on: bool, col: Color32, w: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::click());
    let down = resp.is_pointer_button_down_on() || on;
    raised_h(ui.painter(), rect, down, resp.hovered());
    let px = 2.0;
    let (iw, ih) = (icon[0].len() as f32 * px, icon.len() as f32 * px);
    let dy = if down { 1.0 } else { 0.0 };
    let o = Pos2::new((rect.center().x - iw / 2.0).round(), (rect.center().y - ih / 2.0 + dy).round());
    pixmap(ui.painter(), o, px, icon, col);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Right-click list of every skin; the current one is marked.
fn skin_menu(ui: &mut egui::Ui, acts: &mut Vec<Action>) {
    for (n, name) in SKIN_NAMES.iter().enumerate() {
        let mark = if n == SKIN.load(Ordering::Relaxed) % PALS.len() { "> " } else { "  " };
        if menu_item(ui, &format!("{}{}", mark, name)) {
            acts.push(Action::SkinSet(n));
            ui.close_menu();
        }
    }
}

fn icon_btn(ui: &mut egui::Ui, icon: &[&str], on: bool, col: Color32) -> egui::Response {
    icon_btn_w(ui, icon, on, col, 44.0)
}

/// Icon on the left, a short label on the right.
fn ibtn(ui: &mut egui::Ui, icon: &[&str], text: &str, on: bool) -> egui::Response {
    let w = 10.0 + icon[0].len() as f32 * 2.0 + 8.0 + text_w(text, 2.0) + 10.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::click());
    let down = resp.is_pointer_button_down_on() || on;
    raised_h(ui.painter(), rect, down, resp.hovered());
    let dy = if down { 1.0 } else { 0.0 };
    let o = Pos2::new((rect.min.x + 10.0).round(), (rect.center().y - icon.len() as f32 + dy).round());
    pixmap(ui.painter(), o, 2.0, icon, pal().ink);
    ptext(
        ui.painter(),
        Pos2::new(o.x + icon[0].len() as f32 * 2.0 + 8.0, rect.center().y + dy),
        Align::Min,
        text,
        2.0,
        pal().ink,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Fit a pixel icon into `r` as large as whole dots allow.
fn icon_in(p: &egui::Painter, r: Rect, icon: &[&str], col: Color32) {
    let k = ((r.height() - 3.0) / icon.len() as f32).floor().max(1.0);
    let (w, h) = (icon[0].len() as f32 * k, icon.len() as f32 * k);
    pixmap(p, Pos2::new((r.center().x - w / 2.0).round(), (r.center().y - h / 2.0).round()), k, icon, col);
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
    match style() {
        1 => {
            rfill(p, r, 4.0, fill);
            rline(p, r, 4.0, 1.0, pal().beige_dk);
            // soft shade along the top edge, like glass set into the frame
            rfill(
                p,
                Rect::from_min_size(r.min + Vec2::new(2.0, 1.0), Vec2::new(r.width() - 4.0, 2.0)),
                1.0,
                Color32::from_black_alpha(26),
            );
            return;
        }
        2 => {
            rfill(p, r, 6.0, fill);
            rline(p, r, 6.0, 1.0, pal().groove);
            return;
        }
        _ => {}
    }
    let t1 = thick(1.5);
    let t2 = thick(1.0);
    fill_rect(p, r, fill);
    fill_rect(p, Rect::from_min_size(r.min, Vec2::new(r.width(), t1)), pal().edge);
    fill_rect(p, Rect::from_min_size(r.min, Vec2::new(t1, r.height())), pal().edge);
    fill_rect(p, Rect::from_min_size(Pos2::new(r.min.x, r.max.y - t2), Vec2::new(r.width(), t2)), pal().beige_lt);
    fill_rect(p, Rect::from_min_size(Pos2::new(r.max.x - t2, r.min.y), Vec2::new(t2, r.height())), pal().beige_lt);
}

/// Raised bevelled button face.
fn raised_h(p: &egui::Painter, r: Rect, down: bool, hover: bool) {
    match style() {
        1 => {
            // glossy glass button: gradient body, bright sheen on the top half, thin blue outline
            let (top, bot) = if down {
                (Color32::from_rgb(150, 196, 232), Color32::from_rgb(196, 226, 248))
            } else if hover {
                (Color32::from_rgb(255, 255, 255), Color32::from_rgb(190, 228, 250))
            } else {
                (Color32::from_rgb(250, 253, 255), Color32::from_rgb(196, 222, 244))
            };
            rgrad(p, r, 6.0, top, bot);
            if !down {
                let sheen = Rect::from_min_size(r.min + Vec2::new(2.0, 1.5), Vec2::new(r.width() - 4.0, r.height() * 0.44));
                rfill(p, sheen, 4.0, Color32::from_white_alpha(if hover { 120 } else { 90 }));
            }
            rline(p, r, 6.0, 1.0, if hover { pal().btn_face } else { pal().edge });
            return;
        }
        2 => {
            let face = if down {
                pal().sel
            } else if hover {
                pal().beige_lt
            } else {
                pal().beige_h
            };
            rfill(p, r, 6.0, face);
            rline(p, r, 6.0, 1.0, if hover || down { pal().btn_face } else { pal().groove });
            return;
        }
        _ => {}
    }
    let (hi, lo) = if down { (pal().beige_dk, pal().beige_lt) } else { (pal().beige_lt, pal().beige_dk) };
    let face = if down {
        pal().sel
    } else if hover {
        pal().beige_h
    } else {
        pal().beige
    };
    let t = thick(1.0);
    fill_rect(p, r, face);
    let i = r.shrink(t);
    fill_rect(p, Rect::from_min_size(i.min, Vec2::new(i.width(), t)), hi);
    fill_rect(p, Rect::from_min_size(i.min, Vec2::new(t, i.height())), hi);
    fill_rect(p, Rect::from_min_size(Pos2::new(i.min.x, i.max.y - t), Vec2::new(i.width(), t)), lo);
    fill_rect(p, Rect::from_min_size(Pos2::new(i.max.x - t, i.min.y), Vec2::new(t, i.height())), lo);
    outline(p, r, t, pal().edge);
}

fn raised(p: &egui::Painter, r: Rect, down: bool) {
    raised_h(p, r, down, false);
}

/// Rectangle with its four corners cut off by `n` (a pixel-art "rounded" box).
fn notch_fill(p: &egui::Painter, r: Rect, n: f32, c: Color32) {
    fill_rect(p, Rect::from_min_max(Pos2::new(r.min.x + n, r.min.y), Pos2::new(r.max.x - n, r.max.y)), c);
    fill_rect(p, Rect::from_min_max(Pos2::new(r.min.x, r.min.y + n), Pos2::new(r.max.x, r.max.y - n)), c);
}

/// Black window frame with beige trim, title tab and beige body at `inner`; square, with stepped corners.
fn window_deco(ui: &egui::Ui, inner: Rect, title: &str) {
    let p = ui.painter();
    let u = thick(2.0);
    let outer = Rect::from_min_max(inner.min - Vec2::new(14.0, 30.0), inner.max + Vec2::new(14.0, 14.0));
    if style() != 0 {
        modern_deco(p, outer, inner, title);
        return;
    }
    notch_fill(p, outer, u * 3.0, Color32::BLACK);
    notch_fill(p, outer.shrink(u * 1.5), u * 2.0, pal().trim);
    notch_fill(p, outer.shrink(u * 2.5), u * 2.0, Color32::BLACK);
    notch_fill(p, outer.shrink(u * 3.5), u, pal().groove);
    notch_fill(p, outer.shrink(u * 4.0), u, Color32::BLACK);
    // title tab (with a little pixel icon for the section)
    let label = title.to_string();
    let cy = outer.min.y + 12.0;
    let icon: Option<&[&str]> = match title.split(' ').next().unwrap_or("") {
        "LIBRARY" => Some(&WIN_LIBRARY),
        "QUEUE" => Some(&WIN_QUEUE),
        "LEAD" => Some(&WIN_SHEET),
        "PRACTICE" => Some(&WIN_PRACTICE),
        "PLAYER" => Some(&MARK),
        _ => None,
    };
    let iw = if icon.is_some() { 26.0 } else { 0.0 };
    let tw = (text_w(&label, 2.0) + 28.0 + iw).min(outer.width() - 80.0).max(60.0);
    // left-aligned tab with chamfered top corners, sitting on the frame edge
    let tab = Rect::from_min_size(Pos2::new(outer.min.x + 22.0, cy - 11.0), Vec2::new(tw, 22.0));
    notch_fill(p, tab.expand(3.0), u * 2.0, Color32::BLACK);
    notch_fill(p, tab, u * 1.5, pal().edge);
    ptext_fit(p, tab.center() + Vec2::new(iw / 2.0, 0.0), Align::Center, &label, 2.0, tw - 12.0 - iw, pal().trim);
    if let Some(rows) = icon {
        let o = Pos2::new((tab.min.x + 9.0).round(), (cy - rows.len() as f32).round());
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                // 'o' pixels get a mid tone between tab text and tab fill so they never vanish on any skin
                let col = match ch {
                    '#' => pal().trim,
                    'o' => pal().trim.lerp_to_gamma(pal().edge, 0.4),
                    _ => continue,
                };
                fill_rect(p, Rect::from_min_size(o + Vec2::new(x as f32 * 2.0, y as f32 * 2.0), Vec2::splat(2.0)), col);
            }
        }
    }
    // body
    fill_rect(p, inner.expand(2.0), pal().edge);
    fill_rect(p, inner, pal().beige);
}

/// Window frame for the aero and sleek skins.
fn modern_deco(p: &egui::Painter, outer: Rect, inner: Rect, title: &str) {
    let icon: Option<&[&str]> = match title.split(' ').next().unwrap_or("") {
        "LIBRARY" => Some(&WIN_LIBRARY),
        "QUEUE" => Some(&WIN_QUEUE),
        "LEAD" => Some(&WIN_SHEET),
        "PRACTICE" => Some(&WIN_PRACTICE),
        "PLAYER" => Some(&MARK),
        _ => None,
    };
    let cy = outer.min.y + 14.0;
    let (txt_col, icon_col) = if style() == 1 { (pal().bar_txt, pal().bar_txt) } else { (pal().ink2, pal().ink2) };
    if style() == 1 {
        // glass title bar over a blue frame, like an XP / Vista window
        rfill(p, outer.translate(Vec2::new(0.0, 2.0)), 9.0, Color32::from_black_alpha(60));
        rgrad(p, outer, 9.0, Color32::from_rgb(92, 178, 236), Color32::from_rgb(28, 108, 188));
        let bar = Rect::from_min_size(outer.min + Vec2::new(2.0, 2.0), Vec2::new(outer.width() - 4.0, 24.0));
        rfill(p, bar, 7.0, Color32::from_white_alpha(46));
        rline(p, outer, 9.0, 1.0, pal().edge);
        rfill(p, inner.expand(2.0), 3.0, pal().edge);
        rfill(p, inner, 2.0, pal().beige);
        // a sliver of glare across the top of the body
        rfill(
            p,
            Rect::from_min_size(inner.min + Vec2::new(1.0, 1.0), Vec2::new(inner.width() - 2.0, 3.0)),
            1.0,
            Color32::from_white_alpha(70),
        );
    } else {
        rfill(p, outer, 10.0, pal().beige_dk);
        rline(p, outer, 10.0, 1.0, pal().groove);
        rfill(p, inner, 6.0, pal().beige);
        rline(p, inner, 6.0, 1.0, pal().groove);
    }
    let mut x = outer.min.x + 14.0;
    if let Some(rows) = icon {
        let k = 2.0;
        let ir = Rect::from_center_size(Pos2::new(x + 10.0, cy), Vec2::splat(rows.len() as f32 * k));
        if !vicon::draw(p, ir, rows, icon_col) {
            pixmap(p, ir.min, k, rows, icon_col);
        }
        x += 26.0;
    }
    ptext(p, Pos2::new(x, cy), Align::Min, title, 2.0, txt_col);
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
        fill_rect(p, r, pal().lcd_ghost);
        if lit && m[i] == 1 {
            fill_rect(p, r, pal().ink);
        }
    }
}

/// Filled disc built from square pixels of size `u` (pixel-art circle).
fn pix_disc(p: &egui::Painter, c: Pos2, r: f32, u: f32, col: Color32) {
    let n = (r / u).round() as i32;
    for k in -n..n {
        let y = (k as f32 + 0.5) * u;
        let hw = ((r * r - y * y).max(0.0)).sqrt();
        let hw = (hw / u).round() * u;
        if hw > 0.0 {
            fill_rect(
                p,
                Rect::from_min_max(Pos2::new(c.x - hw, c.y + k as f32 * u), Pos2::new(c.x + hw, c.y + (k + 1) as f32 * u)),
                col,
            );
        }
    }
}

/// Round transport button drawn in big pixels, with a pixel icon and a pixel shine.
fn round_btn(ui: &egui::Ui, center: Pos2, radius: f32, icon: &[&str], px: f32, id: &str) -> egui::Response {
    let rect = Rect::from_center_size(center, Vec2::splat(radius * 2.0));
    let resp = ui.interact(rect, ui.id().with(id), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    let down = resp.is_pointer_button_down_on();
    let u = thick((radius / 9.0).round().max(2.0));
    let off = if down { u } else { 0.0 };
    let p = ui.painter();
    let face = if resp.hovered() { pal().btn_hi } else { pal().btn_face };
    pix_disc(p, center, radius, u, Color32::BLACK);
    pix_disc(p, center + Vec2::new(0.0, off), radius - u, u, face);
    if !down {
        // lower edge shade, then the shine
        pix_disc(p, center + Vec2::new(0.0, u * 0.5), radius - u * 2.0, u, face);
        pix_disc(
            p,
            center + Vec2::new(-radius * 0.3, -radius * 0.34),
            radius * 0.3,
            u,
            Color32::from_rgba_unmultiplied(255, 255, 225, 70),
        );
    }
    let px = px.floor().max(1.0);
    let o = Pos2::new(snap(center.x - 3.5 * px), snap(center.y - 3.5 * px + off));
    pixmap(p, o, px, icon, Color32::from_rgb(34, 32, 16));
    resp
}

/// A little pixel gem with a tide line through it.
fn logo_mark(p: &egui::Painter, origin: Pos2, px: f32) {
    if style() != 0 {
        vicon::gem(p, Rect::from_min_size(origin, Vec2::splat(px * 9.0)), pal().ink, pal().ink2);
        return;
    }
    for (y, row) in MARK.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            let col = match ch {
                '#' => pal().ink,
                'o' => pal().ink2,
                _ => continue,
            };
            fill_rect(p, Rect::from_min_size(origin + Vec2::new(x as f32 * px, y as f32 * px), Vec2::splat(px)), col);
        }
    }
}

/// Logo strip at the top of the library window.
fn logo(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.0), Sense::hover());
    let p = ui.painter();
    inset(p, rect, pal().lcd);
    logo_mark(p, Pos2::new(rect.min.x + 12.0, rect.center().y - 13.5), 3.0);
    let (tsz, ty, gy) = if style() == 0 { (3.0, -8.0, 13.0) } else { (2.6, -9.0, 14.0) };
    ptext(p, Pos2::new(rect.min.x + 50.0, rect.center().y + ty), Align::Min, "TIDALITE", tsz, pal().ink);
    let tag =
        if PRACTICE { "A RETRO PLAYER FOR TIDAL AND MORE - MADE FOR PRACTICE" } else { "A RETRO PLAYER FOR TIDAL AND MORE" };
    ptext_fit(p, Pos2::new(rect.min.x + 51.0, rect.center().y + gy), Align::Min, tag, 1.0, rect.width() - 110.0, pal().ink2);
    ptext(
        p,
        Pos2::new(rect.max.x - 10.0, rect.center().y + gy),
        Align::Max,
        &if update::build() > 0 { format!("{} .{}", VERSION, update::build()) } else { VERSION.to_string() },
        1.0,
        pal().dim,
    );
}

/// Folder-style tabs with a baseline; the open tab joins the panel below. Returns the clicked tab.
fn tab_row(ui: &mut egui::Ui, items: &[&str], cur: usize) -> Option<usize> {
    let h = 26.0;
    let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h + 6.0), Sense::hover());
    let t = thick(2.0);
    let base = row.max.y - 4.0;
    fill_rect(ui.painter(), Rect::from_min_max(Pos2::new(row.min.x, base), Pos2::new(row.max.x, base + t)), pal().edge);
    let (mut x, mut hit) = (row.min.x + 4.0, None);
    for (i, name) in items.iter().enumerate() {
        let w = text_w(name, 2.0) + 24.0;
        let on = i == cur;
        let r = Rect::from_min_max(
            Pos2::new(x, if on { row.min.y } else { row.min.y + 5.0 }),
            Pos2::new(x + w, base + if on { t } else { 0.0 }),
        );
        let resp = ui
            .interact(r, ui.id().with(("tab", items.join("|"), i)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let p = ui.painter();
        if style() != 0 {
            let r = Rect::from_min_max(r.min + Vec2::new(0.0, if on { 0.0 } else { 1.0 }), Pos2::new(r.max.x, r.max.y - 2.0));
            if style() == 1 {
                if on || resp.hovered() {
                    raised_h(p, r, false, on);
                } else {
                    rfill(p, r, 6.0, pal().beige_dk);
                }
            } else if on {
                rfill(p, r, 7.0, pal().btn_face);
            } else if resp.hovered() {
                rfill(p, r, 7.0, pal().beige_h);
            }
            let col = if on && style() == 2 {
                pal().bar_txt
            } else if on {
                pal().ink
            } else {
                pal().ink2
            };
            ptext(p, r.center(), Align::Center, name, 2.0, col);
            if resp.clicked() {
                hit = Some(i);
            }
            x += w + 3.0;
            continue;
        }
        fill_rect(
            p,
            r,
            if on {
                pal().beige_lt
            } else if resp.hovered() {
                pal().beige_h
            } else {
                pal().beige_dk
            },
        );
        fill_rect(p, Rect::from_min_size(r.min, Vec2::new(r.width(), t)), pal().edge);
        fill_rect(p, Rect::from_min_size(r.min, Vec2::new(t, r.height())), pal().edge);
        fill_rect(p, Rect::from_min_size(Pos2::new(r.max.x - t, r.min.y), Vec2::new(t, r.height())), pal().edge);
        // chipped top corners
        fill_rect(p, Rect::from_min_size(r.min, Vec2::splat(t)), pal().beige);
        fill_rect(p, Rect::from_min_size(Pos2::new(r.max.x - t, r.min.y), Vec2::splat(t)), pal().beige);
        ptext(
            p,
            Pos2::new(r.center().x, r.min.y + (base - r.min.y) / 2.0 + 1.0),
            Align::Center,
            name,
            2.0,
            if on { pal().ink } else { pal().ink2 },
        );
        if resp.clicked() {
            hit = Some(i);
        }
        x += w + 3.0;
    }
    hit
}

/// Small pixel check box with a label. Returns the click.
fn check_box(ui: &mut egui::Ui, text: &str, on: bool) -> egui::Response {
    let w = text_w(text, 2.0) + 30.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::click());
    let b = Rect::from_center_size(Pos2::new(rect.min.x + 11.0, rect.center().y), Vec2::splat(16.0));
    inset(ui.painter(), b, pal().lcd);
    if on {
        pixmap(ui.painter(), Pos2::new(b.min.x + 3.0, b.min.y + 4.0), 2.0, &CHECK[..], pal().ink);
    }
    ptext(
        ui.painter(),
        Pos2::new(rect.min.x + 26.0, rect.center().y),
        Align::Min,
        text,
        2.0,
        if resp.hovered() { pal().ink } else { pal().ink2 },
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn retro_btn_w(ui: &mut egui::Ui, text: &str, w: f32, active: bool) -> egui::Response {
    let w = w.max(text_w(text, 2.0) + 16.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::click());
    let down = resp.is_pointer_button_down_on() || active;
    raised_h(ui.painter(), rect, down, resp.hovered());
    let dy = if down { 1.0 } else { 0.0 };
    ptext(ui.painter(), rect.center() + Vec2::new(0.0, dy), Align::Center, text, 2.0, pal().ink);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Button sized to its label.
fn retro_btn(ui: &mut egui::Ui, text: &str, active: bool) -> egui::Response {
    let w = (text_w(text, 2.0) + 20.0).max(44.0);
    retro_btn_w(ui, text, w, active)
}

fn section_header(ui: &mut egui::Ui, text: &str) {
    ui.add_space(6.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
    fill_rect(ui.painter(), rect, pal().edge);
    ptext(ui.painter(), Pos2::new(rect.min.x + 8.0, rect.center().y), Align::Min, text, 2.0, pal().trim);
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
    fill_rect(ui.painter(), rect, pal().groove);
    ptext(ui.painter(), Pos2::new(rect.min.x + 8.0, rect.center().y), Align::Min, text, 1.0, pal().bar_txt);
}

/// One list row. Text is only built (and drawn) when the row is on screen.
fn list_row(
    ui: &mut egui::Ui,
    idx: usize,
    num: Option<usize>,
    left: impl FnOnce() -> String,
    right: impl FnOnce() -> String,
    state: RowState,
    drag: bool,
    stored: bool,
) -> Option<egui::Response> {
    let w = ui.available_width();
    let probe = Rect::from_min_size(ui.cursor().min, Vec2::new(w, ROW_H));
    if !ui.is_rect_visible(probe) {
        ui.allocate_space(Vec2::new(w, ROW_H));
        return None;
    }
    let sense = if drag { Sense::click_and_drag() } else { Sense::click() };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, ROW_H), sense);
    let p = ui.painter();
    let bg = if state == RowState::Playing {
        Some(pal().row_sel)
    } else if resp.hovered() {
        Some(pal().sel)
    } else if idx % 2 == 1 {
        Some(pal().row_alt)
    } else {
        None
    };
    if let Some(c) = bg {
        fill_rect(p, rect, c);
    }
    let (fg, fg2) = match state {
        RowState::Playing => (pal().bar_txt, pal().bar_txt),
        RowState::Played => (pal().dim, pal().dim),
        RowState::Normal => (pal().ink, pal().ink2),
    };
    let cy = rect.center().y;
    let rtxt = right();
    let mut rw = if let Some((a, b)) = rtxt.split_once('\t') {
        // two right-hand columns, lined up with `table_header`
        ptext(p, Pos2::new(rect.max.x - 10.0, cy), Align::Max, b, 2.0, fg2);
        let w = ptext(p, Pos2::new(rect.max.x - 10.0 - TBL_W2, cy), Align::Max, a, 2.0, fg2);
        TBL_W2 + w
    } else {
        ptext(p, Pos2::new(rect.max.x - 10.0, cy), Align::Max, &rtxt, 2.0, fg2)
    };
    if stored {
        pixmap(p, Pos2::new(rect.max.x - 10.0 - rw - 16.0, cy - 4.0), 2.0, &CHECK[..], fg2);
        rw += 16.0;
    }
    let x0 = rect.min.x + 8.0;
    let mut tx = x0;
    if let Some(n) = num {
        if state == RowState::Playing {
            pixmap(p, Pos2::new(x0 + 2.0, cy - 5.0), 2.0, &PLAY_S[..], pal().bar_txt);
        } else {
            ptext_fit(p, Pos2::new(x0 + 34.0, cy), Align::Max, &n.to_string(), 2.0, 34.0, fg2);
        }
        tx = x0 + 44.0;
    }
    let lraw = left();
    if lraw.contains('\t') {
        // cell layout shared with the column header
        let avail = (rect.max.x - 10.0 - 80.0 - tx).max(40.0);
        let cw = col_widths(avail);
        let cells: Vec<&str> = lraw.split('\t').collect();
        let mut x = tx;
        for (i, c) in cells.iter().enumerate().take(3) {
            if cw[i] > 0.0 {
                let t = fit(c, 2.0, (cw[i] - 12.0).max(20.0));
                ptext(p, Pos2::new(x, cy), Align::Min, &t, 2.0, if i == 0 { fg } else { fg2 });
                x += cw[i];
            }
        }
    } else {
        let avail = (rect.max.x - 10.0 - rw - 16.0) - tx;
        let ltxt = fit(&lraw, 2.0, avail.max(20.0));
        ptext(p, Pos2::new(tx, cy), Align::Min, &ltxt, 2.0, fg);
    }
    Some(resp.on_hover_cursor(egui::CursorIcon::PointingHand))
}

/// Retro-styled entry for right-click menus. Returns true when clicked.
fn menu_item(ui: &mut egui::Ui, text: &str) -> bool {
    let w = (text_w(text, 2.0) + 28.0).max(150.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::click());
    if resp.hovered() {
        fill_rect(ui.painter(), rect, pal().sel);
    }
    ptext(ui.painter(), Pos2::new(rect.min.x + 10.0, rect.center().y), Align::Min, text, 2.0, pal().ink);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// Dropdown button: shows `name: current` with a small arrow; the list opens as an overlay
/// right under it (no layout change). Returns the index chosen this frame.
fn dropdown(ui: &mut egui::Ui, id: &str, title: &str, items: &[&str], sel: usize, min_w: f32) -> Option<usize> {
    let cur = items.get(sel).copied().unwrap_or("");
    let text = if title.is_empty() { cur.to_string() } else { format!("{}: {}", title, cur) };
    let w = (text_w(&text, 2.0) + 38.0).max(min_w);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::click());
    let key = egui::Id::new(("dd", id));
    let open = ui.ctx().data(|d| d.get_temp::<bool>(key)).unwrap_or(false);
    let down = resp.is_pointer_button_down_on() || open;
    raised_h(ui.painter(), rect, down, resp.hovered());
    let dy = if down { 1.0 } else { 0.0 };
    ptext(ui.painter(), Pos2::new(rect.min.x + 10.0, rect.center().y + dy), Align::Min, &text, 2.0, pal().ink);
    pixmap(
        ui.painter(),
        Pos2::new(rect.max.x - 20.0, rect.center().y - 3.0 + dy),
        2.0,
        &["#######", ".#####.", "..###..", "...#..."],
        pal().ink,
    );
    if resp.clicked() {
        ui.ctx().data_mut(|d| d.insert_temp(key, !open));
    }
    let mut pick = None;
    if open {
        let area = egui::Area::new(key.with("area"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(rect.min.x, rect.max.y + 1.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::none().fill(pal().beige_lt).stroke(egui::Stroke::new(2.0_f32, pal().edge)).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    for (i, it) in items.iter().enumerate() {
                        let iw = (w - 4.0).max(text_w(it, 2.0) + 28.0);
                        let (r, rr) = ui.allocate_exact_size(Vec2::new(iw, 24.0), Sense::click());
                        if i == sel || rr.hovered() {
                            fill_rect(ui.painter(), r, pal().sel);
                        }
                        ptext(ui.painter(), Pos2::new(r.min.x + 10.0, r.center().y), Align::Min, it, 2.0, pal().ink);
                        if rr.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            pick = Some(i);
                        }
                    }
                });
            });
        let outside = ui.ctx().input(|i| i.pointer.any_click()) && !resp.hovered() && !area.response.hovered();
        if pick.is_some() || outside || ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            ui.ctx().data_mut(|d| d.insert_temp(key, false));
        }
    }
    pick
}

/// Pixel-font hover hint (egui's own tooltip would use its default font).
trait Tip {
    fn tip(self, text: impl AsRef<str>) -> Self;
}

impl Tip for egui::Response {
    fn tip(self, text: impl AsRef<str>) -> Self {
        let on = self.hovered();
        if self.ctx.animate_bool_with_time(self.id.with("tip"), on, 0.45) < 1.0 || !on {
            return self;
        }
        let Some(m) = self.ctx.input(|i| i.pointer.hover_pos()) else { return self };
        let px = spx(1.0);
        let lines = wrap(text.as_ref(), px, 300.0);
        let h = lines.len() as f32 * (px * 9.0 + 5.0) + 12.0;
        let w = lines.iter().map(|l| text_w(l, px)).fold(0.0, f32::max) + 16.0;
        let screen = self.ctx.screen_rect();
        let x = (m.x + 12.0).min(screen.max.x - w - 4.0).max(4.0);
        let y = if m.y + 24.0 + h > screen.max.y { m.y - h - 8.0 } else { m.y + 20.0 };
        let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
        let p = self.ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("tip")));
        fill_rect(&p, r, pal().lcd);
        outline(&p, r, 1.0, pal().ink);
        for (i, l) in lines.iter().enumerate() {
            ptext(
                &p,
                Pos2::new(r.min.x + 8.0, r.min.y + 6.0 + px * 4.5 + i as f32 * (px * 9.0 + 5.0)),
                Align::Min,
                l,
                px,
                pal().ink,
            );
        }
        self
    }
}

/// Small raised chip button (used on window title lines).
fn chip(ui: &egui::Ui, rect: Rect, text: &str, active: bool, id: &str) -> egui::Response {
    let r = ui.interact(rect, ui.id().with(id), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    raised_h(ui.painter(), rect, active || r.is_pointer_button_down_on(), r.hovered() && !active);
    ptext_fit(ui.painter(), rect.center(), Align::Center, text, 1.0, rect.width() - 6.0, pal().ink);
    r
}

/// Sunken LCD readout box.
fn lcd_box(ui: &mut egui::Ui, text: &str, w: f32, col: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::hover());
    inset(ui.painter(), rect, pal().lcd);
    ptext_fit(ui.painter(), rect.center(), Align::Center, text, 2.0, w - 12.0, col);
}

/// Wrapped paragraph in the list well.
fn para(ui: &mut egui::Ui, text: &str, col: Color32) {
    let w = ui.available_width() - 20.0;
    for line in wrap(text, 2.0, w) {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
        ptext(ui.painter(), Pos2::new(rect.min.x + 8.0, rect.center().y), Align::Min, &line, 2.0, col);
    }
    ui.add_space(6.0);
}

/// Break a long unbroken string (a file path) into lines that fit.
fn chunk(text: &str, px: f32, w: f32) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        let mut t = cur.clone();
        t.push(ch);
        if text_w(&t, px) > w && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        cur.push(ch);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Right-click menu for loop points (shared by the seek bar and the practice timeline).
fn loop_menu(ui: &mut egui::Ui, acts: &mut Vec<Action>, t: f32, both: bool, loop_on: bool, practice: bool) {
    if !PRACTICE {
        return;
    }
    if menu_item(ui, &format!("Loop start (A) here  {}", fmt_t(t))) {
        acts.push(Action::SetAAt(t));
        ui.close_menu();
    }
    if menu_item(ui, &format!("Loop end (B) here  {}", fmt_t(t))) {
        acts.push(Action::SetBAt(t));
        ui.close_menu();
    }
    if both {
        if menu_item(ui, if loop_on { "Loop off" } else { "Loop on" }) {
            acts.push(Action::LoopToggle);
            ui.close_menu();
        }
        if menu_item(ui, "Clear loop") {
            acts.push(Action::LoopClear);
            ui.close_menu();
        }
    }
    if menu_item(ui, if practice { "Back to listening mode" } else { "Practice mode" }) {
        acts.push(Action::TogglePractice);
        ui.close_menu();
    }
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

/// Time with tenths, for loop points: 1:23.4
fn fmt_t(s: f32) -> String {
    let s = s.max(0.0);
    let m = (s / 60.0) as u32;
    format!("{}:{:04.1}", m, s - m as f32 * 60.0)
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
    /// average colour of the top and bottom half of each cover, for the art-view gradient
    avg: HashMap<String, [Color32; 2]>,
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
    let round = round.min(0.0);
    ui.painter().rect_filled(rect, Rounding::same(round), pal().ink2);
    if let Some(id) = art_tex(images, url, gray) {
        egui::Image::new(egui::load::SizedTexture::new(id, rect.size())).rounding(Rounding::same(round)).paint_at(ui, rect);
    }
}

/// Width of the last right-hand column in two-column rows.
const TBL_W2: f32 = 140.0;

/// Header bar for a list: a title on the left and up to two right-aligned column titles.
fn table_header(ui: &mut egui::Ui, left: &str, mid: &str, last: &str, numbered: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
    fill_rect(ui.painter(), rect, pal().edge);
    let cy = rect.center().y;
    let p = ui.painter();
    if numbered {
        ptext(p, Pos2::new(rect.min.x + 8.0, cy), Align::Min, "#", 2.0, pal().trim);
    }
    ptext(p, Pos2::new(rect.min.x + 8.0 + if numbered { 44.0 } else { 0.0 }, cy), Align::Min, left, 2.0, pal().trim);
    ptext(p, Pos2::new(rect.max.x - 10.0 - TBL_W2, cy), Align::Max, mid, 2.0, pal().trim);
    ptext(p, Pos2::new(rect.max.x - 10.0, cy), Align::Max, last, 2.0, pal().trim);
}

// ------------------------------------------------------------------ list columns
/// Which extra columns the track lists show: bit0 artist, bit1 album, bit2 length (name is always shown).
static COLS: AtomicU32 = AtomicU32::new(0b101);
const COL_NAMES: [&str; 3] = ["ARTIST", "ALBUM", "LENGTH"];

fn col_on(i: usize) -> bool {
    COLS.load(Ordering::Relaxed) >> i & 1 == 1
}

/// Widths of the NAME, ARTIST and ALBUM cells inside `avail` (0 for hidden ones).
fn col_widths(avail: f32) -> [f32; 3] {
    let (a, b) = (col_on(0), col_on(1));
    match (a, b) {
        (false, false) => [avail, 0.0, 0.0],
        (true, false) => [avail * 0.58, avail * 0.42, 0.0],
        (false, true) => [avail * 0.58, 0.0, avail * 0.42],
        (true, true) => [avail * 0.4, avail * 0.3, avail * 0.3],
    }
}

/// Header row over a track list; right-click it to choose the columns.
fn col_header(ui: &mut egui::Ui, numbered: bool) {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 20.0), Sense::click());
    fill_rect(ui.painter(), rect, pal().edge);
    let cy = rect.center().y;
    let tx = rect.min.x + 8.0 + if numbered { 44.0 } else { 0.0 };
    let avail = (rect.max.x - 10.0 - 80.0 - tx).max(40.0);
    let cw = col_widths(avail);
    let p = ui.painter();
    ptext(p, Pos2::new(rect.min.x + 8.0, cy), Align::Min, if numbered { "#" } else { "" }, 2.0, pal().trim);
    ptext(p, Pos2::new(tx, cy), Align::Min, "NAME", 2.0, pal().trim);
    let mut x = tx + cw[0];
    for (i, name) in COL_NAMES[..2].iter().enumerate() {
        if cw[i + 1] > 0.0 {
            ptext(p, Pos2::new(x, cy), Align::Min, name, 2.0, pal().trim);
            x += cw[i + 1];
        }
    }
    if col_on(2) {
        ptext(p, Pos2::new(rect.max.x - 10.0, cy), Align::Max, "LENGTH", 2.0, pal().trim);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).tip("Right-click to choose columns").context_menu(|ui| {
        for (i, name) in COL_NAMES.iter().enumerate() {
            let mark = if col_on(i) { "[x] " } else { "[ ] " };
            if menu_item(ui, &format!("{}{}", mark, name)) {
                COLS.fetch_xor(1 << i, Ordering::Relaxed);
            }
        }
    });
}

/// One track row's text, split into cells with tabs for `list_row`.
fn track_cells(title: &str, artist: &str, album: &str) -> String {
    format!("{}\t{}\t{}", title, artist, album)
}

// ------------------------------------------------------------------ page view
fn tracks_list(ui: &mut egui::Ui, tracks: &[Track], playing_id: Option<i64>, liked: &HashSet<i64>, acts: &mut Vec<Action>) {
    col_header(ui, true);
    for (i, t) in tracks.iter().enumerate() {
        let state = if playing_id == Some(t.id) { RowState::Playing } else { RowState::Normal };
        let r = list_row(
            ui,
            i,
            Some(i + 1),
            || track_cells(&t.title, &t.artist, &t.album),
            || if col_on(2) { fmt_time(t.duration) } else { String::new() },
            state,
            false,
            cache::has(t.id),
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
                {
                    let lk = if liked.contains(&t.id) { "Remove from My Tracks" } else { "Add to My Tracks" };
                    if menu_item(ui, lk) {
                        acts.push(Action::ToggleLike(t.clone()));
                        ui.close_menu();
                    }
                }
                if PRACTICE && menu_item(ui, "Add to a tune...") {
                    acts.push(Action::PickTune(t.clone()));
                    ui.close_menu();
                }
                if menu_item(ui, "Add to a playlist...") {
                    acts.push(Action::PlaylistPick(t.clone()));
                    ui.close_menu();
                }
                link_item(ui, acts, track_link(t, None));
            });
        }
    }
}

/// The public link for a track: Tidal by id, YouTube / SoundCloud from the saved source. None for files.
fn track_link(t: &Track, ext: Option<&store::Ext>) -> Option<String> {
    if t.id > 0 {
        return Some(format!("https://tidal.com/browse/track/{}", t.id));
    }
    let e = ext?;
    if e.kind == "file" || e.src.is_empty() {
        return None;
    }
    Some(if e.src.starts_with("http") { e.src.clone() } else { format!("https://www.youtube.com/watch?v={}", e.src) })
}

/// Menu entry that shows the link and copies it. Nothing is shown when there is no link (local files).
fn link_item(ui: &mut egui::Ui, acts: &mut Vec<Action>, link: Option<String>) {
    if let Some(l) = link {
        let short: String =
            if l.chars().count() > 46 { format!("{}...", l.chars().take(44).collect::<String>()) } else { l.clone() };
        if menu_item(ui, &format!("Copy link   {}", short.trim_start_matches("https://"))) {
            acts.push(Action::CopyLink(l));
            ui.close_menu();
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
            false,
            false,
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
        if ibtn(ui, &IC_PLAY, "PLAY", false).clicked() {
            acts.push(Action::Play(tracks.to_vec(), 0));
        }
        if ibtn(ui, &IC_SHUF, "SHUFFLE", false).clicked() {
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
    liked: &HashSet<i64>,
    acts: &mut Vec<Action>,
) {
    ui.add_space(6.0);

    // ---- "Your Library": one tab at a time, MY TRACKS first
    if page.library {
        match tab {
            Tab::Tracks => {
                title_line(ui, "MY TRACKS", 3.0, pal().ink);
                let sub = if page.total > page.tracks.len() {
                    format!("{} of {} songs loaded...", page.tracks.len(), page.total)
                } else {
                    format!("{} songs", page.tracks.len())
                };
                title_line(ui, &sub, 2.0, pal().ink2);
                if !page.tracks.is_empty() {
                    ui.add_space(4.0);
                    play_buttons(ui, &page.tracks, acts);
                }
                ui.add_space(8.0);
                if page.tracks.is_empty() {
                    title_line(ui, "No liked songs found.", 2.0, pal().ink2);
                }
                tracks_list(ui, &page.tracks, playing_id, liked, acts);
            }
            t => {
                let (key, name) = match t {
                    Tab::Lists => ("Playlists", "PLAYLISTS"),
                    Tab::Albums => ("Albums", "ALBUMS"),
                    _ => ("Artists", "ARTISTS"),
                };
                title_line(ui, name, 3.0, pal().ink);
                ui.add_space(6.0);
                let mut any = false;
                for (title, cards) in &page.rows {
                    if title == key {
                        any = true;
                        title_line(ui, &format!("{} items", cards.len()), 2.0, pal().ink2);
                        ui.add_space(4.0);
                        cards_list(ui, cards, false, acts);
                    }
                }
                if !any {
                    title_line(ui, "Nothing here yet.", 2.0, pal().ink2);
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
            inset(ui.painter(), cr, pal().edge);
            paint_art(ui, images, &page.image, cr.shrink(2.0), 0.0);
            ui.vertical(|ui| {
                title_line(ui, &page.title, 3.0, pal().ink);
                title_line(ui, &page.subtitle, 2.0, pal().ink2);
                ui.add_space(6.0);
                if !page.tracks.is_empty() {
                    play_buttons(ui, &page.tracks, acts);
                }
            });
        });
    } else {
        title_line(ui, &page.title, 3.0, pal().ink);
    }
    ui.add_space(6.0);

    if page.tracks.is_empty() && page.rows.is_empty() {
        title_line(ui, "Nothing to show.", 2.0, pal().ink2);
    }
    for (title, cards) in &page.rows {
        section_header(ui, title);
        cards_list(ui, cards, true, acts);
    }
    if !page.tracks.is_empty() {
        if !page.rows.is_empty() {
            section_header(ui, "SONGS");
        }
        tracks_list(ui, &page.tracks, playing_id, liked, acts);
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
    bands: Vec<f32>,
    peaks: Vec<f32>,
    viz_n: usize,
    viz_color: u8,
    show_log: bool,
    audio_err_shown: bool,

    art_view: bool,
    art_gray: bool,
    show_spec: bool,
    spec_op: f32,
    spec_h: f32,
    spec_w: f32,
    viz_mode: u8,
    viz_w: f32,
    viz_gain: f32,
    viz_wave: Vec<f32>,
    show_lyrics: bool,
    fullscreen: bool,
    art_tilt: Vec2,
    lyrics: Option<Lyrics>,
    lyric_scroll: f32,
    lyric_free: f32,

    liked: HashSet<i64>,
    q_drag: Option<usize>,
    sleep_at: Option<Instant>,
    sleep_mins: u32,
    mini: bool,
    pinned: bool,
    saved_size: Vec2,
    show_eq: bool,
    eq_gains: [f32; 10],
    eq_on: bool,
    dirty: bool,
    last_save: Instant,
    last_title: String,

    practice: bool,
    loop_a: Option<f32>,
    loop_b: Option<f32>,
    loop_on: bool,
    speed: u32,
    loops: HashMap<i64, (f32, f32)>,
    menu_t: f32,
    show_cache: bool,
    keep_cache: bool,
    cache_stats: (usize, u64),
    cache_at: Instant,

    // ---- sources, tunes, diary (see extras.rs / views.rs)
    store: store::Store,
    store_dirty: bool,
    store_saved: Instant,
    srcmap: HashMap<i64, Src>,
    sec: Sec,
    ed: Ed,
    files_busy: bool,
    yt_busy: bool,
    yt_msg: String,
    yt_in: String,
    sc_in: String,
    marker_drag: u8,
    pan_off: f32,
    pan_len: f32,
    sc_msg: String,
    sc_busy: bool,
    sc_results: Vec<store::Ext>,
    sc_sets: Vec<(String, String)>,
    sc_cur: Option<String>,
    upd: Option<update::Info>,
    upd_state: u8,
    /// (track id, cents from A440, confidence) from the tuning check
    tuning: Option<(i64, i32, f32)>,
    tuning_busy: bool,
    upd_checked: Option<Instant>,
    new_pl: String,
    pl_open: Option<usize>,
    pl_pick: Option<Track>,
    yt_results: Vec<store::Ext>,
    yt_cur: Option<String>,
    offline: bool,
    new_tune: String,
    tune_open: Option<usize>,
    edit_ver: Option<usize>,
    del_arm: bool,
    pick: Option<Track>,
    look_busy: bool,
    look_for: String,
    look_tracks: Vec<(Track, u32)>,
    f_tune: String,
    f_mins: String,
    f_bpm: String,
    f_note: String,
    show_form: bool,

    // ---- practice tools
    wave: (i64, Option<Vec<u8>>),
    focus_mode: bool,
    more: bool,
    /// stems: 0 idle, 1 downloading the tool, 2 splitting a track
    stem_busy: u8,
    stem_data: Option<(i64, stems::Stems)>,
    stem_on: [bool; 4],
    /// focus timer: 0 idle, 1 focus block, 2 break
    pomo: u8,
    pomo_end: Option<Instant>,
    pomo_focus: u32,
    pomo_break: u32,
    pomo_cycles: u32,
    pomo_n: u32,
    pomo_flash: Option<Instant>,
    timer_open: bool,
    metro_open: bool,
    bpm: u32,
    count_in: u32,
    taps: Vec<Instant>,
    trainer: bool,
    trainer_step: u32,
    trainer_n: u32,
    passes: u32,
    last_wraps: u32,
    /// pitch shift in cents (100 = a semitone)
    semis: i32,
    chan: u32,
    acc: f32,
    acc_at: Instant,
    last_cap: Instant,
    sec_name: String,
    pending_seek: Option<f32>,
    sel_anchor: Option<f32>,
    exporting: bool,

    // ---- lead sheet
    rtab: u8,
    chart_tr: i32,
    chart_edit: bool,
    chart_busy: bool,
    chart_sugg: Vec<String>,
    chart_q: String,
    live_q: String,
    live: Option<(String, String, String)>,
    chart_live: bool,
    chart_pick: Option<usize>,
    mtab: u8,
    /// Diary: month shown (year, month; 0 = this month) and the day picked (days since 1970; 0 = today)
    diary_ym: (i32, u32),
    diary_sel: i64,
    mt: band::Metro,
    mt_bpm: u32,
    mt_add: f32,
    mt_step_at: Instant,
    mt_sig: u64,
    mt_group: usize,
    vol_before: f32,
    look_style: u8,
    last_ppp: f32,
    last_size: Vec2,
    frames: u32,
    mt_vis: Option<(Instant, f32, usize)>,
    beat_failed: HashSet<i64>,
    mt_gen: u32,
    mt_pos: (f32, Instant),
    mt_sync_at: Instant,
    beat: Option<(i64, f32, f32)>,
    beat_busy: bool,
    band_on: bool,
    band_style: u8,
    band_parts: [bool; 3],
    band_t0: Instant,
    band_lead: f32,
    band_bar: f32,
    band_order: Vec<usize>,
    band_sig: u64,
    band_chart: String,
    pomo_sound: bool,
    lib_frac: f32,
    stack_frac: f32,
    ebuf: String,
    ebuf_id: u32,
    chart_tried: std::collections::HashSet<String>,
    chart_rn: bool,
    ireal_in: String,
    tunes_ireal: String,
    look_q: String,
    chart_cache: (String, chart::Chart),
}

/// Audio bytes for a track: a file on this computer, from the local cache if it's stored,
/// a YouTube clip (downloaded once with yt-dlp), or otherwise downloaded from Tidal
/// (and then stored, unless caching is switched off).
fn fetch_audio(api: &Api, id: i64, lossless: bool, keep: bool, src: Option<Src>) -> Result<Vec<u8>, String> {
    match src {
        Some(Src::File(p)) => {
            return std::fs::read(&p).map_err(|e| format!("can't open {}: {}", p.display(), e));
        }
        Some(Src::Yt(vid)) => {
            if let Some(b) = cache::get(id) {
                crate::api::log(&format!("youtube {}: playing from disk ({} bytes)", vid, b.len()));
                return Ok(b);
            }
            let b = sources::yt_fetch(&vid)?;
            if let Some(p) = cache::put(id, &b) {
                crate::api::log(&format!("youtube {}: saved to {}", vid, p.display()));
            }
            return Ok(b);
        }
        None => {}
    }
    if let Some(b) = cache::get(id) {
        crate::api::log(&format!("track {}: playing from disk ({} bytes)", id, b.len()));
        return Ok(b);
    }
    let b = api.stream_url(id, lossless).and_then(|u| api.fetch(&u))?;
    if keep {
        match cache::put(id, &b) {
            Some(p) => crate::api::log(&format!("track {}: saved to {}", id, p.display())),
            None => crate::api::log(&format!("track {}: could not save to {}", id, cache::dir().display())),
        }
    }
    Ok(b)
}

fn load_settings() -> serde_json::Value {
    std::fs::read_to_string(api::config_dir().join("settings.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(serde_json::Value::Null)
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> App {
        let st = load_settings();
        if let Some(v) = st["skin"].as_u64() {
            SKIN.store((v as usize) % PALS.len(), Ordering::Relaxed);
        }
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
            images: Images { map: HashMap::new(), requested: HashSet::new(), req, avg: HashMap::new() },
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
            bands: vec![0.0; 32],
            peaks: vec![0.0; 32],
            viz_n: 32,
            viz_color: 0,
            show_log: false,
            audio_err_shown: false,
            art_view: false,
            art_gray: false,
            show_spec: true,
            spec_op: 0.2,
            spec_h: 0.42,
            spec_w: 0.6,
            viz_mode: 0,
            viz_w: 0.8,
            viz_gain: 1.0,
            viz_wave: vec![0.0; WAVE_N],
            show_lyrics: true,
            fullscreen: false,
            art_tilt: Vec2::ZERO,
            lyrics: None,
            lyric_scroll: 0.0,
            lyric_free: 0.0,
            liked: HashSet::new(),
            q_drag: None,
            sleep_at: None,
            sleep_mins: 0,
            mini: false,
            pinned: false,
            saved_size: Vec2::ZERO,
            show_eq: false,
            eq_gains: [0.0; 10],
            eq_on: false,
            dirty: false,
            last_save: Instant::now(),
            last_title: String::new(),
            practice: false,
            loop_a: None,
            loop_b: None,
            loop_on: false,
            speed: 100,
            loops: HashMap::new(),
            menu_t: 0.0,
            show_cache: false,
            keep_cache: true,
            cache_stats: (0, 0),
            cache_at: Instant::now(),
            store: store::Store::load(),
            store_dirty: false,
            store_saved: Instant::now(),
            srcmap: HashMap::new(),
            sec: Sec::Tidal,
            ed: Ed::default(),
            files_busy: false,
            yt_busy: false,
            yt_msg: String::new(),
            yt_in: String::new(),
            sc_in: String::new(),
            marker_drag: 0,
            pan_off: 0.0,
            pan_len: 0.0,
            sc_msg: String::new(),
            sc_busy: false,
            sc_results: Vec::new(),
            sc_sets: Vec::new(),
            sc_cur: None,
            upd: None,
            upd_state: 0,
            tuning: None,
            tuning_busy: false,
            upd_checked: None,
            new_pl: String::new(),
            pl_open: None,
            pl_pick: None,
            yt_results: Vec::new(),
            yt_cur: None,
            offline: false,
            new_tune: String::new(),
            tune_open: None,
            edit_ver: None,
            del_arm: false,
            pick: None,
            look_busy: false,
            look_for: String::new(),
            look_tracks: Vec::new(),
            f_tune: String::new(),
            f_mins: String::new(),
            f_bpm: String::new(),
            f_note: String::new(),
            show_form: false,
            wave: (0, None),
            focus_mode: false,
            more: false,
            stem_busy: 0,
            stem_data: None,
            stem_on: [true; 4],
            pomo: 0,
            pomo_end: None,
            pomo_focus: 25,
            pomo_break: 5,
            pomo_cycles: 4,
            pomo_n: 1,
            pomo_flash: None,
            timer_open: false,
            metro_open: false,
            bpm: 0,
            count_in: 0,
            taps: Vec::new(),
            trainer: false,
            trainer_step: 5,
            trainer_n: 4,
            passes: 0,
            last_wraps: 0,
            semis: 0,
            chan: 0,
            acc: 0.0,
            acc_at: Instant::now(),
            last_cap: Instant::now(),
            sec_name: String::new(),
            pending_seek: None,
            sel_anchor: None,
            exporting: false,
            rtab: 0,
            chart_tr: 0,
            chart_edit: false,
            chart_busy: false,
            chart_sugg: Vec::new(),
            chart_q: String::new(),
            live_q: String::new(),
            live: None,
            chart_live: false,
            chart_pick: None,
            mtab: 0,
            diary_ym: (0, 0),
            diary_sel: 0,
            mt: band::Metro::default(),
            mt_bpm: 100,
            mt_add: 0.0,
            mt_step_at: Instant::now(),
            mt_sig: 0,
            mt_group: 0,
            vol_before: 0.7,
            look_style: 0,
            last_ppp: 0.0,
            last_size: Vec2::ZERO,
            frames: 0,
            mt_vis: None,
            beat_failed: HashSet::new(),
            mt_gen: 0,
            mt_pos: (0.0, Instant::now()),
            mt_sync_at: Instant::now(),
            beat: None,
            beat_busy: false,
            band_on: false,
            band_style: 0,
            band_parts: [true; 3],
            band_t0: Instant::now(),
            band_lead: 0.0,
            band_bar: 2.0,
            band_order: Vec::new(),
            band_sig: 0,
            band_chart: String::new(),
            pomo_sound: false,
            lib_frac: 0.34,
            stack_frac: 1.0,
            ebuf: String::new(),
            ebuf_id: 0,
            chart_tried: Default::default(),
            chart_rn: true,
            ireal_in: String::new(),
            tunes_ireal: String::new(),
            look_q: String::new(),
            chart_cache: (String::new(), chart::Chart::default()),
        };
        app.init_store();
        app.liked.extend(app.store.hearts.iter().map(|e| e.id));
        // ---- restore saved settings
        if let Some(v) = st["volume"].as_f64() {
            app.volume = (v as f32).clamp(0.0, 1.0);
        }
        if let Some(b) = st["lossless"].as_bool() {
            app.prefer_lossless = b;
        }
        if let Some(b) = st["spec"].as_bool() {
            app.show_spec = b;
        }
        if let Some(f) = st["spec_op"].as_f64() {
            app.spec_op = (f as f32).clamp(0.05, 0.8);
        }
        if let Some(n) = st["viz_n"].as_u64() {
            app.viz_n = (n as usize).clamp(8, 96);
        }
        if let Some(n) = st["viz_color2"].as_u64() {
            app.viz_color = (n as u8).min(2);
        }
        if let Some(n) = st["viz_mode"].as_u64() {
            app.viz_mode = (n as u8).min(2);
        }
        if let Some(f) = st["viz_w"].as_f64() {
            app.viz_w = (f as f32).clamp(0.3, 1.0);
        }
        if let Some(f) = st["viz_gain"].as_f64() {
            app.viz_gain = (f as f32).clamp(0.4, 3.0);
        }
        if let Some(f) = st["spec_w"].as_f64() {
            app.spec_w = (f as f32).clamp(0.2, 1.0);
        }
        if let Some(f) = st["spec_h"].as_f64() {
            app.spec_h = (f as f32).clamp(0.15, 0.9);
        }
        if let Some(b) = st["art_gray"].as_bool() {
            app.art_gray = b;
        }
        if let Some(b) = st["lyrics"].as_bool() {
            app.show_lyrics = b;
        }
        if let Some(r) = st["repeat"].as_u64() {
            app.repeat = match r {
                1 => Repeat::All,
                2 => Repeat::One,
                _ => Repeat::Off,
            };
        }
        if let Some(b) = st["eq_on"].as_bool() {
            app.eq_on = b;
        }
        if let Some(a) = st["eq"].as_array() {
            for (i, x) in a.iter().take(10).enumerate() {
                app.eq_gains[i] = (x.as_f64().unwrap_or(0.0) as f32).clamp(-12.0, 12.0);
            }
        }
        if let Some(b) = st["keep_cache"].as_bool() {
            app.keep_cache = b;
        }
        if let Some(b) = st["sc_browser"].as_u64() {
            sources::SC_BROWSER.store((b as u8).min(3), Ordering::Relaxed);
        }
        if st["offline"].as_bool() == Some(true) && !app.api.has_token() {
            app.offline = true;
            app.sec = Sec::Files;
        }
        if let Some(f) = st["lib_frac"].as_f64() {
            app.lib_frac = (f as f32).clamp(0.2, 0.7);
        }
        if let Some(f) = st["stack_frac"].as_f64() {
            app.stack_frac = (f as f32).clamp(0.3, 1.0);
        }
        if let Some(c) = st["cols"].as_u64() {
            COLS.store(c as u32, Ordering::Relaxed);
        }
        if let Some(b) = st["pomo_sound"].as_bool() {
            app.pomo_sound = b;
        }
        if let Some(p) = st["speed"].as_u64() {
            app.speed = (p as u32).clamp(25, 150);
        }
        if let Some(o) = st["loops"].as_object() {
            for (k, v) in o {
                if let (Ok(id), Some(a), Some(b)) = (k.parse::<i64>(), v[0].as_f64(), v[1].as_f64()) {
                    app.loops.insert(id, (a as f32, b as f32));
                }
            }
        }
        cache::scan();
        app.player.send(Cmd::Volume(app.volume * app.volume));
        app.apply_eq();
        media::spawn(app.tx.clone(), app.ctx.clone());
        crate::api::log(&format!("tidalite {} started (token saved: {})", VERSION, app.api.has_token()));
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

    fn apply_eq(&self) {
        self.player.eq.set(self.eq_gains, self.eq_on);
    }

    fn save_settings(&mut self) {
        let v = serde_json::json!({
            "skin": SKIN.load(Ordering::Relaxed),
            "volume": self.volume,
            "lossless": self.prefer_lossless,
            "art_gray": self.art_gray,
            "spec": self.show_spec,
            "spec_op": self.spec_op,
            "spec_h": self.spec_h,
            "spec_w": self.spec_w,
            "viz_mode": self.viz_mode,
            "viz_n": self.viz_n,
            "viz_color2": self.viz_color,
            "viz_w": self.viz_w,
            "viz_gain": self.viz_gain,
            "lyrics": self.show_lyrics,
            "repeat": match self.repeat { Repeat::Off => 0, Repeat::All => 1, Repeat::One => 2 },
            "eq_on": self.eq_on,
            "eq": self.eq_gains.to_vec(),
            "keep_cache": self.keep_cache,
            "speed": self.speed,
            "lib_frac": self.lib_frac,
            "offline": self.offline,
            "sc_browser": sources::SC_BROWSER.load(Ordering::Relaxed),
            "stack_frac": self.stack_frac,
            "pomo_sound": self.pomo_sound,
            "cols": COLS.load(Ordering::Relaxed),
            "loops": self.loops.iter().map(|(k, v)| (k.to_string(), serde_json::json!([v.0, v.1]))).collect::<serde_json::Map<String, serde_json::Value>>(),
        });
        let dir = api::config_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("settings.json"), v.to_string());
        self.dirty = false;
        self.last_save = Instant::now();
    }

    /// Fade out and stop when the sleep timer runs out.
    fn sleep_tick(&mut self) {
        let Some(at) = self.sleep_at else { return };
        let now = Instant::now();
        if now >= at {
            self.sleep_at = None;
            self.sleep_mins = 0;
            self.apply(Action::StopBtn);
            self.player.send(Cmd::Volume(self.volume * self.volume));
            self.set_note("SLEEP TIMER - GOOD NIGHT");
        } else {
            let rem = (at - now).as_secs_f32();
            if rem < 20.0 {
                self.player.send(Cmd::Volume(self.volume * self.volume * (rem / 20.0)));
            }
        }
    }

    /// Push loop / speed state to the audio thread and remember the loop for this track.
    fn sync_loop(&mut self) {
        let both = self.loop_a.zip(self.loop_b);
        let on = self.practice && self.loop_on && both.is_some();
        let (a, b) = both.unwrap_or((0.0, 0.0));
        self.player.ctl.set_loop(on, a, b);
        self.player.ctl.set_speed(if self.practice { self.speed } else { 100 });
        self.player.ctl.set_semis(if self.practice { self.semis } else { 0 });
        self.player.ctl.set_chan(if self.practice { self.chan } else { 0 });
        self.player.ctl.set_count_in(if self.practice { self.count_in } else { 0 }, self.bpm as f32);
        if let Some(t) = self.cur_track() {
            match both {
                Some(ab) => {
                    self.loops.insert(t.id, ab);
                }
                None => {
                    self.loops.remove(&t.id);
                }
            }
        }
        self.dirty = true;
    }

    fn track_len(&self) -> f32 {
        self.cur_track().map(|t| t.duration).unwrap_or(0.0)
    }

    fn sync_library_like(&mut self, t: &Track, like: bool) {
        let f = |p: &mut Arc<Page>| {
            if p.library {
                let pg = Arc::make_mut(p);
                if like {
                    if !pg.tracks.iter().any(|x| x.id == t.id) {
                        pg.tracks.insert(0, t.clone());
                        pg.total += 1;
                    }
                } else {
                    pg.tracks.retain(|x| x.id != t.id);
                    pg.total = pg.total.saturating_sub(1);
                }
            }
        };
        if let Some(p) = self.page.as_mut() {
            f(p);
        }
        for b in self.back.iter_mut() {
            f(b);
        }
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
        self.offline = false;
        self.dirty = true;
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
                        self.liked.extend(p.tracks.iter().map(|t| t.id));
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
                Msg::Media(k) => match k {
                    1 => self.apply(Action::Toggle),
                    2 => self.apply(Action::Next),
                    3 => self.apply(Action::Prev),
                    _ => self.apply(Action::StopBtn),
                },
                Msg::LikeFailed(id, like, e) => {
                    if like {
                        self.liked.remove(&id);
                    } else {
                        self.liked.insert(id);
                    }
                    self.set_err(format!("LIKE FAILED: {}", e));
                }
                Msg::MoreTracks(g, v) => {
                    if g == self.lib_gen {
                        self.liked.extend(v.iter().map(|t| t.id));
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
                        self.start_wave(id, &bytes);
                        self.player.send(Cmd::Play(bytes));
                        self.after_play();
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
                    if let Some(ci) = &img {
                        let half = ci.pixels.len() / 2;
                        let mean = |px: &[Color32]| -> Color32 {
                            let n = px.len().max(1) as u32;
                            let s =
                                px.iter().fold([0u32; 3], |a, c| [a[0] + c.r() as u32, a[1] + c.g() as u32, a[2] + c.b() as u32]);
                            Color32::from_rgb((s[0] / n) as u8, (s[1] / n) as u8, (s[2] / n) as u8)
                        };
                        let (top, bot) = ci.pixels.split_at(half);
                        self.images.avg.insert(url.clone(), [mean(top), mean(bot)]);
                    }
                    let tex = img.map(|ci| ctx.load_texture(&url, ci, egui::TextureOptions::LINEAR));
                    self.images.map.insert(url, tex);
                }
                other => self.drain_extra(other),
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
        self.on_track_change(&t);
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
                self.start_wave(id, &bytes);
                self.player.send(Cmd::Play(bytes));
                self.after_play();
                self.set_kbps(n);
                self.buffering = false;
                self.prefetch_next(id);
                return;
            }
        }
        self.buffering = true;
        let lossless = self.prefer_lossless;
        let keep = self.keep_cache;
        let src = self.srcmap.get(&t.id).cloned();
        let (api, tx, ctx) = (self.api.clone(), self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let r = fetch_audio(&api, t.id, lossless, keep, src);
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
        self.queue.get(cur + 1).cloned().or_else(|| if self.repeat == Repeat::All { self.queue.first().cloned() } else { None })
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
        let keep = self.keep_cache;
        let src = self.srcmap.get(&id).cloned();
        let (api, tx, ctx) = (self.api.clone(), self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            if let Ok(b) = fetch_audio(&api, id, lossless, keep, src) {
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

    /// Bar colours for the visualizer: None = the skin's own, else (main, cap).
    fn viz_tint(&self) -> Option<(Color32, Color32)> {
        match self.viz_color {
            2 => Some((Color32::from_rgb(70, 220, 110), Color32::from_rgb(255, 72, 60))),
            1 => {
                let t = self.cur_track()?;
                let avg = self
                    .images
                    .avg
                    .get(&cover_url(&t.cover, 640))
                    .or_else(|| self.images.avg.get(&cover_url(&t.cover, 160)))
                    .copied()?;
                // the cover's own colour lifted to a readable level, with a gentle complementary tip
                let (h, sat, _) = rgb_to_hsv(avg[1]);
                let (h0, s0, _) = rgb_to_hsv(avg[0]);
                let h = if sat < 0.12 { h0 } else { h };
                let s = if sat < 0.12 { s0 } else { sat };
                let pri = hsv_to_rgb(h, s.clamp(0.35, 0.85), 0.9);
                let sec = hsv_to_rgb((h + 0.42) % 1.0, 0.5, 1.0);
                Some((pri, sec))
            }
            _ => None,
        }
    }

    /// Right-click menu shared by both visualizers.
    fn viz_menu(&mut self, ui: &mut egui::Ui) {
        if menu_item(
            ui,
            &format!("STYLE: {}  (click to change)", ["BARS", "WAVEFORM", "BARS + WAVE"][self.viz_mode as usize % 3]),
        ) {
            self.viz_mode = (self.viz_mode + 1) % 3;
        }
        let mut n = self.viz_n as f32;
        if menu_item(ui, &format!("NUMBER OF BARS  {}   (+)", self.viz_n)) {
            n = (n + 4.0).min(96.0);
        }
        if menu_item(ui, &format!("NUMBER OF BARS  {}   (-)", self.viz_n)) {
            n = (n - 4.0).max(8.0);
        }
        self.viz_n = n as usize;
        let cname = ["SKIN", "FROM COVER ART", "CLASSIC GREEN + RED"][self.viz_color as usize % 3];
        if menu_item(ui, &format!("COLORS: {}  (click to change)", cname)) {
            self.viz_color = (self.viz_color + 1) % 3;
        }
        for (name, v, d, lo, hi) in
            [("BAR WIDTH", &mut self.viz_w, 0.1, 0.3, 1.0), ("SENSITIVITY", &mut self.viz_gain, 0.2, 0.4, 3.0)]
        {
            if menu_item(ui, &format!("{}  {}%   (+)", name, (*v * 100.0).round() as i32)) {
                *v = (*v + d).min(hi);
            }
            if menu_item(ui, &format!("{}  {}%   (-)", name, (*v * 100.0).round() as i32)) {
                *v = (*v - d).max(lo);
            }
        }
        self.dirty = true;
    }

    fn update_bands(&mut self) {
        let active = self.cur.is_some() && !self.paused && !self.stopped && !self.buffering;
        let (samples, rate) = if active {
            let v = self.player.viz.lock().unwrap();
            let n = v.samples.len().min(2048);
            (v.samples[v.samples.len() - n..].to_vec(), v.rate as f32)
        } else {
            (Vec::new(), 44100.0)
        };
        let n = samples.len();
        let nb = self.viz_n.clamp(8, 96);
        if self.bands.len() != nb {
            self.bands = vec![0.0; nb];
            self.peaks = vec![0.0; nb];
        }
        let mut target = vec![0.0f32; nb];
        if n >= 512 && rate > 0.0 {
            let denom = (n - 1) as f32;
            let xs: Vec<f32> = samples
                .iter()
                .enumerate()
                .map(|(i, v)| v * (0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / denom).cos()))
                .collect();
            for i in 0..nb {
                let f = 50.0 * (12000.0f32 / 50.0).powf(i as f32 / (nb - 1) as f32);
                if f >= rate / 2.0 {
                    continue;
                }
                let amp = goertzel(&xs, f, rate) * 2.0;
                let db = 20.0 * (amp + 1e-6).log10();
                target[i] = ((db + 64.0 + 23.0 * i as f32 / nb as f32) / 54.0 * self.viz_gain).clamp(0.0, 1.0);
            }
        }
        // waveform: the loudest swing in each slice of the latest samples
        if self.viz_wave.len() != WAVE_N {
            self.viz_wave = vec![0.0; WAVE_N];
        }
        for k in 0..WAVE_N {
            let tv = if n >= WAVE_N {
                let (a, b) = (k * n / WAVE_N, ((k + 1) * n / WAVE_N).max(k * n / WAVE_N + 1));
                let mut best = 0.0f32;
                for v in &samples[a..b.min(n)] {
                    if v.abs() > best.abs() {
                        best = *v;
                    }
                }
                (best * 1.6 * self.viz_gain).clamp(-1.0, 1.0)
            } else {
                0.0
            };
            self.viz_wave[k] = self.viz_wave[k] * 0.45 + tv * 0.55;
        }
        for i in 0..nb {
            let old = self.bands[i];
            self.bands[i] = if target[i] > old { target[i] } else { (old - 0.06).max(target[i]) };
            self.peaks[i] = self.bands[i].max(self.peaks[i] - 0.012);
        }
    }

    fn apply(&mut self, a: Action) {
        match a {
            Action::StartLogin => self.start_login(),
            Action::Home => {
                self.sec = Sec::Tidal;
                self.back.clear();
                self.load(false, |a| a.home());
            }
            Action::Library => {
                self.sec = Sec::Tidal;
                self.back.clear();
                self.load(false, |a| a.library());
            }
            Action::Tab(t) => {
                self.lib_tab = t;
                self.serial += 1; // scroll back to the top
            }
            Action::Search(q) => {
                if !q.trim().is_empty() {
                    self.sec = Sec::Tidal;
                    self.load(true, move |a| a.search(q.trim()));
                }
            }
            Action::Open(c) => {
                self.sec = Sec::Tidal;
                self.load(true, move |a| a.open(&c))
            }
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
                self.dirty = true;
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
                self.dirty = true;
                self.refresh_prefetch();
            }
            Action::ToggleLossless => {
                self.prefer_lossless = !self.prefer_lossless;
                self.dirty = true;
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
            Action::ToggleLike(t) => {
                if t.id < 0 {
                    // not on Tidal: keep it in the LIKED list on this computer
                    if let Some(i) = self.store.hearts.iter().position(|e| e.id == t.id) {
                        self.store.hearts.remove(i);
                        self.liked.remove(&t.id);
                        self.set_note("REMOVED FROM LIKED");
                    } else {
                        let found = self
                            .store
                            .files
                            .iter()
                            .chain(self.store.yt.iter())
                            .chain(self.store.sc.iter())
                            .chain(self.sc_results.iter())
                            .find(|e| e.id == t.id)
                            .cloned();
                        if let Some(e) = found {
                            self.store.hearts.push(e);
                            self.liked.insert(t.id);
                            self.set_note("ADDED TO LIKED (ON THIS PC)");
                        }
                    }
                    self.store_dirty = true;
                    return;
                }
                let like = !self.liked.contains(&t.id);
                if like {
                    self.liked.insert(t.id);
                } else {
                    self.liked.remove(&t.id);
                }
                self.sync_library_like(&t, like);
                let (api, tx, ctx) = (self.api.clone(), self.tx.clone(), self.ctx.clone());
                let id = t.id;
                std::thread::spawn(move || {
                    if let Err(e) = api.set_liked(id, like) {
                        let _ = tx.send(Msg::LikeFailed(id, like, e));
                        ctx.request_repaint();
                    }
                });
                self.set_note(if like { "ADDED TO MY TRACKS" } else { "REMOVED FROM MY TRACKS" });
            }
            Action::MoveQueue(from, ins) => {
                let n = self.queue.len();
                if from >= n {
                    return;
                }
                let ins = ins.min(n);
                let to = if ins > from { ins - 1 } else { ins };
                if to == from {
                    return;
                }
                let q = Arc::make_mut(&mut self.queue);
                let t = q.remove(from);
                q.insert(to, t);
                if let Some(c) = self.cur {
                    self.cur = Some(if c == from {
                        to
                    } else {
                        let mut c2 = c;
                        if from < c2 {
                            c2 -= 1;
                        }
                        if to <= c2 {
                            c2 += 1;
                        }
                        c2
                    });
                }
                self.refresh_prefetch();
            }
            Action::SkinSet(n) => {
                let n = n % PALS.len();
                SKIN.store(n, Ordering::Relaxed);
                setup_style(&self.ctx);
                self.dirty = true;
                self.set_note(&format!("SKIN: {}", SKIN_NAMES[n]));
            }
            Action::Skin => {
                let n = (SKIN.load(Ordering::Relaxed) + 1) % PALS.len();
                SKIN.store(n, Ordering::Relaxed);
                setup_style(&self.ctx);
                self.dirty = true;
                self.set_note(&format!("SKIN: {}", SKIN_NAMES[n]));
            }
            Action::Sleep => {
                const STEPS: [u32; 6] = [0, 15, 30, 45, 60, 90];
                let cur = STEPS.iter().position(|m| *m == self.sleep_mins).unwrap_or(0);
                let n = STEPS[(cur + 1) % STEPS.len()];
                self.sleep_mins = n;
                if n == 0 {
                    self.sleep_at = None;
                    self.player.send(Cmd::Volume(self.volume * self.volume));
                    self.set_note("SLEEP TIMER OFF");
                } else {
                    self.sleep_at = Some(Instant::now() + Duration::from_secs(n as u64 * 60));
                    self.set_note(&format!("SLEEP IN {} MIN", n));
                }
            }
            Action::ToggleMini => {
                self.mini = !self.mini;
                if self.mini {
                    self.saved_size = self.ctx.screen_rect().size();
                    self.ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(Vec2::new(600.0, 260.0)));
                    self.ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(640.0, 316.0)));
                } else {
                    self.ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(Vec2::new(980.0, 640.0)));
                    let sz = if self.saved_size.x > 100.0 { self.saved_size } else { Vec2::new(1280.0, 820.0) };
                    self.ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(sz));
                }
            }
            Action::TogglePin => {
                self.pinned = !self.pinned;
                let lvl =
                    if self.pinned { egui::viewport::WindowLevel::AlwaysOnTop } else { egui::viewport::WindowLevel::Normal };
                self.ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(lvl));
            }
            Action::ToggleEq => {
                self.show_eq = !self.show_eq;
                if self.show_eq {
                    self.show_log = false;
                    self.show_cache = false;
                }
            }
            Action::ToggleCacheView => {
                self.show_cache = !self.show_cache;
                if self.show_cache {
                    self.show_log = false;
                    self.show_eq = false;
                    self.cache_stats = cache::stats();
                }
            }
            Action::ClearCache => {
                cache::clear();
                self.cache_stats = cache::stats();
                self.set_note("LOCAL CACHE CLEARED");
            }
            Action::OpenCache => cache::open_folder(),
            Action::ToggleKeep => {
                self.keep_cache = !self.keep_cache;
                self.dirty = true;
                self.set_note(if self.keep_cache { "SAVING TRACKS TO DISK" } else { "NOT SAVING NEW TRACKS" });
            }
            Action::TogglePractice if PRACTICE => {
                self.practice = !self.practice;
                if self.practice {
                    self.loop_on = self.loop_a.is_some() && self.loop_b.is_some();
                    self.set_note("PRACTICE MODE");
                } else {
                    self.set_note("LISTENING MODE");
                }
                self.sync_loop();
            }
            Action::SetAAt(t) => {
                if self.cur.is_none() || self.stopped {
                    return;
                }
                self.practice = true;
                let t = t.clamp(0.0, self.track_len());
                self.loop_a = Some(t);
                if let Some(b) = self.loop_b {
                    if b <= t + 0.1 {
                        self.loop_b = None;
                    }
                }
                self.loop_on = self.loop_b.is_some();
                self.sync_loop();
                self.set_note(&format!("LOOP A = {}", fmt_t(t)));
            }
            Action::SetBAt(t) => {
                if self.cur.is_none() || self.stopped {
                    return;
                }
                self.practice = true;
                let t = t.clamp(0.0, self.track_len());
                self.loop_b = Some(t);
                match self.loop_a {
                    Some(a) if a + 0.1 < t => {}
                    _ => self.loop_a = Some(0.0),
                }
                self.loop_on = true;
                self.sync_loop();
                if let (Some(a), Some(b)) = (self.loop_a, self.loop_b) {
                    if self.pos() >= b - 0.05 {
                        self.player.send(Cmd::Seek(a));
                    }
                }
                self.set_note(&format!("LOOP B = {}", fmt_t(t)));
            }
            Action::NudgeA(d) => {
                if let Some(a) = self.loop_a {
                    let hi = self.loop_b.map(|b| b - 0.1).unwrap_or_else(|| self.track_len()).max(0.0);
                    let n = (a + d).clamp(0.0, hi);
                    self.loop_a = Some(n);
                    self.sync_loop();
                    self.player.send(Cmd::Seek(n));
                }
            }
            Action::NudgeB(d) => {
                if let Some(b) = self.loop_b {
                    let lo = self.loop_a.map(|a| a + 0.1).unwrap_or(0.0);
                    let n = (b + d).clamp(lo, self.track_len().max(lo));
                    self.loop_b = Some(n);
                    self.sync_loop();
                    self.player.send(Cmd::Seek((n - 1.5).max(self.loop_a.unwrap_or(0.0))));
                }
            }
            Action::LoopToggle => {
                if self.loop_a.is_none() || self.loop_b.is_none() {
                    self.set_note("SET LOOP A AND B FIRST");
                    return;
                }
                self.practice = true;
                self.loop_on = !self.loop_on;
                if self.loop_on {
                    let (a, b) = (self.loop_a.unwrap_or(0.0), self.loop_b.unwrap_or(0.0));
                    let p = self.pos();
                    if p < a || p >= b {
                        self.player.send(Cmd::Seek(a));
                    }
                }
                self.sync_loop();
            }
            Action::LoopClear => {
                self.loop_a = None;
                self.loop_b = None;
                self.loop_on = false;
                self.sync_loop();
                self.set_note("LOOP CLEARED");
            }
            Action::Speed(p) => {
                self.speed = p.clamp(25, 150);
                if self.cur.is_some() {
                    self.practice = true;
                }
                self.sync_loop();
            }
            Action::ToggleArt => self.art_view = !self.art_view,
            Action::ToggleSpec => {
                self.show_spec = !self.show_spec;
                self.dirty = true;
            }
            Action::ToggleGray => {
                self.art_gray = !self.art_gray;
                self.dirty = true;
            }
            Action::ToggleLyrics => {
                self.show_lyrics = !self.show_lyrics;
                self.dirty = true;
            }
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
                self.sec = Sec::Tidal;
            }
            other => self.apply_extra(other),
        }
    }

    // ---------------------------------------------------------------- views
    fn login_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        let full = ui.max_rect();
        let inner = Rect::from_center_size(full.center(), Vec2::new(480.0, 270.0));
        window_deco(ui, inner, "TIDALITE");
        let p = ui.painter();
        let c = inner.center();
        logo_mark(p, c + Vec2::new(-214.0, -102.0), 4.0);
        logo_mark(p, c + Vec2::new(178.0, -102.0), 4.0);
        ptext(p, c + Vec2::new(0.0, -84.0), Align::Center, "TIDALITE", 7.0, pal().ink);
        if PRACTICE {
            ptext(p, c + Vec2::new(0.0, -50.0), Align::Center, "A retro player for Tidal and more", 2.0, pal().ink2);
            ptext(p, c + Vec2::new(0.0, -32.0), Align::Center, "PRACTICE EDITION: LOOP, SLOW DOWN, LEARN TUNES", 1.0, pal().dim);
        } else {
            ptext(p, c + Vec2::new(0.0, -40.0), Align::Center, "A retro player for Tidal and more", 2.0, pal().ink2);
        }
        match self.auth {
            Auth::Checking => {
                ptext(p, c + Vec2::new(0.0, 24.0), Align::Center, "Checking session...", 2.0, pal().ink);
            }
            Auth::LoggingIn => match &self.login_code {
                Some((code, url)) => {
                    ptext(p, c + Vec2::new(0.0, -6.0), Align::Center, "Approve the login in your browser", 2.0, pal().ink);
                    ptext(p, c + Vec2::new(0.0, 34.0), Align::Center, code, 5.0, pal().ink);
                    let b = Rect::from_center_size(c + Vec2::new(0.0, 86.0), Vec2::new(300.0, 30.0));
                    let r =
                        ui.interact(b, ui.id().with("relink"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    raised_h(ui.painter(), b, r.is_pointer_button_down_on(), r.hovered());
                    ptext(ui.painter(), b.center(), Align::Center, "Open link again", 2.0, pal().ink);
                    if r.clicked() {
                        let _ = webbrowser::open(url);
                    }
                }
                None => {
                    ptext(p, c + Vec2::new(0.0, 24.0), Align::Center, "Contacting Tidal...", 2.0, pal().ink);
                }
            },
            _ => {
                let b = Rect::from_center_size(c + Vec2::new(0.0, 30.0), Vec2::new(320.0, 44.0));
                let r = ui.interact(b, ui.id().with("login"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                raised_h(ui.painter(), b, r.is_pointer_button_down_on(), r.hovered());
                ptext(ui.painter(), b.center(), Align::Center, "LOG IN WITH TIDAL", 3.0, pal().ink);
                if r.clicked() {
                    acts.push(Action::StartLogin);
                }
                let b2 = Rect::from_center_size(c + Vec2::new(0.0, 80.0), Vec2::new(320.0, 30.0));
                let r2 = ui.interact(b2, ui.id().with("skip"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                raised_h(ui.painter(), b2, r2.is_pointer_button_down_on(), r2.hovered());
                let skip = "USE WITHOUT TIDAL";
                ptext(ui.painter(), b2.center(), Align::Center, skip, 2.0, pal().ink2);
                if r2.clicked() {
                    acts.push(Action::Offline(true));
                }
                if !self.login_err.is_empty() {
                    let er = Rect::from_center_size(c + Vec2::new(0.0, 116.0), Vec2::new(430.0, 22.0));
                    inset(ui.painter(), er, pal().lcd);
                    marquee(ui, er, &self.login_err, 2.0, pal().red);
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
        inset(p, rect, pal().lcd);
        let inner = rect.shrink2(Vec2::new(8.0, 2.0));
        let cp = p.with_clip_rect(inner);
        let cy = rect.center().y;
        let prefix: String = self.search.chars().take(self.search_cur).collect();
        let caret_x = if prefix.is_empty() { 0.0 } else { text_w(&prefix, px) + spx(px) };
        let off = (caret_x - (inner.width() - 6.0)).max(0.0);
        if self.search.is_empty() {
            ptext(&cp, Pos2::new(inner.min.x, cy), Align::Min, "Search Tidal...", px, pal().dim);
        } else {
            ptext(&cp, Pos2::new(inner.min.x - off, cy), Align::Min, &self.search, px, pal().ink);
        }
        let t = ui.input(|i| i.time);
        if self.search_focus && (t * 2.0) as i64 % 2 == 0 {
            let r = Rect::from_min_size(Pos2::new(inner.min.x - off + caret_x, cy - 8.0), Vec2::new(spx(2.0), 16.0));
            fill_rect(&cp, r, pal().ink);
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
        egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show_rows(
            ui,
            rh,
            lines.len(),
            |ui, range| {
                for i in range {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), rh), Sense::hover());
                    let (txt, bad) = &lines[i];
                    ptext(
                        ui.painter(),
                        Pos2::new(rect.min.x + 6.0, rect.center().y),
                        Align::Min,
                        txt,
                        2.0,
                        if *bad { pal().red } else { pal().ink },
                    );
                }
            },
        );
    }

    fn eq_view(&mut self, ui: &mut egui::Ui) {
        const LABELS: [&str; 10] = ["31", "62", "125", "250", "500", "1K", "2K", "4K", "8K", "16K"];
        ui.add_space(6.0);
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            if retro_btn_w(ui, if self.eq_on { "EQ ON" } else { "EQ OFF" }, 84.0, self.eq_on).clicked() {
                self.eq_on = !self.eq_on;
                changed = true;
            }
            for (name, g) in EQ_PRESETS.iter() {
                if retro_btn_w(ui, name, 76.0, false).clicked() {
                    self.eq_gains = *g;
                    self.eq_on = true;
                    changed = true;
                }
            }
        });
        ui.add_space(8.0);
        let area = ui.available_rect_before_wrap();
        let h = (area.height() - 30.0).clamp(80.0, 240.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(area.width(), h), Sense::hover());
        let col_w = rect.width() / 10.0;
        let (top, bot) = (rect.min.y + 18.0, rect.max.y - 18.0);
        let p = ui.painter().clone();
        let y0 = top + (bot - top) * 0.5;
        fill_rect(&p, Rect::from_min_max(Pos2::new(rect.min.x + 6.0, y0), Pos2::new(rect.max.x - 6.0, y0 + 1.0)), pal().dim);
        for i in 0..10 {
            let cx = rect.min.x + col_w * (i as f32 + 0.5);
            let col = Rect::from_min_max(Pos2::new(cx - col_w / 2.0, rect.min.y), Pos2::new(cx + col_w / 2.0, rect.max.y));
            let r = ui.interact(col, ui.id().with(("eq_band", i)), Sense::click_and_drag());
            if r.dragged() || r.clicked() {
                if let Some(pp) = r.interact_pointer_pos() {
                    let f = ((pp.y - top) / (bot - top)).clamp(0.0, 1.0);
                    let mut g = ((12.0 - 24.0 * f) * 2.0).round() / 2.0;
                    if g.abs() < 0.75 {
                        g = 0.0;
                    }
                    self.eq_gains[i] = g;
                    self.eq_on = true;
                    changed = true;
                }
            }
            if r.double_clicked() {
                self.eq_gains[i] = 0.0;
                changed = true;
            }
            let g = self.eq_gains[i];
            inset(&p, Rect::from_min_max(Pos2::new(cx - 3.0, top), Pos2::new(cx + 3.0, bot)), pal().groove);
            let y = top + (bot - top) * (0.5 - g / 24.0);
            raised(&p, Rect::from_center_size(Pos2::new(cx, y), Vec2::new((col_w - 6.0).min(30.0), 12.0)), r.dragged());
            let gt = if g.abs() < 0.05 { "0".to_string() } else { format!("{:+}", g.round() as i32) };
            ptext(&p, Pos2::new(cx, rect.min.y + 8.0), Align::Center, &gt, 1.0, if self.eq_on { pal().ink } else { pal().dim });
            ptext(&p, Pos2::new(cx, rect.max.y - 8.0), Align::Center, LABELS[i], 1.0, pal().ink2);
        }
        ui.add_space(4.0);
        para(ui, "Drag a slider; double-click resets it. Range is +/- 12 dB.", pal().dim);
        if changed {
            self.apply_eq();
            self.dirty = true;
        }
    }

    fn cache_view(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        if self.cache_at.elapsed() > Duration::from_secs(1) {
            self.cache_stats = cache::stats();
            self.cache_at = Instant::now();
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            if retro_btn(ui, if self.keep_cache { "SAVING: ON" } else { "SAVING: OFF" }, self.keep_cache).clicked() {
                acts.push(Action::ToggleKeep);
            }
            if retro_btn(ui, "OPEN FOLDER", false).clicked() {
                acts.push(Action::OpenCache);
            }
        });
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            if retro_btn(ui, "CLEAR ALL", false).clicked() {
                acts.push(Action::ClearCache);
            }
            if retro_btn(ui, "CLEAR STEMS", false).tip("Delete all separated stems (frees disk space)").clicked() {
                acts.push(Action::StemClear);
            }
            if retro_btn(ui, "CLOSE", false).clicked() {
                acts.push(Action::ToggleCacheView);
            }
        });
        ui.add_space(10.0);
        let w = ui.available_width() - 20.0;
        let (n, bytes) = self.cache_stats;
        para(ui, "TRACKS ARE SAVED IN THIS FOLDER:", pal().ink2);
        for line in chunk(&cache::dir().display().to_string(), 2.0, w) {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
            ptext(ui.painter(), Pos2::new(rect.min.x + 8.0, rect.center().y), Align::Min, &line, 2.0, pal().ink);
        }
        ui.add_space(8.0);
        para(ui, &format!("{} tracks stored, {} used", n, cache::fmt_size(bytes)), pal().ink);
        para(ui, &format!("Separated stems use {}", cache::fmt_size(stems::stems_size())), pal().ink2);
        para(ui, "A check mark in any list means that track is stored on this computer: it starts instantly and plays with no internet.", pal().ink2);
        para(ui, "Looping and slow-down always play from the stored copy. Loops, speed and seeks stay on this PC - Tidalite never reports plays to Tidal, so your listening statistics are untouched.", pal().ink2);
    }

    fn library_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        window_deco(ui, ui.max_rect(), "LIBRARY");
        logo(ui);
        ui.add_space(4.0);

        // which list: Tidal / my files / YouTube / tunes / diary
        self.section_bar(ui, acts);

        // search row
        if !self.offline {
            ui.horizontal(|ui| {
                let go_w = 54.0;
                let w = (ui.available_width() - go_w - ui.spacing().item_spacing.x).max(80.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::hover());
                self.search_input(ui, rect, acts);
                if retro_btn_w(ui, "GO", go_w, false).clicked() {
                    acts.push(Action::Search(self.search.clone()));
                }
            });
        }

        // navigation row
        if self.offline {
            ui.horizontal(|ui| {
                if retro_btn(ui, "LOG IN TO TIDAL", false).tip("Add your Tidal account - everything else keeps working").clicked()
                {
                    acts.push(Action::Offline(false));
                }
            });
        } else {
            ui.horizontal(|ui| {
                if ibtn(ui, &IC_HOME, "HOME", false).tip("Tidal home").clicked() {
                    acts.push(Action::Home);
                }
                if ibtn(ui, &WIN_LIBRARY, "LIBRARY", false).tip("Your tracks, lists, albums and artists").clicked() {
                    acts.push(Action::Library);
                }
                if !self.back.is_empty() && icon_btn_w(ui, &IC_BACK, false, pal().ink, 38.0).tip("Back").clicked() {
                    acts.push(Action::Back);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if retro_btn(ui, "LOG OUT", false).clicked() {
                        acts.push(Action::Logout);
                    }
                });
            });
        }

        // tools row
        ui.horizontal(|ui| {
            let ink = pal().ink;
            let skin_btn = icon_btn_w(ui, &IC_SKIN, false, ink, 38.0).tip(format!(
                "Skin: {}  (click for the next, right-click to pick one)",
                SKIN_NAMES[SKIN.load(Ordering::Relaxed) % PALS.len()]
            ));
            if skin_btn.clicked() {
                acts.push(Action::Skin);
            }
            skin_btn.context_menu(|ui| skin_menu(ui, acts));
            if icon_btn_w(ui, &IC_EQ, self.show_eq, ink, 38.0).tip("Equalizer").clicked() {
                acts.push(Action::ToggleEq);
            }
            ui.add_space(8.0);
            let sleeping = self.sleep_at.map(|t| t.saturating_duration_since(Instant::now()).as_secs().div_ceil(60));
            let sl_tip = "Sleep timer: click to cycle 15 / 30 / 45 / 60 / 90 / off";
            let sl_clicked = match sleeping {
                Some(mins) => ibtn(ui, &IC_MOON, &format!("{} MIN", mins), true).tip(sl_tip).clicked(),
                None => icon_btn_w(ui, &IC_MOON, false, ink, 38.0).tip(sl_tip).clicked(),
            };
            if sl_clicked {
                acts.push(Action::Sleep);
            }
            if PRACTICE
                && icon_btn_w(ui, &IC_TOMATO, self.pomo > 0 || self.timer_open, ink, 38.0)
                    .tip("Focus timer: work in blocks with rests between")
                    .clicked()
            {
                acts.push(Action::TimerPanel);
            }
            if PRACTICE
                && icon_btn_w(ui, &IC_METRO, self.metro_open, ink, 38.0)
                    .tip("Metronome with beats, subdivisions and a pendulum")
                    .clicked()
            {
                acts.push(Action::MetroPanel);
            }
            ui.add_space(8.0);
            if icon_btn_w(ui, &IC_DISK, self.show_cache, ink, 38.0).tip("Where tracks are stored on this computer").clicked() {
                acts.push(Action::ToggleCacheView);
            }
            if icon_btn_w(ui, &IC_LOG, self.show_log, ink, 38.0).tip("Log").clicked() {
                self.show_log = !self.show_log;
                if self.show_log {
                    self.show_eq = false;
                    self.show_cache = false;
                }
            }
        });

        // library tabs
        let is_lib = self.sec == Sec::Tidal && self.page.as_ref().map(|p| p.library).unwrap_or(false);
        if is_lib && !self.show_log && !self.show_eq && !self.show_cache {
            const TABS: [(Tab, &str); 4] =
                [(Tab::Tracks, "MY TRACKS"), (Tab::Lists, "LISTS"), (Tab::Albums, "ALBUMS"), (Tab::Artists, "ARTISTS")];
            let names: Vec<&str> = TABS.iter().map(|t| t.1).collect();
            let cur = TABS.iter().position(|t| t.0 == self.lib_tab).unwrap_or(0);
            if let Some(i) = tab_row(ui, &names, cur) {
                acts.push(Action::Tab(TABS[i].0));
            }
        }

        // persistent error line
        if self.status_err && !self.status.is_empty() {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::click());
            inset(ui.painter(), rect, pal().lcd);
            marquee(ui, rect, &self.status, 2.0, pal().red);
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                acts.push(Action::ClearStatus);
            }
        }
        self.continue_bar(ui, acts);
        ui.add_space(4.0);

        // list well
        let avail = ui.available_rect_before_wrap();
        if avail.height() < 30.0 {
            return;
        }
        inset(ui.painter(), avail, pal().lcd);
        let area = avail.shrink(thick(3.0));
        let page = self.page.clone();
        let serial = self.serial;
        let loading = self.loading;
        let tab = self.lib_tab;
        let playing_id = self.cur_track().map(|t| t.id);
        let (show_log, show_eq, show_cache) = (self.show_log, self.show_eq, self.show_cache);
        let sec = self.sec;
        let sec_i = sec as u8;
        ui.allocate_ui_at_rect(area, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
            if show_log {
                self.log_view(ui, acts);
                return;
            }
            if show_eq {
                self.eq_view(ui);
                return;
            }
            if show_cache {
                self.cache_view(ui, acts);
                return;
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.push_id((serial, sec_i), |ui| match sec {
                    Sec::Tidal => {
                        if loading {
                            ui.add_space(6.0);
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
                            ptext(
                                ui.painter(),
                                Pos2::new(rect.min.x + 8.0, rect.center().y),
                                Align::Min,
                                "LOADING...",
                                2.0,
                                pal().ink2,
                            );
                        }
                        if let Some(pg) = &page {
                            page_view(ui, &mut self.images, pg, playing_id, tab, &self.liked, acts);
                        }
                    }
                    Sec::Files => self.files_view(ui, acts),
                    Sec::Yt => self.yt_view(ui, acts),
                    Sec::Sc => self.sc_view(ui, acts),
                    Sec::Tunes => self.tunes_view(ui, acts),
                    Sec::Diary => self.diary_view(ui, acts),
                    Sec::Lists => self.playlists_view(ui, acts),
                });
            });
        });
    }

    fn player_window(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        let inner = ui.max_rect();
        window_deco(ui, inner, "PLAYER");
        let s = (inner.width() / 300.0).min(inner.height() / 138.0).max(0.5);
        let ox = inner.min.x + (inner.width() - 300.0 * s) / 2.0;
        let oy = inner.min.y + (inner.height() - 138.0 * s) / 2.0;
        let rc =
            move |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(Pos2::new(ox + x * s, oy + y * s), Vec2::new(w * s, h * s));
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

        // ---- mode switch and window chips on the title line
        {
            let chip_h = 16.0;
            let ty = inner.min.y - 18.0 - chip_h / 2.0;
            let mut right = inner.max.x - 8.0;
            let mut place = |w: f32| {
                let r = Rect::from_min_size(Pos2::new(right - w, ty), Vec2::new(w, chip_h));
                right -= w + 6.0;
                r
            };
            let r_pin = place(28.0);
            let r_mini = place(if self.mini { 36.0 } else { 36.0 });
            let r_prac = place(if PRACTICE { 56.0 } else { 0.0 });
            let r_lis = place(if PRACTICE { 40.0 } else { 0.0 });
            let r_focus = place(if PRACTICE { 48.0 } else { 0.0 });
            if PRACTICE
                && chip(ui, r_focus, "FOCUS", self.focus_mode, "c_focus")
                    .tip("Hide the lists: just the player, your tools and the chart")
                    .clicked()
            {
                acts.push(Action::ToggleFocus);
            }
            if chip(ui, r_pin, "PIN", self.pinned, "c_pin").tip("Keep this window on top").clicked() {
                acts.push(Action::TogglePin);
            }
            if chip(ui, r_mini, if self.mini { "FULL" } else { "MINI" }, self.mini, "c_mini").clicked() {
                acts.push(Action::ToggleMini);
            }
            if PRACTICE
                && chip(ui, r_prac, "PRACTICE", self.practice, "c_prac")
                    .tip("Transcribing: loop a section, slow it down")
                    .clicked()
                && !self.practice
            {
                acts.push(Action::TogglePractice);
            }
            if PRACTICE
                && chip(ui, r_lis, "LISTEN", !self.practice, "c_lis").tip("Just listening: no loop, normal speed").clicked()
                && self.practice
            {
                acts.push(Action::TogglePractice);
            }
        }
        let p = ui.painter();

        // ---- LCD time
        let lcd = rc(6.0, 6.0, 100.0, 40.0);
        inset(p, lcd, pal().lcd);
        let icon: &[&str] = if !active {
            &STOP_S[..]
        } else if self.paused {
            &PAUSE_S[..]
        } else {
            &PLAY_S[..]
        };
        pixmap(p, Pos2::new(snap(ox + 10.0 * s), snap(oy + 11.0 * s)), (0.9 * s).round().max(1.0), icon, pal().ink);
        let blink = !(active && self.paused) || (now_t * 2.0) as i64 % 2 == 0;
        let e = if active { shown.max(0.0) as u32 } else { 0 };
        let (mm, ss) = ((e / 60).min(99), e % 60);
        let digits = [mm / 10, mm % 10, ss / 10, ss % 10];
        let xs = [22.0f32, 38.0, 62.0, 78.0];
        for i in 0..4 {
            seg_digit(p, Pos2::new(ox + xs[i] * s, oy + 14.0 * s), 1.5 * s, digits[i], blink);
        }
        fill_rect(p, rc(54.0, 20.0, 3.0, 3.0), pal().ink);
        fill_rect(p, rc(54.0, 29.0, 3.0, 3.0), pal().ink);

        // ---- spectrum (click: bars / wave / both, right-click: width and sensitivity)
        let sp = rc(6.0, 50.0, 100.0, 28.0);
        inset(p, sp, pal().lcd);
        viz_draw(
            p,
            sp.shrink2(Vec2::new(3.0, 3.0)),
            &self.bands,
            &self.peaks,
            &self.viz_wave,
            self.viz_mode,
            self.viz_w,
            false,
            1.0,
            self.viz_tint(),
        );
        let spr = ui
            .interact(sp, ui.id().with("spectrum"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip("Click: bars / waveform / both.  Right-click: width and sensitivity");
        if spr.clicked() {
            self.viz_mode = (self.viz_mode + 1) % 3;
            self.dirty = true;
        }
        spr.context_menu(|ui| self.viz_menu(ui));

        // ---- title + status
        let tb = rc(112.0, 6.0, 182.0, 14.0);
        inset(p, tb, pal().lcd);
        let title_txt = match &track {
            Some(t) => format!("{} - {}  ({})", t.artist, t.title, fmt_time(t.duration)),
            None => format!("Tidalite {} - a retro player for Tidal", VERSION),
        };
        marquee(ui, tb, &title_txt, px_big, pal().ink);

        let sb = rc(112.0, 22.0, 182.0, 10.0);
        inset(p, sb, pal().lcd);
        let (stxt, scol): (String, Color32) = if !self.status.is_empty() {
            (self.status.clone(), if self.status_err { pal().red } else { pal().ink })
        } else if self.buffering {
            ("BUFFERING...".to_string(), pal().ink2)
        } else if self.loading {
            ("LOADING...".to_string(), pal().ink2)
        } else if let (true, Some(n)) = (active, &next) {
            (format!("NEXT: {} - {}", n.artist, n.title), pal().ink2)
        } else if active {
            ("END OF QUEUE".to_string(), pal().ink2)
        } else {
            (String::new(), pal().ink2)
        };
        marquee(ui, sb, &stxt, px_sm, scol);
        let sr = ui.interact(sb, ui.id().with("status"), Sense::click());
        if sr.clicked() && !self.status.is_empty() {
            acts.push(Action::ClearStatus);
        }

        // ---- kbps / khz / quality
        let kb = rc(112.0, 34.0, 50.0, 12.0);
        inset(p, kb, pal().lcd);
        let kb_txt = if active && self.kbps > 0 { format!("{} KBPS", self.kbps) } else { "--- KBPS".to_string() };
        ptext_fit(p, kb.center(), Align::Center, &kb_txt, px_sm, kb.width() - 6.0, pal().ink);
        let kh = rc(166.0, 34.0, 40.0, 12.0);
        inset(p, kh, pal().lcd);
        let kh_txt = if active && rate > 0 { format!("{} KHZ", rate / 1000) } else { "-- KHZ".to_string() };
        ptext_fit(p, kh.center(), Align::Center, &kh_txt, px_sm, kh.width() - 6.0, pal().ink);
        let qb = rc(210.0, 34.0, 40.0, 12.0);
        let qr = ui.interact(qb, ui.id().with("quality"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        raised_h(p, qb, qr.is_pointer_button_down_on(), qr.hovered());
        ptext_fit(
            p,
            qb.center(),
            Align::Center,
            if self.prefer_lossless { "HIFI" } else { "320K" },
            px_sm,
            qb.width() - 6.0,
            pal().ink,
        );
        if qr.clicked() {
            acts.push(Action::ToggleLossless);
        }

        // ---- cover art
        let cv = rc(258.0, 34.0, 36.0, 36.0);
        inset(p, cv, pal().edge);
        if let Some(t) = &track {
            if !t.cover.is_empty() {
                paint_art_g(ui, &mut self.images, &cover_url(&t.cover, 160), cv.shrink(2.0), 0.0, self.art_gray);
            }
        }
        let cvr = ui.interact(cv, ui.id().with("cover"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        if cvr.clicked() {
            acts.push(Action::ToggleArt);
        }

        // ---- like
        let hb = rc(258.0, 72.0, 36.0, 10.0);
        let is_liked = track.as_ref().map(|t| self.liked.contains(&t.id)).unwrap_or(false);
        // files and YouTube clips are not on Tidal, so they have nothing to like
        let can_like = true;
        if can_like {
            let hr = ui.interact(hb, ui.id().with("heart"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            raised_h(p, hb, hr.is_pointer_button_down_on(), hr.hovered());
            let hpx = (0.9 * s).round().max(1.0);
            pixmap(
                p,
                Pos2::new(snap(hb.center().x - 3.5 * hpx), snap(hb.center().y - 3.0 * hpx)),
                hpx,
                &HEART[..],
                if is_liked { pal().red } else { pal().beige_dk },
            );
            if hr.clicked() {
                if let Some(t) = &track {
                    acts.push(Action::ToggleLike(t.clone()));
                }
            }
        }

        // ---- volume
        let vg = rc(112.0, 54.0, 138.0, 7.0);
        for i in 0..28 {
            let h = 1.0 + i as f32 * 0.09;
            fill_rect(p, rc(113.0 + i as f32 * 4.85, 52.0 - h, 1.0, h), pal().ink2);
        }
        inset(p, vg, pal().groove);
        let thumb_v = 9.0 * s;
        let vx = vg.min.x + (vg.width() - thumb_v) * self.volume;
        fill_rect(p, Rect::from_min_max(vg.min + Vec2::new(1.5, 1.5), Pos2::new(vx + thumb_v / 2.0, vg.max.y - 1.0)), pal().ink2);
        let vthumb = Rect::from_min_size(Pos2::new(vx, oy + 50.0 * s), Vec2::new(thumb_v, 15.0 * s));
        raised(p, vthumb, false);
        let vr = ui.interact(rc(112.0, 48.0, 138.0, 18.0), ui.id().with("vol"), Sense::click_and_drag());
        if vr.dragged() || vr.clicked() {
            if let Some(pp) = vr.interact_pointer_pos() {
                let v = ((pp.x - vg.min.x - thumb_v / 2.0) / (vg.width() - thumb_v)).clamp(0.0, 1.0);
                acts.push(Action::Volume(v));
            }
        }
        // speaker icon (click to mute / unmute)
        let spk_r = rc(112.0, 67.0, 16.0, 12.0);
        let spk_icon: &[&str] = if self.volume <= 0.0 {
            &IC_SPK0
        } else if self.volume < 0.5 {
            &IC_SPK1
        } else {
            &IC_SPK2
        };
        let k = (spk_r.height() / spk_icon.len() as f32).floor().max(1.0);
        pixmap(p, spk_r.min, k, spk_icon, pal().ink2);
        let spk_hit = ui
            .interact(spk_r.expand(3.0), ui.id().with("mute"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip(if self.volume <= 0.0 { "Unmute" } else { "Mute" });
        if spk_hit.clicked() {
            if self.volume > 0.0 {
                self.vol_before = self.volume;
                acts.push(Action::Volume(0.0));
            } else {
                acts.push(Action::Volume(self.vol_before.max(0.3)));
            }
        }
        ptext(
            p,
            Pos2::new(ox + 250.0 * s, oy + 71.0 * s),
            Align::Max,
            &format!("{}%", (self.volume * 100.0).round() as u32),
            px_sm,
            pal().ink2,
        );

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
        if skr.secondary_clicked() && dur > 0.0 {
            if let Some(pp) = skr.interact_pointer_pos() {
                self.menu_t = ((pp.x - sk.min.x - thumb_w / 2.0) / (sk.width() - thumb_w)).clamp(0.0, 1.0) * dur;
            }
        }
        if active && dur > 0.0 {
            let (mt, both, lo, pr) = (self.menu_t, self.loop_a.is_some() && self.loop_b.is_some(), self.loop_on, self.practice);
            skr.context_menu(|ui| loop_menu(ui, acts, mt, both, lo, pr));
        }
        let frac = if active && dur > 0.0 { (shown / dur).clamp(0.0, 1.0) } else { 0.0 };
        inset(p, sk, pal().groove);
        let tx = sk.min.x + (sk.width() - thumb_w) * frac;
        fill_rect(p, Rect::from_min_max(sk.min + Vec2::new(1.5, 1.5), Pos2::new(tx + thumb_w / 2.0, sk.max.y - 1.0)), pal().ink2);
        if active && dur > 0.0 && self.practice {
            let at = |t: f32| sk.min.x + thumb_w / 2.0 + (sk.width() - thumb_w) * (t / dur).clamp(0.0, 1.0);
            if let (Some(a), Some(b)) = (self.loop_a, self.loop_b) {
                let col = if self.loop_on { pal().red.gamma_multiply(0.5) } else { pal().ink.gamma_multiply(0.3) };
                fill_rect(p, Rect::from_min_max(Pos2::new(at(a), sk.min.y + 1.5), Pos2::new(at(b), sk.max.y - 1.0)), col);
            }
            for (v, lab) in [(self.loop_a, "A"), (self.loop_b, "B")] {
                if let Some(t) = v {
                    let x = at(t);
                    fill_rect(
                        p,
                        Rect::from_min_max(Pos2::new(x - 1.0, sk.min.y - 3.0 * s), Pos2::new(x + 1.0, sk.max.y + 1.0)),
                        pal().red,
                    );
                    ptext(p, Pos2::new(x + 3.0, sk.min.y - 5.0 * s), Align::Min, lab, px_sm, pal().red);
                }
            }
        }
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

        // ---- shuffle / repeat: icons, the names show on hover
        let sh = rc(136.0, 102.0, 36.0, 16.0);
        let shr = ui
            .interact(sh, ui.id().with("shuf"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip(if self.shuffle { "Shuffle: on" } else { "Shuffle" });
        raised_h(p, sh, self.shuffle || shr.is_pointer_button_down_on(), shr.hovered() && !self.shuffle);
        icon_in(p, sh, &IC_SHUF, if self.shuffle { pal().red } else { pal().ink });
        if shr.clicked() {
            acts.push(Action::Shuffle);
        }
        let rp = rc(178.0, 102.0, 36.0, 16.0);
        let rep_tip = match self.repeat {
            Repeat::Off => "Repeat: off",
            Repeat::All => "Repeat: all",
            Repeat::One => "Repeat: this track",
        };
        let rpr =
            ui.interact(rp, ui.id().with("rep"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).tip(rep_tip);
        raised_h(
            p,
            rp,
            self.repeat != Repeat::Off || rpr.is_pointer_button_down_on(),
            rpr.hovered() && self.repeat == Repeat::Off,
        );
        let rep_icon: &[&str] = if self.repeat == Repeat::One { &IC_REP1 } else { &IC_REP };
        icon_in(p, rp, rep_icon, if self.repeat != Repeat::Off { pal().red } else { pal().ink });
        if rpr.clicked() {
            acts.push(Action::Repeat);
        }

        // ---- where is this track stored?
        let strip = rc(6.0, 124.0, 288.0, 12.0);
        let strip_r = ui
            .interact(strip, ui.id().with("strip"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip("Click to open the folder where tracks are stored on this computer");
        inset(p, strip, pal().lcd);
        let (dtxt, dcol): (String, Color32) = match &track {
            None => (format!("TRACKS ARE SAVED TO: {}", cache::dir().display()), pal().ink2),
            Some(t) => {
                if let Some(Src::File(fp)) = self.srcmap.get(&t.id) {
                    (format!("LOCAL FILE: {}", fp.display()), pal().ink)
                } else if cache::has(t.id) {
                    let path = cache::path_for(t.id).unwrap_or_else(cache::dir);
                    (format!("STORED ON THIS PC: {}", path.display()), pal().ink)
                } else if self.buffering {
                    (format!("DOWNLOADING TO: {}", cache::dir().display()), pal().ink)
                } else if !self.keep_cache {
                    ("NOT STORED - SAVING IS OFF (STREAM ONLY)".to_string(), pal().ink2)
                } else {
                    ("NOT STORED ON THIS PC YET".to_string(), pal().ink2)
                }
            }
        };
        marquee(ui, strip, &dtxt, px_sm, dcol);
        if strip_r.clicked() {
            acts.push(Action::OpenCache);
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
        let area = Rect::from_min_max(full.min + Vec2::new(28.0, 28.0), Pos2::new(full.max.x - 28.0, full.max.y - bar_h - 12.0));
        let lyrics_on = self.show_lyrics && area.width() > 640.0;
        let art_zone = if lyrics_on {
            Rect::from_min_max(area.min, Pos2::new(area.min.x + area.width() * 0.5 - 10.0, area.max.y))
        } else {
            area
        };
        let caption_h = 64.0;
        let a = (art_zone.width().min(art_zone.height() - caption_h) * 0.82).max(120.0);
        let center = Pos2::new(art_zone.center().x, art_zone.min.y + (art_zone.height() - caption_h) / 2.0);

        // ---- background: the cover's average colour as a soft gradient (never a stretched picture)
        let url = track.as_ref().map(|t| cover_url(&t.cover, 640)).unwrap_or_default();
        let tex = art_tex(&mut self.images, &url, self.art_gray);
        let avg = self
            .images
            .avg
            .get(&format!("gray:{}", url))
            .filter(|_| self.art_gray)
            .or_else(|| self.images.avg.get(&url))
            .copied();
        if let Some([top, bot]) = avg {
            let shade = |c: Color32, k: f32| {
                Color32::from_rgb((c.r() as f32 * k) as u8, (c.g() as f32 * k) as u8, (c.b() as f32 * k) as u8)
            };
            let (ct, cb) = (shade(top, 0.62), shade(bot, 0.2));
            let mut mesh = egui::Mesh::default();
            let uv = egui::epaint::WHITE_UV;
            for (pos, color) in
                [(full.left_top(), ct), (full.right_top(), ct), (full.right_bottom(), cb), (full.left_bottom(), cb)]
            {
                mesh.vertices.push(egui::epaint::Vertex { pos, uv, color });
            }
            mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
            p.add(egui::Shape::mesh(mesh));
        }

        // ---- visualizer, soft and behind everything (only under the cover when lyrics split the view)
        if self.show_spec {
            let zone = if lyrics_on { Rect::from_min_max(full.min, Pos2::new(art_zone.max.x + 14.0, full.max.y)) } else { full };
            // WIDTH = how much of the zone the visualizer spans, centred (the zone is the cover's side when lyrics are up)
            let half = (zone.width() * self.spec_w / 2.0).max(40.0);
            let mid = zone.center().x;
            let r = Rect::from_min_max(
                Pos2::new((mid - half).max(zone.min.x + 6.0), zone.max.y - zone.height() * self.spec_h),
                Pos2::new((mid + half).min(zone.max.x - 6.0), zone.max.y),
            );
            viz_draw(
                &p,
                r,
                &self.bands,
                &self.peaks,
                &self.viz_wave,
                self.viz_mode,
                self.viz_w,
                true,
                self.spec_op,
                self.viz_tint(),
            );
            if self.cur.is_some() && !self.paused && !self.stopped {
                ui.ctx().request_repaint();
            }
        }

        // ---- mild tilt, only while the mouse is on or right next to the cover
        let reach = a / 2.0 * 1.35;
        let target = match hover {
            Some(pp) if (pp.x - center.x).abs() < reach && (pp.y - center.y).abs() < reach => {
                Vec2::new(((pp.x - center.x) / reach).clamp(-1.0, 1.0), ((pp.y - center.y) / reach).clamp(-1.0, 1.0)) * 0.45
            }
            _ => Vec2::ZERO,
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
        p.add(egui::Shape::convex_polygon(corners(h + 9.0).to_vec(), Color32::BLACK, Stroke::new(2.0, pal().trim)));
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
                p.add(egui::Shape::convex_polygon(cs.to_vec(), pal().ink2, Stroke::NONE));
            }
        }

        // ---- caption
        if let Some(tr) = &track {
            let cx = art_zone.center().x;
            let y0 = art_zone.max.y - caption_h + 14.0;
            let w = art_zone.width() - 20.0;
            ptext_fit(&p, Pos2::new(cx, y0), Align::Center, &tr.title, 3.0, w, pal().bar_txt);
            ptext_fit(&p, Pos2::new(cx, y0 + 30.0), Align::Center, &format!("{} - {}", tr.artist, tr.album), 2.0, w, pal().trim);
        } else {
            ptext(&p, art_zone.center(), Align::Center, "Nothing playing", 3.0, pal().trim);
        }

        // ---- lyrics
        if lyrics_on {
            let ly = Rect::from_min_max(Pos2::new(area.min.x + area.width() * 0.5 + 10.0, area.min.y), area.max);
            p.rect_filled(ly, Rounding::same(0.0), Color32::from_black_alpha(150));
            p.rect_stroke(ly, Rounding::same(0.0), Stroke::new(2.0, pal().trim));
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
                        let idx: Option<usize> =
                            if l.synced { l.lines.iter().rposition(|(tt, _)| *tt <= pos + 0.25) } else { None };
                        // scrolling takes over for a few seconds; otherwise the view follows the song
                        self.lyric_free = (self.lyric_free - dt).max(0.0);
                        if over && wheel.abs() > 0.0 {
                            scroll = (scroll - wheel / lh).clamp(0.0, (n.max(1) - 1) as f32);
                            self.lyric_free = 4.0;
                        } else if l.synced && self.lyric_free <= 0.0 {
                            let tgt = idx.unwrap_or(0) as f32;
                            scroll += (tgt - scroll) * (1.0 - (-dt * 7.0).exp());
                        }
                        // the line under the mouse (synced lyrics): click it to jump there
                        let mut hover_i: Option<usize> = None;
                        if l.synced && over {
                            if let Some(pp) = hover {
                                let k = (scroll + (pp.y - cyl) / lh).round();
                                if k >= 0.0 && (k as usize) < n && !l.lines[k as usize].1.is_empty() {
                                    hover_i = Some(k as usize);
                                }
                            }
                        }
                        let lr = ui.interact(ly, ui.id().with("lyric_click"), Sense::click());
                        if hover_i.is_some() {
                            lr.clone().on_hover_cursor(egui::CursorIcon::PointingHand);
                        }
                        if lr.clicked() {
                            if let Some(i) = hover_i {
                                acts.push(Action::Seek(l.lines[i].0));
                                self.lyric_free = 0.0;
                            }
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
                            let (px, col) = if idx == Some(i) {
                                (3.0, pal().bar_txt)
                            } else if hover_i == Some(i) {
                                (2.0, pal().bar_txt)
                            } else {
                                (2.0, pal().trim.gamma_multiply(fade))
                            };
                            ptext_fit(&cp, Pos2::new(ly.center().x, y), Align::Center, txt, px, ly.width() - 40.0, col);
                        }
                    }
                }
            }
            if let Some(m) = msg {
                ptext_fit(&cp, ly.center(), Align::Center, m, 2.0, ly.width() - 30.0, pal().trim);
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
        inset(&p, sb, pal().groove);
        let frac = if active && dur > 0.0 { (pos / dur).clamp(0.0, 1.0) } else { 0.0 };
        fill_rect(
            &p,
            Rect::from_min_max(
                sb.min + Vec2::new(2.0, 2.0),
                Pos2::new(sb.min.x + 2.0 + (sb.width() - 4.0) * frac, sb.max.y - 2.0),
            ),
            pal().bar_txt,
        );
        // saved loops and the live A-B loop, laid over the bar so you can see where the work is (practice mode only)
        if PRACTICE && self.practice && active && dur > 0.0 {
            let x_at = |t: f32| sb.min.x + 2.0 + (sb.width() - 4.0) * (t / dur).clamp(0.0, 1.0);
            let secs: Vec<(String, f32, f32)> = self
                .cur_track()
                .and_then(|t| self.store.sections.get(&t.id))
                .map(|l| l.iter().map(|s| (s.name.clone(), s.a, s.b)).collect())
                .unwrap_or_default();
            let mut last_label_x = f32::MIN;
            for (name, a, b) in &secs {
                let band = Rect::from_min_max(
                    Pos2::new(x_at(*a), sb.min.y - 7.0),
                    Pos2::new(x_at(*b).max(x_at(*a) + 3.0), sb.min.y - 2.0),
                );
                fill_rect(&p, band, pal().ink2);
                if band.min.x - last_label_x > 40.0 {
                    ptext_fit(&p, Pos2::new(band.min.x, sb.min.y - 16.0), Align::Min, name, 1.0, 120.0, pal().ink2);
                    last_label_x = band.min.x;
                }
            }
            let on = self.loop_on;
            let flag = |x: f32, lab: &str| {
                fill_rect(
                    &p,
                    Rect::from_min_max(Pos2::new(x - 1.5, sb.min.y - 4.0), Pos2::new(x + 1.5, sb.max.y + 4.0)),
                    pal().red,
                );
                if x - sb.min.x > 44.0 && sb.max.x - x > 44.0 {
                    ptext(&p, Pos2::new(x, sb.max.y + 14.0), Align::Center, lab, 2.0, pal().red);
                }
            };
            if let (Some(a), Some(b)) = (self.loop_a, self.loop_b) {
                let r = Rect::from_min_max(Pos2::new(x_at(a), sb.min.y), Pos2::new(x_at(b).max(x_at(a) + 3.0), sb.max.y));
                fill_rect(&p, r, pal().red.gamma_multiply(if on { 0.7 } else { 0.4 }));
                flag(r.min.x, "A");
                flag(r.max.x, "B");
            } else if let Some(a) = self.loop_a {
                flag(x_at(a), "A");
            }
        }
        ptext(&p, Pos2::new(sb.min.x, sb.max.y + 14.0), Align::Min, &fmt_time(if active { pos } else { 0.0 }), 2.0, pal().trim);
        ptext(&p, Pos2::new(sb.max.x, sb.max.y + 14.0), Align::Max, &fmt_time(dur), 2.0, pal().trim);

        // ---- buttons
        let row = Rect::from_min_size(Pos2::new(full.min.x + 28.0, full.max.y - 52.0), Vec2::new(full.width() - 56.0, BTN_H));
        let (gray, lyr, fs) = (self.art_gray, self.show_lyrics, self.fullscreen);
        let cur_t = self.cur_track();
        let liked_now = cur_t.as_ref().map(|t| self.liked.contains(&t.id)).unwrap_or(false);
        let playing = active && !self.paused;
        ui.allocate_ui_at_rect(row, |ui| {
            ui.horizontal(|ui| {
                let ink = pal().ink;
                if icon_btn(ui, &IC_PREV, false, ink).tip("Previous").clicked() {
                    acts.push(Action::Prev);
                }
                if icon_btn(ui, if playing { &IC_PAUSE } else { &IC_PLAY }, false, ink)
                    .tip(if playing { "Pause  (Space)" } else { "Play  (Space)" })
                    .clicked()
                {
                    acts.push(Action::Toggle);
                }
                if icon_btn(ui, &IC_NEXT, false, ink).tip("Next").clicked() {
                    acts.push(Action::Next);
                }
                ui.add_space(14.0);
                if icon_btn(ui, &IC_SHUF, self.shuffle, ink)
                    .tip(if self.shuffle { "Shuffle is on - click to turn off" } else { "Shuffle the queue" })
                    .clicked()
                {
                    acts.push(Action::Shuffle);
                }
                let (rep_icon, rep_tip) = match self.repeat {
                    Repeat::Off => (&IC_REP, "Repeat: off  (click for all)"),
                    Repeat::All => (&IC_REP, "Repeat all  (click for one track)"),
                    Repeat::One => (&IC_REP1, "Repeat this track  (click to turn off)"),
                };
                if icon_btn(ui, rep_icon, self.repeat != Repeat::Off, ink).tip(rep_tip).clicked() {
                    acts.push(Action::Repeat);
                }
                ui.add_space(14.0);
                if icon_btn(ui, &IC_BW, gray, ink)
                    .tip(if gray { "Back to color art  (G)" } else { "Black and white art  (G)" })
                    .clicked()
                {
                    acts.push(Action::ToggleGray);
                }
                let sp = icon_btn(ui, &IC_SPEC, self.show_spec, ink)
                    .tip("Spectrum behind the cover  (right-click for size and opacity)");
                if sp.clicked() {
                    acts.push(Action::ToggleSpec);
                }
                sp.context_menu(|ui| {
                    let mut step = |ui: &mut egui::Ui, name: &str, v: &mut f32, d: f32, lo: f32, hi: f32| {
                        if menu_item(ui, &format!("{}  {}%   (+)", name, (*v * 100.0).round() as i32)) {
                            *v = (*v + d).min(hi);
                        }
                        if menu_item(ui, &format!("{}  {}%   (-)", name, (*v * 100.0).round() as i32)) {
                            *v = (*v - d).max(lo);
                        }
                    };
                    step(ui, "OPACITY", &mut self.spec_op, 0.05, 0.05, 0.8);
                    step(ui, "HEIGHT", &mut self.spec_h, 0.08, 0.15, 0.9);
                    step(ui, "WIDTH", &mut self.spec_w, 0.1, 0.2, 1.0);
                    self.viz_menu(ui);
                    self.dirty = true;
                });
                let skin_b = icon_btn(ui, &IC_SKIN, false, ink).tip(format!(
                    "Skin: {}  (click for the next, right-click to pick one)",
                    SKIN_NAMES[SKIN.load(Ordering::Relaxed) % PALS.len()]
                ));
                if skin_b.clicked() {
                    acts.push(Action::Skin);
                }
                skin_b.context_menu(|ui| skin_menu(ui, acts));
                if icon_btn(ui, &IC_MIC, lyr, ink).tip("Lyrics  (L)").clicked() {
                    acts.push(Action::ToggleLyrics);
                }
                if icon_btn(ui, if fs { &IC_WIN } else { &IC_FULL }, false, ink)
                    .tip(if fs { "Leave fullscreen  (F)" } else { "Fullscreen  (F)" })
                    .clicked()
                {
                    acts.push(Action::ToggleFullscreen);
                }
                // files and YouTube clips are not on Tidal, so there is nothing to like
                if icon_btn(ui, &IC_HEART, liked_now, if liked_now { pal().red } else { ink })
                    .tip(if liked_now { "Remove from My Tracks  (H)" } else { "Add to My Tracks  (H)" })
                    .clicked()
                {
                    if let Some(t) = &cur_t {
                        acts.push(Action::ToggleLike(t.clone()));
                    }
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
            "Esc close    F fullscreen    G b&w    L lyrics    H like    Space play/pause",
            1.0,
            pal().dim,
        );
    }

    fn playlist_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        let title = if self.rtab == 1 {
            "LEAD SHEET"
        } else if self.shuffle {
            "QUEUE - SHUFFLED"
        } else {
            "QUEUE"
        };
        window_deco(ui, ui.max_rect(), title);
        if self.right_header(ui, acts) {
            return;
        }
        let queue = self.queue.clone();
        let cur = self.cur;
        let full = ui.available_rect_before_wrap();
        let foot_h = 36.0;
        if full.height() < foot_h + 40.0 {
            return;
        }

        // ---- list well
        let well = Rect::from_min_max(full.min, Pos2::new(full.max.x, full.max.y - foot_h));
        inset(ui.painter(), well, pal().lcd);
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
                    pal().ink2,
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
                let mut rects: Vec<(usize, Rect)> = Vec::new();
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
                                true,
                                cache::has(t.id),
                            );
                            if let Some(r) = r {
                                rects.push((i, r.rect));
                                if r.drag_started() {
                                    self.q_drag = Some(i);
                                }
                                if r.clicked() {
                                    acts.push(Action::PlayIndex(i));
                                }
                                let lk = if self.liked.contains(&t.id) { "Remove from My Tracks" } else { "Add to My Tracks" };
                                let link = track_link(&t, self.ext_of(t.id).as_ref());
                                r.context_menu(|ui| {
                                    if menu_item(ui, lk) {
                                        acts.push(Action::ToggleLike(t.clone()));
                                        ui.close_menu();
                                    }
                                    if menu_item(ui, "Play now") {
                                        acts.push(Action::PlayIndex(i));
                                        ui.close_menu();
                                    }
                                    if PRACTICE && menu_item(ui, "Add to a tune...") {
                                        acts.push(Action::PickTune(t.clone()));
                                        ui.close_menu();
                                    }
                                    if menu_item(ui, "Add to a playlist...") {
                                        acts.push(Action::PlaylistPick(t.clone()));
                                        ui.close_menu();
                                    }
                                    link_item(ui, acts, link.clone());
                                    if cur != Some(i) && menu_item(ui, "Remove from queue") {
                                        acts.push(Action::Remove(i));
                                        ui.close_menu();
                                    }
                                });
                            }
                        }
                    }
                }
                // drag-to-reorder: insertion line while dragging, move on release
                if let Some(from) = self.q_drag {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    let released = ui.input(|i| i.pointer.any_released());
                    if let Some(pp) = ui.input(|i| i.pointer.hover_pos()) {
                        let mut ins = None;
                        for (i, rc) in &rects {
                            if pp.y < rc.center().y {
                                ins = Some((*i, rc.min.y));
                                break;
                            }
                        }
                        let (ins_i, line_y) =
                            ins.or_else(|| rects.last().map(|(i, rc)| (*i + 1, rc.max.y))).unwrap_or((queue.len(), area.min.y));
                        fill_rect(
                            ui.painter(),
                            Rect::from_min_size(Pos2::new(area.min.x, line_y - 1.0), Vec2::new(area.width(), 2.0)),
                            pal().red,
                        );
                        if released {
                            acts.push(Action::MoveQueue(from, ins_i));
                        }
                    }
                    if released || !ui.input(|i| i.pointer.any_down()) {
                        self.q_drag = None;
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
                inset(ui.painter(), rect, pal().lcd);
                let n = queue.len();
                ptext_fit(
                    ui.painter(),
                    rect.center(),
                    Align::Center,
                    &format!("{} / {}    {} {}", fmt_long(elapsed), fmt_long(total), n, if n == 1 { "track" } else { "tracks" }),
                    2.0,
                    rect.width() - 16.0,
                    pal().ink,
                );
            });
        });
    }
}

// ------------------------------------------------------------------- update
fn panel_frame() -> egui::Frame {
    egui::Frame::none().fill(pal().app_bg).inner_margin(egui::Margin { left: 14.0, right: 14.0, top: 30.0, bottom: 14.0 })
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        font::set_ppp(ctx.pixels_per_point());
        font::set_modern(style() != 0);
        // the window's scale and size settle over the first frames: keep drawing until they do
        {
            let (ppp, sz) = (ctx.pixels_per_point(), ctx.screen_rect().size());
            if (ppp - self.last_ppp).abs() > 0.001 || sz != self.last_size || self.frames < 10 {
                self.last_ppp = ppp;
                self.last_size = sz;
                self.frames += 1;
                ctx.request_repaint();
            }
        }
        self.drain(ctx);
        let mut acts: Vec<Action> = Vec::new();

        // files / folders dropped onto the window
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if !dropped.is_empty() {
            self.handle_paths(dropped);
        }
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());

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
                if e.starts_with("DECODE") {
                    // a damaged stored copy must not keep failing: forget it so it is fetched again
                    if let Some(t) = self.cur_track() {
                        cache::remove(t.id);
                    }
                }
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
        self.sleep_tick();
        self.tick_practice();
        let mut title = match self.cur_track() {
            Some(t) if !self.stopped => format!("{} - {}  |  Tidalite", t.artist, t.title),
            _ => format!("Tidalite {}", VERSION),
        };
        if let (true, Some(end)) = (self.pomo > 0, self.pomo_end) {
            let left = end.saturating_duration_since(Instant::now()).as_secs();
            title = format!("{}:{:02} {}  |  {}", left / 60, left % 60, if self.pomo == 1 { "FOCUS" } else { "REST" }, title);
        } else if self.pomo == 3 {
            title = format!("REST OVER - NEXT FOCUS  |  {}", title);
        }
        if self.pomo_end.is_some() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
        if self.dirty && self.last_save.elapsed() > Duration::from_secs(1) {
            self.save_settings();
        }

        if self.auth != Auth::In && !self.offline {
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(pal().app_bg))
                .show(ctx, |ui| self.login_ui(ui, &mut acts));
        } else {
            // Keyboard: Space = play/pause, arrows = seek, classic Winamp keys Z X C V B,
            // A = art viewer, G = black & white, L = lyrics, F / F11 = fullscreen, Esc = back.
            if !self.search_focus && self.ed.id == 0 {
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
                if kp(egui::Key::H) {
                    if let Some(t) = self.cur_track() {
                        acts.push(Action::ToggleLike(t));
                    }
                }
                if kp(egui::Key::M) {
                    acts.push(Action::ToggleMini);
                }
                if kp(egui::Key::P) {
                    acts.push(Action::TogglePractice);
                }
                if kp(egui::Key::OpenBracket) {
                    acts.push(Action::SetAAt(self.pos()));
                }
                if kp(egui::Key::CloseBracket) {
                    acts.push(Action::SetBAt(self.pos()));
                }
                if kp(egui::Key::Backslash) {
                    acts.push(Action::LoopToggle);
                }
                if kp(egui::Key::Comma) {
                    acts.push(Action::SeekRel(-2.0));
                }
                if kp(egui::Key::Period) {
                    acts.push(Action::Seek(self.loop_a.unwrap_or(0.0)));
                }
                if self.practice && kp(egui::Key::ArrowUp) {
                    acts.push(Action::Speed(self.speed + 5));
                }
                if self.practice && kp(egui::Key::ArrowDown) {
                    acts.push(Action::Speed(self.speed.saturating_sub(5)));
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

            if self.mini {
                egui::CentralPanel::default().frame(panel_frame()).show(ctx, |ui| self.player_window(ui, &mut acts));
            } else if self.art_view {
                egui::CentralPanel::default()
                    .frame(egui::Frame::none().fill(pal().app_bg))
                    .show(ctx, |ui| self.art_ui(ui, &mut acts));
            } else {
                let screen = ctx.screen_rect();
                // library width is a share of the window, so it scales with it
                let lib_w = if self.focus_mode {
                    0.0
                } else {
                    (self.lib_frac * screen.width()).clamp(280.0, (screen.width() - 560.0).max(280.0))
                };
                let right_w = screen.width() - lib_w;
                let inner_w = right_w - 28.0;
                let practice_h = if !self.practice { 0.0 } else { 262.0 };
                let stack_max = self.stack_frac * screen.height();
                let max_inner_h = ((screen.height() - practice_h) * 0.5 - 44.0).max(120.0);
                let drag_inner_h = (stack_max - practice_h - 44.0).max(110.0);
                let s = (inner_w / 300.0).min(max_inner_h / 138.0).min(drag_inner_h / 138.0).clamp(0.8, 3.0);
                let player_h = 138.0 * s + 44.0;

                if !self.focus_mode {
                    egui::SidePanel::left("library")
                        .exact_width(lib_w)
                        .resizable(false)
                        .show_separator_line(false)
                        .frame(panel_frame())
                        .show(ctx, |ui| self.library_ui(ui, &mut acts));
                }
                egui::TopBottomPanel::top("player")
                    .exact_height(player_h)
                    .resizable(false)
                    .show_separator_line(false)
                    .frame(panel_frame())
                    .show(ctx, |ui| self.player_window(ui, &mut acts));
                if self.practice {
                    egui::TopBottomPanel::top("practice")
                        .exact_height(practice_h)
                        .resizable(false)
                        .show_separator_line(false)
                        .frame(panel_frame())
                        .show(ctx, |ui| self.practice_ui(ui, &mut acts));
                }
                egui::CentralPanel::default().frame(panel_frame()).show(ctx, |ui| self.playlist_ui(ui, &mut acts));

                // drag handles between the windows
                if !self.focus_mode {
                    let r = Rect::from_min_size(Pos2::new(lib_w - 7.0, screen.min.y), Vec2::new(14.0, screen.height()));
                    match splitter(ctx, "split_v", r, true) {
                        Some(Some(x)) => {
                            self.lib_frac = (x / screen.width()).clamp(0.2, 0.7);
                            self.dirty = true;
                        }
                        Some(None) => self.lib_frac = 0.34,
                        None => {}
                    }
                }
                let y = player_h + practice_h;
                let r = Rect::from_min_size(Pos2::new(lib_w, y - 7.0), Vec2::new(right_w, 14.0));
                match splitter(ctx, "split_h", r, false) {
                    Some(Some(y)) => {
                        self.stack_frac = (y / screen.height()).clamp(0.3, 1.0);
                        self.dirty = true;
                    }
                    Some(None) => self.stack_frac = 1.0,
                    None => {}
                }
            }
        }

        if hovering {
            let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop_hint"));
            let p = ctx.layer_painter(layer);
            let r = ctx.screen_rect();
            p.rect_filled(r, Rounding::same(0.0), Color32::from_black_alpha(170));
            ptext(&p, r.center(), Align::Center, "DROP AUDIO FILES OR FOLDERS TO ADD THEM", 3.0, pal().bar_txt);
        }
        self.floating_tools(ctx, &mut acts);
        self.update_banner(ctx, &mut acts);
        for a in acts {
            self.apply(a);
        }
        let busy = self.cur.is_some() && !self.stopped && !self.paused;
        ctx.request_repaint_after(Duration::from_millis(if self.art_view {
            16
        } else if busy || self.search_focus || self.band_on || self.mt.on {
            33
        } else {
            250
        }));
    }
}

/// A drag handle drawn in the gap between two windows. Some(Some(pos)) while dragging, Some(None) on double click.
fn splitter(ctx: &egui::Context, id: &str, rect: Rect, vertical: bool) -> Option<Option<f32>> {
    let mut out = None;
    egui::Area::new(egui::Id::new(id)).order(egui::Order::Foreground).fixed_pos(rect.min).show(ctx, |ui| {
        let (r, resp) = ui.allocate_exact_size(rect.size(), Sense::click_and_drag());
        let hot = resp.hovered() || resp.dragged();
        let p = ui.painter();
        let t = thick(2.0);
        let (line, grip) = if vertical {
            (Rect::from_center_size(r.center(), Vec2::new(t * 2.0, r.height())), Vec2::new(t * 3.0, t * 6.0))
        } else {
            (Rect::from_center_size(r.center(), Vec2::new(r.width(), t * 2.0)), Vec2::new(t * 6.0, t * 3.0))
        };
        if hot {
            fill_rect(p, line, pal().trim);
            let g = Rect::from_center_size(r.center(), grip + if vertical { Vec2::new(t, 0.0) } else { Vec2::new(0.0, t) });
            fill_rect(p, g.expand(t), Color32::BLACK);
            fill_rect(p, g, pal().beige_lt);
            let step = if vertical { Vec2::new(0.0, t * 2.0) } else { Vec2::new(t * 2.0, 0.0) };
            for k in [-1.0f32, 0.0, 1.0] {
                let d = Rect::from_center_size(r.center() + step * k, Vec2::splat(t));
                fill_rect(p, d, pal().edge);
            }
        }
        if resp.dragged() {
            if let Some(pp) = resp.interact_pointer_pos() {
                out = Some(Some(if vertical { pp.x } else { pp.y }));
            }
        }
        if resp.double_clicked() {
            out = Some(None);
        }
        if hot {
            ui.ctx().set_cursor_icon(if vertical {
                egui::CursorIcon::ResizeHorizontal
            } else {
                egui::CursorIcon::ResizeVertical
            });
        }
    });
    out
}

fn setup_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::light();
    v.panel_fill = pal().app_bg;
    v.window_fill = pal().beige;
    v.window_stroke = Stroke::new(1.0, pal().edge);
    let rad = if style() != 0 { 7.0 } else { 0.0 };
    v.window_rounding = Rounding::same(rad);
    v.menu_rounding = Rounding::same(rad);
    v.extreme_bg_color = pal().lcd;
    v.faint_bg_color = pal().beige_dk;
    v.hyperlink_color = pal().ink;
    v.selection.bg_fill = pal().sel;
    v.selection.stroke = Stroke::new(1.0, pal().edge);
    v.widgets.noninteractive.bg_stroke = Stroke::NONE;
    v.widgets.noninteractive.bg_fill = pal().beige;
    v.widgets.noninteractive.weak_bg_fill = pal().beige;
    v.widgets.inactive.bg_fill = pal().beige_dk;
    v.widgets.inactive.weak_bg_fill = pal().beige_dk;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, pal().edge);
    v.widgets.hovered.bg_fill = pal().groove;
    v.widgets.hovered.weak_bg_fill = pal().groove;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, pal().edge);
    v.widgets.active.bg_fill = pal().ink2;
    v.widgets.active.weak_bg_fill = pal().ink2;
    v.widgets.active.bg_stroke = Stroke::new(1.0, pal().edge);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.fg_stroke = Stroke::new(1.0, pal().ink);
        w.rounding = Rounding::same(rad * 0.7);
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
    // an update downloaded last time is installed before anything opens
    if update::apply_staged_at_start() {
        return Ok(());
    }
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
