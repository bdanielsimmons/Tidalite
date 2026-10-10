//! Smooth vector versions of the pixel icons, drawn by the modern (aero / sleek) skins.
//! `draw` recognises an icon by its pixel rows, so every pixel icon in the app is swapped at one place.
use super::*;
use egui::Shape;

const DROP: [&str; 4] = ["#######", ".#####.", "..###..", "...#..."];

struct Pen<'a> {
    p: &'a egui::Painter,
    o: Pos2,
    s: f32,
    col: Color32,
}

impl Pen<'_> {
    fn pt(&self, x: f32, y: f32) -> Pos2 {
        self.o + Vec2::new(x * self.s, y * self.s)
    }
    fn sw(&self) -> f32 {
        (self.s * 0.11).max(1.3)
    }
    fn poly(&self, pts: &[(f32, f32)]) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.pt(x, y)).collect();
        self.p.add(Shape::convex_polygon(v, self.col, Stroke::new(self.s * 0.06, self.col)));
    }
    fn line(&self, pts: &[(f32, f32)]) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.pt(x, y)).collect();
        self.p.add(Shape::line(v, Stroke::new(self.sw(), self.col)));
    }
    fn ring(&self, pts: &[(f32, f32)]) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.pt(x, y)).collect();
        self.p.add(Shape::closed_line(v, Stroke::new(self.sw(), self.col)));
    }
    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32, rad: f32) {
        rfill(self.p, Rect::from_min_max(self.pt(x0, y0), self.pt(x1, y1)), rad * self.s, self.col);
    }
    fn frame(&self, x0: f32, y0: f32, x1: f32, y1: f32, rad: f32) {
        self.p.rect_stroke(
            Rect::from_min_max(self.pt(x0, y0), self.pt(x1, y1)),
            Rounding::same(rad * self.s),
            Stroke::new(self.sw(), self.col),
        );
    }
    fn dot(&self, x: f32, y: f32, r: f32) {
        self.p.circle_filled(self.pt(x, y), r * self.s, self.col);
    }
    fn circle(&self, x: f32, y: f32, r: f32) {
        self.p.circle_stroke(self.pt(x, y), r * self.s, Stroke::new(self.sw(), self.col));
    }
    /// Points along an arc (angles in degrees, 0 = right, 90 = down).
    fn arc(&self, cx: f32, cy: f32, r: f32, a0: f32, a1: f32) -> Vec<(f32, f32)> {
        (0..=12)
            .map(|i| {
                let a = (a0 + (a1 - a0) * i as f32 / 12.0).to_radians();
                (cx + r * a.cos(), cy + r * a.sin())
            })
            .collect()
    }
}

