//! The guided tour: a dimmed screen with the part being explained left bright, and a small card with
//! BACK / NEXT. One short tour for everyone, and a separate short one for the practice tools.
//! A step can light up a whole window or a single control, and can set the scene first (open the album view,
//! switch practice mode on, open MORE). The tour plays its own song (tour_song.rs) and shows things working on it:
//! a slower speed, a loop, an EQ preset. When it ends, everything comes back: what was showing, the queue, the
//! song and where it was, the speed, the loop and the equalizer.

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
    /// practice mode with MORE open on a tab (0 TRAINER, 1 PITCH & EAR, 2 STEMS)
    PracticeMore(u8),
    /// the album view in practice mode, with its little practice strip
    AlbumPractice,
}

struct Step {
    /// window or control to light up ("" = none, the card sits in the middle)
    key: &'static str,
    scene: Scene,
    title: &'static str,
    text: &'static str,
    /// the tour shows it working on the tour's song: its speed (0 = normal), the A-B loop on, an EQ preset
    speed: u32,
    ab: bool,
    eq: Option<usize>,
}

/// A step with nothing being demonstrated (fill in the rest).
const BASE: Step = Step { key: "", scene: Scene::Normal, title: "", text: "", speed: 0, ab: false, eq: None };

const BASIC: &[Step] = &[
    Step {
        title: "WELCOME",
        text: "A quick tour that shows each thing working. It plays a song so you can hear what happens. NEXT and BACK (or the arrow keys) move along, ESC stops. When it ends, everything comes back as it was: your queue, your song and where you were in it.",
        ..BASE
    },
    Step {
        key: "PLAYER",
        title: "THE PLAYER",
        text: "That is the tour's song playing. Play, pause, skip, seek and volume live here, with the tempo and key of the song. The little screen is a visualizer: click it to change its style, right-click for its settings.",
        ..BASE
    },
    Step {
        key: "SPEEDBTN",
        title: "SPEED",
        text: "Hear it? The song is at 70% now and the pitch has not moved. Click the speed for the next one, right-click to pick any; UP and DOWN step it 5% at a time.",
        speed: 70,
        ..BASE
    },
    Step {
        key: "EQUALIZER",
        title: "THE EQUALIZER",
        text: "The tour switched the equalizer on with the BASS preset: the low end comes up. Pick a preset or drag the sliders; AUTO remembers an EQ for each song. Ctrl+K and \"eq\" gets you here from anywhere.",
        eq: Some(1),
        ..BASE
    },
    Step {
        key: "LIBRARY",
        title: "THE LIBRARY",
        text: "Everything you can play lives here. Click a song to play it, right-click it for more.",
        ..BASE
    },
    Step {
        key: "TABS",
        title: "WHERE THE MUSIC COMES FROM",
        text: "These tabs switch the source: Tidal, SoundCloud, your files, YouTube and your own LISTS. PRACTICE holds your tunes and the practice diary.",
        ..BASE
    },
    Step {
        key: "QUEUE",
        title: "THE QUEUE",
        text: "What plays next. Right now it holds just the tour's song; yours is safe and comes back at the end. Drag a row to reorder it; right-click to remove it or add it to a playlist.",
        ..BASE
    },
    Step {
        key: "COVER",
        title: "THE COVER OPENS THE ALBUM VIEW",
        text: "Click the cover art (or press A) for the big album view. NEXT opens it for you.",
        ..BASE
    },
    Step {
        key: "ARTBTNS",
        scene: Scene::Album,
        title: "THE ALBUM VIEW",
        text: "A big cover, lyrics you can scroll and click to jump, and the visualizer behind it. Click the cover to put it on a spinning record. Playing is in the middle; on the left, how it looks; on the right, the speed, practice tools, the queue and the volume. CLOSE or ESC takes you back.",
        ..BASE
    },
    Step {
        key: "APPMENU",
        title: "MUSIC, PRACTICE, BACKLINE",
        text: "Three tabs: MUSIC (Tidal, your library and the other sources), PRACTICE (your tunes and the diary) and BACKLINE (skins, preferences, help and these tours, storage and the log). Under them: back and forward (the mouse's side buttons work too) and search. BACKLINE has its own tabs, Winamp skins among them. Ctrl+K finds any action by typing.",
        ..BASE
    },
    Step {
        title: "THAT'S IT",
        text: "DONE puts everything back the way you had it. Preferences has ACCESSIBILITY (colour-blind safe loops, reduce motion), and the BACKLINE tab has HIGH CONTRAST skins. When an update is ready, a bar at the top says so. The ? button has every key and tip. Nothing you do here is reported to Tidal.",
        ..BASE
    },
];

