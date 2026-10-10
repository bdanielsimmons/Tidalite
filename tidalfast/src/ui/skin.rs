//! Skins: the eleven palettes, which look each one belongs to, and the colour helpers the whole UI draws with.

use super::*;

// ------------------------------------------------------------------- skins
#[derive(Clone, Copy)]
pub(crate) struct Pal {
    pub(crate) app_bg: Color32,
    pub(crate) trim: Color32,
    pub(crate) beige: Color32,
    pub(crate) beige_lt: Color32,
    pub(crate) beige_dk: Color32,
    pub(crate) beige_h: Color32,
    pub(crate) lcd: Color32,
    pub(crate) lcd_ghost: Color32,
    pub(crate) groove: Color32,
    pub(crate) sel: Color32,
    pub(crate) ink: Color32,
    pub(crate) ink2: Color32,
    pub(crate) dim: Color32,
    pub(crate) red: Color32,
    pub(crate) btn_face: Color32,
    pub(crate) btn_hi: Color32,
    pub(crate) row_alt: Color32,
    pub(crate) row_sel: Color32,
    pub(crate) bar_txt: Color32,
    pub(crate) edge: Color32,
}

pub(crate) const fn c(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

pub(crate) const SKIN_NAMES: [&str; 11] =
    ["OLIVE", "AQUA", "DARK", "AMBER", "PAPER", "PLUM", "SLATE", "CLAY", "AERO GLASS", "SLEEK DARK", "SLEEK LIGHT"];

pub(crate) static PALS: [Pal; 11] = [
    // OLIVE: the classic beige/olive player
    Pal {
        app_bg: c(8, 8, 10),
        trim: c(186, 184, 136),
        beige: c(201, 199, 160),
        beige_lt: c(226, 224, 188),
        beige_dk: c(160, 157, 118),
        beige_h: c(213, 211, 174),
        lcd: c(212, 210, 172),
        lcd_ghost: c(190, 188, 150),
        groove: c(120, 118, 84),
        sel: c(184, 181, 142),
        ink: c(12, 12, 8),
        ink2: c(55, 55, 38),
        dim: c(80, 78, 58),
        red: c(150, 30, 20),
        btn_face: c(150, 146, 92),
        btn_hi: c(190, 186, 128),
        row_alt: c(203, 201, 163),
        row_sel: c(70, 70, 50),
        bar_txt: c(226, 224, 188),
        edge: c(12, 12, 8),
    },
    // AQUA: glassy sky blue
    Pal {
        app_bg: c(4, 16, 28),
        trim: c(150, 215, 235),
        beige: c(170, 210, 228),
        beige_lt: c(226, 246, 253),
        beige_dk: c(110, 160, 186),
        beige_h: c(190, 226, 240),
        lcd: c(200, 236, 238),
        lcd_ghost: c(176, 214, 218),
        groove: c(92, 138, 162),
        sel: c(150, 200, 222),
        ink: c(6, 24, 34),
        ink2: c(24, 62, 81),
        dim: c(55, 87, 101),
        red: c(153, 34, 34),
        btn_face: c(70, 150, 190),
        btn_hi: c(110, 190, 225),
        row_alt: c(188, 224, 230),
        row_sel: c(22, 70, 100),
        bar_txt: c(226, 246, 252),
        edge: c(2, 12, 20),
    },
    // DARK: graphite with green phosphor
    Pal {
        app_bg: c(6, 7, 8),
        trim: c(122, 127, 133),
        beige: c(62, 65, 70),
        beige_lt: c(96, 100, 106),
        beige_dk: c(38, 40, 44),
        beige_h: c(76, 80, 86),
        lcd: c(16, 24, 18),
        lcd_ghost: c(24, 36, 27),
        groove: c(30, 32, 35),
        sel: c(50, 66, 54),
        ink: c(126, 255, 150),
        ink2: c(136, 235, 156),
        dim: c(136, 191, 148),
        red: c(255, 152, 141),
        btn_face: c(88, 92, 98),
        btn_hi: c(118, 122, 130),
        row_alt: c(20, 30, 22),
        row_sel: c(36, 100, 56),
        bar_txt: c(252, 254, 252),
        edge: c(0, 0, 0),
    },
    // AMBER: bronze with amber phosphor
    Pal {
        app_bg: c(8, 6, 4),
        trim: c(154, 118, 68),
        beige: c(74, 62, 46),
        beige_lt: c(112, 94, 68),
        beige_dk: c(44, 36, 26),
        beige_h: c(88, 74, 54),
        lcd: c(24, 16, 6),
        lcd_ghost: c(38, 26, 10),
        groove: c(36, 28, 18),
        sel: c(70, 52, 28),
        ink: c(255, 184, 64),
        ink2: c(246, 177, 74),
        dim: c(215, 169, 96),
        red: c(255, 149, 132),
        btn_face: c(110, 92, 60),
        btn_hi: c(140, 118, 80),
        row_alt: c(32, 22, 10),
        row_sel: c(120, 76, 16),
        bar_txt: c(255, 249, 236),
        edge: c(0, 0, 0),
    },
    // PAPER: warm white with ink-blue type
    Pal {
        app_bg: c(40, 38, 34),
        trim: c(142, 135, 123),
        beige: c(236, 230, 214),
        beige_lt: c(252, 249, 240),
        beige_dk: c(196, 188, 168),
        beige_h: c(244, 239, 226),
        lcd: c(250, 247, 238),
        lcd_ghost: c(234, 229, 214),
        groove: c(150, 142, 124),
        sel: c(214, 206, 186),
        ink: c(24, 34, 72),
        ink2: c(60, 73, 114),
        dim: c(96, 97, 112),
        red: c(176, 36, 36),
        btn_face: c(120, 134, 176),
        btn_hi: c(150, 164, 204),
        row_alt: c(240, 235, 221),
        row_sel: c(40, 56, 110),
        bar_txt: c(246, 244, 236),
        edge: c(24, 22, 20),
    },
    // PLUM: dusky purple with pink phosphor
    Pal {
        app_bg: c(10, 6, 14),
        trim: c(150, 110, 170),
        beige: c(82, 62, 96),
        beige_lt: c(124, 98, 140),
        beige_dk: c(50, 36, 62),
        beige_h: c(98, 76, 114),
        lcd: c(28, 16, 34),
        lcd_ghost: c(40, 26, 48),
        groove: c(44, 30, 54),
        sel: c(84, 54, 100),
        ink: c(255, 221, 242),
        ink2: c(246, 212, 232),
        dim: c(203, 178, 194),
        red: c(255, 161, 154),
        btn_face: c(132, 96, 150),
        btn_hi: c(160, 124, 180),
        row_alt: c(36, 22, 44),
        row_sel: c(130, 50, 100),
        bar_txt: c(255, 233, 245),
        edge: c(2, 0, 4),
    },
    // SLATE: cool grey-blue steel
    Pal {
        app_bg: c(8, 10, 14),
        trim: c(140, 156, 176),
        beige: c(168, 182, 200),
        beige_lt: c(196, 208, 222),
        beige_dk: c(120, 134, 154),
        beige_h: c(182, 196, 212),
        lcd: c(204, 214, 225),
        lcd_ghost: c(168, 182, 196),
        groove: c(86, 100, 120),
        sel: c(146, 162, 184),
        ink: c(10, 18, 30),
        ink2: c(29, 41, 61),
        dim: c(54, 66, 86),
        red: c(131, 24, 24),
        btn_face: c(96, 112, 136),
        btn_hi: c(130, 148, 172),
        row_alt: c(178, 190, 206),
        row_sel: c(30, 52, 88),
        bar_txt: c(230, 238, 246),
        edge: c(4, 8, 14),
    },
    // CLAY: terracotta and sand
    Pal {
        app_bg: c(12, 7, 5),
        trim: c(196, 130, 92),
        beige: c(222, 178, 140),
        beige_lt: c(244, 214, 182),
        beige_dk: c(176, 128, 96),
        beige_h: c(232, 192, 156),
        lcd: c(240, 212, 180),
        lcd_ghost: c(224, 192, 158),
        groove: c(142, 98, 70),
        sel: c(204, 152, 116),
        ink: c(13, 4, 2),
        ink2: c(77, 32, 21),
        dim: c(93, 64, 48),
        red: c(139, 20, 13),
        btn_face: c(186, 98, 62),
        btn_hi: c(214, 128, 88),
        row_alt: c(228, 188, 152),
        row_sel: c(120, 44, 24),
        bar_txt: c(250, 228, 204),
        edge: c(20, 8, 4),
    },
    // AERO GLASS: sky blue, glossy glass and grass green (Windows XP / Vista era)
    Pal {
        app_bg: c(18, 60, 110),
        trim: c(185, 224, 245),
        beige: c(226, 240, 252),
        beige_lt: c(252, 254, 255),
        beige_dk: c(150, 186, 218),
        beige_h: c(240, 248, 255),
        lcd: c(238, 250, 255),
        lcd_ghost: c(214, 236, 248),
        groove: c(150, 184, 212),
        sel: c(188, 222, 246),
        ink: c(14, 40, 72),
        ink2: c(38, 81, 119),
        dim: c(77, 106, 133),
        red: c(189, 52, 43),
        btn_face: c(46, 150, 210),
        btn_hi: c(96, 206, 120),
        row_alt: c(232, 244, 254),
        row_sel: c(34, 124, 196),
        bar_txt: c(255, 255, 255),
        edge: c(36, 92, 142),
    },
    // SLEEK DARK: charcoal with one blue accent
    Pal {
        app_bg: c(12, 13, 17),
        trim: c(140, 148, 168),
        beige: c(28, 30, 37),
        beige_lt: c(52, 56, 68),
        beige_dk: c(20, 21, 27),
        beige_h: c(42, 45, 55),
        lcd: c(18, 19, 25),
        lcd_ghost: c(26, 28, 36),
        groove: c(44, 47, 58),
        sel: c(40, 52, 84),
        ink: c(236, 238, 245),
        ink2: c(168, 173, 187),
        dim: c(142, 146, 158),
        red: c(255, 92, 102),
        btn_face: c(70, 122, 255),
        btn_hi: c(120, 160, 255),
        row_alt: c(32, 34, 42),
        row_sel: c(50, 94, 220),
        bar_txt: c(255, 255, 255),
        edge: c(10, 10, 14),
    },
    // SLEEK LIGHT: soft white with the same blue accent
    Pal {
        app_bg: c(226, 229, 237),
        trim: c(206, 212, 228),
        beige: c(248, 249, 252),
        beige_lt: c(255, 255, 255),
        beige_dk: c(212, 216, 228),
        beige_h: c(238, 241, 249),
        lcd: c(238, 241, 247),
        lcd_ghost: c(226, 230, 240),
        groove: c(206, 211, 224),
        sel: c(216, 227, 252),
        ink: c(22, 25, 34),
        ink2: c(74, 81, 96),
        dim: c(97, 102, 113),
        red: c(194, 50, 61),
        btn_face: c(48, 100, 240),
        btn_hi: c(92, 142, 255),
        row_alt: c(242, 244, 250),
        row_sel: c(48, 100, 240),
        bar_txt: c(255, 255, 255),
        edge: c(44, 50, 70),
    },
];

pub(crate) static SKIN: AtomicUsize = AtomicUsize::new(0);

/// A palette made from a Winamp skin (winamp.rs), worn instead of the built-in ones while set. Each one is
/// leaked on purpose (a few bytes per skin picked), so `pal()` can hand out a plain reference.
static CUSTOM: std::sync::atomic::AtomicPtr<Pal> = std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

pub(crate) fn set_custom(p: Option<Pal>) {
    let ptr = p.map_or(std::ptr::null_mut(), |p| Box::into_raw(Box::new(p)));
    CUSTOM.store(ptr, Ordering::Release);
}

pub(crate) fn custom_on() -> bool {
    !CUSTOM.load(Ordering::Acquire).is_null()
}

/// Look of the skin: 0 retro pixel, 1 aero glass, 2 sleek.
pub(crate) fn style() -> u8 {
    if custom_on() {
        return 0; // Winamp skins are drawn the retro way
    }
    match SKIN.load(Ordering::Relaxed) % PALS.len() {
        8 => 1,
        9 | 10 => 2,
        _ => 0,
    }
}

/// Rounded fill (modern skins).
pub(crate) fn rfill(p: &egui::Painter, r: Rect, rad: f32, c: Color32) {
    p.rect_filled(r, Rounding::same(rad), c);
}

pub(crate) fn rline(p: &egui::Painter, r: Rect, rad: f32, w: f32, c: Color32) {
    p.rect_stroke(r, Rounding::same(rad), Stroke::new(w, c));
}

/// Vertical gradient inside a rounded rectangle, drawn as thin horizontal slices clipped by the corner radius.
pub(crate) fn rgrad(p: &egui::Painter, r: Rect, rad: f32, top: Color32, bot: Color32) {
    let n = (r.height() / 2.0).ceil().max(1.0) as usize;
    let lerp = |a: u8, b: u8, t: f32| (a as f32 + (b as f32 - a as f32) * t) as u8;
    for i in 0..n {
        let t = i as f32 / (n.max(2) - 1) as f32;
        let y0 = r.min.y + r.height() * i as f32 / n as f32;
        let y1 = r.min.y + r.height() * (i + 1) as f32 / n as f32;
        let col = Color32::from_rgb(lerp(top.r(), bot.r(), t), lerp(top.g(), bot.g(), t), lerp(top.b(), bot.b(), t));
        // inset each slice near the corners so the gradient keeps the rounded outline
        let dy = (y0 + y1) * 0.5 - r.min.y;
        let from_edge = dy.min(r.height() - dy);
        let k = if from_edge < rad { rad - (rad * rad - (rad - from_edge) * (rad - from_edge)).max(0.0).sqrt() } else { 0.0 };
        p.rect_filled(Rect::from_min_max(Pos2::new(r.min.x + k, y0), Pos2::new(r.max.x - k, y1 + 0.5)), Rounding::ZERO, col);
    }
}

pub(crate) fn rgb_to_hsv(c: Color32) -> (f32, f32, f32) {
    let (r, g, b) = (c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let d = mx - mn;
    let h = if d == 0.0 {
        0.0
    } else if mx == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if mx == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    (h, if mx == 0.0 { 0.0 } else { d / mx }, mx)
}

pub(crate) fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color32 {
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - f * s), v * (1.0 - (1.0 - f) * s));
    let (r, g, b) = match (i as i32).rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

pub(crate) fn pal() -> &'static Pal {
    let c = CUSTOM.load(Ordering::Acquire);
    if !c.is_null() {
        // SAFETY: set_custom only stores pointers from Box::into_raw that are never freed
        return unsafe { &*c };
    }
    &PALS[SKIN.load(Ordering::Relaxed) % PALS.len()]
}
