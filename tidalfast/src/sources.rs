//! Audio that doesn't come from Tidal: files on this computer, YouTube clips (through yt-dlp),
//! plus helpers that work on any audio: waveform peaks and WAV export of a loop.

use crate::decode::SymSource;
use crate::store::{hash_id, Ext};
use rodio::Source;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use symphonia::core::codecs::CODEC_TYPE_NULL;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub const AUDIO_EXT: [&str; 9] = ["mp3", "flac", "wav", "m4a", "aac", "mp4", "ogg", "oga", "alac"];

/// Where a non-Tidal track comes from.
#[derive(Clone)]
pub enum Src {
    File(PathBuf),
    Yt(String),
}

pub fn is_audio(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).map(|e| AUDIO_EXT.contains(&e.to_ascii_lowercase().as_str())).unwrap_or(false)
}

/// All audio files below `dir` (a few levels deep, capped so a huge drive can't hang the app).
pub fn scan_folder(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    if depth > 6 || out.len() > 5000 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            scan_folder(&p, depth + 1, out);
        } else if is_audio(&p) {
            out.push(p);
        }
    }
}

/// Length in seconds, read from the file header (or by walking the packets when there is none).
pub fn probe_duration(path: &Path) -> f32 {
    let Ok(f) = std::fs::File::open(path) else { return 0.0 };
    let mss = MediaSourceStream::new(Box::new(f), Default::default());
    let mut hint = Hint::new();
    if let Some(e) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(e);
    }
    let Ok(probed) = symphonia::default::get_probe().format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
    else {
        return 0.0;
    };
    let mut format = probed.format;
    let (id, params) = {
        let Some(t) = format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL) else {
            return 0.0;
        };
        (t.id, t.codec_params.clone())
    };
    let rate = params.sample_rate.unwrap_or(44100) as f64;
    if let Some(n) = params.n_frames {
        if let Some(tb) = params.time_base {
            let t = tb.calc_time(n);
            return (t.seconds as f64 + t.frac) as f32;
        }
        return (n as f64 / rate) as f32;
    }
    let mut frames: u64 = 0;
    while let Ok(pkt) = format.next_packet() {
        if pkt.track_id() == id {
            frames += pkt.dur;
        }
    }
    (frames as f64 / rate) as f32
}

pub fn ext_for_file(path: &Path) -> Ext {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("?").to_string();
    let parent = path.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str()).unwrap_or("").to_string();
    let (artist, title) = match stem.split_once(" - ") {
        Some((a, t)) => (a.trim().to_string(), t.trim().to_string()),
        None => (parent, stem.clone()),
    };
    let src = path.to_string_lossy().to_string();
    Ext {
        kind: "file".to_string(),
        id: hash_id(&format!("file:{}", src)),
        dur: probe_duration(path),
        src,
        title,
        artist,
        cover: String::new(),
    }
}

// ------------------------------------------------------------------ YouTube
fn exe_name() -> &'static str {
    if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    }
}

pub fn ytdlp_dir() -> PathBuf {
    crate::api::config_dir().join("bin")
}

fn command(p: &Path) -> Command {
    let mut c = Command::new(p);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // no console window
    }
    c.stdin(Stdio::null());
    c
}

/// yt-dlp next to the app, in the Tidalite folder, or anywhere on PATH.
pub fn ytdlp_path() -> Option<PathBuf> {
    let mut cands = vec![ytdlp_dir().join(exe_name())];
    if let Ok(me) = std::env::current_exe() {
        if let Some(d) = me.parent() {
            cands.push(d.join(exe_name()));
        }
    }
    for c in cands {
        if c.exists() {
            return Some(c);
        }
    }
    let p = PathBuf::from(exe_name());
    if command(&p).arg("--version").output().map(|o| o.status.success()).unwrap_or(false) {
        return Some(p);
    }
    None
}

