//! The library window (left): sign-in, search box, the source lists, and the log / EQ / storage views.

use super::*;

impl App {
    pub(crate) fn login_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
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
    pub(crate) fn search_input(&mut self, ui: &mut egui::Ui, rect: Rect, acts: &mut Vec<Action>) {
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

    pub(crate) fn log_view(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
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

    /// The presets as little cards, each with a preview of its curve. True when one was picked.
    pub(crate) fn eq_presets(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        for (name, g) in EQ_PRESETS.iter() {
            let (card, r) = ui.allocate_exact_size(Vec2::new(96.0, 40.0), Sense::click());
            let r = r.on_hover_cursor(egui::CursorIcon::PointingHand).tip(format!("{} preset", name));
            let on = self.eq_on && self.eq_gains == *g;
            inset(ui.painter(), card, if on || r.hovered() { pal().sel } else { pal().lcd });
            crate::ui_text(ui.painter(), Pos2::new(card.min.x + 6.0, card.min.y + 8.0), Align::Min, name, 1.0, 84.0, pal().ink);
            let gr = Rect::from_min_max(Pos2::new(card.min.x + 6.0, card.min.y + 15.0), card.max - Vec2::new(6.0, 5.0));
            let pts: Vec<Pos2> = (0..10)
                .map(|i| Pos2::new(gr.min.x + gr.width() * i as f32 / 9.0, gr.center().y - g[i] / 12.0 * gr.height() * 0.5))
                .collect();
            ui.painter().add(egui::Shape::line(pts, Stroke::new(1.5_f32, pal().red)));
            if r.clicked() {
                self.eq_gains = *g;
                self.eq_on = true;
                changed = true;
            }
        }
        changed
    }

    /// The equalizer, docked under the player (as Winamp stacks it): the skin's own with a skin on, the
    /// preset cards under it; Tidalite's otherwise. X (or the skin's close button) puts it away.
    pub(crate) fn eq_dock(&mut self, ui: &mut egui::Ui) {
        let inner = ui.max_rect();
        window_deco(ui, inner, "EQUALIZER");
        // the close button, in the title strip
        let xr = Rect::from_min_size(Pos2::new(inner.max.x - 34.0, inner.min.y - 26.0), Vec2::new(30.0, 22.0));
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(xr), |ui| {
            if retro_btn_w(ui, "X", 30.0, false).tip("Close the equalizer").clicked() {
                self.show_eq = false;
            }
        });
        if self.wa_art.is_some() {
            let full = ui.available_rect_before_wrap();
            let eq_area = Rect::from_min_max(full.min, Pos2::new(full.max.x, full.max.y - 46.0));
            self.winamp_eq(ui, eq_area);
            let strip = Rect::from_min_max(Pos2::new(full.min.x, full.max.y - 42.0), full.max);
            let mut changed = false;
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(strip), |ui| {
                ui.horizontal(|ui| {
                    changed = self.eq_presets(ui);
                });
            });
            if changed {
                self.apply_eq();
                self.eq_remember();
                self.dirty = true;
            }
        } else {
            self.eq_view(ui);
        }
    }