const PRACTICE_TOUR: &[Step] = &[
    Step {
        key: "CHIP",
        scene: Scene::PracticeOff,
        title: "PRACTICE MODE",
        text: "Tools for learning music from recordings: loop a hard spot, slow it down, fix the pitch, keep time. The tour plays its song so you can hear each one. Click PRACTICE (or press P) to switch them on, LISTEN to go back to plain listening.",
        ..BASE
    },
    Step {
        key: "PRACTICE",
        scene: Scene::Practice,
        title: "THE PRACTICE PANEL",
        text: "Practice mode opens this panel: the waveform of the song with your loop on it, and the loop and speed tools.",
        ..BASE
    },
    Step {
        key: "LOOPROW",
        scene: Scene::Practice,
        title: "1. LOOP A SPOT",
        text: "The tour just looped a few seconds and jumped there: listen to them come round again. Press A where the hard part starts and B where it ends ([ and ] keys), then loop it (\\ key). Click a time to type it exactly. SAVE LOOP keeps it with the song.",
        ab: true,
        ..BASE
    },
    Step {
        key: "SPEEDROW",
        scene: Scene::Practice,
        title: "2. SLOW IT DOWN",
        text: "Same loop, now at 85%, same pitch. Pick a speed from the list, or step it 1% at a time with - and + (click the number to type any). Play it clean slow, then step up.",
        ab: true,
        speed: 85,
        ..BASE
    },
    Step {
        key: "MOREBOX",
        scene: Scene::PracticeMore(0),
        title: "3. LET IT SPEED YOU UP",
        text: "The tabs along the top hold the bigger tools. The TRAINER plays your loop a set number of times, then speeds it up by a set percent, again and again: start slow and let it bring you up to tempo.",
        ab: true,
        speed: 85,
        ..BASE
    },
    Step {
        key: "MOREBOX",
        scene: Scene::PracticeMore(1),
        title: "4. FIX THE PITCH",
        text: "PITCH moves the key in semitones or 10-cent steps; FIND TUNING tells you how far the record is from A440, and its key, and TUNE TO A440 lines it up with your instrument. The ear modes play one side of the stereo, or take the middle out.",
        ..BASE
    },
    Step {
        key: "MOREBOX",
        scene: Scene::PracticeMore(2),
        title: "5. PULL IT APART",
        text: "STEMS splits a stored track into drums, bass, vocals and the rest, on this computer (a one-time download of the tool). Then play just one part, or everything but it.",
        ..BASE
    },
    Step {
        key: "TIMEBTNS",
        scene: Scene::Practice,
        title: "6. KEEP TIME",
        text: "Up in the practice window's corner: the focus timer (a pomodoro: work blocks with short rests) and the metronome (count-in, and it can lock to the track's own beat).",
        ..BASE
    },
    Step {
        key: "ARTPRACTICE",
        scene: Scene::AlbumPractice,
        title: "7. PRACTICE IN THE ALBUM VIEW",
        text: "Open the album view in practice mode and this strip comes with you: set the loop, switch it on and change the speed without leaving the big cover.",
        ab: true,
        ..BASE
    },
    Step {
        key: "TABS",
        scene: Scene::Practice,
        title: "8. KEEP TRACK",
        text: "The PRACTICE tab holds TUNES (your repertoire, recordings and charts the band can play) and the DIARY (what you practiced). DONE puts everything back the way you had it.",
        ..BASE
    },
];