/// Download (or update) yt-dlp from its official GitHub release into the Tidalite folder.
pub fn ytdlp_download() -> Result<PathBuf, String> {
    let url = if cfg!(windows) {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
    } else {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp"
    };
    let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(300)).build().map_err(|e| e.to_string())?;
    let bytes = client
        .get(url)
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .bytes()
        .map_err(|e| e.to_string())?;
    if bytes.len() < 1_000_000 {
        return Err("download looks incomplete".to_string());
    }
    let dir = ytdlp_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let tmp = dir.join("yt-dlp.download");
    std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    let fin = dir.join(exe_name());
    std::fs::rename(&tmp, &fin).map_err(|e| format!("could not replace yt-dlp ({})", e))?;
    Ok(fin)
}

fn valid_id(s: &str) -> bool {
    s.len() == 11 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The 11-character video id out of any common YouTube link (or a bare id).
pub fn yt_video_id(input: &str) -> Option<String> {
    let t = input.trim();
    if valid_id(t) {
        return Some(t.to_string());
    }
    for marker in ["v=", "youtu.be/", "/shorts/", "/embed/", "/live/"] {
        if let Some(i) = t.find(marker) {
            let rest = &t[i + marker.len()..];
            let id: String = rest.chars().take(11).collect();
            if valid_id(&id) {
                return Some(id);
            }
        }
    }
    None
}

fn last_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("unknown error").trim().to_string()
}

/// Title / uploader / length / thumbnail of a clip.
pub fn yt_info(vid: &str) -> Result<Ext, String> {
    let exe = ytdlp_path().ok_or_else(|| "yt-dlp is not installed - press GET YT-DLP".to_string())?;
    let url = format!("https://www.youtube.com/watch?v={}", vid);
    let out = command(&exe)
        .args(["--no-playlist", "--no-warnings", "--dump-single-json"])
        .arg(&url)
        .output()
        .map_err(|e| format!("could not run yt-dlp: {}", e))?;
    if !out.status.success() {
        return Err(last_line(&out.stderr));
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let title = v["title"].as_str().unwrap_or("YouTube clip").to_string();
    let artist = v["artist"]
        .as_str()
        .or_else(|| v["uploader"].as_str())
        .or_else(|| v["channel"].as_str())
        .unwrap_or("YouTube")
        .replace(" - Topic", "");
    Ok(Ext {
        kind: "yt".to_string(),
        id: hash_id(&format!("yt:{}", vid)),
        src: vid.to_string(),
        title,
        artist,
        dur: v["duration"].as_f64().unwrap_or(0.0) as f32,
        cover: format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", vid),
    })
}

/// Download the audio of a clip (AAC in an m4a, which the player can decode) and return the bytes.
pub fn yt_fetch(vid: &str) -> Result<Vec<u8>, String> {
    let exe = ytdlp_path().ok_or_else(|| "yt-dlp is not installed - press GET YT-DLP".to_string())?;
    let tmp = crate::api::config_dir().join("tmp");
    let _ = std::fs::create_dir_all(&tmp);
    if let Ok(rd) = std::fs::read_dir(&tmp) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().starts_with(vid) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let url = format!("https://www.youtube.com/watch?v={}", vid);
    let out = command(&exe)
        .args(["--no-playlist", "--no-warnings", "--no-progress", "-f", "140/bestaudio[ext=m4a]/bestaudio"])
        .arg("-o")
        .arg(tmp.join(format!("{}.%(ext)s", vid)))
        .arg(&url)
        .output()
        .map_err(|e| format!("could not run yt-dlp: {}", e))?;
    if !out.status.success() {
        return Err(last_line(&out.stderr));
    }
    let mut found: Option<PathBuf> = None;
    if let Ok(rd) = std::fs::read_dir(&tmp) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().starts_with(vid) {
                found = Some(e.path());
            }
        }
    }
    let path = found.ok_or_else(|| "yt-dlp finished but the file is missing".to_string())?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let bytes = std::fs::read(&path).map_err(|e| e.to_string());
    let _ = std::fs::remove_file(&path);
    if ext == "webm" || ext == "opus" {
        return Err("YouTube only offered Opus audio - press UPDATE YT-DLP and try again".to_string());
    }
    bytes
}

