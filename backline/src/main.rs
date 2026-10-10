#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]
#![recursion_limit = "256"]

// Folders: audio/ = playback and sound analysis, data/ = Tidal, files, library and updates, ui/ = everything drawn.
// The modules keep flat names (crate::player, crate::views...) so the folders are purely for reading.

mod extras;

#[path = "audio/band.rs"]
mod band;
#[path = "audio/decode.rs"]
mod decode;
#[path = "audio/intro.rs"]
mod intro;
#[path = "audio/media.rs"]
mod media;
#[path = "audio/player.rs"]
mod player;
#[path = "audio/stems.rs"]
mod stems;
#[path = "audio/tour_song.rs"]
mod tour_song;
#[path = "audio/tuning.rs"]
mod tuning;

#[path = "data/api.rs"]
mod api;
#[path = "data/cache.rs"]
mod cache;
#[path = "data/chart.rs"]
mod chart;
#[path = "data/genskin.rs"]
mod genskin;
#[path = "data/lines.rs"]
mod lines;
#[path = "data/meta.rs"]
mod meta;
#[path = "data/sources.rs"]
mod sources;
#[path = "data/store.rs"]
mod store;
#[path = "data/theory.rs"]
mod theory;
#[path = "data/update.rs"]
mod update;
#[path = "data/winamp.rs"]
mod winamp;

#[path = "ui/album_view.rs"]
mod album_view;
#[path = "ui/chords.rs"]
mod chords;
#[path = "ui/credits.rs"]
mod credits;
#[path = "ui/font.rs"]
mod font;
#[path = "ui/help.rs"]
mod help;
#[path = "ui/history.rs"]
mod history;
#[path = "ui/icons.rs"]
mod icons;
#[path = "ui/library_win.rs"]
mod library_win;
#[path = "ui/lines.rs"]
mod lines_ui;
#[cfg(target_os = "macos")]
#[path = "ui/macmenu.rs"]
mod macmenu;
#[path = "ui/palette.rs"]
mod palette;
#[path = "ui/player_win.rs"]
mod player_win;
#[path = "ui/prefs.rs"]
mod prefs;
#[path = "ui/skin.rs"]
mod skin;
#[path = "ui/tools.rs"]
mod tools;
#[path = "ui/tour.rs"]
mod tour;
#[path = "ui/vicon.rs"]
mod vicon;
#[path = "ui/views.rs"]
mod views;
#[path = "ui/viz.rs"]
mod viz;
#[path = "ui/winamp_ui.rs"]
mod winamp_ui;

use api::{cover_url, Api, Card, GoTo, Kind, Page, Track};
use eframe::egui::{self, Align, Color32, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
use font::{fit, ptext, ptext_fit, snap, spx, text_w, thick, wrap};
use icons::*;
use player::{Cmd, Player};
use skin::*;
use sources::Src;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
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

#[allow(dead_code)]
const NB: usize = 19;
/// Points in the waveform visualizer.
const WAVE_N: usize = 128;
const BTN_H: f32 = 26.0;

/// Button height (it scales with a panel that scales its contents).
fn bh() -> f32 {
    BTN_H * font::ui_scale()
}
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
    /// a click in the macOS menu bar
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Menu(String),
    Tuning(i64, Option<(i32, f32)>),
    UpdateStaged(Result<(), String>),
    YtList(String, Result<(String, Vec<store::Ext>), String>),
    ScMeta(store::Ext),
    Wave(i64, Option<Vec<u8>>),
    Exported(Result<String, String>),
    /// LOOK UP result: tune name, who wrote it, other versions found on Tidal
    Lookup(String, Result<sources::Work, String>, Vec<(Track, u32)>),
    /// who wrote a tune and when, looked up on its own when the tune is opened
    TuneDetails(String, Result<sources::Work, String>),
    Chart(String, Result<(String, String, String), (String, Vec<String>)>),
    Live(String, Result<(String, String, String), (String, Vec<String>)>),
    Beats(i64, Option<(f32, f32)>),
    /// tempo / key detected from a track's audio
    Meta(i64, meta::Info),
    /// Winamp skins found (the list, whether they add to what is shown)
    WaList(Result<Vec<winamp::WaSkin>, String>, bool),
    /// a Winamp skin fetched and turned into colours
    WaSkin(Result<(winamp::WaSkin, skin::Pal, winamp::Art), String>),
    /// a free playlist font fetched (which one)
    FontReady(usize, Result<Vec<u8>, String>),
    /// the file picked for the opening sound
    IntroFile(Option<String>),
    /// the tour's song, fetched
    TourSong(Result<std::path::PathBuf, String>),
    /// the backing track was saved (its path) or not
    BandSaved(Result<String, String>),
    /// a file's own tags were written (or why not)
    TagsSaved(Result<String, String>),
    /// a track's credits: (role, names)
    Credits(Result<Vec<(String, String)>, String>),
    /// stem separation finished for this track id
    Stems(#[allow(dead_code)] i64, Result<(), String>),
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
    Chords,
    Lines,
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
    /// a track's album, artist or radio
    GoTo(i64, GoTo),
    /// show who made a track (id, "artist - title")
    Credits(i64, String),
    /// a track with several artists: ask which one to open
    PickArtist(Vec<(i64, String)>),
    Home,
    Library,
    Search(String),
    Back,
    Forward,
    /// put the windows back the classic way
    /// the queue: 0 shuffle, 1 sort by artist, 2 sort by title, 3 reverse
    QueueOp(u8),
    /// pick the playlist font (index into winamp_ui::PL_FONTS)
    PlFont(usize),
    /// left / right balance, -100 to 100
    Balance(i32),
    /// the Winamp skin browser: open / close, search, load more, wear one
    WaOpen,
    WaSearch,
    WaMore,
    WaApply(winamp::WaSkin),
    /// wear the palette as a Winamp skin drawn by Backline: 0 off, then one per look (genskin::LOOKS)
    WaGen(usize),
    /// a Backline palette in a look: 0 its own pixel panels (RETRO ORIGINAL), 2 the sleek Winamp style (ORIGINAL)
    Theme(usize, usize),
    /// the opening sound: 0 off, 1 Backline's chime, 2 your file; 3 pick a file; 4 play it now
    Intro(u8),
    /// take a skin off the list of ones worn lately
    WaForget(String),
    /// play through another sound output (None = the system default)
    SetDevice(Option<String>),
    /// covers as spinning records, or square
    ToggleVinyl,
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
    LoopToggle,
    LoopClear,
    Speed(u32),
    ToggleCacheView,
    /// the BACKLINE tab: skins, preferences, help, storage, the log
    BacklinePage,
    /// sound these notes (MIDI) as a chord
    PlayNotes(Vec<i32>),
    /// sound these chords one after another
    PlaySequence(Vec<Vec<i32>>),
    /// chords one after another, this many seconds apart
    PlaySequenceAt(Vec<Vec<i32>>, f32, String),
    /// stop a sequence started with PlaySequenceAt
    StopSequence,
    /// open the editor for a song's title and artist (not Tidal's), and save what was typed
    EditInfo(i64, String, String),
    SaveInfo(i64, String, String),
    /// load your Tidal library again
    RefreshLibrary,
    /// look at a picture big: its link, its title, and whether it is an artist's
    ArtPreview(String, String, bool),
    /// the log, under the BACKLINE tab
    ShowLog,
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
    ToggleHelp,
    SetMode(u8),
    QuitApp,
    TogglePrefs,
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
    /// look up who wrote this tune and when (MusicBrainz only, no Tidal search)
    TuneDetails(usize),
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
/// How fast the player's title scrolls (points a second, as f32 bits), so other scrolling text can keep pace.
static TITLE_SCROLL: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// The player title's scrolling speed (points a second), or a usual one before the player has drawn.
fn title_scroll() -> f32 {
    let v = f32::from_bits(TITLE_SCROLL.load(std::sync::atomic::Ordering::Relaxed));
    if v > 0.0 {
        v
    } else {
        9.0 * spx(1.5)
    }
}

/// Scrolling bitmap text inside `rect` (static if it fits).
fn marquee(ui: &egui::Ui, rect: Rect, text: &str, px: f32, color: Color32) {
    marquee_at(ui, rect, text, px, color, 9.0 * spx(px));
}

/// Scrolling bitmap text inside `rect`, moving `speed` points a second (static if it fits).
fn marquee_at(ui: &egui::Ui, rect: Rect, text: &str, px: f32, color: Color32, speed: f32) {
    let px = spx(px);
    let w = text_w(text, px);
    let clip = ui.painter().with_clip_rect(rect.shrink2(Vec2::new(2.0, 0.0)));
    let cy = rect.center().y;
    if w <= rect.width() - 8.0 {
        ptext(&clip, Pos2::new(rect.min.x + 5.0, cy), Align::Min, text, px, color);
    } else {
        let period = w + 8.0 * px;
        let t = ui.input(|i| i.time) as f32;
        let off = (t * speed) % period;
        let x = rect.min.x + 5.0 - off;
        ptext(&clip, Pos2::new(x, cy), Align::Min, text, px, color);
        ptext(&clip, Pos2::new(x + period, cy), Align::Min, text, px, color);
    }
}

/// A button showing a pixel icon, `w` wide. `col` tints it; `on` draws it pressed.
fn icon_btn_w(ui: &mut egui::Ui, icon: &[&str], on: bool, col: Color32, w: f32) -> egui::Response {
    let sc = font::ui_scale();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w * sc, bh()), Sense::click());
    let down = resp.is_pointer_button_down_on() || on;
    raised_h(ui.painter(), rect, down, resp.hovered());
    let px = 2.0 * sc;
    let (iw, ih) = (icon[0].len() as f32 * px, icon.len() as f32 * px);
    let dy = if down { 1.0 } else { 0.0 };
    let o = Pos2::new((rect.center().x - iw / 2.0).round(), (rect.center().y - ih / 2.0 + dy).round());
    pixmap(ui.painter(), o, px, icon, col);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The look Backline's own palette is worn in: 0 its usual look, 1.. a Winamp skin drawn from it (genskin).
fn gen_look(worn: Option<&(String, String)>) -> usize {
    worn.and_then(|w| w.0.strip_prefix("gen:")?.parse::<usize>().ok()).unwrap_or(0)
}

/// The skins, in three sections side by side: ORIGINAL (Backline's palettes as sleek Winamp-style skins, in
/// Silkscreen), RETRO ORIGINAL (Backline's own pixel panels, as it always was) and WINAMP (skins from the Skin
/// Museum: the one worn, and browsing for more). The one worn is marked.
fn skin_menu(ui: &mut egui::Ui, acts: &mut Vec<Action>, worn: Option<&(String, String)>, recent: &[(String, String)]) {
    let look = gen_look(worn);
    let museum = worn.filter(|_| look == 0).map(|w| w.1.as_str());
    let cur = SKIN.load(Ordering::Relaxed) % PALS.len();
    // the columns wrap under each other when the space is narrow
    ui.horizontal_wrapped(|ui| {
        for (title, g) in [("ORIGINAL", 2usize), ("RETRO ORIGINAL", 0)] {
            ui.vertical(|ui| {
                ui.set_min_width(170.0);
                para(ui, title, pal().ink2);
                for (n, name) in SKIN_NAMES.iter().enumerate() {
                    let on = n == cur && if g == 0 { worn.is_none() } else { look == g };
                    if menu_item(ui, &format!("{}{}", if on { "> " } else { "  " }, name)) {
                        acts.push(Action::Theme(n, g));
                        ui.close_menu();
                    }
                }
            });
        }
        ui.vertical(|ui| {
            ui.set_min_width(170.0);
            para(ui, "WINAMP", pal().ink2);
            // the ones worn lately (the one on now marked): back in a click, from the copy kept on disk
            for (md5, name) in recent {
                let on = museum.is_some() && worn.map_or(false, |w| &w.0 == md5);
                ui.horizontal(|ui| {
                    // X takes it off the list
                    if retro_btn_w(ui, "X", 24.0, false).tip("Take it off this list").clicked() {
                        acts.push(Action::WaForget(md5.clone()));
                    }
                    if menu_item(ui, &format!("{}{}", if on { "> " } else { "  " }, name.to_uppercase())) {
                        let s =
                            winamp::WaSkin { md5: md5.clone(), name: name.clone(), shot: String::new(), download: String::new() };
                        acts.push(Action::WaApply(s));
                        ui.close_menu();
                    }
                });
            }
            if menu_item(ui, "  BROWSE SKINS...") {
                acts.push(Action::WaOpen);
                ui.close_menu();
            }
        });
    });
}

fn icon_btn(ui: &mut egui::Ui, icon: &[&str], on: bool, col: Color32) -> egui::Response {
    icon_btn_w(ui, icon, on, col, 44.0)
}

/// Icon on the left, a short label on the right.
fn ibtn(ui: &mut egui::Ui, icon: &[&str], text: &str, on: bool) -> egui::Response {
    let sc = font::ui_scale();
    let ip = 2.0 * sc;
    let w = (10.0 + 8.0 + 10.0) * sc + icon[0].len() as f32 * ip + text_w(text, 2.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, bh()), Sense::click());
    let down = resp.is_pointer_button_down_on() || on;
    raised_h(ui.painter(), rect, down, resp.hovered());
    let dy = if down { 1.0 } else { 0.0 };
    let o = Pos2::new((rect.min.x + 10.0 * sc).round(), (rect.center().y - icon.len() as f32 * ip / 2.0 + dy).round());
    pixmap(ui.painter(), o, ip, icon, pal().ink);
    let tx = o.x + icon[0].len() as f32 * ip + 8.0 * sc;
    ui_text(ui.painter(), Pos2::new(tx, rect.center().y + dy), Align::Min, text, 2.0, rect.max.x - tx - 4.0, pal().ink);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Interface text (buttons, tabs, headers, titles): in the worn Winamp skin's own font when there is one,
/// otherwise Backline's.
fn ui_text(p: &egui::Painter, pos: Pos2, align: Align, text: &str, px: f32, max_w: f32, col: Color32) {
    ptext_fit(p, pos, align, text, px, max_w, col);
}

/// Fit a pixel icon into `r` as large as whole dots allow.
fn icon_in(p: &egui::Painter, r: Rect, icon: &[&str], col: Color32) {
    // the dot size is a whole number of real screen pixels: crisp, and as fine as the button allows
    // with a margin all round: as big as fits in both directions, not just the height
    let ppp = font::ppp();
    let fit = ((r.height() - 5.0) / icon.len() as f32).min((r.width() - 8.0) / icon[0].len() as f32);
    let k = ((fit * ppp).floor().max(1.0)) / ppp;
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
    tour::note(title, outer);
    // a worn Winamp skin dresses every window in its own frame
    if winamp_ui::draw_frame(p, outer, inner, title) {
        return;
    }
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

/// Logo strip at the top of the library window (its words in the skin's font when a skin is worn).
fn logo(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.0), Sense::hover());
    let p = ui.painter();
    inset(p, rect, pal().lcd);
    logo_mark(p, Pos2::new(rect.min.x + 12.0, rect.center().y - 13.5), 3.0);
    let (tsz, ty, gy) = if style() == 0 { (3.0, -8.0, 13.0) } else { (2.6, -9.0, 14.0) };
    ptext(p, Pos2::new(rect.min.x + 50.0, rect.center().y + ty), Align::Min, "BACKLINE", tsz, pal().ink);
    let tag =
        if PRACTICE { "A RETRO PLAYER FOR TIDAL AND MORE - MADE FOR PRACTICE" } else { "A RETRO PLAYER FOR TIDAL AND MORE" };
    let version = if update::build() > 0 { format!("{} .{}", VERSION, update::build()) } else { VERSION.to_string() };
    // the tagline stops short of the version number, whatever the font's width
    let vw = text_w(&version, 1.0);
    let tag_w = rect.width() - 51.0 - vw - 24.0;
    ptext_fit(p, Pos2::new(rect.min.x + 51.0, rect.center().y + gy), Align::Min, tag, 1.0, tag_w, pal().ink2);
    ptext(p, Pos2::new(rect.max.x - 10.0, rect.center().y + gy), Align::Max, &version, 1.0, pal().dim);
}

