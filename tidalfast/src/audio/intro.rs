//! The little sound Tidalite makes when it opens: its own chime (made here in code: glassy bell tones rising
//! over a soft pad, washed in reverb, in the spirit of the glossy "aero" years), or a short file of your choice.

use crate::decode::SymSource;
use rodio::Source;
use std::f32::consts::TAU;

const RATE: u32 = 44100;

/// Play the opening sound on `device` at `vol` (0..1), off the main thread. `file`: your own sound (its first
/// eight seconds); None: Tidalite's chime.
pub fn play(file: Option<String>, device: Option<String>, vol: f32) {
    std::thread::spawn(move || {
        let Ok((_stream, handle)) = crate::player::open_named(device.as_deref()) else { return };
        let Ok(sink) = rodio::Sink::try_new(&handle) else { return };
        sink.set_volume(vol.clamp(0.0, 1.0));
        match file.and_then(|f| std::fs::read(f).ok()).and_then(|b| SymSource::new(b).ok()) {
            Some(src) => sink.append(src.take_duration(std::time::Duration::from_secs(8))),
            None => sink.append(rodio::buffer::SamplesBuffer::new(2, RATE, chime())),
        }
        sink.sleep_until_end();
    });
}

/// A bell: a sine carrier with a sine modulator (FM) whose brightness fades faster than the note.
fn bell(out: &mut [f32], start: f32, freq: f32, len: f32, amp: f32) {
    let s0 = (start * RATE as f32) as usize;
    let n = (len * RATE as f32) as usize;
    for i in 0..n {
        let Some(o) = out.get_mut(s0 + i) else { break };
        let t = i as f32 / RATE as f32;
        let env = (-t * 3.2 / len).exp() * (i.min(120) as f32 / 120.0);
        let index = 2.2 * (-t * 9.0).exp();
        let m = (t * freq * 3.5 * TAU).sin() * index;
        *o += (t * freq * TAU + m).sin() * env * amp;
    }
}

/// Tidalite's opening chime, stereo: a rising E major 9 arpeggio of glassy bells with high sparkles, over a
/// slowly swelling pad, through a roomy reverb.
pub fn chime() -> Vec<f32> {
    let secs = 4.2;
    let n = (secs * RATE as f32) as usize;
    let mut dry = vec![0.0f32; n];
    // the arpeggio: E5 G#5 B5 D#6 F#6, then a high B6 to finish
    for (k, f) in [659.25f32, 830.61, 987.77, 1244.51, 1479.98, 1975.53].iter().enumerate() {
        bell(&mut dry, 0.05 + k as f32 * 0.085, *f, 1.6, if k == 5 { 0.10 } else { 0.16 });
    }
    // sparkles: two very high, quiet pings
    bell(&mut dry, 0.55, 2959.96, 0.6, 0.05);
    bell(&mut dry, 0.70, 3951.07, 0.5, 0.035);
    // the pad: E4 and B4, gently detuned, swelling in and fading out
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let env = (t / 0.9).min(1.0) * (1.0 - ((t - 1.2) / 2.4).clamp(0.0, 1.0));
        let mut v = 0.0;
        for (f, d) in [(329.63f32, 0.0f32), (329.63, 1.3), (493.88, -0.9), (493.88, 0.7)] {
            v += ((f + d) * t * TAU).sin();
        }
        dry[i] += v * 0.025 * env;
    }
    reverb(&dry)
}

/// A small Freeverb-style room: four damped combs and two allpasses per side (slightly different lengths
/// left and right, for width), mixed with the dry sound. Interleaved stereo out.
pub(crate) fn reverb(dry: &[f32]) -> Vec<f32> {
    let side = |spread: usize| -> Vec<f32> {
        let mut combs: Vec<(Vec<f32>, usize, f32)> =
            [1116usize, 1188, 1277, 1356].iter().map(|l| (vec![0.0; l + spread], 0usize, 0.0f32)).collect();
        let mut alls: Vec<(Vec<f32>, usize)> = [556usize, 441].iter().map(|l| (vec![0.0; l + spread], 0usize)).collect();
        let (fb, damp) = (0.86f32, 0.25f32);
        dry.iter()
            .map(|&x| {
                let input = x * 0.3;
                let mut acc = 0.0;
                for (buf, i, store) in combs.iter_mut() {
                    let y = buf[*i];
                    *store = y * (1.0 - damp) + *store * damp;
                    buf[*i] = input + *store * fb;
                    *i = (*i + 1) % buf.len();
                    acc += y;
                }
                for (buf, i) in alls.iter_mut() {
                    let b = buf[*i];
                    buf[*i] = acc + b * 0.5;
                    acc = b - acc;
                    *i = (*i + 1) % buf.len();
                }
                acc
            })
            .collect()
    };
    let (l, r) = (side(0), side(23));
    let mut out = Vec::with_capacity(dry.len() * 2);
    for i in 0..dry.len() {
        // fade the very end so the tail never clicks
        let tail = ((dry.len() - i) as f32 / 4410.0).min(1.0);
        out.push((dry[i] * 0.8 + l[i] * 0.45) * tail);
        out.push((dry[i] * 0.8 + r[i] * 0.45) * tail);
    }
    // brought to a comfortable level, well short of clipping
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
    let k = 0.7 / peak;
    out.iter_mut().for_each(|v| *v *= k);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_chime_is_a_few_seconds_of_clean_stereo() {
        let c = super::chime();
        assert_eq!(c.len() % 2, 0);
        let secs = c.len() as f32 / 2.0 / super::RATE as f32;
        assert!((3.0..6.0).contains(&secs));
        let peak = c.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak > 0.1 && peak < 1.0, "audible and not clipping: {}", peak);
        assert!(c.iter().all(|v| v.is_finite()));
    }
}
