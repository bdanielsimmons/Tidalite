//! The spectrum / waveform visualizer, drawn the same way in every skin.

use super::*;

/// Settings of one visualizer. The player's small one and the album view's big one each have their own.
#[derive(Clone, Copy)]
pub(crate) struct VizCfg {
    /// 0 bars, 1 waveform, 2 both
    pub(crate) mode: u8,
    /// bar width as a share of its slot
    pub(crate) w: f32,
    pub(crate) gain: f32,
    pub(crate) n: usize,
    /// 0 skin, 1 cover art, 2 classic green + red
    pub(crate) color: u8,
}

impl VizCfg {
    pub(crate) fn player() -> Self {
        VizCfg { mode: 0, w: 0.8, gain: 1.0, n: 20, color: 0 }
    }

    pub(crate) fn art() -> Self {
        VizCfg { mode: 0, w: 0.8, gain: 1.0, n: 32, color: 0 }
    }

    pub(crate) fn save(&self) -> serde_json::Value {
        serde_json::json!({ "mode": self.mode, "w": self.w, "gain": self.gain, "n": self.n, "color": self.color })
    }

    pub(crate) fn load(&mut self, v: &serde_json::Value) {
        if let Some(n) = v["mode"].as_u64() {
            self.mode = (n as u8).min(2);
        }
        if let Some(f) = v["w"].as_f64() {
            self.w = (f as f32).clamp(0.3, 1.0);
        }
        if let Some(f) = v["gain"].as_f64() {
            self.gain = (f as f32).clamp(0.4, 3.0);
        }
        if let Some(n) = v["n"].as_u64() {
            self.n = (n as usize).clamp(8, 96);
        }
        if let Some(n) = v["color"].as_u64() {
            self.color = (n as u8).min(2);
        }
    }

    /// The single shared set that older versions saved.
    pub(crate) fn load_old(&mut self, st: &serde_json::Value) {
        let old = serde_json::json!({
            "mode": st["viz_mode"], "w": st["viz_w"], "gain": st["viz_gain"], "n": st["viz_n"], "color": st["viz_color2"]
        });
        self.load(&old);
    }
}

/// Reshape `src` (log-spaced bands) to `n` bars, scaled by `k`: loudest of the group when shrinking,
/// smooth interpolation when growing.
pub(crate) fn resample(src: &[f32], n: usize, k: f32) -> Vec<f32> {
    let len = src.len();
    if len == 0 || n == 0 {
        return vec![0.0; n];
    }
    (0..n)
        .map(|i| {
            let v = if n <= len {
                let (a, b) = (i * len / n, ((i + 1) * len / n).max(i * len / n + 1).min(len));
                src[a..b].iter().copied().fold(0.0, f32::max)
            } else {
                let pos = ((i as f32 + 0.5) * len as f32 / n as f32 - 0.5).clamp(0.0, (len - 1) as f32);
                let (i0, fr) = (pos.floor() as usize, pos.fract());
                src[i0] * (1.0 - fr) + src[(i0 + 1).min(len - 1)] * fr
            };
            (v * k).clamp(0.0, 1.0)
        })
        .collect()
}

