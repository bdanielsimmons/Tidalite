//! Moving around fast: the four screen modes (LIBRARY / PLAYER / ALBUM / MINI) with their strip of buttons,
//! the Ctrl+K (Cmd+K on a Mac) command palette that finds any action by typing, and `run_named`,
//! the one place that turns an action name into the action (used by the palette and the Mac menu bar).

use crate::prefs::{Cmd, CMDS};
use crate::{fill_rect, pal, ptext, ptext_fit, raised_h, retro_btn_w, rfill, style, Action, App, Tip, PRACTICE};
use eframe::egui::{self, Align, Color32, Pos2, Sense, Vec2};

/// (what you see, the action name, only in the practice build)
const ITEMS: &[(&str, &str, bool)] = &[
    ("Play / pause", "toggle", false),
    ("Next song", "next", false),
    ("Previous song", "prev", false),
    ("Like the current song", "like", false),
    ("Mode: library (all three windows)", "mode1", false),
    ("Mode: player (just the player and queue)", "mode2", false),
    ("Mode: album view (big cover, lyrics, visualizer)", "mode3", false),
    ("Mode: mini player", "mode4", false),
    ("Full screen on / off", "full", false),
    ("Lyrics on / off", "lyrics", false),
    ("Visualizer on / off", "spec", false),
    ("Next skin", "skin", false),
    ("Equalizer: show / hide", "eq", false),
    ("Equalizer on / off", "eq_on", false),
    ("Equalizer preset: flat", "eq_flat", false),
    ("Equalizer preset: bass", "eq_bass", false),
    ("Equalizer preset: treble", "eq_treble", false),
    ("Equalizer preset: rock", "eq_rock", false),
    ("Add files...", "add_files", false),
    ("Add folder...", "add_folder", false),
    ("Preferences and shortcuts", "prefs", false),
    ("Help", "help", false),
    ("Take the tour", "tour", false),
    ("Check for updates", "update_check", false),
    ("Practice mode on / off", "practice", true),
    ("Loop on / off", "loop", true),
    ("Set loop start (A)", "mark_a", true),
    ("Set loop end (B)", "mark_b", true),
    ("Jump to loop start", "to_a", true),
    ("Speed up 5 percent", "speed_up", false),
    ("Slow down 5 percent", "speed_down", false),
    ("Metronome", "metro", true),
    ("Focus timer (pomodoro)", "timer", true),
];

fn matches(label: &str, q: &str) -> bool {
    let l = label.to_lowercase();
    q.to_lowercase().split_whitespace().all(|w| l.contains(w))
}

impl App {
    /// 0 library, 1 player, 2 album, 3 mini
    pub(crate) fn mode(&self) -> u8 {
        if self.mini {
            3
        } else if self.art_view {
            2
        } else if self.focus_mode {
            1
        } else {
            0
        }
    }

    /// Switch to a mode from wherever you are, leaving the current one cleanly.
    pub(crate) fn set_mode(&mut self, m: u8) {
        if self.mode() == m {
            return;
        }
        if self.mini {
            self.apply(Action::ToggleMini); // restores the window size
        }
        self.art_view = false;
        match m {
            1 => {
                if !self.focus_mode {
                    self.apply(Action::ToggleFocus);
                }
            }
            _ => {
                if self.focus_mode {
                    self.apply(Action::ToggleFocus);
                }
            }
        }
        match m {
            2 => self.art_view = true,
            3 => {
                if !self.mini {
                    self.apply(Action::ToggleMini);
                }
            }
            _ => {}
        }
    }

    /// The row of mode buttons. Clicking one switches; the number keys do the same from anywhere.
    pub(crate) fn mode_strip(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        let cur = self.mode();
        let tips = [
            ("LIBRARY", 82.0, Cmd::ModeLibrary, "Library, player and queue side by side"),
            ("PLAYER", 74.0, Cmd::ModePlayer, "Just the player and the queue"),
            ("ALBUM", 68.0, Cmd::ModeAlbum, "Big cover, lyrics and the visualizer"),
            ("MINI", 56.0, Cmd::ModeMini, "A small always-handy player window"),
        ];
        ui.horizontal_wrapped(|ui| {
            for (i, (name, w, cmd, tip)) in tips.iter().enumerate() {
                let key = self.key_text(*cmd);
                if retro_btn_w(ui, name, *w, cur == i as u8).tip(format!("{}  ({})", tip, key)).clicked() {
                    acts.push(Action::SetMode(i as u8));
                }
            }
        });
    }

