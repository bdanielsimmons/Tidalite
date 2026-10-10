//! The album view: big tilting cover, the soft visualizer behind it, synced lyrics and its own controls.

use super::*;

impl App {
    pub(crate) fn art_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        let full = ui.max_rect();
        let p = ui.painter().clone();
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
            let (vb, vp, vw) = self.viz_data(1);
            viz_draw(&p, r, &vb, &vp, &vw, self.viz[1].mode, self.viz[1].w, true, self.spec_op, self.viz_tint(self.viz[1].color));
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

        // click the cover to put it on a record (and back)
        let cover_hit = ui.interact(Rect::from_center_size(center, Vec2::splat(a)), ui.id().with("cover_vinyl"), Sense::click());
        if cover_hit.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            acts.push(Action::ToggleVinyl);
        }
        if VINYL.load(Ordering::Relaxed) && tex.is_some() {
            // the record: a soft round shadow, then the disc turning while the music plays
            let off = Vec2::new(-tilt.x * 22.0, -tilt.y * 12.0 + 16.0);
            p.circle_filled(center + off, a * 0.5 + 4.0, Color32::from_black_alpha((60.0 * self.art_op) as u8));
            let spin = vinyl_angle(ui, active && !self.paused);
            // follows the cover opacity setting, and never fully solid, so the visualizer shows through
            paint_vinyl(&p, Rect::from_center_size(center, Vec2::splat(a)), tex, spin, self.art_op.min(0.8));
        } else {
            // shadow, frame, cover
            let off = Vec2::new(-tilt.x * 22.0, -tilt.y * 12.0 + 20.0);
            let shadow: Vec<Pos2> = corners(h + 6.0).iter().map(|c| *c + off).collect();
            let see = self.art_op < 0.99;
            p.add(egui::Shape::convex_polygon(shadow, Color32::from_black_alpha(if see { 50 } else { 120 }), Stroke::NONE));
            // when the cover is see-through the frame is only an outline, so the visualizer behind it stays visible
            p.add(egui::Shape::convex_polygon(
                corners(h + 9.0).to_vec(),
                if see { Color32::TRANSPARENT } else { Color32::BLACK },
                Stroke::new(2.0_f32, pal().trim),
            ));
            let cs = corners(h);
            match tex {
                Some(id) => {
                    let mut mesh = egui::Mesh::with_texture(id);
                    let uvs = [Pos2::new(0.0, 0.0), Pos2::new(1.0, 0.0), Pos2::new(1.0, 1.0), Pos2::new(0.0, 1.0)];
                    for (c, uv) in cs.iter().zip(uvs.iter()) {
                        mesh.vertices.push(egui::epaint::Vertex {
                            pos: *c,
                            uv: *uv,
                            color: Color32::from_white_alpha((self.art_op * 255.0) as u8),
                        });
                    }
                    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
                    p.add(egui::Shape::mesh(mesh));
                }
                None => {
                    p.add(egui::Shape::convex_polygon(cs.to_vec(), pal().ink2, Stroke::NONE));
                    // no cover (yet): the record, turning while it loads
                    let loading = !url.is_empty() && !self.images.failed(&url);
                    paint_record(
                        &p,
                        Rect::from_center_size(center, Vec2::splat(a)),
                        loading.then(|| ui.input(|i| i.time) as f32),
                    );
                    if loading {
                        ui.ctx().request_repaint();
                    }
                }
            }
        }

        // ---- caption
        if let Some(tr) = &track {
            let cx = art_zone.center().x;
            let y0 = art_zone.max.y - caption_h + 14.0;
            let w = art_zone.width() - 60.0;
            let sub = format!("{} - {}", tr.artists_text(), tr.album);
            // a soft dark plate sized to the words, so they stay readable over the visualizer
            let tw = text_w(&tr.title, 3.0).max(text_w(&sub, 2.0)).min(w);
            let plate =
                Rect::from_min_max(Pos2::new(cx - tw * 0.5 - 18.0, y0 - 20.0), Pos2::new(cx + tw * 0.5 + 18.0, y0 + 46.0));
            p.rect_filled(plate, Rounding::same(if style() != 0 { 10.0 } else { 0.0 }), Color32::from_black_alpha(170));
            ptext_fit(&p, Pos2::new(cx, y0), Align::Center, &tr.title, 3.0, w, pal().bar_txt);
            ptext_fit(&p, Pos2::new(cx, y0 + 30.0), Align::Center, &sub, 2.0, w, pal().bar_txt.gamma_multiply(0.8));
        } else {
            ptext(&p, art_zone.center(), Align::Center, "Nothing playing", 3.0, pal().trim);
        }

        // ---- lyrics
        if lyrics_on {
            let ly = Rect::from_min_max(Pos2::new(area.min.x + area.width() * 0.5 + 10.0, area.min.y), area.max);
            p.rect_filled(ly, Rounding::same(0.0), Color32::from_black_alpha(150));
            p.rect_stroke(ly, Rounding::same(0.0), Stroke::new(2.0_f32, pal().trim));
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
        tour::mark("ARTBTNS", row);
        let (gray, lyr, fs) = (self.art_gray, self.show_lyrics, self.fullscreen);
        let cur_t = self.cur_track();
        let liked_now = cur_t.as_ref().map(|t| self.liked.contains(&t.id)).unwrap_or(false);
        let playing = active && !self.paused;
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(row), |ui| {
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
                    let step = |ui: &mut egui::Ui, name: &str, v: &mut f32, d: f32, lo: f32, hi: f32| {
                        if menu_item(ui, &format!("{}  {}%   (+)", name, (*v * 100.0).round() as i32)) {
                            *v = (*v + d).min(hi);
                        }
                        if menu_item(ui, &format!("{}  {}%   (-)", name, (*v * 100.0).round() as i32)) {
                            *v = (*v - d).max(lo);
                        }
                    };
                    step(ui, "OPACITY", &mut self.spec_op, 0.05, 0.05, 0.8);
                    step(ui, "COVER", &mut self.art_op, 0.05, 0.5, 1.0);
                    step(ui, "HEIGHT", &mut self.spec_h, 0.08, 0.15, 0.9);
                    step(ui, "WIDTH", &mut self.spec_w, 0.1, 0.2, 1.0);
                    self.viz_menu(ui, 1);
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
                    ui.add_space(14.0);
                    // volume: drag, click or scroll over it
                    let (sr, vr) = ui.allocate_exact_size(Vec2::new(120.0, BTN_H), Sense::click_and_drag());
                    let vr = vr
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .tip(format!("Volume {}%  (drag, or scroll over it)", (self.volume * 100.0).round()));
                    let g = Rect::from_center_size(sr.center(), Vec2::new(sr.width(), 8.0));
                    let knob_w = 10.0;
                    let kx = g.min.x + (g.width() - knob_w) * self.volume;
                    inset(ui.painter(), g, pal().groove);
                    fill_rect(
                        ui.painter(),
                        Rect::from_min_max(g.min + Vec2::new(1.5, 1.5), Pos2::new(kx + knob_w / 2.0, g.max.y - 1.5)),
                        pal().ink2,
                    );
                    raised(ui.painter(), Rect::from_min_size(Pos2::new(kx, sr.center().y - 8.0), Vec2::new(knob_w, 16.0)), false);
                    if vr.dragged() || vr.clicked() {
                        if let Some(pp) = vr.interact_pointer_pos() {
                            acts.push(Action::Volume(((pp.x - g.min.x - knob_w / 2.0) / (g.width() - knob_w)).clamp(0.0, 1.0)));
                        }
                    }
                    let wheel = ui.input(|i| i.smooth_scroll_delta.y);
                    if vr.hovered() && wheel != 0.0 {
                        acts.push(Action::Volume((self.volume + wheel * 0.002).clamp(0.0, 1.0)));
                    }
                    let spk: &[&str] = if self.volume <= 0.0 {
                        &IC_SPK0
                    } else if self.volume < 0.5 {
                        &IC_SPK1
                    } else {
                        &IC_SPK2
                    };
                    if icon_btn(ui, spk, self.volume <= 0.0, ink)
                        .tip(if self.volume <= 0.0 { "Unmute" } else { "Mute" })
                        .clicked()
                    {
                        if self.volume > 0.0 {
                            self.vol_before = self.volume;
                            acts.push(Action::Volume(0.0));
                        } else {
                            acts.push(Action::Volume(self.vol_before.max(0.3)));
                        }
                    }
                });
            });
        });
        ptext(
            &p,
            Pos2::new(full.max.x - 30.0, full.max.y - 14.0),
            Align::Max,
            "Click the cover: record    Esc close    F fullscreen    G b&w    L lyrics    H like    Space play/pause",
            1.0,
            pal().dim,
        );
    }
}
