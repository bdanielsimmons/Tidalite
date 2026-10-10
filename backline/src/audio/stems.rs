//! Stem separation (drums / bass / other / vocals) with the HT-Demucs model, run on this computer
//! through ONNX Runtime. Nothing ships inside the .exe: the runtime (about 12 MB) and the model
//! (about 170 MB) are downloaded once, when you ask for them.
//!
//! A track is split in chunks of 7.8 s (with overlap) and the four stems are saved as WAV files in
//! `%APPDATA%\backline\stems\<track id>\`. Muting / soloing a stem simply mixes the stems that are
//! switched on and hands that mix to the normal player, so loops, slow-down, pitch, EQ etc. all work.

use crate::decode::SymSource;
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use rodio::Source;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

pub const NAMES: [&str; 4] = ["DRUMS", "BASS", "OTHER", "VOCALS"];
const FILES: [&str; 4] = ["drums", "bass", "other", "vocals"];

const MODEL_URL: &str = "https://huggingface.co/StemSplitio/htdemucs-onnx/resolve/main/htdemucs_fp16weights.onnx";
const RUNTIME_URL: &str = "https://github.com/microsoft/onnxruntime/releases/download/v1.22.0/onnxruntime-win-x64-1.22.0.zip";

const SEG: usize = 343_980; // 7.8 s at 44.1 kHz
const OVERLAP: usize = SEG / 4;
const STRIDE: usize = SEG - OVERLAP;
const MAX_SECONDS: usize = 20 * 60;

pub static DONE: AtomicU32 = AtomicU32::new(0);
pub static TOTAL: AtomicU32 = AtomicU32::new(0);
pub static CANCEL: AtomicBool = AtomicBool::new(false);

fn stage() -> &'static Mutex<String> {
    static S: OnceLock<Mutex<String>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(String::new()))
}

fn set_stage(s: &str) {
    if let Ok(mut g) = stage().lock() {
        *g = s.to_string();
    }
}

/// What the background job is doing right now ("DOWNLOADING MODEL 42%").
pub fn stage_text() -> String {
    stage().lock().map(|g| g.clone()).unwrap_or_default()
}

pub fn dir() -> PathBuf {
    crate::api::config_dir().join("models")
}

fn dll_path() -> PathBuf {
    dir().join("onnxruntime.dll")
}

fn model_path() -> PathBuf {
    dir().join("htdemucs_fp16.onnx")
}

/// Runtime and model are both on disk.
pub fn tool_ready() -> bool {
    dll_path().exists() && model_path().exists() && cfg!(windows)
}

pub fn stems_dir(id: i64) -> PathBuf {
    crate::api::config_dir().join("stems").join(id.to_string())
}

pub fn have_stems(id: i64) -> bool {
    stems_dir(id).join("ok").exists()
}

/// Total size of all saved stems, in bytes.
pub fn stems_size() -> u64 {
    fn walk(p: &Path) -> u64 {
        let mut t = 0;
        if let Ok(rd) = std::fs::read_dir(p) {
            for e in rd.flatten() {
                if let Ok(m) = e.metadata() {
                    t += if m.is_dir() { walk(&e.path()) } else { m.len() };
                }
            }
        }
        t
    }
    walk(&crate::api::config_dir().join("stems"))
}

// ---------------------------------------------------------------- download
fn download_file(url: &str, dest: &Path, label: &str, min_len: u64) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(3600)).build().map_err(|e| e.to_string())?;
    let mut resp = client.get(url).send().map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?;
    let total = resp.content_length().unwrap_or(0);
    if let Some(d) = dest.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = dest.with_extension("part");
    let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 256 * 1024];
    let mut got: u64 = 0;
    loop {
        let n = resp.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        f.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        got += n as u64;
        if total > 0 {
            set_stage(&format!("{} {}%", label, got * 100 / total));
        } else {
            set_stage(&format!("{} {} MB", label, got >> 20));
        }
    }
    drop(f);
    if got < min_len {
        let _ = std::fs::remove_file(&tmp);
        return Err("download looks incomplete".to_string());
    }
    std::fs::rename(&tmp, dest).map_err(|e| e.to_string())
}

