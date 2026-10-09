//! Audio engine on its own thread (rodio + symphonia: FLAC / AAC / MP3 decode in pure Rust).
//! Also taps the decoded samples so the UI can draw a real spectrum analyzer.

use crate::api::log;
use crate::decode::SymSource;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::source::SeekError;
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub enum Cmd {
    Play(Vec<u8>),
    /// replace the audio with another version of the same track (stem mix): keep the position,
    /// no count-in. (bytes, position in s, paused)
    Swap(Vec<u8>, f32, bool),
    Pause,
    Resume,
    Seek(f32),
    Volume(f32),
    /// Metronome click every n seconds (None = off).
    Metro(Option<f32>),
    Stop,
}

#[derive(Default)]
pub struct Shared {
    pub pos: f32,
    pub ended: bool,
    pub rate: u32,
    pub error: Option<String>,
    /// Set if the audio device could not be opened at all (playback is impossible).
    pub dead: Option<String>,
}

/// Most recent mono samples (-1..1), newest last. Read by the UI for the spectrum.
#[derive(Default)]
pub struct Viz {
    pub samples: Vec<f32>,
    pub rate: u32,
}

pub struct Player {
    tx: Sender<Cmd>,
    pub shared: Arc<Mutex<Shared>>,
    pub viz: Arc<Mutex<Viz>>,
    pub eq: Arc<EqShared>,
    pub ctl: Arc<Ctl>,
}

// ------------------------------------------------------- practice controls
/// A-B loop, playback speed and the exact play position. Shared between the UI and the
/// audio thread. Everything here is purely local: nothing is ever reported to Tidal.
pub struct Ctl {
    loop_on: AtomicBool,
    a_ms: AtomicU32,
    b_ms: AtomicU32,
    speed: AtomicU32,
    pos_fr: AtomicU64,
    rate: AtomicU32,
    semis: AtomicI32,
    chan: AtomicU32,
    wraps: AtomicU32,
    count_in: AtomicU32,
    bpm10: AtomicU32,
}

impl Ctl {
    fn new() -> Ctl {
        Ctl {
            loop_on: AtomicBool::new(false),
            a_ms: AtomicU32::new(0),
            b_ms: AtomicU32::new(0),
            speed: AtomicU32::new(100),
            pos_fr: AtomicU64::new(0),
            rate: AtomicU32::new(44100),
            semis: AtomicI32::new(0),
            chan: AtomicU32::new(0),
            wraps: AtomicU32::new(0),
            count_in: AtomicU32::new(0),
            bpm10: AtomicU32::new(0),
        }
    }

    /// Transpose in semitones (speed is unaffected).
    pub fn set_semis(&self, n: i32) {
        self.semis.store(n.clamp(-12, 12), Ordering::Relaxed);
    }

    /// 0 stereo, 1 left, 2 right, 3 mono, 4 center-cancel, 5 bass only
    pub fn set_chan(&self, m: u32) {
        self.chan.store(m, Ordering::Relaxed);
    }

    /// Clicks before each loop pass (0 = off) at this tempo (of the original recording).
    pub fn set_count_in(&self, beats: u32, bpm: f32) {
        self.count_in.store(beats, Ordering::Relaxed);
        self.bpm10.store((bpm.clamp(0.0, 400.0) * 10.0) as u32, Ordering::Relaxed);
    }

    /// How many times the loop has wrapped around so far.
    pub fn wraps(&self) -> u32 {
        self.wraps.load(Ordering::Relaxed)
    }

    pub fn speed_pct(&self) -> u32 {
        self.speed.load(Ordering::Relaxed)
    }

    pub fn set_loop(&self, on: bool, a: f32, b: f32) {
        self.a_ms.store((a.max(0.0) * 1000.0) as u32, Ordering::Relaxed);
        self.b_ms.store((b.max(0.0) * 1000.0) as u32, Ordering::Relaxed);
        self.loop_on.store(on && b > a + 0.05, Ordering::Relaxed);
    }

    /// Playback speed in percent (pitch is preserved).
    pub fn set_speed(&self, pct: u32) {
        self.speed.store(pct.clamp(25, 150), Ordering::Relaxed);
    }

    /// Current position in seconds (of the original recording, regardless of speed).
    pub fn pos(&self) -> f32 {
        let r = self.rate.load(Ordering::Relaxed).max(1) as f64;
        (self.pos_fr.load(Ordering::Relaxed) as f64 / r) as f32
    }
}

