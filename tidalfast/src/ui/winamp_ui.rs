//! Winamp skins drawn for real: the main window and the equalizer from the skin's own bitmaps, at Winamp's own
//! positions (sprite and layout map from Webamp, the open-source Winamp remake), scaled in whole screen pixels.
//! Every control works and drives Tidalite: transport, seek, volume, shuffle, repeat, the equalizer.

use super::*;

/// The worn skin's pictures as textures, with its visualizer and equalizer-curve colours.
pub(crate) struct WaTex {
    pub(crate) sheets: HashMap<String, (egui::TextureHandle, Vec2)>,
    pub(crate) vis: Vec<Color32>,
    pub(crate) graph: Vec<Color32>,
    /// the playlist font the skin asks for
    pub(crate) font: Option<String>,
}

impl WaTex {
    pub(crate) fn new(ctx: &egui::Context, art: crate::winamp::Art) -> WaTex {
        let mut sheets = HashMap::new();
        for (name, size, rgba) in art.sheets {
            let img = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
            let tex = ctx.load_texture(format!("winamp_{}", name), img, egui::TextureOptions::NEAREST);
            sheets.insert(name, (tex, Vec2::new(size[0] as f32, size[1] as f32)));
        }
        WaTex { sheets, vis: art.vis, graph: art.graph, font: art.font }
    }

    fn size(&self, sheet: &str) -> Option<Vec2> {
        self.sheets.get(sheet).map(|s| s.1)
    }

    /// One sprite (x, y, w, h in the sheet) into `dst`. False when the skin lacks that sheet.
    fn spr(&self, p: &egui::Painter, sheet: &str, src: (f32, f32, f32, f32), dst: Rect) -> bool {
        let Some((tex, size)) = self.sheets.get(sheet) else { return false };
        // pull the edges in by a hair: sprites sit side by side in a sheet (the digits 0 to 9...), and sampling
        // right on the border would pick up a sliver of the neighbour
        const E: f32 = 0.02;
        let uv = Rect::from_min_max(
            Pos2::new((src.0 + E) / size.x, (src.1 + E) / size.y),
            Pos2::new((src.0 + src.2 - E) / size.x, (src.1 + src.3 - E) / size.y),
        );
        p.image(tex.id(), dst, uv, Color32::WHITE);
        true
    }
}

/// Winamp's 275 x 116 window placed on screen: `o` the top-left corner, `s` screen points per skin pixel.
#[derive(Clone, Copy)]
struct Place {
    o: Pos2,
    s: f32,
}

impl Place {
    fn fit(area: Rect) -> Place {
        let ppp = font::ppp();
        let fit = (area.width() / 275.0).min(area.height() / 116.0);
        // whole screen pixels per skin pixel, so the art stays crisp; in a panel too small for even one, it shrinks
        // to fit rather than spilling over the edges
        let s = if fit * ppp >= 1.0 { (fit * ppp).floor() / ppp } else { fit.max(0.2) };
        // the corner on a whole screen pixel too, so every skin pixel lands exactly on screen pixels
        let snap = |v: f32| (v * ppp).round() / ppp;
        Place { o: Pos2::new(snap(area.center().x - 137.5 * s), snap(area.center().y - 58.0 * s)), s }
    }
    fn r(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.o + Vec2::new(x, y) * self.s, Vec2::new(w, h) * self.s)
    }
}

/// text.bmp: which cell (row, column) each character is in; cells are 5 x 6.
const FONT: &[(char, u8, u8)] = &[
    ('a', 0, 0),
    ('b', 0, 1),
    ('c', 0, 2),
    ('d', 0, 3),
    ('e', 0, 4),
    ('f', 0, 5),
    ('g', 0, 6),
    ('h', 0, 7),
    ('i', 0, 8),
    ('j', 0, 9),
    ('k', 0, 10),
    ('l', 0, 11),
    ('m', 0, 12),
    ('n', 0, 13),
    ('o', 0, 14),
    ('p', 0, 15),
    ('q', 0, 16),
    ('r', 0, 17),
    ('s', 0, 18),
    ('t', 0, 19),
    ('u', 0, 20),
    ('v', 0, 21),
    ('w', 0, 22),
    ('x', 0, 23),
    ('y', 0, 24),
    ('z', 0, 25),
    ('"', 0, 26),
    ('@', 0, 27),
    (' ', 0, 30),
    ('0', 1, 0),
    ('1', 1, 1),
    ('2', 1, 2),
    ('3', 1, 3),
    ('4', 1, 4),
    ('5', 1, 5),
    ('6', 1, 6),
    ('7', 1, 7),
    ('8', 1, 8),
    ('9', 1, 9),
    ('…', 1, 10),
    ('.', 1, 11),
    (':', 1, 12),
    ('(', 1, 13),
    (')', 1, 14),
    ('-', 1, 15),
    ('\'', 1, 16),
    ('!', 1, 17),
    ('_', 1, 18),
    ('+', 1, 19),
    ('\\', 1, 20),
    ('/', 1, 21),
    ('[', 1, 22),
    (']', 1, 23),
    ('^', 1, 24),
    ('&', 1, 25),
    ('%', 1, 26),
    (',', 1, 27),
    ('=', 1, 28),
    ('$', 1, 29),
    ('#', 1, 30),
    ('Å', 2, 0),
    ('Ö', 2, 1),
    ('Ä', 2, 2),
    ('?', 2, 3),
    ('*', 2, 4),
    ('<', 1, 22),
    ('>', 1, 23),
    ('{', 1, 22),
    ('}', 1, 23),
];

fn glyph(c: char) -> (u8, u8) {
    let c = c.to_ascii_lowercase();
    FONT.iter().find(|f| f.0 == c).map_or((0, 30), |f| (f.1, f.2))
}

/// Text in the skin's own font, from skin pixel (x, y).
fn wa_text(t: &WaTex, p: &egui::Painter, pl: Place, x: f32, y: f32, s: &str) {
    for (i, c) in s.chars().enumerate() {
        let (row, col) = glyph(c);
        t.spr(p, "text", (col as f32 * 5.0, row as f32 * 6.0, 5.0, 6.0), pl.r(x + i as f32 * 5.0, y, 5.0, 6.0));
    }
}

// ---------------------------------------------------------------- window frames
// With a skin on, every Tidalite window wears the skin's own frame (the pieces of its playlist window) and its
// title in the skin's font. Kept here for the free function that draws window frames.
thread_local! {
    static CHROME: std::cell::RefCell<Option<((egui::TextureHandle, Vec2), Option<(egui::TextureHandle, Vec2)>)>> =
        const { std::cell::RefCell::new(None) };
}

/// Use this skin's frames for every window (None: Tidalite's own again).
pub(crate) fn set_chrome(t: Option<&WaTex>) {
    let v = t.and_then(|t| Some((t.sheets.get("pledit")?.clone(), t.sheets.get("text").cloned())));
    CHROME.with(|c| *c.borrow_mut() = v);
}

/// A window's frame and title from the worn skin. False when no skin is worn (Tidalite draws its own).
pub(crate) fn draw_frame(p: &egui::Painter, outer: Rect, inner: Rect, title: &str) -> bool {
    CHROME.with(|c| {
        let c = c.borrow();
        let Some(((tex, size), text)) = c.as_ref() else { return false };
        let piece = |p: &egui::Painter, tex: &egui::TextureHandle, size: Vec2, src: (f32, f32, f32, f32), dst: Rect| {
            const E: f32 = 0.02;
            let uv = Rect::from_min_max(
                Pos2::new((src.0 + E) / size.x, (src.1 + E) / size.y),
                Pos2::new((src.0 + src.2 - E) / size.x, (src.1 + src.3 - E) / size.y),
            );
            p.image(tex.id(), dst, uv, Color32::WHITE);
        };
        let top_h = inner.min.y - outer.min.y;
        let k = top_h / 20.0; // the skin's 20-pixel title bar fills our title strip
        let tw = 25.0 * k;
        // the title bar: corners at the ends, tiles between
        let top = Rect::from_min_max(outer.min, Pos2::new(outer.max.x, inner.min.y));
        let clip = p.with_clip_rect(top);
        let mut x = outer.min.x + tw;
        while x < outer.max.x - tw {
            piece(
                &clip,
                tex,
                *size,
                (127.0, 0.0, 25.0, 20.0),
                Rect::from_min_size(Pos2::new(x, top.min.y), Vec2::new(tw, top_h)),
            );
            x += tw;
        }
        piece(p, tex, *size, (0.0, 0.0, 25.0, 20.0), Rect::from_min_size(top.min, Vec2::new(tw, top_h)));
        // the right end is a plain piece: the skin's own corner has minimize / close buttons, which would do
        // nothing on these windows
        piece(
            p,
            tex,
            *size,
            (127.0, 0.0, 25.0, 20.0),
            Rect::from_min_size(Pos2::new(outer.max.x - tw, top.min.y), Vec2::new(tw, top_h)),
        );
        // the sides: the skin's edge tiles, repeated down
        let (lw, rw, bh) = (inner.min.x - outer.min.x, outer.max.x - inner.max.x, outer.max.y - inner.max.y);
        for (x0, w, src) in [(outer.min.x, lw, (0.0, 42.0, 12.0, 29.0)), (inner.max.x, rw, (39.0, 42.0, 12.0, 29.0))] {
            let col = Rect::from_min_max(Pos2::new(x0, inner.min.y), Pos2::new(x0 + w, inner.max.y));
            let clip = p.with_clip_rect(col);
            let th = 29.0 * (w / 12.0);
            let mut y = col.min.y;
            while y < col.max.y {
                piece(&clip, tex, *size, src, Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, th)));
                y += th;
            }
        }
        // the bottom: the lowest rows of the skin's bottom tile, repeated across
        let rows = (bh / k).clamp(4.0, 38.0);
        let bot = Rect::from_min_max(Pos2::new(outer.min.x, inner.max.y), outer.max);
        let clip = p.with_clip_rect(bot);
        let mut x = outer.min.x;
        while x < outer.max.x {
            piece(
                &clip,
                tex,
                *size,
                (179.0, 38.0 - rows, 25.0, rows),
                Rect::from_min_size(Pos2::new(x, bot.min.y), Vec2::new(tw, bh)),
            );
            x += tw;
        }
        // the title on a dark plate in light text, so it reads over any decoration in any skin
        let size = top_h * 0.46;
        let label = title.to_uppercase();
        let tw2 = sans_width(&label, size);
        let at = Pos2::new(outer.min.x + tw + 6.0, top.center().y);
        p.rect_filled(
            Rect::from_min_max(
                Pos2::new(at.x - 5.0, top.min.y + top_h * 0.18),
                Pos2::new(at.x + tw2 + 5.0, top.max.y - top_h * 0.18),
            ),
            2.0,
            Color32::from_black_alpha(190),
        );
        sans_text(p, at, Align::Min, &label, size, outer.width() * 0.5, Color32::from_gray(235));
        let _ = text;
        true
    })
}