/// One-time setup: the ONNX Runtime library and the Demucs model.
pub fn download_tool() -> Result<(), String> {
    if !cfg!(windows) {
        return Err("stem separation is only set up for Windows".to_string());
    }
    if !dll_path().exists() {
        let zip_path = dir().join("onnxruntime.zip");
        download_file(RUNTIME_URL, &zip_path, "DOWNLOADING RUNTIME", 1_000_000)?;
        set_stage("UNPACKING RUNTIME");
        let bytes = std::fs::read(&zip_path).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
        let mut found = false;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
            let name = f.name().replace('\\', "/");
            if name.ends_with("/lib/onnxruntime.dll") {
                let mut data = Vec::new();
                f.read_to_end(&mut data).map_err(|e| e.to_string())?;
                std::fs::write(dll_path(), data).map_err(|e| e.to_string())?;
                found = true;
                break;
            }
        }
        let _ = std::fs::remove_file(&zip_path);
        if !found {
            return Err("onnxruntime.dll not found in the download".to_string());
        }
    }
    if !model_path().exists() {
        download_file(MODEL_URL, &model_path(), "DOWNLOADING MODEL", 50_000_000)?;
    }
    set_stage("");
    Ok(())
}

fn init_runtime() -> Result<(), String> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    INIT.get_or_init(|| {
        let p = dll_path();
        if !p.exists() {
            return Err("stem tool is not installed".to_string());
        }
        ort::init_from(p.to_string_lossy().to_string()).commit().map(|_| ()).map_err(|e| e.to_string())
    })
    .clone()
}

// ---------------------------------------------------------------- audio
/// Decode to 44.1 kHz stereo floats.
fn decode_44k(bytes: Vec<u8>) -> Result<(Vec<f32>, Vec<f32>), String> {
    let src = SymSource::new(bytes)?;
    let ch = (src.channels() as usize).max(1);
    let rate = src.sample_rate().max(8000) as usize;
    let mut l: Vec<f32> = Vec::new();
    let mut r: Vec<f32> = Vec::new();
    let mut frame = [0.0f32; 2];
    let mut i = 0usize;
    for s in src {
        if i < 2 {
            frame[i] = s as f32 / 32768.0;
        }
        i += 1;
        if i == ch {
            if ch == 1 {
                frame[1] = frame[0];
            }
            l.push(frame[0]);
            r.push(frame[1]);
            i = 0;
            if l.len() > MAX_SECONDS * rate {
                return Err("this track is too long to split".to_string());
            }
        }
    }
    if l.is_empty() {
        return Err("no audio to split".to_string());
    }
    if rate == 44100 {
        return Ok((l, r));
    }
    let n_out = (l.len() as f64 * 44100.0 / rate as f64) as usize;
    let conv = |x: &Vec<f32>| -> Vec<f32> {
        (0..n_out)
            .map(|k| {
                let p = k as f64 * rate as f64 / 44100.0;
                let i0 = (p as usize).min(x.len() - 1);
                let i1 = (i0 + 1).min(x.len() - 1);
                let fr = (p - i0 as f64) as f32;
                x[i0] * (1.0 - fr) + x[i1] * fr
            })
            .collect()
    };
    Ok((conv(&l), conv(&r)))
}

fn wav_header(data_len: u32) -> Vec<u8> {
    let mut h: Vec<u8> = Vec::with_capacity(44);
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(36 + data_len).to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes());
    h.extend_from_slice(&2u16.to_le_bytes());
    h.extend_from_slice(&44100u32.to_le_bytes());
    h.extend_from_slice(&(44100u32 * 4).to_le_bytes());
    h.extend_from_slice(&4u16.to_le_bytes());
    h.extend_from_slice(&16u16.to_le_bytes());
    h.extend_from_slice(b"data");
    h.extend_from_slice(&data_len.to_le_bytes());
    h
}

fn write_wav(path: &Path, data: &[i16]) -> Result<(), String> {
    let mut out = wav_header((data.len() * 2) as u32);
    out.reserve(data.len() * 2);
    for s in data {
        out.extend_from_slice(&s.to_le_bytes());
    }
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, out).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- separation
/// Split one track into four stems and save them. Takes minutes; reports through DONE / TOTAL.
pub fn separate(bytes: Vec<u8>, id: i64) -> Result<(), String> {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| separate_inner(bytes, id)));
    match r {
        Ok(x) => x,
        Err(_) => Err("the stem tool crashed (is onnxruntime.dll blocked? try GET STEMS TOOL again)".to_string()),
    }
}

