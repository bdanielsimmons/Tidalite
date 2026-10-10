//! Two small panels from a Tidal track's right-click menu: the credits (who produced, wrote, played and mixed it)
//! and "which artist?" for a track with several.

use crate::api::{Card, Kind};
use crate::{pal, para, retro_btn, retro_btn_w, title_line, Action, App};
use eframe::egui::{self, Pos2, Rect, Sense, Vec2};

/// While a panel (or the tour) is up, nothing behind it takes clicks: an invisible layer over the whole app
/// catches them, so only the panel itself works until it is closed.
pub(crate) fn block_behind(ctx: &egui::Context, id: &str) {
    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new((id, "block"))).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
        ui.allocate_response(screen.size(), egui::Sense::click_and_drag());
    });
}

/// Dim the screen and show `body` in a centred box like the other dialogs.
pub(crate) fn panel(ctx: &egui::Context, id: &str, width: f32, body: impl FnOnce(&mut egui::Ui)) {
    block_behind(ctx, id);
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new((id, "dim"))));
    p.rect_filled(ctx.screen_rect(), 0.0, egui::Color32::from_black_alpha(150));
    let w = width.min(ctx.screen_rect().width() - 40.0);
    egui::Area::new(egui::Id::new(id)).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(
        ctx,
        |ui| {
            egui::Frame::none()
                .fill(pal().beige)
                .stroke(egui::Stroke::new(2.0_f32, pal().edge))
                .rounding(if crate::style() != 0 { 8.0 } else { 0.0 })
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.set_width(w - 24.0);
                    body(ui);
                });
        },
    );
}

impl App {
    /// Which of a track's artists to open: one button each.
    pub(crate) fn artist_picker(&mut self, ctx: &egui::Context, acts: &mut Vec<Action>) {
        let Some(list) = &self.artist_pick else { return };
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let mut picked: Option<Card> = None;
        panel(ctx, "artist_pick", 360.0, |ui| {
            title_line(ui, "GO TO WHICH ARTIST?", 3.0, pal().ink);
            ui.add_space(4.0);
            let bw = ui.available_width();
            for (id, name) in list {
                if retro_btn_w(ui, &name.to_uppercase(), bw, false).clicked() {
                    picked = Some(Card {
                        kind: Kind::Artist,
                        id: id.to_string(),
                        title: name.clone(),
                        subtitle: "Artist".to_string(),
                        image: String::new(),
                    });
                }
                ui.add_space(4.0);
            }
            ui.add_space(6.0);
            if retro_btn(ui, "CANCEL  (ESC)", false).clicked() {
                close = true;
            }
        });
        if let Some(c) = picked {
            acts.push(Action::Open(c));
            close = true;
        }
        if close {
            self.artist_pick = None;
        }
    }

    pub(crate) fn credits_overlay(&mut self, ctx: &egui::Context) {
        let Some((label, credits)) = &self.credits else { return };
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let max_h = (ctx.screen_rect().height() - 200.0).max(120.0);
        panel(ctx, "credits", 460.0, |ui| {
            title_line(ui, "CREDITS", 3.0, pal().ink);
            para(ui, label, pal().ink2);
            match credits {
                None => para(ui, "Asking Tidal...", pal().ink2),
                Some(Err(e)) => para(ui, &format!("Could not get the credits: {}", e), pal().red),
                Some(Ok(list)) if list.is_empty() => para(ui, "Tidal has no credits for this track.", pal().ink2),
                Some(Ok(list)) => {
                    egui::ScrollArea::vertical().max_height(max_h).auto_shrink([false, true]).show(ui, |ui| {
                        for (role, names) in list {
                            para(ui, &role.to_uppercase(), pal().ink2);
                            ui.add_space(-6.0);
                            para(ui, names, pal().ink);
                        }
                    });
                }
            }
            ui.add_space(4.0);
            if retro_btn(ui, "CLOSE  (ESC)", false).clicked() {
                close = true;
            }
        });
        if close {
            self.credits = None;
        }
    }

