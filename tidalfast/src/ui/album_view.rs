//! The album view: big tilting cover, the soft visualizer behind it, synced lyrics and its own controls.

use super::*;
use crate::extras::Knob;
use crate::tools::F_SPEED;
use crate::views::label;

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
        // in practice mode the loop and speed tools get their own band along the top; everything else moves down
        let prac = PRACTICE && self.practice;
        let strip_h = BTN_H + 14.0;
        let top = 28.0 + if prac { strip_h + 10.0 } else { 0.0 };
        let area = Rect::from_min_max(full.min + Vec2::new(28.0, top), Pos2::new(full.max.x - 28.0, full.max.y - bar_h - 12.0));
        let lyrics_on = self.show_lyrics && area.width() > 640.0;
        let art_zone = if lyrics_on {
            Rect::from_min_max(area.min, Pos2::new(area.min.x + area.width() * 0.5 - 10.0, area.max.y))
        } else {
            area
        };
        let cap_k = [0.8, 1.0, 1.35][self.cap_size.min(2) as usize];
        let caption_h = 64.0 * cap_k;
        let a = (art_zone.width().min(art_zone.height() - caption_h) * 0.82).max(120.0);
        let center = Pos2::new(art_zone.center().x, art_zone.min.y + (art_zone.height() - caption_h) / 2.0);

        // ---- background: the cover's average colour as a soft gradient (never a stretched picture)
        let url = track.as_ref().map(|t| cover_url(&t.cover, 640)).unwrap_or_default();
        // the cover on show stays up while the next one loads, then the new one fades in
        let (tex, prev, fade) = self.images.held("album", &url, false, ui.input(|i| i.time));
        if fade < 1.0 {
            ui.ctx().request_repaint();
        }
        let avg = self.images.avg.get(&url).copied();
        // ease from the old cover's colours to the new one's instead of jumping
        if let Some(tg) = avg {
            let k = 1.0 - (-dt * 4.0).exp();
            let mix = |a: Color32, b: Color32| {
                let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k).round() as u8;
                Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
            };
            let base = pal().app_bg;
            let c = self.bg_now.unwrap_or([base, base]);
            let now = [mix(c[0], tg[0]), mix(c[1], tg[1])];
            if now != tg {
                ui.ctx().request_repaint();
            }
            self.bg_now = Some(now);
        }
        if let Some([top, bot]) = self.bg_now {
            // dark and deep, whatever the cover: keep its hue, cap how bright it gets, fall to near-black at the
            // bottom, and darken the edges so the cover and the visualizer stand out
            let deep = |c: Color32, cap: f32| {
                let mut h = egui::ecolor::Hsva::from(c);
                h.s = (h.s * 1.15).min(0.8);
                h.v = (h.v * 0.45).min(cap);
                Color32::from(h)
            };
            let (tl, tr, bl, br) = (deep(top, 0.26), deep(top, 0.18), deep(bot, 0.08), deep(bot, 0.05));
            let mut mesh = egui::Mesh::default();
            let uv = egui::epaint::WHITE_UV;
            for (pos, color) in
                [(full.left_top(), tl), (full.right_top(), tr), (full.right_bottom(), br), (full.left_bottom(), bl)]
            {
                mesh.vertices.push(egui::epaint::Vertex { pos, uv, color });
            }
            mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
            p.add(egui::Shape::mesh(mesh));
            // vignette: clear in the middle, shading to dark at the edges
            let inner = full.shrink(full.width().min(full.height()) * 0.22);
            let (o, i) = (Color32::from_black_alpha(150), Color32::TRANSPARENT);
            let mut v = egui::Mesh::default();
            for (pos, color) in [
                (full.left_top(), o),
                (full.right_top(), o),
                (full.right_bottom(), o),
                (full.left_bottom(), o),
                (inner.left_top(), i),
                (inner.right_top(), i),
                (inner.right_bottom(), i),
                (inner.left_bottom(), i),
            ] {
                v.vertices.push(egui::epaint::Vertex { pos, uv, color });
            }
            for k in 0..4u32 {
                let n = (k + 1) % 4;
                v.indices.extend_from_slice(&[k, n, 4 + n, k, 4 + n, 4 + k]);
            }
            p.add(egui::Shape::mesh(v));
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
            // SOFT EDGES: it fades out at both ends, so it finishes cleanly at any width
            let (vb, vp, vw) = self.viz_data(1);
            let soft = self.spec_frame == 1;
            let r = if soft { Rect::from_min_max(r.min, r.max - Vec2::new(0.0, 12.0)) } else { r };
            crate::viz::set_edge_fade(soft);
            viz_draw(&p, r, &vb, &vp, &vw, self.viz[1].mode, self.viz[1].w, true, self.spec_op, self.viz_tint(self.viz[1].color));
            crate::viz::set_edge_fade(false);
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
        // a rigid card in 3D, gently pushed by the mouse: the corner nearest it sinks back, the far one comes
        // forward. Small angles and a far-off eye, so it tilts without warping; it also drifts a touch away
        let (ky, kx) = (-tilt.x * 0.28, -tilt.y * 0.28);
        let (cy, cx) = (ky.cos(), kx.cos());
        let dist = a * 4.0;
        let push = Vec2::new(-tilt.x, -tilt.y) * 6.0;
        let xf = move |x: f32, y: f32| -> Pos2 {
            let z = x * ky.sin() + y * kx.sin();
            let sc = dist / (dist - z);
            Pos2::new(center.x + push.x + x * cy * sc, center.y + push.y + y * cx * sc)
        };
        let corners = |h: f32| [xf(-h, -h), xf(h, -h), xf(h, h), xf(-h, h)];
        let h = a / 2.0;

        {
            // click the cover to put it on a record (and back)
            let cover_hit =
                ui.interact(Rect::from_center_size(center, Vec2::splat(a)), ui.id().with("cover_vinyl"), Sense::click());
            let cover_hit = cover_hit.on_hover_cursor(egui::CursorIcon::PointingHand);
            if cover_hit.clicked() {
                acts.push(Action::ToggleVinyl);
            }
            // right-click: the cover's border
            cover_hit.context_menu(|ui| {
                for (i, name) in ["NONE", "SUBTLE", "SKIN COLOUR"].iter().enumerate() {
                    let mark = if self.art_border as usize == i { "> " } else { "   " };
                    if menu_item(ui, &format!("{}BORDER: {}", mark, name)) {
                        self.art_border = i as u8;
                        self.dirty = true;
                        ui.close_menu();
                    }
                }
            });
            if VINYL.load(Ordering::Relaxed) && tex.is_some() {
                // the record: a soft round shadow, then the disc turning while the music plays
                let off = Vec2::new(-tilt.x * 22.0, -tilt.y * 12.0 + 16.0);
                p.circle_filled(center + off, a * 0.5 + 4.0, Color32::from_black_alpha((60.0 * self.art_op) as u8));
                let spin = vinyl_angle(ui, if active && !self.paused { self.speed as f32 / 100.0 } else { 0.0 });
                // follows the cover opacity setting, and never fully solid, so the visualizer shows through
                paint_vinyl(&p, Rect::from_center_size(center, Vec2::splat(a)), tex, spin, self.art_op.min(0.8));
            } else {
                // a soft shadow in a few faint layers (lighter when the cover is see-through), no heavy frame
                let off = Vec2::new(-tilt.x * 14.0, -tilt.y * 8.0 + 14.0);
                let see = self.art_op < 0.99;
                for (grow, al) in [(16.0, 14u8), (10.0, 22), (5.0, 34)] {
                    let sh: Vec<Pos2> = corners(h + grow).iter().map(|c| *c + off).collect();
                    p.add(egui::Shape::convex_polygon(
                        sh,
                        Color32::from_black_alpha(if see { al / 2 } else { al }),
                        Stroke::NONE,
                    ));
                }
                let cs = corners(h);
                match tex {
                    Some(id) => {
                        // the old cover underneath, the new one fading in on top
                        if let Some(old) = prev {
                            cover_grid(&p, &xf, h, old, self.art_op);
                        }
                        if prev.is_none() && fade < 1.0 {
                            p.add(egui::Shape::convex_polygon(
                                cs.to_vec(),
                                Color32::from_black_alpha((70.0 * (1.0 - fade)) as u8),
                                Stroke::NONE,
                            ));
                        }
                        cover_grid(&p, &xf, h, id, self.art_op * fade);
                        // a faint edge, just enough to finish it
                        // the border sits right on the cover's edge
                        let edge = match self.art_border {
                            1 => Some(Stroke::new(1.0_f32, Color32::from_white_alpha(30))),
                            2 => Some(Stroke::new(2.0_f32, pal().trim.gamma_multiply(self.art_op))),
                            _ => None,
                        };
                        if let Some(s) = edge {
                            p.add(egui::Shape::closed_line(cs.to_vec(), s));
                        }
                    }
                    None => {
                        // no cover yet: a quiet dark slot it will fade into; the record only if it is slow to come
                        // (or there is none at all)
                        p.add(egui::Shape::convex_polygon(cs.to_vec(), Color32::from_black_alpha(70), Stroke::NONE));
                        let loading = !url.is_empty() && !self.images.failed(&url);
                        let now = ui.input(|i| i.time);
                        let since_id = egui::Id::new(("cover_wait", url.as_str()));
                        let since: f64 = ui.ctx().data_mut(|d| *d.get_temp_mut_or_insert_with(since_id, || now));
                        let slow = now - since > 1.5;
                        if !loading || slow {
                            let spin = (loading && slow).then(|| now as f32);
                            paint_record(&p, Rect::from_center_size(center, Vec2::splat(a)), spin);
                        }
                        if loading {
                            ui.ctx().request_repaint();
                        }
                    }
                }
            }
        }

        // ---- caption
        if let Some(tr) = &track {
            let cx = art_zone.center().x;
            let k = cap_k;
            let y0 = art_zone.max.y - caption_h + 14.0 * k;
            let w = art_zone.width() - 60.0;
            let sub = format!("{} - {}", tr.artists_text(), tr.album);
            // soft white on a near-black plate (or soft black on a near-white one): the most restful contrast,
            // whatever the skin, sized to the words so they stay readable over the visualizer
            let (ink, plate_col) = if self.cap_dark {
                (Color32::from_rgb(24, 24, 24), Color32::from_rgba_unmultiplied(238, 238, 234, 235))
            } else {
                (Color32::from_rgb(238, 238, 234), Color32::from_rgba_unmultiplied(16, 16, 16, 225))
            };
            let tw = text_w(&tr.title, 3.0 * k).max(text_w(&sub, 2.0 * k)).min(w);
            let plate = Rect::from_min_max(
                Pos2::new(cx - tw * 0.5 - 18.0, y0 - 20.0 * k),
                Pos2::new(cx + tw * 0.5 + 18.0, y0 + 46.0 * k),
            );
            p.rect_filled(plate, Rounding::same(if style() != 0 { 10.0 } else { 0.0 }), plate_col);
            ptext_fit(&p, Pos2::new(cx, y0), Align::Center, &tr.title, 3.0 * k, w, ink);
            ptext_fit(&p, Pos2::new(cx, y0 + 30.0 * k), Align::Center, &sub, 2.0 * k, w, ink.gamma_multiply(0.85));
            // right-click: text size and colour
            ui.interact(plate, ui.id().with("caption"), Sense::click()).context_menu(|ui| {
                for (i, name) in ["SMALL", "MEDIUM", "LARGE"].iter().enumerate() {
                    let mark = if self.cap_size as usize == i { "> " } else { "   " };
                    if menu_item(ui, &format!("{}TEXT: {}", mark, name)) {
                        self.cap_size = i as u8;
                        self.dirty = true;
                        ui.close_menu();
                    }
                }
                for (dark, name) in [(false, "SOFT WHITE"), (true, "SOFT BLACK")] {
                    let mark = if self.cap_dark == dark { "> " } else { "   " };
                    if menu_item(ui, &format!("{}COLOUR: {}", mark, name)) {
                        self.cap_dark = dark;
                        self.dirty = true;
                        ui.close_menu();
                    }
                }
            });
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

        // ---- the queue, as a panel on the right (the three-lines button)
        if self.art_queue {
            let w = (full.width() * 0.4).min(380.0);
            let panel =
                Rect::from_min_max(Pos2::new(full.max.x - 28.0 - w, area.min.y), Pos2::new(full.max.x - 28.0, area.max.y));
            let queue = self.queue.clone();
            let cur = self.cur;
            let mut close = false;
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(panel), |ui| {
                egui::Frame::none()
                    .fill(pal().app_bg.gamma_multiply(0.92))
                    .stroke(Stroke::new(1.5_f32, pal().edge))
                    .rounding(if style() != 0 { 8.0 } else { 0.0 })
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.set_min_size(panel.size() - Vec2::splat(20.0));
                        ui.horizontal(|ui| {
                            title_line(ui, &format!("QUEUE  {}", queue.len()), 2.5, pal().ink);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if icon_btn(ui, &IC_X, false, pal().ink).tip("Close the queue").clicked() {
                                    close = true;
                                }
                            });
                        });
                        if queue.is_empty() {
                            para(ui, "Nothing queued. Play a song or right-click one: Add to queue.", pal().ink2);
                        }
                        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                            for (i, t) in queue.iter().enumerate() {
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
                                    false,
                                    cache::has(t.id),
                                );
                                if r.is_some_and(|r| r.clicked()) {
                                    acts.push(Action::PlayIndex(i));
                                }
                            }
                        });
                    });
            });
            if close {
                self.art_queue = false;
            }
        }

        // ---- buttons
        let row = Rect::from_min_size(Pos2::new(full.min.x + 28.0, full.max.y - 52.0), Vec2::new(full.width() - 56.0, BTN_H));
        tour::mark("ARTBTNS", row);
        if prac {
            // practice mode: the loop tools on the left, the speed on the right, in the band along the top
            let strip =
                Rect::from_min_size(Pos2::new(full.min.x + 28.0, full.min.y + 12.0), Vec2::new(full.width() - 56.0, strip_h));
            p.rect_filled(strip, Rounding::same(if style() != 0 { 8.0 } else { 0.0 }), Color32::from_black_alpha(120));
            tour::mark("ARTPRACTICE", strip);
            let both = self.loop_a.is_some() && self.loop_b.is_some();
            let ab = format!(
                "A {}   B {}",
                self.loop_a.map(fmt_t).unwrap_or_else(|| "-:--.-".to_string()),
                self.loop_b.map(fmt_t).unwrap_or_else(|| "-:--.-".to_string())
            );
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(strip.shrink2(Vec2::new(10.0, 7.0))), |ui| {
                ui.horizontal(|ui| {
                    if retro_btn_w(ui, "SET A", 64.0, false).tip("Loop start here  ([ key)").clicked() && active {
                        acts.push(Action::SetAAt(pos));
                    }
                    if retro_btn_w(ui, "SET B", 64.0, false).tip("Loop end here  (] key)").clicked() && active {
                        acts.push(Action::SetBAt(pos));
                    }
                    if icon_btn(ui, &IC_REP, self.loop_on && both, pal().ink).tip("Loop on / off  (\\ key)").clicked() {
                        acts.push(Action::LoopToggle);
                    }
                    if icon_btn(ui, &IC_X, false, pal().ink).tip("Clear the loop").clicked() {
                        acts.push(Action::LoopClear);
                    }
                    lcd_box(ui, &ab, 200.0, pal().ink);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        self.num_step(
                            ui,
                            acts,
                            F_SPEED,
                            64.0,
                            Knob::Speed,
                            format!("{}%", self.speed),
                            "Slower / faster (Down / Up keys) - or type a percent",
                        );
                        for pct in [125u32, 100, 85, 70, 50] {
                            if retro_btn_w(ui, &format!("{}", pct), 44.0, self.speed == pct).clicked() {
                                acts.push(Action::Speed(pct));
                            }
                        }
                        label(ui, "SPEED", 56.0);
                    });
                });
            });
        }
        let (lyr, fs) = (self.show_lyrics, self.fullscreen);
        let cur_t = self.cur_track();
        let liked_now = cur_t.as_ref().map(|t| self.liked.contains(&t.id)).unwrap_or(false);
        let playing = active && !self.paused;
        let ink = pal().ink;
        // three groups: how it looks on the left, playing in the middle, the rest on the right
        let mid = Rect::from_center_size(row.center(), Vec2::new(6.0 * 44.0 + 5.0 * 8.0, BTN_H));
        let left = Rect::from_min_max(row.min, Pos2::new(mid.min.x - 24.0, row.max.y));
        let right = Rect::from_min_max(Pos2::new(mid.max.x + 24.0, row.min.y), row.max);
        let roomy = row.width() >= 980.0;

        // ---- how it looks
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(left), |ui| {
            ui.horizontal(|ui| {
                if icon_btn(ui, &IC_MIC, lyr, ink).tip("Lyrics  (L)").clicked() {
                    acts.push(Action::ToggleLyrics);
                }
                let sp = icon_btn(ui, &IC_SPEC, self.show_spec, ink)
                    .tip("Visualizer behind the cover  (right-click: its settings, with a preview)");
                if sp.clicked() {
                    acts.push(Action::ToggleSpec);
                }
                if sp.secondary_clicked() {
                    self.viz_panel = Some(1);
                }
                if icon_btn(ui, if fs { &IC_WIN } else { &IC_FULL }, false, ink)
                    .tip(if fs { "Leave fullscreen  (F)" } else { "Fullscreen  (F)" })
                    .clicked()
                {
                    acts.push(Action::ToggleFullscreen);
                }
            });
        });

        // ---- playing, in the middle
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(mid), |ui| {
            ui.horizontal(|ui| {
                if icon_btn(ui, &IC_SHUF, self.shuffle, ink)
                    .tip(if self.shuffle { "Shuffle is on - click to turn off" } else { "Shuffle the queue" })
                    .clicked()
                {
                    acts.push(Action::Shuffle);
                }
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
                let (rep_icon, rep_tip) = match self.repeat {
                    Repeat::Off => (&IC_REP, "Repeat: off  (click for all)"),
                    Repeat::All => (&IC_REP, "Repeat all  (click for one track)"),
                    Repeat::One => (&IC_REP1, "Repeat this track  (click to turn off)"),
                };
                if icon_btn(ui, rep_icon, self.repeat != Repeat::Off, ink).tip(rep_tip).clicked() {
                    acts.push(Action::Repeat);
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
            });
        });

        // ---- the rest: speed, practice, queue, then sound and CLOSE at the edge
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(right), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if retro_btn(ui, "CLOSE", false).clicked() {
                    acts.push(Action::ToggleArt);
                }
                ui.add_space(10.0);
                if roomy {
                    // volume: drag, click or scroll over it
                    let (sr, vr) = ui.allocate_exact_size(Vec2::new(100.0, BTN_H), Sense::click_and_drag());
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
                }
                let spk: &[&str] = if self.volume <= 0.0 {
                    &IC_SPK0
                } else if self.volume < 0.5 {
                    &IC_SPK1
                } else {
                    &IC_SPK2
                };
                if icon_btn(ui, spk, self.volume <= 0.0, ink).tip(if self.volume <= 0.0 { "Unmute" } else { "Mute" }).clicked() {
                    if self.volume > 0.0 {
                        self.vol_before = self.volume;
                        acts.push(Action::Volume(0.0));
                    } else {
                        acts.push(Action::Volume(self.vol_before.max(0.3)));
                    }
                }
                let ob =
                    icon_btn(ui, &IC_OUT, self.out_device.is_some(), ink).tip("Sound output: speakers, headphones, Bluetooth...");
                self.output_popup(ui, &ob, acts);
                ui.add_space(14.0);
                if icon_btn(ui, &IC_MENU, self.art_queue, ink).tip("The queue: what plays next").clicked() {
                    self.art_queue = !self.art_queue;
                }
                if PRACTICE
                    && retro_btn(ui, "PRACTICE", self.practice).tip("The loop and speed tools along the top  (P)").clicked()
                {
                    acts.push(Action::TogglePractice);
                }
                let sb = retro_btn(ui, &crate::tools::speed_label(self.speed), self.speed != 100)
                    .tip("Speed (pitch stays): click for the next one, right-click to pick or type any  (Up / Down keys)");
                if sb.clicked() {
                    acts.push(Action::Speed(crate::tools::next_speed(self.speed)));
                }
                sb.context_menu(|ui| self.speed_menu(ui, acts));
            });
        });
        ptext(
            &p,
            Pos2::new(full.max.x - 30.0, full.max.y - 14.0),
            Align::Max,
            "Click the cover: record    Right-click it: border    Esc close    F fullscreen    G b&w    L lyrics    H like    Space play/pause",
            1.0,
            pal().dim,
        );
    }
}