/// Wraps the decoder: counts frames (exact position) and jumps from B back to A.
struct Looper<S: Source<Item = i16>> {
    inner: S,
    ctl: Arc<Ctl>,
    ch: u16,
    rate: u32,
    frame: u64,
    sub: u16,
}

impl<S: Source<Item = i16>> Looper<S> {
    fn new(inner: S, ctl: Arc<Ctl>) -> Looper<S> {
        let ch = inner.channels().max(1);
        let rate = inner.sample_rate().max(1);
        ctl.rate.store(rate, Ordering::Relaxed);
        ctl.pos_fr.store(0, Ordering::Relaxed);
        Looper { inner, ctl, ch, rate, frame: 0, sub: 0 }
    }

    fn jump_to_a(&mut self) -> bool {
        let a_ms = self.ctl.a_ms.load(Ordering::Relaxed) as u64;
        if self.inner.try_seek(Duration::from_millis(a_ms)).is_ok() {
            self.frame = a_ms * self.rate as u64 / 1000;
            self.sub = 0;
            self.ctl.wraps.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }
}

impl<S: Source<Item = i16>> Iterator for Looper<S> {
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        if self.sub == 0 && self.ctl.loop_on.load(Ordering::Relaxed) {
            let b = self.ctl.b_ms.load(Ordering::Relaxed) as u64 * self.rate as u64 / 1000;
            if self.frame == b {
                self.jump_to_a();
            }
        }
        let s = match self.inner.next() {
            Some(s) => s,
            None => {
                // the loop end is at (or past) the end of the file
                if self.ctl.loop_on.load(Ordering::Relaxed) && self.jump_to_a() {
                    self.inner.next()?
                } else {
                    return None;
                }
            }
        };
        self.sub += 1;
        if self.sub >= self.ch {
            self.sub = 0;
            self.frame += 1;
            self.ctl.pos_fr.store(self.frame, Ordering::Relaxed);
        }
        Some(s)
    }
}

impl<S: Source<Item = i16>> Source for Looper<S> {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(pos)?;
        self.frame = (pos.as_secs_f64() * self.rate as f64) as u64;
        self.sub = 0;
        self.ctl.pos_fr.store(self.frame, Ordering::Relaxed);
        Ok(())
    }
}

// ----------------------------------------------------------- slow-down (WSOLA)
const SN: usize = 2048; // window length (frames)
const SH: usize = 1024; // output hop
const SD: usize = 384; // search tolerance

/// Pitch-preserving time stretch (waveform-similarity overlap-add). At 100% it is a pure
/// pass-through and costs nothing.
struct Stretch<S: Source<Item = i16>> {
    inner: S,
    ctl: Arc<Ctl>,
    ch: usize,
    win: Vec<f32>,
    inb: Vec<f32>,
    in_base: usize,
    eof: bool,
    nat: Option<usize>,
    pos_f: f64,
    tail: Vec<f32>,
    out: Vec<i16>,
    oi: usize,
    raw_s: usize,
    stretching: bool,
}

