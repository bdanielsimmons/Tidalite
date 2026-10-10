//! Side sounds for practising: metronome patterns, a small synth "band" that plays a chart, and a
//! beat finder that locks the metronome to a recording. Everything is generated as 44.1 kHz mono.

use crate::chart::{self, Chart};
use crate::decode::SymSource;
use rodio::Source;

pub const RATE: f32 = 44100.0;

// ------------------------------------------------------------------ small synth
struct Mix {
    buf: Vec<f32>,
    spb: f32,
    rng: u32,
}

fn freq(midi: i32) -> f32 {
    440.0 * 2f32.powf((midi as f32 - 69.0) / 12.0)
}

impl Mix {
    fn new(beats: f32, bpm: f32) -> Mix {
        let spb = 60.0 / bpm.max(20.0) * RATE;
        Mix { buf: vec![0.0; ((beats * spb).round() as usize).max(1)], spb, rng: 0x9E37_79B9 }
    }

    fn start(&self, beat: f32) -> usize {
        (beat * self.spb) as usize
    }

    fn add(&mut self, start: usize, i: usize, v: f32) {
        let n = self.buf.len();
        self.buf[(start + i) % n] += v;
    }

    fn noise(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    fn bass(&mut self, t: f32, d: f32, midi: i32, vel: f32) {
        let (s, f) = (self.start(t), freq(midi));
        let n = ((d * self.spb) as usize + 3000).min(RATE as usize);
        for i in 0..n {
            let x = i as f32 / RATE;
            let env = (-x * 5.0).exp() * (i as f32 / 120.0).min(1.0);
            let w = (std::f32::consts::TAU * f * x).sin() + 0.35 * (std::f32::consts::TAU * 2.0 * f * x).sin() * (-x * 9.0).exp();
            self.add(s, i, w * env * 0.55 * vel);
        }
    }

    /// Guitar / piano style chord: notes spread over a few milliseconds, then ringing out.
    fn stab(&mut self, t: f32, d: f32, midis: &[i32], vel: f32, decay: f32) {
        let s = self.start(t);
        let n = ((d * self.spb) as usize + 4000).min((RATE * 2.0) as usize);
        for (j, m) in midis.iter().enumerate() {
            let (f, off) = (freq(*m), j * 260);
            for i in 0..n.saturating_sub(off) {
                let x = i as f32 / RATE;
                let env =
                    (-x * decay).exp() * (i as f32 / 150.0).min(1.0) * if i + 2000 > n { (n - i) as f32 / 2000.0 } else { 1.0 };
                let w = (std::f32::consts::TAU * f * x).sin()
                    + 0.5 * (std::f32::consts::TAU * 2.0 * f * x).sin()
                    + 0.22 * (std::f32::consts::TAU * 3.0 * f * x).sin();
                self.add(s + off, i, w * env * 0.1 * vel);
            }
        }
    }

    fn kick(&mut self, t: f32, vel: f32) {
        let s = self.start(t);
        let mut ph = 0.0f32;
        for i in 0..(RATE * 0.25) as usize {
            let x = i as f32 / RATE;
            ph += std::f32::consts::TAU * (48.0 + 95.0 * (-x * 32.0).exp()) / RATE;
            self.add(s, i, ph.sin() * (-x * 14.0).exp() * 0.8 * vel);
        }
    }

    fn snare(&mut self, t: f32, vel: f32) {
        let s = self.start(t);
        for i in 0..(RATE * 0.2) as usize {
            let x = i as f32 / RATE;
            let v = self.noise() * (-x * 22.0).exp() * 0.4 + (std::f32::consts::TAU * 190.0 * x).sin() * (-x * 30.0).exp() * 0.3;
            self.add(s, i, v * vel);
        }
    }

    fn hat(&mut self, t: f32, open: bool, vel: f32) {
        let s = self.start(t);
        let (len, dec) = if open { (0.3, 12.0) } else { (0.07, 70.0) };
        let mut prev = 0.0;
        for i in 0..(RATE * len) as usize {
            let x = i as f32 / RATE;
            let n = self.noise();
            let v = (n - prev) * (-x * dec).exp() * 0.22 * vel;
            prev = n;
            self.add(s, i, v);
        }
    }

    fn ride(&mut self, t: f32, vel: f32) {
        let s = self.start(t);
        let mut prev = 0.0;
        for i in 0..(RATE * 0.6) as usize {
            let x = i as f32 / RATE;
            let n = self.noise();
            let ping: f32 =
                [3300.0f32, 4900.0, 6700.0].iter().map(|f| (std::f32::consts::TAU * f * x).sin()).sum::<f32>() * 0.035;
            let v = (ping * (-x * 7.0).exp() + (n - prev) * (-x * 10.0).exp() * 0.05) * vel;
            prev = n;
            self.add(s, i, v);
        }
    }

    fn rim(&mut self, t: f32, vel: f32) {
        let s = self.start(t);
        for i in 0..(RATE * 0.05) as usize {
            let x = i as f32 / RATE;
            let v = (std::f32::consts::TAU * 1750.0 * x).sin() * (-x * 90.0).exp() * 0.35;
            let n = self.noise() * (-x * 120.0).exp() * 0.1;
            self.add(s, i, (v + n) * vel);
        }
    }

    fn finish(self) -> Vec<i16> {
        let peak = self.buf.iter().fold(0.0f32, |a, v| a.max(v.abs())).max(0.85);
        self.buf.into_iter().map(|v| ((v / peak * 1.15).tanh() * 26000.0) as i16).collect()
    }
}

// --------------------------------------------------------------------- metronome
#[derive(Clone)]
pub struct Metro {
    pub on: bool,
    /// beats per bar
    pub beats: u32,
    /// volume of each beat of the bar, 0 (silent) to 3 (loud); click a beat to step through them
    pub levels: [u8; 32],
    /// the note value of one pulse: 4 (quarters), 8 (eighths) or 16. BPM is always counted in quarter notes, so 22/8 at 120 clicks 22 eighths at 240 a minute
    pub unit: u8,
    /// ticks between the beats: 0 none, 1 eighths, 2 triplets, 3 sixteenths
    pub sub: u8,
    /// play this many bars, then be silent for `gap_mute` (0 = never silent)
    pub gap_play: u32,
    pub gap_mute: u32,
    /// every this many bars get `ramp_bpm` faster (0 = off), up to `ramp_max`
    pub ramp_bars: u32,
    pub ramp_bpm: u32,
    pub ramp_max: u32,
    /// follow the speed setting of the track that is playing
    pub follow: bool,
    /// lock to the beats found in the playing track
    pub sync: bool,
    /// fine shift of the lock, in milliseconds
    pub shift_ms: i32,
}

impl Default for Metro {
    fn default() -> Metro {
        Metro {
            on: false,
            beats: 4,
            levels: {
                let mut l = [2u8; 32];
                l[0] = 3;
                l
            },
            unit: 4,
            sub: 0,
            gap_play: 0,
            gap_mute: 1,
            ramp_bars: 0,
            ramp_bpm: 4,
            ramp_max: 240,
            follow: true,
            sync: false,
            shift_ms: 0,
        }
    }
}

fn click(buf: &mut [i16], start: usize, f: f32, vel: f32) {
    for i in 0..(RATE * 0.035) as usize {
        if start + i >= buf.len() {
            break;
        }
        let t = i as f32 / RATE;
        let s = (std::f32::consts::TAU * f * t).sin() * (-t * 130.0).exp();
        buf[start + i] = buf[start + i].saturating_add((s * 15000.0 * vel) as i16);
    }
}

impl Metro {
    /// Clicks per real minute for a tempo given in quarter notes.
    pub fn pulse(&self, bpm: f32) -> f32 {
        bpm * self.unit as f32 / 4.0
    }