/// Bars and/or waveform inside `r`. `art` = soft white overlay (album view); otherwise the skin's own colours.
pub(crate) fn viz_draw(
    p: &egui::Painter,
    r: Rect,
    bands: &[f32],
    peaks: &[f32],
    wave: &[f32],
    mode: u8,
    wfrac: f32,
    art: bool,
    alpha: f32,
    tint: Option<(Color32, Color32)>,
) {
    let nb = bands.len().max(1);
    let modern = style() != 0;
    let a1 = (alpha * 255.0) as u8;
    let a2 = ((alpha * 2.0).min(0.9) * 255.0) as u8;
    let (body, cap, line) = if let Some((pri, sec)) = tint {
        if art {
            let f = |c: Color32, a: u8| Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a);
            (f(pri, a1), f(sec, a2), f(pri, a2.max(130)))
        } else {
            (pri, sec, pri)
        }
    } else if art {
        // the skin's own colours, picked to stand out against the dark cover backdrop
        let pl = pal();
        let score = |c: &Color32| {
            let (_, sat, v) = rgb_to_hsv(*c);
            sat * 0.6 + v
        };
        let lum = |c: &Color32| rgb_to_hsv(*c).2;
        let body_c = [pl.ink, pl.ink2, pl.trim, pl.btn_hi, pl.btn_face, pl.red, pl.lcd]
            .into_iter()
            .max_by(|x, y| score(x).total_cmp(&score(y)))
            .unwrap_or(pl.ink);
        let cap_c = [pl.beige_lt, pl.beige_h, pl.trim, pl.ink2, pl.btn_hi]
            .into_iter()
            .max_by(|x, y| lum(x).total_cmp(&lum(y)))
            .unwrap_or(pl.beige_lt);
        let f = |c: Color32, a: u8| Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a);
        (f(body_c, a1), f(cap_c, a2), f(body_c, a2.max(130)))
    } else if modern {
        (pal().btn_hi, pal().ink2, pal().btn_face)
    } else {
        (pal().ink, pal().ink2, pal().ink)
    };
    if mode == 0 || mode == 2 {
        let slot = r.width() / nb as f32;
        let bw = (slot * wfrac).max(1.5);
        let q = if modern {
            0.0
        } else if art {
            6.0
        } else {
            2.0
        };
        for i in 0..nb.min(peaks.len()) {
            let x = r.min.x + i as f32 * slot + (slot - bw) / 2.0;
            let mut h = bands[i] * r.height();
            let mut ph = peaks[i] * r.height();
            if q > 0.0 {
                h = (h / q).floor() * q;
                ph = (ph / q).floor() * q;
            }
            let rad = if modern { (bw * 0.35).min(3.0) } else { 0.0 };
            if h > 0.5 {
                let br = Rect::from_min_size(Pos2::new(x, r.max.y - h), Vec2::new(bw, h));
                if modern {
                    rfill(p, br, rad, body);
                } else {
                    fill_rect(p, br, body);
                }
            }
            if ph > 1.0 {
                let pr = Rect::from_min_size(
                    Pos2::new(x, r.max.y - ph - 2.0),
                    Vec2::new(bw, if modern { 2.0 } else { 1.0_f32.max(q / 2.0) }),
                );
                if modern {
                    rfill(p, pr, 1.0, cap);
                } else {
                    fill_rect(p, pr, cap);
                }
            }
        }
    }
    if mode >= 1 && wave.len() > 1 {
        let cy = r.center().y;
        let amp = r.height() * if mode == 1 { 0.46 } else { 0.32 };
        let n = wave.len();
        if modern {
            let pts: Vec<Pos2> =
                (0..n).map(|k| Pos2::new(r.min.x + r.width() * k as f32 / (n - 1) as f32, cy - wave[k] * amp)).collect();
            let w: f32 = if art { 3.0 } else { 1.5 };
            // the area under the wave is lit too, fading toward the bottom like the bars do
            let mut mesh = egui::Mesh::default();
            let uv = egui::epaint::WHITE_UV;
            let (ft, fb) = (body.linear_multiply(0.5), body.linear_multiply(0.08));
            for pt in &pts {
                mesh.vertices.push(egui::epaint::Vertex { pos: *pt, uv, color: ft });
                mesh.vertices.push(egui::epaint::Vertex { pos: Pos2::new(pt.x, r.max.y), uv, color: fb });
            }
            for k in 0..(pts.len() as u32 - 1) {
                let b = k * 2;
                mesh.indices.extend_from_slice(&[b, b + 1, b + 2, b + 1, b + 3, b + 2]);
            }
            p.add(egui::Shape::mesh(mesh));
            if style() == 1 || art {
                // soft glow under the line
                p.add(egui::Shape::line(
                    pts.clone(),
                    Stroke::new(w * 3.0, Color32::from_rgba_unmultiplied(line.r(), line.g(), line.b(), 36)),
                ));
            }
            p.add(egui::Shape::line(pts, Stroke::new(w, line)));
        } else {
            // pixel scope: 2-dot columns, joined vertically
            let step = if art { 6.0 } else { 2.0 };
            let cols = (r.width() / step).floor().max(2.0) as usize;
            let mut prev: Option<f32> = None;
            for c in 0..cols {
                let f = c as f32 / (cols - 1) as f32 * (n - 1) as f32;
                let (i0, fr) = (f.floor() as usize, f.fract());
                let v = wave[i0] * (1.0 - fr) + wave[(i0 + 1).min(n - 1)] * fr;
                let y = ((cy - v * amp) / 2.0).floor() * 2.0;
                let (lo, hi) = match prev {
                    Some(pv) => (pv.min(y), pv.max(y)),
                    None => (y, y),
                };
                let col = Color32::from_rgba_unmultiplied(line.r(), line.g(), line.b(), line.a());
                let top = y + step.min(3.0);
                if r.max.y > top {
                    fill_rect(
                        p,
                        Rect::from_min_max(
                            Pos2::new(r.min.x + c as f32 * step, top),
                            Pos2::new(r.min.x + c as f32 * step + step.min(4.0), r.max.y),
                        ),
                        body.linear_multiply(0.4),
                    );
                }
                fill_rect(
                    p,
                    Rect::from_min_size(
                        Pos2::new(r.min.x + c as f32 * step, lo),
                        Vec2::new(step.min(4.0), hi - lo + step.min(3.0)),
                    ),
                    col,
                );
                prev = Some(y);
            }
        }
    }
}
