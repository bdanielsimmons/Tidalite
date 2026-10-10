//! The guided tour: a dimmed screen with the part being explained left bright, and a small card with
//! BACK / NEXT. One short tour for everyone, and a separate short one for the practice tools.
//! A step can light up a whole window or a single control, and can set the scene first (open the album view,
//! switch practice mode on, open MORE); whatever was showing before the tour comes back when it ends.

use crate::{pal, para, retro_btn, title_line, Action, App, PRACTICE};
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
    mark(title.split(' ').next().unwrap_or(""), r);
}

/// A control reports where it is, so a tour step can point right at it.
pub fn mark(key: &str, r: Rect) {
    RECTS.with(|m| m.borrow_mut().insert(key.to_string(), r));
}

fn find(key: &str) -> Option<Rect> {
    RECTS.with(|m| m.borrow().get(key).copied())
}

/// What should be on screen for a step.
#[derive(Clone, Copy, PartialEq)]
enum Scene {
    /// the normal windows, practice mode as it was
    Normal,
    /// the big album view
    Album,
    /// practice mode off (to show where it is switched on)
    PracticeOff,
    Practice,
    /// practice mode with MORE open on PITCH & EAR
    PracticePitch,
}

struct Step {
    /// window or control to light up ("" = none, the card sits in the middle)
    key: &'static str,
    scene: Scene,
    title: &'static str,
    text: &'static str,
}

const BASIC: &[Step] = &[
    Step {
        key: "",
        scene: Scene::Normal,
        title: "WELCOME",
        text: "A one-minute look at how Tidalite is laid out. Use NEXT and BACK (or the arrow keys), or press ESC to stop any time. You can take this again from the ? button.",
    },
    Step {
        key: "LIBRARY",
        scene: Scene::Normal,
        title: "THE LIBRARY",
        text: "Everything you can play lives here. Search at the top, click a song to play it, right-click it for more.",
    },
    Step {
        key: "TABS",
        scene: Scene::Normal,
        title: "WHERE THE MUSIC COMES FROM",
        text: "These tabs switch the source: Tidal, SoundCloud, your files, YouTube and your own LISTS. PRACTICE holds your tunes and the practice diary.",
    },
    Step {
        key: "PLAYER",
        scene: Scene::Normal,
        title: "THE PLAYER",
        text: "Play, skip, seek and volume, with the tempo and key of the song. The little screen is a visualizer: click it to change the style, right-click it for its settings.",
    },
    Step {
        key: "COVER",
        scene: Scene::Normal,
        title: "THE COVER IS A BUTTON",
        text: "Click the cover art (or press A) to open the big album view.",
    },
    Step {
        key: "ARTBTNS",
        scene: Scene::Album,
        title: "THE ALBUM VIEW",
        text: "A big cover, lyrics you can scroll and click to jump, and the visualizer behind it. Click the cover to put it on a spinning record. These buttons switch lyrics, the spectrum, black-and-white art, fullscreen and the skin, with the volume on the right. CLOSE or ESC takes you back.",
    },
    Step {
        key: "QUEUE",
        scene: Scene::Normal,
        title: "THE QUEUE",
        text: "What plays next. Drag a row to reorder it; right-click to remove it or add it to a playlist.",
    },
    Step {
        key: "SHUFFLE",
        scene: Scene::Normal,
        title: "SHUFFLE, REPEAT, FULLSCREEN",
        text: "Shuffle, repeat (all or one song) and fullscreen sit together on the player.",
    },
    Step {
        key: "LIBRARY",
        scene: Scene::Normal,
        title: "RIGHT-CLICK ANY SONG",
        text: "Go to its album, its artist or its radio, see the credits, add it to a playlist (your own LISTS mix any sources) or copy its link. The mouse's back and forward buttons step through the pages you opened.",
    },
    Step {
        key: "SKIN",
        scene: Scene::Normal,
        title: "MAKE IT LOOK RIGHT",
        text: "This button changes the skin: click for the next one, right-click to pick. There are retro skins, a glassy one and sleek ones.",
    },
    Step {
        key: "",
        scene: Scene::Normal,
        title: "THAT'S IT",
        text: "When an update is ready, a bar at the top says so: click it to switch to the new version. The ? button has every key and tip. Nothing you do here is reported to Tidal.",
    },
];