    /// Mode buttons floating bottom-left in player mode, the way back to the library with the mouse.
    /// The album view stays clean (keys 1-4 / Ctrl+K still switch).
    pub(crate) fn floating_modes(&mut self, ctx: &egui::Context, acts: &mut Vec<Action>) {
        if self.mode() != 1 {
            return;
        }
        let area = egui::Area::new(egui::Id::new("floating_modes")).order(egui::Order::Foreground);
        area.anchor(egui::Align2::LEFT_BOTTOM, [10.0, -10.0]).show(ctx, |ui| {
            egui::Frame::none()
                .fill(pal().app_bg)
                .rounding(if style() != 0 { 8.0 } else { 0.0 })
                .inner_margin(4.0)
                .show(ui, |ui| self.mode_strip(ui, acts));
        });
    }

    /// Open from the keyboard (Ctrl+K / Cmd+K) or the Mac menu.
    pub(crate) fn open_palette(&mut self) {
        self.palette_open = true;
        self.palette_q.clear();
        self.palette_sel = 0;
        self.palette_frame = self.frames;
        self.search_focus = false;
    }

    /// Run an action by name: the keyboard actions (by their key-binding name) and the extras above.
    /// Put on an equalizer preset by name (and the equalizer with it).
    fn eq_preset(&mut self, want: &str) {
        if let Some((name, g)) = crate::EQ_PRESETS.iter().find(|p| p.0 == want) {
            self.eq_gains = *g;
            self.eq_on = true;
            self.apply_eq();
            self.eq_remember();
            self.dirty = true;
            self.set_note(&format!("EQUALIZER: {}", name));
        }
    }

    pub(crate) fn run_named(&mut self, id: &str) {
        if let Some(c) = CMDS.iter().find(|c| c.id == id) {
            if c.group == 2 && !PRACTICE {
                return;
            }
            let mut acts = Vec::new();
            self.run_cmd(c.cmd, &mut acts);
            for a in acts {
                self.apply(a);
            }
            return;
        }
        match id {
            "about" => {
                self.show_prefs = false;
                self.help_tab = 99; // clamps to the last tab, ABOUT
                self.show_help = true;
            }
            "prefs" => self.apply(Action::TogglePrefs),
            "quit" => self.apply(Action::QuitApp),
            "add_files" => self.apply(Action::AddFiles),
            "add_folder" => self.apply(Action::AddFolder),
            "skin" => self.apply(Action::Skin),
            "eq" => self.apply(Action::ToggleEq),
            "eq_on" => {
                self.eq_on = !self.eq_on;
                self.apply_eq();
                self.dirty = true;
                self.set_note(if self.eq_on { "EQUALIZER ON" } else { "EQUALIZER OFF" });
            }
            "eq_flat" => self.eq_preset("FLAT"),
            "eq_bass" => self.eq_preset("BASS"),
            "eq_treble" => self.eq_preset("TREBLE"),
            "eq_rock" => self.eq_preset("ROCK"),
            "spec" => self.apply(Action::ToggleSpec),
            "help" => self.apply(Action::ToggleHelp),
            "tour" => {
                self.show_help = false;
                self.tour_start(0);
            }
            "metro" if PRACTICE => self.apply(Action::MetroPanel),
            "timer" if PRACTICE => self.apply(Action::TimerPanel),
            "update_check" => {
                if matches!(self.upd_state, 0 | 3) {
                    self.start_update_check();
                    self.set_note("CHECKING FOR UPDATES...");
                }
            }
            "palette" => self.open_palette(),
            _ => {}
        }
    }

