//! The logic behind the practice extras: files / YouTube sources, the tune list, the practice
//! diary, saved sections, the speed trainer, metronome and the rest. (The drawing is in views.rs.)

use crate::api::{self, Track};
use crate::band;
use crate::player::Cmd;
use crate::sources::{self, Src};
use crate::stems;
use crate::store::{self, Entry, Ext, Last, Section, Tune, Version};
use crate::{cache, Action, App, Msg, Sec};
use eframe::egui;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const CHAN_NAMES: [&str; 6] = ["STEREO", "LEFT CHANNEL", "RIGHT CHANNEL", "MONO", "NO CENTER", "BASS ONLY"];

/// The settable numbers (the +/- buttons and the typed boxes next to them).
#[derive(Clone, Copy, PartialEq)]
pub enum Knob {
    Focus,
    Rest,
    Blocks,
    Loops,
    Step,
    Speed,
    LoopA,
    LoopB,
    Bpm,
    Beats,
    Unit,
    GapPlay,
    GapMute,
    RampBars,
    RampBpm,
    Shift,
}

/// Switches and small cycles (metronome, band, timer).
#[derive(Clone, Copy)]
pub enum Opt {
    MetroOn,
    Preset(u8),
    Level(usize),
    Sub,
    Follow,
    Sync,
    Double,
    Half,
    FindBeats,
    Band,
    BandStyleSet(u8),
    Part(usize),
    PomoSound,
    UnitSet(u8),
    Group,
}

/// "1:23.5", "83" -> seconds
pub fn parse_time(t: &str) -> Option<f32> {
    let t = t.trim();
    match t.split_once(':') {
        Some((m, s)) => Some(m.trim().parse::<f32>().ok()? * 60.0 + s.trim().parse::<f32>().ok()?),
        None => t.parse::<f32>().ok(),
    }
    .filter(|v| *v >= 0.0)
}

fn knob_step(k: Knob) -> f32 {
    match k {
        Knob::Focus | Knob::Speed | Knob::Shift => 5.0,
        Knob::LoopA | Knob::LoopB => 0.2,
        _ => 1.0,
    }
}

fn wave_path(id: i64) -> PathBuf {
    api::config_dir().join("waves").join(format!("{}.pk", id))
}

/// Find the files below the given paths (folders are searched, and remembered for RESCAN).
fn scan_paths(paths: Vec<PathBuf>) -> (Vec<Ext>, Vec<String>) {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut folders: Vec<String> = Vec::new();
    for p in paths {
        if p.is_dir() {
            folders.push(p.to_string_lossy().to_string());
            sources::scan_folder(&p, 0, &mut files);
        } else if sources::is_audio(&p) {
            files.push(p);
        }
    }
    let mut seen: HashSet<PathBuf> = HashSet::new();
    files.retain(|f| seen.insert(f.clone()));
    (files.iter().map(|f| sources::ext_for_file(f)).collect(), folders)
}

/// Tune name from a track title: "Cherokee (Remastered 2005)" -> "Cherokee".
fn tune_name_from(title: &str) -> String {
    let mut s = title.to_string();
    for sep in [" (", " [", " - ", " – "] {
        if let Some(i) = s.find(sep) {
            s.truncate(i);
        }
    }
    let s = s.trim().to_string();
    if s.is_empty() {
        title.trim().to_string()
    } else {
        s
    }
}

fn safe_name(s: &str) -> String {
    s.chars().map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c }).collect::<String>().trim().to_string()
}

impl App {
    // ------------------------------------------------------------ start-up
    pub(crate) fn init_store(&mut self) {
        let mut vs: Vec<Version> = Vec::new();
        vs.extend(self.store.files.iter().map(|e| e.to_version()));
        vs.extend(self.store.yt.iter().map(|e| e.to_version()));
        vs.extend(self.store.sc.iter().map(|e| e.to_version()));
        for t in &self.store.tunes {
            vs.extend(t.versions.iter().cloned());
        }
        if let Some(l) = &self.store.last {
            vs.push(l.version.clone());
        }
        for v in &vs {
            self.register(v);
        }
        if !self.store.folders.is_empty() {
            self.rescan();
        }
    }

    /// Remember where a non-Tidal recording comes from, so it can be played from any list.
    pub(crate) fn register(&mut self, v: &Version) {
        match v.kind.as_str() {
            "file" => {
                self.srcmap.insert(v.id, Src::File(PathBuf::from(&v.src)));
            }
            "yt" => {
                self.srcmap.insert(v.id, Src::Yt(v.src.clone()));
            }
            _ => {}
        }
    }

    pub(crate) fn version_of(&self, t: &Track) -> Version {
        let (kind, src) = match self.srcmap.get(&t.id) {
            Some(Src::File(p)) => ("file", p.to_string_lossy().to_string()),
            Some(Src::Yt(v)) => ("yt", v.clone()),
            None => ("tidal", String::new()),
        };
        Version {
            kind: kind.to_string(),
            id: t.id,
            src,
            title: t.title.clone(),
            artist: t.artist.clone(),
            album: t.album.clone(),
            cover: t.cover.clone(),
            dur: t.duration,
            ..Default::default()
        }
    }

    pub(crate) fn cur_tune(&self) -> Option<usize> {
        self.cur_track().and_then(|t| self.store.tune_of(t.id))
    }

    pub(crate) fn find_tune(&self, name: &str) -> Option<usize> {
        let w = name.trim().to_lowercase();
        self.store.tunes.iter().position(|t| t.name.trim().to_lowercase() == w)
    }

    /// The tune the lead sheet follows: the one you opened, or the playing track's tune. None = a
    /// track that is in no tune (its chart is looked up on its own).
    pub(crate) fn chart_tune(&self) -> Option<usize> {
        if self.chart_live {
            return None;
        }
        self.chart_pick.filter(|i| *i < self.store.tunes.len()).or_else(|| self.cur_tune())
    }

    /// Chart text on show right now.
    pub(crate) fn chart_text(&self) -> Option<String> {
        match self.chart_tune() {
            Some(i) => Some(self.store.tunes[i].chart.clone()),
            None => self.live.as_ref().map(|l| l.0.clone()),
        }
        .filter(|t| !t.trim().is_empty())
    }

    pub(crate) fn practice_label(&self) -> String {
        match self.cur_tune() {
            Some(i) => self.store.tunes[i].name.clone(),
            None => "Free practice".to_string(),
        }
    }

    // ------------------------------------------------- per-frame bookkeeping
    pub(crate) fn tick_practice(&mut self) {
        let dt = self.acc_at.elapsed().as_secs_f32().min(1.0);
        self.acc_at = Instant::now();
        let playing = self.cur.is_some() && !self.stopped && !self.paused && !self.buffering;
        // only real loop work counts - not just having practice mode open
        let looping = self.loop_on && self.loop_a.is_some() && self.loop_b.is_some();
        if self.practice && playing && looping {
            self.acc += dt;
        }
        let w = self.player.ctl.wraps();
        if w > self.last_wraps && self.practice && playing {
            self.on_loops(w - self.last_wraps);
        }
        self.last_wraps = w;
        if self.acc >= 5.0 {
            self.flush_practice();
        }
        self.update_metro(playing);
        self.pomo_tick();
        if playing && self.last_cap.elapsed() > Duration::from_secs(10) {
            self.capture_last();
        }
        if self.store_dirty && self.store_saved.elapsed() > Duration::from_secs(3) {
            self.store.save();
            self.store_dirty = false;
            self.store_saved = Instant::now();
        }
    }

