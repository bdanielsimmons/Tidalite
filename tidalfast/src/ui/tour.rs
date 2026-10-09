//! The guided tour: a dimmed screen with the part being explained left bright, and a small card with
//! BACK / NEXT. One short tour for everyone, and a separate short one for the practice tools.

use crate::{pal, para, retro_btn, title_line, App, PRACTICE};
use eframe::egui::{self, Color32, Pos2, Rect};
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static RECTS: RefCell<HashMap<String, Rect>> = RefCell::new(HashMap::new());
}

/// Called at the start of every frame: windows that are not drawn this frame are not highlighted.
pub fn begin_frame() {
    RECTS.with(|r| r.borrow_mut().clear());
}

/// A window reports where it is (keyed by the first word of its title).
pub fn note(title: &str, r: Rect) {
    let key = title.split(' ').next().unwrap_or("").to_string();
    RECTS.with(|m| m.borrow_mut().insert(key, r));
}

fn find(key: &str) -> Option<Rect> {
    RECTS.with(|m| m.borrow().get(key).copied())
}

struct Step {
    /// window to light up ("" = none, the card sits in the middle)
    key: &'static str,
    title: &'static str,
    text: &'static str,
}

const BASIC: &[Step] = &[
    Step {
        key: "",
        title: "WELCOME",
        text: "A one-minute look at how Tidalite is laid out. Use NEXT and BACK, or press ESC to stop any time. You can come back to this from the ? button.",
    },
    Step {
        key: "LIBRARY",
        title: "THE LIBRARY",
        text: "Everything you can play lives here. The tabs along the top switch the source: Tidal, SoundCloud, your files, YouTube, and your own LISTS. Search at the top. Click a song to play it, right-click for more.",
    },
    Step {
        key: "PLAYER",
        title: "THE PLAYER",
        text: "Play, skip, seek and volume. The little screen is a visualizer - click it to change style, right-click for its settings. Click the cover art to open the big album view.",
    },
    Step {
        key: "QUEUE",
        title: "THE QUEUE",
        text: "What plays next. Drag a row to reorder it, right-click to remove or add to a playlist. Shuffle and repeat are on the player.",
    },
    Step {
        key: "",
        title: "THE ALBUM VIEW",
        text: "Press A for the big view: large cover, lyrics you can scroll and click to jump, and a visualizer behind it. The buttons under the cover turn on lyrics, the spectrum, black-and-white art, and change the skin.",
    },
    Step {
        key: "",
        title: "PLAYLISTS AND LINKS",
        text: "Right-click any song, anywhere: Add to a playlist builds your own lists in the LISTS tab (mix any sources). Copy link gives you the song's Tidal, SoundCloud or YouTube link.",
    },
    Step {
        key: "",
        title: "MAKE IT LOOK RIGHT",
        text: "The squares button changes the skin - click for the next one or right-click to pick. There are retro skins, a glassy one and two sleek ones.",
    },
    Step {
        key: "",
        title: "THAT'S IT",
        text: "Updates install by themselves. The ? button has the full list of keys and tips. Nothing you do here is reported to Tidal.",
    },
];

const PRACTICE_TOUR: &[Step] = &[
    Step {
        key: "",
        title: "PRACTICE MODE",
        text: "Tools for learning music from recordings: loop a hard spot, slow it down, fix the pitch, keep time. Turn it on with the PRACTICE chip or the P key. You can ignore all of it when you just want to listen.",
    },
    Step {
        key: "PRACTICE",
        title: "THE PRACTICE PANEL",
        text: "This panel appears in practice mode. It holds the loop, speed, pitch, metronome and timer tools. If you don't see it, switch practice mode on first.",
    },
    Step {
        key: "",
        title: "1. LOOP A SPOT",
        text: "Press [ where the hard part starts and ] where it ends, then \\ to loop it. Or right-click the seek bar. Save a loop with a name to get back to it.",
    },
    Step {
        key: "",
        title: "2. SLOW IT DOWN",
        text: "UP and DOWN arrows change the speed 5 percent at a time without changing the pitch. A good routine: loop it, play it clean at a slow speed, then step up.",
    },
    Step {
        key: "",
        title: "3. FIX THE PITCH",
        text: "PITCH moves the key in semitones or in 10-cent steps. FIND TUNING measures a track that isn't at A440 and TUNE TO A440 lines it up with your instrument.",
    },
    Step {
        key: "",
        title: "4. KEEP TIME",
        text: "The metronome has a count-in and can lock to the track's own beat. The focus timer runs work blocks with rests between them.",
    },
    Step {
        key: "",
        title: "5. KEEP TRACK",
        text: "TUNES holds your repertoire and recordings, charts hold lead sheets the band can play, and the DIARY shows what you practiced. Time only counts while a loop plays in practice mode.",
    },
];