impl<S: Source<Item = i16>> Stretch<S> {
    fn new(inner: S, ctl: Arc<Ctl>) -> Stretch<S> {
        let ch = inner.channels().max(1) as usize;
        let win = (0..SN).map(|j| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * j as f32 / SN as f32).cos()).collect();
        Stretch {
            inner,
            ctl,
            ch,
            win,
            inb: Vec::new(),
            in_base: 0,
            eof: false,
            nat: None,
            pos_f: 0.0,
            tail: vec![0.0; (SN - SH) * ch],
            out: Vec::new(),
            oi: 0,
            raw_s: 0,
            stretching: false,
        }
    }

    fn reset(&mut self) {
        self.inb.clear();
        self.in_base = 0;
        self.raw_s = 0;
        self.eof = false;
        self.nat = None;
        self.pos_f = 0.0;
        for t in self.tail.iter_mut() {
            *t = 0.0;
        }
        self.out.clear();
        self.oi = 0;
    }

    fn get(&self, frame: usize, k: usize) -> f32 {
        if frame < self.in_base {
            return 0.0;
        }
        let i = (frame - self.in_base) * self.ch + k;
        self.inb.get(i).copied().unwrap_or(0.0)
    }

    fn fill(&mut self, need_end: usize) {
        while !self.eof && self.in_base + self.inb.len() / self.ch < need_end {
            for _ in 0..(1024 * self.ch) {
                match self.inner.next() {
                    Some(s) => {
                        self.inb.push(s as f32 / 32768.0);
                        self.raw_s += 1;
                    }
                    None => {
                        self.eof = true;
                        break;
                    }
                }
            }
        }
    }

    /// Candidate start (within +-SD of `ideal`) whose waveform best continues the previous window.
    fn best(&self, nat: usize, ideal: usize) -> usize {
        const L: usize = 512;
        let lo = ideal.saturating_sub(SD).max(self.in_base);
        let hi = ideal + SD;
        let mut best_c = ideal.max(self.in_base);
        let mut best_s = f32::MIN;
        let mut c = lo;
        while c <= hi {
            let mut dot = 0.0f32;
            let mut e = 1e-3f32;
            let mut j = 0;
            while j < L {
                let r = self.get(nat + j, 0);
                let x = self.get(c + j, 0);
                dot += r * x;
                e += x * x;
                j += 4;
            }
            let sc = dot / e.sqrt();
            if sc > best_s {
                best_s = sc;
                best_c = c;
            }
            c += 1;
        }
        best_c
    }

    /// Produce the next SH frames into `out`. Returns false at the end of the stream.
    fn step(&mut self, speed: f64) -> bool {
        let ch = self.ch;
        let ideal = self.pos_f.max(self.in_base as f64) as usize;
        let mut need_end = ideal + SD + SN + 8;
        if let Some(n) = self.nat {
            need_end = need_end.max(n + SN);
        }
        self.fill(need_end);
        let avail_end = self.in_base + self.inb.len() / ch;
        if self.eof && ideal >= avail_end {
            return false;
        }
        let c = match self.nat {
            None => ideal,
            Some(n) => self.best(n, ideal),
        };
        self.out.clear();
        self.oi = 0;
        for j in 0..SH {
            for k in 0..ch {
                let v = self.get(c + j, k) * self.win[j] + self.tail[j * ch + k];
                self.out.push((v * 32768.0).clamp(-32768.0, 32767.0) as i16);
            }
        }
        for j in 0..(SN - SH) {
            for k in 0..ch {
                self.tail[j * ch + k] = self.get(c + SH + j, k) * self.win[SH + j];
            }
        }
        self.nat = Some(c + SH);
        self.pos_f += SH as f64 * speed;
        // drop input that can no longer be used
        let keep = (self.pos_f as usize).saturating_sub(SD).min(c + SH);
        if keep > self.in_base {
            let n = ((keep - self.in_base) * ch).min(self.inb.len());
            self.inb.drain(0..n);
            self.in_base += n / ch;
        }
        true
    }
}

impl<S: Source<Item = i16>> Iterator for Stretch<S> {
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        if self.oi < self.out.len() {
            let s = self.out[self.oi];
            self.oi += 1;
            return Some(s);
        }
        let pct = self.ctl.speed.load(Ordering::Relaxed);
        let semis = self.ctl.semis.load(Ordering::Relaxed);
        let plain = pct == 100 && semis == 0;
        if plain && self.stretching {
            // leaving slow-down: hand over the already-buffered audio so nothing is skipped
            self.stretching = false;
            if let Some(n) = self.nat {
                let ch = self.ch;
                self.out.clear();
                self.oi = 0;
                for j in 0..SH {
                    for k in 0..ch {
                        let v = self.get(n + j, k) * self.win[j] + self.tail[j * ch + k];
                        self.out.push((v * 32768.0).clamp(-32768.0, 32767.0) as i16);
                    }
                }
                let skip = (n + SH).saturating_sub(self.in_base) * ch;
                let rest: Vec<i16> = self.inb.iter().skip(skip).map(|v| (v * 32768.0).clamp(-32768.0, 32767.0) as i16).collect();
                self.out.extend(rest);
                self.inb.clear();
                self.nat = None;
                return self.next();
            }
        }
        if plain || (!self.stretching && self.raw_s % self.ch != 0) {
            self.stretching = false;
            let s = self.inner.next()?;
            self.raw_s += 1;
            return Some(s);
        }
        if !self.stretching {
            // switch from pass-through to stretching, continuing from the current input position
            self.stretching = true;
            self.inb.clear();
            self.in_base = self.raw_s / self.ch;
            self.eof = false;
            self.nat = None;
            self.pos_f = self.in_base as f64;
            for t in self.tail.iter_mut() {
                *t = 0.0;
            }
        }
        let tempo = pct as f64 / 100.0 / 2f64.powf(semis as f64 / 12.0);
        if !self.step(tempo) {
            return None;
        }
        self.oi = 1;
        Some(self.out[0])
    }
}