/// Folder-style tabs with a baseline; the open tab joins the panel below. Returns the clicked tab.
fn tab_row(ui: &mut egui::Ui, items: &[&str], cur: usize) -> Option<usize> {
    let h = 26.0 * font::ui_scale();
    let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h + 6.0), Sense::hover());
    let t = thick(2.0);
    let base = row.max.y - 4.0;
    fill_rect(ui.painter(), Rect::from_min_max(Pos2::new(row.min.x, base), Pos2::new(row.max.x, base + t)), pal().edge);
    let (mut x, mut hit) = (row.min.x + 4.0, None);
    // too many tabs for the width: tighten the padding first, then squeeze them evenly (the text shrinks to fit)
    let avail = (row.width() - 8.0).max(60.0);
    let gaps = 3.0 * items.len().saturating_sub(1) as f32;
    let total = |pad: f32| items.iter().map(|n| text_w(n, 2.0) + pad).sum::<f32>() + gaps;
    let mut pad = 24.0;
    while pad > 10.0 && total(pad) > avail {
        pad -= 2.0;
    }
    let shrink = (avail / total(pad)).min(1.0);
    for (i, name) in items.iter().enumerate() {
        let w = (text_w(name, 2.0) + pad) * shrink;
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
            ptext_fit(p, r.center(), Align::Center, name, 2.0, w - 8.0, col);
            if resp.clicked() {
                hit = Some(i);
            }
            x += w + 3.0;
            continue;
        }
        // in the skin's colours (a piece of its title bar would bring its stripes through the words)
        {
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
        }
        ui_text(
            p,
            Pos2::new(r.center().x, r.min.y + (base - r.min.y) / 2.0 + 1.0),
            Align::Center,
            name,
            2.0,
            w - 8.0,
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
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, bh()), Sense::click());
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
    let w = (w * font::ui_scale()).max(text_w(text, 2.0) + 16.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, bh()), Sense::click());
    let down = resp.is_pointer_button_down_on() || active;
    raised_h(ui.painter(), rect, down, resp.hovered());
    let dy = if down { 1.0 } else { 0.0 };
    ui_text(ui.painter(), rect.center() + Vec2::new(0.0, dy), Align::Center, text, 2.0, rect.width() - 8.0, pal().ink);
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
    ui_text(ui.painter(), Pos2::new(rect.min.x + 8.0, rect.center().y), Align::Min, text, 2.0, rect.width() - 16.0, pal().trim);
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

/// One piece of text in a list row: the skin's playlist font when a Winamp skin is on, Backline's pixel font
/// otherwise; cut to fit `max_w`. Returns how wide it came out.
fn row_text(p: &egui::Painter, pos: Pos2, align: Align, text: &str, max_w: f32, col: Color32) -> f32 {
    if winamp_ui::chrome_on() {
        winamp_ui::sans_text(p, pos, align, text, ROW_H * 0.7, max_w, col)
    } else {
        ptext(p, pos, align, &fit(text, 2.0, max_w), 2.0, col)
    }
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
        row_text(p, Pos2::new(rect.max.x - 10.0, cy), Align::Max, b, 200.0, fg2);
        let w = row_text(p, Pos2::new(rect.max.x - 10.0 - TBL_W2, cy), Align::Max, a, 200.0, fg2);
        TBL_W2 + w
    } else {
        row_text(p, Pos2::new(rect.max.x - 10.0, cy), Align::Max, &rtxt, 300.0, fg2)
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
            row_text(p, Pos2::new(x0 + 34.0, cy), Align::Max, &n.to_string(), 34.0, fg2);
        }
        tx = x0 + 44.0;
    }
    let lraw = left();
    if lraw.contains('\t') {
        // cell layout shared with the column header
        let avail = (rect.max.x - 10.0 - right_w() - tx).max(40.0);
        let cw = col_widths(avail);
        let cells: Vec<&str> = lraw.split('\t').collect();
        let mut x = tx;
        for (i, c) in cells.iter().enumerate().take(3) {
            if cw[i] > 0.0 {
                row_text(p, Pos2::new(x, cy), Align::Min, c, (cw[i] - 12.0).max(20.0), if i == 0 { fg } else { fg2 });
                x += cw[i];
            }
        }
        if let (true, Some((key, bpm))) = (col_on(3), cells.get(3).and_then(|c| c.split_once('|'))) {
            let kx = rect.max.x - 10.0 - 80.0 - KB_W;
            row_text(p, Pos2::new(kx, cy), Align::Min, key, KB_GAP - 6.0, fg2);
            row_text(p, Pos2::new(kx + KB_GAP, cy), Align::Min, bpm, 50.0, fg2);
        }
    } else {
        let avail = (rect.max.x - 10.0 - rw - 16.0) - tx;
        row_text(p, Pos2::new(tx, cy), Align::Min, &lraw, avail.max(20.0), fg);
    }
    Some(resp.on_hover_cursor(egui::CursorIcon::PointingHand))
}

/// "Go to album / artist / track radio" and the credits, for a Tidal track (files, YouTube and SoundCloud have none).
/// A track with several artists gets one entry per artist, so you pick which one.
fn goto_items(ui: &mut egui::Ui, acts: &mut Vec<Action>, t: &Track, tidal: bool) {
    // your files, YouTube and SoundCloud songs can be renamed; a Tidal song's names are Tidal's, but its key and
    // tempo can be set (kept on this computer)
    if menu_item(ui, if tidal { "Key and BPM..." } else { "Edit info..." }) {
        acts.push(Action::EditInfo(t.id, t.title.clone(), t.artist.clone()));
        ui.close_menu();
    }
    if !tidal {
        return;
    }
    let mut go = |ui: &mut egui::Ui, label: &str, a: Action| {
        if menu_item(ui, label) {
            acts.push(a);
            ui.close_menu();
        }
    };
    go(ui, "Go to album", Action::GoTo(t.id, GoTo::Album));
    if t.artists.len() > 1 {
        go(ui, "Go to artist...", Action::PickArtist(t.artists.clone()));
    } else {
        go(ui, "Go to artist", Action::GoTo(t.id, GoTo::Artist));
    }
    go(ui, "Go to track radio", Action::GoTo(t.id, GoTo::Radio));
    go(ui, "Show credits", Action::Credits(t.id, format!("{} - {}", t.artists_text(), t.title)));
}

/// Retro-styled entry for right-click menus. Returns true when clicked.
fn menu_item(ui: &mut egui::Ui, text: &str) -> bool {
    let sc = font::ui_scale();
    let w = (text_w(text, 2.0) + 28.0 * sc).max(150.0 * sc);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0 * sc), Sense::click());
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
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, bh()), Sense::click());
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
    ui_text(ui.painter(), rect.center(), Align::Center, text, 1.0, rect.width() - 6.0, pal().ink);
    r
}

/// Sunken LCD readout box.
fn lcd_box(ui: &mut egui::Ui, text: &str, w: f32, col: Color32) {
    // never narrower than its text (a wide font or a bigger number would be cut off)
    let w = (w * font::ui_scale()).max(text_w(text, 2.0) + 16.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, bh()), Sense::hover());
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
    ui_text(ui.painter(), Pos2::new(rect.min.x + 2.0, rect.center().y), Align::Min, &t, px, rect.width() - 4.0, col);
}

