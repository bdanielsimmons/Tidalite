//! Preferences: how updates behave and which keys do what. Opened with the PREFS button (or from Help).
//! Key bindings live in `App::binds` (one slot per entry of CMDS, None = unbound) and are saved with the settings.

use crate::{check_box, pal, para, retro_btn, section_header, title_line, Action, App, PRACTICE};
use eframe::egui::{self, Key};

#[derive(Clone, Copy, PartialEq)]
pub enum Cmd {
    PlayPause,
    SeekBack,
    SeekFwd,
    Prev,
    Play,
    Pause,
    Stop,
    Next,
    Like,
    Mini,
    Art,
    Lyrics,
    Gray,
    Fullscreen,
    Practice,
    MarkA,
    MarkB,
    LoopToggle,
    Back2,
    ToA,
    SpeedUp,
    SpeedDown,
}

pub struct CmdDef {
    pub cmd: Cmd,
    pub id: &'static str,
    pub label: &'static str,
    pub def: Key,
    pub group: u8, // 0 playing, 1 album view, 2 practice
}

pub const CMDS: &[CmdDef] = &[
    CmdDef { cmd: Cmd::PlayPause, id: "toggle", label: "Play / pause", def: Key::Space, group: 0 },
    CmdDef { cmd: Cmd::SeekBack, id: "back5", label: "Back 5 seconds", def: Key::ArrowLeft, group: 0 },
    CmdDef { cmd: Cmd::SeekFwd, id: "fwd5", label: "Forward 5 seconds", def: Key::ArrowRight, group: 0 },
    CmdDef { cmd: Cmd::Prev, id: "prev", label: "Previous song", def: Key::Z, group: 0 },
    CmdDef { cmd: Cmd::Play, id: "play", label: "Play", def: Key::X, group: 0 },
    CmdDef { cmd: Cmd::Pause, id: "pause", label: "Pause", def: Key::C, group: 0 },
    CmdDef { cmd: Cmd::Stop, id: "stop", label: "Stop", def: Key::V, group: 0 },
    CmdDef { cmd: Cmd::Next, id: "next", label: "Next song", def: Key::B, group: 0 },
    CmdDef { cmd: Cmd::Like, id: "like", label: "Like the current song", def: Key::H, group: 0 },
    CmdDef { cmd: Cmd::Mini, id: "mini", label: "Mini player", def: Key::M, group: 0 },
    CmdDef { cmd: Cmd::Art, id: "art", label: "Album view on / off", def: Key::A, group: 1 },
    CmdDef { cmd: Cmd::Lyrics, id: "lyrics", label: "Lyrics", def: Key::L, group: 1 },
    CmdDef { cmd: Cmd::Gray, id: "gray", label: "Black and white art", def: Key::G, group: 1 },
    CmdDef { cmd: Cmd::Fullscreen, id: "full", label: "Fullscreen (album view)", def: Key::F, group: 1 },
    CmdDef { cmd: Cmd::Practice, id: "practice", label: "Practice mode on / off", def: Key::P, group: 2 },
    CmdDef { cmd: Cmd::MarkA, id: "mark_a", label: "Set loop start (A)", def: Key::OpenBracket, group: 2 },
    CmdDef { cmd: Cmd::MarkB, id: "mark_b", label: "Set loop end (B)", def: Key::CloseBracket, group: 2 },
    CmdDef { cmd: Cmd::LoopToggle, id: "loop", label: "Loop on / off", def: Key::Backslash, group: 2 },
    CmdDef { cmd: Cmd::Back2, id: "back2", label: "Back 2 seconds", def: Key::Comma, group: 2 },
    CmdDef { cmd: Cmd::ToA, id: "to_a", label: "Jump to loop start", def: Key::Period, group: 2 },
    CmdDef { cmd: Cmd::SpeedUp, id: "speed_up", label: "Speed up 5 percent", def: Key::ArrowUp, group: 2 },
    CmdDef { cmd: Cmd::SpeedDown, id: "speed_down", label: "Slow down 5 percent", def: Key::ArrowDown, group: 2 },
];

pub fn default_binds() -> Vec<Option<Key>> {
    CMDS.iter().map(|c| Some(c.def)).collect()
}

/// Keys that always keep their meaning, so they can't be given away.
fn reserved(k: Key) -> bool {
    matches!(k, Key::Escape | Key::F1 | Key::F11 | Key::Tab | Key::Enter)
}