impl<S: Source<Item = i16>> Source for Stretch<S> {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(pos)?;
        self.reset();
        Ok(())
    }
}

/// Transpose: resamples the (tempo-corrected) audio so the pitch moves by `semis` semitones.
struct Resample<S: Source<Item = i16>> {
    inner: S,
    ctl: Arc<Ctl>,
    ch: usize,
    a: Vec<f32>,
    b: Vec<f32>,
    t: f64,
    ready: bool,
    eof: bool,
    done: bool,
    out: Vec<i16>,
    oi: usize,
}

impl<S: Source<Item = i16>> Resample<S> {
    fn new(inner: S, ctl: Arc<Ctl>) -> Resample<S> {
        let ch = inner.channels().max(1) as usize;
        Resample {
            inner,
            ctl,
            ch,
            a: Vec::new(),
            b: Vec::new(),
            t: 0.0,
            ready: false,
            eof: false,
            done: false,
            out: Vec::new(),
            oi: 0,
        }
    }

    fn read_frame(&mut self) -> Option<Vec<f32>> {
        let mut f = Vec::with_capacity(self.ch);
        for k in 0..self.ch {
            match self.inner.next() {
                Some(s) => f.push(s as f32),
                None => {
                    if k == 0 {
                        return None;
                    }
                    f.push(0.0);
                }
            }
        }
        Some(f)
    }

    fn reset(&mut self) {
        self.ready = false;
        self.eof = false;
        self.done = false;
        self.out.clear();
        self.oi = 0;
    }
}

impl<S: Source<Item = i16>> Iterator for Resample<S> {
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        if self.oi < self.out.len() {
            let s = self.out[self.oi];
            self.oi += 1;
            return Some(s);
        }
        let semis = self.ctl.semis.load(Ordering::Relaxed);
        if semis == 0 {
            self.ready = false;
            self.done = false;
            self.eof = false;
            return self.inner.next();
        }
        if self.done {
            return None;
        }
        let r = 2f64.powf(semis as f64 / 12.0);
        if !self.ready {
            self.a = self.read_frame()?;
            self.b = self.read_frame().unwrap_or_else(|| self.a.clone());
            self.t = 0.0;
            self.ready = true;
            self.eof = false;
        }
        self.out.clear();
        self.oi = 0;
        let t = self.t as f32;
        for k in 0..self.ch {
            let v = self.a[k] + (self.b[k] - self.a[k]) * t;
            self.out.push(v.clamp(-32768.0, 32767.0) as i16);
        }
        self.t += r;
        while self.t >= 1.0 {
            self.a = std::mem::take(&mut self.b);
            match self.read_frame() {
                Some(f) => self.b = f,
                None => {
                    if self.eof {
                        self.done = true;
                    }
                    self.eof = true;
                    self.b = self.a.clone();
                }
            }
            self.t -= 1.0;
        }
        self.oi = 1;
        Some(self.out[0])
    }
}

impl<S: Source<Item = i16>> Source for Resample<S> {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(pos)?;
        self.reset();
        Ok(())
    }
}

/// Channel tools for hearing a part: left only, right only, mono, vocals/center removed, bass only.
struct Chan<S: Source<Item = i16>> {
    inner: S,
    ctl: Arc<Ctl>,
    ch: usize,
    mode: u32,
    tick: u32,
    pend: Option<i16>,
    lp: [Biquad; 2],
    z: [[f32; 2]; 2],
}

impl<S: Source<Item = i16>> Chan<S> {
    fn new(inner: S, ctl: Arc<Ctl>) -> Chan<S> {
        let ch = inner.channels().max(1) as usize;
        let rate = inner.sample_rate().max(8000) as f32;
        // 4th-order low-pass (two cascaded RBJ biquads) at 320 Hz
        let f0 = 320.0f32.min(rate * 0.4);
        let w0 = 2.0 * std::f32::consts::PI * f0 / rate;
        let (sw, cw) = (w0.sin(), w0.cos());
        let alpha = sw / (2.0 * 0.707);
        let a0 = 1.0 + alpha;
        let bq = Biquad {
            b0: (1.0 - cw) / 2.0 / a0,
            b1: (1.0 - cw) / a0,
            b2: (1.0 - cw) / 2.0 / a0,
            a1: -2.0 * cw / a0,
            a2: (1.0 - alpha) / a0,
        };
        Chan { inner, ctl, ch, mode: 0, tick: 0, pend: None, lp: [bq, bq], z: [[0.0; 2]; 2] }
    }

