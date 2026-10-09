//! How far from A440 a recording is tuned (old records run slow, some modern tracks are tuned on purpose).
//! The track's spectrum is checked against the grid of equal-tempered notes at every offset from -50 to +50
//! cents; the offset where the notes line up best wins.

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

/// Offset of the recording from A440 in cents (positive = sharp) and how clear the answer is.
/// `x` is mono audio at `sr` Hz.
pub fn offset_cents(x: &[f32], sr: f32) -> Option<(i32, f32)> {
    const N: usize = 8192;
    if x.len() < N * 8 {
        return None;
    }
    // a stretch from the middle: intros and fade-outs are the least tonal
    let (start, end) =
        if x.len() > (sr * 60.0) as usize { ((sr * 10.0) as usize, ((sr * 60.0) as usize).min(x.len())) } else { (0, x.len()) };
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
    // keep only what stands out from its surroundings, so steady noise does not count
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
    let bin_hz = sr / N as f32;
    let score = |cents: f32| -> f32 {
        let mut s = 0.0;
        for m in 52..=100 {
            let f = 440.0 * 2f32.powf((m as f32 - 69.0) / 12.0 + cents / 1200.0);
            let b = f / bin_hz;
            let i = b.floor() as usize;
            if i + 1 >= N / 2 || f > sr * 0.45 {
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

// WRAPPER-START
use crate::decode::SymSource;
use rodio::Source;

/// Decode a stored track and measure its tuning.
pub fn detect(bytes: Vec<u8>) -> Option<(i32, f32)> {
    let src = SymSource::new(bytes).ok()?;
    let ch = src.channels().max(1) as usize;
    let rate = src.sample_rate() as usize;
    let dec = (rate / 11025).max(1);
    let sr = rate as f32 / dec as f32;
    let limit = (65.0 * sr) as usize;
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
}