pub fn key_label(k: Key) -> String {
    match k {
        Key::ArrowLeft => "LEFT".into(),
        Key::ArrowRight => "RIGHT".into(),
        Key::ArrowUp => "UP".into(),
        Key::ArrowDown => "DOWN".into(),
        Key::OpenBracket => "[".into(),
        Key::CloseBracket => "]".into(),
        Key::Backslash => "\\".into(),
        Key::Comma => ",".into(),
        Key::Period => ".".into(),
        Key::Minus => "-".into(),
        Key::Plus | Key::Equals => "=".into(),
        Key::Semicolon => ";".into(),
        Key::Slash => "/".into(),
        Key::Backtick => "`".into(),
        Key::Quote => "'".into(),
        Key::Num0 => "0".into(),
        Key::Num1 => "1".into(),
        Key::Num2 => "2".into(),
        Key::Num3 => "3".into(),
        Key::Num4 => "4".into(),
        Key::Num5 => "5".into(),
        Key::Num6 => "6".into(),
        Key::Num7 => "7".into(),
        Key::Num8 => "8".into(),
        Key::Num9 => "9".into(),
        other => other.name().to_uppercase(),
    }
}

pub fn binds_to_json(b: &[Option<Key>]) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    for (i, c) in CMDS.iter().enumerate() {
        let v = match b.get(i).copied().flatten() {
            Some(k) => serde_json::Value::String(k.name().to_string()),
            None => serde_json::Value::Null,
        };
        m.insert(c.id.to_string(), v);
    }
    serde_json::Value::Object(m)
}

/// Missing entries keep their default; null means the user cleared that key.
pub fn binds_from_json(v: &serde_json::Value) -> Vec<Option<Key>> {
    let mut out = default_binds();
    let Some(m) = v.as_object() else { return out };
    for (i, c) in CMDS.iter().enumerate() {
        if let Some(x) = m.get(c.id) {
            out[i] = x.as_str().and_then(Key::from_name).filter(|k| !reserved(*k));
        }
    }
    // a saved file could hold the same key twice: keep the first
    for i in 0..out.len() {
        for j in 0..i {
            if out[i].is_some() && out[i] == out[j] {
                out[i] = None;
            }
        }
    }
    out
}

impl App {
    /// Which command (if any) a key press means right now.
    pub(crate) fn key_commands(&self, ctx: &egui::Context) -> Vec<Cmd> {
        let mut out = Vec::new();
        for (i, c) in CMDS.iter().enumerate() {
            if let Some(k) = self.binds.get(i).copied().flatten() {
                if ctx.input(|inp| inp.key_pressed(k)) {
                    out.push(c.cmd);
                }
            }
        }
        out
    }

    pub(crate) fn run_cmd(&mut self, cmd: Cmd, acts: &mut Vec<Action>) {
        match cmd {
            Cmd::PlayPause => acts.push(Action::Toggle),
            Cmd::SeekBack => acts.push(Action::SeekRel(-5.0)),
            Cmd::SeekFwd => acts.push(Action::SeekRel(5.0)),
            Cmd::Prev => acts.push(Action::Prev),
            Cmd::Play => acts.push(Action::PlayBtn),
            Cmd::Pause => acts.push(Action::PauseBtn),
            Cmd::Stop => acts.push(Action::StopBtn),
            Cmd::Next => acts.push(Action::Next),
            Cmd::Like => {
                if let Some(t) = self.cur_track() {
                    acts.push(Action::ToggleLike(t));
                }
            }
            Cmd::Mini => acts.push(Action::ToggleMini),
            Cmd::Art => acts.push(Action::ToggleArt),
            Cmd::Lyrics => acts.push(Action::ToggleLyrics),
            Cmd::Gray => acts.push(Action::ToggleGray),
            Cmd::Fullscreen => {
                if self.art_view {
                    acts.push(Action::ToggleFullscreen)
                }
            }
            Cmd::Practice => acts.push(Action::TogglePractice),
            Cmd::MarkA => acts.push(Action::SetAAt(self.pos())),
            Cmd::MarkB => acts.push(Action::SetBAt(self.pos())),
            Cmd::LoopToggle => acts.push(Action::LoopToggle),
            Cmd::Back2 => acts.push(Action::SeekRel(-2.0)),
            Cmd::ToA => acts.push(Action::Seek(self.loop_a.unwrap_or(0.0))),
            Cmd::SpeedUp => {
                if self.practice {
                    acts.push(Action::Speed(self.speed + 5))
                }
            }
            Cmd::SpeedDown => {
                if self.practice {
                    acts.push(Action::Speed(self.speed.saturating_sub(5)))
                }
            }
        }
    }

    /// The key currently bound to a command, as text (for help and tooltips).
    pub(crate) fn key_text(&self, cmd: Cmd) -> String {
        let i = CMDS.iter().position(|c| c.cmd == cmd).unwrap_or(0);
        self.binds.get(i).copied().flatten().map_or("(none)".to_string(), key_label)
    }