    fn filt(&mut self, x: f32) -> f32 {
        let mut x = x;
        for i in 0..2 {
            let k = self.lp[i];
            let z = &mut self.z[i];
            let y = k.b0 * x + z[0];
            z[0] = k.b1 * x - k.a1 * y + z[1];
            z[1] = k.b2 * x - k.a2 * y;
            x = y;
        }
        x
    }
}

impl<S: Source<Item = i16>> Iterator for Chan<S> {
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        if let Some(s) = self.pend.take() {
            return Some(s);
        }
        self.tick = self.tick.wrapping_add(1);
        if self.tick % 256 == 0 {
            self.mode = self.ctl.chan.load(Ordering::Relaxed);
        }
        if self.mode == 0 || self.ch != 2 {
            return self.inner.next();
        }
        let l = self.inner.next()? as f32;
        let r = self.inner.next().map(|v| v as f32).unwrap_or(l);
        let (ol, or) = match self.mode {
            1 => (l, l),
            2 => (r, r),
            3 => {
                let m = (l + r) * 0.5;
                (m, m)
            }
            4 => {
                let sd = (l - r) * 0.7;
                (sd, sd)
            }
            5 => {
                let y = self.filt((l + r) * 0.5) * 1.6;
                (y, y)
            }
            _ => (l, r),
        };
        self.pend = Some(or.clamp(-32768.0, 32767.0) as i16);
        Some(ol.clamp(-32768.0, 32767.0) as i16)
    }
}

impl<S: Source<Item = i16>> Source for Chan<S> {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.pend = None;
        self.z = [[0.0; 2]; 2];
        self.inner.try_seek(pos)
    }
}

/// Click track: `beats` clicks `interval` seconds apart (first one accented), mono 44.1 kHz.
fn click_buf(beats: u32, interval: f32, tail: bool) -> Vec<i16> {
    let rate = 44100.0f32;
    let step = (interval * rate) as usize;
    let total = step * beats as usize + if tail { 0 } else { 0 };
    let mut v = vec![0i16; total.max(1)];
    for b in 0..beats as usize {
        let f = if b == 0 { 1600.0 } else { 1100.0 };
        let start = b * step;
        for i in 0..(rate * 0.03) as usize {
            if start + i >= v.len() {
                break;
            }
            let t = i as f32 / rate;
            let s = (2.0 * std::f32::consts::PI * f * t).sin() * (-t * 140.0).exp();
            v[start + i] = (s * 14000.0) as i16;
        }
    }
    v
}

// ------------------------------------------------------------- equalizer
pub const EQ_FREQS: [f32; 10] = [31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];

/// Gains (dB per band) and on/off switch, shared between the UI and the audio thread.
pub struct EqShared {
    gains: Mutex<[f32; 10]>,
    on: AtomicBool,
    version: AtomicU32,
}

impl EqShared {
    fn new() -> EqShared {
        EqShared { gains: Mutex::new([0.0; 10]), on: AtomicBool::new(false), version: AtomicU32::new(1) }
    }

    pub fn set(&self, gains: [f32; 10], on: bool) {
        if let Ok(mut g) = self.gains.lock() {
            *g = gains;
        }
        self.on.store(on, Ordering::Relaxed);
        self.version.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Clone, Copy, Default)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

/// 10-band peaking equalizer (RBJ biquads). Passes audio through untouched when switched off or flat.
struct Equalizer<S: Source<Item = i16>> {
    inner: S,
    sh: Arc<EqShared>,
    chans: usize,
    rate: f32,
    coef: [Biquad; 10],
    active: [bool; 10],
    z: Vec<[[f32; 2]; 10]>,
    ver: u32,
    enabled: bool,
    any: bool,
    tick: u32,
    idx: usize,
}

impl<S: Source<Item = i16>> Equalizer<S> {
    fn new(inner: S, sh: Arc<EqShared>) -> Equalizer<S> {
        let chans = inner.channels().max(1) as usize;
        let rate = inner.sample_rate() as f32;
        let mut e = Equalizer {
            inner,
            sh,
            chans,
            rate,
            coef: [Biquad::default(); 10],
            active: [false; 10],
            z: vec![[[0.0; 2]; 10]; chans],
            ver: 0,
            enabled: false,
            any: false,
            tick: 0,
            idx: 0,
        };
        e.refresh();
        e
    }