    pub(crate) fn palette_overlay(&mut self, ctx: &egui::Context) {
        if !self.palette_open {
            return;
        }
        // typing, arrows, enter, escape
        let mut run: Option<String> = None;
        let rows: Vec<(&str, &str)> =
            ITEMS.iter().filter(|i| (PRACTICE || !i.2) && matches(i.0, &self.palette_q)).map(|i| (i.0, i.1)).collect();
        if self.palette_sel >= rows.len() {
            self.palette_sel = rows.len().saturating_sub(1);
        }
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            match e {
                egui::Event::Text(s) if self.frames > self.palette_frame => {
                    let s: String = s.chars().filter(|c| !c.is_control()).collect();
                    if !s.is_empty() {
                        self.palette_q.push_str(&s);
                        self.palette_sel = 0;
                    }
                }
                egui::Event::Key { key, pressed: true, .. } => match key {
                    egui::Key::Escape => self.palette_open = false,
                    egui::Key::Backspace => {
                        self.palette_q.pop();
                        self.palette_sel = 0;
                    }
                    egui::Key::ArrowDown => self.palette_sel = (self.palette_sel + 1).min(rows.len().saturating_sub(1)),
                    egui::Key::ArrowUp => self.palette_sel = self.palette_sel.saturating_sub(1),
                    egui::Key::Enter => {
                        if let Some(r) = rows.get(self.palette_sel) {
                            run = Some(r.1.to_string());
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        if !self.palette_open {
            return;
        }

        crate::credits::block_behind(ctx, "palette");
        let dim = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("palette_dim")));
        dim.rect_filled(ctx.screen_rect(), 0.0, Color32::from_black_alpha(130));
        let w = 560.0_f32.min(ctx.screen_rect().width() - 40.0);
        let mut clicked: Option<String> = None;
        egui::Area::new(egui::Id::new("palette_area"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 70.0])
            .show(ctx, |ui| {
                let frame = egui::Frame::none()
                    .fill(pal().beige)
                    .stroke(egui::Stroke::new(2.0_f32, pal().edge))
                    .rounding(if style() != 0 { 8.0 } else { 0.0 })
                    .inner_margin(10.0);
                frame.show(ui, |ui| {
                    ui.set_width(w - 20.0);
                    // the typed text, with a blinking caret
                    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::hover());
                    crate::inset(ui.painter(), r, pal().lcd);
                    let blink = (ui.input(|i| i.time) * 2.0) as i64 % 2 == 0;
                    let shown = if self.palette_q.is_empty() {
                        "TYPE WHAT YOU WANT TO DO...".to_string()
                    } else {
                        format!("{}{}", self.palette_q.to_uppercase(), if blink { "_" } else { "" })
                    };
                    let col = if self.palette_q.is_empty() { pal().ink2 } else { pal().ink };
                    ptext_fit(
                        ui.painter(),
                        Pos2::new(r.min.x + 10.0, r.center().y),
                        Align::Min,
                        &shown,
                        2.0,
                        r.width() - 20.0,
                        col,
                    );
                    ui.add_space(6.0);
                    if rows.is_empty() {
                        crate::para(ui, "Nothing matches. Try a shorter word.", pal().ink2);
                    }
                    let show = rows.len().min(10);
                    // keep the selected row in view
                    let first = self.palette_sel.saturating_sub(show.saturating_sub(1)).min(rows.len().saturating_sub(show));
                    for (k, (label, id)) in rows.iter().enumerate().skip(first).take(show) {
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
                        let sel = k == self.palette_sel;
                        if resp.hovered() && ui.input(|i| i.pointer.delta() != Vec2::ZERO) {
                            self.palette_sel = k;
                        }
                        if sel {
                            if style() != 0 {
                                rfill(ui.painter(), r, 6.0, pal().btn_face);
                            } else {
                                raised_h(ui.painter(), r, false, true);
                            }
                        } else if style() == 0 {
                            fill_rect(ui.painter(), r, pal().beige);
                        }
                        let ink = if sel && style() == 2 { pal().bar_txt } else { pal().ink };
                        let key = CMDS.iter().position(|c| c.id == *id).map_or(String::new(), |i| {
                            self.binds.get(i).copied().flatten().map_or(String::new(), crate::prefs::key_label)
                        });
                        let kw = if key.is_empty() { 0.0 } else { 90.0 };
                        ptext_fit(
                            ui.painter(),
                            Pos2::new(r.min.x + 10.0, r.center().y),
                            Align::Min,
                            label,
                            2.0,
                            r.width() - 24.0 - kw,
                            ink,
                        );
                        if !key.is_empty() {
                            ptext(ui.painter(), Pos2::new(r.max.x - 10.0, r.center().y), Align::Max, &key, 2.0, pal().ink2);
                        }
                        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            clicked = Some(id.to_string());
                        }
                    }
                    ui.add_space(4.0);
                    crate::para(ui, "ENTER runs it   UP / DOWN choose   ESC closes", pal().ink2);
                });
            });
        if let Some(id) = run.or(clicked) {
            self.palette_open = false;
            self.run_named(&id);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_words_in_any_order() {
        assert!(matches("Mode: album view (big cover)", "album"));
        assert!(matches("Mode: album view (big cover)", "COVER  mode"));
        assert!(matches("Next song", ""));
        assert!(!matches("Next song", "next album"));
    }

    #[test]
    fn every_palette_entry_runs_something() {
        // a name must be a keyboard command or have its own case in run_named, or picking it does nothing.
        // The Mac menu bar sends the same names, so its items are checked too (that code only builds on a Mac).
        let src = include_str!("palette.rs");
        let body = &src[src.find("fn run_named").unwrap()..];
        let handled = |id: &str| {
            CMDS.iter().any(|c| c.id == id)
                || body.contains(&format!("\"{}\" =>", id))
                || body.contains(&format!("\"{}\" if", id))
        };
        let names: Vec<&str> = ITEMS.iter().map(|i| i.1).collect();
        for (i, n) in names.iter().enumerate() {
            assert!(!names[..i].contains(n), "duplicate: {}", n);
            assert!(handled(n), "palette entry {} does nothing", n);
        }
        let mac = include_str!("macmenu.rs");
        for part in mac.split("it(\"").skip(1) {
            let id = &part[..part.find('"').unwrap()];
            assert!(handled(id), "Mac menu item {} does nothing", id);
        }
    }
}
