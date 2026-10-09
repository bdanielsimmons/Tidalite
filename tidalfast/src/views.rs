//! Everything drawn for the practice extras: the FILES / YOUTUBE / TUNES / DIARY lists, the
//! practice panel, the lead sheet and the little text-field widget they share.

use crate::api::Track;
use crate::extras::{Knob, CHAN_NAMES};
use crate::font::{ptext, ptext_fit, spx, text_w};
use crate::sources;
use crate::stems;
use crate::store::{self, Ext};
use crate::tools::{F_LOOK, F_LOOP_A, F_LOOP_B, F_SPEED, F_TR_LOOPS, F_TR_STEP, F_TUNE_IREAL};
use crate::{
    cache, check_box, chip, col_header, col_on, fill_rect, fmt_t, fmt_time, inset, lcd_box, list_row, menu_item, outline, pal,
    para, play_buttons, retro_btn, retro_btn_w, section_header, tab_row, table_header, title_line, track_cells, window_deco,
    Action, App, Ed, RowState, Sec, Tip, BTN_H, PRACTICE,
};
use eframe::egui::{self, Align, Pos2, Rect, Sense, Vec2};
use std::sync::atomic::Ordering;

/// "45 SEC", "12 MIN", "1 H 05 MIN"
/// 0 = a file, 2 = SoundCloud, 1 = YouTube.
fn heart_group(e: &Ext) -> u8 {
    if e.kind == "file" {
        0
    } else if e.src.contains("soundcloud.com") {
        2
    } else {
        1
    }
}

fn mins_text(secs: u32) -> String {
    match secs {
        0..=59 => format!("{} SEC", secs),
        60..=3599 => format!("{} MIN", secs / 60),
        _ => format!("{} H {:02} MIN", secs / 3600, secs % 3600 / 60),
    }
}

fn src_is_link(s: &str) -> bool {
    s.starts_with("http")
}

// text-field ids (only one field has the keyboard at a time)
const F_NEW_TUNE: u32 = 1;
const F_T_NAME: u32 = 2;
const F_T_KEY: u32 = 3;
const F_T_NOTES: u32 = 4;
const F_V_NOTE: u32 = 5;
const F_YT: u32 = 6;
const F_TUNE: u32 = 7;
const F_MINS: u32 = 8;
const F_BPM: u32 = 9;
const F_NOTE: u32 = 10;
const F_SEC: u32 = 11;
pub(crate) const F_CHART: u32 = 12;
pub(crate) const F_IREAL: u32 = 13;
const F_SC: u32 = 14;

// -------------------------------------------------------------- text field
pub struct FieldOut {
    pub changed: bool,
    pub enter: bool,
}

/// Wrapped line ranges (in characters) of a multi-line text.
fn layout_lines(chars: &[char], px: f32, w: f32) -> Vec<(usize, usize)> {
    let mut lines = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut last_space: Option<usize> = None;
    while i < chars.len() {
        if chars[i] == '\n' {
            lines.push((start, i));
            i += 1;
            start = i;
            last_space = None;
            continue;
        }
        let seg: String = chars[start..=i].iter().collect();
        if text_w(&seg, px) > w && i > start {
            let brk = match last_space {
                Some(sp) if sp > start => sp + 1,
                _ => i,
            };
            lines.push((start, brk));
            start = brk;
            i = brk;
            last_space = None;
            continue;
        }
        if chars[i] == ' ' {
            last_space = Some(i);
        }
        i += 1;
    }
    lines.push((start, chars.len()));
    lines
}

fn caret_x(chars: &[char], s: usize, cur: usize, px: f32) -> f32 {
    if cur <= s {
        return 0.0;
    }
    let pre: String = chars[s..cur.min(chars.len())].iter().collect();
    text_w(&pre, px) + spx(px)
}

fn line_of(lines: &[(usize, usize)], cur: usize) -> usize {
    let mut li = 0;
    for (k, (s, _)) in lines.iter().enumerate() {
        if *s <= cur {
            li = k;
        }
    }
    li
}

/// One text field drawn in the pixel font (single line, or several wrapped lines).
/// Edits are written straight into `text`.
pub fn field(ui: &mut egui::Ui, ed: &mut Ed, id: u32, text: &mut String, rect: Rect, hint: &str, multi: bool) -> FieldOut {
    let px = 2.0;
    let lh = 7.0 * spx(px) + 6.0;
    let inner = rect.shrink2(Vec2::new(8.0, 3.0));
    let mut out = FieldOut { changed: false, enter: false };
    let resp = ui.interact(rect, ui.id().with(("fld", id)), Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
    }
    let mut chars: Vec<char> = text.chars().collect();
    let focused = ed.id == id;
    ed.cur = if focused { ed.cur.min(chars.len()) } else { ed.cur };

    // scroll offsets that keep the caret visible (only while it has the keyboard)
    let offsets = |chars: &[char], cur: usize, on: bool| -> (Vec<(usize, usize)>, f32, f32) {
        let lines = if multi { layout_lines(chars, px, inner.width()) } else { vec![(0, chars.len())] };
        if !on {
            return (lines, 0.0, 0.0);
        }
        let li = line_of(&lines, cur);
        if multi {
            let voff = ((li as f32 + 1.0) * lh - inner.height()).max(0.0);
            (lines, 0.0, voff)
        } else {
            let cx = caret_x(chars, 0, cur, px);
            let hoff = (cx - (inner.width() - 6.0)).max(0.0);
            (lines, hoff, 0.0)
        }
    };

    if resp.clicked() {
        let (lines, hoff, voff) = offsets(&chars, ed.cur, focused);
        ed.id = id;
        if let Some(pp) = resp.interact_pointer_pos() {
            let li = if multi { (((pp.y - inner.min.y + voff) / lh).floor().max(0.0) as usize).min(lines.len() - 1) } else { 0 };
            let (s, e) = lines[li];
            let x = pp.x - inner.min.x + hoff;
            let mut best = s;
            let mut best_d = f32::MAX;
            for k in s..=e {
                let w = caret_x(&chars, s, k, px);
                let d = (w - x).abs();
                if d < best_d {
                    best_d = d;
                    best = k;
                }
            }
            ed.cur = best;
        } else {
            ed.cur = chars.len();
        }
    } else if focused && ui.input(|i| i.pointer.primary_pressed()) && !resp.hovered() {
        ed.id = 0;
    }

    if ed.id == id {
        let events = ui.input(|i| i.events.clone());
        let mut cur = ed.cur.min(chars.len());
        let mut edited = false;
        for ev in events {
            match ev {
                egui::Event::Text(t) | egui::Event::Paste(t) => {
                    for c in t.chars() {
                        if c == '\n' || c == '\r' {
                            if multi {
                                chars.insert(cur, '\n');
                                cur += 1;
                                edited = true;
                            }
                        } else if !c.is_control() {
                            chars.insert(cur, c);
                            cur += 1;
                            edited = true;
                        }
                    }
                }
                egui::Event::Key { key, pressed: true, modifiers, .. } => match key {
                    egui::Key::Backspace => {
                        if modifiers.ctrl {
                            while cur > 0 && chars[cur - 1] == ' ' {
                                chars.remove(cur - 1);
                                cur -= 1;
                                edited = true;
                            }
                            while cur > 0 && chars[cur - 1] != ' ' && chars[cur - 1] != '\n' {
                                chars.remove(cur - 1);
                                cur -= 1;
                                edited = true;
                            }
                        } else if cur > 0 {
                            chars.remove(cur - 1);
                            cur -= 1;
                            edited = true;
                        }
                    }
                    egui::Key::Delete => {
                        if cur < chars.len() {
                            chars.remove(cur);
                            edited = true;
                        }
                    }
                    egui::Key::ArrowLeft => cur = cur.saturating_sub(1),
                    egui::Key::ArrowRight => cur = (cur + 1).min(chars.len()),
                    egui::Key::ArrowUp | egui::Key::ArrowDown if multi => {
                        let lines = layout_lines(&chars, px, inner.width());
                        let li = line_of(&lines, cur);
                        let col = cur - lines[li].0;
                        let to =
                            if key == egui::Key::ArrowUp { li.checked_sub(1) } else { Some(li + 1).filter(|n| *n < lines.len()) };
                        if let Some(t) = to {
                            let (s, e) = lines[t];
                            cur = (s + col).min(e);
                        }
                    }
                    egui::Key::Home => {
                        cur = if multi {
                            layout_lines(&chars, px, inner.width())[line_of(&layout_lines(&chars, px, inner.width()), cur)].0
                        } else {
                            0
                        };
                    }
                    egui::Key::End => {
                        cur = if multi {
                            let l = layout_lines(&chars, px, inner.width());
                            l[line_of(&l, cur)].1
                        } else {
                            chars.len()
                        };
                    }
                    egui::Key::Enter => {
                        if multi {
                            chars.insert(cur, '\n');
                            cur += 1;
                            edited = true;
                        } else {
                            out.enter = true;
                        }
                    }
                    egui::Key::Escape => ed.id = 0,
                    _ => {}
                },
                _ => {}
            }
        }
        ed.cur = cur;
        if edited {
            *text = chars.iter().collect();
            out.changed = true;
        }
    }

    // ---- draw
    let focused = ed.id == id;
    let (lines, hoff, voff) = offsets(&chars, ed.cur, focused);
    let p = ui.painter();
    inset(p, rect, pal().lcd);
    if focused {
        outline(p, rect, 1.0, pal().ink2);
    }
    let cp = p.with_clip_rect(inner);
    if chars.is_empty() {
        ptext(&cp, Pos2::new(inner.min.x, inner.min.y + lh / 2.0), Align::Min, hint, px, pal().dim);
    }
    for (li, (s, e)) in lines.iter().enumerate() {
        let y = inner.min.y - voff + li as f32 * lh + lh / 2.0;
        if y < inner.min.y - lh || y > inner.max.y + lh {
            continue;
        }
        let line: String = chars[*s..(*e).min(chars.len())].iter().collect();
        let cy = if multi { y } else { rect.center().y };
        ptext(&cp, Pos2::new(inner.min.x - hoff, cy), Align::Min, &line, px, pal().ink);
    }
    if focused && (ui.input(|i| i.time) * 2.0) as i64 % 2 == 0 {
        let li = line_of(&lines, ed.cur);
        let (s, _) = lines[li];
        let x = inner.min.x - hoff + caret_x(&chars, s, ed.cur, px);
        let cy = if multi { inner.min.y - voff + li as f32 * lh + lh / 2.0 } else { rect.center().y };
        fill_rect(&cp, Rect::from_min_size(Pos2::new(x, cy - 8.0), Vec2::new(spx(2.0), 16.0)), pal().ink);
    }
    out
}