fn separate_inner(bytes: Vec<u8>, id: i64) -> Result<(), String> {
    CANCEL.store(false, Ordering::Relaxed);
    DONE.store(0, Ordering::Relaxed);
    TOTAL.store(0, Ordering::Relaxed);
    set_stage("PREPARING");
    init_runtime()?;
    let (l, r) = decode_44k(bytes)?;
    let n = l.len();
    let chunks = n.div_ceil(STRIDE).max(1);
    TOTAL.store(chunks as u32, Ordering::Relaxed);

    let threads = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(4).saturating_sub(1).clamp(1, 12);
    let mut session = Session::builder()
        .map_err(|e| e.to_string())?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| e.to_string())?
        .with_intra_threads(threads)
        .map_err(|e| e.to_string())?
        .commit_from_file(model_path())
        .map_err(|e| format!("can't load the model: {}", e))?;

    // stem s, channel c -> accumulated audio; plus the overlap weights
    let mut acc: Vec<Vec<f32>> = (0..8).map(|_| vec![0.0f32; n]).collect();
    let mut weight = vec![0.0f32; n];

    set_stage("SPLITTING");
    for k in 0..chunks {
        if CANCEL.load(Ordering::Relaxed) {
            return Err("cancelled".to_string());
        }
        let start = k * STRIDE;
        let end = (start + SEG).min(n);
        let valid = end - start;
        let mut input = vec![0.0f32; 2 * SEG];
        input[..valid].copy_from_slice(&l[start..end]);
        input[SEG..SEG + valid].copy_from_slice(&r[start..end]);
        let tensor = Tensor::from_array(([1usize, 2, SEG], input)).map_err(|e| e.to_string())?;
        let outputs = session.run(ort::inputs!["mix" => tensor]).map_err(|e| format!("model error: {}", e))?;
        let (shape, data) = outputs["stems"].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
        if data.len() < 8 * SEG {
            return Err(format!("unexpected model output ({} values)", shape.num_elements()));
        }
        for i in 0..valid {
            let up = (i + 1) as f32 / OVERLAP as f32;
            let down = (SEG - i) as f32 / OVERLAP as f32;
            let w = up.min(down).min(1.0);
            weight[start + i] += w;
            for sc in 0..8 {
                acc[sc][start + i] += data[sc * SEG + i] * w;
            }
        }
        DONE.store(k as u32 + 1, Ordering::Relaxed);
    }
    drop(session);

    set_stage("SAVING");
    let d = stems_dir(id);
    let _ = std::fs::remove_file(d.join("ok"));
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    for s in 0..4 {
        let mut pcm: Vec<i16> = Vec::with_capacity(n * 2);
        for i in 0..n {
            let w = weight[i].max(1e-8);
            for c in 0..2 {
                let v = (acc[s * 2 + c][i] / w).clamp(-1.0, 1.0);
                pcm.push((v * 32767.0) as i16);
            }
        }
        write_wav(&d.join(format!("{}.wav", FILES[s])), &pcm)?;
    }
    std::fs::write(d.join("ok"), b"1").map_err(|e| e.to_string())?;
    set_stage("");
    Ok(())
}

// ---------------------------------------------------------------- mixing
pub struct Stems {
    pub data: [Vec<i16>; 4],
}

pub fn load(id: i64) -> Result<Stems, String> {
    let d = stems_dir(id);
    let mut out: [Vec<i16>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    for s in 0..4 {
        let b = std::fs::read(d.join(format!("{}.wav", FILES[s]))).map_err(|e| e.to_string())?;
        if b.len() < 44 {
            return Err("damaged stem file".to_string());
        }
        out[s] = b[44..].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
    }
    Ok(Stems { data: out })
}

/// The stems that are switched on, added together, as a WAV file in memory.
pub fn mix_wav(st: &Stems, on: [bool; 4]) -> Vec<u8> {
    let n = st.data.iter().map(|v| v.len()).min().unwrap_or(0);
    let mut out = wav_header((n * 2) as u32);
    out.reserve(n * 2);
    for i in 0..n {
        let mut sum: i32 = 0;
        for s in 0..4 {
            if on[s] {
                sum += st.data[s][i] as i32;
            }
        }
        out.extend_from_slice(&(sum.clamp(-32768, 32767) as i16).to_le_bytes());
    }
    out
}