    pub(crate) fn eq_view(&mut self, ui: &mut egui::Ui) {
        const LABELS: [&str; 10] = ["31", "62", "125", "250", "500", "1K", "2K", "4K", "8K", "16K"];
        ui.add_space(6.0);
        let mut changed = false;
        ui.horizontal_wrapped(|ui| {
            ui.add_space(4.0);
            if retro_btn_w(ui, if self.eq_on { "EQ ON" } else { "EQ OFF" }, 84.0, self.eq_on).clicked() {
                self.eq_on = !self.eq_on;
                changed = true;
            }
            if retro_btn_w(ui, "AUTO", 64.0, self.eq_auto)
                .tip("Remember the EQ for each song and bring it back when it plays")
                .clicked()
            {
                self.eq_auto = !self.eq_auto;
                self.dirty = true;
            }
            ui.add_space(8.0);
            changed |= self.eq_presets(ui);
        });
        ui.add_space(8.0);
        // Winamp-style: the response curve on a little screen, then the sliders, each filled green to red
        // from the middle line to its knob
        let heat = |g: f32| -> Color32 {
            // -12 dB green, 0 yellow-green, +12 red
            let f = ((g + 12.0) / 24.0).clamp(0.0, 1.0);
            let (r, gr) = if f < 0.5 { (f * 2.0 * 230.0, 210.0) } else { (230.0, 210.0 - (f - 0.5) * 2.0 * 170.0) };
            Color32::from_rgb(r as u8, gr as u8, 40)
        };
        let eq_on = self.eq_on;
        let col_on = move |c: Color32| if eq_on { c } else { c.gamma_multiply(0.35) };
        let area = ui.available_rect_before_wrap();
        let scale_w = 40.0;
        // the curve
        let (graph, _) = ui.allocate_exact_size(Vec2::new(area.width(), 46.0), Sense::hover());
        inset(ui.painter(), graph, pal().lcd);
        let gi = graph.shrink2(Vec2::new(10.0, 6.0));
        let gx = |i: usize| gi.min.x + scale_w + (gi.width() - scale_w) * (i as f32 + 0.5) / 10.0;
        let gy = |g: f32| gi.center().y - g / 12.0 * gi.height() * 0.5;
        ui.painter().line_segment(
            [Pos2::new(gi.min.x + scale_w, gi.center().y), Pos2::new(gi.max.x, gi.center().y)],
            Stroke::new(1.0_f32, pal().dim.gamma_multiply(0.6)),
        );
        // a smooth line through the ten bands (each piece coloured by its height)
        let pts: Vec<(f32, f32)> = (0..10).map(|i| (gx(i), self.eq_gains[i])).collect();
        for w in 0..9 {
            let (x0, g0) = pts[w];
            let (x1, g1) = pts[w + 1];
            for s in 0..8 {
                let (u0, u1) = (s as f32 / 8.0, (s + 1) as f32 / 8.0);
                let ease = |u: f32| u * u * (3.0 - 2.0 * u);
                let ga = g0 + (g1 - g0) * ease(u0);
                let gb = g0 + (g1 - g0) * ease(u1);
                ui.painter().line_segment(
                    [Pos2::new(x0 + (x1 - x0) * u0, gy(ga)), Pos2::new(x0 + (x1 - x0) * u1, gy(gb))],
                    Stroke::new(2.0_f32, col_on(heat((ga + gb) / 2.0))),
                );
            }
        }
        ptext(ui.painter(), Pos2::new(gi.min.x, gi.center().y), Align::Min, "CURVE", 1.0, pal().ink2);
        ui.add_space(8.0);

        let area = ui.available_rect_before_wrap();
        let h = (area.height() - 30.0).clamp(80.0, 220.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(area.width(), h), Sense::hover());
        let bands = Rect::from_min_max(Pos2::new(rect.min.x + scale_w, rect.min.y), rect.max);
        let col_w = bands.width() / 10.0;
        let (top, bot) = (rect.min.y + 18.0, rect.max.y - 18.0);
        let p = ui.painter().clone();
        let y0 = top + (bot - top) * 0.5;
        // the dB scale
        for (g, s) in [(12.0f32, "+12"), (0.0, "0 DB"), (-12.0, "-12")] {
            let y = top + (bot - top) * (0.5 - g / 24.0);
            ptext(&p, Pos2::new(rect.min.x + scale_w - 8.0, y), Align::Max, s, 1.0, pal().ink2);
        }
        fill_rect(&p, Rect::from_min_max(Pos2::new(bands.min.x + 4.0, y0), Pos2::new(bands.max.x - 4.0, y0 + 1.0)), pal().dim);
        for i in 0..10 {
            let cx = bands.min.x + col_w * (i as f32 + 0.5);
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
            let tw = (col_w * 0.28).clamp(6.0, 12.0);
            inset(&p, Rect::from_min_max(Pos2::new(cx - tw / 2.0, top), Pos2::new(cx + tw / 2.0, bot)), pal().groove);
            let y = top + (bot - top) * (0.5 - g / 24.0);
            // the fill: from the middle line to the knob, in the colour of how far it is pushed
            let (fy0, fy1) = if y < y0 { (y, y0) } else { (y0, y) };
            if fy1 - fy0 > 1.0 {
                fill_rect(
                    &p,
                    Rect::from_min_max(Pos2::new(cx - tw / 2.0 + 2.0, fy0), Pos2::new(cx + tw / 2.0 - 2.0, fy1)),
                    col_on(heat(g)),
                );
            }
            raised(&p, Rect::from_center_size(Pos2::new(cx, y), Vec2::new((col_w - 10.0).clamp(12.0, 22.0), 8.0)), r.dragged());
            let gt = if g.abs() < 0.05 { "0".to_string() } else { format!("{:+}", g.round() as i32) };
            ptext(&p, Pos2::new(cx, rect.min.y + 8.0), Align::Center, &gt, 1.0, if self.eq_on { pal().ink } else { pal().dim });
            ptext(&p, Pos2::new(cx, rect.max.y - 8.0), Align::Center, LABELS[i], 1.0, pal().ink2);
        }
        ui.add_space(4.0);
        para(ui, "Drag a slider; double-click resets it. Range is +/- 12 dB.", pal().dim);
        if changed {
            self.apply_eq();
            self.eq_remember();
            self.dirty = true;
        }
    }

