//! Floating tools (focus timer, metronome), the lead sheet with its band, and the typed number boxes.

use crate::band;
use crate::chart;
use crate::extras::{Knob, Opt};
use crate::font::{ptext, ptext_fit, text_w};
use crate::views::{dim_line, field, label, F_CHART, F_IREAL};
use crate::{
    check_box, fill_rect, inset, lcd_box, outline, pal, para, retro_btn, retro_btn_w, title_line, Action, App, Tip, BTN_H,
};
use eframe::egui::{self, Align, Pos2, Rect, Sense, Vec2};
use std::time::Instant;

// text-field ids for the boxes below (the lists use 1..13)
pub const F_FOCUS: u32 = 30;
pub const F_REST: u32 = 31;
pub const F_BLOCKS: u32 = 32;
pub const F_BPM: u32 = 33;
pub const F_BEATS: u32 = 34;
pub const F_GAP_PLAY: u32 = 35;
pub const F_GAP_MUTE: u32 = 36;
pub const F_RAMP_BARS: u32 = 37;
pub const F_RAMP_BPM: u32 = 38;
pub const F_SHIFT: u32 = 39;
pub const F_LOOP_A: u32 = 40;
pub const F_LOOP_B: u32 = 41;
pub const F_SPEED: u32 = 42;
pub const F_TR_LOOPS: u32 = 43;
pub const F_TR_STEP: u32 = 44;
pub const F_CHART_Q: u32 = 45;
pub const F_BAND_BPM: u32 = 46;