    /// Move whole seconds of practice into today's diary.
    pub(crate) fn flush_practice(&mut self) {
        let secs = self.acc as u32;
        if secs == 0 {
            return;
        }
        self.acc -= secs as f32;
        let label = self.practice_label();
        let date = store::today_str();
        let d = self.store.day_mut(&date);
        *d.secs.entry(label).or_insert(0) += secs;
        self.store_dirty = true;
    }

    fn on_loops(&mut self, n: u32) {
        let date = store::today_str();
        self.store.day_mut(&date).loops += n;
        self.store_dirty = true;
        if self.trainer {
            self.passes += n;
            self.check_trainer();
        }
    }

    fn check_trainer(&mut self) {
        if self.passes < self.trainer_n {
            return;
        }
        self.passes = 0;
        if self.speed < 100 {
            self.speed = (self.speed + self.trainer_step).min(100);
            self.sync_loop();
            if self.speed >= 100 {
                self.set_note("FULL SPEED - NICE WORK");
            } else {
                self.set_note(&format!("SPEED UP: {}%", self.speed));
            }
        }
    }

    pub(crate) fn capture_last(&mut self) {
        if self.stopped {
            return;
        }
        if let Some(t) = self.cur_track() {
            let v = self.version_of(&t);
            self.store.last = Some(Last { version: v, pos: self.pos(), speed: self.speed });
            self.store_dirty = true;
            self.last_cap = Instant::now();
        }
    }

    /// Tempo the metronome clicks at right now (beats per real minute).
    fn metro_tempo(&self, playing: bool) -> f32 {
        let base = (if self.bpm > 0 { self.bpm } else { self.mt_bpm }) as f32 + self.mt_add;
        if self.mt.follow && self.practice && playing {
            base * self.speed as f32 / 100.0
        } else {
            base
        }
    }

    /// Keep the metronome and the band going: tempo changes, speed-up drill, locking to the track.
    fn update_metro(&mut self, playing: bool) {
        self.band_tick();
        if !self.mt.on {
            if self.mt_sig != 0 {
                self.player.send(Cmd::Pcm(0, Vec::new(), Vec::new(), 0.0));
                self.mt_sig = 0;
                self.mt_vis = None;
            }
            return;
        }
        let tid = self.cur_track().map(|t| t.id);
        let locked = self.mt.sync && playing && self.beat.map_or(false, |b| Some(b.0) == tid);
        if self.mt.sync && !locked {
            // locking was asked for but there is nothing to lock to yet
            if self.mt_sig != 0 {
                self.player.send(Cmd::Pcm(0, Vec::new(), Vec::new(), 0.0));
                self.mt_sig = 0;
                self.mt_vis = None;
            }
            return;
        }
        let ratio = if self.practice && playing { self.speed as f32 / 100.0 } else { 1.0 };
        if !locked && self.mt.ramp_bars > 0 {
            let tempo = self.metro_tempo(playing);
            let bar = self.mt.beats as f32 * 60.0 / self.mt.pulse(tempo.max(20.0));
            if self.mt_step_at.elapsed().as_secs_f32() >= self.mt.ramp_bars as f32 * bar {
                if tempo + self.mt.ramp_bpm as f32 <= self.mt.ramp_max as f32 {
                    self.mt_add += self.mt.ramp_bpm as f32;
                }
                self.mt_step_at = Instant::now();
            }
        }
        if locked {
            let pos = self.pos();
            let (last, at) = self.mt_pos;
            if (pos - (last + at.elapsed().as_secs_f32() * ratio)).abs() > 0.25 || self.mt_sync_at.elapsed().as_secs() >= 12 {
                self.mt_gen += 1;
                self.mt_sync_at = Instant::now();
            }
            self.mt_pos = (pos, Instant::now());
        }
        let period = self.beat.map_or(0.5, |b| b.1);
        let tempo = if locked { 60.0 * ratio / period } else { self.metro_tempo(playing) };
        let m = &self.mt;
        let mut sig = (tempo * 10.0) as u64 ^ ((m.beats as u64) << 20) ^ ((m.sub as u64) << 24) ^ ((m.gap_play as u64) << 28);
        sig ^= (m.gap_mute as u64) << 34 ^ (locked as u64) << 40 ^ (self.mt_gen as u64) << 41 ^ (m.unit as u64) << 52;
        for l in &m.levels[..m.beats.clamp(1, 32) as usize] {
            sig = sig.wrapping_mul(31).wrapping_add(*l as u64 + 1);
        }
        sig |= 1 << 63;
        if sig == self.mt_sig {
            return;
        }
        self.mt_sig = sig;
        let mut cfg = self.mt.clone();
        let mut lead = Vec::new();
        let mut wait = 0.0f32;
        if locked {
            cfg.beats = 1;
            cfg.levels = [2; 32];
            cfg.unit = 4;
            cfg.gap_play = 0;
            if let Some((_, period, phase)) = self.beat {
                let pos = self.pos();
                let next = phase + ((pos - phase) / period).ceil().max(0.0) * period;
                let each = period / ratio;
                wait = ((next - pos).max(0.0) / ratio + self.mt.shift_ms as f32 / 1000.0).rem_euclid(each);
                lead = vec![0i16; (wait * band::RATE) as usize];
            }
        }
        let body = band::click_loop(&cfg, tempo);
        let cycle =
            cfg.beats.clamp(1, 32) as usize * if cfg.gap_play > 0 { (cfg.gap_play + cfg.gap_mute.max(1)) as usize } else { 1 };
        self.mt_vis = Some((Instant::now() + Duration::from_secs_f32(wait), 60.0 / cfg.pulse(tempo), cycle));
        self.player.send(Cmd::Pcm(0, lead, body, 0.8));
    }

    // ------------------------------------------------------------------ band
    fn band_sig_now(&self) -> u64 {
        let bpm = (if self.bpm > 0 { self.bpm } else { self.mt_bpm }) as u64;
        bpm ^ (self.band_style as u64) << 12
            ^ (self.band_parts.iter().enumerate().map(|(i, p)| (*p as u64) << i).sum::<u64>()) << 20
    }

    fn band_start(&mut self) {
        let Some(text) = self.chart_text() else {
            self.set_note("NO CHART TO PLAY YET");
            return;
        };
        let chart = crate::chart::parse_friendly(&text);
        if chart.bars.is_empty() {
            return;
        }
        let order = chart.play_order();
        let bpm = (if self.bpm > 0 { self.bpm } else { self.mt_bpm }) as f32;
        let body = band::render(&chart, &order, bpm, self.band_style, self.band_parts);
        let lead = band::count_in(chart.beats, bpm);
        self.band_bar = chart.beats as f32 * 60.0 / bpm;
        self.band_lead = self.band_bar;
        self.band_order = order;
        self.band_t0 = Instant::now();
        self.band_on = true;
        self.band_chart = text;
        self.band_sig = self.band_sig_now();
        self.player.send(Cmd::Pcm(1, lead, body, 0.9));
    }

    pub(crate) fn band_stop(&mut self) {
        if self.band_on {
            self.player.send(Cmd::Pcm(1, Vec::new(), Vec::new(), 0.0));
            self.band_on = false;
        }
    }