/// Winamp's own visualizer, 76 x 16 skin pixels from `o` at `s` points per pixel, in the skin's colours
/// (viscolor.txt): 19 bars (2 = their top colour ... 17 = the bottom, 23 = the peaks), the oscilloscope (18 to
/// 22, brightest at the centre line), or both. `mode`: 0 bars, 1 wave, 2 both.
#[allow(clippy::too_many_arguments)]
fn skin_viz(p: &egui::Painter, o: Pos2, s: f32, vis: &[Color32], bands: &[f32], peaks: &[f32], wave: &[f32], mode: u8) {
    if vis.len() < 24 {
        return;
    }
    let px = |x: f32, y: f32, w: f32, h: f32, c: Color32| {
        p.rect_filled(Rect::from_min_size(o + Vec2::new(x, y) * s, Vec2::new(w, h) * s), 0.0, c);
    };
    if mode != 1 && !bands.is_empty() {
        for b in 0..19usize {
            let i = b * bands.len() / 19;
            let h = (bands[i].clamp(0.0, 1.0) * 16.0).round() as usize;
            for row in 0..h {
                let top = 15 - row;
                px(b as f32 * 4.0, top as f32, 3.0, 1.0, vis[2 + top]);
            }
            if let Some(pk) = peaks.get(i) {
                let ph = (pk.clamp(0.0, 1.0) * 16.0).round() as usize;
                if ph > 0 {
                    px(b as f32 * 4.0, (16 - ph) as f32, 3.0, 1.0, vis[23]);
                }
            }
        }
    }
    if mode != 0 && !wave.is_empty() {
        // one dot per column, joined to the last one, as Winamp's line scope draws
        let y_at = |x: usize| {
            let v = wave[x * wave.len() / 76];
            (7.5 - v.clamp(-1.0, 1.0) * 8.0).round().clamp(0.0, 15.0) as i32
        };
        let mut prev = y_at(0);
        for x in 0..76usize {
            let y = y_at(x);
            let (a, b) = if prev < y { (prev, y) } else { (y, prev) };
            for yy in a..=b {
                let k = ((yy - 8).unsigned_abs() as usize / 2).min(4);
                px(x as f32, yy as f32, 1.0, 1.0, vis[18 + k]);
            }
            prev = y;
        }
    }
}

impl App {
    /// The visualizer settings' preview of the player screen while a skin is worn: the skin's own visualizer,
    /// as large as fits in `r`. False when no skin is worn.
    pub(crate) fn skin_viz_preview(
        &self,
        p: &egui::Painter,
        r: Rect,
        bands: &[f32],
        peaks: &[f32],
        wave: &[f32],
        mode: u8,
    ) -> bool {
        let Some(t) = self.wa_art.as_ref() else { return false };
        let ppp = font::ppp();
        let s = (((r.width() / 76.0).min(r.height() / 16.0) * ppp).floor().max(1.0)) / ppp;
        let o = Pos2::new(r.center().x - 38.0 * s, r.center().y - 8.0 * s);
        p.rect_filled(Rect::from_min_size(o, Vec2::new(76.0, 16.0) * s), 0.0, t.vis[0]);
        skin_viz(p, o, s, &t.vis, bands, peaks, wave, mode);
        true
    }