    /// Winamp skins from the Skin Museum: search, a grid of screenshots, click one to wear it.
    pub(crate) fn winamp_view(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            title_line(ui, "WINAMP SKINS", 3.0, pal().ink);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if retro_btn(ui, "CLOSE", false).tip("Back to the library").clicked() {
                    acts.push(Action::WaOpen);
                }
            });
        });
        para(
            ui,
            "Classic skins from the Winamp Skin Museum (skins.webamp.org). Click one to wear its colours; your own skins are back in the TIDALITE menu.",
            pal().ink2,
        );
        if let Some((_, name)) = &self.wa_worn {
            para(ui, &format!("WEARING: {}", name.to_uppercase()), pal().ink);
        }
        ui.horizontal(|ui| {
            let go_w = 54.0;
            let w = (ui.available_width() - go_w - 70.0 - 3.0 * ui.spacing().item_spacing.x - 8.0).max(80.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, BTN_H), Sense::hover());
            let o = crate::views::field(
                ui,
                &mut self.ed,
                crate::tools::F_WINAMP,
                &mut self.wa_q,
                rect,
                "Search skins (zelda, metal, anime...)",
                false,
            );
            if o.enter || retro_btn_w(ui, "GO", go_w, false).clicked() {
                acts.push(Action::WaSearch);
            }
            if retro_btn_w(ui, "BEST", 70.0, false).tip("The museum's favourites").clicked() {
                self.wa_q.clear();
                acts.push(Action::WaSearch);
            }
        });
        ui.add_space(8.0);
        let list = self.wa_list.clone();
        let worn = self.wa_worn.as_ref().map(|w| w.0.clone());
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // cards: the screenshot (main window, equalizer and playlist) and the name
            let (cw, ch) = (150.0, 150.0 * 348.0 / 275.0);
            let gap = 12.0;
            let per_row = ((ui.available_width() + gap) / (cw + gap)).floor().max(1.0) as usize;
            for row in list.chunks(per_row) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for s in row {
                        let (cell, r) = ui.allocate_exact_size(Vec2::new(cw, ch + 22.0), Sense::click());
                        let r = r.on_hover_cursor(egui::CursorIcon::PointingHand).tip(format!("{}  (click to wear it)", s.name));
                        let shot = Rect::from_min_size(cell.min, Vec2::new(cw, ch));
                        match self.images.get(&s.shot) {
                            Some(id) => {
                                egui::Image::new(egui::load::SizedTexture::new(id, shot.size())).paint_at(ui, shot);
                            }
                            None => {
                                ui.painter().rect_filled(shot, 0.0, pal().lcd);
                                ui.ctx().request_repaint();
                            }
                        }
                        let on = worn.as_deref() == Some(s.md5.as_str());
                        if r.hovered() || on {
                            ui.painter().rect_stroke(
                                shot.expand(2.0),
                                0.0,
                                Stroke::new(2.0_f32, if on { pal().red } else { pal().ink }),
                            );
                        }
                        let label = crate::font::fit(&s.name, 1.0, cw);
                        crate::font::ptext(
                            ui.painter(),
                            Pos2::new(cell.min.x, cell.max.y - 9.0),
                            Align::Min,
                            &label,
                            1.0,
                            pal().ink2,
                        );
                        if r.clicked() {
                            acts.push(Action::WaApply(s.clone()));
                        }
                    }
                });
                ui.add_space(gap);
            }
            if self.wa_busy {
                para(ui, "LOADING SKINS...", pal().ink2);
            } else if list.is_empty() {
                para(ui, "No skins found. Try another word, or BEST.", pal().ink2);
            } else if !self.wa_end && retro_btn(ui, "MORE SKINS", false).clicked() {
                acts.push(Action::WaMore);
            }
        });
    }

    pub(crate) fn cache_view(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
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

    /// The TIDALITE menu, next to HOME and LIBRARY: skins (and Winamp skins), preferences, help, storage, the log.
    fn app_menu(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        let pid = ui.make_persistent_id("app_menu");
        let open = ui.memory(|m| m.is_popup_open(pid));
        let r =
            ibtn(ui, &MARK, "TIDALITE", open).tip("Skins (and Winamp skins), preferences, help and tours, storage and the log");
        tour::mark("APPMENU", r.rect);
        if r.clicked() {
            ui.memory_mut(|m| m.toggle_popup(pid));
        }
        egui::popup::popup_below_widget(ui, pid, &r, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
            if self.app_menu_items(ui, acts) {
                ui.memory_mut(|m| m.close_popup());
            }
        });
    }

    /// What the TIDALITE menu holds (also opened by a Winamp skin's own options button). True once something
    /// was picked, so the menu can close.
    pub(crate) fn app_menu_items(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) -> bool {
        ui.set_min_width(240.0);
        let before = acts.len();
        let mut done = false;
        if menu_item(ui, "Preferences") {
            acts.push(Action::TogglePrefs);
        }
        if menu_item(ui, "Help and tours  (F1)") {
            acts.push(Action::ToggleHelp);
        }
        if menu_item(ui, if self.show_cache { "Where tracks are stored  (close)" } else { "Where tracks are stored" }) {
            acts.push(Action::ToggleCacheView);
        }
        if menu_item(ui, if self.show_log { "Log  (close)" } else { "Log" }) {
            self.show_log = !self.show_log;
            if self.show_log {
                self.show_eq = false;
                self.show_cache = false;
            }
            done = true;
        }
        if menu_item(ui, if self.show_winamp { "Winamp skins  (close)" } else { "Winamp skins: browse and try on" }) {
            acts.push(Action::WaOpen);
        }
        ui.add_space(4.0);
        para(ui, "SKIN", pal().ink2);
        let worn = self.wa_worn.as_ref().map(|w| w.1.clone());
        skin_menu(ui, acts, worn.as_deref());
        done || acts.len() != before
    }

    pub(crate) fn library_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        window_deco(ui, ui.max_rect(), "LIBRARY");
        logo(ui);
        ui.add_space(4.0);

        // which list: Tidal / my files / YouTube / tunes / diary
        let tabs_top = ui.cursor().min.y;
        self.section_bar(ui, acts);
        let lr = ui.min_rect();
        tour::mark("TABS", Rect::from_min_max(Pos2::new(lr.min.x, tabs_top), Pos2::new(lr.max.x, ui.cursor().min.y)));

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
                self.app_menu(ui, acts);
            });
        } else {
            ui.horizontal(|ui| {
                if ibtn(ui, &IC_HOME, "HOME", false).tip("Tidal home").clicked() {
                    acts.push(Action::Home);
                }
                if ibtn(ui, &WIN_LIBRARY, "LIBRARY", false).tip("Your tracks, lists, albums and artists").clicked() {
                    acts.push(Action::Library);
                }
                self.app_menu(ui, acts);
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

        // library tabs
        let is_lib = self.sec == Sec::Tidal && self.page.as_ref().map(|p| p.library).unwrap_or(false);
        if is_lib && !self.show_log && !self.show_cache && !self.show_winamp {
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
        // how fast a record on the page turns: the playback speed while music plays, else still
        let sounding = if self.cur.is_some() && !self.paused && !self.stopped { self.speed as f32 / 100.0 } else { 0.0 };
        let (show_log, show_cache, show_winamp) = (self.show_log, self.show_cache, self.show_winamp);
        let sec = self.sec;
        let sec_i = sec as u8;
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(area), |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
            if show_log {
                self.log_view(ui, acts);
                return;
            }
            if show_cache {
                self.cache_view(ui, acts);
                return;
            }
            if show_winamp {
                self.winamp_view(ui, acts);
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
                            page_view(ui, &mut self.images, pg, playing_id, sounding, tab, &self.liked, acts);
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
}