    fn refresh(&mut self) {
        self.enabled = self.sh.on.load(Ordering::Relaxed);
        let v = self.sh.version.load(Ordering::Relaxed);
        if v == self.ver {
            return;
        }
        self.ver = v;
        let gains = self.sh.gains.lock().map(|g| *g).unwrap_or([0.0; 10]);
        for b in 0..10 {
            let f = EQ_FREQS[b];
            let g = gains[b];
            self.active[b] = g.abs() > 0.05 && f < self.rate * 0.45;
            if self.active[b] {
                let a = 10f32.powf(g / 40.0);
                let w0 = 2.0 * std::f32::consts::PI * f / self.rate;
                let alpha = w0.sin() / (2.0 * 1.4);
                let cw = w0.cos();
                let a0 = 1.0 + alpha / a;
                self.coef[b] = Biquad {
                    b0: (1.0 + alpha * a) / a0,
                    b1: (-2.0 * cw) / a0,
                    b2: (1.0 - alpha * a) / a0,
                    a1: (-2.0 * cw) / a0,
                    a2: (1.0 - alpha / a) / a0,
                };
            }
        }
        self.any = self.active.iter().any(|x| *x);
    }
}

impl<S: Source<Item = i16>> Iterator for Equalizer<S> {
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        let s = self.inner.next()?;
        self.tick += 1;
        if self.tick >= 512 {
            self.tick = 0;
            self.refresh();
        }
        let c = self.idx;
        self.idx = (self.idx + 1) % self.chans;
        if !self.enabled || !self.any {
            return Some(s);
        }
        let mut x = s as f32;
        for b in 0..10 {
            if self.active[b] {
                let k = self.coef[b];
                let z = &mut self.z[c][b];
                let y = k.b0 * x + z[0];
                z[0] = k.b1 * x - k.a1 * y + z[1];
                z[1] = k.b2 * x - k.a2 * y;
                x = y;
            }
        }
        Some(x.clamp(-32768.0, 32767.0) as i16)
    }
}

impl<S: Source<Item = i16>> Source for Equalizer<S> {
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        for ch in self.z.iter_mut() {
            *ch = [[0.0; 2]; 10];
        }
        self.inner.try_seek(pos)
    }
}

/// Pass-through source that copies one channel of samples into `Viz`.
struct Tap<S: Source<Item = i16>> {
    inner: S,
    viz: Arc<Mutex<Viz>>,
    scratch: Vec<f32>,
    ch: u16,
    phase: u16,
}

impl<S: Source<Item = i16>> Tap<S> {
    fn new(inner: S, viz: Arc<Mutex<Viz>>) -> Tap<S> {
        let ch = inner.channels().max(1);
        Tap { inner, viz, scratch: Vec::with_capacity(256), ch, phase: 0 }
    }

    fn flush(&mut self) {
        if let Ok(mut v) = self.viz.lock() {
            v.samples.extend(self.scratch.drain(..));
            let len = v.samples.len();
            if len > 4096 {
                v.samples.drain(0..len - 4096);
            }
        } else {
            self.scratch.clear();
        }
    }
}

impl<S: Source<Item = i16>> Iterator for Tap<S> {
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        let s = self.inner.next()?;
        self.phase += 1;
        if self.phase >= self.ch {
            self.phase = 0;
            self.scratch.push(s as f32 / 32768.0);
            if self.scratch.len() >= 256 {
                self.flush();
            }
        }
        Some(s)
    }
}

impl<S: Source<Item = i16>> Source for Tap<S> {
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(pos)
    }
}

/// Play a count-in on its own sink and wait for it to finish.
fn play_clicks(handle: &OutputStreamHandle, beats: u32, interval: f32, vol: f32) {
    if let Ok(c) = Sink::try_new(handle) {
        c.set_volume((vol * 1.2).min(1.0));
        c.append(rodio::buffer::SamplesBuffer::new(1, 44100, click_buf(beats, interval.clamp(0.15, 4.0), false)));
        c.sleep_until_end();
    }
}

