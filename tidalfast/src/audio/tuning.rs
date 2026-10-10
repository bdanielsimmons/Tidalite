//! Pitch analysis of a recording: how far from A440 it is tuned (old records run slow, some modern tracks are
//! tuned on purpose), and which key it is in.
//! The track's spectrum is checked against the grid of equal-tempered notes at every offset from -50 to +50
//! cents; the offset where the notes line up best wins. For the key, the notes found (on the recording's own
//! tuning) are folded into the 12 pitch classes and matched against major / minor key profiles.

use std::f64::consts::PI;

/// In-place FFT of `re` / `im` (length a power of two).
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let tw: Vec<(f32, f32)> = (0..n / 2)
        .map(|k| ((-2.0 * PI * k as f64 / n as f64).cos() as f32, (-2.0 * PI * k as f64 / n as f64).sin() as f32))
        .collect();
    let mut len = 2;
    while len <= n {
        let stride = n / len;
        for i in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (wr, wi) = tw[k * stride];
                let (a, b) = (i + k, i + k + len / 2);
                let tr = re[b] * wr - im[b] * wi;
                let ti = re[b] * wi + im[b] * wr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

/// Average spectrum of `x` (mono at `sr` Hz) between `from` and `to` seconds, keeping only what stands out
/// from its surroundings, so steady noise and drums count for little. Returns it with the Hz per bin.
fn spectrum(x: &[f32], sr: f32, from: f32, to: f32) -> Option<(Vec<f32>, f32)> {
    const N: usize = 8192;
    if x.len() < N * 8 {
        return None;
    }
    // a stretch from the middle: intros and fade-outs are the least tonal
    let (start, end) = if x.len() > (sr * to) as usize { ((sr * from) as usize, (sr * to) as usize) } else { (0, x.len()) };
    let win: Vec<f32> = (0..N).map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / N as f64).cos() as f32).collect();
    let mut avg = vec![0f32; N / 2];
    let mut frames = 0;
    let mut pos = start;
    while pos + N <= end {
        let mut re: Vec<f32> = (0..N).map(|i| x[pos + i] * win[i]).collect();
        let mut im = vec![0f32; N];
        fft(&mut re, &mut im);
        for k in 0..N / 2 {
            avg[k] += (re[k] * re[k] + im[k] * im[k]).sqrt();
        }
        frames += 1;
        pos += N / 2;
    }
    if frames < 4 {
        return None;
    }
    let w = 24usize;
    let mut pre = vec![0f64; N / 2 + 1];
    for k in 0..N / 2 {
        pre[k + 1] = pre[k] + avg[k] as f64;
    }
    let white: Vec<f32> = (0..N / 2)
        .map(|k| {
            let (lo, hi) = (k.saturating_sub(w), (k + w + 1).min(N / 2));
            let mean = (pre[hi] - pre[lo]) / (hi - lo) as f64;
            (avg[k] as f64 / (mean + 1e-9)).min(8.0) as f32
        })
        .collect();
    Some((white, sr / N as f32))
}

/// Offset of the recording from A440 in cents (positive = sharp) and how clear the answer is.
/// `x` is mono audio at `sr` Hz.
pub fn offset_cents(x: &[f32], sr: f32) -> Option<(i32, f32)> {
    let (white, bin_hz) = spectrum(x, sr, 10.0, 60.0)?;
    let score = |cents: f32| -> f32 {
        let mut s = 0.0;
        for m in 52..=100 {
            let f = 440.0 * 2f32.powf((m as f32 - 69.0) / 12.0 + cents / 1200.0);
            let b = f / bin_hz;
            let i = b.floor() as usize;
            if i + 1 >= white.len() || f > sr * 0.45 {
                break;
            }
            let fr = b - i as f32;
            s += white[i] * (1.0 - fr) + white[i + 1] * fr;
        }
        s
    };
    let scores: Vec<f32> = (-50..=50).map(|c| score(c as f32)).collect();
    let mean = scores.iter().sum::<f32>() / scores.len() as f32;
    let (bi, best) = scores.iter().copied().enumerate().fold((0, f32::MIN), |a, (i, s)| if s > a.1 { (i, s) } else { a });
    if mean <= 0.0 {
        return None;
    }
    Some((bi as i32 - 50, best / mean - 1.0))
}

// Temperley's profiles: how strongly each scale degree belongs to a major / minor key (tonic first).
// (Tried against Tidal's own keys on real tracks, these beat the classic Krumhansl ones.)
const MAJOR: [f32; 12] = [5.0, 2.0, 3.5, 2.0, 4.5, 4.0, 2.0, 4.5, 2.0, 3.5, 1.5, 4.0];
const MINOR: [f32; 12] = [5.0, 2.0, 3.5, 4.5, 2.0, 4.0, 2.0, 4.5, 3.5, 2.0, 1.5, 4.0];