/// "LABEL  [ text field ]" on one row (or a taller multi-line box).
pub(crate) fn field_row(
    ui: &mut egui::Ui,
    ed: &mut Ed,
    id: u32,
    label: &str,
    text: &mut String,
    hint: &str,
    tall: Option<f32>,
) -> FieldOut {
    let h = tall.unwrap_or(BTN_H);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
    let ly = if tall.is_some() { rect.min.y + 13.0 } else { rect.center().y };
    ptext(ui.painter(), Pos2::new(rect.min.x + 4.0, ly), Align::Min, label, 2.0, pal().ink2);
    let fr = Rect::from_min_max(Pos2::new(rect.min.x + 76.0, rect.min.y), rect.max);
    field(ui, ed, id, text, fr, hint, tall.is_some())
}

pub(crate) fn label(ui: &mut egui::Ui, text: &str, w: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::hover());
    ptext(ui.painter(), Pos2::new(rect.min.x + 2.0, rect.center().y), Align::Min, text, 2.0, pal().ink2);
}

pub(crate) fn dim_line(ui: &mut egui::Ui, text: &str, px: f32, col: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 7.0 * spx(px) + 6.0), Sense::hover());
    ptext_fit(ui.painter(), Pos2::new(rect.min.x + 6.0, rect.center().y), Align::Min, text, px, rect.width() - 12.0, col);
}

// little 7x7 pictures for the stems: drums, bass (a woofer), other (keys), vocals (a mic)
const STEM_ICONS: [[&str; 7]; 4] = [
    [".#####.", "#######", "#.....#", "#.....#", "#.....#", ".#####.", "#.....#"],
    [".#####.", "#.....#", "#.###.#", "#.###.#", "#.###.#", "#.....#", ".#####."],
    ["#######", "#.#.#.#", "#.#.#.#", "#.#.#.#", "#.#.#.#", "#.#.#.#", "#######"],
    [".#####.", "#######", "#######", ".#####.", "...#...", "...#...", ".#####."],
];

/// Icon button for one stem: pressed in (bright) while the stem is playing.
fn stem_btn(ui: &mut egui::Ui, i: usize, on: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(44.0, BTN_H), Sense::click());
    crate::raised_h(ui.painter(), rect, resp.is_pointer_button_down_on() || on, resp.hovered() && !on);
    let dy = if on { 1.0 } else { 0.0 };
    let o = Pos2::new((rect.center().x - 7.0).round(), (rect.center().y - 7.0 + dy).round());
    crate::pixmap(ui.painter(), o, 2.0, &STEM_ICONS[i][..], if on { pal().ink } else { pal().dim });
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn stars_text(n: u8) -> String {
    let n = n.min(5) as usize;
    format!("{}{}", "*".repeat(n), ".".repeat(5 - n))
}

impl App {
    // ------------------------------------------------------------ switchers
    pub(crate) fn section_bar(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        const ALL: [(Sec, &str); 6] = [
            (Sec::Tidal, "TIDAL"),
            (Sec::Sc, "SOUNDCLOUD"),
            (Sec::Files, "FILES"),
            (Sec::Yt, "YT"),
            (Sec::Tunes, "TUNES"),
            (Sec::Diary, "DIARY"),
        ];
        let secs: Vec<(Sec, &str)> = ALL
            .iter()
            .copied()
            .filter(|s| match s.0 {
                Sec::Tidal => !self.offline,
                Sec::Sc => true,
                _ => PRACTICE,
            })
            .collect();
        if secs.len() < 2 {
            return;
        }
        let names: Vec<&str> = secs.iter().map(|s| s.1).collect();
        let cur = secs.iter().position(|s| s.0 == self.sec).unwrap_or(0);
        if let Some(i) = tab_row(ui, &names, cur) {
            acts.push(Action::Section(secs[i].0));
        }
    }

