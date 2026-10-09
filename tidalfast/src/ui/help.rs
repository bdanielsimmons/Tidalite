//! The help / about overlay (the ? button or F1): what Tidalite is, how it is meant to be used, the keys,
//! and a little about where it came from. Short chunks on purpose - read only what you need.

use crate::{pal, para, retro_btn, section_header, tab_row, title_line, Action, App, PRACTICE};
use eframe::egui;

fn points(ui: &mut egui::Ui, items: &[&str]) {
    for s in items {
        para(ui, &format!("- {}", s), pal().ink);
    }
}

impl App {
    pub(crate) fn help_overlay(&mut self, ctx: &egui::Context, acts: &mut Vec<Action>) {
        if !self.show_help {
            return;
        }
        let mut tour: Option<u8> = None;
        let mut open_prefs = false;
        let binds = self.binds.clone();
        // dim everything behind it
        let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("help_dim")));
        p.rect_filled(ctx.screen_rect(), 0.0, egui::Color32::from_black_alpha(150));
        let size = egui::vec2(640.0_f32.min(ctx.screen_rect().width() - 40.0), 520.0_f32.min(ctx.screen_rect().height() - 40.0));
        egui::Area::new(egui::Id::new("help_area"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                let frame = egui::Frame::none()
                    .fill(pal().beige)
                    .stroke(egui::Stroke::new(2.0, pal().edge))
                    .rounding(if crate::style() != 0 { 8.0 } else { 0.0 })
                    .inner_margin(12.0);
                frame.show(ui, |ui| {
                    ui.set_width(size.x - 24.0);
                    ui.set_height(size.y - 24.0);
                    title_line(ui, "HELP", 3.0, pal().ink);
                    let tabs: Vec<&str> = if PRACTICE {
                        vec!["TOUR", "START", "KEYS", "PRACTICE", "ABOUT"]
                    } else {
                        vec!["TOUR", "START", "KEYS", "ABOUT"]
                    };
                    let cur = self.help_tab.min(tabs.len() - 1);
                    if let Some(i) = tab_row(ui, &tabs, cur) {
                        self.help_tab = i;
                    }
                    let name = tabs[cur];
                    egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(size.y - 150.0).show(
                        ui,
                        |ui| match name {
                            "TOUR" => tour_tab(ui, &mut tour),
                            "START" => start_tab(ui),
                            "KEYS" => keys_tab(ui, &binds, &mut open_prefs),
                            "PRACTICE" => practice_tab(ui),
                            _ => about_tab(ui),
                        },
                    );
                    ui.add_space(6.0);
                    if retro_btn(ui, "CLOSE  (ESC)", false).clicked() {
                        acts.push(Action::ToggleHelp);
                    }
                });
            });
        if let Some(k) = tour {
            self.tour_start(k);
        }
        if open_prefs {
            acts.push(Action::TogglePrefs);
        }
    }
}

fn tour_tab(ui: &mut egui::Ui, pick: &mut Option<u8>) {
    section_header(ui, "WANT A TOUR?");
    para(
        ui,
        "A short guided look at the screen, one piece at a time. About a minute. You can stop any time with ESC.",
        pal().ink,
    );
    ui.add_space(4.0);
    if retro_btn(ui, "TAKE THE TOUR", true).clicked() {
        *pick = Some(0);
    }
    if PRACTICE {
        ui.add_space(8.0);
        para(ui, "The practice tools have their own tour, so they don't bury the basics. Take it when you want to loop and slow down music.", pal().ink);
        if retro_btn(ui, "PRACTICE TOUR", false).clicked() {
            *pick = Some(1);
        }
    }
    ui.add_space(8.0);
    para(ui, "Prefer to read? The other tabs have the same things written out.", pal().ink2);
}

fn start_tab(ui: &mut egui::Ui) {
    section_header(ui, "WHAT THIS IS");
    para(
        ui,
        "A small desktop player for Tidal, SoundCloud, YouTube and your own files, made for practicing music. The retro look is on purpose. Nothing here is required - skip to what you need.",
        pal().ink,
    );
    section_header(ui, "THE 30-SECOND TOUR");
    points(
        ui,
        &[
            "LEFT window = LIBRARY. The tabs at the top switch the source.",
            "MIDDLE window = PLAYER. Play, skip, seek, volume. Click the cover to open the big album view.",
            "RIGHT window = QUEUE. Drag a row to reorder it.",
            "RIGHT-CLICK almost anything: songs, the seek bar, the skin button, the visualizer.",
            "Right-click a song for: play next, add to queue, add to a playlist, copy its link.",
        ],
    );
    section_header(ui, "THE ALBUM VIEW");
    para(
        ui,
        "Click the cover or press A for the big view: large art, lyrics, a visualizer behind it. Made for listening.",
        pal().ink,
    );
    points(
        ui,
        &[
            "L shows lyrics next to the cover. Scroll them, or click a line to jump there.",
            "The spectrum icon turns the visualizer on and off. Right-click it for opacity, height, width, bar count, colors.",
            "The skin button is in the view too, so you can change the look without leaving.",
            "In practice mode your loop shows on the seek bar. Right-click the bar to set one.",
            "The little visualizer in the player window has its own settings - right-click it. The two don't affect each other.",
        ],
    );
    section_header(ui, "WHERE THE MUSIC COMES FROM");
    points(
        ui,
        &[
            "TIDAL: sign in with your own subscription. The sign-in stays on this computer.",
            "SOUNDCLOUD and YT: press GET YT-DLP once (a free helper tool). Then search or paste links.",
            "FILES: drag audio files or folders onto the window.",
            "LISTS: your own playlists. Mix any of the sources in one list.",
        ],
    );
    section_header(ui, "MAKE IT YOURS");
    points(
        ui,
        &[
            "Click the little squares button for the next skin, or right-click it to pick one.",
            "In the album view: right-click the spectrum icon for bars, waveform, colors, size.",
            "Everything saves by itself.",
        ],
    );
}