impl App {
    /// A box you can type into. Shows `shown`; returns the typed text when you press Enter or click away.
    pub(crate) fn entry(&mut self, ui: &mut egui::Ui, id: u32, w: f32, shown: &str) -> Option<String> {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::hover());
        let editing = self.ed.id == id;
        if editing && self.ebuf_id != id {
            self.ebuf = shown.to_string();
            self.ebuf_id = id;
        }
        let out = if editing {
            field(ui, &mut self.ed, id, &mut self.ebuf, rect, "", false)
        } else {
            let mut tmp = shown.to_string();
            field(ui, &mut self.ed, id, &mut tmp, rect, "", false)
        };
        if self.ebuf_id == id && (out.enter || self.ed.id != id) {
            if out.enter {
                self.ed.id = 0;
            }
            self.ebuf_id = 0;
            return Some(std::mem::take(&mut self.ebuf));
        }
        None
    }

    /// `-  [typed number]  +`
    pub(crate) fn num_step(
        &mut self,
        ui: &mut egui::Ui,
        acts: &mut Vec<Action>,
        id: u32,
        w: f32,
        k: Knob,
        shown: String,
        tip: &str,
    ) {
        if retro_btn_w(ui, "-", 28.0, false).tip(tip).clicked() {
            acts.push(Action::Knob(k, -1));
        }
        if let Some(t) = self.entry(ui, id, w, &shown) {
            acts.push(Action::SetKnob(k, t));
        }
        if retro_btn_w(ui, "+", 28.0, false).clicked() {
            acts.push(Action::Knob(k, 1));
        }
        ui.add_space(6.0);
    }

    // ---------------------------------------------------------------- TIMER
    /// Flash border + the focus timer (and the metronome) floating over every screen.
    pub(crate) fn floating_tools(&mut self, ctx: &egui::Context, acts: &mut Vec<Action>) {
        if !crate::PRACTICE {
            return;
        }
        let flash = self.pomo_flash.map_or(false, |t| self.pomo == 3 || t.elapsed().as_secs() < 6);
        let blink = flash && (ctx.input(|i| i.time) * 2.5) as u32 % 2 == 0;
        if flash {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
            if blink {
                let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("flash")));
                outline(&p, ctx.screen_rect(), 6.0, pal().ink);
            }
        }
        let timer_on = self.pomo != 0 || self.timer_open;
        if timer_on {
            egui::Area::new(egui::Id::new("timer"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::RIGHT_BOTTOM, [-10.0, -10.0])
                .show(ctx, |ui| self.timer_box(ui, acts, blink));
        }
        if self.metro_open {
            egui::Area::new(egui::Id::new("metro"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::RIGHT_BOTTOM, [-10.0, if timer_on { -110.0 } else { -10.0 }])
                .show(ctx, |ui| self.metro_box(ui, acts));
        }
    }

    fn timer_box(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>, blink: bool) {
        let frame = egui::Frame::none().fill(pal().app_bg).stroke(egui::Stroke::new(2.0, pal().edge)).inner_margin(6.0);
        frame.show(ui, |ui| {
            let left = self.pomo_end.map_or(0, |e| e.saturating_duration_since(Instant::now()).as_secs());
            let clock = format!("{}:{:02}", left / 60, left % 60);
            let blocks = format!("{}/{}", self.pomo_n, self.pomo_cycles);
            match self.pomo {
                1 => {
                    ui.horizontal(|ui| {
                        lcd_box(ui, &format!("FOCUS {}   {}", clock, blocks), 210.0, pal().ink);
                        if retro_btn_w(ui, "STOP", 60.0, false).clicked() {
                            acts.push(Action::Pomo);
                        }
                    });
                }
                2 => {
                    ui.horizontal(|ui| {
                        lcd_box(ui, &format!("REST {}   {}", clock, blocks), 210.0, pal().ink2);
                        if retro_btn_w(ui, "STOP", 60.0, false).clicked() {
                            acts.push(Action::Pomo);
                        }
                    });
                }
                3 => {
                    ui.horizontal(|ui| {
                        lcd_box(ui, "REST OVER", 110.0, if blink { pal().ink2 } else { pal().ink });
                        if retro_btn_w(ui, "NEXT FOCUS", 120.0, blink).clicked() {
                            acts.push(Action::Pomo);
                        }
                        if retro_btn_w(ui, "STOP", 60.0, false).clicked() {
                            acts.push(Action::Pomo);
                            acts.push(Action::Pomo);
                        }
                    });
                }
                _ => {
                    ui.horizontal(|ui| {
                        label(ui, "FOCUS", 60.0);
                        let s = self.pomo_focus.to_string();
                        self.num_step(ui, acts, F_FOCUS, 44.0, Knob::Focus, s, "Minutes of focus");
                        label(ui, "REST", 44.0);
                        let s = self.pomo_break.to_string();
                        self.num_step(ui, acts, F_REST, 40.0, Knob::Rest, s, "Minutes of rest");
                        label(ui, "BLOCKS", 66.0);
                        let s = self.pomo_cycles.to_string();
                        self.num_step(ui, acts, F_BLOCKS, 40.0, Knob::Blocks, s, "How many focus blocks");
                    });
                    ui.horizontal(|ui| {
                        if check_box(ui, "SOUND", self.pomo_sound)
                            .tip("Play a soft chime when a block ends (off = just flash)")
                            .clicked()
                        {
                            acts.push(Action::Opt(Opt::PomoSound));
                        }
                        if retro_btn_w(ui, "START", 80.0, false).clicked() {
                            acts.push(Action::Pomo);
                        }
                        if retro_btn_w(ui, "CLOSE", 70.0, false).clicked() {
                            acts.push(Action::TimerPanel);
                        }
                    });
                }
            }
        });
    }

    // ------------------------------------------------------------ METRONOME
    fn metro_box(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        let frame = egui::Frame::none().fill(pal().app_bg).stroke(egui::Stroke::new(2.0, pal().edge)).inner_margin(8.0);
        frame.show(ui, |ui| {
            ui.set_width(560.0);
            let on = self.mt.on;
            ui.horizontal(|ui| {
                if retro_btn_w(ui, if on { "STOP" } else { "START" }, 84.0, on).clicked() {
                    acts.push(Action::Opt(Opt::MetroOn));
                }
                label(ui, "BPM", 40.0);
                let s = self.knob_bpm().to_string();
                self.num_step(ui, acts, F_BPM, 52.0, Knob::Bpm, s, "Beats per minute");
                if retro_btn_w(ui, "TAP", 52.0, false).tip("Tap along with the music to find the tempo").clicked() {
                    acts.push(Action::TapTempo);
                }
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if retro_btn_w(ui, "CLOSE", 70.0, false).clicked() {
                        acts.push(Action::MetroPanel);
                    }
                });
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                self.metro_vis(ui, acts);
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                label(ui, "BEATS", 60.0);
                let s = self.mt.beats.to_string();
                self.num_step(ui, acts, F_BEATS, 36.0, Knob::Beats, s, "Beats per bar");
                let sub = ["NO SUBDIV", "EIGHTHS", "TRIPLETS", "SIXTEENTHS"][(self.mt.sub as usize).min(3)];
                if retro_btn_w(ui, sub, 120.0, self.mt.sub > 0).tip("Quiet ticks between the beats").clicked() {
                    acts.push(Action::Opt(Opt::Sub));
                }
                for (i, name) in ["ALL", "2 & 4", "1 ONLY"].iter().enumerate() {
                    if retro_btn_w(ui, name, 66.0, false).tip("Preset beat volumes (click a beat to set its own)").clicked() {
                        acts.push(Action::Opt(Opt::Preset(i as u8)));
                    }
                }
            });
            ui.horizontal(|ui| {
                label(ui, "GAP", 60.0);
                let p = if self.mt.gap_play == 0 { "OFF".to_string() } else { self.mt.gap_play.to_string() };
                self.num_step(ui, acts, F_GAP_PLAY, 44.0, Knob::GapPlay, p, "Bars of click, then silence (0 = off)");
                label(ui, "THEN MUTE", 90.0);
                let m = self.mt.gap_mute.to_string();
                self.num_step(ui, acts, F_GAP_MUTE, 36.0, Knob::GapMute, m, "Silent bars");
            });
            ui.horizontal(|ui| {
                label(ui, "FASTER", 60.0);
                let r = if self.mt.ramp_bars == 0 { "OFF".to_string() } else { self.mt.ramp_bars.to_string() };
                self.num_step(ui, acts, F_RAMP_BARS, 44.0, Knob::RampBars, r, "Every this many bars the tempo goes up (0 = off)");
                label(ui, "BARS  +", 70.0);
                let b = self.mt.ramp_bpm.to_string();
                self.num_step(ui, acts, F_RAMP_BPM, 36.0, Knob::RampBpm, b, "BPM added each time");
                label(ui, "BPM", 40.0);
            });
            if self.cur.is_some() && !self.stopped {
                ui.horizontal(|ui| {
                    if check_box(ui, "FOLLOW SPEED", self.mt.follow).tip("Click gets slower / faster with the track").clicked() {
                        acts.push(Action::Opt(Opt::Follow));
                    }
                    let tip = "Find the beat of this track and click along with it";
                    if check_box(ui, "LOCK TO TRACK", self.mt.sync).tip(tip).clicked() {
                        acts.push(Action::Opt(Opt::Sync));
                    }
                    let busy = self.beat_busy;
                    if retro_btn_w(ui, if busy { "LISTENING..." } else { "FIND BEAT" }, 110.0, busy).clicked() {
                        acts.push(Action::Opt(Opt::FindBeats));
                    }
                });
                if self.beat.is_some() {
                    ui.horizontal(|ui| {
                        label(ui, "TRACK BEAT", 100.0);
                        let bpm = self.beat.map_or(0.0, |b| 60.0 / b.1);
                        lcd_box(ui, &format!("{:.0} BPM", bpm), 80.0, pal().ink);
                        if retro_btn_w(ui, "x2", 40.0, false).tip("It found half the tempo: double it").clicked() {
                            acts.push(Action::Opt(Opt::Double));
                        }
                        if retro_btn_w(ui, "/2", 40.0, false).tip("It found double the tempo: halve it").clicked() {
                            acts.push(Action::Opt(Opt::Half));
                        }
                        label(ui, "SHIFT", 56.0);
                        let s = format!("{}", self.mt.shift_ms);
                        self.num_step(ui, acts, F_SHIFT, 52.0, Knob::Shift, s, "Nudge the click earlier / later (milliseconds)");
                    });
                }
            }
        });
    }

    /// Tempo shown in the box: the playing track's, else the metronome's own.
    fn knob_bpm(&self) -> u32 {
        if self.bpm > 0 {
            self.bpm
        } else {
            self.mt_bpm
        }
    }

    /// Beat boxes (click one to change its volume) and a swinging pendulum.
    fn metro_vis(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        let beats = self.mt.beats.clamp(1, 12) as usize;
        // where are we in the bar?
        let (beat_no, frac, muted) = match (self.mt.on, self.mt_vis) {
            (true, Some((t0, spb, cycle))) => {
                let now = Instant::now();
                if now < t0 {
                    (None, 0.0, false)
                } else {
                    let e = (now - t0).as_secs_f32() / spb.max(0.05);
                    let n = e.floor() as usize % cycle.max(1);
                    let silent = self.mt.gap_play > 0 && n >= self.mt.gap_play as usize * beats;
                    (Some(n % beats), e.fract(), silent)
                }
            }
            _ => (None, 0.0, false),
        };
        if self.mt.on {
            ui.ctx().request_repaint();
        }

        // pendulum
        let (rect, _) = ui.allocate_exact_size(Vec2::new(96.0, 104.0), Sense::hover());
        inset(ui.painter(), rect, pal().lcd);
        let p = ui.painter();
        let pivot = Pos2::new(rect.center().x, rect.max.y - 12.0);
        let swing = match (self.mt.on, self.mt_vis) {
            (true, Some((t0, spb, _))) if Instant::now() >= t0 => {
                let e = (Instant::now() - t0).as_secs_f32() / spb.max(0.05);
                (std::f32::consts::PI * e).cos()
            }
            _ => 0.0,
        };
        let ang = swing * 0.62;
        let len = 82.0;
        let tip = pivot + Vec2::new(ang.sin() * len, -ang.cos() * len);
        let step = 3.0;
        let n = (len / step) as i32;
        for k in 0..=n {
            let q = pivot + (tip - pivot) * (k as f32 / n as f32);
            fill_rect(p, Rect::from_center_size(q, Vec2::splat(3.0)), pal().ink2);
        }
        let weight = pivot + (tip - pivot) * 0.7;
        fill_rect(p, Rect::from_center_size(weight, Vec2::new(14.0, 10.0)), pal().ink);
        let hit = self.mt.on && frac < 0.18 && !muted;
        fill_rect(p, Rect::from_center_size(tip, Vec2::splat(10.0)), if hit { pal().red } else { pal().ink });
        fill_rect(p, Rect::from_center_size(pivot, Vec2::new(30.0, 8.0)), pal().ink);
        ptext(
            p,
            Pos2::new(rect.min.x + 6.0, rect.min.y + 8.0),
            Align::Min,
            "MUTED",
            1.0,
            if muted { pal().red } else { pal().lcd },
        );

        // beat boxes
        ui.add_space(6.0);
        let w = ((ui.available_width() - 6.0) / beats as f32 - 4.0).clamp(26.0, 56.0);
        for b in 0..beats {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 104.0), Sense::click());
            let lvl = self.mt.levels[b].min(3) as usize;
            let now_beat = beat_no == Some(b) && !muted;
            inset(ui.painter(), r, if now_beat && lvl > 0 { pal().sel } else { pal().lcd });
            let p = ui.painter();
            // volume pips (bottom = 1)
            for k in 0..3usize {
                let pip = Rect::from_min_size(
                    Pos2::new(r.min.x + 6.0, r.max.y - 26.0 - k as f32 * 22.0),
                    Vec2::new(r.width() - 12.0, 16.0),
                );
                let lit = k < lvl;
                fill_rect(
                    p,
                    pip,
                    if lit {
                        if now_beat {
                            pal().red
                        } else {
                            pal().ink2
                        }
                    } else {
                        pal().lcd_ghost
                    },
                );
            }
            ptext(
                p,
                Pos2::new(r.center().x, r.min.y + 9.0),
                Align::Center,
                &(b + 1).to_string(),
                2.0,
                if now_beat { pal().red } else { pal().ink },
            );
            if now_beat {
                outline(p, r, 2.0, pal().red);
            }
            if resp
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .tip("Click to change this beat's volume (3 loud ... 0 silent)")
                .clicked()
            {
                acts.push(Action::Opt(Opt::Level(b)));
            }
        }
    }

    // ---------------------------------------------------------- LEAD SHEET
    pub(crate) fn chart_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        let ti = self.chart_tune();
        let playing_title = self.cur_track().filter(|_| !self.stopped).map(|t| t.title.clone());
        // what is on show: a saved tune, or whatever is playing (looked up on its own)
        let (name, key, bpm, info, text, live) = match (ti, playing_title) {
            (Some(i), _) => {
                let t = &self.store.tunes[i];
                (t.name.clone(), t.key.clone(), t.bpm, t.info.clone(), t.chart.clone(), false)
            }
            (None, Some(title)) => {
                let q = chart::clean_title(&title);
                if self.live_q != q && !self.chart_busy {
                    acts.push(Action::LiveChart(q.clone(), None));
                }
                let (txt, key, comp) = if self.live_q == q { self.live.clone().unwrap_or_default() } else { Default::default() };
                let info = if comp.is_empty() { String::new() } else { format!("written by {}", comp) };
                (q, key, 0, info, txt, true)
            }
            (None, None) => {
                ui.add_space(8.0);
                para(
                    ui,
                    "Play a song, or open a tune in TUNES, and its chord changes show up here. They are looked up for you.",
                    pal().ink2,
                );
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
            }
        };
        // a tune with no chart yet is looked up once on its own
        if let (Some(i), true) = (ti, text.trim().is_empty()) {
            if !self.chart_tried.contains(&name) && !self.chart_busy {
                self.chart_tried.insert(name.clone());
                acts.push(Action::FindChart(i, None));
            }
        }
        let busy = self.chart_busy;
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

        let chart = self.chart_for(&text);
        let has = !chart.bars.is_empty();
        ui.horizontal(|ui| {
            for (lab, semis, tip) in [
                ("CONCERT", 0, "As written (C instruments)"),
                ("Bb", 2, "For Bb horns: trumpet, tenor sax"),
                ("Eb", 9, "For Eb horns: alto and bari sax"),
                ("F", 7, "For F horn"),
            ] {
                if retro_btn(ui, lab, self.chart_tr == semis).tip(tip).clicked() {
                    self.chart_tr = semis;
                }
            }
            if retro_btn(ui, "I-IV-V", self.chart_rn).tip("Roman numerals under the chords, relative to the key").clicked() {
                acts.push(Action::ToggleNumerals);
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if let Some(i) = ti {
                    if retro_btn(ui, if self.chart_edit { "DONE" } else { "EDIT" }, self.chart_edit).clicked() {
                        self.chart_edit = !self.chart_edit;
                        self.ed.id = 0;
                    }
                    if retro_btn(ui, if busy { "LOOKING..." } else { "REFRESH" }, busy).tip("Look the chart up again").clicked() {
                        acts.push(Action::FindChart(i, None));
                    }
                } else {
                    if has && retro_btn(ui, "SAVE TO TUNES", false).tip("Keep this song as a tune, with its chart").clicked() {
                        acts.push(Action::SaveLive);
                    }
                    if retro_btn(ui, if busy { "LOOKING..." } else { "REFRESH" }, busy).tip("Look the chart up again").clicked() {
                        self.live_q.clear();
                    }
                }
            });
        });

        // ---- the band
        if has {
            ui.horizontal(|ui| {
                let on = self.band_on;
                if retro_btn_w(ui, if on { "STOP BAND" } else { "PLAY BAND" }, 116.0, on)
                    .tip("A little band plays the changes: bass, chords and drums")
                    .clicked()
                {
                    acts.push(Action::Opt(Opt::Band));
                }
                if retro_btn_w(ui, band::STYLES[self.band_style as usize % band::STYLES.len()], 96.0, false)
                    .tip("Feel: swing, ballad, bossa, shuffle (gospel), straight")
                    .clicked()
                {
                    acts.push(Action::Opt(Opt::BandStyle));
                }
                label(ui, "BPM", 36.0);
                let s = self.knob_bpm().to_string();
                self.num_step(ui, acts, F_BAND_BPM, 46.0, Knob::Bpm, s, "Band tempo");
                for (i, n) in ["BASS", "CHORDS", "DRUMS"].iter().enumerate() {
                    if retro_btn_w(ui, n, 70.0, self.band_parts[i]).tip("Turn this part on or off").clicked() {
                        acts.push(Action::Opt(Opt::Part(i)));
                    }
                }
            });
            if self.band_on && self.band_chart != text {
                self.band_stop();
            }
        }
        ui.add_space(2.0);

        if self.chart_edit && ti.is_some() {
            let ti = ti.unwrap_or(0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                para(ui, "Type the changes: one bar between each |, sections like *A, time like T44. { } repeat, [1 [2 endings, @coda @segno @fine @dc. % repeats the last bar. Example:", pal().ink2);
                para(ui, "T44 *A { Dm7 G7 | Cmaj7 } | *B | Em7b5 A7 | Dm7 G7 |", pal().ink);
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

        if !has {
            ui.add_space(6.0);
            if busy {
                para(ui, "Looking for the changes...", pal().ink2);
            } else {
                para(ui, "No chart found for this name yet. The free chart list covers jazz standards. If you know it by another name, or a line of the lyric, try it here:", pal().ink2);
            }
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 90.0).max(80.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::hover());
                let o = field(ui, &mut self.ed, F_CHART_Q, &mut self.chart_q, rect, "Other title or words of the lyric", false);
                if (o.enter || retro_btn_w(ui, "FIND", 80.0, false).clicked()) && !self.chart_q.trim().is_empty() {
                    let q = self.chart_q.trim().to_string();
                    acts.push(match ti {
                        Some(i) => Action::FindChart(i, Some(q)),
                        None => Action::LiveChart(name.clone(), Some(q)),
                    });
                }
            });
            if !self.chart_sugg.is_empty() {
                dim_line(ui, "DID YOU MEAN:", 2.0, pal().ink2);
                for s in self.chart_sugg.clone() {
                    if retro_btn_w(ui, &s, (text_w(&s, 2.0) + 24.0).min(ui.available_width()), false).clicked() {
                        acts.push(match ti {
                            Some(i) => Action::FindChart(i, Some(s.clone())),
                            None => Action::LiveChart(name.clone(), Some(s.clone())),
                        });
                    }
                }
            }
            para(ui, "Or press EDIT to type the changes in, or paste an iReal Pro link.", pal().ink2);
            if live {
                para(ui, "Tip: save the song to your tunes first, then EDIT works on it.", pal().dim);
            }
            return;
        }
        self.chart_grid(ui, &chart, &key);
    }

    /// The bars of the chart: repeat signs, endings, coda marks, numerals and the band's position.
    fn chart_grid(&mut self, ui: &mut egui::Ui, chart: &chart::Chart, key: &str) {
        let tr = self.chart_tr;
        let tonic = if self.chart_rn { chart::tonic(key, chart) } else { None };
        // which bar the band is on, and which beat
        let (now_bar, now_beat) = if self.band_on {
            ui.ctx().request_repaint();
            let t = self.band_t0.elapsed().as_secs_f32() - self.band_lead;
            if t < 0.0 || self.band_order.is_empty() {
                (None, 0)
            } else {
                let b = t / self.band_bar.max(0.1);
                let k = b.floor() as usize % self.band_order.len();
                (self.band_order.get(k).copied(), (b.fract() * chart.beats as f32) as usize)
            }
        } else {
            (None, 0)
        };
        // bars inside a 1st / 2nd ending bracket
        let mut in_end = vec![false; chart.bars.len()];
        let mut open = false;
        for (i, b) in chart.bars.iter().enumerate() {
            if !b.ending.is_empty() {
                open = true;
            } else if i > 0 && (chart.bars[i - 1].end_rep || !b.label.is_empty() || b.start_rep) {
                open = false;
            }
            in_end[i] = open;
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(4.0);
            let w = ui.available_width() - 6.0;
            let per: usize = if w >= 760.0 { 8 } else { 4 };
            let gap = 6.0;
            let cw = ((w - gap * (per as f32 - 1.0)) / per as f32).floor().max(40.0);
            let ch = if tonic.is_some() { 64.0 } else { 54.0 };
            for (ri, row) in chart.bars.chunks(per).enumerate() {
                let (rr, _) = ui.allocate_exact_size(Vec2::new(w, ch + 8.0), Sense::hover());
                if !ui.is_rect_visible(rr) {
                    continue;
                }
                for (ci, bar) in row.iter().enumerate() {
                    let idx = ri * per + ci;
                    let cell =
                        Rect::from_min_size(Pos2::new(rr.min.x + ci as f32 * (cw + gap), rr.min.y + 4.0), Vec2::new(cw, ch));
                    let p = ui.painter();
                    let now = now_bar == Some(idx);
                    inset(p, cell, if now { pal().sel } else { pal().lcd });
                    // ending bracket
                    if in_end[idx] {
                        fill_rect(
                            p,
                            Rect::from_min_size(cell.min + Vec2::new(2.0, 2.0), Vec2::new(cell.width() - 4.0, 2.0)),
                            pal().ink,
                        );
                        if !bar.ending.is_empty() {
                            fill_rect(p, Rect::from_min_size(cell.min + Vec2::new(2.0, 2.0), Vec2::new(2.0, 16.0)), pal().ink);
                            ptext(p, cell.min + Vec2::new(8.0, 12.0), Align::Min, &bar.ending, 1.0, pal().ink);
                        }
                    }
                    // repeat signs
                    for (on, left) in [(bar.start_rep, true), (bar.end_rep, false)] {
                        if !on {
                            continue;
                        }
                        let x = if left { cell.min.x + 5.0 } else { cell.max.x - 8.0 };
                        fill_rect(p, Rect::from_min_size(Pos2::new(x, cell.min.y + 5.0), Vec2::new(3.0, ch - 10.0)), pal().ink);
                        let dx = if left { x + 6.0 } else { x - 5.0 };
                        for dy in [ch * 0.38, ch * 0.62] {
                            fill_rect(p, Rect::from_min_size(Pos2::new(dx, cell.min.y + dy), Vec2::splat(3.0)), pal().ink);
                        }
                    }
                    let mut top = 0.0;
                    if !bar.label.is_empty() {
                        let lw = text_w(&bar.label, 2.0) + 10.0;
                        let tag = Rect::from_min_size(cell.min + Vec2::new(14.0, 4.0), Vec2::new(lw, 16.0));
                        fill_rect(p, tag, pal().edge);
                        ptext(p, tag.center(), Align::Center, &bar.label, 2.0, pal().trim);
                        top = 6.0;
                    }
                    if !bar.marks.is_empty() {
                        ptext_fit(
                            p,
                            Pos2::new(cell.max.x - 6.0, cell.min.y + 10.0),
                            Align::Max,
                            &bar.marks.join(" "),
                            1.0,
                            cell.width() - 40.0,
                            pal().red,
                        );
                    }
                    let cy = cell.center().y + top - if tonic.is_some() { 4.0 } else { 0.0 };
                    if bar.repeat {
                        // a chord that goes on: the repeat sign, drawn as a pixel slash with two dots
                        let c = Pos2::new(cell.center().x, cy);
                        for k in -3..=3i32 {
                            fill_rect(
                                p,
                                Rect::from_center_size(c + Vec2::new(k as f32 * 4.0, -k as f32 * 4.0), Vec2::splat(4.0)),
                                pal().ink2,
                            );
                        }
                        fill_rect(p, Rect::from_center_size(c + Vec2::new(-13.0, -9.0), Vec2::splat(4.0)), pal().ink2);
                        fill_rect(p, Rect::from_center_size(c + Vec2::new(13.0, 9.0), Vec2::splat(4.0)), pal().ink2);
                    } else {
                        let n = bar.chords.len().max(1);
                        let slot = (cell.width() - 24.0) / n as f32;
                        for (k, c) in bar.chords.iter().enumerate() {
                            let shown = chart::transpose(c, tr);
                            let px = if n == 1 { 3.0 } else { 2.0 };
                            let cx = cell.min.x + 12.0 + slot * (k as f32 + 0.5);
                            if let Some(t) = tonic {
                                ptext_fit(
                                    p,
                                    Pos2::new(cx, cy + 19.0),
                                    Align::Center,
                                    &chart::roman(c, t),
                                    1.0,
                                    slot - 4.0,
                                    pal().ink2,
                                );
                            }
                            ptext_fit(
                                p,
                                Pos2::new(cx, cy - if tonic.is_some() { 5.0 } else { 0.0 }),
                                Align::Center,
                                &shown,
                                px,
                                slot - 4.0,
                                pal().ink,
                            );
                        }
                    }
                    ptext(p, Pos2::new(cell.max.x - 5.0, cell.max.y - 6.0), Align::Max, &(idx + 1).to_string(), 1.0, pal().dim);
                    if now {
                        outline(p, cell, 2.0, pal().red);
                        // beat pips along the bottom
                        for b in 0..chart.beats as usize {
                            let r = Rect::from_min_size(
                                Pos2::new(cell.min.x + 12.0 + b as f32 * 8.0, cell.max.y - 8.0),
                                Vec2::splat(4.0),
                            );
                            fill_rect(p, r, if b == now_beat { pal().red } else { pal().groove });
                        }
                    }
                }
            }
            ui.add_space(14.0);
        });
    }
}