/// The cover as a fine grid of points, each placed through the same 3D tilt, so the picture stays a flat,
/// stiff card at any angle (stretched over just two triangles it would bend along the diagonal).
fn cover_grid(p: &egui::Painter, xf: &impl Fn(f32, f32) -> Pos2, h: f32, id: egui::TextureId, alpha: f32) {
    const N: u32 = 12;
    const E: f32 = 0.006;
    let color = Color32::from_white_alpha((alpha.clamp(0.0, 1.0) * 255.0) as u8);
    let mut mesh = egui::Mesh::with_texture(id);
    for j in 0..=N {
        for i in 0..=N {
            let (u, v) = (i as f32 / N as f32, j as f32 / N as f32);
            let pos = xf(-h + 2.0 * h * u, -h + 2.0 * h * v);
            let uv = Pos2::new(E + (1.0 - 2.0 * E) * u, E + (1.0 - 2.0 * E) * v);
            mesh.vertices.push(egui::epaint::Vertex { pos, uv, color });
        }
    }
    for j in 0..N {
        for i in 0..N {
            let a = j * (N + 1) + i;
            let b = a + N + 1;
            mesh.indices.extend_from_slice(&[a, a + 1, b + 1, a, b + 1, b]);
        }
    }
    p.add(egui::Shape::mesh(mesh));
}
