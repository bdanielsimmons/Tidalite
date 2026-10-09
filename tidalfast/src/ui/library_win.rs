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

    pub(crate) fn eq_view(&mut self, ui: &mut egui::Ui) {
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

    pub(crate) fn library_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
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
            if icon_btn_w(ui, &IC_HELP, self.show_help, ink, 38.0).tip("Help, tips and about  (F1)").clicked() {
                acts.push(Action::ToggleHelp);
            }
            if retro_btn(ui, "PREFS", self.show_prefs).tip("Preferences: updates and keyboard shortcuts").clicked() {
                acts.push(Action::TogglePrefs);
            }
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
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(area), |ui| {
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
}