/// Draw a smooth version of `rows` inside `r`. False when the icon has no vector form.
pub fn draw(p: &egui::Painter, r: Rect, rows: &[&str], col: Color32) -> bool {
    // the drawing's square fits inside the icon's box (a wide icon such as the speaker would spill out otherwise)
    let s = r.width().max(r.height()).min(r.width().min(r.height()) * 1.15);
    let pen = Pen { p, o: r.center() - Vec2::splat(s / 2.0), s, col };
    let is = |a: &[&str]| rows == a;
    if is(&IC_PLAY) || is(&PLAY_S) {
        pen.poly(&[(0.22, 0.1), (0.9, 0.5), (0.22, 0.9)]);
    } else if is(&IC_PAUSE) || is(&PAUSE_S) {
        pen.rect(0.18, 0.12, 0.42, 0.88, 0.08);
        pen.rect(0.58, 0.12, 0.82, 0.88, 0.08);
    } else if is(&STOP) || is(&STOP_S) {
        pen.rect(0.16, 0.16, 0.84, 0.84, 0.12);
    } else if is(&IC_PREV) {
        pen.rect(0.16, 0.16, 0.28, 0.84, 0.05);
        pen.poly(&[(0.86, 0.16), (0.86, 0.84), (0.36, 0.5)]);
    } else if is(&IC_NEXT) {
        pen.rect(0.72, 0.16, 0.84, 0.84, 0.05);
        pen.poly(&[(0.14, 0.16), (0.14, 0.84), (0.64, 0.5)]);
    } else if is(&IC_REW) {
        pen.poly(&[(0.5, 0.18), (0.5, 0.82), (0.06, 0.5)]);
        pen.poly(&[(0.94, 0.18), (0.94, 0.82), (0.5, 0.5)]);
    } else if is(&IC_HEART) || is(&HEART) {
        pen.dot(0.3, 0.33, 0.23);
        pen.dot(0.7, 0.33, 0.23);
        pen.poly(&[(0.09, 0.42), (0.91, 0.42), (0.5, 0.92)]);
    } else if is(&CHECK) {
        pen.line(&[(0.12, 0.55), (0.4, 0.84), (0.9, 0.18)]);
    } else if is(&IC_X) {
        pen.line(&[(0.2, 0.2), (0.8, 0.8)]);
        pen.line(&[(0.8, 0.2), (0.2, 0.8)]);
    } else if is(&IC_FULL) {
        for (sx, sy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
            let (x, y) = (0.14 + sx * 0.72, 0.14 + sy * 0.72);
            let (dx, dy) = (if sx == 0.0 { 0.24 } else { -0.24 }, if sy == 0.0 { 0.24 } else { -0.24 });
            pen.line(&[(x + dx, y), (x, y), (x, y + dy)]);
        }
    } else if is(&IC_WIN) {
        for (sx, sy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
            let (x, y) = (0.3 + sx * 0.4, 0.3 + sy * 0.4);
            let (dx, dy) = (if sx == 0.0 { -0.2 } else { 0.2 }, if sy == 0.0 { -0.2 } else { 0.2 });
            pen.line(&[(x + dx, y), (x, y), (x, y + dy)]);
        }
    } else if is(&IC_MIC) {
        pen.rect(0.36, 0.06, 0.64, 0.56, 0.14);
        pen.line(&pen.arc(0.5, 0.45, 0.28, 0.0, 180.0));
        pen.line(&[(0.5, 0.74), (0.5, 0.9)]);
        pen.line(&[(0.34, 0.92), (0.66, 0.92)]);
    } else if is(&IC_SPEC) {
        for (i, h) in [0.4, 0.7, 0.95, 0.55, 0.8].iter().enumerate() {
            let x = 0.08 + i as f32 * 0.19;
            pen.rect(x, 0.92 - h * 0.84, x + 0.13, 0.92, 0.05);
        }
    } else if is(&IC_SHUF) {
        pen.line(&[(0.06, 0.3), (0.34, 0.3), (0.64, 0.7), (0.82, 0.7)]);
        pen.line(&[(0.06, 0.7), (0.34, 0.7), (0.64, 0.3), (0.82, 0.3)]);
        pen.poly(&[(0.8, 0.18), (0.96, 0.3), (0.8, 0.42)]);
        pen.poly(&[(0.8, 0.58), (0.96, 0.7), (0.8, 0.82)]);
    } else if is(&IC_REP) || is(&IC_REP1) {
        pen.frame(0.1, 0.3, 0.9, 0.72, 0.2);
        pen.poly(&[(0.6, 0.14), (0.6, 0.46), (0.84, 0.3)]);
        if is(&IC_REP1) {
            pen.line(&[(0.44, 0.44), (0.5, 0.4), (0.5, 0.62)]);
        }
    } else if is(&IC_HOME) {
        pen.poly(&[(0.5, 0.08), (0.94, 0.5), (0.06, 0.5)]);
        pen.rect(0.2, 0.5, 0.8, 0.9, 0.05);
    } else if is(&IC_BACK) {
        pen.line(&[(0.86, 0.5), (0.16, 0.5)]);
        pen.line(&[(0.42, 0.24), (0.16, 0.5), (0.42, 0.76)]);
    } else if is(&IC_FWD) {
        pen.line(&[(0.14, 0.5), (0.84, 0.5)]);
        pen.line(&[(0.58, 0.24), (0.84, 0.5), (0.58, 0.76)]);
    } else if is(&IC_SEARCH) {
        pen.circle(0.42, 0.42, 0.28);
        pen.line(&[(0.63, 0.63), (0.9, 0.9)]);
    } else if is(&IC_CLOCK) {
        pen.circle(0.5, 0.5, 0.4);
        pen.line(&[(0.5, 0.5), (0.5, 0.24)]);
        pen.line(&[(0.5, 0.5), (0.7, 0.62)]);
    } else if is(&IC_METRO) {
        pen.ring(&[(0.2, 0.92), (0.38, 0.1), (0.62, 0.1), (0.8, 0.92)]);
        pen.line(&[(0.5, 0.78), (0.66, 0.28)]);
    } else if is(&IC_EQ) {
        for (x, k) in [(0.2, 0.68), (0.5, 0.3), (0.8, 0.54)] {
            pen.line(&[(x, 0.08), (x, 0.92)]);
            pen.dot(x, k, 0.12);
        }
    } else if is(&IC_HELP) {
        pen.circle(0.5, 0.5, 0.4);
        p.text(pen.pt(0.5, 0.52), egui::Align2::CENTER_CENTER, "?", egui::FontId::proportional(s * 0.62), col);
    } else if is(&IC_SKIN) {
        pen.rect(0.1, 0.1, 0.46, 0.46, 0.1);
        pen.rect(0.54, 0.1, 0.9, 0.46, 0.1);
        pen.rect(0.1, 0.54, 0.46, 0.9, 0.1);
        pen.rect(0.54, 0.54, 0.9, 0.9, 0.1);
    } else if is(&IC_SPK0) || is(&IC_SPK1) || is(&IC_SPK2) {
        pen.rect(0.06, 0.38, 0.28, 0.62, 0.04);
        pen.poly(&[(0.28, 0.38), (0.52, 0.14), (0.52, 0.86), (0.28, 0.62)]);
        if is(&IC_SPK0) {
            pen.line(&[(0.66, 0.38), (0.9, 0.62)]);
            pen.line(&[(0.9, 0.38), (0.66, 0.62)]);
        } else {
            pen.line(&pen.arc(0.52, 0.5, 0.2, -50.0, 50.0));
            if is(&IC_SPK2) {
                pen.line(&pen.arc(0.52, 0.5, 0.38, -50.0, 50.0));
            }
        }
    } else if is(&IC_TOMATO) {
        pen.dot(0.5, 0.58, 0.36);
        pen.line(&[(0.5, 0.2), (0.5, 0.08)]);
        pen.line(&[(0.5, 0.2), (0.7, 0.1)]);
    } else if is(&IC_DISK) {
        pen.rect(0.1, 0.1, 0.9, 0.9, 0.1);
        pen.rect(0.28, 0.1, 0.72, 0.38, 0.03);
    } else if is(&IC_BOOK) {
        pen.rect(0.2, 0.08, 0.84, 0.92, 0.08);
    } else if is(&IC_MOON) {
        pen.dot(0.5, 0.5, 0.4);
        // bite out of the disc, using the panel behind it
        p.circle_filled(pen.pt(0.68, 0.36), 0.33 * s, pal().beige);
    } else if is(&WIN_LIBRARY) {
        pen.rect(0.1, 0.12, 0.3, 0.86, 0.05);
        pen.rect(0.4, 0.28, 0.6, 0.86, 0.05);
        pen.rect(0.7, 0.18, 0.9, 0.86, 0.05);
    } else if is(&WIN_QUEUE) {
        for y in [0.2, 0.5, 0.8] {
            pen.dot(0.14, y, 0.08);
            pen.rect(0.32, y - 0.07, 0.92, y + 0.07, 0.07);
        }
    } else if is(&WIN_SHEET) {
        pen.ring(&[(0.2, 0.08), (0.6, 0.08), (0.82, 0.3), (0.82, 0.92), (0.2, 0.92)]);
        pen.line(&[(0.34, 0.5), (0.68, 0.5)]);
        pen.line(&[(0.34, 0.7), (0.68, 0.7)]);
    } else if is(&WIN_PRACTICE) {
        pen.circle(0.5, 0.5, 0.4);
        pen.dot(0.5, 0.5, 0.14);
    } else if is(&MARK) {
        gem(p, r, col, pal().ink2);
    } else if is(&IC_OUT) {
        pen.frame(0.24, 0.04, 0.76, 0.96, 0.08);
        pen.dot(0.5, 0.24, 0.06);
        pen.circle(0.5, 0.62, 0.17);
        pen.dot(0.5, 0.62, 0.05);
    } else if is(&IC_MENU) {
        for y in [0.18, 0.44, 0.7] {
            pen.rect(0.1, y, 0.9, y + 0.12, 0.06);
        }
    } else if is(&DROP) {
        pen.poly(&[(0.08, 0.3), (0.92, 0.3), (0.5, 0.78)]);
    } else {
        return false;
    }
    true
}

/// The Backline gem: a faceted diamond with a tide line through it.
pub fn gem(p: &egui::Painter, r: Rect, a: Color32, b: Color32) {
    let s = r.width().max(r.height());
    let o = r.center() - Vec2::splat(s / 2.0);
    let pt = |x: f32, y: f32| o + Vec2::new(x * s, y * s);
    let diamond = vec![pt(0.5, 0.02), pt(0.97, 0.5), pt(0.5, 0.98), pt(0.03, 0.5)];
    p.add(Shape::convex_polygon(diamond, a, Stroke::new(s * 0.05, a)));
    // lighter upper-left facet
    p.add(Shape::convex_polygon(vec![pt(0.5, 0.12), pt(0.5, 0.5), pt(0.14, 0.5)], b, Stroke::NONE));
    let wave: Vec<Pos2> = (0..=16)
        .map(|i| {
            let t = i as f32 / 16.0;
            pt(0.1 + 0.8 * t, 0.52 + 0.1 * (t * std::f32::consts::TAU * 1.5).sin())
        })
        .collect();
    p.add(Shape::line(wave, Stroke::new((s * 0.07).max(1.2), b)));
}