    /// Restart the band when its tempo, style or parts changed.
    fn band_tick(&mut self) {
        if self.band_on && self.band_sig != self.band_sig_now() {
            self.band_start();
        }
    }

    // ----------------------------------------------------------- track events
    /// A new track is about to start: restore its loop and tempo, reset per-track practice state.
    pub(crate) fn on_track_change(&mut self, t: &Track) {
        self.flush_practice();
        match self.loops.get(&t.id).copied() {
            Some((a, b)) => {
                self.loop_a = Some(a);
                self.loop_b = Some(b);
                self.loop_on = self.practice;
            }
            None => {
                self.loop_a = None;
                self.loop_b = None;
                self.loop_on = false;
            }
        }
        // the lead sheet stays with the tune you have open; it only follows the playing song when none is open
        match self.tune_open.filter(|i| *i < self.store.tunes.len()) {
            Some(i) => {
                self.chart_pick = Some(i);
                self.chart_live = false;
            }
            None => match self.store.tune_of(t.id) {
                Some(i) => {
                    self.chart_pick = Some(i);
                    self.chart_live = false;
                }
                None => self.chart_live = true,
            },
        }
        self.semis = 0;
        self.stem_on = [true; 4];
        if self.stem_data.as_ref().map(|d| d.0) != Some(t.id) {
            self.stem_data = None;
        }
        self.passes = 0;
        self.pending_seek = None;
        self.sel_anchor = None;
        self.last_wraps = self.player.ctl.wraps();
        self.bpm = self
            .store
            .bpm
            .get(&t.id)
            .copied()
            .or_else(|| self.store.tune_of(t.id).map(|i| self.store.tunes[i].bpm).filter(|b| *b > 0))
            .unwrap_or(0);
        self.wave = (t.id, None);
        self.sync_loop();
    }

    /// The audio was handed to the player.
    pub(crate) fn after_play(&mut self) {
        if let Some(t) = self.pending_seek.take() {
            self.player.send(Cmd::Seek(t));
        }
        self.last_wraps = self.player.ctl.wraps();
    }

