//! Everything drawn for the practice extras: the FILES / YOUTUBE / TUNES / DIARY lists, the
//! practice panel, the lead sheet and the little text-field widget they share.

use crate::api::Track;
use crate::chart;
use crate::extras::{CHAN_NAMES, TRAIN_CYCLE_PASSES, TRAIN_CYCLE_STEP};
use crate::font::{ptext, ptext_fit, spx, text_w};
use crate::sources;
use crate::stems;
use crate::store::{self, Ext};
use crate::{
    cache, chip, fill_rect, fmt_t, fmt_time, inset, lcd_box, list_row, menu_item, outline, pal, para, play_buttons, retro_btn,
    retro_btn_w, section_header, title_line, window_deco, Action, App, Ed, RowState, Sec, BTN_H,
};
use eframe::egui::{self, Align, Pos2, Rect, Sense, Vec2};
use std::sync::atomic::Ordering;

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
const F_CHART: u32 = 12;
const F_IREAL: u32 = 13;

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
fn field_row(ui: &mut egui::Ui, ed: &mut Ed, id: u32, label: &str, text: &mut String, hint: &str, tall: Option<f32>) -> FieldOut {
    let h = tall.unwrap_or(BTN_H);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
    let ly = if tall.is_some() { rect.min.y + 13.0 } else { rect.center().y };
    ptext(ui.painter(), Pos2::new(rect.min.x + 4.0, ly), Align::Min, label, 2.0, pal().ink2);
    let fr = Rect::from_min_max(Pos2::new(rect.min.x + 76.0, rect.min.y), rect.max);
    field(ui, ed, id, text, fr, hint, tall.is_some())
}

fn label(ui: &mut egui::Ui, text: &str, w: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::hover());
    ptext(ui.painter(), Pos2::new(rect.min.x + 2.0, rect.center().y), Align::Min, text, 2.0, pal().ink2);
}