fn keys_tab(ui: &mut egui::Ui, binds: &[Option<egui::Key>], open_prefs: &mut bool) {
    para(ui, "Keys work whenever you are not typing in a box. You can change any of them in Preferences.", pal().ink2);
    if retro_btn(ui, "CHANGE SHORTCUTS", false).clicked() {
        *open_prefs = true;
    }
    for (g, name) in ["PLAYING", "ALBUM VIEW", "PRACTICE"].iter().enumerate() {
        if g == 2 && !PRACTICE {
            continue;
        }
        section_header(ui, name);
        for (i, c) in crate::prefs::CMDS.iter().enumerate() {
            if c.group as usize == g {
                let k = binds.get(i).copied().flatten().map_or("(none)".to_string(), crate::prefs::key_label);
                para(ui, &format!("- {}  {}", k, c.label), pal().ink);
            }
        }
        if g == 0 {
            para(ui, "- F1  this help", pal().ink);
        }
        if g == 1 {
            para(ui, "- ESC  back", pal().ink);
        }
    }
}

fn practice_tab(ui: &mut egui::Ui) {
    section_header(ui, "THE IDEA");
    para(ui, "Pick a hard spot, loop it, slow it down, play it clean, then step the speed up. Turn practice mode on with the PRACTICE chip or P.", pal().ink);
    section_header(ui, "THE FOUR TOOLS");
    points(
        ui,
        &[
            "LOOP: [ and ] set the ends, \\ turns it on. Right-click the seek bar for more. Loops are remembered per track.",
            "SPEED: UP / DOWN arrows, 5 percent at a time, pitch unchanged.",
            "PITCH: semitones, or 10-cent steps. FIND TUNING lines up tracks that aren't at A440.",
            "TIME: metronome with count-in, and a focus timer for work blocks.",
        ],
    );
    section_header(ui, "KEEPING TRACK");
    points(
        ui,
        &[
            "TUNES is your repertoire, with charts and recordings. DIARY shows what you practiced.",
            "Practice time counts only while a loop plays in practice mode.",
        ],
    );
}

fn about_tab(ui: &mut egui::Ui) {
    section_header(ui, "VERSION");
    let b = crate::update::build();
    let v = if b > 0 {
        format!("Tidalite {} build {}", crate::VERSION, b)
    } else {
        format!("Tidalite {} (local build)", crate::VERSION)
    };
    para(ui, &v, pal().ink);
    para(ui, "It checks for updates by itself and installs the new version the next time you open it (or when you click the bar at the top).", pal().ink2);
    section_header(ui, "WHO MADE IT");
    para(ui, "Built by Daniel Simmons.", pal().ink);
    section_header(ui, "WHY IT EXISTS");
    para(
        ui,
        "Built for personal practice: a fast player that doesn't crash, with loops, slow-down and a calm interface that stays out of your way. Written in Rust.",
        pal().ink,
    );
    section_header(ui, "PRIVACY");
    points(
        ui,
        &[
            "Loops, speed, seeks, playlists and your diary stay on this computer.",
            "Nothing is reported to Tidal, SoundCloud or YouTube about what you play.",
            "The one thing sent out on its own is the update check to GitHub.",
        ],
    );
    section_header(ui, "WHERE YOUR STUFF IS");
    para(ui, &crate::api::config_dir().display().to_string(), pal().ink2);
    para(ui, "library.json holds your tunes, loops, playlists and diary. Back that file up if you care about it.", pal().ink2);
    section_header(ui, "FINE PRINT");
    para(
        ui,
        "Unofficial. Not made by or affiliated with Tidal, SoundCloud or YouTube. You need your own Tidal subscription. Downloading from YouTube or SoundCloud may go against their terms; personal use is at your own discretion.",
        pal().dim,
    );
    section_header(ui, "BUILT WITH");
    para(ui, "egui, symphonia, rodio, yt-dlp, and the HT-Demucs model for stem splitting.", pal().ink2);
}