    /// The visualizer settings, with a live preview of exactly what they make.
    pub(crate) fn viz_panel(&mut self, ctx: &egui::Context) {
        let Some(mut which) = self.viz_panel else { return };
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let playing = self.cur.is_some() && !self.paused && !self.stopped;
        panel(ctx, "viz_panel", 640.0, |ui| {
            ui.horizontal(|ui| {
                title_line(ui, "VISUALIZER", 3.0, pal().ink);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if retro_btn(ui, "CLOSE  (ESC)", false).clicked() {
                        close = true;
                    }
                });
            });
            if let Some(i) = crate::tab_row(ui, &["PLAYER SCREEN", "ALBUM VIEW"], which) {
                which = i;
            }
            ui.add_space(6.0);
            // the preview: the real drawing, with the music (or a stand-in signal when nothing plays)
            let (mut vb, mut vp, mut vw) = self.viz_data(which);
            if !playing {
                let t = ui.input(|i| i.time) as f32;
                let n = self.viz[which].n.max(4);
                vb = (0..n)
                    .map(|k| (0.45 + 0.35 * (t * 2.1 + k as f32 * 0.6).sin() * (1.0 - k as f32 / n as f32 * 0.5)).clamp(0.0, 1.0))
                    .collect();
                vp = vb.iter().map(|b| (b + 0.08).min(1.0)).collect();
                vw = (0..128).map(|k| (t * 6.0 + k as f32 * 0.25).sin() * 0.5 * (t * 0.7).sin().abs()).collect();
            }
            ui.ctx().request_repaint();
            let (pv, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 150.0), Sense::hover());
            let c = &self.viz[which];
            if which == 0 && self.wa_art.is_some() {
                // a skin is worn: the player shows the skin's own visualizer, exactly as Winamp draws it
                crate::inset(ui.painter(), pv, pal().edge);
                self.skin_viz_preview(ui.painter(), pv.shrink(6.0), &vb, &vp, &vw, c.mode);
            } else if which == 0 {
                crate::inset(ui.painter(), pv, pal().lcd);
                crate::viz_draw(ui.painter(), pv.shrink(6.0), &vb, &vp, &vw, c.mode, c.w, false, 1.0, self.viz_tint(c.color));
            } else {
                ui.painter().rect_filled(pv, 0.0, pal().app_bg);
                let half = pv.width() * self.spec_w / 2.0;
                let r = Rect::from_min_max(
                    Pos2::new(pv.center().x - half, pv.max.y - pv.height() * self.spec_h.max(0.3)),
                    Pos2::new(pv.center().x + half, pv.max.y - 6.0),
                );
                crate::viz::set_edge_fade(self.spec_frame == 1);
                crate::viz_draw(ui.painter(), r, &vb, &vp, &vw, c.mode, c.w, true, self.spec_op, self.viz_tint(c.color));
                crate::viz::set_edge_fade(false);
            }
            ui.add_space(8.0);
            // the settings
            let choice = |ui: &mut egui::Ui, label_txt: &str, names: &[&str], cur: u8| -> Option<u8> {
                let mut out = None;
                ui.horizontal(|ui| {
                    crate::views::label(ui, label_txt, 140.0);
                    for (i, n) in names.iter().enumerate() {
                        if retro_btn(ui, n, cur as usize == i).clicked() {
                            out = Some(i as u8);
                        }
                    }
                });
                out
            };
            let stepper = |ui: &mut egui::Ui, label_txt: &str, v: &mut f32, d: f32, lo: f32, hi: f32, pct: bool| -> bool {
                let mut changed = false;
                ui.horizontal(|ui| {
                    crate::views::label(ui, label_txt, 140.0);
                    if retro_btn_w(ui, "-", 28.0, false).clicked() {
                        *v = (*v - d).max(lo);
                        changed = true;
                    }
                    let shown = if pct { format!("{}%", (*v * 100.0).round()) } else { format!("{}", v.round()) };
                    crate::lcd_box(ui, &shown, 70.0, pal().ink);
                    if retro_btn_w(ui, "+", 28.0, false).clicked() {
                        *v = (*v + d).min(hi);
                        changed = true;
                    }
                });
                changed
            };
            let mut dirty = false;
            if let Some(m) = choice(ui, "STYLE", &["BARS", "WAVEFORM", "BARS + WAVE"], self.viz[which].mode) {
                self.viz[which].mode = m;
                dirty = true;
            }
            // with a skin on, the player screen keeps Winamp's own: its colours, 19 bars
            let skin_screen = which == 0 && self.wa_art.is_some();
            if skin_screen {
                crate::views::dim_line(ui, "THE SKIN'S OWN VISUALIZER: ITS COLOURS, 19 BARS, AS IN WINAMP", 1.0, pal().dim);
            } else {
                if let Some(m) = choice(ui, "COLOURS", &["SKIN", "FROM COVER", "GREEN + RED"], self.viz[which].color) {
                    self.viz[which].color = m;
                    dirty = true;
                }
                let mut n = self.viz[which].n as f32;
                if stepper(ui, "BARS", &mut n, 4.0, 8.0, 96.0, false) {
                    self.viz[which].n = n as usize;
                    dirty = true;
                }
                dirty |= stepper(ui, "BAR WIDTH", &mut self.viz[which].w, 0.1, 0.3, 1.0, true);
            }
            dirty |= stepper(ui, "SENSITIVITY", &mut self.viz[which].gain, 0.2, 0.4, 3.0, true);
            if which == 1 {
                dirty |= stepper(ui, "OPACITY", &mut self.spec_op, 0.05, 0.05, 0.8, true);
                dirty |= stepper(ui, "HEIGHT", &mut self.spec_h, 0.08, 0.15, 0.9, true);
                dirty |= stepper(ui, "WIDTH", &mut self.spec_w, 0.1, 0.2, 1.0, true);
                dirty |= stepper(ui, "COVER", &mut self.art_op, 0.05, 0.5, 1.0, true);
                if let Some(f) = choice(ui, "FRAME", &["NONE", "SOFT EDGES"], self.spec_frame) {
                    self.spec_frame = f;
                    dirty = true;
                }
            }
            if dirty {
                self.dirty = true;
            }
        });
        self.viz_panel = if close { None } else { Some(which) };
    }

    /// A picture looked at big: the sharpest copy Tidal has (1280 for covers, 750 for artists) shown at a
    /// comfortable size, never blown up past its own pixels. X, ESC or a click outside closes it.
    pub(crate) fn art_preview_overlay(&mut self, ctx: &egui::Context) {
        let Some((url, title, artist)) = self.art_preview.clone() else { return };
        let big = if url.contains("resources.tidal.com") {
            let want = if artist { "750x750" } else { "1280x1280" };
            url.replace("320x320", want).replace("640x640", want)
        } else {
            url.clone()
        };
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let screen = ctx.screen_rect();
        // the dim behind; a click on it closes the picture
        let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("art_preview_dim")));
        p.rect_filled(screen, 0.0, egui::Color32::from_black_alpha(170));
        egui::Area::new(egui::Id::new("art_preview_block")).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
            if ui.allocate_response(screen.size(), egui::Sense::click()).clicked() {
                close = true;
            }
        });
        // the sharp copy when it is here (or when it cannot be had), the small one meanwhile
        let (shown, ready) = match self.images.get(&big) {
            Some(id) => (Some((id, big.clone())), true),
            None if big != url && !self.images.failed(&big) => (self.images.get(&url).map(|id| (id, url.clone())), false),
            None => (self.images.get(&url).map(|id| (id, url.clone())), true),
        };
        let ppp = ctx.pixels_per_point();
        let native = shown.as_ref().and_then(|s| self.images.size(&s.1)).map_or(640.0, |s| s[0].max(s[1]) as f32 / ppp);
        let room = (screen.width() - 80.0).min(screen.height() - 140.0);
        let side = native.min(room).min(720.0).max(160.0);
        egui::Area::new(egui::Id::new("art_preview"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(pal().beige)
                    .stroke(egui::Stroke::new(2.0_f32, pal().edge))
                    .rounding(if crate::style() != 0 { 8.0 } else { 0.0 })
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.set_width(side);
                            let at = ui.cursor().min + Vec2::new(0.0, 13.0);
                            crate::ptext_fit(ui.painter(), at, egui::Align::Min, &title, 2.0, side - 50.0, pal().ink);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if crate::Tip::tip(retro_btn_w(ui, "X", 30.0, false), "Close  (ESC)").clicked() {
                                    close = true;
                                }
                            });
                        });
                        ui.add_space(8.0);
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(side), egui::Sense::hover());
                        match shown {
                            Some((id, _)) => {
                                egui::Image::new(egui::load::SizedTexture::new(id, r.size())).paint_at(ui, r);
                            }
                            None => crate::paint_record(ui.painter(), r, Some(ui.input(|i| i.time) as f32)),
                        }
                        if !ready {
                            ui.ctx().request_repaint();
                        }
                    });
            });
        if close {
            self.art_preview = None;
        }
    }

    /// While a Winamp skin is fetched and put on: a small panel with a moving bar, so it never looks stuck. It
    /// closes by itself when the skin is on; the music keeps playing meanwhile.
    pub(crate) fn skin_loading_panel(&mut self, ctx: &egui::Context) {
        let Some(name) = self.wa_loading.clone() else { return };
        let t = ctx.input(|i| i.time) as f32;
        egui::Area::new(egui::Id::new("skin_loading"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 60.0])
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(20, 20, 22))
                    .stroke(egui::Stroke::new(2.0_f32, egui::Color32::from_gray(110)))
                    .rounding(6.0)
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.set_width(320.0);
                        title_line(ui, "PUTTING ON THE SKIN", 2.0, egui::Color32::from_rgb(240, 240, 236));
                        crate::views::dim_line(ui, &name.to_uppercase(), 1.0, egui::Color32::from_gray(190));
                        ui.add_space(6.0);
                        // a bar that sweeps back and forth
                        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 6.0), Sense::hover());
                        ui.painter().rect_filled(r, 3.0, egui::Color32::from_gray(50));
                        let w = r.width() * 0.3;
                        let x = r.min.x + (r.width() - w) * (0.5 + 0.5 * (t * 2.2).sin());
                        ui.painter().rect_filled(
                            Rect::from_min_size(Pos2::new(x, r.min.y), Vec2::new(w, r.height())),
                            3.0,
                            pal().btn_hi,
                        );
                    });
            });
        ctx.request_repaint();
    }

    /// Edit a song's title and artist (your files, YouTube and SoundCloud songs) and its key and tempo; for a Tidal
    /// song only the key and tempo, kept on this computer.
    pub(crate) fn edit_info_panel(&mut self, ctx: &egui::Context) {
        let Some((id, _, _)) = self.edit_info.clone() else { return };
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let mut save = false;
        let is_file = self.store.files.iter().any(|e| e.id == id);
        let tidal = self.ext_of(id).is_none() && !is_file;
        panel(ctx, "edit_info", 520.0, |ui| {
            title_line(ui, if tidal { "KEY AND BPM" } else { "EDIT INFO" }, 3.0, pal().ink);
            ui.add_space(6.0);
            let Some((_, title, artist)) = self.edit_info.as_mut() else { return };
            if tidal {
                para(ui, &format!("{} - {}", artist, title), pal().ink);
            }
            let (key, bpm) = (&mut self.edit_kb.0, &mut self.edit_kb.1);
            let mut rows = vec![
                ("KEY", crate::tools::F_INFO_KEY, key, "e.g. Dm, F#, Eb major"),
                ("BPM", crate::tools::F_INFO_BPM, bpm, "e.g. 120"),
            ];
            if !tidal {
                rows.insert(0, ("ARTIST", crate::tools::F_INFO_ARTIST, artist, "Artist"));
                rows.insert(0, ("TITLE", crate::tools::F_INFO_TITLE, title, "Song title"));
            }
            for (lab, fid, val, hint) in rows {
                ui.horizontal(|ui| {
                    crate::views::label(ui, lab, 80.0);
                    let w = ui.available_width() - 6.0;
                    let (r, _) = ui.allocate_exact_size(Vec2::new(w, crate::bh()), Sense::hover());
                    if crate::views::field(ui, &mut self.ed, fid, val, r, hint, false).enter {
                        save = true;
                    }
                });
                ui.add_space(4.0);
            }
            para(
                ui,
                if tidal {
                    "Only for you, on this computer: Tidal's own info stays as it is. Empty both to go back to Tidal's."
                } else if is_file {
                    "Renamed everywhere in Backline, and written into the file itself so other players see it too. Key and BPM stay in Backline."
                } else {
                    "Renamed in your lists, playlists, tunes and the queue. Tidal's own album and search pages keep Tidal's names."
                },
                pal().ink2,
            );
            ui.horizontal(|ui| {
                if retro_btn(ui, "SAVE", true).clicked() {
                    save = true;
                }
                if retro_btn(ui, "CANCEL", false).clicked() {
                    close = true;
                }
            });
        });
        if save {
            if let Some((id, title, artist)) = self.edit_info.take() {
                // key and tempo: yours win; both empty hands them back to Tidal or detection
                let key = crate::meta::parse_typed_key(&self.edit_kb.0);
                let bpm = self.edit_kb.1.trim().parse::<u16>().ok().filter(|b| (20..=400).contains(b));
                let before = crate::meta::get(id).unwrap_or_default();
                if key != before.key || bpm != before.bpm {
                    crate::meta::set_mine(id, bpm, key);
                    self.store.meta = crate::meta::saved();
                    self.store_dirty = true;
                }
                if !tidal {
                    self.apply(Action::SaveInfo(id, title, artist));
                }
            }
            self.ed.id = 0;
        } else if close {
            self.edit_info = None;
            self.ed.id = 0;
        }
    }
}