fn dim_line(ui: &mut egui::Ui, text: &str, px: f32, col: egui::Color32) {
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
        ui.horizontal(|ui| {
            for (s, name) in
                [(Sec::Tidal, "TIDAL"), (Sec::Files, "FILES"), (Sec::Yt, "YT"), (Sec::Tunes, "TUNES"), (Sec::Diary, "DIARY")]
            {
                if retro_btn(ui, name, self.sec == s).clicked() {
                    acts.push(Action::Section(s));
                }
            }
        });
    }

    /// "Pick up where you left off" in one press.
    pub(crate) fn continue_bar(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        if !(self.cur.is_none() || self.stopped) {
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
        if chip(ui, r_chart, "CHART", self.rtab == 1, "c_rchart").on_hover_text("Chord changes of the open tune").clicked() {
            acts.push(Action::RightTab(1));
        }
        if chip(ui, r_queue, "QUEUE", self.rtab == 0, "c_rqueue").clicked() {
            acts.push(Action::RightTab(0));
        }
    }

    /// Called at the top of the queue window.
    pub(crate) fn right_header(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) -> bool {
        self.right_tabs(ui, acts);
        if self.rtab == 1 {
            self.chart_ui(ui, acts);
            true
        } else {
            false
        }
    }

    // ---------------------------------------------------------------- FILES
    fn ext_list(&self, ui: &mut egui::Ui, acts: &mut Vec<Action>, yt: bool) {
        let list = if yt { &self.store.yt } else { &self.store.files };
        let playing = self.cur_track().map(|t| t.id);
        for (i, e) in list.iter().enumerate() {
            let state = if playing == Some(e.id) { RowState::Playing } else { RowState::Normal };
            let stored = if yt { cache::has(e.id) } else { true };
            let r =
                list_row(ui, i, Some(i + 1), || format!("{} - {}", e.artist, e.title), || fmt_time(e.dur), state, false, stored);
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
                    if menu_item(ui, "Remove from this list") {
                        acts.push(Action::RemoveExt(e.id));
                        ui.close_menu();
                    }
                });
            }
        }
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
            if retro_btn(ui, "RESCAN", false).on_hover_text("Look for new files in the folders you added").clicked() {
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
            self.ext_list(ui, acts, false);
        }
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
                .on_hover_text("Downloads the free yt-dlp tool from its official GitHub page")
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
            self.ext_list(ui, acts, true);
        }
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
        ui.add_space(4.0);
        if self.store.tunes.is_empty() {
            para(ui, "Keep the tunes you are learning here, with the recordings worth studying for each: Joe Pass and Herb Ellis on Cherokee, Sonny Stitt, the horn players... Add a recording from any list (right-click a song, then Add to a tune), or play one and press ADD CURRENT inside a tune.", pal().ink2);
        }
        let tags = ["LEARN", "WORKING", "READY"];
        for (i, t) in self.store.tunes.iter().enumerate() {
            let mins = self.store.tune_secs(&t.name) / 60;
            let r = list_row(
                ui,
                i,
                None,
                || format!("[{}] {}", tags[t.status as usize % 3], t.name),
                || format!("{} rec  {}m", t.versions.len(), mins),
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
        let status_names = ["LEARNING", "WORKING", "READY"];
        let (status, nvers, secs, tname) = {
            let t = &self.store.tunes[i];
            (t.status, t.versions.len(), self.store.tune_secs(&t.name), t.name.clone())
        };
        ui.horizontal(|ui| {
            if retro_btn(ui, "< TUNES", false).clicked() {
                acts.push(Action::OpenTune(None));
            }
            if retro_btn(ui, status_names[status as usize % 3], false)
                .on_hover_text("Click to change how well you know it")
                .clicked()
            {
                acts.push(Action::CycleStatus(i));
            }
            if retro_btn(ui, "CHART", self.rtab == 1).on_hover_text("Show the chord changes on the right").clicked() {
                acts.push(Action::RightTab(if self.rtab == 1 { 0 } else { 1 }));
            }
        });
        ui.add_space(4.0);
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
            if retro_btn(ui, if self.look_busy { "LOOKING..." } else { "LOOK UP" }, self.look_busy)
                .on_hover_text("Who wrote it, and other recordings on Tidal. Only searches when you press this.")
                .clicked()
            {
                acts.push(Action::LookUp(i));
            }
        });
        ui.horizontal(|ui| {
            if retro_btn(ui, "ADD CURRENT", false).on_hover_text("Add the recording that is playing now").clicked() {
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
        for vi in 0..nvers {
            let (artist, title, kind, stars, note, id) = {
                let v = &self.store.tunes[i].versions[vi];
                (v.artist.clone(), v.title.clone(), v.kind.clone(), v.stars, v.note.clone(), v.id)
            };
            let state = if playing == Some(id) { RowState::Playing } else { RowState::Normal };
            let tag = match kind.as_str() {
                "yt" => "[YT] ",
                "file" => "[FILE] ",
                _ => "",
            };
            let r = list_row(
                ui,
                vi,
                Some(vi + 1),
                || format!("{}{} - {}", tag, artist, title),
                || stars_text(stars),
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
            section_header(ui, "OTHER RECORDINGS ON TIDAL");
            dim_line(ui, "CLICK TO PLAY - RIGHT-CLICK TO ADD TO THIS TUNE", 1.0, pal().dim);
            let found = self.look_tracks.clone();
            for (k, t) in found.iter().enumerate() {
                let state = if playing == Some(t.id) { RowState::Playing } else { RowState::Normal };
                let r = list_row(
                    ui,
                    k,
                    Some(k + 1),
                    || format!("{} - {}", t.artist, t.title),
                    || fmt_time(t.duration),
                    state,
                    false,
                    cache::has(t.id),
                );
                if let Some(r) = r {
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
        let date = store::date_str(today);
        let secs = self.store.secs_on(&date) + self.acc as u32;
        let goal = self.store.goal_min.max(1);
        let streak = self.store.streak(today);
        title_line(ui, &date, 2.0, pal().ink2);
        ui.add_space(4.0);

        // today's goal bar
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
        inset(ui.painter(), rect, pal().lcd);
        let frac = (secs as f32 / (goal * 60) as f32).clamp(0.0, 1.0);
        if frac > 0.0 {
            fill_rect(
                ui.painter(),
                Rect::from_min_max(
                    rect.min + Vec2::new(2.0, 2.0),
                    Pos2::new(rect.min.x + 2.0 + (rect.width() - 4.0) * frac, rect.max.y - 2.0),
                ),
                pal().ink2,
            );
        }
        let on_bar = if frac > 0.5 { pal().lcd } else { pal().ink };
        ptext(ui.painter(), rect.center(), Align::Center, &format!("TODAY  {} / {} MIN", secs / 60, goal), 2.0, on_bar);
        let msg = if secs >= goal * 60 {
            "GOAL REACHED - NICE WORK".to_string()
        } else if secs == 0 {
            "ANY PRACTICE COUNTS. EVEN 5 MINUTES.".to_string()
        } else {
            format!("{} MORE MIN TO YOUR GOAL", (goal * 60 - secs).div_ceil(60))
        };
        dim_line(ui, &msg, 2.0, pal().ink2);
        ui.horizontal(|ui| {
            label(ui, "GOAL", 60.0);
            if retro_btn_w(ui, "-", 28.0, false).clicked() {
                acts.push(Action::GoalAdj(-5));
            }
            lcd_box(ui, &format!("{} MIN", goal), 80.0, pal().ink);
            if retro_btn_w(ui, "+", 28.0, false).clicked() {
                acts.push(Action::GoalAdj(5));
            }
        });

        // the week
        ui.add_space(6.0);
        let week = self.store.week(today);
        let (wr, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), Sense::hover());
        for (k, did) in week.iter().enumerate() {
            let x = wr.min.x + 8.0 + k as f32 * 38.0;
            let cell = Rect::from_min_size(Pos2::new(x, wr.min.y), Vec2::new(30.0, 22.0));
            inset(ui.painter(), cell, if *did { pal().ink2 } else { pal().lcd });
            ptext(
                ui.painter(),
                Pos2::new(cell.center().x, cell.max.y + 9.0),
                Align::Center,
                store::weekday(today - 6 + k as i64),
                1.0,
                if k == 6 { pal().ink } else { pal().dim },
            );
        }
        let streak_txt = if streak == 0 {
            "A NEW STREAK STARTS WITH TODAY".to_string()
        } else {
            format!("{} DAY{} IN A ROW", streak, if streak == 1 { "" } else { "S" })
        };
        ptext(
            ui.painter(),
            Pos2::new(wr.min.x + 8.0 + 7.0 * 38.0 + 6.0, wr.min.y + 11.0),
            Align::Min,
            &streak_txt,
            1.0,
            pal().ink2,
        );

        // what happened today
        let (loops, clean, pomos, by_tune) = match self.store.days.iter().find(|d| d.date == date) {
            Some(d) => (d.loops, d.clean, d.pomos, d.secs.iter().map(|(k, v)| (k.clone(), *v)).collect::<Vec<_>>()),
            None => (0, 0, 0, Vec::new()),
        };
        if !by_tune.is_empty() {
            section_header(ui, "TODAY");
            for (k, (name, s)) in by_tune.iter().enumerate() {
                let _ = list_row(
                    ui,
                    k,
                    None,
                    || name.clone(),
                    || if *s < 60 { format!("{} SEC", s) } else { format!("{} MIN", s / 60) },
                    RowState::Normal,
                    false,
                    false,
                );
            }
            dim_line(ui, &format!("LOOPS {}   CLEAN PASSES {}   FOCUS BLOCKS {}", loops, clean, pomos), 2.0, pal().ink2);
        }

        // log form
        section_header(ui, "JOURNAL");
        if self.show_form {
            let hint_t = self.practice_label();
            let o = field_row(ui, &mut self.ed, F_TUNE, "TUNE", &mut self.f_tune, &hint_t, None);
            let _ = o;
            let _ = field_row(ui, &mut self.ed, F_MINS, "MINUTES", &mut self.f_mins, &format!("{}", secs / 60), None);
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
        } else if retro_btn(ui, "LOG PRACTICE", false).on_hover_text("Write a note about this session").clicked() {
            self.show_form = true;
        }
        ui.add_space(6.0);
        if self.store.entries.is_empty() {
            para(ui, "Your practice time adds up by itself while you play in PRACTICE mode. Use LOG PRACTICE to write down what you worked on and the tempo you reached.", pal().ink2);
        }
        for e in self.store.entries.iter().rev().take(40) {
            let head = format!(
                "{}  {}{}{}",
                e.date,
                e.tune,
                if e.mins > 0 { format!("  {}m", e.mins) } else { String::new() },
                if e.bpm > 0 { format!("  {} bpm", e.bpm) } else { String::new() }
            );
            dim_line(ui, &head, 2.0, pal().ink);
            if !e.note.is_empty() {
                para(ui, &e.note, pal().ink2);
            }
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
            .on_hover_text("Click to jump. Shift + drag to select a loop. Right-click for more.");
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
            if let Some(anc) = self.sel_anchor {
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
            if retro_btn_w(ui, "SET A", 64.0, false).on_hover_text("Loop start at the current position  ([ key)").clicked()
                && active
            {
                acts.push(Action::SetAAt(pos));
            }
            lcd_box(ui, &a_txt, 92.0, pal().ink);
            if retro_btn_w(ui, "-", 28.0, false).on_hover_text("Nudge earlier (shift = 1 s)").clicked() {
                acts.push(Action::NudgeA(-step));
            }
            if retro_btn_w(ui, "+", 28.0, false).on_hover_text("Nudge later (shift = 1 s)").clicked() {
                acts.push(Action::NudgeA(step));
            }
            ui.add_space(8.0);
            if retro_btn_w(ui, "SET B", 64.0, false).on_hover_text("Loop end at the current position  (] key)").clicked()
                && active
            {
                acts.push(Action::SetBAt(pos));
            }
            lcd_box(ui, &b_txt, 92.0, pal().ink);
            if retro_btn_w(ui, "-", 28.0, false).clicked() {
                acts.push(Action::NudgeB(-step));
            }
            if retro_btn_w(ui, "+", 28.0, false).clicked() {
                acts.push(Action::NudgeB(step));
            }
        });
        ui.horizontal(|ui| {
            if retro_btn_w(ui, "LOOP", 70.0, self.loop_on && both).on_hover_text("Loop on / off  (\\ key)").clicked() {
                acts.push(Action::LoopToggle);
            }
            if retro_btn_w(ui, "CLEAR", 70.0, false).clicked() {
                acts.push(Action::LoopClear);
            }
            ui.add_space(8.0);
            if retro_btn_w(ui, "BACK 2S", 86.0, false).on_hover_text("Jump back two seconds  (, key)").clicked() {
                acts.push(Action::SeekRel(-2.0));
            }
            if retro_btn_w(ui, "RESTART", 86.0, false).on_hover_text("Back to loop start (or track start)  (. key)").clicked() {
                acts.push(Action::Seek(self.loop_a.unwrap_or(0.0)));
            }
            ui.add_space(8.0);
            if retro_btn_w(ui, "MORE", 64.0, self.more)
                .on_hover_text("Speed trainer, pitch, metronome, ear modes, export")
                .clicked()
            {
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
            if retro_btn_w(ui, "-", 28.0, false).on_hover_text("Slower (Down key)").clicked() {
                acts.push(Action::Speed(self.speed.saturating_sub(5)));
            }
            lcd_box(ui, &format!("{}%", self.speed), 64.0, pal().ink);
            if retro_btn_w(ui, "+", 28.0, false).on_hover_text("Faster (Up key)").clicked() {
                acts.push(Action::Speed(self.speed + 5));
            }
        });

        // ---- saved loops for this track
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 100.0 - 6.0).max(60.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::hover());
            let o = field(ui, &mut self.ed, F_SEC, &mut self.sec_name, rect, "Name this loop (bridge, head, lick...)", false);
            if o.enter {
                acts.push(Action::SaveSection);
            }
            if retro_btn_w(ui, "SAVE LOOP", 100.0, false).on_hover_text("Keep this A-B loop with the track").clicked() {
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
                let r = retro_btn_w(ui, name, w, on).on_hover_text(format!(
                    "{} - {}   (right-click to delete)",
                    fmt_t(*a),
                    fmt_t(*b)
                ));
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

        // ---- MORE: trainer, pitch, ear, metronome
        if self.more {
            ui.horizontal(|ui| {
                label(ui, "TRAIN", 56.0);
                let names = ["OFF", "AUTO", "EARNED"];
                if retro_btn_w(ui, names[self.trainer as usize % 3], 80.0, self.trainer > 0)
                    .on_hover_text(
                        "OFF / AUTO speeds up by itself every few loops / EARNED speeds up after clean passes you confirm",
                    )
                    .clicked()
                {
                    acts.push(Action::Trainer((self.trainer + 1) % 3));
                }
                if retro_btn_w(ui, &format!("+{}%", self.trainer_step), 56.0, false)
                    .on_hover_text("How much faster each step")
                    .clicked()
                {
                    acts.push(Action::Trainer(TRAIN_CYCLE_STEP));
                }
                if retro_btn_w(ui, &format!("EVERY {}", self.trainer_n), 84.0, false)
                    .on_hover_text("Loops (or clean passes) per step")
                    .clicked()
                {
                    acts.push(Action::Trainer(TRAIN_CYCLE_PASSES));
                }
                if retro_btn_w(ui, "CLEAN", 72.0, false).on_hover_text("That pass was clean  (K key)").clicked() {
                    acts.push(Action::Clean);
                }
                lcd_box(ui, &format!("{}/{}", self.passes, self.trainer_n), 64.0, pal().ink);
            });
            ui.horizontal(|ui| {
                label(ui, "PITCH", 56.0);
                if retro_btn_w(ui, "-", 28.0, false).on_hover_text("Lower the pitch a semitone (speed stays)").clicked() {
                    acts.push(Action::Transpose(-1));
                }
                lcd_box(ui, &format!("{:+}", self.semis), 52.0, pal().ink);
                if retro_btn_w(ui, "+", 28.0, false).clicked() {
                    acts.push(Action::Transpose(1));
                }
                ui.add_space(8.0);
                if retro_btn_w(ui, CHAN_NAMES[self.chan as usize % 6], 112.0, self.chan != 0)
                    .on_hover_text("Ear mode: left / right only, mono, take out the middle, or just the bass")
                    .clicked()
                {
                    acts.push(Action::Chan);
                }
                if retro_btn_w(ui, if self.exporting { "SAVING..." } else { "EXPORT WAV" }, 112.0, false)
                    .on_hover_text("Save the A-B loop as a WAV file in Music/Tidalite loops")
                    .clicked()
                {
                    acts.push(Action::Export);
                }
            });
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
                        .on_hover_text(
                            "One-time download: the AI model that separates drums, bass, vocals and the rest. Runs on this PC.",
                        )
                        .clicked()
                    {
                        acts.push(Action::StemGet);
                    }
                } else if tid.map_or(false, stems::have_stems) {
                    for (i, name) in stems::NAMES.iter().enumerate() {
                        let r = stem_btn(ui, i, self.stem_on[i])
                            .on_hover_text(format!("{}: click on / off, right-click for only this one", name));
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
                    .on_hover_text(
                        "Separates the stored track into drums, bass, other and vocals. Takes a few minutes; saved afterwards.",
                    )
                    .clicked()
                {
                    acts.push(Action::StemSplit);
                }
            });
            ui.horizontal(|ui| {
                label(ui, "TIMER", 56.0);
                let left = self.pomo_end.map(|e| e.saturating_duration_since(std::time::Instant::now()).as_secs()).unwrap_or(0);
                let txt = match self.pomo {
                    0 => "START".to_string(),
                    1 => format!("FOCUS {}:{:02}", left / 60, left % 60),
                    _ => format!("BREAK {}:{:02}", left / 60, left % 60),
                };
                if retro_btn_w(ui, &txt, 130.0, self.pomo > 0)
                    .on_hover_text("Focus timer: work in blocks, then a real break. Click again to stop.")
                    .clicked()
                {
                    acts.push(Action::Pomo);
                }
                if retro_btn_w(ui, &format!("{} MIN", self.pomo_focus), 70.0, false).on_hover_text("Focus block length").clicked()
                {
                    acts.push(Action::PomoLen);
                }
                if retro_btn_w(ui, &format!("REST {}", self.pomo_break), 80.0, false)
                    .on_hover_text("Break length (minutes)")
                    .clicked()
                {
                    acts.push(Action::PomoBreak);
                }
            });
            ui.horizontal(|ui| {
                if retro_btn_w(ui, "METRO", 80.0, self.metro_on)
                    .on_hover_text("Click track at the tune's tempo, following your speed")
                    .clicked()
                {
                    acts.push(Action::MetroToggle);
                }
                if retro_btn_w(ui, "-", 28.0, false).clicked() {
                    acts.push(Action::BpmAdj(-1));
                }
                lcd_box(ui, &if self.bpm == 0 { "---".to_string() } else { self.bpm.to_string() }, 60.0, pal().ink);
                if retro_btn_w(ui, "+", 28.0, false).clicked() {
                    acts.push(Action::BpmAdj(1));
                }
                if retro_btn_w(ui, "TAP", 52.0, false).on_hover_text("Tap along with the music to find the tempo").clicked() {
                    acts.push(Action::TapTempo);
                }
                let ci = if self.count_in == 0 { "COUNT OFF".to_string() } else { format!("COUNT {}", self.count_in) };
                if retro_btn_w(ui, &ci, 100.0, self.count_in > 0).on_hover_text("Clicks before every loop pass").clicked() {
                    acts.push(Action::CountIn);
                }
                if retro_btn_w(ui, "FOCUS", 76.0, self.focus_mode)
                    .on_hover_text("Hide the lists - just the player and these tools")
                    .clicked()
                {
                    acts.push(Action::ToggleFocus);
                }
            });
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

    // ---------------------------------------------------------- LEAD SHEET
    pub(crate) fn chart_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        let Some(ti) = self.chart_tune() else {
            ui.add_space(8.0);
            para(ui, "No tune selected. Open a tune in the TUNES list - or play a recording that belongs to one - and its chord changes show up here.", pal().ink2);
            para(ui, "Have an iReal Pro link? Paste it to bring in the changes (and the composer):", pal().ink2);
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 90.0).max(80.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::hover());
                let _ = field(ui, &mut self.ed, F_IREAL, &mut self.ireal_in, rect, "irealb://...", false);
                if retro_btn_w(ui, "IMPORT", 80.0, false).clicked() {
                    acts.push(Action::ImportIreal(None));
                }
            });
            return;
        };
        let (name, key, bpm, info, text) = {
            let t = &self.store.tunes[ti];
            (t.name.clone(), t.key.clone(), t.bpm, t.info.clone(), t.chart.clone())
        };
        ui.add_space(4.0);
        title_line(ui, &name, 3.0, pal().ink);
        let mut sub = String::new();
        if !key.is_empty() {
            sub.push_str(&format!("Key {}    ", key));
        }
        if bpm > 0 {
            sub.push_str(&format!("{} BPM    ", bpm));
        }
        if let Some(first) = info.lines().next() {
            sub.push_str(first);
        }
        title_line(ui, &sub, 2.0, pal().ink2);
        ui.horizontal(|ui| {
            for (lab, semis, tip) in [
                ("CONCERT", 0, "As written (C instruments)"),
                ("Bb", 2, "For Bb horns: trumpet, tenor sax"),
                ("Eb", 9, "For Eb horns: alto and bari sax"),
                ("F", 7, "For F horn"),
            ] {
                if retro_btn(ui, lab, self.chart_tr == semis).on_hover_text(tip).clicked() {
                    self.chart_tr = semis;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if retro_btn(ui, if self.chart_edit { "DONE" } else { "EDIT" }, self.chart_edit).clicked() {
                    self.chart_edit = !self.chart_edit;
                    self.ed.id = 0;
                }
            });
        });
        ui.add_space(2.0);

        if self.chart_edit {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                para(ui, "Type the changes: one bar between each |, sections like *A or *B, time like T44, % repeats the last bar. Example:", pal().ink2);
                para(ui, "T44 *A | Dm7 G7 | Cmaj7 | Cmaj7 % | *B | Em7b5 A7 | Dm7 G7 |", pal().ink);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 150.0), Sense::hover());
                let o = field(ui, &mut self.ed, F_CHART, &mut self.store.tunes[ti].chart, rect, "T44 *A | Dm7 G7 | Cmaj7 |", true);
                self.store_dirty |= o.changed;
                ui.add_space(8.0);
                para(ui, "Or paste an iReal Pro link (irealb://...). It fills in the changes, key, tempo and composer.", pal().ink2);
                ui.horizontal(|ui| {
                    let w = (ui.available_width() - 90.0).max(80.0);
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::hover());
                    let _ = field(ui, &mut self.ed, F_IREAL, &mut self.ireal_in, rect, "irealb://...", false);
                    if retro_btn_w(ui, "IMPORT", 80.0, false).clicked() {
                        acts.push(Action::ImportIreal(Some(ti)));
                    }
                });
            });
            return;
        }

        let chart = self.chart_for(&text);
        if chart.bars.is_empty() {
            ui.add_space(8.0);
            para(ui, "No changes saved for this tune yet. Press EDIT to type them in or paste an iReal Pro link.", pal().ink2);
            return;
        }
        let tr = self.chart_tr;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(4.0);
            let w = ui.available_width() - 6.0;
            let per: usize = if w >= 760.0 { 8 } else { 4 };
            let gap = 6.0;
            let cw = ((w - gap * (per as f32 - 1.0)) / per as f32).floor().max(40.0);
            let ch = 50.0;
            for (ri, row) in chart.bars.chunks(per).enumerate() {
                let (rr, _) = ui.allocate_exact_size(Vec2::new(w, ch + 6.0), Sense::hover());
                if !ui.is_rect_visible(rr) {
                    continue;
                }
                for (ci, bar) in row.iter().enumerate() {
                    let x = rr.min.x + ci as f32 * (cw + gap);
                    let cell = Rect::from_min_size(Pos2::new(x, rr.min.y + 3.0), Vec2::new(cw, ch));
                    let p = ui.painter();
                    inset(p, cell, pal().lcd);
                    let mut top = 0.0;
                    if !bar.label.is_empty() {
                        let lw = text_w(&bar.label, 2.0) + 10.0;
                        let tag = Rect::from_min_size(cell.min + Vec2::new(3.0, 3.0), Vec2::new(lw, 16.0));
                        fill_rect(p, tag, pal().edge);
                        ptext(p, tag.center(), Align::Center, &bar.label, 2.0, pal().trim);
                        top = 6.0;
                    }
                    let cy = cell.center().y + top;
                    if bar.repeat {
                        ptext(p, Pos2::new(cell.center().x, cy), Align::Center, "%", 3.0, pal().ink2);
                    } else {
                        let n = bar.chords.len().max(1);
                        let slot = cell.width() / n as f32;
                        for (k, c) in bar.chords.iter().enumerate() {
                            let shown = chart::transpose(c, tr);
                            let px = if n == 1 { 3.0 } else { 2.0 };
                            ptext_fit(
                                p,
                                Pos2::new(cell.min.x + slot * (k as f32 + 0.5), cy),
                                Align::Center,
                                &shown,
                                px,
                                slot - 6.0,
                                pal().ink,
                            );
                        }
                    }
                    ptext(
                        p,
                        Pos2::new(cell.max.x - 4.0, cell.max.y - 6.0),
                        Align::Max,
                        &(ri * per + ci + 1).to_string(),
                        1.0,
                        pal().dim,
                    );
                }
            }
            ui.add_space(14.0);
        });
    }
}