/// How things were when the tour started, to put back when it ends.
pub(crate) struct TourPlay {
    queue: std::sync::Arc<Vec<crate::Track>>,
    orig_queue: Option<Vec<crate::Track>>,
    cur: Option<usize>,
    pos: f32,
    playing: bool,
    shuffle: bool,
    speed: u32,
    lp: (Option<f32>, Option<f32>, bool),
    eq: ([f32; 10], bool, bool),
    last: Option<crate::store::Last>,
    /// repeat as it was (the tour plays its song on repeat one)
    repeat: crate::Repeat,
    /// the tour's song, once it is the one playing
    song: Option<i64>,
}

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
        // keep what was playing, then put on the tour song
        let active = self.cur.is_some() && !self.stopped;
        let snap = TourPlay {
            queue: self.queue.clone(),
            orig_queue: self.orig_queue.clone(),
            cur: self.cur,
            pos: self.pos(),
            playing: active && !self.paused,
            shuffle: self.shuffle,
            speed: self.speed,
            lp: (self.loop_a, self.loop_b, self.loop_on),
            eq: (self.eq_gains, self.eq_on, self.show_eq),
            last: self.store.last.clone(),
            repeat: self.repeat,
            song: None,
        };
        self.tour_play = Some(snap);
        // the tour's song: right away when it was fetched before, otherwise once it arrives
        match crate::tour_song::ready() {
            Some(path) => self.tour_put_on(path),
            None => {
                let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
                std::thread::spawn(move || {
                    let _ = tx.send(crate::Msg::TourSong(crate::tour_song::fetch()));
                    ctx.request_repaint();
                });
            }
        }
    }

    /// Put on the tour's song (if the tour is still on and nothing has been put on yet).
    pub(crate) fn tour_put_on(&mut self, path: std::path::PathBuf) {
        if self.tour.is_none() || self.tour_play.as_ref().map_or(true, |s| s.song.is_some()) {
            return;
        }
        let mut track = crate::sources::ext_for_file(&path).to_track();
        // its own cover, drawn for Backline
        track.cover = crate::tour_song::COVER.to_string();
        if let Some(s) = self.tour_play.as_mut() {
            s.song = Some(track.id);
        }
        self.srcmap.insert(track.id, crate::Src::File(path));
        self.queue = std::sync::Arc::new(vec![track]);
        self.orig_queue = None;
        self.shuffle = false;
        // round and round, so it never runs out mid-tour
        self.repeat = crate::Repeat::One;
        self.loop_a = None;
        self.loop_b = None;
        self.loop_on = false;
        self.apply(Action::PlayIndex(0));
    }

    /// Show the step's feature working on the tour's song (only what differs, so nothing restarts).
    fn tour_demo(&mut self, speed: u32, ab: bool, eq: Option<usize>) {
        let Some(snap) = &self.tour_play else { return };
        if snap.song.is_none() {
            return;
        }
        let (saved_eq, song) = (snap.eq, snap.song);
        if self.cur_track().map(|t| t.id) != song {
            return; // something else was picked meanwhile: leave it alone
        }
        let want = if speed > 0 { speed } else { 100 };
        if self.speed != want {
            self.apply(Action::Speed(want));
        }
        if ab {
            let (a, b) = crate::tour_song::loop_span();
            if self.loop_a != Some(a) || self.loop_b != Some(b) {
                self.apply(Action::SetAAt(a));
                self.apply(Action::SetBAt(b));
                // and go there, so the loop is what you hear
                self.apply(Action::Seek(a));
            }
            if !self.loop_on {
                self.apply(Action::LoopToggle);
            }
        } else if self.loop_a.is_some() || self.loop_b.is_some() {
            self.loop_a = None;
            self.loop_b = None;
            self.loop_on = false;
            self.sync_loop();
        }
        let (gains, on, show) = match eq {
            Some(p) => (crate::EQ_PRESETS[p.min(crate::EQ_PRESETS.len() - 1)].1, true, true),
            None => saved_eq,
        };
        if self.eq_gains != gains || self.eq_on != on || self.show_eq != show {
            self.eq_gains = gains;
            self.eq_on = on;
            self.show_eq = show;
            self.apply_eq();
        }
    }

    /// Put back what was playing before the tour: the queue, the song (playing again where it was if it was
    /// playing), the speed, the loop and the equalizer.
    fn tour_unplay(&mut self) {
        let Some(s) = self.tour_play.take() else { return };
        if s.song.is_some() {
            self.apply(Action::StopBtn);
        }
        self.queue = s.queue;
        self.orig_queue = s.orig_queue;
        self.shuffle = s.shuffle;
        self.repeat = s.repeat;
        (self.eq_gains, self.eq_on, self.show_eq) = s.eq;
        self.apply_eq();
        self.store.last = s.last;
        self.store_dirty = true;
        if s.song.is_none() {
            return;
        }
        self.cur = s.cur;
        match s.cur {
            Some(i) if s.playing => {
                self.apply(Action::PlayIndex(i));
                if s.pos > 1.0 {
                    if self.buffering {
                        self.pending_seek = Some(s.pos);
                    } else {
                        self.player.send(crate::player::Cmd::Seek(s.pos));
                    }
                }
            }
            _ => {}
        }
        self.speed = s.speed;
        (self.loop_a, self.loop_b, self.loop_on) = s.lp;
        self.sync_loop();
    }

    /// Put the screen in the state a step needs (only what differs, so nothing flickers).
    fn tour_scene(&mut self, scene: Scene) {
        let Some((_, practice, more, mtab)) = self.tour_restore else { return };
        let want_art = matches!(scene, Scene::Album | Scene::AlbumPractice);
        let want_practice = match scene {
            Scene::PracticeOff => false,
            Scene::Practice | Scene::PracticeMore(_) | Scene::AlbumPractice => true,
            _ => practice,
        };
        let (want_more, want_tab) = match scene {
            Scene::PracticeMore(tab) => (true, tab),
            _ => (more, mtab),
        };
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
        self.tour_unplay();
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
        self.tour_demo(step.speed, step.ab, step.eq);
        let screen = ctx.screen_rect();
        let target = if step.key.is_empty() { None } else { find(step.key) };
        // dim everything except the highlighted window
        // the tour has the floor: only its card takes clicks until it is done
        crate::credits::block_behind(ctx, "tour");
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
            // the card keeps its own colours (soft white on near black), so it reads the same whatever skin is worn
            let frame = egui::Frame::none()
                .fill(Color32::from_rgb(20, 20, 22))
                .stroke(egui::Stroke::new(2.0_f32, Color32::from_gray(110)))
                .rounding(if crate::style() != 0 { 8.0 } else { 0.0 })
                .inner_margin(14.0);
            frame.show(ui, |ui| {
                ui.set_width(460.0_f32.min(screen.width() - 60.0));
                // the step's title, wrapped onto more lines when it is long (never cut off)
                let head = format!("{}   {}/{}", step.title, i + 1, list.len());
                for line in crate::wrap(&head, 2.5, ui.available_width() - 8.0) {
                    title_line(ui, &line, 2.5, Color32::from_rgb(240, 240, 236));
                }
                para(ui, step.text, Color32::from_rgb(225, 225, 220));
                // the credit the song's licence asks for, while it plays
                if self.tour_play.as_ref().is_some_and(|s| s.song.is_some()) {
                    // scrolling at the same speed as the song's name in the player
                    let (r, _) = ui.allocate_exact_size(
                        egui::Vec2::new(ui.available_width(), 7.0 * crate::spx(1.0) + 6.0),
                        egui::Sense::hover(),
                    );
                    crate::marquee_at(ui, r, crate::tour_song::CREDIT, 1.0, Color32::from_gray(160), crate::title_scroll());
                }
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
        let windows = ["LIBRARY", "PLAYER", "PRACTICE", "QUEUE", "EQUALIZER"];
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
        for s in PRACTICE_TOUR.iter().filter(|s| ["PRACTICE", "LOOPROW", "SPEEDROW", "MOREBOX", "ARTPRACTICE"].contains(&s.key)) {
            assert!(matches!(s.scene, Scene::Practice | Scene::PracticeMore(_) | Scene::AlbumPractice), "{}", s.title);
        }
        // the MORE steps open MORE, each on its own tab
        let tabs: Vec<u8> =
            PRACTICE_TOUR.iter().filter_map(|s| if let Scene::PracticeMore(t) = s.scene { Some(t) } else { None }).collect();
        assert_eq!(tabs, vec![0, 1, 2], "trainer, pitch and stems each get a step");
        assert!(BASIC.iter().any(|s| s.scene == Scene::Album), "the main tour visits the album view");
        assert!(BASIC.last().unwrap().scene == Scene::Normal, "and comes back from it");
    }
}