    pub(crate) fn start_wave(&mut self, id: i64, bytes: &[u8]) {
        if self.wave.0 == id && self.wave.1.is_some() {
            return;
        }
        self.wave = (id, None);
        if let Ok(b) = std::fs::read(wave_path(id)) {
            if b.len() > 50 {
                self.wave = (id, Some(b));
                return;
            }
        }
        let bytes = bytes.to_vec();
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let w = sources::wave_peaks(bytes);
            if let Some(w) = &w {
                let p = wave_path(id);
                if let Some(d) = p.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                let _ = std::fs::write(&p, w);
            }
            let _ = tx.send(Msg::Wave(id, w));
            ctx.request_repaint();
        });
    }

    // ------------------------------------------------------------ sources
    pub(crate) fn handle_paths(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        self.files_busy = true;
        self.sec = Sec::Files;
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let (files, folders) = scan_paths(paths);
            let _ = tx.send(Msg::Added(files, folders));
            ctx.request_repaint();
        });
    }

    fn pick_files(&mut self, folder: bool) {
        if self.files_busy {
            return;
        }
        self.files_busy = true;
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let paths: Vec<PathBuf> = if folder {
                rfd::FileDialog::new().set_title("Add a folder of music").pick_folder().into_iter().collect()
            } else {
                rfd::FileDialog::new()
                    .set_title("Add audio files")
                    .add_filter("Audio", &sources::AUDIO_EXT)
                    .pick_files()
                    .unwrap_or_default()
            };
            let (files, folders) = scan_paths(paths);
            let _ = tx.send(Msg::Added(files, folders));
            ctx.request_repaint();
        });
    }

    pub(crate) fn rescan(&mut self) {
        let folders = self.store.folders.clone();
        let known: HashSet<i64> = self.store.files.iter().map(|e| e.id).collect();
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let mut found: Vec<PathBuf> = Vec::new();
            for f in &folders {
                sources::scan_folder(&PathBuf::from(f), 0, &mut found);
            }
            let new: Vec<Ext> = found
                .iter()
                .filter(|p| !known.contains(&store::hash_id(&format!("file:{}", p.to_string_lossy()))))
                .map(|p| sources::ext_for_file(p))
                .collect();
            if !new.is_empty() {
                let _ = tx.send(Msg::Scanned(new));
                ctx.request_repaint();
            }
        });
    }

    fn add_exts(&mut self, exts: Vec<Ext>) -> usize {
        let mut n = 0;
        for e in exts {
            if self.store.files.iter().any(|x| x.id == e.id) {
                continue;
            }
            self.register(&e.to_version());
            self.store.files.push(e);
            n += 1;
        }
        if n > 0 {
            self.store_dirty = true;
        }
        n
    }

    pub(crate) fn drain_extra(&mut self, m: Msg) {
        match m {
            Msg::Added(files, folders) => {
                self.files_busy = false;
                for f in folders {
                    if !self.store.folders.contains(&f) {
                        self.store.folders.push(f);
                    }
                }
                let n = self.add_exts(files);
                self.store_dirty = true;
                self.set_note(&format!("ADDED {} FILES", n));
            }
            Msg::Scanned(files) => {
                let n = self.add_exts(files);
                if n > 0 {
                    self.set_note(&format!("FOUND {} NEW FILES", n));
                }
            }
            Msg::YtAdded(r) => {
                self.yt_busy = false;
                match r {
                    Ok(e) => {
                        self.yt_msg = format!("ADDED: {}", e.title);
                        self.yt_in.clear();
                        self.register(&e.to_version());
                        if !self.store.yt.iter().any(|x| x.id == e.id) {
                            self.store.yt.insert(0, e);
                            self.store_dirty = true;
                        }
                    }
                    Err(e) => {
                        api::log(&format!("youtube: {}", e));
                        self.yt_msg = format!("ERROR: {}", e);
                    }
                }
            }
            Msg::Sc(r) => {
                self.sc_busy = false;
                match r {
                    Ok(list) => {
                        self.sc_msg = format!("{} TRACKS", list.len());
                        for e in &list {
                            self.register(&e.to_version());
                        }
                        // the real titles, covers and lengths: four at a time, shown as they arrive
                        let srcs: Vec<String> = list.iter().take(40).map(|e| e.src.clone()).collect();
                        for w in 0..4 {
                            let mine: Vec<String> = srcs.iter().skip(w).step_by(4).cloned().collect();
                            let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
                            std::thread::spawn(move || {
                                for s in mine {
                                    if let Ok(e) = sources::yt_info(&s) {
                                        let _ = tx.send(Msg::ScMeta(e));
                                        ctx.request_repaint();
                                    }
                                }
                            });
                        }
                        self.sc_results = list;
                    }
                    Err(e) => {
                        api::log(&format!("soundcloud: {}", e));
                        self.sc_msg = format!("ERROR: {}", e);
                    }
                }
            }
            Msg::ScMeta(e) => {
                for slot in self.sc_results.iter_mut().chain(self.store.sc.iter_mut()) {
                    if slot.id == e.id {
                        slot.title = e.title.clone();
                        slot.artist = e.artist.clone();
                        slot.dur = e.dur;
                        slot.cover = e.cover.clone();
                    }
                }
                if self.store.sc.iter().any(|x| x.id == e.id) {
                    self.store_dirty = true;
                }
            }
            Msg::YtTool(r) => {
                self.yt_busy = false;
                match r {
                    Ok(p) => self.yt_msg = format!("YT-DLP READY: {}", p.display()),
                    Err(e) => {
                        api::log(&format!("yt-dlp download: {}", e));
                        self.yt_msg = format!("ERROR: {}", e);
                    }
                }
            }
            Msg::Wave(id, w) => {
                if self.wave.0 == id {
                    self.wave.1 = w;
                }
            }
            Msg::Exported(r) => {
                self.exporting = false;
                match r {
                    Ok(p) => self.set_note(&format!("SAVED: {}", p)),
                    Err(e) => self.set_err(format!("EXPORT FAILED: {}", e)),
                }
            }
            Msg::Stems(_, r) => {
                self.stem_busy = 0;
                match r {
                    Ok(()) => self.set_note("STEMS READY - USE THE STEMS ROW IN MORE"),
                    Err(e) if e == "cancelled" => self.set_note("SPLIT CANCELLED"),
                    Err(e) => self.set_err(format!("STEMS FAILED: {}", e)),
                }
            }
            Msg::StemTool(r) => {
                self.stem_busy = 0;
                match r {
                    Ok(()) => self.set_note("STEM TOOL READY - NOW SPLIT A TRACK"),
                    Err(e) => self.set_err(format!("STEM TOOL DOWNLOAD FAILED: {}", e)),
                }
            }
            Msg::Lookup(name, info, tracks) => {
                self.look_busy = false;
                match info {
                    Ok(text) => {
                        if let Some(i) = self.find_tune(&name) {
                            self.store.tunes[i].info = text;
                            self.store_dirty = true;
                        }
                    }
                    Err(e) => self.set_note(&format!("NO WRITER INFO FOUND ({})", e)),
                }
                self.look_for = name;
                self.look_tracks = tracks;
            }
            Msg::Chart(name, r) => {
                self.chart_busy = false;
                let Some(i) = self.find_tune(&name) else { return };
                match r {
                    Ok((chart, key, composer)) => {
                        let t = &mut self.store.tunes[i];
                        t.chart = chart;
                        if t.key.is_empty() {
                            t.key = key;
                        }
                        if t.info.is_empty() && !composer.is_empty() {
                            t.info = format!("{} - written by {}", name, composer);
                        }
                        self.chart_sugg.clear();
                        self.store_dirty = true;
                        self.set_note(&format!("FOUND THE CHANGES FOR {}", name.to_uppercase()));
                    }
                    Err((e, sugg)) => {
                        self.chart_sugg = sugg;
                        self.set_note(&format!("NO CHART FOR {}: {}", name.to_uppercase(), e.to_uppercase()));
                    }
                }
            }
            Msg::Live(q, r) => {
                self.chart_busy = false;
                if q == self.live_q {
                    match r {
                        Ok(found) => {
                            self.live = Some(found);
                            self.chart_sugg.clear();
                        }
                        Err((_, sugg)) => self.chart_sugg = sugg,
                    }
                }
            }
            Msg::Beats(id, r) => {
                self.beat_busy = false;
                match r {
                    Some((period, phase)) => {
                        self.beat = Some((id, period, phase));
                        self.mt_gen += 1;
                        self.set_note(&format!("FOUND THE BEAT: {:.0} BPM - NOW TURN ON LOCK", 60.0 / period));
                    }
                    None => self.set_note("COULDN'T FIND A STEADY BEAT IN THIS TRACK"),
                }
            }
            _ => {}
        }
    }

    // -------------------------------------------------------------- tunes
    fn add_version(&mut self, i: usize, v: Version) {
        if i >= self.store.tunes.len() {
            return;
        }
        self.register(&v);
        let t = &mut self.store.tunes[i];
        if t.versions.iter().any(|x| x.id == v.id) {
            let n = t.name.clone();
            self.set_note(&format!("ALREADY IN {}", n.to_uppercase()));
            return;
        }
        let n = t.name.clone();
        t.versions.push(v);
        self.store_dirty = true;
        self.set_note(&format!("ADDED TO {}", n.to_uppercase()));
    }

    fn new_tune_named(&mut self, name: &str) -> usize {
        if let Some(i) = self.find_tune(name) {
            return i;
        }
        self.store.tunes.push(Tune { name: name.trim().to_string(), ..Default::default() });
        self.store_dirty = true;
        self.store.tunes.len() - 1
    }

    pub(crate) fn apply_extra(&mut self, a: Action) {
        match a {
            Action::Section(s) => {
                self.show_log = false;
                self.show_eq = false;
                self.show_cache = false;
                self.sec = s;
                self.serial += 1;
                if s != Sec::Tunes {
                    self.pick = None;
                }
            }
            Action::AddFiles => self.pick_files(false),
            Action::AddFolder => self.pick_files(true),
            Action::Rescan => {
                self.rescan();
                self.set_note("LOOKING FOR NEW FILES...");
            }
            Action::ForgetFolder(i) => {
                if i < self.store.folders.len() {
                    self.store.folders.remove(i);
                    self.store_dirty = true;
                }
            }
            Action::AddYt => {
                if self.yt_busy {
                    return;
                }
                let Some(vid) = sources::yt_video_id(&self.yt_in) else {
                    self.yt_msg = "THAT DOESN'T LOOK LIKE A YOUTUBE LINK".to_string();
                    return;
                };
                if sources::ytdlp_path().is_none() {
                    self.yt_msg = "PRESS GET YT-DLP FIRST (ONE-TIME SETUP)".to_string();
                    return;
                }
                self.yt_busy = true;
                self.yt_msg = "READING THE VIDEO...".to_string();
                let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
                std::thread::spawn(move || {
                    let _ = tx.send(Msg::YtAdded(sources::yt_info(&vid)));
                    ctx.request_repaint();
                });
            }
            Action::ScGo => {
                if self.sc_busy {
                    return;
                }
                let q = self.sc_in.trim().to_string();
                if q.is_empty() {
                    return;
                }
                if sources::ytdlp_path().is_none() {
                    self.sc_msg = "PRESS GET YT-DLP FIRST (ONE-TIME SETUP)".to_string();
                    return;
                }
                self.sc_busy = true;
                self.sc_msg = "LOOKING...".to_string();
                let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
                std::thread::spawn(move || {
                    let r = if q.starts_with("http") { sources::sc_list(&q) } else { sources::sc_search(&q) };
                    let _ = tx.send(Msg::Sc(r));
                    ctx.request_repaint();
                });
            }
            Action::ScBrowser => {
                use std::sync::atomic::Ordering;
                let n = (sources::SC_BROWSER.load(Ordering::Relaxed) + 1) % 4;
                sources::SC_BROWSER.store(n, Ordering::Relaxed);
                self.dirty = true;
                self.sc_msg = if n == 0 {
                    "BROWSER LOGIN OFF".to_string()
                } else {
                    format!("USING YOUR {} SIGN-IN. CLOSE THAT BROWSER IF IT FAILS.", sources::SC_BROWSERS[n as usize])
                };
            }
            Action::LookStyle => self.look_style = (self.look_style + 1) % 4,
            Action::ScKeep(id) => {
                if let Some(e) = self.sc_results.iter().find(|e| e.id == id).cloned() {
                    if !self.store.sc.iter().any(|x| x.id == id) {
                        self.store.sc.insert(0, e);
                        self.store_dirty = true;
                    }
                    self.set_note("KEPT IN MY SOUNDCLOUD");
                }
            }
            Action::Offline(on) => {
                self.offline = on;
                self.dirty = true;
                if on {
                    self.sec = if crate::PRACTICE { Sec::Files } else { Sec::Sc };
                } else {
                    self.sec = Sec::Tidal;
                }
                self.serial += 1;
            }
            Action::GetYtDlp => {
                if self.yt_busy {
                    return;
                }
                self.yt_busy = true;
                self.yt_msg = "DOWNLOADING YT-DLP...".to_string();
                let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
                std::thread::spawn(move || {
                    let _ = tx.send(Msg::YtTool(sources::ytdlp_download()));
                    ctx.request_repaint();
                });
            }
            Action::RemoveExt(id) => {
                self.store.files.retain(|e| e.id != id);
                self.store.yt.retain(|e| e.id != id);
                self.store.sc.retain(|e| e.id != id);
                self.store_dirty = true;
            }

            // ---- tunes
            Action::PickTune(t) => {
                self.set_note(&format!("PICK A TUNE FOR: {}", t.title.to_uppercase()));
                self.pick = Some(t);
                self.sec = Sec::Tunes;
                self.tune_open = None;
                self.serial += 1;
            }
            Action::AddToTune(i, t) => {
                let v = self.version_of(&t);
                self.add_version(i, v);
                self.pick = None;
            }
            Action::NewTuneFrom(t) => {
                let name = tune_name_from(&t.title);
                let i = self.new_tune_named(&name);
                let v = self.version_of(&t);
                self.add_version(i, v);
                self.pick = None;
                self.tune_open = Some(i);
                self.sec = Sec::Tunes;
                self.serial += 1;
            }
            Action::NewTune => {
                let name = self.new_tune.trim().to_string();
                if name.is_empty() {
                    return;
                }
                let i = self.new_tune_named(&name);
                self.new_tune.clear();
                if let Some(t) = self.pick.take() {
                    let v = self.version_of(&t);
                    self.add_version(i, v);
                }
                self.tune_open = Some(i);
                self.serial += 1;
            }
            Action::OpenTune(i) => {
                self.tune_open = i;
                if i.is_some() {
                    self.chart_pick = i;
                    self.chart_live = false;
                }
                self.edit_ver = None;
                self.del_arm = false;
                self.chart_edit = false;
                self.ed.id = 0;
                self.serial += 1;
            }
            Action::RemoveTune(i) => {
                if !self.del_arm {
                    self.del_arm = true;
                    return;
                }
                self.del_arm = false;
                if i < self.store.tunes.len() {
                    self.store.tunes.remove(i);
                    self.store_dirty = true;
                }
                self.tune_open = None;
                self.edit_ver = None;
                self.serial += 1;
            }
            Action::RemoveVersion(i, vi) => {
                if let Some(t) = self.store.tunes.get_mut(i) {
                    if vi < t.versions.len() {
                        t.versions.remove(vi);
                        self.store_dirty = true;
                    }
                }
                self.edit_ver = None;
            }
            Action::PlayTune(i, shuffled) => {
                let vs: Vec<Version> = match self.store.tunes.get(i) {
                    Some(t) => t.versions.clone(),
                    None => return,
                };
                if vs.is_empty() {
                    self.set_note("NO VERSIONS YET - ADD ONE FIRST");
                    return;
                }
                for v in &vs {
                    self.register(v);
                }
                let tracks: Vec<Track> = vs.iter().map(|v| v.to_track()).collect();
                if shuffled {
                    self.apply(Action::PlayShuffled(tracks));
                } else {
                    self.apply(Action::Play(tracks, 0));
                }
            }
            Action::AddCurrentToTune(i) => match self.cur_track() {
                Some(t) => {
                    let v = self.version_of(&t);
                    self.add_version(i, v);
                }
                None => self.set_note("PLAY A RECORDING FIRST"),
            },
            Action::FindChart(i, alt) => {
                let Some(t) = self.store.tunes.get(i) else { return };
                let name = t.name.clone();
                if self.chart_busy {
                    return;
                }
                self.chart_busy = true;
                self.chart_sugg.clear();
                let query = alt.unwrap_or_else(|| name.clone());
                let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
                std::thread::spawn(move || {
                    let _ = tx.send(Msg::Chart(name, sources::find_chart(&query)));
                    ctx.request_repaint();
                });
            }
            Action::LiveChart(title, alt) => {
                if self.chart_busy {
                    return;
                }
                self.chart_busy = true;
                self.live_q = title.clone();
                self.live = None;
                self.chart_sugg.clear();
                let query = alt.unwrap_or_else(|| title.clone());
                let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
                std::thread::spawn(move || {
                    let _ = tx.send(Msg::Live(title, sources::find_chart(&query)));
                    ctx.request_repaint();
                });
            }
            Action::SaveLive => {
                let (Some(t), Some((chart, key, composer))) = (self.cur_track(), self.live.clone()) else { return };
                let name = tune_name_from(&t.title);
                let i = self.new_tune_named(&name);
                let v = self.version_of(&t);
                self.add_version(i, v);
                let tu = &mut self.store.tunes[i];
                tu.chart = chart;
                tu.key = key;
                if !composer.is_empty() {
                    tu.info = format!("{} - written by {}", name, composer);
                }
                self.chart_live = false;
                self.chart_pick = Some(i);
                self.store_dirty = true;
                self.set_note("SAVED TO YOUR TUNES");
            }
            Action::ToggleNumerals => self.chart_rn = !self.chart_rn,
            Action::Status(i, n) => {
                if let Some(t) = self.store.tunes.get_mut(i) {
                    t.status = n.min(3);
                    self.store_dirty = true;
                }
            }
            Action::Knob(k, d) => {
                let v = self.knob_val(k) + d as f32 * knob_step(k);
                self.knob_set(k, v);
            }
            Action::SetKnob(k, text) if matches!(k, Knob::Beats) && text.contains('/') => {
                // a whole meter typed in one go, like 11/17
                let mut it = text.split('/').map(|s| s.trim().parse::<f32>().ok());
                match (it.next().flatten(), it.next().flatten()) {
                    (Some(b), Some(u)) => {
                        self.knob_set(Knob::Beats, b);
                        self.knob_set(Knob::Unit, u);
                    }
                    _ => self.set_note("TYPE A METER LIKE 11/17"),
                }
            }
            Action::SetKnob(k, text) => {
                let v = if matches!(k, Knob::LoopA | Knob::LoopB) {
                    parse_time(&text)
                } else {
                    text.trim().trim_end_matches('%').trim().parse::<f32>().ok()
                };
                match v {
                    _ if text.trim().is_empty() => {}
                    Some(v) => self.knob_set(k, v),
                    None => self.set_note("THAT IS NOT A NUMBER"),
                }
            }
            Action::Opt(o) => self.opt(o),
            Action::LookUp(i) => {
                if self.look_busy {
                    return;
                }
                let Some(t) = self.store.tunes.get(i) else { return };
                let name = t.name.clone();
                let known: Vec<String> = self
                    .store
                    .tunes
                    .iter()
                    .flat_map(|t| t.versions.iter().filter(|v| v.stars >= 3).map(|v| v.artist.clone()))
                    .collect();
                self.look_busy = true;
                self.look_for = name.clone();
                self.look_tracks.clear();
                let (api, tx, ctx) = (self.api.clone(), self.tx.clone(), self.ctx.clone());
                let want_style = self.look_style;
                // something typed beside LOOK UP is searched instead of the tune's own name
                let typed = self.look_q.trim().to_string();
                let tune_name = name.clone();
                let name = if typed.is_empty() { name } else { typed };
                std::thread::spawn(move || {
                    let info = sources::lookup_composer(&name);
                    // a tune in the jazz standards list is jazz (and that list knows who wrote it)
                    let listed = sources::find_chart(&name).ok().map(|c| c.2);
                    let style = match want_style {
                        0 => u8::from(listed.is_some()),
                        1 => 1,
                        2 => 2,
                        _ => 0,
                    };
                    let composer = listed.unwrap_or_else(|| info.as_ref().map(|s| s.to_string()).unwrap_or_default());
                    let composer = composer.split(|c| c == ',' || c == '\n').next().unwrap_or("").replace("Written by", "");
                    let tracks = api.popular(&name, &known, style, composer.trim()).unwrap_or_default();
                    let _ = tx.send(Msg::Lookup(tune_name, info, tracks));
                    ctx.request_repaint();
                });
            }

            // ---- sections and loops
            Action::SaveSection => {
                let (Some(a), Some(b)) = (self.loop_a, self.loop_b) else {
                    self.set_note("SET LOOP A AND B FIRST");
                    return;
                };
                let Some(t) = self.cur_track() else { return };
                let typed = self.sec_name.trim().to_string();
                let list = self.store.sections.entry(t.id).or_default();
                if list.iter().any(|s| (s.a - a).abs() < 0.05 && (s.b - b).abs() < 0.05) {
                    self.set_note("THIS LOOP IS ALREADY SAVED");
                    return;
                }
                let name = if typed.is_empty() { format!("LOOP {}", list.len() + 1) } else { typed };
                list.push(Section { name: name.clone(), a, b });
                self.sec_name.clear();
                self.store_dirty = true;
                self.set_note(&format!("SAVED: {}", name.to_uppercase()));
            }
            Action::GoSection(i) => {
                let Some(t) = self.cur_track() else { return };
                let Some(s) = self.store.sections.get(&t.id).and_then(|l| l.get(i)).cloned() else { return };
                self.practice = true;
                self.loop_a = Some(s.a);
                self.loop_b = Some(s.b);
                self.loop_on = true;
                self.sync_loop();
                self.player.send(Cmd::Seek(s.a));
            }
            Action::DeleteSection(i) => {
                if let Some(t) = self.cur_track() {
                    if let Some(l) = self.store.sections.get_mut(&t.id) {
                        if i < l.len() {
                            l.remove(i);
                            self.store_dirty = true;
                        }
                        if l.is_empty() {
                            self.store.sections.remove(&t.id);
                        }
                    }
                }
            }

            // ---- practice tools
            Action::ToggleFocus => {
                self.focus_mode = !self.focus_mode;
                if self.focus_mode {
                    let has_chart = self.chart_tune().map(|i| !self.store.tunes[i].chart.is_empty()).unwrap_or(false);
                    self.rtab = if has_chart { 1 } else { 0 };
                }
                self.set_note(if self.focus_mode { "FOCUS: JUST THE PLAYER AND TOOLS" } else { "FULL LAYOUT" });
            }
            Action::ToggleMore => self.more = !self.more,
            Action::Continue => {
                let Some(l) = self.store.last.clone() else { return };
                self.register(&l.version);
                self.queue = Arc::new(vec![l.version.to_track()]);
                self.orig_queue = None;
                self.shuffle = false;
                self.speed = if l.speed == 0 { 100 } else { l.speed.clamp(25, 150) };
                self.practice = true;
                self.play_index(0);
                if l.pos > 3.0 {
                    let t = (l.pos - 2.0).max(0.0);
                    if self.buffering {
                        self.pending_seek = Some(t);
                    } else {
                        self.player.send(Cmd::Seek(t));
                    }
                }
            }
            Action::Trainer => {
                self.trainer = !self.trainer;
                self.passes = 0;
                if self.trainer {
                    self.practice = true;
                    self.set_note(&format!("EVERY {} LOOPS THE SPEED GOES UP {}%", self.trainer_n, self.trainer_step));
                } else {
                    self.set_note("SPEED TRAINER OFF");
                }
                self.sync_loop();
            }
            Action::TapTempo => {
                let now = Instant::now();
                if let Some(l) = self.taps.last() {
                    if now.duration_since(*l) > Duration::from_millis(2000) {
                        self.taps.clear();
                    }
                }
                self.taps.push(now);
                if self.taps.len() > 8 {
                    self.taps.remove(0);
                }
                if self.taps.len() >= 3 {
                    let span = now.duration_since(self.taps[0]).as_secs_f32();
                    let n = (self.taps.len() - 1) as f32;
                    let heard = 60.0 / (span / n).max(0.05);
                    let factor = if self.practice { self.speed as f32 / 100.0 } else { 1.0 };
                    self.knob_set(Knob::Bpm, (heard / factor).round());
                } else {
                    self.set_note("KEEP TAPPING...");
                }
            }
            Action::CountIn => {
                self.count_in = match self.count_in {
                    0 => 2,
                    2 => 4,
                    _ => 0,
                };
                if self.count_in > 0 && self.bpm == 0 {
                    self.set_note("COUNT-IN NEEDS A TEMPO - TAP IT OR SET BPM");
                }
                self.sync_loop();
            }
            Action::Chan => {
                self.chan = (self.chan + 1) % CHAN_NAMES.len() as u32;
                self.practice = true;
                self.sync_loop();
                self.set_note(&format!("EAR: {}", CHAN_NAMES[self.chan as usize]));
            }
            Action::Transpose(d) => {
                self.semis = (self.semis + d).clamp(-12, 12);
                if self.cur.is_some() {
                    self.practice = true;
                }
                self.sync_loop();
                self.set_note(&format!("PITCH {:+} SEMITONES (SPEED UNCHANGED)", self.semis));
            }
            Action::Export => {
                let (Some(a), Some(b)) = (self.loop_a, self.loop_b) else {
                    self.set_note("SET LOOP A AND B FIRST");
                    return;
                };
                let Some(t) = self.cur_track() else { return };
                if self.exporting {
                    return;
                }
                let bytes = match self.srcmap.get(&t.id) {
                    Some(Src::File(p)) => std::fs::read(p).ok(),
                    _ => cache::get(t.id),
                };
                let Some(bytes) = bytes else {
                    self.set_note("PLAY IT ONCE FIRST SO IT IS STORED, THEN EXPORT");
                    return;
                };
                let dir = dirs::audio_dir().unwrap_or_else(api::config_dir).join("Tidalite loops");
                let name = safe_name(&format!("{} - {} ({}s-{}s).wav", t.artist, t.title, a as u32, b as u32));
                let path = dir.join(name);
                self.exporting = true;
                let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
                std::thread::spawn(move || {
                    let r = sources::export_wav(bytes, a, b, &path).map(|_| path.display().to_string());
                    let _ = tx.send(Msg::Exported(r));
                    ctx.request_repaint();
                });
            }

            // ---- diary
            Action::SaveEntry => {
                let today = if self.diary_sel == 0 { store::today_str() } else { store::date_str(self.diary_sel) };
                let default_mins = self.store.secs_on(&today) / 60;
                let mins = self.f_mins.trim().parse::<u32>().unwrap_or(default_mins);
                let bpm = self.f_bpm.trim().parse::<u32>().unwrap_or(0);
                let tune = if self.f_tune.trim().is_empty() { self.practice_label() } else { self.f_tune.trim().to_string() };
                // a logged day with no tracked time still counts on the calendar
                if mins > 0 && self.store.secs_on(&today) == 0 {
                    *self.store.day_mut(&today).secs.entry(tune.clone()).or_insert(0) += mins * 60;
                }
                self.store.entries.push(Entry { date: today, tune, mins, bpm, note: self.f_note.trim().to_string() });
                self.f_mins.clear();
                self.f_bpm.clear();
                self.f_note.clear();
                self.f_tune.clear();
                self.show_form = false;
                self.ed.id = 0;
                self.store_dirty = true;
                self.set_note("LOGGED IN THE DIARY");
            }
            Action::RightTab(n) => self.rtab = n,
            Action::StemGet => {
                if self.stem_busy != 0 {
                    return;
                }
                self.stem_busy = 1;
                let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
                std::thread::spawn(move || {
                    let _ = tx.send(Msg::StemTool(stems::download_tool()));
                    ctx.request_repaint();
                });
            }
            Action::StemSplit => {
                if self.stem_busy != 0 {
                    return;
                }
                let Some(t) = self.cur_track() else {
                    self.set_note("PLAY A TRACK FIRST");
                    return;
                };
                let bytes = match self.srcmap.get(&t.id) {
                    Some(Src::File(p)) => std::fs::read(p).ok(),
                    _ => cache::get(t.id),
                };
                let Some(bytes) = bytes else {
                    self.set_note("PLAY THE TRACK ONCE SO IT IS STORED, THEN SPLIT");
                    return;
                };
                self.stem_busy = 2;
                self.set_note("SPLITTING - KEEP PRACTICING, IT TAKES A FEW MINUTES");
                let id = t.id;
                let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
                std::thread::spawn(move || {
                    let _ = tx.send(Msg::Stems(id, stems::separate(bytes, id)));
                    ctx.request_repaint();
                });
            }
            Action::StemCancel => stems::CANCEL.store(true, std::sync::atomic::Ordering::Relaxed),
            Action::StemToggle(i) => {
                if i < 4 {
                    let mut on = self.stem_on;
                    on[i] = !on[i];
                    self.apply_mix(on);
                }
            }
            Action::StemSolo(i) => {
                if i < 4 {
                    let mut on = [false; 4];
                    on[i] = true;
                    self.apply_mix(on);
                }
            }
            Action::StemAll => self.apply_mix([true; 4]),
            Action::StemClear => {
                let _ = std::fs::remove_dir_all(api::config_dir().join("stems"));
                self.stem_data = None;
                self.stem_on = [true; 4];
                self.set_note("ALL STEMS DELETED");
            }
            Action::Pomo => {
                if self.pomo == 1 || self.pomo == 2 {
                    self.pomo = 0;
                    self.pomo_end = None;
                    self.set_note("FOCUS TIMER STOPPED");
                } else {
                    self.pomo_n = if self.pomo == 3 { self.pomo_n + 1 } else { 1 };
                    self.pomo = 1;
                    self.pomo_end = Some(Instant::now() + Duration::from_secs(self.pomo_focus as u64 * 60));
                    self.set_note(&format!("FOCUS {} OF {} - ONE THING AT A TIME", self.pomo_n, self.pomo_cycles));
                }
                self.pomo_flash = None;
            }
            Action::TimerPanel => self.timer_open = !self.timer_open,
            Action::PlayAlong(name) => {
                let q: String = format!("{} backing track play along", name)
                    .bytes()
                    .map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{:02X}", b) })
                    .collect();
                let _ = webbrowser::open(&format!("https://www.youtube.com/results?search_query={}", q));
                self.set_note("OPENED YOUTUBE IN YOUR BROWSER");
            }
            Action::MetroPanel => self.metro_open = !self.metro_open,
            Action::ImportIreal(target) => {
                let text = self.ireal_in.trim().to_string();
                match crate::chart::parse_ireal_url(&text) {
                    Err(e) => self.set_err(format!("CAN'T READ THAT LINK: {}", e)),
                    Ok(songs) if songs.is_empty() => self.set_note("NO SONGS IN THAT LINK"),
                    Ok(songs) => {
                        let mut count = 0;
                        let single = songs.len() == 1;
                        let mut last = None;
                        for s in songs {
                            let idx = match target {
                                Some(t) if single && t < self.store.tunes.len() => t,
                                _ => self.new_tune_named(&s.title),
                            };
                            let tune = &mut self.store.tunes[idx];
                            tune.chart = crate::chart::to_friendly(&s.measures, &s.time);
                            if tune.key.is_empty() {
                                tune.key = s.key.clone();
                            }
                            if tune.bpm == 0 {
                                tune.bpm = s.bpm;
                            }
                            if tune.info.is_empty() && !s.composer.is_empty() {
                                tune.info = format!("Written by {}", s.composer);
                            }
                            last = Some(idx);
                            count += 1;
                        }
                        self.store_dirty = true;
                        self.ireal_in.clear();
                        self.chart_edit = false;
                        self.ed.id = 0;
                        if let Some(i) = last {
                            self.tune_open = Some(i);
                        }
                        self.set_note(&format!("IMPORTED {} CHART{}", count, if count == 1 { "" } else { "S" }));
                    }
                }
            }
            _ => {}
        }
    }

    /// Focus -> rest -> wait for you. No sound: the window just flashes.
    fn pomo_tick(&mut self) {
        let Some(end) = self.pomo_end else { return };
        if Instant::now() < end {
            return;
        }
        self.pomo_flash = Some(Instant::now());
        if self.pomo_sound {
            self.player.send(Cmd::Chime);
        }
        self.ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational));
        if self.pomo == 1 {
            let date = store::today_str();
            self.store.day_mut(&date).pomos += 1;
            self.store_dirty = true;
            if self.cur.is_some() && !self.stopped && !self.paused {
                self.apply(Action::PauseBtn);
            }
            if self.pomo_n >= self.pomo_cycles {
                self.pomo = 0;
                self.pomo_end = None;
                self.set_note(&format!("ALL {} FOCUS BLOCKS DONE - GOOD WORK", self.pomo_cycles));
            } else {
                self.pomo = 2;
                self.pomo_end = Some(Instant::now() + Duration::from_secs(self.pomo_break as u64 * 60));
                self.set_note(&format!("REST FOR {} MIN. STRETCH, WATER.", self.pomo_break));
            }
        } else {
            self.pomo = 3;
            self.pomo_end = None;
            self.set_note(&format!("REST OVER - START FOCUS {} OF {} WHEN READY", self.pomo_n + 1, self.pomo_cycles));
        }
    }

    /// Play the mix of the stems that are switched on, from the same spot.
    fn apply_mix(&mut self, on: [bool; 4]) {
        let Some(t) = self.cur_track() else { return };
        if self.stopped || self.buffering {
            self.set_note("PLAY THE TRACK FIRST");
            return;
        }
        if !stems::have_stems(t.id) {
            self.set_note("SPLIT THIS TRACK FIRST");
            return;
        }
        if self.stem_data.as_ref().map(|d| d.0) != Some(t.id) {
            match stems::load(t.id) {
                Ok(s) => self.stem_data = Some((t.id, s)),
                Err(e) => {
                    self.set_err(format!("STEMS: {}", e));
                    return;
                }
            }
        }
        if let Some((_, s)) = &self.stem_data {
            let bytes = stems::mix_wav(s, on);
            self.player.send(Cmd::Swap(bytes, self.pos(), self.paused));
            self.stem_on = on;
        }
    }

    fn knob_val(&self, k: Knob) -> f32 {
        match k {
            Knob::Focus => self.pomo_focus as f32,
            Knob::Rest => self.pomo_break as f32,
            Knob::Blocks => self.pomo_cycles as f32,
            Knob::Loops => self.trainer_n as f32,
            Knob::Step => self.trainer_step as f32,
            Knob::Speed => self.speed as f32,
            Knob::LoopA => self.loop_a.unwrap_or(0.0),
            Knob::LoopB => self.loop_b.unwrap_or(0.0),
            Knob::Bpm => (if self.bpm > 0 { self.bpm } else { self.mt_bpm }) as f32,
            Knob::Beats => self.mt.beats as f32,
            Knob::Unit => self.mt.unit as f32,
            Knob::GapPlay => self.mt.gap_play as f32,
            Knob::GapMute => self.mt.gap_mute as f32,
            Knob::RampBars => self.mt.ramp_bars as f32,
            Knob::RampBpm => self.mt.ramp_bpm as f32,
            Knob::Shift => self.mt.shift_ms as f32,
        }
    }

    fn knob_set(&mut self, k: Knob, v: f32) {
        let u = |lo: f32, hi: f32| v.round().clamp(lo, hi) as u32;
        match k {
            Knob::Focus => self.pomo_focus = u(1.0, 180.0),
            Knob::Rest => self.pomo_break = u(1.0, 60.0),
            Knob::Blocks => self.pomo_cycles = u(1.0, 16.0),
            Knob::Loops => self.trainer_n = u(1.0, 99.0),
            Knob::Step => self.trainer_step = u(1.0, 50.0),
            Knob::Speed => self.apply(Action::Speed(u(25.0, 150.0))),
            Knob::LoopA => self.apply(Action::SetAAt(v.max(0.0))),
            Knob::LoopB => self.apply(Action::SetBAt(v.max(0.0))),
            Knob::Bpm => {
                let b = u(30.0, 300.0);
                self.mt_bpm = b;
                self.mt_add = 0.0;
                self.mt_step_at = Instant::now();
                self.bpm = b;
                if self.cur.is_some() && !self.stopped {
                    self.set_bpm(b as i32);
                }
            }
            Knob::Beats => {
                self.mt.beats = u(1.0, 32.0);
                self.mt.preset(0);
                self.mt_group = 0;
            }
            Knob::Unit => self.mt.unit = u(1.0, 64.0) as u8,
            Knob::GapPlay => self.mt.gap_play = u(0.0, 32.0),
            Knob::GapMute => self.mt.gap_mute = u(1.0, 16.0),
            Knob::RampBars => self.mt.ramp_bars = u(0.0, 32.0),
            Knob::RampBpm => self.mt.ramp_bpm = u(1.0, 30.0),
            Knob::Shift => {
                self.mt.shift_ms = v.round().clamp(-300.0, 300.0) as i32;
                self.mt_gen += 1;
            }
        }
    }

    fn opt(&mut self, o: Opt) {
        match o {
            Opt::MetroOn => {
                self.mt.on = !self.mt.on;
                self.mt_add = 0.0;
                self.mt_step_at = Instant::now();
                if !self.mt.on {
                    // silence it right now (the click loop plays forever until told to stop)
                    self.player.send(Cmd::Pcm(0, Vec::new(), Vec::new(), 0.0));
                    self.mt_vis = None;
                }
                self.mt_sig = 0;
            }
            Opt::Preset(n) => {
                self.mt.preset(n);
                self.mt_group = 0;
            }
            Opt::UnitSet(n) => self.mt.unit = n.clamp(1, 64),
            Opt::Group => {
                let gs = band::groupings(self.mt.beats);
                if gs.is_empty() {
                    return;
                }
                self.mt_group = (self.mt_group + 1) % (gs.len() + 1);
                match self.mt_group {
                    0 => self.mt.preset(0),
                    k => self.mt.apply_group(&gs[k - 1]),
                }
            }
            Opt::Level(i) => {
                self.mt_group = 0;
                let l = &mut self.mt.levels[i.min(31)];
                *l = if *l == 0 { 3 } else { *l - 1 };
            }
            Opt::Sub => self.mt.sub = (self.mt.sub + 1) % 4,
            Opt::Follow => self.mt.follow = !self.mt.follow,
            Opt::Sync => {
                self.mt.sync = !self.mt.sync;
                if self.mt.sync && self.beat.map(|b| b.0) != self.cur_track().map(|t| t.id) {
                    self.opt(Opt::FindBeats);
                }
            }
            Opt::Double | Opt::Half => {
                if let Some(b) = self.beat.as_mut() {
                    b.1 *= if matches!(o, Opt::Double) { 0.5 } else { 2.0 };
                    self.mt_gen += 1;
                }
            }
            Opt::FindBeats => {
                let Some(t) = self.cur_track() else {
                    self.set_note("PLAY A TRACK FIRST");
                    return;
                };
                if self.beat_busy {
                    return;
                }
                let bytes = match self.srcmap.get(&t.id) {
                    Some(Src::File(p)) => std::fs::read(p).ok(),
                    _ => cache::get(t.id),
                };
                let Some(bytes) = bytes else {
                    self.set_note("PLAY IT ONCE FIRST SO IT IS STORED, THEN FIND THE BEAT");
                    return;
                };
                self.beat_busy = true;
                self.set_note("LISTENING FOR THE BEAT...");
                let (tx, ctx, id) = (self.tx.clone(), self.ctx.clone(), t.id);
                std::thread::spawn(move || {
                    let _ = tx.send(Msg::Beats(id, band::detect_beats(bytes)));
                    ctx.request_repaint();
                });
            }
            Opt::Band => {
                if self.band_on {
                    self.band_stop();
                } else {
                    self.band_start();
                }
            }
            Opt::BandStyleSet(i) => self.band_style = i,
            Opt::Part(i) => self.band_parts[i.min(2)] = !self.band_parts[i.min(2)],
            Opt::PomoSound => self.pomo_sound = !self.pomo_sound,
        }
    }

    fn set_bpm(&mut self, b: i32) {
        let b = b.clamp(30, 300) as u32;
        self.bpm = b;
        if let Some(t) = self.cur_track() {
            self.store.bpm.insert(t.id, b);
            if let Some(i) = self.store.tune_of(t.id) {
                if self.store.tunes[i].bpm == 0 {
                    self.store.tunes[i].bpm = b;
                }
            }
            self.store_dirty = true;
        }
        self.sync_loop();
    }

    pub(crate) fn chart_for(&mut self, text: &str) -> crate::chart::Chart {
        if self.chart_cache.0 != text {
            self.chart_cache = (text.to_string(), crate::chart::parse_friendly(text));
        }
        self.chart_cache.1.clone()
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.flush_practice();
        self.capture_last();
        self.store.save();
    }
}