    /// Accent a grouping such as 3+3+3+4: loud on the bar's first beat, medium on each group start, soft in between.
    pub fn apply_group(&mut self, g: &[u32]) {
        for l in self.levels.iter_mut() {
            *l = 1;
        }
        let mut at = 0usize;
        for (i, size) in g.iter().enumerate() {
            if at < 32 {
                self.levels[at] = if i == 0 { 3 } else { 2 };
            }
            at += *size as usize;
        }
    }

    /// Preset volumes: every beat, only 2 and 4 (the jazz way), or only the first beat.
    pub fn preset(&mut self, which: u8) {
        for b in 0..32 {
            self.levels[b] = match which {
                0 => {
                    if b == 0 {
                        3
                    } else {
                        2
                    }
                }
                1 => {
                    if b % 2 == 1 {
                        3
                    } else {
                        0
                    }
                }
                _ => {
                    if b == 0 {
                        3
                    } else {
                        0
                    }
                }
            };
        }
    }
}

/// Ways to split `n` pulses into groups of 2, 3 and 4 (7 = 2+2+3, 22 = 3+3+3+3+3+3+4 ...).
pub fn groupings(n: u32) -> Vec<Vec<u32>> {
    let mut out: Vec<Vec<u32>> = Vec::new();
    for size in [3u32, 2, 4] {
        if n < size {
            continue;
        }
        let (q, r) = (n / size, n % size);
        let mut g = vec![size; q as usize];
        match r {
            0 => {}
            1 => *g.last_mut().unwrap() += 1,
            _ => g.push(r),
        }
        let mut alt = g.clone();
        alt.rotate_right(1);
        for cand in [g, alt] {
            if cand.iter().sum::<u32>() == n && !out.contains(&cand) {
                out.push(cand);
            }
        }
    }
    out
}

/// One cycle of the metronome (a bar, or the play + silent bars of a gap drill), to repeat forever.
pub fn click_loop(m: &Metro, bpm: f32) -> Vec<i16> {
    let beats = m.beats.clamp(1, 32) as usize;
    let spb = 60.0 / m.pulse(bpm.clamp(20.0, 400.0)).clamp(20.0, 1000.0) * RATE;
    let bars = if m.gap_play > 0 { (m.gap_play + m.gap_mute.max(1)) as usize } else { 1 };
    let mut v = vec![0i16; ((bars * beats) as f32 * spb).round() as usize + 1];
    for bar in 0..bars {
        if m.gap_play > 0 && bar >= m.gap_play as usize {
            break;
        }
        for b in 0..beats {
            let at = ((bar * beats + b) as f32 * spb) as usize;
            let lvl = m.levels[b].min(3) as usize;
            if lvl > 0 {
                click(&mut v, at, [0.0, 1000.0, 1250.0, 1650.0][lvl], [0.0, 0.4, 0.7, 1.0][lvl]);
            }
            let div = [1usize, 2, 3, 4][(m.sub as usize).min(3)];
            for k in 1..div {
                click(&mut v, at + (spb * k as f32 / div as f32) as usize, 2300.0, 0.3);
            }
        }
    }
    v
}

/// A bar of plain clicks, played once before the band starts.
pub fn count_in(beats: u32, bpm: f32) -> Vec<i16> {
    click_loop(&Metro { beats, ..Metro::default() }, bpm)
}

// ------------------------------------------------------------------------ chords
struct Chord {
    root: i32,
    third: i32,
    fifth: i32,
    seventh: Option<i32>,
    ext: Option<i32>,
    bass: i32,
}

fn parse_chord(c: &str) -> Option<Chord> {
    // typed changes may use iReal's short spelling (D-7, C^7, Bh7, Co7): read them like Dm7, Cmaj7...
    let c = chart::pretty_chord(c);
    let mut it = c.split('/');
    let main = it.next()?;
    let (root_s, rest) = chart::split_root(main);
    let root = chart::note_index(root_s)?;
    let bass = it.next().and_then(|b| chart::note_index(chart::split_root(b).0)).unwrap_or(root);
    let r = rest;
    let minor = (r.starts_with('m') && !r.starts_with("maj")) || r.starts_with("dim");
    let digits = r.chars().any(|c| c.is_ascii_digit());
    let third = if r.contains("sus") {
        5
    } else if minor {
        3
    } else {
        4
    };
    let fifth = if r.contains("b5") || r.starts_with("dim") {
        6
    } else if r.contains("#5") || r.starts_with("aug") || r.contains('+') {
        8
    } else {
        7
    };
    let seventh = if r.starts_with("dim") && r.contains('7') {
        Some(9)
    } else if r.contains("69") {
        Some(9) // a 6/9 chord has the sixth, not a flat seventh
    } else if r.contains("maj") || r.contains('^') || r.contains("ma7") {
        digits.then_some(11)
    } else if r.contains('7') || r.contains('9') || r.contains("11") || r.contains("13") {
        Some(10)
    } else if r.contains('6') {
        Some(9)
    } else {
        None
    };
    // the colour note that replaces the fifth in the voicing (b9, #9, #11, b13, 13, 11, 9)
    let ext = if r.contains("b9") {
        Some(13)
    } else if r.contains("#9") {
        Some(15)
    } else if r.contains("#11") {
        Some(18)
    } else if r.contains("b13") {
        Some(20)
    } else if r.contains("13") {
        Some(21)
    } else if r.contains("11") {
        Some(17)
    } else if r.contains('9') {
        Some(14)
    } else {
        None
    };
    Some(Chord { root, third, fifth, seventh, ext, bass })
}

fn bass_midi(pc: i32) -> i32 {
    28 + (pc - 28).rem_euclid(12)
}

fn mid_midi(pc: i32) -> i32 {
    55 + (pc - 55).rem_euclid(12)
}

impl Chord {
    fn voicing(&self) -> Vec<i32> {
        let mut v = vec![self.root + self.third, self.root + self.seventh.unwrap_or(self.fifth)];
        v.push(self.root + self.ext.unwrap_or(self.fifth));
        if self.seventh.is_none() {
            v.push(self.root + 12);
        }
        let mut m: Vec<i32> = v.into_iter().map(|x| mid_midi(x.rem_euclid(12))).collect();
        m.sort();
        m.dedup();
        m
    }
}

pub const STYLES: [&str; 5] = ["SWING", "BALLAD", "BOSSA", "SHUFFLE", "STRAIGHT"];

fn swing(t: f32, on: bool) -> f32 {
    if on && (t.fract() - 0.5).abs() < 0.01 {
        t.floor() + 0.667
    } else {
        t
    }
}

/// Render the chart as an endless backing loop: bass, chords and drums in the chosen style.
/// `parts` switches [bass, chords, drums] on or off.
pub fn render(chart: &Chart, order: &[usize], bpm: f32, style: u8, parts: [bool; 3]) -> Vec<i16> {
    let nb = chart.beats.clamp(2, 7) as usize;
    let mut mix = Mix::new((order.len() * nb) as f32, bpm);
    let chords_of = |bar: usize| -> Vec<Option<Chord>> { chart.bars[bar].chords.iter().map(|c| parse_chord(c)).collect() };
    let style = if nb == 3 { 5 } else { style };
    let jazz = style == 0 || style == 5;
    for (oi, bar_ix) in order.iter().enumerate() {
        let cs = chords_of(*bar_ix);
        let next_cs = chords_of(order[(oi + 1) % order.len()]);
        let n = cs.len().max(1);
        let base = (oi * nb) as f32;
        // chord at a beat, where that chord starts and how long it lasts (in beats)
        let span = |k: usize| -> (usize, usize) {
            let a = (k * nb + n - 1) / n;
            let b = (((k + 1) * nb + n - 1) / n).max(a + 1).min(nb);
            (a, b)
        };
        let idx_at = |b: usize| (0..n).find(|k| span(*k).0 <= b && b < span(*k).1).unwrap_or(n - 1);
        for b in 0..nb {
            let k = idx_at(b);
            let (a, e) = span(k);
            let t = base + b as f32;
            let Some(Some(ch)) = cs.get(k) else { continue };
            let next_root = if b + 1 == e {
                let nk = if k + 1 < n { cs.get(k + 1) } else { next_cs.first() };
                nk.and_then(|c| c.as_ref()).map(|c| c.root)
            } else {
                None
            };
            let pos = b - a;
            let len = e - a;

            // ---- bass
            if parts[0] {
                match style {
                    0 | 5 => {
                        let pc = if pos == 0 {
                            ch.bass
                        } else if b + 1 == e && len > 1 {
                            next_root.map_or(ch.root + 7, |r| r + if oi % 2 == 0 { 11 } else { 1 })
                        } else if pos == 1 {
                            ch.root + ch.third
                        } else {
                            ch.root + ch.fifth
                        };
                        let vel = if style == 5 && b > 0 { 0.7 } else { 1.0 };
                        mix.bass(t, 0.95, bass_midi(pc.rem_euclid(12)), vel);
                    }
                    1 => {
                        if pos == 0 {
                            mix.bass(t, len.min(2) as f32, bass_midi(ch.bass), 1.0);
                        } else if pos == 2 {
                            mix.bass(t, 2.0, bass_midi((ch.root + ch.fifth) % 12), 0.8);
                        }
                    }
                    2 => {
                        if pos % 2 == 0 {
                            mix.bass(t, 1.4, bass_midi(ch.bass), 1.0);
                        }
                        if pos % 2 == 1 || len == 1 {
                            mix.bass(t + 0.5, 1.0, bass_midi((ch.root + ch.fifth) % 12), 0.85);
                        }
                    }
                    3 => {
                        let pc = match pos % 4 {
                            0 => ch.bass,
                            1 => ch.root + ch.fifth,
                            2 => ch.root + 12,
                            _ => ch.root + ch.fifth,
                        };
                        mix.bass(t, 0.9, bass_midi(pc.rem_euclid(12)), if pos == 0 { 1.0 } else { 0.8 });
                    }
                    _ => {
                        mix.bass(t, 0.45, bass_midi(ch.bass), 1.0);
                        mix.bass(t + 0.5, 0.45, bass_midi(if pos % 2 == 1 { (ch.root + ch.fifth) % 12 } else { ch.bass }), 0.75);
                    }
                }
            }

            // ---- chords
            if parts[1] {
                let v = ch.voicing();
                match style {
                    0 => mix.stab(t, 0.5, &v, if b % 2 == 1 { 0.8 } else { 0.4 }, 9.0),
                    1 => {
                        if pos == 0 {
                            mix.stab(t, len as f32, &v, 0.75, 1.6);
                        }
                    }
                    2 => {
                        // 1, the "and" of 2, and 4
                        if b == 0 || b == 3 {
                            mix.stab(t, 0.9, &v, 0.7, 7.0);
                        } else if b == 1 {
                            mix.stab(t + 0.5, 0.9, &v, 0.7, 7.0);
                        }
                    }
                    3 => {
                        if pos == 0 {
                            mix.stab(t, len as f32, &v, 0.35, 0.8);
                        }
                        mix.stab(t + 0.667, 0.3, &v, 0.55, 12.0);
                    }
                    4 => {
                        if b % 2 == 0 {
                            mix.stab(t, 0.6, &v, 0.6, 8.0);
                        }
                    }
                    _ => {
                        if b > 0 {
                            mix.stab(t, 0.7, &v, 0.65, 8.0);
                        }
                    }
                }
            }

            // ---- drums
            if parts[2] {
                let sw = jazz;
                match style {
                    0 | 5 => {
                        mix.ride(t, 0.9);
                        if b % 2 == 1 || nb == 3 && b > 0 {
                            mix.ride(t + swing(0.5, sw), 0.5);
                        }
                        if b % 2 == 1 {
                            mix.hat(t, false, 0.7);
                        }
                        mix.kick(t, if style == 5 { 0.5 } else { 0.18 });
                    }
                    1 => {
                        mix.ride(t, 0.35);
                        if b % 2 == 1 {
                            mix.hat(t, false, 0.35);
                        }
                    }
                    2 => {
                        mix.hat(t, false, 0.35);
                        mix.hat(t + 0.5, false, 0.25);
                        if b % 2 == 0 {
                            mix.kick(t, 0.6);
                        }
                        if b == 0 || b == 3 {
                            mix.rim(t, 0.7);
                        }
                        if b == 1 {
                            mix.rim(t + 0.5, 0.7);
                        }
                    }
                    3 => {
                        if b % 2 == 0 {
                            mix.kick(t, 0.9);
                        } else {
                            mix.snare(t, 0.8);
                        }
                        mix.hat(t, false, 0.4);
                        mix.hat(t + 0.667, false, 0.28);
                    }
                    _ => {
                        if b % 2 == 0 {
                            mix.kick(t, 0.9);
                        } else {
                            mix.snare(t, 0.8);
                        }
                        mix.hat(t, false, 0.4);
                        mix.hat(t + 0.5, false, 0.28);
                    }
                }
            }
        }
    }
    mix.finish()
}

// ----------------------------------------------------------------- beat finder
/// Find the tempo and the position of a beat in a recording: (seconds per beat, seconds of a beat).
/// Looks at how sharply the sound changes (about 200 times a second) and picks the pulse that
/// lines up with the most of those changes. Good on steady music; it can land on half or double.
pub fn detect_beats(bytes: Vec<u8>) -> Option<(f32, f32)> {
    let src = SymSource::new(bytes).ok()?;
    let ch = src.channels().max(1) as usize;
    let hop = (src.sample_rate() as usize / 200).max(1);
    let (mut env, mut acc, mut n, mut c, mut mono, mut prev) = (Vec::<f32>::new(), 0f32, 0usize, 0usize, 0f32, 0f32);
    for s in src {
        mono += s as f32 / 32768.0;
        c += 1;
        if c == ch {
            let x = mono / ch as f32;
            (c, mono) = (0, 0.0);
            acc += (x - prev).abs();
            prev = x;
            n += 1;
            if n == hop {
                env.push(acc / hop as f32);
                (acc, n) = (0.0, 0);
                if env.len() >= 200 * 150 {
                    break;
                }
            }
        }
    }
    if env.len() < 800 {
        return None;
    }
    let w = 8;
    let mut on = vec![0f32; env.len()];
    let mut run: f32 = env[..w].iter().sum();
    for i in w..env.len() {
        on[i] = (env[i] - run / w as f32).max(0.0);
        run += env[i] - env[i - w];
    }
    // tempo: strongest repeat distance between onsets, leaning gently towards 120 bpm
    let (mut best, mut lag0) = (0.0f32, 100usize);
    for lag in 60..=200usize {
        let sum: f32 = (0..on.len() - lag).map(|i| on[i] * on[i + lag]).sum::<f32>() / (on.len() - lag) as f32;
        let bpm = 60.0 * 200.0 / lag as f32;
        let score = sum * (-0.5 * ((bpm / 120.0).log2() / 0.9).powi(2)).exp();
        if score > best {
            (best, lag0) = (score, lag);
        }
    }
    // fine tune the beat length and find where the beats fall
    let (mut top, mut period, mut phase) = (-1.0f32, lag0 as f32, 0.0f32);
    let mut p = lag0 as f32 - 3.0;
    while p <= lag0 as f32 + 3.0 {
        for ph in 0..p as usize {
            let (mut sum, mut beats, mut t) = (0.0f32, 0.0f32, ph as f32);
            while (t as usize) + 1 < on.len() {
                let k = t.round() as usize;
                sum += on[k].max(on[k.saturating_sub(1)] * 0.7).max(on[(k + 1).min(on.len() - 1)] * 0.7);
                beats += 1.0;
                t += p;
            }
            let score = sum / beats.max(1.0);
            if score > top {
                (top, period, phase) = (score, p, ph as f32);
            }
        }
        p += 0.1;
    }
    Some((period / 200.0, phase / 200.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notes(c: &str) -> (i32, i32, i32, Option<i32>, Option<i32>, i32) {
        let ch = parse_chord(c).unwrap_or_else(|| panic!("{} did not parse", c));
        (ch.root, ch.third, ch.fifth, ch.seventh, ch.ext, ch.bass)
    }

    #[test]
    fn chord_spellings() {
        // root, third, fifth, seventh, colour note, bass (semitones; root and bass as note numbers)
        assert_eq!(notes("C"), (0, 4, 7, None, None, 0));
        assert_eq!(notes("Dm7"), (2, 3, 7, Some(10), None, 2));
        assert_eq!(notes("Cmaj7"), (0, 4, 7, Some(11), None, 0));
        assert_eq!(notes("Bm7b5"), (11, 3, 6, Some(10), None, 11));
        assert_eq!(notes("Cdim7"), (0, 3, 6, Some(9), None, 0));
        assert_eq!(notes("Csus4"), (0, 5, 7, None, None, 0));
        assert_eq!(notes("Caug"), (0, 4, 8, None, None, 0));
        assert_eq!(notes("G7b9"), (7, 4, 7, Some(10), Some(13), 7));
        assert_eq!(notes("F7#11"), (5, 4, 7, Some(10), Some(18), 5));
        assert_eq!(notes("Cm6"), (0, 3, 7, Some(9), None, 0));
        assert_eq!(notes("C/E"), (0, 4, 7, None, None, 4));
        assert!(parse_chord("N.C.").is_none());
    }

    #[test]
    fn six_nine_has_no_flat_seven() {
        assert_eq!(notes("C69"), (0, 4, 7, Some(9), Some(14), 0));
    }

    #[test]
    fn typed_ireal_spellings() {
        assert_eq!(notes("D-7"), notes("Dm7"));
        assert_eq!(notes("C^7"), notes("Cmaj7"));
        assert_eq!(notes("Bh7"), notes("Bm7b5"));
        assert_eq!(notes("Co7"), notes("Cdim7"));
        assert_eq!(notes("Eb-7/Db"), (3, 3, 7, Some(10), None, 1));
    }

    #[test]
    fn voicings_sit_in_the_middle() {
        for c in ["C", "Dm7", "G7b9", "Cmaj7", "F#m7b5"] {
            let v = parse_chord(c).unwrap().voicing();
            assert!(v.iter().all(|m| (55..67).contains(m)), "{} -> {:?}", c, v);
            assert!(v.windows(2).all(|w| w[0] < w[1]), "sorted, no doubles");
        }
        assert_eq!(bass_midi(0), 36);
        assert!((28..40).contains(&bass_midi(11)));
    }

    #[test]
    fn beat_groupings() {
        assert!(groupings(7).contains(&vec![3, 4]) || groupings(7).contains(&vec![2, 2, 3]));
        for n in 2..=32 {
            let g = groupings(n);
            assert!(!g.is_empty(), "{}", n);
            for cand in &g {
                assert_eq!(cand.iter().sum::<u32>(), n, "{:?}", cand);
                assert!(cand.iter().all(|x| (2..=5).contains(x)), "{} -> {:?}", n, cand);
            }
        }
        assert!(groupings(1).is_empty());
    }

    #[test]
    fn metronome_bar_length() {
        let m = Metro::default();
        // one 4/4 bar at 120 = 2 seconds
        let v = click_loop(&m, 120.0);
        assert!((v.len() as f32 - 2.0 * RATE).abs() < 2.0, "{}", v.len());
        // 7/8 at 120 (quarter-note bpm): 7 eighths = 1.75 s
        let m8 = Metro { beats: 7, unit: 8, ..Metro::default() };
        assert!((click_loop(&m8, 120.0).len() as f32 - 1.75 * RATE).abs() < 2.0);
        // play 2 bars, rest 1: three bars long
        let gap = Metro { gap_play: 2, gap_mute: 1, ..Metro::default() };
        assert!((click_loop(&gap, 120.0).len() as f32 - 6.0 * RATE).abs() < 2.0);
        assert!(count_in(3, 60.0).len() as f32 > 2.9 * RATE);
    }

    #[test]
    fn swing_moves_only_the_offbeat() {
        assert_eq!(swing(2.0, true), 2.0);
        assert!((swing(2.5, true) - 2.667).abs() < 0.001);
        assert_eq!(swing(2.5, false), 2.5);
        assert!((freq(69) - 440.0).abs() < 0.01 && (freq(57) - 220.0).abs() < 0.01);
    }
}