fn steps(kind: u8) -> &'static [Step] {
    if kind == 1 && PRACTICE {
        PRACTICE_TOUR
    } else {
        BASIC
    }
}

impl App {
    pub(crate) fn tour_start(&mut self, kind: u8) {
        self.show_help = false;
        self.tour = Some((kind, 0));
        self.tour_seen = true;
        self.dirty = true;
    }

    pub(crate) fn tour_overlay(&mut self, ctx: &egui::Context) {
        let Some((kind, i)) = self.tour else { return };
        let list = steps(kind);
        let i = i.min(list.len() - 1);
        let step = &list[i];
        let screen = ctx.screen_rect();
        let target = if step.key.is_empty() { None } else { find(step.key) };
        // dim everything except the highlighted window
        let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("tour_dim")));
        let dim = Color32::from_black_alpha(165);
        match target {
            Some(t) => {
                let t = t.expand(4.0).intersect(screen);
                p.rect_filled(Rect::from_min_max(screen.min, Pos2::new(screen.max.x, t.min.y)), 0.0, dim);
                p.rect_filled(Rect::from_min_max(Pos2::new(screen.min.x, t.max.y), screen.max), 0.0, dim);
                p.rect_filled(Rect::from_min_max(Pos2::new(screen.min.x, t.min.y), Pos2::new(t.min.x, t.max.y)), 0.0, dim);
                p.rect_filled(Rect::from_min_max(Pos2::new(t.max.x, t.min.y), Pos2::new(screen.max.x, t.max.y)), 0.0, dim);
                p.rect_stroke(t, 4.0, egui::Stroke::new(3.0_f32, pal().bar_txt));
            }
            None => {
                p.rect_filled(screen, 0.0, dim);
            }
        }
        // card at the bottom, or at the top when the highlighted window is down there
        let low = target.map_or(false, |t| t.center().y > screen.center().y);
        let (align, off) =
            if low { (egui::Align2::CENTER_TOP, [0.0, 24.0]) } else { (egui::Align2::CENTER_BOTTOM, [0.0, -24.0]) };
        let mut go: i32 = 0;
        egui::Area::new(egui::Id::new("tour_card")).order(egui::Order::Tooltip).anchor(align, off).show(ctx, |ui| {
            let frame = egui::Frame::none()
                .fill(pal().beige)
                .stroke(egui::Stroke::new(2.0_f32, pal().edge))
                .rounding(if crate::style() != 0 { 8.0 } else { 0.0 })
                .inner_margin(14.0);
            frame.show(ui, |ui| {
                ui.set_width(460.0_f32.min(screen.width() - 60.0));
                title_line(ui, &format!("{}   {}/{}", step.title, i + 1, list.len()), 2.5, pal().ink);
                para(ui, step.text, pal().ink);
                ui.horizontal(|ui| {
                    if i > 0 && retro_btn(ui, "BACK", false).clicked() {
                        go = -1;
                    }
                    if retro_btn(ui, if i + 1 == list.len() { "DONE" } else { "NEXT" }, true).clicked() {
                        go = 1;
                    }
                    if i + 1 < list.len() && retro_btn(ui, "SKIP", false).clicked() {
                        go = 2;
                    }
                });
            });
        });
        if ctx.input(|k| k.key_pressed(egui::Key::Escape)) {
            go = 2;
        }
        if ctx.input(|k| k.key_pressed(egui::Key::ArrowRight)) {
            go = 1;
        }
        if ctx.input(|k| k.key_pressed(egui::Key::ArrowLeft)) && i > 0 {
            go = -1;
        }
        match go {
            1 if i + 1 < list.len() => self.tour = Some((kind, i + 1)),
            -1 => self.tour = Some((kind, i - 1)),
            0 => {}
            _ => self.tour = None,
        }
        ctx.request_repaint();
    }
}