    /// "Pick up where you left off" in one press.
    pub(crate) fn continue_bar(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        if !PRACTICE || !(self.cur.is_none() || self.stopped) {
            return;
        }
        let Some(l) = &self.store.last else { return };
        let txt = format!(
            "CONTINUE: {} - {}  @ {}  {}%",
            l.version.artist,
            l.version.title,
            fmt_time(l.pos),
            if l.speed == 0 { 100 } else { l.speed }
        );
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
        crate::raised_h(ui.painter(), rect, resp.is_pointer_button_down_on(), resp.hovered());
        ptext_fit(ui.painter(), rect.center(), Align::Center, &txt, 2.0, rect.width() - 16.0, pal().ink);
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            acts.push(Action::Continue);
        }
    }

    fn right_tabs(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        let inner = ui.max_rect();
        let chip_h = 16.0;
        let ty = inner.min.y - 18.0 - chip_h / 2.0;
        let mut right = inner.max.x - 8.0;
        let mut place = |w: f32| {
            let r = Rect::from_min_size(Pos2::new(right - w, ty), Vec2::new(w, chip_h));
            right -= w + 6.0;
            r
        };
        let r_chart = place(48.0);
        let r_queue = place(48.0);
        if chip(ui, r_chart, "CHART", self.rtab == 1, "c_rchart").tip("Chord changes of the open tune").clicked() {
            acts.push(Action::RightTab(1));
        }
        if chip(ui, r_queue, "QUEUE", self.rtab == 0, "c_rqueue").clicked() {
            acts.push(Action::RightTab(0));
        }
    }

    /// Called at the top of the queue window.
    pub(crate) fn right_header(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) -> bool {
        if !PRACTICE {
            return false;
        }
        self.right_tabs(ui, acts);
        if self.rtab == 1 {
            self.chart_ui(ui, acts);
            true
        } else {
            false
        }
    }

    // ---------------------------------------------------------------- FILES
    /// `which`: 0 files, 1 YouTube clips, 2 kept SoundCloud tracks, 3 SoundCloud search results
    fn ext_list(&self, ui: &mut egui::Ui, acts: &mut Vec<Action>, which: u8) {
        let yt = which != 0 && which != 4;
        let hearts: Vec<Ext>;
        let list = match which {
            0 => &self.store.files,
            1 => &self.store.yt,
            2 => &self.store.sc,
            4..=6 => {
                hearts = self.store.hearts.iter().filter(|e| heart_group(e) == which - 4).cloned().collect();
                &hearts
            }
            _ => &self.sc_results,
        };
        let playing = self.cur_track().map(|t| t.id);
        col_header(ui, true);
        for (i, e) in list.iter().enumerate() {
            let state = if playing == Some(e.id) { RowState::Playing } else { RowState::Normal };
            let stored = if yt { cache::has(e.id) } else { true };
            let r = list_row(
                ui,
                i,
                Some(i + 1),
                || track_cells(&e.title, &e.artist, ""),
                || {
                    if e.dur == 0.0 && e.cover.is_empty() && (which == 2 || which == 3 || which == 6) {
                        "(loading info)".to_string()
                    } else if col_on(2) {
                        fmt_time(e.dur)
                    } else {
                        String::new()
                    }
                },
                state,
                false,
                stored,
            );
            if let Some(r) = r {
                if r.clicked() {
                    let tracks: Vec<Track> = list.iter().map(|x: &Ext| x.to_track()).collect();
                    acts.push(Action::Play(tracks, i));
                }
                r.context_menu(|ui| {
                    if menu_item(ui, "Play next") {
                        acts.push(Action::PlayNext(e.to_track()));
                        ui.close_menu();
                    }
                    if menu_item(ui, "Add to queue") {
                        acts.push(Action::Enqueue(e.to_track()));
                        ui.close_menu();
                    }
                    if menu_item(ui, "Add to a tune...") {
                        acts.push(Action::PickTune(e.to_track()));
                        ui.close_menu();
                    }
                    if which >= 4 {
                        if menu_item(ui, "Remove from LIKED") {
                            acts.push(Action::ToggleLike(e.to_track()));
                            ui.close_menu();
                        }
                    } else if which == 3 {
                        if menu_item(ui, "Keep in my SoundCloud list") {
                            acts.push(Action::ScKeep(e.id));
                            ui.close_menu();
                        }
                    } else if menu_item(ui, "Remove from this list") {
                        acts.push(Action::RemoveExt(e.id));
                        ui.close_menu();
                    }
                });
            }
        }
    }

    /// The hearts you gave to this kind of source (0 files, 1 YouTube, 2 SoundCloud); stored on this PC only.
    fn liked_block(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>, group: u8) {
        let n = self.store.hearts.iter().filter(|e| heart_group(e) == group).count();
        if n == 0 {
            return;
        }
        section_header(ui, "LIKED (ON THIS PC)");
        let tracks: Vec<Track> = self.store.hearts.iter().filter(|e| heart_group(e) == group).map(|e| e.to_track()).collect();
        ui.add_space(4.0);
        play_buttons(ui, &tracks, acts);
        ui.add_space(4.0);
        self.ext_list(ui, acts, 4 + group);
    }

    pub(crate) fn files_view(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        ui.add_space(6.0);
        title_line(ui, "MY FILES", 3.0, pal().ink);
        title_line(ui, &format!("{} files - played from where they are", self.store.files.len()), 2.0, pal().ink2);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if retro_btn(ui, "ADD FILES", false).clicked() {
                acts.push(Action::AddFiles);
            }
            if retro_btn(ui, "ADD FOLDER", false).clicked() {
                acts.push(Action::AddFolder);
            }
            if retro_btn(ui, "RESCAN", false).tip("Look for new files in the folders you added").clicked() {
                acts.push(Action::Rescan);
            }
        });
        if self.files_busy {
            dim_line(ui, "READING FILES...", 2.0, pal().ink2);
        }
        if self.store.files.is_empty() && !self.files_busy {
            ui.add_space(6.0);
            para(ui, "Drag audio files or whole folders onto this window, or use the buttons. Nothing is copied - files stay where they are, and loops, speed and sections all work on them.", pal().ink2);
        }
        if !self.store.folders.is_empty() {
            section_header(ui, "WATCHED FOLDERS (CLICK TO STOP)");
            for (i, f) in self.store.folders.iter().enumerate() {
                if let Some(r) = list_row(ui, i, None, || f.clone(), || "FORGET".to_string(), RowState::Normal, false, false) {
                    if r.clicked() {
                        acts.push(Action::ForgetFolder(i));
                    }
                }
            }
        }
        if !self.store.files.is_empty() {
            section_header(ui, "FILES");
            let tracks: Vec<Track> = self.store.files.iter().map(|e| e.to_track()).collect();
            ui.add_space(4.0);
            play_buttons(ui, &tracks, acts);
            ui.add_space(4.0);
            self.ext_list(ui, acts, 0);
        }
        self.liked_block(ui, acts, 0);
        ui.add_space(20.0);
    }

    // -------------------------------------------------------------- YOUTUBE
    pub(crate) fn yt_view(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        ui.add_space(6.0);
        title_line(ui, "YOUTUBE", 3.0, pal().ink);
        title_line(ui, "audio only, saved on this computer for looping", 2.0, pal().ink2);
        ui.add_space(4.0);
        let have = sources::ytdlp_path().is_some();
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(150.0, BTN_H), Sense::hover());
            ptext(
                ui.painter(),
                Pos2::new(rect.min.x + 2.0, rect.center().y),
                Align::Min,
                if have { "YT-DLP: READY" } else { "YT-DLP: MISSING" },
                2.0,
                if have { pal().ink } else { pal().red },
            );
            if retro_btn(ui, if have { "UPDATE YT-DLP" } else { "GET YT-DLP" }, !have)
                .tip("Downloads the free yt-dlp tool from its official GitHub page")
                .clicked()
            {
                acts.push(Action::GetYtDlp);
            }
        });
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 78.0).max(80.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::hover());
            let o = field(ui, &mut self.ed, F_YT, &mut self.yt_in, rect, "Paste a YouTube link...", false);
            if o.enter || retro_btn_w(ui, "ADD", 70.0, false).clicked() {
                acts.push(Action::AddYt);
            }
        });
        if self.yt_busy || !self.yt_msg.is_empty() {
            let col = if self.yt_msg.starts_with("ERROR") || self.yt_msg.starts_with("PRESS") || self.yt_msg.starts_with("THAT") {
                pal().red
            } else {
                pal().ink2
            };
            para(ui, &self.yt_msg.clone(), col);
        }
        if self.store.yt.is_empty() {
            para(ui, "One-time setup: press GET YT-DLP. Then paste a link and press ADD. The first play downloads just the audio (a few MB); after that it is stored and starts instantly, like everything in this player. Play-along videos, lessons, live takes - anything you want to loop and slow down.", pal().ink2);
            para(ui, "Note: downloading from YouTube may go against YouTube's terms. Use it for personal practice, at your own discretion.", pal().dim);
        } else {
            section_header(ui, "SAVED CLIPS");
            let tracks: Vec<Track> = self.store.yt.iter().map(|e| e.to_track()).collect();
            ui.add_space(4.0);
            play_buttons(ui, &tracks, acts);
            ui.add_space(4.0);
            self.ext_list(ui, acts, 1);
        }
        self.liked_block(ui, acts, 1);
        ui.add_space(20.0);
    }

    // ----------------------------------------------------------- SOUNDCLOUD
    pub(crate) fn sc_view(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        ui.add_space(6.0);
        title_line(ui, "SOUNDCLOUD", 3.0, pal().ink);
        title_line(ui, "search it, paste a link, or type a user and press THEIR PLAYLISTS", 2.0, pal().ink2);
        ui.add_space(4.0);
        let have = sources::ytdlp_path().is_some();
        if !have {
            ui.horizontal(|ui| {
                label(ui, "ONE-TIME SETUP", 150.0);
                if retro_btn(ui, "GET YT-DLP", true).tip("Downloads the free yt-dlp tool, which also reads SoundCloud").clicked()
                {
                    acts.push(Action::GetYtDlp);
                }
            });
            if self.yt_busy || !self.yt_msg.is_empty() {
                para(ui, &self.yt_msg.clone(), pal().ink2);
            }
        }
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 78.0).max(80.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::hover());
            let o = field(ui, &mut self.ed, F_SC, &mut self.sc_in, rect, "Search SoundCloud, or paste a link...", false);
            if o.enter || retro_btn_w(ui, "GO", 70.0, false).clicked() {
                acts.push(Action::ScGo);
            }
        });
        ui.horizontal(|ui| {
            if retro_btn(ui, "THEIR PLAYLISTS", false)
                .tip("Type a SoundCloud user name (or paste their profile link) above, then press this to list all their public playlists")
                .clicked()
            {
                acts.push(Action::ScSets);
            }
        });
        ui.horizontal(|ui| {
            let n = sources::SC_BROWSER.load(Ordering::Relaxed) as usize;
            label(ui, "SIGN-IN FROM", 130.0);
            if retro_btn_w(ui, sources::SC_BROWSERS[n], 110.0, n > 0)
                .tip("Use the SoundCloud sign-in of a browser on this PC (private playlists, Go+). Click to change. Firefox works most reliably.")
                .clicked()
            {
                acts.push(Action::ScBrowser);
            }
        });
        if !self.sc_msg.is_empty() {
            let col = if self.sc_msg.starts_with("ERROR") || self.sc_msg.starts_with("PRESS") { pal().red } else { pal().ink2 };
            para(ui, &self.sc_msg.clone(), col);
        }
        if self.sc_results.is_empty() && self.store.sc.is_empty() {
            para(ui, "Your own account: paste your profile's likes link (soundcloud.com/YOU/likes) or a playlist link and press GO - public ones list right here. Click a track to play; the first play stores just its audio so looping and slow-down work like everywhere else.", pal().ink2);
            para(ui, "Note: no sign-in is used, so private and Go+ tracks may play as previews or not at all. Downloading may go against SoundCloud's terms; personal practice use is at your discretion.", pal().dim);
        }
        if !self.sc_sets.is_empty() {
            section_header(ui, "PLAYLISTS (CLICK TO OPEN)");
            for (i, (title, url)) in self.sc_sets.iter().enumerate() {
                if let Some(r) =
                    list_row(ui, i, Some(i + 1), || title.clone(), || "OPEN".to_string(), RowState::Normal, false, false)
                {
                    if r.clicked() {
                        acts.push(Action::ScOpenSet(url.clone()));
                    }
                }
            }
        }
        if !self.sc_results.is_empty() {
            section_header(ui, "RESULTS (RIGHT-CLICK TO KEEP)");
            let tracks: Vec<Track> = self.sc_results.iter().map(|e| e.to_track()).collect();
            ui.add_space(4.0);
            play_buttons(ui, &tracks, acts);
            ui.add_space(4.0);
            self.ext_list(ui, acts, 3);
        }
        if !self.store.sc.is_empty() {
            section_header(ui, "MY SOUNDCLOUD");
            let tracks: Vec<Track> = self.store.sc.iter().map(|e| e.to_track()).collect();
            ui.add_space(4.0);
            play_buttons(ui, &tracks, acts);
            ui.add_space(4.0);
            self.ext_list(ui, acts, 2);
        }
        self.liked_block(ui, acts, 2);
        ui.add_space(20.0);
    }

    // ---------------------------------------------------------------- TUNES
    pub(crate) fn tunes_view(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        if let Some(i) = self.tune_open {
            if i < self.store.tunes.len() && self.pick.is_none() {
                self.tune_detail(ui, acts, i);
                return;
            }
        }
        ui.add_space(6.0);
        if let Some(t) = self.pick.clone() {
            para(ui, &format!("ADD \"{} - {}\" TO WHICH TUNE?", t.artist, t.title), pal().ink);
            ui.horizontal(|ui| {
                if retro_btn(ui, "+ NEW TUNE FROM THIS", false).clicked() {
                    acts.push(Action::NewTuneFrom(t.clone()));
                }
                if retro_btn(ui, "CANCEL", false).clicked() {
                    self.pick = None;
                }
            });
            ui.add_space(6.0);
        } else {
            title_line(ui, "TUNES", 3.0, pal().ink);
            title_line(ui, &format!("{} tunes - your repertoire", self.store.tunes.len()), 2.0, pal().ink2);
            ui.add_space(4.0);
        }
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 78.0).max(80.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::hover());
            let o = field(ui, &mut self.ed, F_NEW_TUNE, &mut self.new_tune, rect, "New tune name...", false);
            if o.enter || retro_btn_w(ui, "ADD", 70.0, false).clicked() {
                acts.push(Action::NewTune);
            }
        });
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), BTN_H), Sense::hover());
        let _ =
            field(ui, &mut self.ed, F_TUNE_IREAL, &mut self.tunes_ireal, rect, "iReal Pro link or playlist: irealb://...", false);
        if retro_btn(ui, "IMPORT IREAL", false)
            .tip("Paste an iReal Pro link or a whole playlist above - every song becomes a tune with its chart")
            .clicked()
        {
            self.ireal_in = std::mem::take(&mut self.tunes_ireal);
            acts.push(Action::ImportIreal(None));
        }
        ui.add_space(4.0);
        if self.store.tunes.is_empty() {
            para(ui, "Your repertoire lives here, with the recordings you study for each tune. Add a recording from any list (right-click a song, then Add to a tune), or play one and press ADD CURRENT inside a tune.", pal().ink2);
        }
        if !self.store.tunes.is_empty() {
            table_header(ui, "TUNE", "RECORDINGS", "PRACTICED", false);
        }
        for (i, t) in self.store.tunes.iter().enumerate() {
            let mins = self.store.tune_secs(&t.name) / 60;
            let r = list_row(
                ui,
                i,
                None,
                || {
                    format!(
                        "[{}] {}",
                        "#".repeat(t.status.min(3) as usize + 1) + &"-".repeat(3 - t.status.min(3) as usize),
                        t.name
                    )
                },
                || format!("{}\t{}", t.versions.len(), mins_text(mins * 60)),
                RowState::Normal,
                false,
                false,
            );
            if let Some(r) = r {
                if r.clicked() {
                    match &self.pick {
                        Some(p) => acts.push(Action::AddToTune(i, p.clone())),
                        None => acts.push(Action::OpenTune(Some(i))),
                    }
                }
            }
        }
        ui.add_space(20.0);
    }

    fn tune_detail(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>, i: usize) {
        ui.add_space(6.0);
        let (status, nvers, secs, tname) = {
            let t = &self.store.tunes[i];
            (t.status, t.versions.len(), self.store.tune_secs(&t.name), t.name.clone())
        };
        ui.horizontal(|ui| {
            if retro_btn(ui, "< TUNES", false).clicked() {
                acts.push(Action::OpenTune(None));
            }
            if retro_btn(ui, "CHART", self.rtab == 1).tip("Show the chord changes on the right").clicked() {
                acts.push(Action::RightTab(if self.rtab == 1 { 0 } else { 1 }));
            }
        });
        ui.add_space(4.0);
        if let Some(n) = comfort_meter(ui, status) {
            acts.push(Action::Status(i, n));
        }
        let o = field_row(ui, &mut self.ed, F_T_NAME, "NAME", &mut self.store.tunes[i].name, "Tune name", None);
        self.store_dirty |= o.changed;
        let o = field_row(ui, &mut self.ed, F_T_KEY, "KEY", &mut self.store.tunes[i].key, "e.g. Bb, Dm", None);
        self.store_dirty |= o.changed;
        ui.horizontal(|ui| {
            label(ui, "BPM", 74.0);
            if retro_btn_w(ui, "-", 28.0, false).clicked() {
                let t = &mut self.store.tunes[i];
                t.bpm = if t.bpm == 0 { 100 } else { t.bpm.saturating_sub(5).max(30) };
                self.store_dirty = true;
            }
            let b = self.store.tunes[i].bpm;
            lcd_box(ui, &if b == 0 { "---".to_string() } else { b.to_string() }, 64.0, pal().ink);
            if retro_btn_w(ui, "+", 28.0, false).clicked() {
                let t = &mut self.store.tunes[i];
                t.bpm = if t.bpm == 0 { 100 } else { (t.bpm + 5).min(300) };
                self.store_dirty = true;
            }
            ui.add_space(8.0);
            lcd_box(ui, &format!("{} MIN PRACTICED", secs / 60), 150.0, pal().ink2);
        });
        let o = field_row(
            ui,
            &mut self.ed,
            F_T_NOTES,
            "NOTES",
            &mut self.store.tunes[i].notes,
            "What to work on, forms, tricky bars...",
            Some(84.0),
        );
        self.store_dirty |= o.changed;
        let info = self.store.tunes[i].info.clone();
        if !info.is_empty() {
            ui.add_space(2.0);
            para(ui, &info, pal().ink2);
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if retro_btn(ui, "PLAY ALL", false).clicked() {
                acts.push(Action::PlayTune(i, false));
            }
            if retro_btn(ui, "SHUFFLE", false).clicked() {
                acts.push(Action::PlayTune(i, true));
            }
            if crate::ibtn(ui, &crate::IC_SEARCH, if self.look_busy { "LOOKING..." } else { "LOOK UP" }, self.look_busy)
                .tip("Who wrote it, and the most popular recordings on Tidal. Only searches when you press this.")
                .clicked()
            {
                acts.push(Action::LookUp(i));
            }
            if retro_btn(ui, ["AUTO", "JAZZ", "GOSPEL", "ANY"][self.look_style as usize % 4], self.look_style > 0)
                .tip("What kind of recordings LOOK UP favors. AUTO = jazz when the tune is a known standard.")
                .clicked()
            {
                acts.push(Action::LookStyle);
            }
        });
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 4.0).max(120.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::hover());
            let o = field(
                ui,
                &mut self.ed,
                F_LOOK,
                &mut self.look_q,
                rect,
                "LOOK UP something else: a title, or title + artist (empty = this tune)",
                false,
            );
            if o.enter {
                acts.push(Action::LookUp(i));
            }
        });
        ui.horizontal(|ui| {
            if retro_btn(ui, "PLAY-ALONG", false)
                .tip("Search YouTube for backing tracks of this tune (opens your browser)")
                .clicked()
            {
                acts.push(Action::PlayAlong(tname.clone()));
            }
        });
        ui.horizontal(|ui| {
            if retro_btn(ui, "ADD CURRENT", false).tip("Add the recording that is playing now").clicked() {
                acts.push(Action::AddCurrentToTune(i));
            }
            if retro_btn(ui, if self.del_arm { "SURE? CLICK AGAIN" } else { "REMOVE TUNE" }, self.del_arm).clicked() {
                acts.push(Action::RemoveTune(i));
            }
        });

        // ---- the recordings
        section_header(ui, &format!("RECORDINGS TO STUDY ({})", nvers));
        if nvers == 0 {
            para(
                ui,
                "Nothing here yet. Play a recording and press ADD CURRENT, or right-click any song and choose Add to a tune.",
                pal().ink2,
            );
        }
        let vtracks: Vec<Track> = self.store.tunes[i].versions.iter().map(|v| v.to_track()).collect();
        let playing = self.cur_track().map(|t| t.id);
        if nvers > 0 {
            table_header(ui, "TRACK", "LENGTH", "RATING", true);
        }
        for vi in 0..nvers {
            let (artist, title, kind, stars, note, id) = {
                let v = &self.store.tunes[i].versions[vi];
                (v.artist.clone(), v.title.clone(), v.kind.clone(), v.stars, v.note.clone(), v.id)
            };
            let state = if playing == Some(id) { RowState::Playing } else { RowState::Normal };
            let tag = match kind.as_str() {
                "yt" if src_is_link(&self.store.tunes[i].versions[vi].src) => "[SC] ",
                "yt" => "[YT] ",
                "file" => "[FILE] ",
                _ => "",
            };
            let r = list_row(
                ui,
                vi,
                Some(vi + 1),
                || format!("{}{} - {}", tag, artist, title),
                || format!("{}\t{}", fmt_time(vtracks[vi].duration), stars_text(stars)),
                state,
                false,
                cache::has(id),
            );
            if let Some(r) = r {
                if r.clicked() {
                    acts.push(Action::Play(vtracks.clone(), vi));
                }
                r.context_menu(|ui| {
                    if menu_item(ui, "Rate / add a note") {
                        self.edit_ver = Some(vi);
                        ui.close_menu();
                    }
                    if menu_item(ui, "Play next") {
                        acts.push(Action::PlayNext(vtracks[vi].clone()));
                        ui.close_menu();
                    }
                    if menu_item(ui, "Add to queue") {
                        acts.push(Action::Enqueue(vtracks[vi].clone()));
                        ui.close_menu();
                    }
                    if menu_item(ui, "Remove from this tune") {
                        acts.push(Action::RemoveVersion(i, vi));
                        ui.close_menu();
                    }
                });
            }
            if self.edit_ver == Some(vi) {
                ui.horizontal(|ui| {
                    label(ui, "RATING", 74.0);
                    for n in 1..=5u8 {
                        if retro_btn_w(ui, &n.to_string(), 34.0, stars == n).clicked() {
                            self.store.tunes[i].versions[vi].stars = if stars == n { 0 } else { n };
                            self.store_dirty = true;
                        }
                    }
                    if retro_btn(ui, "DONE", false).clicked() {
                        self.edit_ver = None;
                        self.ed.id = 0;
                    }
                });
                let o = field_row(
                    ui,
                    &mut self.ed,
                    F_V_NOTE,
                    "NOTE",
                    &mut self.store.tunes[i].versions[vi].note,
                    "Why it's worth studying",
                    None,
                );
                self.store_dirty |= o.changed;
                ui.add_space(4.0);
            } else if !note.is_empty() {
                dim_line(ui, &format!("> {}", note), 2.0, pal().ink2);
            }
        }

        // ---- other versions (only after LOOK UP)
        if self.look_busy {
            dim_line(ui, "SEARCHING...", 2.0, pal().ink2);
        }
        if self.look_for == tname && !self.look_tracks.is_empty() {
            section_header(ui, "RECORDINGS TO STUDY");
            dim_line(
                ui,
                "BEST FIRST: TIDAL POPULARITY, WITHOUT KARAOKE/COMPILATIONS, ARTISTS YOU RATE UP. CLICK TO PLAY, RIGHT-CLICK TO ADD.",
                1.0,
                pal().dim,
            );
            let pops: Vec<u32> = self.look_tracks.iter().map(|x| x.1).collect();
            let found: Vec<Track> = self.look_tracks.iter().map(|x| x.0.clone()).collect();
            table_header(ui, "TRACK", "POPULARITY", "LENGTH", true);
            for (k, t) in found.iter().enumerate() {
                let state = if playing == Some(t.id) { RowState::Playing } else { RowState::Normal };
                let r = list_row(
                    ui,
                    k,
                    Some(k + 1),
                    || format!("{} - {}", t.artist, t.title),
                    || format!("{}\t{}", pops[k], fmt_time(t.duration)),
                    state,
                    false,
                    cache::has(t.id),
                );
                if let Some(r) = r {
                    let r = r.tip(format!("From the album {}", t.album));
                    if r.clicked() {
                        acts.push(Action::Play(found.clone(), k));
                    }
                    r.context_menu(|ui| {
                        if menu_item(ui, "Add to this tune") {
                            acts.push(Action::AddToTune(i, t.clone()));
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
        ui.add_space(20.0);
    }

    // ---------------------------------------------------------------- DIARY
    pub(crate) fn diary_view(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        ui.add_space(6.0);
        title_line(ui, "PRACTICE DIARY", 3.0, pal().ink);
        let today = store::today_days();
        if self.diary_sel == 0 {
            self.diary_sel = today;
        }
        if self.diary_ym.0 == 0 {
            let (y, m, _) = store::civil_from_days(self.diary_sel);
            self.diary_ym = (y, m);
        }
        let live = self.acc as u32;
        let day_secs = |app: &App, d: i64| app.store.secs_on(&store::date_str(d)) + if d == today { live } else { 0 };

        // ---- summary
        let week: u32 = (0..7).map(|k| day_secs(self, today - k)).sum();
        let (cy, cm, _) = store::civil_from_days(today);
        let month_prefix = format!("{:04}-{:02}", cy, cm);
        let month: u32 = self
            .store
            .days
            .iter()
            .filter(|d| d.date.starts_with(&month_prefix))
            .map(|d| d.secs.values().sum::<u32>())
            .sum::<u32>()
            + live;
        let total: u32 = self.store.days.iter().map(|d| d.secs.values().sum::<u32>()).sum::<u32>() + live;
        let streak = self.store.streak(today);
        let tiles = [
            ("TODAY", mins_text(day_secs(self, today))),
            ("7 DAYS", mins_text(week)),
            ("THIS MONTH", mins_text(month)),
            ("STREAK", format!("{} DAY{}", streak, if streak == 1 { "" } else { "S" })),
            ("ALL TIME", mins_text(total)),
        ];
        let w = ui.available_width();
        let tw = ((w - 4.0 * 6.0) / 5.0).floor().max(60.0);
        let (sr, _) = ui.allocate_exact_size(Vec2::new(w, 46.0), Sense::hover());
        for (k, (name, val)) in tiles.iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(sr.min.x + k as f32 * (tw + 6.0), sr.min.y), Vec2::new(tw, 46.0));
            inset(ui.painter(), r, pal().lcd);
            ptext(ui.painter(), Pos2::new(r.center().x, r.min.y + 13.0), Align::Center, name, 1.0, pal().dim);
            ptext_fit(ui.painter(), Pos2::new(r.center().x, r.min.y + 31.0), Align::Center, val, 2.0, tw - 8.0, pal().ink);
        }

        // ---- calendar
        ui.add_space(8.0);
        let (y, m) = self.diary_ym;
        const MONTHS: [&str; 12] = [
            "JANUARY",
            "FEBRUARY",
            "MARCH",
            "APRIL",
            "MAY",
            "JUNE",
            "JULY",
            "AUGUST",
            "SEPTEMBER",
            "OCTOBER",
            "NOVEMBER",
            "DECEMBER",
        ];
        ui.horizontal(|ui| {
            if retro_btn_w(ui, "<", 34.0, false).tip("Previous month").clicked() {
                self.diary_ym = if m == 1 { (y - 1, 12) } else { (y, m - 1) };
            }
            let (r, _) = ui.allocate_exact_size(Vec2::new(190.0, BTN_H), Sense::hover());
            ptext(ui.painter(), r.center(), Align::Center, &format!("{} {}", MONTHS[(m as usize - 1) % 12], y), 2.0, pal().ink);
            if retro_btn_w(ui, ">", 34.0, false).tip("Next month").clicked() {
                self.diary_ym = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
            }
            if retro_btn(ui, "TODAY", false).tip("Jump to today").clicked() {
                self.diary_sel = today;
                self.diary_ym = (cy, cm);
            }
        });
        ui.add_space(4.0);
        let first = store::days_from_civil(y, m, 1);
        let next = if m == 12 { store::days_from_civil(y + 1, 1, 1) } else { store::days_from_civil(y, m + 1, 1) };
        let ndays = next - first;
        let lead = (first + 4).rem_euclid(7);
        let gw = ui.available_width().min(7.0 * 64.0);
        let cw = (gw / 7.0).floor();
        let rows = (lead + ndays + 6) / 7;
        let (gr, _) = ui.allocate_exact_size(Vec2::new(cw * 7.0, 16.0 + rows as f32 * 32.0), Sense::hover());
        for (k, n) in ["S", "M", "T", "W", "T", "F", "S"].iter().enumerate() {
            ptext(ui.painter(), Pos2::new(gr.min.x + k as f32 * cw + cw / 2.0, gr.min.y + 7.0), Align::Center, n, 1.0, pal().dim);
        }
        let busiest = (0..ndays).map(|k| day_secs(self, first + k)).max().unwrap_or(0).max(1);
        for k in 0..ndays {
            let d = first + k;
            let pos = lead + k;
            let r = Rect::from_min_size(
                Pos2::new(gr.min.x + (pos % 7) as f32 * cw + 2.0, gr.min.y + 16.0 + (pos / 7) as f32 * 32.0 + 2.0),
                Vec2::new(cw - 4.0, 28.0),
            );
            let secs = day_secs(self, d);
            let resp =
                ui.interact(r, ui.id().with(("diaryday", d)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            let level = if secs == 0 { 0 } else { 1 + (secs * 3 / busiest).min(2) };
            let face = match level {
                0 => pal().lcd,
                1 => pal().row_alt,
                2 => pal().ink2,
                _ => pal().ink,
            };
            inset(ui.painter(), r, face);
            if d == self.diary_sel {
                outline(ui.painter(), r, 2.0, pal().red);
            } else if d == today {
                outline(ui.painter(), r, 1.0, pal().ink);
            }
            let txt_col = if level >= 2 { pal().lcd } else { pal().ink };
            ptext(ui.painter(), Pos2::new(r.min.x + 5.0, r.min.y + 8.0), Align::Min, &format!("{}", k + 1), 1.0, txt_col);
            if secs >= 60 {
                ptext(
                    ui.painter(),
                    Pos2::new(r.max.x - 4.0, r.max.y - 7.0),
                    Align::Max,
                    &format!("{}m", secs / 60),
                    1.0,
                    txt_col,
                );
            }
            if resp.clicked() {
                self.diary_sel = d;
            }
        }

        // ---- the picked day
        let sel = self.diary_sel;
        let date = store::date_str(sel);
        ui.add_space(6.0);
        section_header(ui, &format!("{} {}  -  {}", store::weekday(sel), date, mins_text(day_secs(self, sel))));
        let (mut rows_d, mut loops, mut pomos) = (Vec::new(), 0, 0);
        if let Some(d) = self.store.days.iter().find(|d| d.date == date) {
            rows_d = d.secs.iter().map(|(k, v)| (k.clone(), *v)).collect::<Vec<_>>();
            (loops, pomos) = (d.loops, d.pomos);
        }
        rows_d.sort_by(|a, b| b.1.cmp(&a.1));
        let tot: u32 = rows_d.iter().map(|r| r.1).sum();
        if rows_d.is_empty() {
            dim_line(ui, "NO PRACTICE TIME TRACKED THIS DAY", 2.0, pal().ink2);
        }
        for (k, (name, s)) in rows_d.iter().take(12).enumerate() {
            let _ = list_row(
                ui,
                k,
                None,
                || name.clone(),
                || format!("{}  {}%", mins_text(*s), s * 100 / tot.max(1)),
                RowState::Normal,
                false,
                false,
            );
        }
        if loops + pomos > 0 {
            dim_line(ui, &format!("LOOPS {}   FOCUS BLOCKS {}", loops, pomos), 2.0, pal().ink2);
        }

        let mins_hint = format!("{}", day_secs(self, sel) / 60);
        // ---- journal for that day
        ui.add_space(4.0);
        if self.show_form {
            let hint_t = self.practice_label();
            let _ = field_row(ui, &mut self.ed, F_TUNE, "TUNE", &mut self.f_tune, &hint_t, None);
            let _ = field_row(ui, &mut self.ed, F_MINS, "MINUTES", &mut self.f_mins, &mins_hint, None);
            let _ = field_row(ui, &mut self.ed, F_BPM, "BPM", &mut self.f_bpm, "tempo you reached", None);
            let _ = field_row(ui, &mut self.ed, F_NOTE, "NOTE", &mut self.f_note, "What went well? What next?", Some(70.0));
            ui.horizontal(|ui| {
                if retro_btn(ui, "SAVE ENTRY", false).clicked() {
                    acts.push(Action::SaveEntry);
                }
                if retro_btn(ui, "CANCEL", false).clicked() {
                    self.show_form = false;
                    self.ed.id = 0;
                }
            });
        } else if retro_btn(ui, &format!("LOG PRACTICE FOR {}", date), false)
            .tip("Write down what you worked on - works for past days too")
            .clicked()
        {
            self.show_form = true;
        }
        ui.add_space(6.0);
        let mut any = false;
        for e in self.store.entries.iter().rev().filter(|e| e.date == date) {
            any = true;
            let head = format!(
                "{}{}{}",
                e.tune,
                if e.mins > 0 { format!("  {}m", e.mins) } else { String::new() },
                if e.bpm > 0 { format!("  {} bpm", e.bpm) } else { String::new() }
            );
            dim_line(ui, &head, 2.0, pal().ink);
            if !e.note.is_empty() {
                para(ui, &e.note, pal().ink2);
            }
        }
        if !any && self.store.entries.is_empty() {
            para(ui, "Practice time adds up by itself while you work on a loop in PRACTICE mode. Pick any day above and use LOG PRACTICE to write down what you worked on and the tempo you reached.", pal().ink2);
        }
        ui.add_space(20.0);
    }

    // ------------------------------------------------------------- PRACTICE
    /// Transcribing tools: waveform, A-B loop, saved sections, slow-down and the extras behind MORE.
    pub(crate) fn practice_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        window_deco(ui, ui.max_rect(), "PRACTICE");
        let dur = self.track_len();
        let pos = self.pos();
        let tid = self.cur_track().map(|t| t.id);
        let active = self.cur.is_some() && !self.stopped && dur > 0.0;
        let shown = self.seek_drag.unwrap_or(pos);
        let both = self.loop_a.is_some() && self.loop_b.is_some();
        let step = if ui.input(|i| i.modifiers.shift) { 1.0 } else { 0.1 };
        let shift = ui.input(|i| i.modifiers.shift);

        // ---- waveform timeline
        let (bar, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), Sense::click_and_drag());
        let resp = resp
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip("Click to jump. Shift + drag to select a loop (inside a loop: slide it). Ctrl + drag A or B to move it. Right-click for more.");
        inset(ui.painter(), bar, pal().lcd);
        let inn = bar.shrink2(Vec2::new(5.0, 5.0));
        let at = |t: f32| inn.min.x + inn.width() * (t / dur.max(0.001)).clamp(0.0, 1.0);
        let time_at = |x: f32| ((x - inn.min.x) / inn.width()).clamp(0.0, 1.0) * dur;
        if active {
            let p = ui.painter().clone();
            // waveform
            if let (Some(id), (wid, Some(w))) = (tid, &self.wave) {
                if *wid == id && w.len() > 2 {
                    let cols = (inn.width() / 2.0).max(1.0) as usize;
                    let mid = inn.center().y;
                    for c in 0..cols {
                        let a = c * w.len() / cols;
                        let b = ((c + 1) * w.len() / cols).max(a + 1).min(w.len());
                        let m = w[a.min(w.len() - 1)..b].iter().copied().max().unwrap_or(0);
                        let h = (m as f32 / 255.0) * (inn.height() * 0.5);
                        let x = inn.min.x + c as f32 * 2.0;
                        fill_rect(
                            &p,
                            Rect::from_min_max(Pos2::new(x, mid - h - 0.5), Pos2::new(x + 1.5, mid + h + 0.5)),
                            pal().ink2.gamma_multiply(0.55),
                        );
                    }
                }
            }
            // saved sections as faint bands along the bottom
            if let Some(list) = tid.and_then(|id| self.store.sections.get(&id)) {
                for s in list {
                    fill_rect(
                        &p,
                        Rect::from_min_max(Pos2::new(at(s.a), inn.max.y - 3.0), Pos2::new(at(s.b).max(at(s.a) + 1.0), inn.max.y)),
                        pal().ink.gamma_multiply(0.6),
                    );
                }
            }
            if let (Some(a), Some(b)) = (self.loop_a, self.loop_b) {
                let col = if self.loop_on { pal().red.gamma_multiply(0.35) } else { pal().ink.gamma_multiply(0.2) };
                fill_rect(&p, Rect::from_min_max(Pos2::new(at(a), inn.min.y), Pos2::new(at(b), inn.max.y)), col);
            }
            for (v, lab) in [(self.loop_a, "A"), (self.loop_b, "B")] {
                if let Some(t) = v {
                    fill_rect(
                        &p,
                        Rect::from_min_max(Pos2::new(at(t) - 1.0, bar.min.y + 2.0), Pos2::new(at(t) + 1.0, bar.max.y - 2.0)),
                        pal().red,
                    );
                    ptext(&p, Pos2::new(at(t) + 4.0, inn.min.y + 5.0), Align::Min, lab, 1.0, pal().red);
                }
            }
            fill_rect(
                &p,
                Rect::from_min_max(Pos2::new(at(shown) - 1.0, bar.min.y + 2.0), Pos2::new(at(shown) + 1.0, bar.max.y - 2.0)),
                pal().ink,
            );
            ptext(
                &p,
                Pos2::new(bar.max.x - 8.0, bar.min.y + 8.0),
                Align::Max,
                &format!("{} / {}", fmt_t(shown), fmt_t(dur)),
                1.0,
                pal().ink,
            );

            // shift + drag selects a loop; plain click / drag seeks
            if resp.drag_started() && shift {
                if let Some(pp) = resp.interact_pointer_pos() {
                    self.sel_anchor = Some(time_at(pp.x));
                }
            }
            // ctrl + drag near A or B moves that marker
            let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
            if resp.drag_started() && ctrl {
                if let Some(pp) = resp.interact_pointer_pos() {
                    self.marker_drag = [(1u8, self.loop_a), (2u8, self.loop_b)]
                        .into_iter()
                        .filter_map(|(k, v)| v.map(|t| (k, (at(t) - pp.x).abs())))
                        .filter(|c| c.1 < 14.0)
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                        .map_or(0, |c| c.0);
                }
            }
            // shift + drag inside the loop slides the whole loop, keeping its length
            if resp.drag_started() && shift && !ctrl {
                if let (Some(a), Some(b), Some(pp)) = (self.loop_a, self.loop_b, resp.interact_pointer_pos()) {
                    let t = time_at(pp.x);
                    if t > a && t < b {
                        self.marker_drag = 3;
                        self.pan_off = t - a;
                        self.pan_len = b - a;
                        self.sel_anchor = None;
                    }
                }
            }
            if self.marker_drag != 0 {
                if let Some(pp) = ui.input(|i| i.pointer.hover_pos()) {
                    let t = time_at(pp.x);
                    if self.marker_drag == 3 {
                        let a = (t - self.pan_off).clamp(0.0, (dur - self.pan_len).max(0.0));
                        acts.push(Action::SetAAt(a));
                        acts.push(Action::SetBAt(a + self.pan_len));
                    } else if self.marker_drag == 1 {
                        acts.push(Action::SetAAt(self.loop_b.map_or(t, |b| t.min(b - 0.2))));
                    } else {
                        acts.push(Action::SetBAt(self.loop_a.map_or(t, |a| t.max(a + 0.2))));
                    }
                }
                if resp.drag_stopped() {
                    self.marker_drag = 0;
                }
            } else if let Some(anc) = self.sel_anchor {
                if let Some(pp) = ui.input(|i| i.pointer.hover_pos()) {
                    let (x1, x2) = (at(anc), pp.x.clamp(inn.min.x, inn.max.x));
                    let (lo, hi) = if x1 < x2 { (x1, x2) } else { (x2, x1) };
                    fill_rect(
                        &p,
                        Rect::from_min_max(Pos2::new(lo, inn.min.y), Pos2::new(hi, inn.max.y)),
                        pal().red.gamma_multiply(0.3),
                    );
                }
                if resp.drag_stopped() {
                    if let Some(pp) = resp.interact_pointer_pos() {
                        let t = time_at(pp.x);
                        let (a, b) = if anc < t { (anc, t) } else { (t, anc) };
                        if b - a > 0.3 {
                            acts.push(Action::SetAAt(a));
                            acts.push(Action::SetBAt(b));
                        }
                    }
                    self.sel_anchor = None;
                }
            } else {
                if resp.dragged() || resp.clicked() {
                    if let Some(pp) = resp.interact_pointer_pos() {
                        self.seek_drag = Some(time_at(pp.x));
                    }
                }
                if resp.drag_stopped() || resp.clicked() {
                    if let Some(t) = self.seek_drag.take() {
                        acts.push(Action::Seek(t));
                    }
                }
            }
            if resp.secondary_clicked() {
                if let Some(pp) = resp.interact_pointer_pos() {
                    self.menu_t = time_at(pp.x);
                }
            }
            let (mt, lo, pr) = (self.menu_t, self.loop_on, self.practice);
            resp.context_menu(|ui| crate::loop_menu(ui, acts, mt, both, lo, pr));
        } else {
            ptext(ui.painter(), bar.center(), Align::Center, "PLAY A TRACK TO START", 2.0, pal().dim);
        }
        ui.add_space(2.0);

        // ---- A / B
        let a_txt = format!("A {}", self.loop_a.map(fmt_t).unwrap_or_else(|| "-:--.-".to_string()));
        let b_txt = format!("B {}", self.loop_b.map(fmt_t).unwrap_or_else(|| "-:--.-".to_string()));
        ui.horizontal(|ui| {
            if retro_btn_w(ui, "SET A", 64.0, false).tip("Loop start at the current position  ([ key)").clicked() && active {
                acts.push(Action::SetAAt(pos));
            }
            self.num_step(
                ui,
                acts,
                F_LOOP_A,
                92.0,
                Knob::LoopA,
                a_txt,
                "Nudge earlier / later - or click the box and type a time like 1:23.5",
            );
            ui.add_space(8.0);
            if retro_btn_w(ui, "SET B", 64.0, false).tip("Loop end at the current position  (] key)").clicked() && active {
                acts.push(Action::SetBAt(pos));
            }
            self.num_step(
                ui,
                acts,
                F_LOOP_B,
                92.0,
                Knob::LoopB,
                b_txt,
                "Nudge earlier / later - or click the box and type a time like 2:05",
            );
        });
        ui.horizontal(|ui| {
            let ink = pal().ink;
            if crate::icon_btn_w(ui, &crate::IC_REP, self.loop_on && both, ink, 44.0).tip("Loop on / off  (\\ key)").clicked() {
                acts.push(Action::LoopToggle);
            }
            if crate::icon_btn_w(ui, &crate::IC_X, false, ink, 44.0).tip("Clear the A-B loop").clicked() {
                acts.push(Action::LoopClear);
            }
            ui.add_space(8.0);
            if crate::icon_btn_w(ui, &crate::IC_REW, false, ink, 44.0).tip("Back two seconds  (, key)").clicked() {
                acts.push(Action::SeekRel(-2.0));
            }
            if crate::icon_btn_w(ui, &crate::IC_PREV, false, ink, 44.0)
                .tip("Restart: back to the loop start (or the track start)  (. key)")
                .clicked()
            {
                acts.push(Action::Seek(self.loop_a.unwrap_or(0.0)));
            }
            ui.add_space(8.0);
            if retro_btn_w(ui, "MORE", 64.0, self.more).tip("Speed trainer, pitch, metronome, ear modes, export").clicked() {
                acts.push(Action::ToggleMore);
            }
        });
        ui.horizontal(|ui| {
            label(ui, "SPEED", 64.0);
            for pct in [50u32, 70, 85, 100] {
                if retro_btn_w(ui, &format!("{}", pct), 44.0, self.speed == pct).clicked() {
                    acts.push(Action::Speed(pct));
                }
            }
            ui.add_space(8.0);
            self.num_step(
                ui,
                acts,
                F_SPEED,
                64.0,
                Knob::Speed,
                format!("{}%", self.speed),
                "Slower / faster (Down / Up keys) - or type a percent",
            );
        });

        // ---- saved loops for this track
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 100.0 - 6.0).max(60.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::hover());
            let o = field(ui, &mut self.ed, F_SEC, &mut self.sec_name, rect, "Name this loop (bridge, head, lick...)", false);
            if o.enter {
                acts.push(Action::SaveSection);
            }
            if crate::ibtn(ui, &crate::IC_BOOK, "SAVE LOOP", false).tip("Keep this A-B loop with the track").clicked() {
                acts.push(Action::SaveSection);
            }
        });
        let secs: Vec<(String, f32, f32)> = tid
            .and_then(|id| self.store.sections.get(&id))
            .map(|l| l.iter().map(|s| (s.name.clone(), s.a, s.b)).collect())
            .unwrap_or_default();
        ui.horizontal(|ui| {
            if secs.is_empty() {
                label(ui, "SAVED LOOPS OF THIS TRACK SHOW UP HERE", 300.0);
            }
            let mut budget = ui.available_width();
            for (i, (name, a, b)) in secs.iter().enumerate() {
                let w = (text_w(name, 2.0) + 20.0).clamp(44.0, 150.0);
                if w + 6.0 > budget {
                    label(ui, &format!("+{}", secs.len() - i), 40.0);
                    break;
                }
                budget -= w + 6.0;
                let on = self.loop_on
                    && self.loop_a.map_or(false, |x| (x - a).abs() < 0.05)
                    && self.loop_b.map_or(false, |x| (x - b).abs() < 0.05);
                let r = retro_btn_w(ui, name, w, on).tip(format!("{} - {}   (right-click to delete)", fmt_t(*a), fmt_t(*b)));
                if r.clicked() {
                    acts.push(Action::GoSection(i));
                }
                r.context_menu(|ui| {
                    if menu_item(ui, "Delete this loop") {
                        acts.push(Action::DeleteSection(i));
                        ui.close_menu();
                    }
                });
            }
        });

        // ---- MORE: grouped in tabs
        if self.more {
            let more_pos = ui.cursor().min;
            let more_w = ui.available_width();
            egui::Area::new(egui::Id::new("more_overlay")).order(egui::Order::Foreground).fixed_pos(more_pos).show(
                ui.ctx(),
                |ui| {
                    egui::Frame::none()
                        .fill(pal().beige)
                        .stroke(egui::Stroke::new(2.0_f32, pal().edge))
                        .inner_margin(egui::Margin::same(6.0))
                        .show(ui, |ui| {
                            ui.set_width((more_w - 16.0).max(200.0));
                            if let Some(t) = tab_row(ui, &["TRAINER", "PITCH & EAR", "STEMS"], self.mtab as usize) {
                                self.mtab = t as u8;
                            }
                            ui.add_space(2.0);
                            match self.mtab {
                                0 => {
                                    ui.horizontal(|ui| {
                                        label(ui, "TRAIN", 56.0);
                                        if retro_btn_w(ui, if self.trainer { "ON" } else { "OFF" }, 56.0, self.trainer)
                                            .tip("Practice the loop a set number of times, then speed up by a set percent")
                                            .clicked()
                                        {
                                            acts.push(Action::Trainer);
                                        }
                                        if self.trainer {
                                            lcd_box(ui, &format!("PASS {}/{}", self.passes, self.trainer_n), 100.0, pal().ink);
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        label(ui, "EVERY", 60.0);
                                        self.num_step(
                                            ui,
                                            acts,
                                            F_TR_LOOPS,
                                            44.0,
                                            Knob::Loops,
                                            self.trainer_n.to_string(),
                                            "Loops to play before each speed-up - or type it",
                                        );
                                        label(ui, "LOOPS", 56.0);
                                        self.num_step(
                                            ui,
                                            acts,
                                            F_TR_STEP,
                                            52.0,
                                            Knob::Step,
                                            format!("+{}%", self.trainer_step),
                                            "How much faster each step - or type it",
                                        );
                                    });
                                    dim_line(ui, "STARTS AT THE SPEED SET ABOVE AND STOPS AT 100%", 1.0, pal().dim);
                                    ui.horizontal(|ui| {
                                        let ci = if self.count_in == 0 {
                                            "COUNT OFF".to_string()
                                        } else {
                                            format!("COUNT {}", self.count_in)
                                        };
                                        if retro_btn_w(ui, &ci, 100.0, self.count_in > 0)
                                            .tip("Clicks before every loop pass")
                                            .clicked()
                                        {
                                            acts.push(Action::CountIn);
                                        }
                                        if retro_btn_w(ui, "FOCUS", 76.0, self.focus_mode)
                                            .tip("Hide the lists - just the player and these tools")
                                            .clicked()
                                        {
                                            acts.push(Action::ToggleFocus);
                                        }
                                        if retro_btn_w(ui, "METRONOME", 116.0, self.metro_open)
                                            .tip("Open the metronome")
                                            .clicked()
                                        {
                                            acts.push(Action::MetroPanel);
                                        }
                                    });
                                }
                                1 => {
                                    ui.horizontal(|ui| {
                                        label(ui, "PITCH", 56.0);
                                        if retro_btn_w(ui, "-", 28.0, false)
                                            .tip("Lower the pitch a semitone (speed stays)")
                                            .clicked()
                                        {
                                            acts.push(Action::Transpose(-1));
                                        }
                                        lcd_box(ui, &format!("{:+}", self.semis), 52.0, pal().ink);
                                        if retro_btn_w(ui, "+", 28.0, false).clicked() {
                                            acts.push(Action::Transpose(1));
                                        }
                                        ui.add_space(8.0);
                                        if retro_btn_w(ui, CHAN_NAMES[self.chan as usize % 6], 100.0, self.chan != 0)
                                            .tip("Ear mode. LEFT / RIGHT CHANNEL plays just that side in both ears (old jazz records often hide the bass or piano on one side; modern mixes barely differ). NO CENTER removes vocals; BASS ONLY keeps the low end.")
                                            .clicked()
                                        {
                                            acts.push(Action::Chan);
                                        }
                                        if retro_btn_w(ui, if self.exporting { "SAVING..." } else { "EXPORT WAV" }, 100.0, false)
                                            .tip("Save the A-B loop as a WAV file in Music/Tidalite loops")
                                            .clicked()
                                        {
                                            acts.push(Action::Export);
                                        }
                                    });
                                }
                                _ => {
                                    ui.horizontal(|ui| {
                                        label(ui, "STEMS", 56.0);
                                        if self.stem_busy == 1 {
                    lcd_box(ui, &stems::stage_text(), 300.0, pal().ink2);
                } else if self.stem_busy == 2 {
                    let (d, t) = (stems::DONE.load(Ordering::Relaxed), stems::TOTAL.load(Ordering::Relaxed));
                    lcd_box(ui, &format!("{} {}/{}", stems::stage_text(), d, t), 240.0, pal().ink2);
                    if retro_btn(ui, "CANCEL", false).clicked() {
                        acts.push(Action::StemCancel);
                    }
                } else if !stems::tool_ready() {
                    if retro_btn_w(ui, "GET STEMS TOOL (170 MB)", 230.0, false)
                        .tip("One-time download: the AI model that separates drums, bass, vocals and the rest. Runs on this PC.")
                        .clicked()
                    {
                        acts.push(Action::StemGet);
                    }
                } else if tid.map_or(false, stems::have_stems) {
                    for (i, name) in stems::NAMES.iter().enumerate() {
                        let r = stem_btn(ui, i, self.stem_on[i])
                            .tip(format!("{}: click on / off, right-click for only this one", name));
                        if r.clicked() {
                            acts.push(Action::StemToggle(i));
                        }
                        r.context_menu(|ui| {
                            if menu_item(ui, "Only this stem") {
                                acts.push(Action::StemSolo(i));
                                ui.close_menu();
                            }
                        });
                    }
                    if retro_btn_w(ui, "ALL", 44.0, false).clicked() {
                        acts.push(Action::StemAll);
                    }
                } else if retro_btn_w(ui, "SPLIT THIS TRACK", 190.0, false)
                    .tip("Separates the stored track into drums, bass, other and vocals. Takes a few minutes; saved afterwards.")
                    .clicked()
                {
                    acts.push(Action::StemSplit);
                }
                                    });
                                }
                            }
                        });
                },
            );
        }
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 14.0), Sense::hover());
        ptext_fit(
            ui.painter(),
            Pos2::new(rect.min.x + 2.0, rect.center().y),
            Align::Min,
            "LOOPS, SPEED AND SEEKS STAY ON THIS PC - NOTHING IS REPORTED TO TIDAL",
            1.0,
            rect.width() - 4.0,
            pal().dim,
        );
    }
}

const COMFORT: [&str; 4] = ["LEARNING", "OK", "GOOD", "COMFORTABLE"];

/// How well you know a tune: four pips, click one to set the level.
fn comfort_meter(ui: &mut egui::Ui, level: u8) -> Option<u8> {
    let level = level.min(3);
    let mut hit = None;
    ui.horizontal(|ui| {
        label(ui, "KNOW IT", 74.0);
        for n in 0..4u8 {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(30.0, BTN_H), Sense::click());
            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand).tip(COMFORT[n as usize]);
            let on = n <= level;
            let p = ui.painter();
            inset(p, r, pal().beige_dk);
            if on {
                fill_rect(p, r.shrink(5.0), if level == 3 { pal().ink } else { pal().red });
            }
            if resp.clicked() {
                hit = Some(n);
            }
        }
        ui.add_space(8.0);
        lcd_box(ui, COMFORT[level as usize], 130.0, pal().ink);
    });
    hit
}