// -------------------------------------------------------------------- misc
fn fmt_time(s: f32) -> String {
    let s = s.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Time with tenths, for loop points: 1:23.4
fn fmt_t(s: f32) -> String {
    // round to tenths first, so 59.96 shows as 1:00.0 and not 0:60.0
    let tenths = (s.max(0.0) * 10.0).round() as u32;
    format!("{}:{:02}.{}", tenths / 600, tenths / 10 % 60, tenths % 10)
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
    /// texture (None = failed to load) and when it was last drawn
    map: HashMap<String, (Option<egui::TextureHandle>, Instant)>,
    requested: HashSet<String>,
    req: Sender<String>,
    /// average colour of the top and bottom half of each cover, for the art-view gradient
    avg: HashMap<String, [Color32; 2]>,
    /// per place a cover is shown (album view, player): the cover on show, the one it is fading from, and when
    /// the new one arrived. The old cover stays up while the next loads, so a track change never flashes.
    hold: HashMap<&'static str, (Option<egui::TextureId>, Option<egui::TextureId>, f64)>,
}

impl Images {
    /// A loaded picture's size in pixels.
    fn size(&self, url: &str) -> Option<[usize; 2]> {
        self.map.get(url).and_then(|(t, _)| t.as_ref()).map(|h| h.size())
    }

    fn get(&mut self, url: &str) -> Option<egui::TextureId> {
        if url.is_empty() {
            return None;
        }
        if let Some((tex, used)) = self.map.get_mut(url) {
            *used = Instant::now();
            return tex.as_ref().map(|h| h.id());
        }
        if self.requested.insert(url.to_string()) {
            let _ = self.req.send(url.to_string());
        }
        None
    }

    /// The cover to show at `slot` for `url`: (now showing, fading from, how far the fade is 0..1).
    /// While the new cover loads, the previous one stays; when it arrives it fades in over the old.
    fn held(
        &mut self,
        slot: &'static str,
        url: &str,
        gray: bool,
        now: f64,
    ) -> (Option<egui::TextureId>, Option<egui::TextureId>, f32) {
        let new = art_tex(self, url, gray);
        let gone = url.is_empty() || self.failed(url);
        let e = self.hold.entry(slot).or_insert((None, None, 0.0));
        match new {
            Some(n) if e.0 != Some(n) => *e = (Some(n), e.0, now),
            None if gone => *e = (None, None, 0.0),
            _ => {}
        }
        let f = ((now - e.2) / 0.35).clamp(0.0, 1.0) as f32;
        if f >= 1.0 {
            e.1 = None;
        }
        (e.0, e.1, f)
    }

    /// Tried and failed (or no picture at all): nothing is coming.
    fn failed(&self, url: &str) -> bool {
        matches!(self.map.get(url), Some((None, _)))
    }

    /// Covers not drawn for two minutes are let go (a big one is ~1.6 MB), so long browsing doesn't keep growing.
    /// They load again if they come back on screen.
    fn trim(&mut self) {
        if self.map.len() < 150 {
            return;
        }
        let old: Vec<String> =
            self.map.iter().filter(|(_, (_, t))| t.elapsed() > Duration::from_secs(120)).map(|(u, _)| u.clone()).collect();
        for u in old {
            self.map.remove(&u);
            self.requested.remove(&u);
        }
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
    } else {
        // no cover (yet): the record, turning while it loads
        let loading = !url.is_empty() && !images.failed(url);
        paint_record(ui.painter(), rect, loading.then(|| ui.input(|i| i.time) as f32));
        if loading {
            ui.ctx().request_repaint();
        }
    }
}

/// The stand-in for a missing picture: a quiet square with the Backline gem, or (`spin` = seconds) a small
/// turning arc while it loads.
fn paint_record(p: &egui::Painter, rect: Rect, spin: Option<f32>) {
    let c = rect.center();
    let s = rect.width().min(rect.height());
    p.rect_filled(Rect::from_center_size(c, Vec2::splat(s)), 0.0, Color32::from_black_alpha(60));
    let col = pal().ink2.gamma_multiply(0.7);
    match spin {
        Some(t) => {
            // three quarters of a ring, turning
            let r = s * 0.12;
            let pts: Vec<Pos2> = (0..=24).map(|k| c + Vec2::angled(t * 4.0 + k as f32 / 24.0 * 4.7) * r).collect();
            p.add(egui::Shape::line(pts, Stroke::new((s * 0.025).max(2.0), col)));
        }
        None => {
            // the Backline gem, quietly
            let g = Rect::from_center_size(c, Vec2::splat(s * 0.36));
            vicon::gem(p, g, pal().ink2.gamma_multiply(0.75), pal().ink2.gamma_multiply(0.35));
        }
    }
}

/// Covers shown as records instead of squares (click a cover to switch). Saved in the settings.
static VINYL: AtomicBool = AtomicBool::new(false);

/// How far the record has turned: it spins up to 33 1/3 rpm times the playback speed (`speed`, 0 = stopped)
/// and coasts to a stop on pause.
fn vinyl_angle(ui: &egui::Ui, speed: f32) -> f32 {
    let id = egui::Id::new("vinyl_angle");
    let dt = ui.input(|i| i.stable_dt).min(0.1);
    let (ang, vel): (f32, f32) = ui.ctx().data(|d| d.get_temp(id)).unwrap_or((0.0, 0.0));
    let target = if skin::calm() { 0.0 } else { 3.49 * speed }; // radians a second at 33 1/3 rpm
    let vel = vel + (target - vel) * (1.0 - (-dt * 2.0).exp());
    let ang = (ang + vel * dt) % std::f32::consts::TAU;
    ui.ctx().data_mut(|d| d.insert_temp(id, (ang, vel)));
    if vel > 0.005 {
        ui.ctx().request_repaint();
    }
    ang
}

/// A record whose label is the cover, turned by `angle`. The light on the grooves stays put, like a real one.
/// `alpha` below 1 lets what is behind it (the visualizer) show through.
fn paint_vinyl(p: &egui::Painter, rect: Rect, tex: Option<egui::TextureId>, angle: f32, alpha: f32) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.5;
    let k = |col: Color32| col.gamma_multiply(alpha);
    p.circle_filled(c, r, k(Color32::from_gray(14)));
    for g in 0..9 {
        let shade = Color32::from_gray(if g % 3 == 0 { 44 } else { 30 });
        p.circle_stroke(c, r * (0.47 + g as f32 * 0.058), Stroke::new(1.0_f32, k(shade)));
    }
    // the light: two soft wedges, fixed
    for (a0, a1) in [(-2.4f32, -1.9f32), (0.75, 1.25)] {
        let mut pts = vec![];
        for s in 0..=8 {
            pts.push(c + Vec2::angled(a0 + (a1 - a0) * s as f32 / 8.0) * r * 0.97);
        }
        for s in (0..=8).rev() {
            pts.push(c + Vec2::angled(a0 + (a1 - a0) * s as f32 / 8.0) * r * 0.46);
        }
        p.add(egui::Shape::Path(egui::epaint::PathShape::convex_polygon(pts, k(Color32::from_white_alpha(10)), Stroke::NONE)));
    }
    // the label: the cover cut to a circle, turning
    let lr = r * 0.42;
    match tex {
        Some(id) => {
            let tint = k(Color32::WHITE);
            let mut mesh = egui::Mesh::with_texture(id);
            mesh.vertices.push(egui::epaint::Vertex { pos: c, uv: Pos2::new(0.5, 0.5), color: tint });
            let n = 64;
            for s in 0..=n {
                let a = s as f32 / n as f32 * std::f32::consts::TAU;
                let uv = Vec2::angled(a - angle) * 0.5;
                let pos = c + Vec2::angled(a) * lr;
                mesh.vertices.push(egui::epaint::Vertex { pos, uv: Pos2::new(0.5 + uv.x, 0.5 + uv.y), color: tint });
                if s > 0 {
                    mesh.indices.extend_from_slice(&[0, s as u32, s as u32 + 1]);
                }
            }
            p.add(egui::Shape::mesh(mesh));
        }
        None => {
            p.circle_filled(c, lr, k(pal().red));
            let mark = [c + Vec2::angled(angle) * lr * 0.3, c + Vec2::angled(angle) * lr * 0.85];
            p.line_segment(mark, Stroke::new(2.0_f32, k(Color32::from_white_alpha(120))));
        }
    }
    p.circle_stroke(c, lr, Stroke::new(1.5_f32, k(Color32::from_black_alpha(160))));
    p.circle_filled(c, (r * 0.035).max(2.0), Color32::from_gray(8));
}

/// A page's own picture, round for an artist; a spinning record while it loads, a still one when there is none.
fn paint_page_art(ui: &egui::Ui, images: &mut Images, url: &str, rect: Rect, artist: bool) {
    match images.get(url) {
        Some(id) => {
            let round = if artist { rect.width() * 0.5 } else { 0.0 };
            egui::Image::new(egui::load::SizedTexture::new(id, rect.size())).rounding(Rounding::same(round)).paint_at(ui, rect);
            if artist {
                ui.painter().circle_stroke(rect.center(), rect.width() * 0.5, Stroke::new(2.0_f32, pal().trim));
            }
        }
        None if url.is_empty() || images.failed(url) => paint_record(ui.painter(), rect, None),
        None => {
            paint_record(ui.painter(), rect, Some(ui.input(|i| i.time) as f32));
            ui.ctx().request_repaint();
        }
    }
}

