//! The player window (the skinned deck with transport, seek and volume) and the queue under it.

use super::*;

impl App {
    pub(crate) fn player_window(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
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
            // each chip as wide as its word in the font being worn
            let fit = |s: &str| text_w(s, 1.0) + 14.0;
            let r_pin = place(fit("PIN"));
            let r_mini = place(fit(if self.mini { "FULL" } else { "MINI" }));
            let r_prac = place(if PRACTICE { fit("PRACTICE") } else { 0.0 });
            let r_lis = place(if PRACTICE { fit("LISTEN") } else { 0.0 });
            let r_focus = place(if PRACTICE { fit("FOCUS") } else { 0.0 });
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
            if PRACTICE {
                tour::mark("CHIP", r_prac.union(r_lis));
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
                && chip(ui, r_lis, "LISTEN", !self.practice, "c_lis").tip("Just listening: no loop, no pitch change").clicked()
                && self.practice
            {
                acts.push(Action::TogglePractice);
            }
        }
        // a worn Winamp skin draws the whole player from its own pictures
        if self.wa_art.is_some() {
            self.winamp_main(ui, inner, acts);
            return;
        }
        let painter = ui.painter().clone();
        let p = &painter;

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
        let (vb, vp, vw) = self.viz_data(0);
        viz_draw(
            p,
            sp.shrink2(Vec2::new(3.0, 3.0)),
            &vb,
            &vp,
            &vw,
            self.viz[0].mode,
            self.viz[0].w,
            false,
            1.0,
            self.viz_tint(self.viz[0].color),
        );
        let spr = ui
            .interact(sp, ui.id().with("spectrum"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip("Click: bars / waveform / both.  Right-click: all its settings, with a preview");
        if spr.clicked() {
            self.viz[0].mode = (self.viz[0].mode + 1) % 3;
            self.dirty = true;
        }
        if spr.secondary_clicked() {
            self.viz_panel = Some(0);
        }

        // ---- title + status
        let tb = rc(112.0, 6.0, 182.0, 14.0);
        inset(p, tb, pal().lcd);
        let title_txt = match &track {
            Some(t) => format!("{} - {}  ({})", t.artists_text(), t.title, fmt_time(t.duration)),
            None => format!("Tidalite {} - a retro player for Tidal", VERSION),
        };
        marquee(ui, tb, &title_txt, px_big, pal().ink);
        // right-click the playing song: its album, artist or radio
        if let Some(t) = track.as_ref().filter(|t| self.ext_of(t.id).is_none()) {
            let t = (*t).clone();
            let tr = ui.interact(tb, ui.id().with("title_menu"), Sense::click());
            tr.tip("Right-click: go to the album, the artist or the track radio, or see the credits").context_menu(|ui| {
                goto_items(ui, acts, &t, true);
            });
        }

        let sb = rc(112.0, 22.0, 182.0, 10.0);
        inset(p, sb, pal().lcd);
        let (stxt, scol): (String, Color32) = if !self.status.is_empty() {
            (self.status.clone(), if self.status_err { pal().red } else { pal().ink })
        } else if self.buffering {
            ("BUFFERING...".to_string(), pal().ink2)
        } else if self.loading {
            ("LOADING...".to_string(), pal().ink2)
        } else if let (true, Some(n)) = (active, &next) {
            (format!("NEXT: {} - {}", n.artists_text(), n.title), pal().ink2)
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

        // ---- tempo / key / quality (the stream's kbps and kHz show on hover)
        let info = track.as_ref().and_then(|t| meta::get(t.id)).filter(|_| active).unwrap_or_default();
        let from = if info.tidal { "from Tidal" } else { "an estimate from listening to the track" };
        let stream = |s: String| if active && !s.is_empty() { format!("\nStream: {}", s) } else { String::new() };
        let kb = rc(112.0, 34.0, 50.0, 12.0);
        inset(p, kb, pal().lcd);
        let kb_txt = info.bpm.map_or("--- BPM".to_string(), |b| format!("{} BPM", b));
        ptext_fit(p, kb.center(), Align::Center, &kb_txt, px_sm, kb.width() - 6.0, pal().ink);
        let kb_tip = match info.bpm {
            Some(b) => format!("Tempo: {} BPM, {}", b, from),
            None => "Tempo: worked out the first time a track plays".to_string(),
        };
        ui.interact(kb, ui.id().with("bpm"), Sense::hover())
            .tip(kb_tip + &stream(if self.kbps > 0 { format!("{} kbps", self.kbps) } else { String::new() }));
        let kh = rc(166.0, 34.0, 40.0, 12.0);
        inset(p, kh, pal().lcd);
        let kh_txt = if info.key.is_some() { meta::key_text(&info) } else { "KEY --".to_string() };
        ptext_fit(p, kh.center(), Align::Center, &kh_txt, px_sm, kh.width() - 6.0, pal().ink);
        let kh_tip = match info.key {
            Some(k) => format!("Key: {}, {}\nThe number is its Camelot code, for mixing", meta::key_long(k), from),
            None => "Key: worked out the first time a track plays".to_string(),
        };
        ui.interact(kh, ui.id().with("key"), Sense::hover())
            .tip(kh_tip + &stream(if rate > 0 { format!("{} kHz", rate / 1000) } else { String::new() }));
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
        tour::mark("COVER", cv);
        inset(p, cv, pal().edge);
        if let Some(t) = &track {
            if !t.cover.is_empty() {
                paint_held(ui, &mut self.images, "player", &cover_url(&t.cover, 160), cv.shrink(2.0), false);
            }
        }
        // the cover opens the album view
        let cvr = ui
            .interact(cv, ui.id().with("cover"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip("Album view  (A)");
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
        let vr = ui
            .interact(rc(112.0, 48.0, 138.0, 18.0), ui.id().with("vol"), Sense::click_and_drag())
            .tip(format!("Volume {}%", (self.volume * 100.0).round() as u32));
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
        // speed: click for the next preset, right-click to pick one or type any
        let spd = rc(134.0, 67.0, 40.0, 12.0);
        tour::mark("SPEEDBTN", spd);
        let spr = ui
            .interact(spd, ui.id().with("speed"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip("Speed (pitch stays): click for the next one, right-click to pick or type any  (Up / Down keys)");
        raised_h(p, spd, self.speed != 100 || spr.is_pointer_button_down_on(), spr.hovered());
        ptext_fit(p, spd.center(), Align::Center, &crate::tools::speed_label(self.speed), px_sm, spd.width() - 4.0, pal().ink);
        if spr.clicked() {
            acts.push(Action::Speed(crate::tools::next_speed(self.speed)));
        }
        spr.context_menu(|ui| self.speed_menu(ui, acts));
        // sleep timer: a moon, or the minutes left while it runs
        let slp = rc(178.0, 67.0, 40.0, 12.0);
        let sleeping = self.sleep_at.map(|t| t.saturating_duration_since(Instant::now()).as_secs().div_ceil(60));
        let slr = ui
            .interact(slp, ui.id().with("sleep"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip("Sleep timer: click for 15 / 30 / 45 / 60 / 90 minutes or off (it fades out at the end)");
        raised_h(p, slp, sleeping.is_some() || slr.is_pointer_button_down_on(), slr.hovered());
        match sleeping {
            Some(m) => {
                ptext_fit(p, slp.center(), Align::Center, &format!("{}M", m), px_sm, slp.width() - 4.0, pal().ink);
            }
            None => icon_in(p, slp, &IC_MOON, pal().ink),
        }
        if slr.clicked() {
            acts.push(Action::Sleep);
        }
        // where the sound goes: a speaker button with the list of outputs
        let ob = rc(224.0, 67.0, 26.0, 12.0);
        let obr = ui
            .interact(ob, ui.id().with("outputs"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip("Sound output: speakers, headphones, Bluetooth...");
        raised_h(p, ob, self.out_device.is_some() || obr.is_pointer_button_down_on(), obr.hovered());
        icon_in(p, ob, &IC_OUT, pal().ink);
        self.output_popup(ui, &obr, acts);

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
        let sh = rc(140.0, 103.0, 30.0, 14.0);
        tour::mark("SHUFFLE", sh.union(rc(248.0, 103.0, 30.0, 14.0)));
        let shr = ui
            .interact(sh, ui.id().with("shuf"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip(if self.shuffle { "Shuffle: on" } else { "Shuffle" });
        raised_h(p, sh, self.shuffle || shr.is_pointer_button_down_on(), shr.hovered() && !self.shuffle);
        icon_in(p, sh, &IC_SHUF, if self.shuffle { pal().red } else { pal().ink });
        if shr.clicked() {
            acts.push(Action::Shuffle);
        }
        let rp = rc(176.0, 103.0, 30.0, 14.0);
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
        if !self.mini {
            let fb = rc(212.0, 103.0, 30.0, 14.0);
            let fs = self.fullscreen;
            let fbr = ui.interact(fb, ui.id().with("full"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).tip(
                format!(
                    "{}  ({} / F11)",
                    if fs { "Leave fullscreen" } else { "Fullscreen" },
                    self.key_text(prefs::Cmd::Fullscreen)
                ),
            );
            raised_h(p, fb, fs || fbr.is_pointer_button_down_on(), fbr.hovered() && !fs);
            icon_in(p, fb, if fs { &IC_WIN } else { &IC_FULL }, pal().ink);
            if fbr.clicked() {
                acts.push(Action::ToggleFullscreen);
            }
        }
        // the equalizer, next to fullscreen
        if !self.mini {
            let eb = rc(248.0, 103.0, 30.0, 14.0);
            let er = ui
                .interact(eb, ui.id().with("eq"), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .tip("Equalizer");
            raised_h(p, eb, self.show_eq || er.is_pointer_button_down_on(), er.hovered() && !self.show_eq);
            icon_in(p, eb, &IC_EQ, pal().ink);
            if er.clicked() {
                acts.push(Action::ToggleEq);
            }
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

    pub(crate) fn playlist_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        // a worn Winamp skin draws the queue as its own playlist window
        if self.rtab == 0 && self.wa_art.as_ref().is_some_and(|t| t.sheets.contains_key("pledit")) {
            let inner = ui.max_rect();
            let outer = Rect::from_min_max(inner.min - Vec2::new(14.0, 30.0), inner.max + Vec2::new(14.0, 14.0));
            self.winamp_playlist(ui, outer, acts);
            return;
        }
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

        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(area), |ui| {
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
                                || format!("{} - {}", t.artists_text(), t.title),
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
                                let tidal = self.ext_of(t.id).is_none();
                                r.context_menu(|ui| {
                                    if menu_item(ui, lk) {
                                        acts.push(Action::ToggleLike(t.clone()));
                                        ui.close_menu();
                                    }
                                    goto_items(ui, acts, &t, tidal);
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
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(foot), |ui| {
            ui.horizontal(|ui| {
                if retro_btn(ui, "CLEAR QUEUE", false).clicked() {
                    acts.push(Action::ClearQueue);
                }
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(340.0), bh()), Sense::hover());
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