/// Open the default output device; if that fails, try every device the system lists.
fn open_output() -> Result<(OutputStream, OutputStreamHandle), String> {
    let why = match OutputStream::try_default() {
        Ok(x) => {
            log("audio: default output device opened");
            return Ok(x);
        }
        Err(e) => format!("default device failed: {}", e),
    };
    log(&format!("audio: {}", why));
    let host = rodio::cpal::default_host();
    if let Ok(devs) = host.output_devices() {
        for d in devs {
            let name = d.name().unwrap_or_else(|_| "?".to_string());
            match OutputStream::try_from_device(&d) {
                Ok(x) => {
                    log(&format!("audio: opened device '{}'", name));
                    return Ok(x);
                }
                Err(e) => log(&format!("audio: device '{}' failed: {}", name, e)),
            }
        }
    }
    Err(why)
}

impl Player {
    pub fn new() -> Player {
        let (tx, rx) = channel::<Cmd>();
        let shared = Arc::new(Mutex::new(Shared::default()));
        let viz = Arc::new(Mutex::new(Viz::default()));
        let sh = shared.clone();
        let vz = viz.clone();
        let eq = Arc::new(EqShared::new());
        let eq_t = eq.clone();
        let ctl = Arc::new(Ctl::new());
        let ctl_t = ctl.clone();

        std::thread::spawn(move || {
            let (_stream, handle) = match open_output() {
                Ok(x) => x,
                Err(e) => {
                    let msg = format!("NO AUDIO OUTPUT: {}", e);
                    log(&msg);
                    let mut g = sh.lock().unwrap();
                    g.error = Some(msg.clone());
                    g.dead = Some(msg);
                    return;
                }
            };
            let mut sink: Option<Sink> = None;
            let mut vol = 0.64f32;
            let mut metro: Option<Sink> = None;
            let mut user_paused = false;
            let mut last_wraps = 0u32;

            loop {
                match rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(cmd @ (Cmd::Play(_) | Cmd::Swap(..))) => {
                        let (bytes, swap) = match cmd {
                            Cmd::Play(b) => (b, None),
                            Cmd::Swap(b, p, paused) => (b, Some((p, paused))),
                            _ => continue,
                        };
                        log(&format!("player: received {} bytes", bytes.len()));
                        if let Some(s) = sink.take() {
                            s.stop();
                        }
                        let sink_res = Sink::try_new(&handle);
                        let (dtx, drx) = channel::<Result<SymSource, String>>();
                        std::thread::spawn(move || {
                            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| SymSource::new(bytes)));
                            let _ = dtx.send(match r {
                                Ok(x) => x,
                                Err(_) => Err("decoder panicked".to_string()),
                            });
                        });
                        let dec_res = match drx.recv_timeout(Duration::from_secs(15)) {
                            Ok(r) => r,
                            Err(_) => Err("decoder timed out".to_string()),
                        };
                        match (sink_res, dec_res) {
                            (Ok(s), Ok(dec)) => {
                                let rate = dec.sample_rate();
                                log(&format!("decoder ok: {} Hz, {} ch", rate, dec.channels()));
                                s.set_volume(vol);
                                {
                                    let mut v = vz.lock().unwrap();
                                    v.samples.clear();
                                    v.rate = rate;
                                }
                                let chain =
                                    Resample::new(Stretch::new(Looper::new(dec, ctl_t.clone()), ctl_t.clone()), ctl_t.clone());
                                let chain = Chan::new(chain, ctl_t.clone());
                                let beats = ctl_t.count_in.load(Ordering::Relaxed);
                                let bpm = ctl_t.bpm10.load(Ordering::Relaxed) as f32 / 10.0;
                                let counting = swap.is_none() && beats > 0 && bpm > 0.0;
                                if counting {
                                    s.pause();
                                }
                                s.append(Tap::new(Equalizer::new(chain, eq_t.clone()), vz.clone()));
                                last_wraps = ctl_t.wraps.load(Ordering::Relaxed);
                                user_paused = false;
                                if counting {
                                    let iv = 60.0 / bpm * 100.0 / ctl_t.speed.load(Ordering::Relaxed).max(25) as f32;
                                    play_clicks(&handle, beats, iv, vol);
                                    s.play();
                                }
                                if let Some((p, paused)) = swap {
                                    let d = Duration::try_from_secs_f32(p.max(0.0)).unwrap_or_default();
                                    let _ = s.try_seek(d);
                                    if paused {
                                        s.pause();
                                        user_paused = true;
                                    }
                                }
                                sink = Some(s);
                                let mut g = sh.lock().unwrap();
                                g.ended = false;
                                g.error = None;
                                g.pos = 0.0;
                                g.rate = rate;
                            }
                            (_, Err(e)) => {
                                let m = format!("DECODE: can't decode audio ({})", e);
                                log(&m);
                                sh.lock().unwrap().error = Some(m);
                            }
                            (Err(e), _) => {
                                let m = format!("Audio device error: {}", e);
                                log(&m);
                                sh.lock().unwrap().error = Some(m);
                            }
                        }
                    }
                    Ok(Cmd::Pause) => {
                        user_paused = true;
                        if let Some(s) = &sink {
                            s.pause();
                        }
                    }
                    Ok(Cmd::Resume) => {
                        user_paused = false;
                        if let Some(s) = &sink {
                            s.play();
                        }
                    }
                    Ok(Cmd::Seek(t)) => {
                        if let Some(s) = &sink {
                            let d = Duration::try_from_secs_f32(t.max(0.0)).unwrap_or_default();
                            let _ = s.try_seek(d);
                        }
                    }
                    Ok(Cmd::Volume(v)) => {
                        vol = v;
                        if let Some(s) = &sink {
                            s.set_volume(v);
                        }
                        if let Some(m) = &metro {
                            m.set_volume((v * 0.8).min(1.0));
                        }
                    }
                    Ok(Cmd::Metro(iv)) => {
                        if let Some(m) = metro.take() {
                            m.stop();
                        }
                        if let Some(iv) = iv {
                            if let Ok(m) = Sink::try_new(&handle) {
                                let buf = click_buf(1, iv.clamp(0.2, 3.0), true);
                                let n = (iv.clamp(0.2, 3.0) * 44100.0) as usize;
                                let mut v = buf;
                                v.resize(n.max(1), 0);
                                m.set_volume((vol * 0.8).min(1.0));
                                m.append(rodio::buffer::SamplesBuffer::new(1, 44100, v).repeat_infinite());
                                metro = Some(m);
                            }
                        }
                    }
                    Ok(Cmd::Stop) => {
                        user_paused = false;
                        if let Some(s) = sink.take() {
                            s.stop();
                        }
                        ctl_t.pos_fr.store(0, Ordering::Relaxed);
                        vz.lock().unwrap().samples.clear();
                        let mut g = sh.lock().unwrap();
                        g.ended = false;
                        g.pos = 0.0;
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }

                let w = ctl_t.wraps.load(Ordering::Relaxed);
                if w != last_wraps {
                    last_wraps = w;
                    let beats = ctl_t.count_in.load(Ordering::Relaxed);
                    let bpm = ctl_t.bpm10.load(Ordering::Relaxed) as f32 / 10.0;
                    if beats > 0 && bpm > 0.0 && !user_paused {
                        if let Some(s) = &sink {
                            let spd = ctl_t.speed.load(Ordering::Relaxed).max(25) as f32;
                            let rate = ctl_t.rate.load(Ordering::Relaxed).max(1) as f32;
                            // let the audio that is already buffered ahead of the wrap play out first
                            let wait = 3500.0 * 100.0 / spd / rate * 1000.0 + 60.0;
                            std::thread::sleep(Duration::from_millis(wait as u64));
                            s.pause();
                            play_clicks(&handle, beats, 60.0 / bpm * 100.0 / spd, vol);
                            let a = ctl_t.a_ms.load(Ordering::Relaxed) as u64;
                            let _ = s.try_seek(Duration::from_millis(a));
                            s.play();
                            last_wraps = ctl_t.wraps.load(Ordering::Relaxed);
                        }
                    }
                }
                let pos = if sink.is_some() { ctl_t.pos() } else { 0.0 };
                let finished = sink.as_ref().map(|s| s.empty()).unwrap_or(false);
                if finished {
                    sink = None;
                }
                let mut g = sh.lock().unwrap();
                g.pos = pos;
                if finished {
                    if pos < 1.0 {
                        // The decoder gave us (almost) nothing: don't treat that as "track finished",
                        // or the whole queue would race by in silence.
                        log("decoder produced no audio");
                        g.error = Some("DECODE: decoder produced no audio".to_string());
                    } else {
                        g.ended = true;
                    }
                }
            }
        });

        Player { tx, shared, viz, eq, ctl }
    }

    pub fn send(&self, c: Cmd) {
        let _ = self.tx.send(c);
    }
}