/// A cover drawn into four corners at `alpha`, with a hair cropped off each edge (where JPEG junk lives).
fn cover_quad(p: &egui::Painter, cs: [Pos2; 4], id: egui::TextureId, alpha: f32) {
    const E: f32 = 0.006;
    let uvs = [Pos2::new(E, E), Pos2::new(1.0 - E, E), Pos2::new(1.0 - E, 1.0 - E), Pos2::new(E, 1.0 - E)];
    let color = Color32::from_white_alpha((alpha.clamp(0.0, 1.0) * 255.0) as u8);
    let mut mesh = egui::Mesh::with_texture(id);
    for (pos, uv) in cs.iter().zip(uvs.iter()) {
        mesh.vertices.push(egui::epaint::Vertex { pos: *pos, uv: *uv, color });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    p.add(egui::Shape::mesh(mesh));
}

/// A cover that changes smoothly: the old one stays until the new one is in, then the new fades in over it.
fn paint_held(ui: &egui::Ui, images: &mut Images, slot: &'static str, url: &str, rect: Rect, gray: bool) {
    let (tex, prev, f) = images.held(slot, url, gray, ui.input(|i| i.time));
    let cs = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()];
    match tex {
        Some(id) => {
            if let Some(old) = prev {
                cover_quad(ui.painter(), cs, old, 1.0);
            }
            cover_quad(ui.painter(), cs, id, f);
            if f < 1.0 {
                ui.ctx().request_repaint();
            }
        }
        None => paint_art_g(ui, images, url, rect, 0.0, gray),
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
/// Which extra columns the track lists show: bit0 artist, bit1 album, bit2 length, bit3 key and tempo
/// (name is always shown).
static COLS: AtomicU32 = AtomicU32::new(0b1101);
const COL_NAMES: [&str; 4] = ["ARTIST", "ALBUM", "LENGTH", "KEY / BPM"];
/// width of the KEY / BPM slot, just left of LENGTH; the BPM sits `KB_GAP` in (room for "C#m?")
const KB_W: f32 = 120.0;
const KB_GAP: f32 = 64.0;
/// how the NAME, ARTIST and ALBUM columns share the room (drag the lines between them in the header)
static COL_W: Mutex<[f32; 3]> = Mutex::new([0.4, 0.3, 0.3]);
/// the columns were changed (shown, hidden or resized): the settings need saving
static COLS_CHANGED: AtomicBool = AtomicBool::new(false);

/// Room kept on the right of a row for LENGTH (and KEY / BPM when shown).
fn right_w() -> f32 {
    80.0 + if col_on(3) { KB_W } else { 0.0 }
}

fn col_on(i: usize) -> bool {
    COLS.load(Ordering::Relaxed) >> i & 1 == 1
}

/// Widths of the NAME, ARTIST and ALBUM cells inside `avail` (0 for hidden ones).
fn col_widths(avail: f32) -> [f32; 3] {
    let w = *COL_W.lock().unwrap();
    let on = [true, col_on(0), col_on(1)];
    let sum: f32 = (0..3).filter(|&i| on[i]).map(|i| w[i]).sum();
    let mut out = [0.0; 3];
    for i in (0..3).filter(|&i| on[i]) {
        out[i] = avail * w[i] / sum.max(1e-6);
    }
    out
}

/// Header row over a track list; right-click it to choose the columns.
fn col_header(ui: &mut egui::Ui, numbered: bool) {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 20.0), Sense::click());
    fill_rect(ui.painter(), rect, pal().edge);
    let cy = rect.center().y;
    let tx = rect.min.x + 8.0 + if numbered { 44.0 } else { 0.0 };
    let avail = (rect.max.x - 10.0 - right_w() - tx).max(40.0);
    let cw = col_widths(avail);
    let p = ui.painter();
    ui_text(p, Pos2::new(rect.min.x + 8.0, cy), Align::Min, if numbered { "#" } else { "" }, 2.0, 400.0, pal().trim);
    if col_on(3) {
        let kx = rect.max.x - 10.0 - 80.0 - KB_W;
        ui_text(p, Pos2::new(kx, cy), Align::Min, "KEY", 2.0, 400.0, pal().trim);
        ui_text(p, Pos2::new(kx + KB_GAP, cy), Align::Min, "BPM", 2.0, 400.0, pal().trim);
    }
    ui_text(p, Pos2::new(tx, cy), Align::Min, "NAME", 2.0, 400.0, pal().trim);
    let mut x = tx + cw[0];
    for (i, name) in COL_NAMES[..2].iter().enumerate() {
        if cw[i + 1] > 0.0 {
            ui_text(p, Pos2::new(x, cy), Align::Min, name, 2.0, 400.0, pal().trim);
            x += cw[i + 1];
        }
    }
    if col_on(2) {
        ui_text(p, Pos2::new(rect.max.x - 10.0, cy), Align::Max, "LENGTH", 2.0, 400.0, pal().trim);
    }
    // drag the line between two columns to share the room differently
    let vis: Vec<usize> = (0..3).filter(|&i| cw[i] > 0.0).collect();
    let mut bx = tx;
    for k in 0..vis.len().saturating_sub(1) {
        bx += cw[vis[k]];
        let hit = Rect::from_center_size(Pos2::new(bx - 6.0, cy), Vec2::new(12.0, rect.height()));
        let r =
            ui.interact(hit, ui.id().with(("col_split", k)), Sense::drag()).on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
        if r.hovered() || r.dragged() {
            fill_rect(ui.painter(), Rect::from_center_size(hit.center(), Vec2::new(2.0, rect.height() - 4.0)), pal().trim);
        }
        if r.dragged() {
            let mut w = COL_W.lock().unwrap();
            let (a, b) = (vis[k], vis[k + 1]);
            let sum: f32 = vis.iter().map(|&i| w[i]).sum();
            let min = 0.1 * sum;
            let d = (r.drag_delta().x / avail * sum).clamp(min - w[a], w[b] - min);
            w[a] += d;
            w[b] -= d;
            COLS_CHANGED.store(true, Ordering::Relaxed);
        }
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).tip("Right-click to choose columns").context_menu(|ui| {
        for (i, name) in COL_NAMES.iter().enumerate() {
            let mark = if col_on(i) { "[x] " } else { "[ ] " };
            if menu_item(ui, &format!("{}{}", mark, name)) {
                COLS.fetch_xor(1 << i, Ordering::Relaxed);
                COLS_CHANGED.store(true, Ordering::Relaxed);
            }
        }
    });
}

/// One track row's text, split into cells with tabs for `list_row` (the last cell is key and tempo, if known).
fn track_cells(id: i64, title: &str, artist: &str, album: &str) -> String {
    let i = meta::get(id).unwrap_or_default();
    let key = meta::key_text(&i);
    let bpm = i.bpm.map_or(String::new(), |b| b.to_string());
    format!("{}\t{}\t{}\t{}|{}", title, artist, album, key, bpm)
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
            || track_cells(t.id, &t.title, &t.artists_text(), &t.album),
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
                goto_items(ui, acts, t, true);
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
            if !c.image.is_empty() {
                r.context_menu(|ui| {
                    if menu_item(ui, "View art") {
                        acts.push(Action::ArtPreview(c.image.clone(), c.title.clone(), c.kind == Kind::Artist));
                        ui.close_menu();
                    }
                });
            }
        }
    }
}