const PRACTICE_TOUR: &[Step] = &[
    Step {
        key: "CHIP",
        scene: Scene::PracticeOff,
        title: "PRACTICE MODE",
        text: "Tools for learning music from recordings: loop a hard spot, slow it down, fix the pitch, keep time. Click PRACTICE (or press P) to switch them on, LISTEN to go back to plain listening.",
    },
    Step {
        key: "PRACTICE",
        scene: Scene::Practice,
        title: "THE PRACTICE PANEL",
        text: "Practice mode opens this panel: the waveform with your loop on it, and the loop, speed and pitch tools.",
    },
    Step {
        key: "LOOPROW",
        scene: Scene::Practice,
        title: "1. LOOP A SPOT",
        text: "SET A where the hard part starts and SET B where it ends ([ and ] keys), then loop it (\\ key). Click a time to type it exactly. Save a loop with a name to come back to it.",
    },
    Step {
        key: "SPEEDROW",
        scene: Scene::Practice,
        title: "2. SLOW IT DOWN",
        text: "Pick a speed, or step it 5 percent at a time (UP and DOWN keys), without changing the pitch. Loop it, play it clean slow, then step up.",
    },
    Step {
        key: "MOREBOX",
        scene: Scene::PracticePitch,
        title: "3. FIX THE PITCH",
        text: "MORE opens this panel. PITCH moves the key in semitones or 10-cent steps; FIND TUNING tells you how far the record is from A440, and its key, and TUNE TO A440 lines it up with your instrument.",
    },
    Step {
        key: "TIMEBTNS",
        scene: Scene::Practice,
        title: "4. KEEP TIME",
        text: "The metronome has a count-in and can lock to the track's own beat. The focus timer runs work blocks with rests between them.",
    },
    Step {
        key: "TABS",
        scene: Scene::Practice,
        title: "5. KEEP TRACK",
        text: "The PRACTICE tab holds TUNES (your repertoire, recordings and charts the band can play) and the DIARY (what you practiced). Time only counts while a loop plays in practice mode.",
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
        self.tour_restore = Some((self.art_view, self.practice, self.more, self.mtab));
        self.tour_seen = true;
        self.dirty = true;
    }

    /// Put the screen in the state a step needs (only what differs, so nothing flickers).
    fn tour_scene(&mut self, scene: Scene) {
        let Some((_, practice, more, mtab)) = self.tour_restore else { return };
        let want_art = scene == Scene::Album;
        let want_practice = match scene {
            Scene::PracticeOff => false,
            Scene::Practice | Scene::PracticePitch => true,
            _ => practice,
        };
        let (want_more, want_tab) = if scene == Scene::PracticePitch { (true, 1) } else { (more, mtab) };
        if self.art_view != want_art {
            self.art_view = want_art;
        }
        if PRACTICE && self.practice != want_practice {
            self.apply(Action::TogglePractice);
        }
        self.more = want_more;
        self.mtab = want_tab;
    }

    /// The tour is over: everything back the way it was.
    fn tour_end(&mut self) {
        if let Some((art, practice, more, mtab)) = self.tour_restore.take() {
            self.art_view = art;
            if PRACTICE && self.practice != practice {
                self.apply(Action::TogglePractice);
            }
            self.more = more;
            self.mtab = mtab;
        }
        self.tour = None;
    }

    pub(crate) fn tour_overlay(&mut self, ctx: &egui::Context) {
        let Some((kind, i)) = self.tour else { return };
        let list = steps(kind);
        let i = i.min(list.len() - 1);
        let step = &list[i];
        self.tour_scene(step.scene);
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
            _ => self.tour_end(),
        }
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_step_points_at_something_that_reports_itself() {
        // windows report themselves by title; single controls call tour::mark("KEY", ...)
        let windows = ["LIBRARY", "PLAYER", "PRACTICE", "QUEUE"];
        let src = [
            include_str!("../main.rs"),
            include_str!("library_win.rs"),
            include_str!("player_win.rs"),
            include_str!("album_view.rs"),
            include_str!("views.rs"),
        ]
        .concat();
        for s in BASIC.iter().chain(PRACTICE_TOUR) {
            if s.key.is_empty() {
                continue;
            }
            let marked = src.contains(&format!("mark(\"{}\"", s.key)) || src.contains(&format!("row_mark(ui, \"{}\"", s.key));
            assert!(marked || windows.contains(&s.key), "tour step {} points at {}, which nothing reports", s.title, s.key);
        }
    }

    #[test]
    fn practice_steps_have_practice_on() {
        // the practice panel only exists in practice mode: steps pointing into it must switch it on
        for s in PRACTICE_TOUR.iter().filter(|s| ["PRACTICE", "LOOPROW", "SPEEDROW", "MOREBOX"].contains(&s.key)) {
            assert!(matches!(s.scene, Scene::Practice | Scene::PracticePitch), "{}", s.title);
        }
        assert!(BASIC.iter().any(|s| s.scene == Scene::Album), "the main tour visits the album view");
        assert!(BASIC.last().unwrap().scene == Scene::Normal, "and comes back from it");
    }
}