/// How much of each of the 12 notes the music holds, frame by frame (each frame weighs the same, so quiet
/// passages count as much as loud ones), from C1 to C6 on the recording's own tuning.
fn chroma(x: &[f32], sr: f32, cents: i32) -> [f32; 12] {
    const N: usize = 8192;
    let mut out = [0f32; 12];
    let (start, end) = if x.len() > (sr * 150.0) as usize { ((sr * 10.0) as usize, (sr * 150.0) as usize) } else { (0, x.len()) };
    let win: Vec<f32> = (0..N).map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / N as f64).cos() as f32).collect();
    let bin_hz = sr / N as f32;
    let mut pos = start;
    while pos + N <= end {
        let mut re: Vec<f32> = (0..N).map(|i| x[pos + i] * win[i]).collect();
        let mut im = vec![0f32; N];
        fft(&mut re, &mut im);
        let mag: Vec<f32> = (0..N / 2).map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt()).collect();
        let top = mag.iter().cloned().fold(0f32, f32::max).max(1e-9);
        // the strongest bin within a quarter tone of the note, on a log scale
        let note = |m: i32| -> f32 {
            let f = 440.0 * 2f32.powf((m as f32 - 69.0) / 12.0 + cents as f32 / 1200.0);
            if f > sr * 0.45 {
                return 0.0;
            }
            let b = f / bin_hz;
            let (b0, b1) =
                ((b * 2f32.powf(-1.0 / 24.0)).floor() as usize, ((b * 2f32.powf(1.0 / 24.0)).ceil() as usize).min(mag.len() - 1));
            (1.0 + 100.0 * mag[b0..=b1].iter().cloned().fold(0f32, f32::max) / top).ln()
        };
        let mut c = [0f32; 12];
        for m in 24..=84 {
            // every note also rings a fifth higher (its 3rd harmonic): take half of that back out,
            // or the key comes out a fifth too high
            c[m as usize % 12] += (note(m) - 0.5 * note(m - 19)).max(0.0);
        }
        let s: f32 = c.iter().sum();
        if s > 0.0 {
            for k in 0..12 {
                out[k] += c[k] / s;
            }
        }
        pos += N / 2;
    }
    out
}

fn correlate(a: &[f32; 12], b: &[f32; 12]) -> f32 {
    let (ma, mb) = (a.iter().sum::<f32>() / 12.0, b.iter().sum::<f32>() / 12.0);
    let (mut num, mut da, mut db) = (0.0, 0.0, 0.0);
    for i in 0..12 {
        num += (a[i] - ma) * (b[i] - mb);
        da += (a[i] - ma).powi(2);
        db += (b[i] - mb).powi(2);
    }
    num / (da * db).sqrt().max(1e-9)
}

/// The key of `x` (mono at `sr` Hz): 0-11 = C..B major, 12-23 = C..B minor, and the gap to the runner-up.
/// `cents` is the recording's tuning offset. One key for the whole track: an estimate, and music that
/// changes key (a lot of jazz) has no single right answer.
pub fn key_of(x: &[f32], sr: f32, cents: i32) -> Option<(u8, f32)> {
    if x.len() < 8192 * 8 {
        return None;
    }
    let ch = chroma(x, sr, cents);
    if ch.iter().sum::<f32>() <= 0.0 {
        return None;
    }
    let mut scores: Vec<(u8, f32)> = Vec::with_capacity(24);
    for (base, prof) in [(0u8, &MAJOR), (12u8, &MINOR)] {
        for tonic in 0..12 {
            let mut rot = [0f32; 12];
            for d in 0..12 {
                rot[(tonic + d) % 12] = prof[d];
            }
            scores.push((base + tonic as u8, correlate(&ch, &rot)));
        }
    }
    scores.sort_by(|a, b| b.1.total_cmp(&a.1));
    Some((scores[0].0, scores[0].1 - scores[1].1))
}

// WRAPPER-START
use crate::decode::SymSource;
use rodio::Source;

/// Decode up to `secs` seconds of a stored track as mono audio at about 11 kHz (plenty for pitch work).
pub fn decode_mono(bytes: Vec<u8>, secs: f32) -> Option<(Vec<f32>, f32)> {
    let src = SymSource::new(bytes).ok()?;
    let ch = src.channels().max(1) as usize;
    let rate = src.sample_rate() as usize;
    let dec = (rate / 11025).max(1);
    let sr = rate as f32 / dec as f32;
    let limit = (secs * sr) as usize;
    let mut x: Vec<f32> = Vec::new();
    let (mut acc, mut n, mut c, mut mono) = (0f32, 0usize, 0usize, 0f32);
    for s in src {
        mono += s as f32 / 32768.0;
        c += 1;
        if c == ch {
            acc += mono / ch as f32;
            (c, mono) = (0, 0.0);
            n += 1;
            if n == dec {
                x.push(acc / dec as f32);
                (acc, n) = (0.0, 0);
                if x.len() >= limit {
                    break;
                }
            }
        }
    }
    Some((x, sr))
}