// --------------------------------------------------------------- analysis
/// Loudness envelope, 50 values per second (0..255), for drawing the waveform.
pub fn wave_peaks(bytes: Vec<u8>) -> Option<Vec<u8>> {
    let src = SymSource::new(bytes).ok()?;
    let ch = src.channels().max(1) as usize;
    let per = (src.sample_rate() / 50).max(1) as usize;
    let mut out = Vec::new();
    let (mut peak, mut n, mut c) = (0i32, 0usize, 0usize);
    for s in src {
        peak = peak.max((s as i32).abs());
        c += 1;
        if c == ch {
            c = 0;
            n += 1;
            if n == per {
                let v = (peak as f32 / 32768.0).clamp(0.0, 1.0).powf(0.6);
                out.push((v * 255.0) as u8);
                peak = 0;
                n = 0;
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Write the part between `a` and `b` seconds as a 16-bit WAV file.
pub fn export_wav(bytes: Vec<u8>, a: f32, b: f32, path: &Path) -> Result<(), String> {
    let mut src = SymSource::new(bytes)?;
    let (ch, rate) = (src.channels() as usize, src.sample_rate());
    src.try_seek(Duration::from_secs_f32(a.max(0.0))).map_err(|_| "can't seek in this file".to_string())?;
    let want = ((b - a).max(0.0) * rate as f32) as usize * ch;
    let data: Vec<i16> = src.take(want).collect();
    if data.is_empty() {
        return Err("nothing to export".to_string());
    }
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let mut f = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let len = (data.len() * 2) as u32;
    let mut h: Vec<u8> = Vec::with_capacity(44);
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(36 + len).to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes());
    h.extend_from_slice(&(ch as u16).to_le_bytes());
    h.extend_from_slice(&rate.to_le_bytes());
    h.extend_from_slice(&(rate * ch as u32 * 2).to_le_bytes());
    h.extend_from_slice(&((ch * 2) as u16).to_le_bytes());
    h.extend_from_slice(&16u16.to_le_bytes());
    h.extend_from_slice(b"data");
    h.extend_from_slice(&len.to_le_bytes());
    f.write_all(&h).map_err(|e| e.to_string())?;
    let mut buf = Vec::with_capacity(data.len() * 2);
    for s in data {
        buf.extend_from_slice(&s.to_le_bytes());
    }
    f.write_all(&buf).map_err(|e| e.to_string())
}

// ------------------------------------------------------------ tune lookup
fn urlenc(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{:02X}", b));
        }
    }
    o
}

/// Who wrote a tune, from MusicBrainz (free, no key). Only called when you ask for it.
pub fn lookup_composer(name: &str) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("Tidalite/0.7 (personal practice app)")
        .build()
        .map_err(|e| e.to_string())?;
    let get = |url: String| -> Result<serde_json::Value, String> {
        client
            .get(url)
            .send()
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json::<serde_json::Value>()
            .map_err(|e| e.to_string())
    };
    let q = format!("work:\"{}\"", name);
    let v = get(format!("https://musicbrainz.org/ws/2/work/?query={}&fmt=json&limit=5", urlenc(&q)))?;
    let works = v["works"].as_array().cloned().unwrap_or_default();
    let want = name.to_lowercase();
    let pick = works
        .iter()
        .find(|w| w["title"].as_str().map(|t| t.to_lowercase() == want).unwrap_or(false))
        .or_else(|| works.first())
        .ok_or_else(|| "no work found".to_string())?;
    let id = pick["id"].as_str().ok_or_else(|| "no work id".to_string())?.to_string();
    std::thread::sleep(Duration::from_millis(1100)); // MusicBrainz asks for at most one request per second
    let w = get(format!("https://musicbrainz.org/ws/2/work/{}?inc=artist-rels&fmt=json", id))?;
    let mut writers: Vec<String> = Vec::new();
    for r in w["relations"].as_array().cloned().unwrap_or_default() {
        let t = r["type"].as_str().unwrap_or("");
        if t == "composer" || t == "writer" || t == "lyricist" {
            if let Some(n) = r["artist"]["name"].as_str() {
                let tag = if t == "lyricist" { format!("{} (words)", n) } else { n.to_string() };
                if !writers.contains(&tag) {
                    writers.push(tag);
                }
            }
        }
    }
    let title = pick["title"].as_str().unwrap_or(name);
    if writers.is_empty() {
        Ok(format!("{}: no writer listed", title))
    } else {
        Ok(format!("{} - written by {}", title, writers.join(", ")))
    }
}