    pub(crate) fn prefs_overlay(&mut self, ctx: &egui::Context, acts: &mut Vec<Action>) {
        if !self.show_prefs {
            return;
        }
        // listening for a new key?
        if let Some(slot) = self.rebinding {
            let pressed: Option<Key> = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Key { key, pressed: true, repeat: false, .. } => Some(*key),
                    _ => None,
                })
            });
            if let Some(k) = pressed {
                self.rebinding = None;
                if k == Key::Escape {
                    self.bind_note = String::new();
                } else if reserved(k) {
                    self.bind_note = format!("{} is kept for the app (ESC, F1, F11, TAB, ENTER).", key_label(k));
                } else {
                    let mut note = String::new();
                    for j in 0..self.binds.len() {
                        if j != slot && self.binds[j] == Some(k) {
                            self.binds[j] = None;
                            note = format!("{} was on \"{}\" - that one is now unbound.", key_label(k), CMDS[j].label);
                        }
                    }
                    self.binds[slot] = Some(k);
                    self.bind_note = note;
                    self.dirty = true;
                }
            }
        } else if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.show_prefs = false;
            return;
        }

        let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("prefs_dim")));
        p.rect_filled(ctx.screen_rect(), 0.0, egui::Color32::from_black_alpha(150));
        let size = egui::vec2(640.0_f32.min(ctx.screen_rect().width() - 40.0), 540.0_f32.min(ctx.screen_rect().height() - 40.0));
        egui::Area::new(egui::Id::new("prefs_area"))
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
                    title_line(ui, "PREFERENCES", 3.0, pal().ink);
                    egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(size.y - 90.0).show(ui, |ui| {
                        section_header(ui, "UPDATES");
                        let status = match self.upd_state {
                            1 => "Downloading an update...".to_string(),
                            2 => format!(
                                "{} is downloaded. Use the bar at the top to restart, or it installs the next time you open Tidalite.",
                                self.upd.as_ref().map_or("An update".to_string(), |u| u.tag.clone())
                            ),
                            3 => "The last check or download failed. It will try again later.".to_string(),
                            _ => format!("You are up to date (build {}).", crate::update::build()),
                        };
                        para(ui, &status, pal().ink2);
                        if check_box(ui, "RESTART BY ITSELF WHEN I'M AWAY", self.auto_restart)
                            .tip("Off by default. When on, a downloaded update restarts Tidalite after about 5 quiet minutes with nothing playing. When off, nothing ever restarts unless you click.")
                            .clicked()
                        {
                            self.auto_restart = !self.auto_restart;
                            self.dirty = true;
                        }
                        para(
                            ui,
                            "Updates are checked when the app opens and then every 6 hours, a tiny request that costs nothing when there is nothing new.",
                            pal().ink2,
                        );
                        if matches!(self.upd_state, 0 | 3) && retro_btn(ui, "CHECK NOW", false).clicked() {
                            self.start_update_check();
                        }

                        section_header(ui, "KEYBOARD SHORTCUTS");
                        para(ui, "Click a key, then press the new one. ESC cancels. Giving a key to something else unbinds the old one.", pal().ink2);
                        if !self.bind_note.is_empty() {
                            para(ui, &self.bind_note.clone(), pal().ink);
                        }
                        let groups = ["PLAYING", "ALBUM VIEW", "PRACTICE"];
                        for (g, gname) in groups.iter().enumerate() {
                            if g == 2 && !PRACTICE {
                                continue;
                            }
                            ui.add_space(4.0);
                            para(ui, gname, pal().ink);
                            for (i, c) in CMDS.iter().enumerate() {
                                if c.group as usize != g {
                                    continue;
                                }
                                ui.horizontal(|ui| {
                                    ui.set_min_width(size.x - 60.0);
                                    let listening = self.rebinding == Some(i);
                                    let txt = if listening {
                                        "PRESS A KEY...".to_string()
                                    } else {
                                        self.binds[i].map_or("(NONE)".to_string(), key_label)
                                    };
                                    if retro_btn(ui, &txt, listening).clicked() {
                                        self.rebinding = if listening { None } else { Some(i) };
                                        self.bind_note.clear();
                                    }
                                    para(ui, c.label, pal().ink);
                                });
                            }
                        }
                        ui.add_space(8.0);
                        if retro_btn(ui, "RESET SHORTCUTS TO DEFAULT", false).clicked() {
                            self.binds = default_binds();
                            self.rebinding = None;
                            self.bind_note.clear();
                            self.dirty = true;
                        }
                        para(ui, "ESC, F1 (help) and F11 (fullscreen) always keep their jobs.", pal().ink2);
                    });
                    ui.add_space(6.0);
                    if retro_btn(ui, "CLOSE  (ESC)", false).clicked() {
                        acts.push(Action::TogglePrefs);
                    }
                });
            });
    }
}