/// Decode a stored track and measure its tuning.
pub fn detect(bytes: Vec<u8>) -> Option<(i32, f32)> {
    let (x, sr) = decode_mono(bytes, 65.0)?;
    offset_cents(&x, sr)
}
// WRAPPER-END

#[cfg(test)]
mod tests {
    use super::*;

    /// A chord progression with harmonics, tuned `cents` away from A440, plus noise.
    fn synth(cents: f32, sr: f32, secs: f32) -> Vec<f32> {
        let n = (sr * secs) as usize;
        let chords: [[i32; 3]; 4] = [[48, 52, 55], [53, 57, 60], [55, 59, 62], [48, 52, 55]];
        let mut seed = 12345u64;
        (0..n)
            .map(|i| {
                let t = i as f32 / sr;
                let ch = chords[((t / 2.0) as usize) % 4];
                let mut v = 0.0;
                for m in ch {
                    let f = 440.0 * 2f32.powf((m as f32 - 69.0) / 12.0 + cents / 1200.0);
                    for h in 1..=5 {
                        v += (2.0 * std::f32::consts::PI * f * h as f32 * t).sin() / h as f32;
                    }
                }
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                v * 0.2 + ((seed >> 40) as f32 / 16777216.0 - 0.5) * 0.3
            })
            .collect()
    }

    #[test]
    fn finds_the_offset() {
        for target in [-35, -12, 0, 8, 23, 41] {
            let (c, conf) = offset_cents(&synth(target as f32, 11025.0, 40.0), 11025.0).unwrap();
            assert!((c - target).abs() <= 3, "target {} got {} (conf {})", target, c, conf);
            assert!(conf > 0.05, "confidence {}", conf);
        }
    }

    #[test]
    fn noise_is_not_confident() {
        let mut seed = 99u64;
        let x: Vec<f32> = (0..11025 * 40)
            .map(|_| {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (seed >> 40) as f32 / 16777216.0 - 0.5
            })
            .collect();
        let (_, conf) = offset_cents(&x, 11025.0).unwrap();
        assert!(conf < 0.05, "noise confidence {}", conf);
    }

    /// Like `synth`, for any chord progression (MIDI notes), 2 s per chord.
    fn chords(prog: &[[i32; 3]], cents: f32, sr: f32, secs: f32) -> Vec<f32> {
        let n = (sr * secs) as usize;
        let mut seed = 777u64;
        (0..n)
            .map(|i| {
                let t = i as f32 / sr;
                let ch = prog[((t / 2.0) as usize) % prog.len()];
                let mut v = 0.0;
                for m in ch {
                    let f = 440.0 * 2f32.powf((m as f32 - 69.0) / 12.0 + cents / 1200.0);
                    for h in 1..=5 {
                        v += (2.0 * std::f32::consts::PI * f * h as f32 * t).sin() / h as f32;
                    }
                }
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                v * 0.2 + ((seed >> 40) as f32 / 16777216.0 - 0.5) * 0.3
            })
            .collect()
    }

    #[test]
    fn finds_major_and_minor_keys() {
        // I IV V I in C, then the same moved up to F#
        let c = [[48, 52, 55], [53, 57, 60], [55, 59, 62], [48, 52, 55]];
        assert_eq!(key_of(&chords(&c, 0.0, 11025.0, 40.0), 11025.0, 0).unwrap().0, 0);
        let fs: Vec<[i32; 3]> = c.iter().map(|ch| ch.map(|m| m + 6)).collect();
        assert_eq!(key_of(&chords(&fs, 0.0, 11025.0, 40.0), 11025.0, 0).unwrap().0, 6);
        // i iv V i in A minor
        let am = [[57, 60, 64], [50, 53, 57], [52, 56, 59], [57, 60, 64]];
        assert_eq!(key_of(&chords(&am, 0.0, 11025.0, 40.0), 11025.0, 0).unwrap().0, 12 + 9);
    }

    #[test]
    fn key_follows_the_recordings_tuning() {
        // a record 40 cents flat: measured on its own tuning it is still in C
        let c = [[48, 52, 55], [53, 57, 60], [55, 59, 62], [48, 52, 55]];
        let x = chords(&c, -40.0, 11025.0, 40.0);
        let (cents, _) = offset_cents(&x, 11025.0).unwrap();
        assert_eq!(key_of(&x, 11025.0, cents).unwrap().0, 0);
    }
}