/// PLAY and SHUFFLE for a list; `refresh`: and REFRESH, to load it from Tidal again (your library).
fn play_buttons(ui: &mut egui::Ui, tracks: &[Track], acts: &mut Vec<Action>, refresh: bool) {
    ui.horizontal(|ui| {
        if ibtn(ui, &IC_PLAY, "PLAY", false).clicked() {
            acts.push(Action::Play(tracks.to_vec(), 0));
        }
        if ibtn(ui, &IC_SHUF, "SHUFFLE", false).clicked() {
            acts.push(Action::PlayShuffled(tracks.to_vec()));
        }
        if refresh && ibtn(ui, &IC_REP, "REFRESH", false).tip("Load your library from Tidal again").clicked() {
            acts.push(Action::RefreshLibrary);
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
                ui.add_space(4.0);
                play_buttons(ui, &page.tracks, acts, true);
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
                ui.add_space(4.0);
                // the same library load brings these, so the same REFRESH
                if ibtn(ui, &IC_REP, "REFRESH", false).tip("Load your library from Tidal again").clicked() {
                    acts.push(Action::RefreshLibrary);
                }
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
            // the page's picture, square and framed like the rest of the skin; click it to look at it big
            let (cr, cresp) = ui.allocate_exact_size(Vec2::splat(104.0), Sense::click());
            inset(ui.painter(), cr, pal().edge);
            paint_page_art(ui, images, &page.image, cr.shrink(2.0), false);
            if !page.image.is_empty()
                && cresp.on_hover_cursor(egui::CursorIcon::PointingHand).tip("Look at the picture big").clicked()
            {
                acts.push(Action::ArtPreview(page.image.clone(), page.title.clone(), page.artist));
            }
            ui.vertical(|ui| {
                title_line(ui, &page.title, 3.0, pal().ink);
                title_line(ui, &page.subtitle, 2.0, pal().ink2);
                ui.add_space(6.0);
                if !page.tracks.is_empty() {
                    play_buttons(ui, &page.tracks, acts, false);
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
    // songs first, as in any music player, then the albums / artists / playlists
    if !page.tracks.is_empty() {
        if !page.rows.is_empty() {
            section_header(ui, "SONGS");
        }
        tracks_list(ui, &page.tracks, playing_id, liked, acts);
    }
    for (title, cards) in &page.rows {
        section_header(ui, title);
        cards_list(ui, cards, true, acts);
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
    /// your library as last loaded: MY LIBRARY shows it at once (REFRESH loads it again)
    lib_page: Option<Arc<Page>>,
    /// a Winamp skin being fetched and put on (its name), for the little "putting it on" panel
    wa_loading: Option<String>,
    /// the picture being looked at big (link, title, an artist's?)
    art_preview: Option<(String, String, bool)>,
    back: Vec<Arc<Page>>,
    /// pages left with Back, for Forward (cleared when you open something new)
    fwd: Vec<Arc<Page>>,
    /// the credits panel: which track, and its credits once they arrive
    credits: Option<(String, Option<Result<Vec<(String, String)>, String>>)>,
    /// the "which artist?" panel
    artist_pick: Option<Vec<(i64, String)>>,
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
    show_log: bool,
    audio_err_shown: bool,

    art_view: bool,
    show_spec: bool,
    spec_op: f32,
    spec_h: f32,
    spec_w: f32,
    /// the album view's visualizer in a box: 0 none, 1 a themed box, 2 a thin line
    spec_frame: u8,
    /// the visualizer settings panel, open on 0 (the player's screen) or 1 (the album view)
    viz_panel: Option<usize>,
    /// the Winamp skin browser: open, what was searched, the skins found, still loading, no more to load,
    /// and the skin being worn (md5, name)
    show_winamp: bool,
    /// the BACKLINE page (skins, preferences, help, storage, the log, signing out)
    show_tl: bool,
    /// where you have been in the library, for back / forward (see history.rs)
    hist_back: Vec<history::Spot>,
    hist_fwd: Vec<history::Spot>,
    hist_at: Option<history::Spot>,
    wa_q: String,
    wa_list: Vec<winamp::WaSkin>,
    wa_busy: bool,
    wa_end: bool,
    wa_worn: Option<(String, String)>,
    /// Winamp skins worn lately (md5, name), newest first, to go back to one in a click
    wa_recent: Vec<(String, String)>,
    /// the worn Winamp skin's own pictures, drawn for the player and the equalizer
    wa_art: Option<winamp_ui::WaTex>,
    /// the album view cover's border (right-click the cover): 0 none, 1 subtle, 2 the skin's colour
    art_border: u8,
    /// art view caption: size (0 small, 1 medium, 2 large) and soft black text on a light plate instead of soft white
    cap_size: u8,
    cap_dark: bool,
    /// the album view's background colours as shown, easing toward the new cover's
    bg_now: Option<[Color32; 2]>,
    /// the sound output chosen in Preferences (None = the system default), and the outputs last found
    out_device: Option<String>,
    out_list: Vec<String>,
    /// the queue panel open over the album view
    art_queue: bool,
    /// visualizer settings: [0] the small one in the player, [1] the one in the album view
    viz: [viz::VizCfg; 2],
    viz_wave: Vec<f32>,
    show_lyrics: bool,
    fullscreen: bool,
    /// fullscreen was turned on inside the album view, so leaving the view also leaves fullscreen
    fs_by_art: bool,
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
    /// the equalizer's preamp, in dB (-12 to +12)
    eq_pre: f32,
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
    /// the list last open in each group (MUSIC / PRACTICE), to come back to it
    last_music: Sec,
    last_practice: Sec,
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
    upd_note: String,
    upd_state: u8,
    last_active: Instant,
    show_help: bool,
    palette_open: bool,
    palette_q: String,
    palette_sel: usize,
    palette_frame: u32,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    menu_keep: Option<Box<dyn std::any::Any>>,
    fs_cmd_at: Option<Instant>,
    art_op: f32,
    show_prefs: bool,
    auto_restart: bool,
    /// the opening sound: 0 off, 1 Backline's chime, 2 the file below
    intro: u8,
    intro_file: Option<String>,
    binds: Vec<Option<egui::Key>>,
    rebinding: Option<usize>,
    bind_note: String,
    /// the running tour: (0 general / 1 practice, step)
    tour: Option<(u8, usize)>,
    /// what was showing when the tour began (album view, practice mode, MORE and its tab), put back after
    tour_restore: Option<(bool, bool, bool, u8)>,
    /// what was playing before the tour (it plays its own song), put back when it ends
    tour_play: Option<tour::TourPlay>,
    tour_seen: bool,
    help_tab: usize,
    /// (track id, cents from A440, confidence) from the tuning check
    tuning: Option<(i64, i32, f32)>,
    tuning_busy: bool,
    upd_checked: Option<Instant>,
    new_pl: String,
    pl_open: Option<usize>,
    pl_pick: Option<Track>,
    yt_results: Vec<store::Ext>,
    /// the opened playlist's name, and whether its songs are shown (it starts folded up)
    yt_title: String,
    yt_open: bool,
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
    /// bars picked on the lead sheet by dragging across them: the band loops just these (first, last)
    chart_sel: Option<(usize, usize)>,
    /// the bar a drag across the lead sheet started on
    chart_drag: Option<usize>,
    chart_edit: bool,
    /// the CHORDS tab: which side (find / name), the chord typed, the notes clicked on the neck and the piano,
    /// and guitar shapes already worked out
    chords_tab: u8,
    chord_q: String,
    an_frets: [Option<u8>; 6],
    /// how chord names are written, the details panel, the neck's size, how strong the hover rings are
    chord_sp: theory::Spelling,
    chord_details: bool,
    /// the CLUSTERS tab: key, parent scale, notes in the cluster, string set, the degree on show
    cl_key: i32,
    cl_scale: usize,
    cl_view: u8,
    cl_semi: bool,
    cl_low: i32,
    cl_bass: i32,
    cl_changes: String,
    cl_chord: usize,
    vc_family: u8,
    vc_root: i32,
    vc_type: usize,
    vc_set: usize,
    pass_pair: usize,
    /// LINES: which tab, the Slonimsky division / kind / pattern / first note / direction / octave up
    ln_tab: u8,
    /// SLONIMSKY'S DIARY: the book's pattern open, the note it starts on (0 = C, as written), played backwards
    ln_at: usize,
    ln_root: i32,
    ln_back: bool,
    /// the ABOUT THIS PATTERN panel open
    ln_about: bool,
    /// the HOW THESE PATTERNS WORK panel open
    ln_how: bool,
    /// with no pattern book: the octave division, the way notes are added, the pattern
    sl_div: usize,
    sl_kind: usize,
    sl_pat: usize,
    /// tempo, notes to a beat, round and round, and the line being played (for the diary's count)
    ln_bpm: u32,
    ln_sub: u8,
    ln_loop: bool,
    ln_playing: String,
    /// SCALES: family, scale, pattern, octaves, direction, root
    sc_fam: usize,
    sc_idx: usize,
    sc_pat: usize,
    sc_oct: usize,
    sc_dir: usize,
    sc_root: i32,
    /// the last sequence played, to go round again
    seq_last: Option<Vec<Vec<i32>>>,
    /// PRACTICE PROGRESSIONS: which groups are folded open, key, minor, 7th chords, all twelve keys, one being typed
    prog_shown: Vec<bool>,
    prog_key: i32,
    prog_minor: bool,
    prog_7: bool,
    prog_all: bool,
    prog_new: String,
    /// a practice progression on the lead sheet instead of a tune: (title, chart)
    prac_chart: Option<(String, String)>,
    /// a chord sequence playing: (started, seconds a chord, which list, chords)
    seq_play: Option<(Instant, f32, String, usize)>,
    /// the song whose info is being edited: its id, the title and the artist as typed
    edit_info: Option<(i64, String, String)>,
    /// key and BPM being typed in the edit info panel
    edit_kb: (String, String),
    /// which skin groups are open on the BACKLINE page (ORIGINAL, RETRO ORIGINAL, WINAMP)
    skins_open: [bool; 3],
    neck_zoom: f32,
    ghost_alpha: f32,
    /// the analyzer's reading: the one picked (click), the one hovered
    an_pick: Option<String>,
    an_hover: Option<String>,
    shape_cache: HashMap<String, Vec<[Option<u8>; 6]>>,
    /// the bar being edited by click in the chart: which bar, and its chords as typed
    bar_edit: Option<(usize, String)>,
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
    /// how loud each part plays (bass, chords, drums), 0 to 1.5
    band_lv: [f32; 3],
    /// times through the chart before the band stops (0 = until you stop it)
    band_loops: u32,
    /// the band's parts as last rendered (for which chart, tempo and style), so a level change only remixes
    band_stems: Option<(u64, std::sync::Arc<[Vec<f32>; 3]>)>,
    band_t0: Instant,
    band_lead: f32,
    band_bar: f32,
    /// seconds a beat, and where each bar of band_order starts (in beats, the total last)
    band_beat: f32,
    band_starts: Vec<f32>,
    band_order: Vec<usize>,
    band_sig: u64,
    band_chart: String,
    pomo_sound: bool,
    lib_frac: f32,
    stack_frac: f32,
    /// the practice panel's height (drag the edge above it)
    prac_h: f32,
    /// the queue hidden (the PL button), the player taking the whole column
    queue_hidden: bool,
    /// where each floating window sits (outer rectangle), by name
    /// the first row shown in the Winamp playlist (a fraction while scrolling)
    wa_pl_top: f32,
    /// the playlist font picked (index into winamp_ui::PL_FONTS; 0 = the skin's own)
    pl_font: usize,
    /// equalizer AUTO: remember an EQ per song, and the ones remembered
    eq_auto: bool,
    eq_songs: HashMap<i64, [f32; 10]>,
    /// left / right balance, -100 to 100
    balance: i32,
    ebuf: String,
    ebuf_id: u32,
    chart_tried: std::collections::HashSet<String>,
    /// tunes whose writer and year were looked up this session
    details_tried: std::collections::HashSet<String>,
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
            if keep {
                if let Some(p) = cache::put(id, &b) {
                    crate::api::log(&format!("youtube {}: saved to {}", vid, p.display()));
                }
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
        // the installed-font list (Winamp skins name their font) is read in the background, never on screen
        std::thread::spawn(winamp_ui::warm_fonts);
        // smooth text (the sleek skins, tooltips) in Chakra Petch: retro and techy without being pixel text
        // (SIL Open Font License, see assets/ChakraPetch-OFL.txt)
        {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("chakra".to_string(), egui::FontData::from_static(include_bytes!("../assets/ChakraPetch-Regular.ttf")));
            if let Some(f) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                f.insert(0, "chakra".to_string());
            }
            cc.egui_ctx.set_fonts(fonts);
        }
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
            images: Images { map: HashMap::new(), requested: HashSet::new(), req, avg: HashMap::new(), hold: HashMap::new() },
            _font_tex: font_tex,
            auth: Auth::LoggedOut,
            login_code: None,
            login_err: String::new(),
            page: None,
            lib_page: None,
            art_preview: None,
            wa_loading: None,
            back: Vec::new(),
            fwd: Vec::new(),
            credits: None,
            artist_pick: None,
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
            show_log: false,
            audio_err_shown: false,
            art_view: false,
            show_spec: true,
            spec_op: 0.2,
            spec_h: 0.42,
            spec_w: 0.6,
            spec_frame: 1,
            viz_panel: None,
            show_winamp: false,
            show_tl: false,
            hist_back: Vec::new(),
            hist_fwd: Vec::new(),
            hist_at: None,
            wa_q: String::new(),
            wa_list: Vec::new(),
            wa_busy: false,
            wa_end: false,
            wa_worn: None,
            wa_recent: Vec::new(),
            wa_art: None,
            art_border: 1,
            cap_size: 1,
            cap_dark: false,
            bg_now: None,
            out_device: None,
            out_list: Vec::new(),
            art_queue: false,
            viz: [viz::VizCfg::player(), viz::VizCfg::art()],
            viz_wave: vec![0.0; WAVE_N],
            show_lyrics: true,
            fullscreen: false,
            fs_by_art: false,
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
            eq_pre: 0.0,
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
            // tracks are streamed, not kept; saving them to disk is switched on by hand
            keep_cache: false,
            cache_stats: (0, 0),
            cache_at: Instant::now(),
            store: store::Store::load(),
            store_dirty: false,
            store_saved: Instant::now(),
            srcmap: HashMap::new(),
            sec: Sec::Tidal,
            last_music: Sec::Tidal,
            last_practice: Sec::Tunes,
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
            upd_note: String::new(),
            upd_state: 0,
            last_active: Instant::now(),
            show_help: false,
            palette_open: false,
            palette_q: String::new(),
            palette_sel: 0,
            palette_frame: 0,
            menu_keep: None,
            fs_cmd_at: None,
            art_op: 0.9,
            show_prefs: false,
            auto_restart: false,
            intro: 1,
            intro_file: None,
            binds: prefs::default_binds(),
            rebinding: None,
            bind_note: String::new(),
            tour: None,
            tour_restore: None,
            tour_play: None,
            tour_seen: false,
            help_tab: 0,
            tuning: None,
            tuning_busy: false,
            upd_checked: None,
            new_pl: String::new(),
            pl_open: None,
            pl_pick: None,
            yt_results: Vec::new(),
            yt_title: String::new(),
            yt_open: false,
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
            chart_sel: None,
            chart_drag: None,
            chart_edit: false,
            bar_edit: None,
            chords_tab: 0,
            chord_q: "Dm7".to_string(),
            an_frets: [None; 6],
            chord_sp: theory::Spelling { slash: true, ..theory::Spelling::default() },
            chord_details: false,
            cl_key: 0,
            cl_scale: 0,
            cl_view: 0,
            cl_semi: true,
            cl_low: 11,
            cl_bass: 0,
            cl_changes: "Dm7b5 G7 Cmaj7 Gbm7b5 Fm7 Em7 Ebdim7 Dm7 G7 Cmaj7".to_string(),
            cl_chord: 0,
            vc_family: 4,
            vc_root: 0,
            vc_type: 0,
            vc_set: 0,
            pass_pair: 0,
            ln_tab: 0,
            ln_at: 0,
            ln_root: 0,
            ln_back: false,
            ln_about: false,
            ln_how: false,
            sl_div: 0,
            sl_kind: 0,
            sl_pat: 0,
            ln_bpm: 80,
            ln_sub: 2,
            ln_loop: true,
            ln_playing: String::new(),
            sc_fam: 0,
            sc_idx: 0,
            sc_pat: 0,
            sc_oct: 2,
            sc_dir: 2,
            sc_root: 0,
            seq_last: None,
            prog_shown: Vec::new(),
            prog_key: 0,
            prog_minor: false,
            prog_7: true,
            prog_all: false,
            prog_new: String::new(),
            prac_chart: None,
            seq_play: None,
            skins_open: [false; 3],
            edit_info: None,
            edit_kb: (String::new(), String::new()),
            neck_zoom: 1.0,
            ghost_alpha: 0.7,
            an_pick: None,
            an_hover: None,
            shape_cache: HashMap::new(),
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
            band_lv: [1.0; 3],
            band_loops: 0,
            band_stems: None,
            band_t0: Instant::now(),
            band_lead: 0.0,
            band_bar: 2.0,
            band_beat: 0.5,
            band_starts: Vec::new(),
            band_order: Vec::new(),
            band_sig: 0,
            band_chart: String::new(),
            pomo_sound: false,
            lib_frac: 0.34,
            stack_frac: 1.0,
            prac_h: 300.0,
            queue_hidden: false,
            wa_pl_top: 0.0,
            pl_font: 0,
            eq_auto: false,
            eq_songs: HashMap::new(),
            balance: 0,
            ebuf: String::new(),
            ebuf_id: 0,
            chart_tried: Default::default(),
            details_tried: Default::default(),
            chart_rn: true,
            ireal_in: String::new(),
            tunes_ireal: String::new(),
            look_q: String::new(),
            chart_cache: (String::new(), chart::Chart::default()),
        };
        meta::load(&app.store.meta);
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
        // older saves had one shared set: both visualizers start from it
        for k in 0..2 {
            app.viz[k].load_old(&st);
            if let Some(v) = st["viz"].get(k) {
                app.viz[k].load(v);
            }
        }
        if let Some(z) = st["zoom"].as_f64() {
            app.ctx.set_zoom_factor((z as f32).clamp(0.8, 2.0));
        }
        if let Some(f) = st["art_op"].as_f64() {
            app.art_op = (f as f32).clamp(0.5, 1.0);
        }
        skin::LOOP_SAFE.store(st["loop_safe"].as_bool().unwrap_or(false), Ordering::Relaxed);
        skin::CALM.store(st["reduce_motion"].as_bool().unwrap_or(false), Ordering::Relaxed);
        if let Some(z) = st["neck_zoom"].as_f64() {
            app.neck_zoom = (z as f32).clamp(0.8, 2.2);
        }
        if let Some(a) = st["ghost_alpha"].as_f64() {
            app.ghost_alpha = (a as f32).clamp(0.1, 1.0);
        }
        if let Some(v) = st["chord_sp"].as_array() {
            let b = |i: usize, d: bool| v.get(i).and_then(|x| x.as_bool()).unwrap_or(d);
            app.chord_sp =
                theory::Spelling { symbols: b(0, false), sharps: b(1, false), detailed: b(2, false), slash: b(3, true) };
        }
        if let Some(m) = st["intro"].as_u64() {
            app.intro = (m as u8).min(2);
        }
        app.intro_file = st["intro_file"].as_str().map(|s| s.to_string());
        if let Some(b) = st["auto_restart"].as_bool() {
            app.auto_restart = b;
        }
        if st.get("binds").is_some() {
            app.binds = prefs::binds_from_json(&st["binds"]);
        }
        if let Some(b) = st["tour_seen"].as_bool() {
            app.tour_seen = b;
        }
        if let Some(d) = st["out_device"].as_str() {
            app.out_device = Some(d.to_string());
            app.player.send(Cmd::Device(app.out_device.clone()));
        }
        if let Some(b) = st["cap_size"].as_u64() {
            app.cap_size = (b as u8).min(2);
        }
        if let Some(b) = st["cap_dark"].as_bool() {
            app.cap_dark = b;
        }
        if let Some(b) = st["art_border"].as_u64() {
            app.art_border = (b as u8).min(2);
        }
        // the Winamp skin worn last time, from its saved file
        if let Some(list) = st["wa_recent"].as_array() {
            app.wa_recent = list.iter().filter_map(|v| Some((v[0].as_str()?.to_string(), v[1].as_str()?.to_string()))).collect();
        }
        if let Some(b) = st["balance"].as_i64() {
            app.balance = (b as i32).clamp(-100, 100);
            app.player.ctl.set_balance(app.balance);
        }
        if let Some(f) = st["pl_font"].as_u64() {
            app.pl_font = (f as usize).min(winamp_ui::PL_FONTS.len() - 1);
        }
        app.eq_auto = st["eq_auto"].as_bool().unwrap_or(false);
        // the skin worn last time (SLEEK unless PLAIN was picked), put on before the first frame so the window
        // opens already wearing it, with no glimpse of the plain look
        let gen = match st["winamp"][0].as_str() {
            Some(m) => m.strip_prefix("gen:").and_then(|g| g.parse::<usize>().ok()),
            None if st["plain"].as_bool() != Some(true) => Some(2),
            None => None,
        };
        if let Some(g) = gen.filter(|g| *g > 0) {
            let n = SKIN.load(Ordering::Relaxed) % PALS.len();
            let look = (g - 1).min(genskin::LOOKS.len() - 1);
            let s = app.gen_skin(n, look);
            app.wear_skin(genskin::wear(&PALS[n], look).map(|(p, art)| (s, p, art)), false);
        } else if let (Some(md5), Some(name)) = (st["winamp"][0].as_str(), st["winamp"][1].as_str()) {
            let s = winamp::WaSkin { md5: md5.to_string(), name: name.to_string(), shot: String::new(), download: String::new() };
            if let Some(b) = winamp::load_saved(&s.md5) {
                app.wear_skin(winamp::load(&b).map(|(p, art)| (s, p, art)), false);
            }
        }
        if let Some(m) = st["eq_songs"].as_object() {
            for (k, v) in m {
                let g: Vec<f32> =
                    v.as_array().map(|a| a.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect()).unwrap_or_default();
                if let (Ok(id), true) = (k.parse::<i64>(), g.len() == 10) {
                    app.eq_songs.insert(id, [g[0], g[1], g[2], g[3], g[4], g[5], g[6], g[7], g[8], g[9]]);
                }
            }
        }

        if let Some(f) = st["spec_frame"].as_u64() {
            app.spec_frame = (f as u8).min(1);
        }
        if let Some(f) = st["spec_w"].as_f64() {
            app.spec_w = (f as f32).clamp(0.2, 1.0);
        }
        if let Some(f) = st["spec_h"].as_f64() {
            app.spec_h = (f as f32).clamp(0.15, 0.9);
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
        app.eq_pre = st["eq_pre"].as_f64().map_or(0.0, |v| (v as f32).clamp(-12.0, 12.0));
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
            app.stack_frac = (f as f32).clamp(0.1, 1.0);
        }
        if let Some(h) = st["prac_h"].as_f64() {
            app.prac_h = (h as f32).clamp(220.0, 900.0);
        }
        if let Some(c) = st["cols"].as_u64() {
            // settings from before the KEY / BPM column: show it once, it can be turned off from the header
            let new = if st["cols_v"].as_u64().unwrap_or(1) < 2 { 0b1000 } else { 0 };
            COLS.store(c as u32 | new, Ordering::Relaxed);
        }
        if let Some(v) = st["vinyl"].as_bool() {
            VINYL.store(v, Ordering::Relaxed);
        }
        if let Some(w) = st["col_w"].as_array() {
            let w: Vec<f32> = w.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect();
            if w.len() == 3 && w.iter().all(|x| *x > 0.01) {
                *COL_W.lock().unwrap() = [w[0], w[1], w[2]];
            }
        }
        if let Some(b) = st["pomo_sound"].as_bool() {
            app.pomo_sound = b;
        }
        if let Some(p) = st["speed"].as_u64() {
            app.speed = (p as u32).clamp(25, 250);
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
        crate::api::log(&format!("backline {} started (token saved: {})", VERSION, app.api.has_token()));
        app.play_intro();
        if app.api.has_token() {
            // the app opens straight away; the saved session is checked behind it (the library shows LOADING
            // meanwhile), and only an expired one brings up the sign-in screen
            app.auth = Auth::In;
            app.loading = true;
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
        self.player.eq.set(self.eq_gains, self.eq_pre, self.eq_on);
    }

    fn save_settings(&mut self) {
        let v = serde_json::json!({
            "skin": SKIN.load(Ordering::Relaxed),
            "volume": self.volume,
            "lossless": self.prefer_lossless,
            "spec": self.show_spec,
            "tour_seen": self.tour_seen,
            "auto_restart": self.auto_restart,
            "intro": self.intro,
            "neck_zoom": self.neck_zoom,
            "ghost_alpha": self.ghost_alpha,
            "chord_sp": [self.chord_sp.symbols, self.chord_sp.sharps, self.chord_sp.detailed, self.chord_sp.slash],
            "loop_safe": skin::LOOP_SAFE.load(Ordering::Relaxed),
            "reduce_motion": skin::calm(),
            "intro_file": self.intro_file,
            "art_op": self.art_op,
            "zoom": self.ctx.zoom_factor(),
            "binds": prefs::binds_to_json(&self.binds),
            "spec_op": self.spec_op,
            "spec_h": self.spec_h,
            "spec_w": self.spec_w,
            "spec_frame": self.spec_frame,
            "pl_font": self.pl_font,
            "eq_auto": self.eq_auto,
            "eq_songs": self.eq_songs,
            "balance": self.balance,
            "winamp": self.wa_worn,
            "wa_recent": self.wa_recent,
            "plain": self.wa_worn.is_none(),
            "art_border": self.art_border,
            "cap_size": self.cap_size,
            "cap_dark": self.cap_dark,
            "out_device": self.out_device,
            "viz": [self.viz[0].save(), self.viz[1].save()],
            "lyrics": self.show_lyrics,
            "repeat": match self.repeat { Repeat::Off => 0, Repeat::All => 1, Repeat::One => 2 },
            "eq_on": self.eq_on,
            "eq": self.eq_gains.to_vec(),
            "eq_pre": self.eq_pre,
            "keep_cache": self.keep_cache,
            "speed": self.speed,
            "lib_frac": self.lib_frac,
            "offline": self.offline,
            "sc_browser": sources::SC_BROWSER.load(Ordering::Relaxed),
            "stack_frac": self.stack_frac,
            "prac_h": self.prac_h,
            "pomo_sound": self.pomo_sound,
            "cols": COLS.load(Ordering::Relaxed),
            "cols_v": 2,
            "vinyl": VINYL.load(Ordering::Relaxed),
            "col_w": *COL_W.lock().unwrap(),
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
        self.player.ctl.set_speed(self.speed);
        self.player.ctl.set_semis(if self.practice { self.semis } else { 0 });
        self.player.ctl.set_chan(if self.practice { self.chan } else { 0 });
        self.player.ctl.set_balance(self.balance);
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
        // the saved copy of your library too, unless it is the page on show (already done)
        if let Some(l) = self.lib_page.as_mut() {
            if !self.page.as_ref().is_some_and(|p| Arc::ptr_eq(p, l)) {
                f(l);
            }
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
        self.fwd.clear();
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
                        self.fwd.clear();
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
                        self.start_meta(id, &bytes);
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
                    self.images.trim();
                    self.images.map.insert(url, (tex, Instant::now()));
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
                self.start_meta(id, &bytes);
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
    fn viz_tint(&self, color: u8) -> Option<(Color32, Color32)> {
        if color == 0 {
            if let Some(v) = self.wa_art.as_ref().map(|t| &t.vis).filter(|v| v.len() >= 24) {
                return Some((v[12], v[2]));
            }
        }
        match color {
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

    /// Bands, peaks and wave shaped for one visualizer: its own bar count and sensitivity.
    fn viz_data(&self, i: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let c = self.viz[i];
        let k = c.gain / self.viz[0].gain.max(self.viz[1].gain);
        (viz::resample(&self.bands, c.n, k), viz::resample(&self.peaks, c.n, k), self.viz_wave.iter().map(|v| v * k).collect())
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
        // analysed once at the finer of the two bar counts and the higher sensitivity; each view reshapes it
        let nb = self.viz[0].n.max(self.viz[1].n).clamp(8, 96);
        let gain = self.viz[0].gain.max(self.viz[1].gain);
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
                target[i] = ((db + 64.0 + 23.0 * i as f32 / nb as f32) / 54.0 * gain).clamp(0.0, 1.0);
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
                (best * 1.6 * gain).clamp(-1.0, 1.0)
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

    /// The opening sound, as set in Preferences.
    fn play_intro(&self) {
        let file = match self.intro {
            0 => return,
            2 => self.intro_file.clone(),
            _ => None,
        };
        intro::play(file, self.out_device.clone(), (self.volume * self.volume).max(0.15));
    }

    /// The name card of palette `n` drawn in `look` (genskin).
    fn gen_skin(&self, n: usize, look: usize) -> winamp::WaSkin {
        winamp::WaSkin {
            md5: format!("gen:{}", look + 1),
            name: format!("{} {}", SKIN_NAMES[n], genskin::LOOKS[look]),
            shot: String::new(),
            download: String::new(),
        }
    }

    fn apply(&mut self, a: Action) {
        // going anywhere in the library leaves the side pages (skins, BACKLINE, storage, the log)
        if matches!(
            a,
            Action::Home | Action::Library | Action::Section(_) | Action::Search(_) | Action::Open(_) | Action::GoTo(..)
        ) {
            self.show_winamp = false;
            self.show_tl = false;
            self.show_log = false;
            self.show_cache = false;
        }
        match a {
            Action::StartLogin => self.start_login(),
            Action::Home => {
                self.sec = Sec::Tidal;
                self.back.clear();
                self.fwd.clear();
                self.load(false, |a| a.home());
            }
            Action::Library => {
                self.sec = Sec::Tidal;
                match self.lib_page.clone() {
                    // already loaded: shown straight away, no waiting
                    Some(p) => {
                        self.page = Some(p);
                        self.serial += 1;
                    }
                    None => self.load(false, |a| a.library()),
                }
            }
            Action::ArtPreview(url, title, artist) => {
                if !url.is_empty() {
                    self.art_preview = Some((url, title, artist));
                }
            }
            Action::RefreshLibrary => {
                self.sec = Sec::Tidal;
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
            Action::GoTo(id, to) => {
                self.sec = Sec::Tidal;
                self.load(true, move |a| a.track_page(id, to))
            }
            Action::PickArtist(list) => self.artist_pick = Some(list),
            Action::Credits(id, label) => {
                self.credits = Some((label, None));
                let (api, tx, ctx) = (self.api.clone(), self.tx.clone(), self.ctx.clone());
                std::thread::spawn(move || {
                    let _ = tx.send(Msg::Credits(api.credits(id)));
                    ctx.request_repaint();
                });
            }
            // one history for the whole library: every tab, page, tune and side page (history.rs)
            Action::Back => self.hist_go(true),
            Action::EditInfo(id, title, artist) => {
                let m = meta::get(id).unwrap_or_default();
                self.edit_kb = (
                    m.key.map_or(String::new(), |k| meta::key_short(k).to_string()),
                    m.bpm.map_or(String::new(), |b| b.to_string()),
                );
                self.edit_info = Some((id, title, artist));
                self.ed.id = 0;
            }
            Action::SaveInfo(id, title, artist) => self.save_info(id, title, artist),
            Action::PlaySequence(chords) => {
                if !chords.is_empty() {
                    self.player.send(Cmd::Once(band::sequence_sound(&chords, 0.9), 0.9));
                }
            }
            Action::PlaySequenceAt(chords, gap, tag) => {
                if !chords.is_empty() {
                    self.player.send(Cmd::StopOnce);
                    // a single-note line is played short and clean; chords ring into each other
                    let pcm = if tag == "line" { band::line_sound(&chords, gap) } else { band::sequence_sound(&chords, gap) };
                    self.player.send(Cmd::Once(pcm, 0.9));
                    self.seq_last = Some(chords.clone());
                    self.seq_play = Some((Instant::now(), gap, tag, chords.len()));
                }
            }
            Action::StopSequence => {
                self.player.send(Cmd::StopOnce);
                self.seq_play = None;
            }
            Action::PlayNotes(notes) => {
                if !notes.is_empty() {
                    self.player.send(Cmd::Once(band::chord_sound(&notes), 0.9));
                }
            }
            Action::ShowLog => {
                self.show_log = true;
                self.show_tl = false;
                self.show_winamp = false;
                self.show_cache = false;
                self.show_eq = false;
            }
            Action::BacklinePage => {
                self.show_tl = true;
                self.show_winamp = false;
                self.show_log = false;
                self.show_cache = false;
            }
            Action::ToggleVinyl => {
                let on = !VINYL.fetch_xor(true, Ordering::Relaxed);
                if on {
                    // a flick: it spins fast and winds down to its normal speed
                    let id = egui::Id::new("vinyl_angle");
                    let ang: f32 = self.ctx.data(|d| d.get_temp::<(f32, f32)>(id)).map_or(0.0, |s| s.0);
                    self.ctx.data_mut(|d| d.insert_temp(id, (ang, 14.0f32)));
                }
                self.dirty = true;
            }
            Action::SetDevice(d) => {
                self.out_device = d.clone();
                self.dirty = true;
                self.player.send(Cmd::Device(d));
                // the song starts again on the new output, where it was (paused stays paused)
                if let (Some(i), Some(t)) = (self.cur, self.cur_track()) {
                    if !self.stopped {
                        let bytes = match self.srcmap.get(&t.id) {
                            Some(Src::File(p)) => std::fs::read(p).ok(),
                            _ => cache::get(t.id),
                        };
                        match bytes {
                            Some(b) => self.player.send(Cmd::Swap(b, self.pos(), self.paused)),
                            None => self.play_index(i),
                        }
                    }
                }
                self.set_note("SOUND OUTPUT CHANGED");
            }
            Action::WaOpen => {
                self.show_winamp = !self.show_winamp;
                self.show_tl = false;
                self.show_log = false;
                if self.show_winamp {
                    self.show_log = false;
                    self.show_eq = false;
                    self.show_cache = false;
                    if self.wa_list.is_empty() && !self.wa_busy {
                        self.apply(Action::WaSearch);
                    }
                }
            }
            Action::WaSearch | Action::WaMore => {
                let more = matches!(a, Action::WaMore);
                if self.wa_busy || (more && self.wa_end) {
                    return;
                }
                self.wa_busy = true;
                if !more {
                    self.wa_list.clear();
                    self.wa_end = false;
                }
                let (q, from) = (self.wa_q.clone(), if more { self.wa_list.len() } else { 0 });
                let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
                std::thread::spawn(move || {
                    let _ = tx.send(Msg::WaList(winamp::list(&q, from), more));
                    ctx.request_repaint();
                });
            }
            Action::WaApply(s) => {
                self.wa_loading = Some(s.name.clone());
                let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
                std::thread::spawn(move || {
                    let r = winamp::fetch(&s).and_then(|b| winamp::load(&b)).map(|(p, art)| (s, p, art));
                    let _ = tx.send(Msg::WaSkin(r));
                    ctx.request_repaint();
                });
            }
            Action::WaForget(md5) => {
                self.wa_recent.retain(|r| r.0 != md5);
                self.dirty = true;
            }
            Action::Intro(m) => match m {
                3 => {
                    let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
                    std::thread::spawn(move || {
                        let f = rfd::FileDialog::new()
                            .set_title("A short sound for when Backline opens")
                            .add_filter("Audio", &sources::AUDIO_EXT)
                            .pick_file();
                        let _ = tx.send(Msg::IntroFile(f.map(|p| p.to_string_lossy().to_string())));
                        ctx.request_repaint();
                    });
                }
                4 => self.play_intro(),
                m => {
                    self.intro = m.min(2);
                    self.dirty = true;
                }
            },
            Action::Theme(n, g) => {
                if g == 0 {
                    self.wa_worn = None;
                    self.apply(Action::SkinSet(n));
                } else {
                    SKIN.store(n % PALS.len(), Ordering::Relaxed);
                    self.dirty = true;
                    self.apply(Action::WaGen(g));
                }
            }
            Action::WaGen(g) => {
                if g == 0 {
                    self.apply(Action::SkinSet(SKIN.load(Ordering::Relaxed)));
                    return;
                }
                let n = SKIN.load(Ordering::Relaxed) % PALS.len();
                let look = (g - 1).min(genskin::LOOKS.len() - 1);
                let s = self.gen_skin(n, look);
                let _ = self.tx.send(Msg::WaSkin(genskin::wear(&PALS[n], look).map(|(p, art)| (s, p, art))));
                self.ctx.request_repaint();
            }
            Action::Balance(b) => {
                self.balance = b.clamp(-100, 100);
                self.player.ctl.set_balance(self.balance);
                self.dirty = true;
            }
            Action::QueueOp(op) => {
                let cur_id = self.cur_track().map(|t| t.id);
                let mut v: Vec<Track> = (*self.queue).clone();
                match op {
                    0 => shuffle_vec(&mut v),
                    1 => v.sort_by_key(|t| (t.artist.to_lowercase(), t.title.to_lowercase())),
                    2 => v.sort_by_key(|t| t.title.to_lowercase()),
                    _ => v.reverse(),
                }
                self.cur = cur_id.and_then(|id| v.iter().position(|t| t.id == id));
                self.queue = Arc::new(v);
                self.orig_queue = None;
                self.set_note(["QUEUE SHUFFLED", "SORTED BY ARTIST", "SORTED BY TITLE", "QUEUE REVERSED"][op.min(3) as usize]);
            }
            Action::PlFont(i) => {
                self.pl_font = i;
                self.dirty = true;
                self.apply_pl_font();
            }
            Action::Forward => self.hist_go(false),
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
            Action::SkinSet(n) if gen_look(self.wa_worn.as_ref()) > 0 => {
                // in the Winamp style: draw the new palette's skin
                let g = gen_look(self.wa_worn.as_ref());
                SKIN.store(n % PALS.len(), Ordering::Relaxed);
                self.dirty = true;
                self.apply(Action::WaGen(g));
            }
            Action::SkinSet(n) => {
                skin::set_custom(None);
                self.wa_worn = None;
                self.wa_art = None;
                winamp_ui::set_chrome(None);
                let n = n % PALS.len();
                SKIN.store(n, Ordering::Relaxed);
                setup_style(&self.ctx);
                self.dirty = true;
                self.set_note(&format!("SKIN: {}", SKIN_NAMES[n]));
            }
            Action::Skin if gen_look(self.wa_worn.as_ref()) > 0 => {
                self.apply(Action::SkinSet(SKIN.load(Ordering::Relaxed) + 1));
            }
            Action::Skin => {
                skin::set_custom(None);
                self.wa_worn = None;
                self.wa_art = None;
                winamp_ui::set_chrome(None);
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
            Action::ToggleEq => self.show_eq = !self.show_eq,
            Action::ToggleCacheView => {
                self.show_cache = !self.show_cache;
                if self.show_cache {
                    self.show_tl = false;
                    self.show_winamp = false;
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
                // playback speed is for everyone, listening or practicing
                self.speed = p.clamp(25, 250);
                self.sync_loop();
            }
            Action::ToggleArt => self.art_view = !self.art_view,
            Action::ToggleSpec => {
                self.show_spec = !self.show_spec;
                self.dirty = true;
            }
            Action::ToggleLyrics => {
                self.show_lyrics = !self.show_lyrics;
                self.dirty = true;
            }
            Action::ToggleFullscreen => {
                self.fullscreen = !self.fullscreen;
                self.fs_by_art = self.fullscreen && self.art_view;
                self.fs_cmd_at = Some(Instant::now());
                self.ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
            }
            Action::Logout => {
                self.art_view = false;
                self.player.send(Cmd::Stop);
                self.api.logout();
                self.auth = Auth::LoggedOut;
                self.page = None;
                self.lib_page = None;
                self.back.clear();
                self.fwd.clear();
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
}

// ------------------------------------------------------------------- update
fn panel_frame() -> egui::Frame {
    egui::Frame::none().fill(pal().app_bg).inner_margin(egui::Margin { left: 14.0, right: 14.0, top: 30.0, bottom: 14.0 })
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // This app draws and handles its own buttons and text boxes. egui's own keyboard focus would make Enter / Space
        // "press" whichever button was clicked last (typing a loop time and pressing Enter also hit the + next to it), so drop it.
        if let Some(id) = ctx.memory(|mem| mem.focused()) {
            ctx.memory_mut(|mem| mem.surrender_focus(id));
        }
        font::set_ppp(ctx.pixels_per_point());
        font::set_modern(style() != 0);
        tour::begin_frame();
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
            Some(t) if !self.stopped => format!("{} - {}  |  Backline", t.artists_text(), t.title),
            _ => format!("Backline {}", VERSION),
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
        if COLS_CHANGED.swap(false, Ordering::Relaxed) {
            self.dirty = true;
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
            if !self.search_focus
                && self.ed.id == 0
                && !self.show_help
                && self.credits.is_none()
                && self.viz_panel.is_none()
                && self.artist_pick.is_none()
                && !self.show_prefs
                && !self.palette_open
                && self.tour.is_none()
            {
                let kp = |k: egui::Key| ctx.input(|i| i.key_pressed(k));
                for cmd in self.key_commands(ctx) {
                    self.run_cmd(cmd, &mut acts);
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
            // the mouse's side buttons: back closes the album view, otherwise both walk the page history
            let (m_back, m_fwd) = ctx.input(|i| {
                (i.pointer.button_pressed(egui::PointerButton::Extra1), i.pointer.button_pressed(egui::PointerButton::Extra2))
            });
            if m_back && self.art_view {
                acts.push(Action::ToggleArt);
            } else if m_back {
                acts.push(Action::Back);
            }
            if m_fwd && !self.art_view {
                acts.push(Action::Forward);
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
                // the practice panel's height and the player's are yours to drag (double-click a handle: back to auto)
                let practice_h = if !self.practice { 0.0 } else { self.prac_h.clamp(220.0, (screen.height() * 0.6).max(220.0)) };
                let max_inner_h = ((screen.height() - practice_h) * 0.5 - 44.0).max(120.0);
                let auto_s = (inner_w / 300.0).min(max_inner_h / 138.0).clamp(0.8, 3.0);
                let s = if self.stack_frac >= 0.99 {
                    auto_s
                } else {
                    let want = (self.stack_frac * screen.height() - 44.0).max(0.0);
                    (inner_w / 300.0).min(want / 138.0).clamp(0.6, 3.0)
                };
                let player_h = 138.0 * s + 44.0;

                if !self.focus_mode {
                    egui::SidePanel::left("library")
                        .exact_width(lib_w)
                        .resizable(false)
                        .show_separator_line(false)
                        .frame(panel_frame())
                        .show(ctx, |ui| {
                            // drawn in a child that does not stretch the panel: something too wide for the column
                            // (long words in a wide font) is cut off at the edge instead of pushing the player away
                            let r = ui.max_rect();
                            let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(r));
                            self.library_ui(&mut inner, &mut acts);
                            ui.allocate_rect(r, Sense::hover());
                        });
                }
                // the equalizer docks under the player: the skin's at the player's own scale, or Backline's
                let eq_h = if self.wa_art.is_some() { 116.0 * s + 44.0 + 46.0 } else { 340.0 };
                if self.queue_hidden {
                    // PL off: no queue, the player fills the column (the practice panel and equalizer under it)
                    if self.show_eq {
                        egui::TopBottomPanel::bottom("eq_dock")
                            .exact_height(eq_h)
                            .resizable(false)
                            .show_separator_line(false)
                            .frame(panel_frame())
                            .show(ctx, |ui| self.eq_dock(ui));
                    }
                    if self.practice {
                        egui::TopBottomPanel::bottom("practice")
                            .exact_height(practice_h)
                            .resizable(false)
                            .show_separator_line(false)
                            .frame(panel_frame())
                            .show(ctx, |ui| self.practice_ui(ui, &mut acts));
                    }
                    egui::CentralPanel::default().frame(panel_frame()).show(ctx, |ui| self.player_window(ui, &mut acts));
                } else {
                    egui::TopBottomPanel::top("player")
                        .exact_height(player_h)
                        .resizable(false)
                        .show_separator_line(false)
                        .frame(panel_frame())
                        .show(ctx, |ui| self.player_window(ui, &mut acts));
                    if self.show_eq {
                        egui::TopBottomPanel::top("eq_dock")
                            .exact_height(eq_h)
                            .resizable(false)
                            .show_separator_line(false)
                            .frame(panel_frame())
                            .show(ctx, |ui| self.eq_dock(ui));
                    }
                    // the practice panel sits under the playlist, so player, equalizer and playlist stay one stack
                    if self.practice {
                        egui::TopBottomPanel::bottom("practice")
                            .exact_height(practice_h)
                            .resizable(false)
                            .show_separator_line(false)
                            .frame(panel_frame())
                            .show(ctx, |ui| self.practice_ui(ui, &mut acts));
                    }
                    egui::CentralPanel::default().frame(panel_frame()).show(ctx, |ui| self.playlist_ui(ui, &mut acts));
                }

                // drag handles between the windows: under the player (or the equalizer under it), its height;
                // above the practice panel, the panel's height
                if !self.queue_hidden {
                    let y = screen.min.y + player_h + if self.show_eq { eq_h } else { 0.0 };
                    let r = Rect::from_min_size(Pos2::new(lib_w, y - 7.0), Vec2::new(right_w, 14.0));
                    match splitter(ctx, "split_h", r, false) {
                        Some(Some(py)) => {
                            let ph = (py - screen.min.y - if self.show_eq { eq_h } else { 0.0 }).max(120.0);
                            self.stack_frac = (ph / screen.height()).clamp(0.1, 0.98);
                            self.dirty = true;
                        }
                        Some(None) => {
                            self.stack_frac = 1.0;
                            self.dirty = true;
                        }
                        None => {}
                    }
                }
                if self.practice {
                    let y = screen.max.y - practice_h;
                    let r = Rect::from_min_size(Pos2::new(lib_w, y - 7.0), Vec2::new(right_w, 14.0));
                    match splitter(ctx, "split_p", r, false) {
                        Some(Some(py)) => {
                            self.prac_h = (screen.max.y - py).clamp(220.0, (screen.height() * 0.6).max(220.0));
                            self.dirty = true;
                        }
                        Some(None) => {
                            self.prac_h = 300.0;
                            self.dirty = true;
                        }
                        None => {}
                    }
                }
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
        // the window can also go fullscreen/windowed by itself (the green Mac button, F11): follow it,
        // except for a moment after our own command, while the window reports the old state
        if self.fs_cmd_at.map_or(true, |t| t.elapsed() > Duration::from_millis(1500)) {
            if let Some(f) = ctx.input(|i| i.viewport().fullscreen) {
                self.fullscreen = f;
                self.fs_by_art &= f;
            }
        }
        // left the album view (Esc, A, a mode button...) after going fullscreen in it: back to the window
        if self.fs_by_art && !self.art_view {
            self.apply(Action::ToggleFullscreen);
        }
        #[cfg(target_os = "macos")]
        if self.frames == 3 {
            self.menu_keep = macmenu::install(&self.tx, &self.ctx);
        }
        self.update_banner(ctx, &mut acts);
        self.floating_modes(ctx, &mut acts);
        self.help_overlay(ctx, &mut acts);
        self.credits_overlay(ctx);
        self.art_preview_overlay(ctx);
        self.edit_info_panel(ctx);
        self.skin_loading_panel(ctx);
        self.viz_panel(ctx);
        self.artist_picker(ctx, &mut acts);
        self.palette_overlay(ctx);
        self.prefs_overlay(ctx, &mut acts);
        // first time in: open the help on the tour page
        if !self.tour_seen && (self.auth == Auth::In || self.offline) && self.frames >= 10 {
            self.tour_seen = true;
            self.help_tab = 0;
            self.show_help = true;
            self.dirty = true;
        }
        self.tour_overlay(ctx);
        if self.tour.is_none() && !self.show_prefs && ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::K)) {
            self.open_palette();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            acts.push(Action::ToggleHelp);
        }
        if self.show_help && self.tour.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_help = false;
        }
        for a in acts {
            self.apply(a);
        }
        self.hist_track();
        // keep the library copy current (more songs arrive in pages; likes change it)
        if self.page.as_ref().is_some_and(|p| p.library) {
            self.lib_page = self.page.clone();
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
    v.window_stroke = Stroke::new(1.0_f32, pal().edge);
    let rad = if style() != 0 { 7.0 } else { 0.0 };
    v.window_rounding = Rounding::same(rad);
    v.menu_rounding = Rounding::same(rad);
    v.extreme_bg_color = pal().lcd;
    v.faint_bg_color = pal().beige_dk;
    v.hyperlink_color = pal().ink;
    v.selection.bg_fill = pal().sel;
    v.selection.stroke = Stroke::new(1.0_f32, pal().edge);
    v.widgets.noninteractive.bg_stroke = Stroke::NONE;
    v.widgets.noninteractive.bg_fill = pal().beige;
    v.widgets.noninteractive.weak_bg_fill = pal().beige;
    v.widgets.inactive.bg_fill = pal().beige_dk;
    v.widgets.inactive.weak_bg_fill = pal().beige_dk;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, pal().edge);
    v.widgets.hovered.bg_fill = pal().groove;
    v.widgets.hovered.weak_bg_fill = pal().groove;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, pal().edge);
    v.widgets.active.bg_fill = pal().ink2;
    v.widgets.active.weak_bg_fill = pal().ink2;
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, pal().edge);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.fg_stroke = Stroke::new(1.0_f32, pal().ink);
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
    // the settings folder from when the app had its old name comes across before anything reads or writes it
    api::migrate_config();
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
            .with_title("Backline")
            .with_icon(Arc::new(app_icon()))
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native("Backline", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: i64) -> Track {
        Track { id, title: format!("t{}", id), ..Default::default() }
    }

    #[test]
    fn times() {
        assert_eq!(fmt_time(0.0), "0:00");
        assert_eq!(fmt_time(83.9), "1:23");
        assert_eq!(fmt_time(-5.0), "0:00");
        assert_eq!(fmt_t(83.44), "1:23.4");
        assert_eq!(fmt_t(5.0), "0:05.0");
        assert_eq!(fmt_t(59.96), "1:00.0", "tenths round up into the next minute");
        assert_eq!(fmt_t(-1.0), "0:00.0");
        assert_eq!(fmt_long(59.0), "0:59");
        assert_eq!(fmt_long(3725.0), "1:02:05");
        // what fmt_t shows, the loop-time box reads back
        for s in [0.0, 1.5, 59.9, 83.4, 600.1] {
            assert!((crate::extras::parse_time(&fmt_t(s)).unwrap() - s).abs() < 0.051, "{}", s);
        }
    }

    #[test]
    fn shuffle_keeps_the_chosen_track_first() {
        let list: Vec<Track> = (1..=20).map(track).collect();
        for first in [0, 7, 19, 99] {
            let s = shuffled_from(&list, first);
            assert_eq!(s.len(), 20);
            assert_eq!(s[0].id, list[first.min(19)].id);
            let mut ids: Vec<i64> = s.iter().map(|t| t.id).collect();
            ids.sort();
            assert_eq!(ids, (1..=20).collect::<Vec<_>>(), "every track exactly once");
        }
        assert!(shuffled_from(&[], 0).is_empty());
    }

    #[test]
    fn lcd_digits() {
        // segments a-g; an 8 lights them all, a 1 only the right side
        assert_eq!(seg_mask(8), [1; 7]);
        assert_eq!(seg_mask(1), [0, 1, 1, 0, 0, 0, 0]);
        let lit: Vec<usize> = (0..10).map(|d| seg_mask(d).iter().filter(|&&s| s == 1).count()).collect();
        assert_eq!(lit, vec![6, 2, 5, 5, 4, 5, 6, 3, 7, 6]);
    }

    #[test]
    fn tone_strength() {
        let rate = 8000.0;
        let x: Vec<f32> = (0..4000).map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / rate).sin()).collect();
        assert!((goertzel(&x, 440.0, rate) - 1.0).abs() < 0.05, "a full-scale 440 Hz tone reads about 1");
        assert!(goertzel(&x, 1000.0, rate) < 0.05, "and nothing at 1 kHz");
    }

    #[test]
    fn column_widths_fill_the_space() {
        let saved = COLS.load(Ordering::Relaxed);
        for bits in [0b0000, 0b0001, 0b0010, 0b0011] {
            COLS.store(bits, Ordering::Relaxed);
            let w = col_widths(600.0);
            assert!((w.iter().sum::<f32>() - 600.0).abs() < 0.01, "{:b}", bits);
            assert_eq!(w[1] > 0.0, bits & 1 == 1);
            assert_eq!(w[2] > 0.0, bits & 2 == 2);
        }
        COLS.store(0b1000, Ordering::Relaxed);
        assert_eq!(right_w(), 80.0 + KB_W, "room for KEY / BPM when shown");
        COLS.store(saved, Ordering::Relaxed);
    }

    #[test]
    fn links_to_share() {
        assert_eq!(track_link(&track(42), None).as_deref(), Some("https://tidal.com/browse/track/42"));
        let yt = store::Ext { kind: "yt".into(), src: "dQw4w9WgXcQ".into(), ..Default::default() };
        assert_eq!(track_link(&track(-1), Some(&yt)).as_deref(), Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ"));
        let sc = store::Ext { kind: "yt".into(), src: "https://soundcloud.com/a/b".into(), ..Default::default() };
        assert_eq!(track_link(&track(-2), Some(&sc)).as_deref(), Some("https://soundcloud.com/a/b"));
        let file = store::Ext { kind: "file".into(), src: "C:/a.mp3".into(), ..Default::default() };
        assert_eq!(track_link(&track(-3), Some(&file)), None);
        assert_eq!(track_link(&track(-4), None), None);
    }

    #[test]
    fn key_and_tempo_cell() {
        meta::from_tidal(888001, Some(120), Some(12 + 9));
        assert!(track_cells(888001, "T", "A", "B").ends_with("\tAm|120"));
        meta::set_detected(888002, meta::Info { bpm: Some(95), key: Some(0), ..Default::default() });
        assert!(track_cells(888002, "T", "A", "B").ends_with("\tC?|95"), "detected keys carry a ?");
        assert!(track_cells(888003, "T", "A", "").ends_with("\t|"), "unknown: empty cell");
    }
}