    /// The main window, drawn from the worn skin. Everything is clickable and drives Tidalite.
    pub(crate) fn winamp_main(&mut self, ui: &mut egui::Ui, area: Rect, acts: &mut Vec<Action>) {
        let Some(t) = self.wa_art.take() else { return };
        // the player sits in the middle, in line with the equalizer under it; the album cover is small, in the
        // margin to its right, framed in the skin's colours; a click on it opens the album view
        let side = (area.height() * 0.55).min(area.width() * 0.14);
        let skin_area = area.shrink2(Vec2::new(side + 20.0, 0.0));
        let pl = Place::fit(skin_area);
        let right = pl.o.x + 275.0 * pl.s;
        let cover = Rect::from_center_size(Pos2::new((right + area.max.x) / 2.0, area.center().y), Vec2::splat(side));
        if let Some(tr) = self.cur_track() {
            let url = cover_url(&tr.cover, 320);
            // framed in the skin's colours
            raised(ui.painter(), cover.expand(4.0), false);
            inset(ui.painter(), cover.expand(1.0), pal().lcd);
            paint_held(ui, &mut self.images, "player", &url, cover, false);
        } else {
            paint_record(ui.painter(), cover, None);
        }
        let ch = ui
            .interact(cover, ui.id().with("winamp_cover"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip("Album view  (A)");
        if ch.clicked() {
            acts.push(Action::ToggleArt);
        }
        let p = ui.painter().clone();
        let track = self.cur_track();
        let pos = self.seek_drag.unwrap_or(self.pos());
        let dur = track.as_ref().map(|t| t.duration).unwrap_or(0.0);
        let active = self.cur.is_some() && !self.stopped;
        let playing = active && !self.paused;
        let rate = self.player.shared.lock().unwrap().rate;
        let id = ui.id().with("winamp_main");
        let hit = |ui: &mut egui::Ui, name: &str, r: Rect, drag: bool| {
            ui.interact(r, id.with(name), if drag { Sense::click_and_drag() } else { Sense::click() })
                .on_hover_cursor(egui::CursorIcon::PointingHand)
        };

        t.spr(&p, "main", (0.0, 0.0, 275.0, 116.0), pl.r(0.0, 0.0, 275.0, 116.0));
        t.spr(&p, "titlebar", (27.0, 0.0, 275.0, 14.0), pl.r(0.0, 0.0, 275.0, 14.0));
        // the title bar's own buttons: options menu, minimize, shade (MINI), close
        let tb = |ui: &mut egui::Ui, name: &str, x: f32, src: (f32, f32), tip: &str| -> egui::Response {
            let r = pl.r(x, 3.0, 9.0, 9.0);
            let h = ui.interact(r, id.with(name), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).tip(tip);
            if h.is_pointer_button_down_on() {
                t.spr(&p, "titlebar", (src.0, src.1 + 9.0, 9.0, 9.0), r);
            }
            h
        };
        let opt = tb(ui, "opt", 6.0, (0.0, 0.0), "Options: skins, preferences, help...");
        let opt_menu = id.with("opt_menu");
        if opt.clicked() {
            ui.memory_mut(|m| m.toggle_popup(opt_menu));
        }
        egui::popup::popup_below_widget(ui, opt_menu, &opt, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
            if self.app_menu_items(ui, acts) {
                ui.memory_mut(|m| m.close_popup());
            }
        });
        if tb(ui, "min", 244.0, (9.0, 0.0), "Minimize").clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        if tb(ui, "shade", 254.0, (0.0, 18.0), "Mini player").clicked() {
            acts.push(Action::ToggleMini);
        }
        if tb(ui, "close", 264.0, (18.0, 0.0), "Quit Tidalite").clicked() {
            acts.push(Action::QuitApp);
        }
        // the clutter bar: Options, Always on top, song Info, Double size, Visualizer
        if t.size("titlebar").map_or(false, |s| s.x >= 344.0) {
            t.spr(&p, "titlebar", (304.0, 0.0, 8.0, 43.0), pl.r(10.0, 22.0, 8.0, 43.0));
            let zoomed = ui.ctx().zoom_factor() > 1.4;
            let items: [(&str, f32, f32, f32, bool, &str); 5] = [
                ("o", 304.0, 3.0, 8.0, false, "Options"),
                ("a", 312.0, 11.0, 7.0, self.pinned, "Always on top"),
                ("i", 320.0, 18.0, 7.0, false, "Song info (credits)"),
                ("d", 328.0, 25.0, 8.0, zoomed, "Double size"),
                ("v", 336.0, 33.0, 7.0, false, "Visualizer settings"),
            ];
            for (name, sx, y, h, on, tip) in items {
                let r = pl.r(10.0, 22.0 + y, 8.0, h);
                let hr = ui
                    .interact(r, id.with(("clutter", name)), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .tip(tip);
                if on || hr.is_pointer_button_down_on() {
                    t.spr(&p, "titlebar", (sx, 44.0 + y, 8.0, h), r);
                }
                if hr.clicked() {
                    match name {
                        "o" => ui.memory_mut(|m| m.toggle_popup(opt_menu)),
                        "a" => acts.push(Action::TogglePin),
                        "i" => {
                            if let Some(tr) = self.cur_track().filter(|t| self.ext_of(t.id).is_none()) {
                                acts.push(Action::Credits(tr.id, format!("{} - {}", tr.artists_text(), tr.title)));
                            }
                        }
                        "d" => {
                            ui.ctx().set_zoom_factor(if zoomed { 1.0 } else { 2.0 });
                            self.dirty = true;
                        }
                        _ => self.viz_panel = Some(0),
                    }
                }
            }
        }
        // playing / paused / stopped
        let st = if playing {
            0.0
        } else if active {
            9.0
        } else {
            18.0
        };
        t.spr(&p, "playpaus", (st, 0.0, 9.0, 9.0), pl.r(26.0, 28.0, 9.0, 9.0));
        // the time, in the skin's big digits
        let digits = if t.size("nums_ex").is_some() { "nums_ex" } else { "numbers" };
        let secs = pos.max(0.0) as u32;
        let (m, s) = ((secs / 60) % 100, secs % 60);
        if active {
            for (x, d) in [(48.0, m / 10), (60.0, m % 10), (78.0, s / 10), (90.0, s % 10)] {
                t.spr(&p, digits, (d as f32 * 9.0, 0.0, 9.0, 13.0), pl.r(x, 26.0, 9.0, 13.0));
            }
        }
        // the scrolling title; while a slider is hovered or dragged it says where that slider is, as Winamp does
        let hint_id = id.with("hint");
        let hint: Option<String> = ui.ctx().data_mut(|d| d.remove_temp(hint_id));
        let title = match (&hint, &track) {
            (Some(h), _) => h.clone(),
            (None, Some(tr)) => format!("{} - {} ({})", tr.artists_text(), tr.title, fmt_time(tr.duration)),
            (None, None) => "TIDALITE - A RETRO PLAYER FOR TIDAL".to_string(),
        };
        let marquee = pl.r(111.0, 24.0, 154.0, 6.0);
        let w = title.chars().count() as f32 * 5.0;
        let shift = if w > 154.0 && hint.is_none() { ((ui.input(|i| i.time) as f32 * 22.0) % (w + 25.0)).floor() } else { 0.0 };
        let clip = p.with_clip_rect(marquee);
        wa_text(&t, &clip, pl, 111.0 - shift, 24.0, &title);
        if w > 154.0 {
            wa_text(&t, &clip, pl, 111.0 - shift + w + 25.0, 24.0, &title);
            ui.ctx().request_repaint();
        }
        // kbps, kHz, mono / stereo
        if active {
            wa_text(&t, &p, pl, 111.0, 43.0, &format!("{:>3}", self.kbps.min(999)));
            wa_text(&t, &p, pl, 156.0, 43.0, &format!("{:>2}", (rate / 1000).min(99)));
        }
        t.spr(&p, "monoster", (29.0, 12.0, 27.0, 12.0), pl.r(212.0, 41.0, 27.0, 12.0));
        t.spr(&p, "monoster", (0.0, if active { 0.0 } else { 12.0 }, 29.0, 12.0), pl.r(239.0, 41.0, 29.0, 12.0));
        // the visualizer, in the skin's own colours (2 = top of the bars ... 17 = bottom, 23 = peaks);
        // click it for bars / wave / both, right-click for all its settings
        let vz = hit(ui, "viz", pl.r(24.0, 43.0, 76.0, 16.0), false).tip("Visualizer: click to change, right-click for settings");
        if vz.clicked() {
            self.viz[0].mode = (self.viz[0].mode + 1) % 3;
            self.dirty = true;
        }
        if vz.secondary_clicked() {
            self.viz_panel = Some(0);
        }
        let (bands, peaks, wave) = self.viz_data(0);
        if active {
            skin_viz(&p, pl.o + Vec2::new(24.0, 43.0) * pl.s, pl.s, &t.vis, &bands, &peaks, &wave, self.viz[0].mode);
            ui.ctx().request_repaint();
        }
        // volume: the strip picture follows the level; right-click it for the sound output
        let vr = pl.r(107.0, 57.0, 68.0, 13.0);
        // as many level pictures as this skin has (usually 28), 15 pixels apart
        let frames = |sheet: &str| t.size(sheet).map_or(28.0, |s| ((s.y + 2.0) / 15.0).floor().clamp(1.0, 28.0));
        let frame = (self.volume * (frames("volume") - 1.0)).round();
        t.spr(&p, "volume", (0.0, frame * 15.0, 68.0, 13.0), vr);
        let vh = hit(ui, "volume", vr, true)
            .tip(format!("Volume {}%  (right-click for the sound output)", (self.volume * 100.0).round()));
        if t.size("volume").map_or(false, |s| s.y >= 433.0) {
            let src = if vh.dragged() { (0.0, 422.0, 14.0, 11.0) } else { (15.0, 422.0, 14.0, 11.0) };
            t.spr(&p, "volume", src, pl.r(107.0 + self.volume * 54.0, 58.0, 14.0, 11.0));
        }
        if vh.dragged() || vh.clicked() {
            if let Some(pp) = vh.interact_pointer_pos() {
                // the knob's middle follows the mouse, as in Winamp
                acts.push(Action::Volume(((pp.x - vr.min.x - pl.s * 7.0) / (pl.s * 54.0)).clamp(0.0, 1.0)));
            }
        }
        let mut say: Option<String> = None;
        if vh.hovered() || vh.dragged() {
            say = Some(format!("VOLUME: {}%", (self.volume * 100.0).round()));
        }
        vh.context_menu(|ui| self.output_menu(ui, acts));
        // balance: left / right; double-click to centre it again
        let br = pl.r(177.0, 57.0, 38.0, 13.0);
        // skins without a balance picture borrow the volume one, as Winamp does
        let bal = if t.size("balance").is_some() { "balance" } else { "volume" };
        let bframe = (self.balance.unsigned_abs() as f32 / 100.0 * (frames(bal) - 1.0)).round();
        t.spr(&p, bal, (9.0, bframe * 15.0, 38.0, 13.0), br);
        let bh = hit(ui, "balance", br, true).tip(match self.balance {
            0 => "Balance: centre  (drag; double-click to centre)".to_string(),
            b if b < 0 => format!("Balance: {}% left  (double-click to centre)", -b),
            b => format!("Balance: {}% right  (double-click to centre)", b),
        });
        if t.size(bal).map_or(false, |s| s.y >= 433.0) {
            let src = if bh.dragged() { (0.0, 422.0, 14.0, 11.0) } else { (15.0, 422.0, 14.0, 11.0) };
            t.spr(&p, bal, src, pl.r(177.0 + (self.balance + 100) as f32 / 200.0 * 24.0, 58.0, 14.0, 11.0));
        }
        if bh.hovered() || bh.dragged() {
            say = Some(match self.balance {
                0 => "BALANCE: CENTER".to_string(),
                b if b < 0 => format!("BALANCE: {}% LEFT", -b),
                b => format!("BALANCE: {}% RIGHT", b),
            });
        }
        if bh.double_clicked() {
            acts.push(Action::Balance(0));
        } else if bh.dragged() || bh.clicked() {
            if let Some(pp) = bh.interact_pointer_pos() {
                let f = ((pp.x - br.min.x - pl.s * 7.0) / (pl.s * 24.0)).clamp(0.0, 1.0);
                let b = (f * 200.0 - 100.0).round() as i32;
                // it clicks into the middle
                acts.push(Action::Balance(if b.abs() < 10 { 0 } else { b }));
            }
        }
        // EQ and playlist buttons
        let eq = pl.r(219.0, 58.0, 23.0, 12.0);
        let eqh = hit(ui, "eq", eq, false).tip("Equalizer");
        let eq_src = match (self.show_eq, eqh.is_pointer_button_down_on()) {
            (false, false) => (0.0, 61.0),
            (true, false) => (0.0, 73.0),
            (false, true) => (46.0, 61.0),
            (true, true) => (46.0, 73.0),
        };
        t.spr(&p, "shufrep", (eq_src.0, eq_src.1, 23.0, 12.0), eq);
        if eqh.clicked() {
            acts.push(Action::ToggleEq);
        }
        // PL: shows or hides the playlist (the queue)
        let plr = pl.r(242.0, 58.0, 23.0, 12.0);
        let plh = hit(ui, "pl", plr, false).tip(if self.queue_hidden { "Show the queue" } else { "Hide the queue" });
        let pl_src = match (!self.queue_hidden, plh.is_pointer_button_down_on()) {
            (false, false) => (23.0, 61.0),
            (true, false) => (23.0, 73.0),
            (false, true) => (69.0, 61.0),
            (true, true) => (69.0, 73.0),
        };
        t.spr(&p, "shufrep", (pl_src.0, pl_src.1, 23.0, 12.0), plr);
        if plh.clicked() {
            self.queue_hidden = !self.queue_hidden;
        }
        // the seek bar
        let pr = pl.r(16.0, 72.0, 248.0, 10.0);
        t.spr(&p, "posbar", (0.0, 0.0, 248.0, 10.0), pr);
        let ph = hit(ui, "seek", pr, true).tip("Drag or click to move in the song");
        if active && dur > 0.0 {
            let f = (pos / dur).clamp(0.0, 1.0);
            let src = if ph.dragged() { (278.0, 0.0, 29.0, 10.0) } else { (248.0, 0.0, 29.0, 10.0) };
            t.spr(&p, "posbar", src, pl.r(16.0 + f * 219.0, 72.0, 29.0, 10.0));
            if ph.dragged() || ph.clicked() {
                if let Some(pp) = ph.interact_pointer_pos() {
                    self.seek_drag = Some(((pp.x - pr.min.x - 14.5 * pl.s) / (219.0 * pl.s)).clamp(0.0, 1.0) * dur);
                }
            }
            if ph.dragged() || ph.hovered() {
                let at = self.seek_drag.unwrap_or(pos);
                say = Some(format!("SEEK TO: {}/{} ({}%)", fmt_time(at), fmt_time(dur), (at / dur * 100.0).round()));
            }
            if ph.drag_stopped() || ph.clicked() {
                if let Some(s) = self.seek_drag.take() {
                    acts.push(Action::Seek(s));
                }
            }
        }
        if let Some(s) = say {
            ui.ctx().data_mut(|d| d.insert_temp(hint_id, s));
            ui.ctx().request_repaint();
        }
        // transport
        let buttons: [(&str, f32, f32, f32, Action, &str); 5] = [
            ("prev", 16.0, 0.0, 23.0, Action::Prev, "Previous"),
            ("play", 39.0, 23.0, 23.0, Action::PlayBtn, "Play  (Space)"),
            ("pause", 62.0, 46.0, 23.0, Action::PauseBtn, "Pause  (Space)"),
            ("stop", 85.0, 69.0, 23.0, Action::StopBtn, "Stop"),
            ("next", 108.0, 92.0, 22.0, Action::Next, "Next"),
        ];
        for (name, x, sx, w, a, tip) in buttons {
            let r = pl.r(x, 88.0, w, 18.0);
            let h = hit(ui, name, r, false).tip(tip);
            let down = if h.is_pointer_button_down_on() { 18.0 } else { 0.0 };
            t.spr(&p, "cbuttons", (sx, down, w, 18.0), r);
            if h.clicked() {
                acts.push(a);
            }
        }
        let ej = pl.r(136.0, 89.0, 22.0, 16.0);
        let ejh = hit(ui, "eject", ej, false).tip("Open files");
        t.spr(&p, "cbuttons", (114.0, if ejh.is_pointer_button_down_on() { 16.0 } else { 0.0 }, 22.0, 16.0), ej);
        if ejh.clicked() {
            acts.push(Action::AddFiles);
        }
        // shuffle and repeat
        let sh = pl.r(164.0, 89.0, 47.0, 15.0);
        let shh = hit(ui, "shuffle", sh, false).tip("Shuffle");
        let sy = if self.shuffle { 30.0 } else { 0.0 } + if shh.is_pointer_button_down_on() { 15.0 } else { 0.0 };
        t.spr(&p, "shufrep", (28.0, sy, 47.0, 15.0), sh);
        if shh.clicked() {
            acts.push(Action::Shuffle);
        }
        let rp = pl.r(210.0, 89.0, 28.0, 15.0);
        let rph = hit(ui, "repeat", rp, false).tip(match self.repeat {
            Repeat::Off => "Repeat: off  (click: repeat all)",
            Repeat::All => "Repeat: all  (click: repeat this song)",
            _ => "Repeat: this song  (click: off)",
        });
        let ry = if self.repeat != Repeat::Off { 30.0 } else { 0.0 } + if rph.is_pointer_button_down_on() { 15.0 } else { 0.0 };
        t.spr(&p, "shufrep", (0.0, ry, 28.0, 15.0), rp);
        // Winamp's button has only on / off: repeating one song gets a small "1" in the skin's own font, in the
        // button's corner, so every skin shows which it is
        if self.repeat == Repeat::One {
            p.rect_filled(pl.r(231.0, 90.0, 6.0, 7.0), 0.0, t.vis.first().copied().unwrap_or(Color32::BLACK));
            wa_text(&t, &p, pl, 232.0, 90.5, "1");
        }
        if rph.clicked() {
            acts.push(Action::Repeat);
        }
        self.wa_art = Some(t);
    }

    /// The equalizer, drawn from the worn skin: ON, PRESETS, the curve and the ten sliders.
    pub(crate) fn winamp_eq(&mut self, ui: &mut egui::Ui, area: Rect) {
        let Some(t) = self.wa_art.take() else { return };
        let pl = Place::fit(area);
        let p = ui.painter().clone();
        let id = ui.id().with("winamp_eq");
        let mut changed = false;
        t.spr(&p, "eqmain", (0.0, 0.0, 275.0, 116.0), pl.r(0.0, 0.0, 275.0, 116.0));
        t.spr(&p, "eqmain", (0.0, 134.0, 275.0, 14.0), pl.r(0.0, 0.0, 275.0, 14.0));
        // ON
        let on_r = pl.r(14.0, 18.0, 26.0, 12.0);
        let on_h = ui
            .interact(on_r, id.with("on"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip("Equalizer on / off");
        let on_src = if self.eq_on { (69.0, 119.0) } else { (10.0, 119.0) };
        t.spr(&p, "eqmain", (on_src.0, on_src.1, 26.0, 12.0), on_r);
        if on_h.clicked() {
            self.eq_on = !self.eq_on;
            changed = true;
        }
        // AUTO: remember the EQ for each song and bring it back when that song plays
        let au_r = pl.r(40.0, 18.0, 32.0, 12.0);
        let au = ui
            .interact(au_r, id.with("auto"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip("AUTO: remember the EQ for each song and bring it back when it plays");
        t.spr(&p, "eqmain", (if self.eq_auto { 95.0 } else { 36.0 }, 119.0, 32.0, 12.0), au_r);
        if au.clicked() {
            self.eq_auto = !self.eq_auto;
            self.dirty = true;
        }
        // the title bar's close button closes the equalizer
        let cl = pl.r(264.0, 3.0, 9.0, 9.0);
        let clh = ui.interact(cl, id.with("close"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).tip("Close");
        if clh.is_pointer_button_down_on() {
            t.spr(&p, "eqmain", (0.0, 125.0, 9.0, 9.0), cl);
        }
        if clh.clicked() {
            self.show_eq = false;
        }
        // PRESETS: click for the list
        let pr_r = pl.r(217.0, 18.0, 44.0, 12.0);
        let pr_h =
            ui.interact(pr_r, id.with("presets"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).tip("Presets");
        t.spr(&p, "eqmain", (224.0, if pr_h.is_pointer_button_down_on() { 176.0 } else { 164.0 }, 44.0, 12.0), pr_r);
        let menu_id = id.with("presets_menu");
        if pr_h.clicked() {
            ui.memory_mut(|m| m.toggle_popup(menu_id));
        }
        egui::popup::popup_below_widget(ui, menu_id, &pr_h, egui::PopupCloseBehavior::CloseOnClick, |ui| {
            ui.set_min_width(140.0);
            for (name, g) in EQ_PRESETS.iter() {
                if menu_item(ui, name) {
                    self.eq_gains = *g;
                    self.eq_on = true;
                    changed = true;
                }
            }
        });
        // the curve, in the skin's own curve colours
        t.spr(&p, "eqmain", (0.0, 294.0, 113.0, 19.0), pl.r(86.0, 17.0, 113.0, 19.0));
        for x in 0..112 {
            let f = x as f32 / 111.0 * 9.0;
            let i = (f.floor() as usize).min(8);
            let u = f - i as f32;
            let (a, b) = (self.eq_gains[i], self.eq_gains[i + 1]);
            let g = a + (b - a) * (u * u * (3.0 - 2.0 * u));
            let y = (26.0 - g / 12.0 * 9.0).round().clamp(17.0, 35.0);
            let c = t.graph.get((y - 17.0) as usize).copied().unwrap_or(Color32::YELLOW);
            p.rect_filled(pl.r(86.0 + x as f32, y, 1.0, 1.0), 0.0, c);
        }
        // the preamp slider (a plain gain before the bands) and the ten bands
        let band = |p: &egui::Painter, x: f32, value: f32, down: bool| {
            let n = (value / 100.0 * 27.0).round().clamp(0.0, 27.0) as i32;
            let (sx, sy) = ((n % 14) as f32 * 15.0, (n / 14) as f32 * 65.0);
            t.spr(p, "eqmain", (13.0 + sx, 164.0 + sy, 14.0, 63.0), pl.r(x, 38.0, 14.0, 63.0));
            let ty = 38.0 + (1.0 - value / 100.0) * 51.0;
            t.spr(p, "eqmain", (0.0, if down { 176.0 } else { 164.0 }, 11.0, 11.0), pl.r(x + 1.0, ty, 11.0, 11.0));
        };
        let pr = pl.r(21.0, 38.0, 14.0, 63.0);
        let ph = ui.interact(pr, id.with("preamp"), Sense::click_and_drag()).on_hover_cursor(egui::CursorIcon::PointingHand).tip(
            format!("Preamp: {:+} dB, louder or softer before the bands  (double-click to reset)", self.eq_pre.round() as i32),
        );
        if ph.dragged() || ph.clicked() {
            if let Some(pp) = ph.interact_pointer_pos() {
                let f = ((pp.y - pr.min.y - 5.5 * pl.s) / (51.0 * pl.s)).clamp(0.0, 1.0);
                let mut g = ((12.0 - 24.0 * f) * 2.0).round() / 2.0;
                if g.abs() < 0.75 {
                    g = 0.0;
                }
                self.eq_pre = g;
                self.eq_on = true;
                changed = true;
            }
        }
        if ph.double_clicked() {
            self.eq_pre = 0.0;
            changed = true;
        }
        band(&p, 21.0, (self.eq_pre + 12.0) / 24.0 * 100.0, ph.dragged());
        for i in 0..10 {
            let x = 78.0 + i as f32 * 18.0;
            let r = pl.r(x, 38.0, 14.0, 63.0);
            const HZ: [&str; 10] = ["31", "62", "125", "250", "500", "1K", "2K", "4K", "8K", "16K"];
            let h = ui
                .interact(r, id.with(("band", i)), Sense::click_and_drag())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .tip(format!("{} Hz: {:+} dB  (double-click to reset)", HZ[i], self.eq_gains[i].round() as i32));
            if h.dragged() || h.clicked() {
                if let Some(pp) = h.interact_pointer_pos() {
                    let f = ((pp.y - r.min.y - 5.5 * pl.s) / (51.0 * pl.s)).clamp(0.0, 1.0);
                    let mut g = ((12.0 - 24.0 * f) * 2.0).round() / 2.0;
                    if g.abs() < 0.75 {
                        g = 0.0;
                    }
                    self.eq_gains[i] = g;
                    self.eq_on = true;
                    changed = true;
                }
            }
            if h.double_clicked() {
                self.eq_gains[i] = 0.0;
                changed = true;
            }
            band(&p, x, (self.eq_gains[i] + 12.0) / 24.0 * 100.0, h.dragged());
        }
        self.wa_art = Some(t);
        if changed {
            self.apply_eq();
            self.eq_remember();
            self.dirty = true;
        }
    }

    /// The sound-output button's pop-up (shared by the player and the album view).
    pub(crate) fn output_popup(&mut self, ui: &mut egui::Ui, r: &egui::Response, acts: &mut Vec<Action>) {
        let id = r.id.with("outputs");
        if r.clicked() {
            self.out_list = crate::player::output_devices();
            ui.memory_mut(|m| m.toggle_popup(id));
        }
        egui::popup::popup_below_widget(ui, id, r, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
            ui.set_min_width(300.0);
            self.output_menu(ui, acts);
        });
    }

    /// The sound outputs as a menu.
    pub(crate) fn output_menu(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        if self.out_list.is_empty() {
            self.out_list = crate::player::output_devices();
        }
        para(ui, "SOUND OUTPUT", pal().ink2);
        let mark = |on: bool| if on { "> " } else { "   " };
        if menu_item(ui, &format!("{}System default", mark(self.out_device.is_none()))) {
            acts.push(Action::SetDevice(None));
            ui.close_menu();
        }
        for name in self.out_list.clone() {
            let on = self.out_device.as_deref() == Some(name.as_str());
            if menu_item(ui, &format!("{}{}", mark(on), name)) {
                acts.push(Action::SetDevice(Some(name)));
                ui.close_menu();
            }
        }
        if menu_item(ui, "   Look again (after plugging in or pairing)") {
            self.out_list = crate::player::output_devices();
        }
    }
}

// ---------------------------------------------------------------- the playlist font
// Winamp's playlist is written in an ordinary font, and each skin names its own (Tahoma, Verdana, Arial...).
// That one is used when the computer has it; the playlist's MISC menu offers others, classic Windows fonts and
// free retro ones (open-licensed, from Google Fonts) fetched once and kept.

pub(crate) enum FontSrc {
    /// the one the skin asks for
    Skin,
    /// a font installed on the computer, by family name
    Sys(&'static str),
    /// fetched once from Google Fonts: (path in google/fonts, the pixel size it is drawn at)
    Web(&'static str, f32),
}

pub(crate) const PL_FONTS: [(&str, FontSrc); 13] = [
    ("THE SKIN'S OWN", FontSrc::Skin),
    ("TAHOMA", FontSrc::Sys("tahoma")),
    ("MS SANS SERIF", FontSrc::Sys("microsoft sans serif")),
    ("VERDANA", FontSrc::Sys("verdana")),
    ("ARIAL", FontSrc::Sys("arial")),
    ("LUCIDA CONSOLE", FontSrc::Sys("lucida console")),
    ("VT323 (TERMINAL)", FontSrc::Web("ofl/vt323/VT323-Regular.ttf", 14.0)),
    ("PIXELIFY SANS", FontSrc::Web("ofl/pixelifysans/PixelifySans%5Bwght%5D.ttf", 12.0)),
    ("SILKSCREEN", FontSrc::Web("ofl/silkscreen/Silkscreen-Regular.ttf", 8.0)),
    ("SHARE TECH MONO", FontSrc::Web("ofl/sharetechmono/ShareTechMono-Regular.ttf", 11.0)),
    ("PRESS START 2P", FontSrc::Web("ofl/pressstart2p/PressStart2P-Regular.ttf", 8.0)),
    ("JERSEY 10", FontSrc::Web("ofl/jersey10/Jersey10-Regular.ttf", 12.0)),
    ("MICRO 5", FontSrc::Web("ofl/micro5/Micro5-Regular.ttf", 10.0)),
];

/// Windows' list of installed fonts: family name (lower case, e.g. "arial narrow", "trebuchet ms italic") to file.
fn installed_fonts() -> &'static HashMap<String, std::path::PathBuf> {
    static MAP: std::sync::OnceLock<HashMap<String, std::path::PathBuf>> = std::sync::OnceLock::new();
    MAP.get_or_init(|| {
        let mut map = HashMap::new();
        if !cfg!(windows) {
            return map;
        }
        let key = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts";
        for root in ["HKLM", "HKCU"] {
            let Ok(out) = std::process::Command::new("reg").args(["query", &format!("{}\\{}", root, key)]).output() else {
                continue;
            };
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let Some((name, file)) = line.split_once("REG_SZ") else { continue };
                let file = file.trim();
                let lower = file.to_ascii_lowercase();
                // only outline fonts can be drawn (the old .fon bitmap ones cannot)
                if !(lower.ends_with(".ttf") || lower.ends_with(".otf") || lower.ends_with(".ttc")) {
                    continue;
                }
                let path = if file.contains('\\') {
                    std::path::PathBuf::from(file)
                } else {
                    std::path::Path::new("C:\\Windows\\Fonts").join(file)
                };
                let name = name.trim().trim_end_matches("(TrueType)").trim_end_matches("(OpenType)").trim();
                for n in name.split(" & ") {
                    map.entry(n.trim().to_ascii_lowercase()).or_insert_with(|| path.clone());
                }
            }
        }
        map
    })
}

/// A font installed on this computer, by the name a skin gives (Windows' font list first, then the usual files
/// and the Mac font folders).
fn system_font(name: &str) -> Option<Vec<u8>> {
    let n = name.trim().to_ascii_lowercase();
    if let Some(b) = installed_fonts().get(&n).and_then(|p| std::fs::read(p).ok()) {
        return Some(b);
    }
    let file = match n.as_str() {
        "arial" => "arial.ttf",
        "tahoma" => "tahoma.ttf",
        "verdana" => "verdana.ttf",
        "trebuchet ms" | "trebuchet" => "trebuc.ttf",
        "lucida console" => "lucon.ttf",
        "ms sans serif" | "microsoft sans serif" | "sans serif" => "micross.ttf",
        "courier new" | "courier" => "cour.ttf",
        "times new roman" | "times" => "times.ttf",
        "georgia" => "georgia.ttf",
        "comic sans ms" => "comic.ttf",
        "segoe ui" => "segoeui.ttf",
        "consolas" => "consola.ttf",
        _ => "",
    };
    let squashed = format!("{}.ttf", n.replace(' ', ""));
    let mac = name.trim().to_string();
    let mut tries: Vec<String> = Vec::new();
    for f in [file, squashed.as_str()] {
        if !f.is_empty() {
            tries.push(format!("C:\\Windows\\Fonts\\{}", f));
        }
    }
    tries.push(format!("/System/Library/Fonts/Supplemental/{}.ttf", mac));
    tries.push(format!("/Library/Fonts/{}.ttf", mac));
    tries.iter().find_map(|p| std::fs::read(p).ok())
}

fn font_dir() -> std::path::PathBuf {
    crate::api::config_dir().join("fonts")
}

/// The font for a menu choice when it is at hand (installed, or fetched before). None: it needs fetching
/// (or the skin's font is not on this computer).
pub(crate) fn font_now(choice: usize, skin_font: Option<&str>) -> Option<Vec<u8>> {
    match &PL_FONTS.get(choice)?.1 {
        FontSrc::Skin => skin_font.and_then(system_font).or_else(|| system_font("arial")).or_else(|| system_font("tahoma")),
        FontSrc::Sys(n) => system_font(n),
        FontSrc::Web(path, _) => std::fs::read(font_dir().join(path.rsplit('/').next()?)).ok(),
    }
}

/// Fetch a free font once and keep it (call off the main thread).
pub(crate) fn fetch_font(choice: usize) -> Result<Vec<u8>, String> {
    let FontSrc::Web(path, _) = &PL_FONTS.get(choice).ok_or("no such font")?.1 else { return Err("not a download".into()) };
    let url = format!("https://github.com/google/fonts/raw/main/{}", path);
    let b = reqwest::blocking::get(&url).and_then(|r| r.error_for_status()).and_then(|r| r.bytes()).map_err(|e| e.to_string())?;
    let _ = std::fs::create_dir_all(font_dir());
    let _ = std::fs::write(font_dir().join(path.rsplit('/').next().unwrap_or("font.ttf")), &b);
    Ok(b.to_vec())
}

/// The playlist font, drawn the way Windows drew it for Winamp: the font's own hinting snaps every stroke onto
/// whole pixels (so stems keep one even width instead of breaking up), and each pixel is fully on or off, like
/// Windows text before ClearType. A font's built-in bitmaps for small sizes win when it has them, as on Windows.
/// Lines are drawn straight at the size they show on screen, once, and kept.
struct PixFont {
    bytes: std::sync::Arc<Vec<u8>>,
    /// the font's own size (Winamp's 10 pixels for ordinary fonts, a pixel font's native size)
    px: f32,
    scaler: swash::scale::ScaleContext,
    cache: HashMap<(String, u32), (egui::TextureHandle, Vec2)>,
}

impl PixFont {
    fn new(bytes: Vec<u8>, px: f32) -> Option<PixFont> {
        swash::FontRef::from_index(&bytes, 0)?;
        Some(PixFont { bytes: std::sync::Arc::new(bytes), px, scaler: swash::scale::ScaleContext::new(), cache: HashMap::new() })
    }

    fn font(&self) -> swash::FontRef<'_> {
        swash::FontRef::from_index(&self.bytes, 0).expect("checked in new")
    }

    /// Each character's glyph and its advance in whole pixels at `size` (whole pixels, as Windows lays text out).
    fn glyphs(&self, s: &str, size: f32) -> Vec<(swash::GlyphId, f32)> {
        let font = self.font();
        let (map, metrics) = (font.charmap(), font.glyph_metrics(&[]).scale(size));
        s.chars()
            .map(|ch| {
                let id = map.map(ch);
                (id, metrics.advance_width(id).round())
            })
            .collect()
    }

    /// Width in screen pixels at `size` pixels.
    fn width_at(&self, s: &str, size: f32) -> f32 {
        self.glyphs(s, size).iter().map(|g| g.1).sum()
    }

    #[cfg(test)]
    fn width(&self, s: &str) -> f32 {
        self.width_at(s, self.px)
    }

    /// The line's pixels at `size`: width, height and one coverage byte per pixel (0 or 255).
    fn raster(&mut self, s: &str, size: f32) -> (usize, usize, Vec<u8>) {
        use swash::scale::{Render, Source, StrikeWith};
        let glyphs = self.glyphs(s, size);
        let bytes = self.bytes.clone();
        let font = swash::FontRef::from_index(&bytes, 0).expect("checked in new");
        let m = font.metrics(&[]).scale(size);
        let asc = m.ascent.round() as i32;
        let h = (m.ascent + m.descent).ceil() as usize + 2;
        let w = glyphs.iter().map(|g| g.1).sum::<f32>() as usize + 4;
        let mut px = vec![0u8; w * h];
        let mut scaler = self.scaler.builder(font).size(size).hint(true).build();
        let mut x = 1i32;
        for (id, adv) in glyphs {
            let img = Render::new(&[Source::Bitmap(StrikeWith::ExactSize), Source::Outline])
                .format(swash::zeno::Format::Alpha)
                .render(&mut scaler, id);
            if let Some(img) = img {
                let pl = img.placement;
                let embedded = matches!(img.source, Source::Bitmap(_));
                for gy in 0..pl.height as i32 {
                    for gx in 0..pl.width as i32 {
                        let c = img.data[(gy * pl.width as i32 + gx) as usize];
                        // hinted outlines sit on the pixel grid; a stroke a little off it still counts (a quarter
                        // covered), so light fonts like Courier keep every stroke at small sizes
                        if c >= if embedded { 1 } else { 64 } {
                            let (dx, dy) = (x + pl.left + gx, 1 + asc - pl.top + gy);
                            if dx >= 0 && dy >= 0 && (dx as usize) < w && (dy as usize) < h {
                                px[dy as usize * w + dx as usize] = 255;
                            }
                        }
                    }
                }
            }
            x += adv as i32;
        }
        (w, h, px)
    }

    /// The line drawn at `size` screen pixels (cached).
    fn line(&mut self, ctx: &egui::Context, s: &str, size: f32) -> (egui::TextureId, Vec2) {
        let key = (s.to_string(), size.round() as u32);
        if let Some((t, sz)) = self.cache.get(&key) {
            return (t.id(), *sz);
        }
        if self.cache.len() > 900 {
            self.cache.clear();
        }
        let (w, h, cov) = self.raster(s, size.round());
        let pixels = cov.iter().map(|&v| Color32::from_rgba_premultiplied(v, v, v, v)).collect();
        let tex = ctx.load_texture("playlist_line", egui::ColorImage { size: [w, h], pixels }, egui::TextureOptions::NEAREST);
        let sz = Vec2::new(w as f32, h as f32);
        let id = tex.id();
        self.cache.insert(key, (tex, sz));
        (id, sz)
    }

    /// The size in screen pixels for text meant to look about `size` points tall: a whole multiple of the
    /// font's own size (pixel fonts stay pixel-perfect), never smaller than it.
    fn screen_px(&self, size: f32, ppp: f32) -> f32 {
        let want = size * ppp;
        if self.px >= 10.0 {
            want.round().max(self.px)
        } else {
            (want / self.px).round().max(1.0) * self.px
        }
    }
}

thread_local! {
    static PIX: std::cell::RefCell<Option<PixFont>> = const { std::cell::RefCell::new(None) };
}

/// Make `bytes` the playlist font (None: back to plain text).
pub(crate) fn set_playlist_font(_ctx: &egui::Context, bytes: Option<Vec<u8>>, choice: usize) {
    let px = match PL_FONTS.get(choice).map(|f| &f.1) {
        Some(FontSrc::Web(_, px)) => *px,
        _ => 10.0, // Winamp's playlist size
    };
    PIX.with(|c| *c.borrow_mut() = bytes.and_then(|b| PixFont::new(b, px)));
}

/// Whether a skin's frames are on (lists then use the playlist font and colours).
pub(crate) fn chrome_on() -> bool {
    CHROME.with(|c| c.borrow().is_some())
}

/// Whether text should take the skin's playlist font (a skin is worn and its font is ready).
pub(crate) fn text_override() -> bool {
    chrome_on() && PIX.with(|c| c.borrow().is_some())
}

/// How wide a line comes out in the playlist font at `size`.
pub(crate) fn sans_width(text: &str, size: f32) -> f32 {
    PIX.with(|c| {
        let c = c.borrow();
        match c.as_ref() {
            Some(pf) => {
                let ppp = font::ppp();
                pf.width_at(text, pf.screen_px(size, ppp)) / ppp
            }
            None => text.chars().count() as f32 * size * 0.5,
        }
    })
}

/// One line in the playlist font, cut with "..." to `max_w`; `pos` is the left / centre / right middle point and
/// `size` about how tall it should come out. Returns how wide it came out.
pub(crate) fn sans_text(p: &egui::Painter, pos: Pos2, align: Align, text: &str, size: f32, max_w: f32, col: Color32) -> f32 {
    let drawn = PIX.with(|c| {
        let mut c = c.borrow_mut();
        let pf = c.as_mut()?;
        let ppp = font::ppp();
        let spx = pf.screen_px(size, ppp);
        let mut s = text.to_string();
        if pf.width_at(&s, spx) / ppp > max_w {
            while !s.is_empty() && pf.width_at(&format!("{}\u{2026}", s), spx) / ppp > max_w {
                s.pop();
            }
            s = format!("{}\u{2026}", s.trim_end());
        }
        let (tex, sz) = pf.line(p.ctx(), &s, spx);
        let (w, h) = (sz.x / ppp, sz.y / ppp);
        let x = match align {
            Align::Min => pos.x,
            Align::Center => pos.x - w / 2.0,
            Align::Max => pos.x - w,
        };
        let snap = |v: f32| (v * ppp).round() / ppp;
        let r = Rect::from_min_size(Pos2::new(snap(x), snap(pos.y - h / 2.0)), Vec2::new(w, h));
        p.image(tex, r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), col);
        Some(w)
    });
    drawn.unwrap_or_else(|| {
        // no font yet: plain smooth text
        let mut job = egui::text::LayoutJob::single_section(
            text.to_string(),
            egui::TextFormat { font_id: egui::FontId::proportional(size), color: col, ..Default::default() },
        );
        job.wrap = egui::text::TextWrapping {
            max_width: max_w.max(8.0),
            max_rows: 1,
            break_anywhere: true,
            overflow_character: Some('\u{2026}'),
        };
        let g = p.layout_job(job);
        let w = g.size().x;
        let x = match align {
            Align::Min => pos.x,
            Align::Center => pos.x - w / 2.0,
            Align::Max => pos.x - w,
        };
        p.galley(Pos2::new(x, pos.y - g.size().y / 2.0), g, col);
        w
    })
}

impl App {
    /// Put the chosen playlist font in place (fetching a free one first if it is not here yet).
    pub(crate) fn apply_pl_font(&mut self) {
        let skin_font = self.wa_art.as_ref().and_then(|t| t.font.clone());
        // the skin's own font, when it names one of the free fonts (SLEEK asks for Silkscreen), comes from there
        let mut choice = self.pl_font;
        if matches!(PL_FONTS.get(choice).map(|f| &f.1), Some(FontSrc::Skin)) {
            if let Some(i) = skin_font
                .as_deref()
                .and_then(|n| PL_FONTS.iter().position(|f| matches!(f.1, FontSrc::Web(..)) && f.0.eq_ignore_ascii_case(n)))
            {
                choice = i;
            }
        }
        match font_now(choice, skin_font.as_deref()) {
            Some(b) => set_playlist_font(&self.ctx, Some(b), choice),
            None if matches!(PL_FONTS.get(choice).map(|f| &f.1), Some(FontSrc::Web(..))) => {
                let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
                self.set_note("FETCHING THE FONT...");
                std::thread::spawn(move || {
                    let _ = tx.send(Msg::FontReady(choice, fetch_font(choice)));
                    ctx.request_repaint();
                });
            }
            None => set_playlist_font(&self.ctx, None, self.pl_font),
        }
    }

    /// AUTO on: keep the EQ for the song playing.
    pub(crate) fn eq_remember(&mut self) {
        if self.eq_auto {
            if let Some(id) = self.cur_track().map(|t| t.id) {
                self.eq_songs.insert(id, self.eq_gains);
            }
        }
    }

    /// The queue as a real Winamp playlist window, drawn from the worn skin: its frame with the title piece, the
    /// tracks in the playlist font and colours, its scrollbar, and the bottom bar with working buttons.
    pub(crate) fn winamp_playlist(&mut self, ui: &mut egui::Ui, outer: Rect, acts: &mut Vec<Action>) {
        let Some(t) = self.wa_art.take() else { return };
        let p = ui.painter().clone();
        let ppp = font::ppp();
        // the window's pieces and the text grow and shrink with the playlist (its width and its height; whole screen
        // pixels, so the skin stays crisp)
        // fluid (no steps), and gentle: the square root of the size change
        let fit = (outer.width() / 480.0).min(outer.height() / 220.0).max(0.1);
        let k = (1.5 * fit.sqrt()).clamp(0.8, 3.0);
        let _ = ppp;
        let id = ui.id().with("winamp_playlist");
        let piece = |src: (f32, f32, f32, f32), dst: Rect| {
            t.spr(&p, "pledit", src, dst);
        };
        let tiled = |src: (f32, f32, f32, f32), area: Rect, horizontal: bool| {
            let clip = p.with_clip_rect(area);
            let (tw, th) = (src.2 * k, src.3 * k);
            if horizontal {
                let mut x = area.min.x;
                while x < area.max.x {
                    t.spr(&clip, "pledit", src, Rect::from_min_size(Pos2::new(x, area.min.y), Vec2::new(tw, th)));
                    x += tw;
                }
            } else {
                let mut y = area.min.y;
                while y < area.max.y {
                    t.spr(&clip, "pledit", src, Rect::from_min_size(Pos2::new(area.min.x, y), Vec2::new(tw, th)));
                    y += th;
                }
            }
        };
        let (top, left, right, bottom) = (20.0 * k, 12.0 * k, 20.0 * k, 38.0 * k);
        let list = Rect::from_min_max(outer.min + Vec2::new(left, top), outer.max - Vec2::new(right, bottom));
        // the list's own background, in the skin's playlist colour
        p.rect_filled(list, 0.0, pal().lcd);

        // ---- the frame
        tiled((127.0, 0.0, 25.0, 20.0), Rect::from_min_max(outer.min, Pos2::new(outer.max.x, outer.min.y + top)), true);
        piece((0.0, 0.0, 25.0, 20.0), Rect::from_min_size(outer.min, Vec2::new(25.0, 20.0) * k));
        piece(
            (153.0, 0.0, 25.0, 20.0),
            Rect::from_min_size(Pos2::new(outer.max.x - 25.0 * k, outer.min.y), Vec2::new(25.0, 20.0) * k),
        );
        piece(
            (26.0, 0.0, 100.0, 20.0),
            Rect::from_center_size(Pos2::new(outer.center().x, outer.min.y + top / 2.0), Vec2::new(100.0, 20.0) * k),
        );
        tiled(
            (0.0, 42.0, 12.0, 29.0),
            Rect::from_min_max(Pos2::new(outer.min.x, list.min.y), Pos2::new(list.min.x, list.max.y)),
            false,
        );
        let rcol = Rect::from_min_max(Pos2::new(list.max.x, list.min.y), Pos2::new(outer.max.x, list.max.y));
        tiled((31.0, 42.0, 20.0, 29.0), rcol, false);
        let bot = Rect::from_min_max(Pos2::new(outer.min.x, list.max.y), outer.max);
        tiled((179.0, 0.0, 25.0, 38.0), bot, true);
        piece((0.0, 72.0, 125.0, 38.0), Rect::from_min_size(bot.min, Vec2::new(125.0, 38.0) * k));
        let br = Rect::from_min_size(Pos2::new(outer.max.x - 150.0 * k, bot.min.y), Vec2::new(150.0, 38.0) * k);
        piece((126.0, 72.0, 150.0, 38.0), br);

        // ---- the tracks
        let queue = self.queue.clone();
        let cur = self.cur;
        let row_h = (13.0 * k).round();
        let size = row_h * 0.78;
        let rows = ((list.height() - 4.0) / row_h).floor().max(1.0) as usize;
        let max_top = queue.len().saturating_sub(rows) as f32;
        let mut first = self.wa_pl_top.clamp(0.0, max_top);
        if self.q_scroll {
            if let Some(c) = cur {
                first = (c as f32 - rows as f32 / 2.0).clamp(0.0, max_top);
            }
            self.q_scroll = false;
        }
        let area = ui.interact(list, id.with("list"), Sense::hover());
        if area.hovered() {
            let wheel = ui.input(|i| i.smooth_scroll_delta.y);
            first = (first - wheel / row_h).clamp(0.0, max_top);
        }
        let digits = format!("{}", queue.len()).len();
        let clip = p.with_clip_rect(list);
        for (n, i) in (first.floor() as usize..queue.len()).take(rows + 1).enumerate() {
            let tr = &queue[i];
            let y = list.min.y + 2.0 + n as f32 * row_h - (first.fract() * row_h);
            let r = Rect::from_min_size(Pos2::new(list.min.x, y), Vec2::new(list.width(), row_h));
            if r.max.y > list.max.y + row_h {
                break;
            }
            let h = ui
                .interact(r.intersect(list), id.with(("row", i)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .tip("Click to play, right-click for more");
            if h.hovered() {
                clip.rect_filled(r, 0.0, pal().row_sel);
            }
            let col = if cur == Some(i) { pal().bar_txt } else { pal().ink };
            let label = format!("{:>w$}. {} - {}", i + 1, tr.artists_text(), tr.title, w = digits);
            let dur = fmt_time(tr.duration);
            let dw = sans_text(&clip, Pos2::new(r.max.x - 4.0, r.center().y), Align::Max, &dur, size, 80.0, col);
            sans_text(&clip, Pos2::new(r.min.x + 4.0, r.center().y), Align::Min, &label, size, r.width() - dw - 16.0, col);
            if h.clicked() {
                acts.push(Action::PlayIndex(i));
            }
            let tidal = self.ext_of(tr.id).is_none();
            let tr = tr.clone();
            h.context_menu(|ui| {
                goto_items(ui, acts, &tr, tidal);
                if menu_item(ui, "Remove from the queue") {
                    acts.push(Action::Remove(i));
                    ui.close_menu();
                }
            });
        }
        // the scrollbar: the skin's handle, riding the right edge
        let track = Rect::from_min_max(Pos2::new(rcol.min.x + 5.0 * k, rcol.min.y), Pos2::new(rcol.min.x + 13.0 * k, rcol.max.y));
        let hh = 18.0 * k;
        let f = if max_top > 0.0 { first / max_top } else { 0.0 };
        let handle = Rect::from_min_size(Pos2::new(track.min.x, track.min.y + f * (track.height() - hh)), Vec2::new(8.0 * k, hh));
        let sh = ui.interact(track, id.with("scroll"), Sense::click_and_drag()).on_hover_cursor(egui::CursorIcon::PointingHand);
        piece(if sh.dragged() { (61.0, 53.0, 8.0, 18.0) } else { (52.0, 53.0, 8.0, 18.0) }, handle);
        if sh.dragged() || sh.clicked() {
            if let Some(pp) = sh.interact_pointer_pos() {
                first = ((pp.y - track.min.y - hh / 2.0) / (track.height() - hh)).clamp(0.0, 1.0) * max_top;
            }
        }
        self.wa_pl_top = first;

        // ---- the bottom bar: ADD and REM work; LIST opens the lead sheet
        let bl = |x: f32| Rect::from_min_size(bot.min + Vec2::new(x, 8.0) * k, Vec2::new(22.0, 18.0) * k);
        let add = ui
            .interact(bl(14.0), id.with("add"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip("Add files");
        if add.clicked() {
            acts.push(Action::AddFiles);
        }
        let rem = ui
            .interact(bl(43.0), id.with("rem"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip("Clear the queue");
        if rem.clicked() {
            acts.push(Action::ClearQueue);
        }
        let sel = ui
            .interact(bl(72.0), id.with("sel"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip("Shuffle the queue");
        if sel.clicked() {
            acts.push(Action::QueueOp(0));
        }
        // MISC: sort the queue, and pick the playlist font
        let misc = ui
            .interact(bl(101.0), id.with("misc"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip("Sort the queue, playlist font");
        let misc_menu = id.with("misc_menu");
        if misc.clicked() {
            ui.memory_mut(|m| m.toggle_popup(misc_menu));
        }
        egui::popup::popup_below_widget(ui, misc_menu, &misc, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
            ui.set_min_width(220.0);
            let before = acts.len();
            for (name, op) in [("Sort by artist", 1u8), ("Sort by title", 2), ("Reverse", 3)] {
                if menu_item(ui, name) {
                    acts.push(Action::QueueOp(op));
                }
            }
            ui.add_space(4.0);
            para(ui, "PLAYLIST FONT", pal().ink2);
            for (i, (name, _)) in PL_FONTS.iter().enumerate() {
                if menu_item(ui, &format!("{}{}", if self.pl_font == i { "> " } else { "   " }, name)) {
                    acts.push(Action::PlFont(i));
                }
            }
            if acts.len() != before {
                ui.memory_mut(|m| m.close_popup());
            }
        });
        // the title bar's close (and shade) hide the queue
        for (name, x) in [("pl_close", 11.0), ("pl_shade", 21.0)] {
            let r = Rect::from_min_size(Pos2::new(outer.max.x - x * k, outer.min.y + 3.0 * k), Vec2::new(9.0, 9.0) * k);
            if ui
                .interact(r, id.with(name), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .tip("Hide the queue")
                .clicked()
            {
                self.queue_hidden = true;
            }
        }
        // a small visualizer in the skin's own panel for it, when the bar is wide enough
        if bot.width() >= (125.0 + 75.0 + 150.0) * k {
            let vr = Rect::from_min_size(Pos2::new(br.min.x - 75.0 * k, bot.min.y), Vec2::new(75.0, 38.0) * k);
            piece((205.0, 0.0, 75.0, 38.0), vr);
            let (bands, _, _) = self.viz_data(0);
            let inner = Rect::from_min_size(vr.min + Vec2::new(3.0, 12.0) * k, Vec2::new(72.0, 16.0) * k);
            if self.cur.is_some() && !self.stopped && t.vis.len() >= 18 && !bands.is_empty() {
                for b in 0..18usize {
                    let i = b * bands.len() / 18;
                    let h = (bands[i].clamp(0.0, 1.0) * 16.0).round() as usize;
                    for row in 0..h {
                        let topr = 15 - row;
                        p.rect_filled(
                            Rect::from_min_size(inner.min + Vec2::new(b as f32 * 4.0, topr as f32) * k, Vec2::new(3.0, 1.0) * k),
                            0.0,
                            t.vis[2 + topr],
                        );
                    }
                }
            }
        }
        let list_btn = Rect::from_min_size(br.min + Vec2::new(106.0, 8.0) * k, Vec2::new(22.0, 18.0) * k);
        let lb = ui
            .interact(list_btn, id.with("list_btn"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip("The lead sheet");
        if lb.clicked() {
            self.rtab = 1;
        }
        // running time (where we are / the whole queue) and the mini clock, in the skin's font
        let total: f32 = queue.iter().map(|t| t.duration).sum();
        let pos = self.pos();
        let text_at = |x: f32, y: f32, s: &str| {
            for (n, ch) in s.chars().enumerate() {
                let (row, col) = glyph(ch);
                t.spr(
                    &p,
                    "text",
                    (col as f32 * 5.0, row as f32 * 6.0, 5.0, 6.0),
                    Rect::from_min_size(br.min + Vec2::new(x + n as f32 * 5.0, y) * k, Vec2::new(5.0, 6.0) * k),
                );
            }
        };
        if self.cur.is_some() {
            text_at(7.0, 10.0, &format!("{}/{}", fmt_time(pos), fmt_long(total)));
            text_at(66.0, 23.0, &format!("{:>5}", fmt_time(pos)));
        }
        // the mini transport
        let acts_row = [Action::Prev, Action::PlayBtn, Action::PauseBtn, Action::StopBtn, Action::Next, Action::AddFiles];
        const MINI_TIPS: [&str; 6] = ["Previous", "Play", "Pause", "Stop", "Next", "Open files"];
        for (n, a) in acts_row.into_iter().enumerate() {
            let r = Rect::from_min_size(br.min + Vec2::new(3.0 + n as f32 * 10.0, 22.0) * k, Vec2::new(10.0, 10.0) * k);
            if ui
                .interact(r, id.with(("mini", n)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .tip(MINI_TIPS[n])
                .clicked()
            {
                acts.push(a);
            }
        }
        self.wa_art = Some(t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn skins_fonts_are_found_by_name() {
        // the names skins really give, styles included
        for name in [
            "Arial",
            "Arial Narrow",
            "Arial Bold",
            "Trebuchet MS Italic",
            "Verdana Italic",
            "Tahoma",
            "Comic Sans MS",
            "MS Sans Serif",
        ] {
            assert!(system_font(name).is_some(), "{} not found", name);
        }
        assert!(system_font("Adelaide CE").is_none(), "a font that is not installed is None (Arial stands in)");
    }

    #[cfg(windows)]
    #[test]
    fn lines_are_crisp_pixels() {
        let mut pf = PixFont::new(system_font("Arial").unwrap(), 10.0).unwrap();
        let w = pf.width("1. Kanye West - FML");
        assert!(w > 60.0 && w < 140.0, "about Winamp's width at 10 px: {}", w);
        assert!(pf.width("WWW") > pf.width("iii"), "proportional");
        assert!(pf.screen_px(20.0, 1.0) >= 20.0 && pf.screen_px(4.0, 1.0) >= 10.0, "never below its own size");
        let pixel = PixFont::new(system_font("Arial").unwrap(), 8.0).unwrap();
        assert_eq!(pixel.screen_px(15.0, 1.0), 16.0, "a pixel font comes in whole multiples of its size");
        // hinted: every upright stroke comes out the same width at every size, the cure for grainy text
        for size in [10.0, 12.0, 13.0, 15.0, 17.0, 20.0] {
            let (w, h, px) = pf.raster("lllllll", size);
            let mut widths = std::collections::BTreeSet::new();
            let y = (0..h).max_by_key(|y| (0..w).filter(|x| px[y * w + x] > 0).count()).unwrap();
            let mut run = 0;
            for x in 0..w {
                if px[y * w + x] > 0 {
                    run += 1;
                } else if run > 0 {
                    widths.insert(run);
                    run = 0;
                }
            }
            assert_eq!(widths.len(), 1, "even stems at {} px: {:?}", size, widths);
        }
    }
}
