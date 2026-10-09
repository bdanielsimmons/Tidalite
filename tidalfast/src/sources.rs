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
    } else if cfg!(target_os = "macos") {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos"
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
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&fin, std::fs::Permissions::from_mode(0o755));
    }
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

/// Which browser's SoundCloud sign-in yt-dlp borrows: 0 none, 1 firefox, 2 chrome, 3 edge.
pub static SC_BROWSER: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
pub const SC_BROWSERS: [&str; 4] = ["OFF", "FIREFOX", "CHROME", "EDGE"];

/// `--cookies-from-browser <name>` for SoundCloud requests when switched on.
fn cookie_args() -> Vec<String> {
    match SC_BROWSER.load(std::sync::atomic::Ordering::Relaxed) {
        1 => vec!["--cookies-from-browser".into(), "firefox".into()],
        2 => vec!["--cookies-from-browser".into(), "chrome".into()],
        3 => vec!["--cookies-from-browser".into(), "edge".into()],
        _ => Vec::new(),
    }
}

/// A YouTube video id, or (SoundCloud and others) a full link.
fn media_url(src: &str) -> String {
    if src.starts_with("http") {
        src.to_string()
    } else {
        format!("https://www.youtube.com/watch?v={}", src)
    }
}

/// A file-name-safe stem for the temporary download of `src`.
fn file_stem(src: &str) -> String {
    if src.starts_with("http") {
        format!("sc{}", hash_id(src).unsigned_abs())
    } else {
        src.to_string()
    }
}

/// "soundcloud.com/some-artist/cool-track" -> ("Some Artist", "Cool Track"), when the link has that shape.
fn slug_meta(url: &str) -> Option<(String, String)> {
    let rest = url.split("://").nth(1)?;
    let (host, path) = rest.split_once('/')?;
    if !host.ends_with("soundcloud.com") || host.starts_with("api.") {
        return None;
    }
    let segs: Vec<&str> = path.split(['/', '?']).filter(|s| !s.is_empty()).collect();
    if segs.len() < 2 || ["sets", "likes", "reposts", "tracks", "albums", "popular-tracks"].contains(&segs[1]) {
        return None;
    }
    let pretty = |s: &str| -> String {
        s.split('-')
            .map(|w| {
                let mut c = w.chars();
                c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    Some((pretty(segs[0]), pretty(segs[1])))
}

/// SoundCloud entries as yt-dlp lists them (one JSON object per line, "flat" = no per-track requests).
fn parse_entries(bytes: &[u8]) -> Vec<Ext> {
    let mut out: Vec<Ext> = Vec::new();
    for line in String::from_utf8_lossy(bytes).lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let url = v["webpage_url"].as_str().or_else(|| v["url"].as_str()).unwrap_or("");
        if !url.starts_with("http") || !url.contains("soundcloud.com") || out.iter().any(|e| e.src == url) {
            continue;
        }
        let cover = v["thumbnail"]
            .as_str()
            .map(|s| s.to_string())
            .or_else(|| v["thumbnails"].as_array().and_then(|a| a.last()).and_then(|t| t["url"].as_str()).map(|s| s.to_string()))
            .unwrap_or_default();
        out.push(Ext {
            kind: "yt".to_string(),
            id: hash_id(&format!("yt:{}", url)),
            src: url.to_string(),
            // the quick listing has no titles: borrow them from the link until the real ones arrive
            title: v["title"]
                .as_str()
                .map(|s| s.to_string())
                .or_else(|| slug_meta(url).map(|m| m.1))
                .unwrap_or_else(|| "SoundCloud track".to_string()),
            artist: v["uploader"]
                .as_str()
                .or_else(|| v["channel"].as_str())
                .map(|s| s.to_string())
                .or_else(|| slug_meta(url).map(|m| m.0))
                .unwrap_or_else(|| "SoundCloud".to_string()),
            dur: v["duration"].as_f64().unwrap_or(0.0) as f32,
            cover,
        });
    }
    out
}

/// Search SoundCloud (no account needed).
pub fn sc_search(query: &str) -> Result<Vec<Ext>, String> {
    let exe = ytdlp_path().ok_or_else(|| "yt-dlp is not installed - press GET YT-DLP".to_string())?;
    let out = command(&exe)
        .args(["--no-warnings", "--flat-playlist", "--dump-json"])
        .args(cookie_args())
        .arg(format!("scsearch15:{}", query))
        .output()
        .map_err(|e| format!("could not run yt-dlp: {}", e))?;
    let list = parse_entries(&out.stdout);
    if list.is_empty() {
        return Err(if out.status.success() { "nothing found".to_string() } else { last_line(&out.stderr) });
    }
    Ok(list)
}

/// A SoundCloud track, playlist, likes page or profile link: list what is in it (first 100).
pub fn sc_list(url: &str) -> Result<Vec<Ext>, String> {
    let exe = ytdlp_path().ok_or_else(|| "yt-dlp is not installed - press GET YT-DLP".to_string())?;
    let out = command(&exe)
        .args(["--no-warnings", "--flat-playlist", "--dump-json", "--playlist-end", "100"])
        .args(cookie_args())
        .arg(url.trim())
        .output()
        .map_err(|e| format!("could not run yt-dlp: {}", e))?;
    let mut list = parse_entries(&out.stdout);
    if list.is_empty() && out.status.success() {
        // a single track comes back as one full object
        list = parse_entries(&out.stdout);
    }
    if list.is_empty() {
        return Err(if out.status.success() { "nothing found at that link".to_string() } else { last_line(&out.stderr) });
    }
    Ok(list)
}

/// A YouTube playlist link (not a single video that happens to sit in one).
pub fn is_yt_list(s: &str) -> bool {
    s.contains("youtu") && s.contains("playlist?list=")
}

/// A readable name from a link's last part: ".../sets/my-cool-mix" -> "My Cool Mix".
pub fn list_name(url: &str) -> String {
    let last = url.split('?').next().unwrap_or(url).trim_end_matches('/').rsplit('/').next().unwrap_or("playlist");
    let name: Vec<String> = last
        .split(|c| c == '-' || c == '_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
        .collect();
    if name.is_empty() { "Playlist".to_string() } else { name.join(" ") }
}

/// A YouTube playlist: what is in it (first 200), quick listing without per-video requests.
pub fn yt_list(url: &str) -> Result<Vec<Ext>, String> {
    let exe = ytdlp_path().ok_or_else(|| "yt-dlp is not installed - press GET YT-DLP".to_string())?;
    let out = command(&exe)
        .args(["--no-warnings", "--flat-playlist", "--dump-json", "--playlist-end", "200"])
        .arg(url.trim())
        .output()
        .map_err(|e| format!("could not run yt-dlp: {}", e))?;
    let mut list: Vec<Ext> = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let Some(vid) = v["id"].as_str().filter(|s| valid_id(s)) else { continue };
        if list.iter().any(|e| e.src == vid) {
            continue;
        }
        list.push(Ext {
            kind: "yt".to_string(),
            id: hash_id(&format!("yt:{}", vid)),
            src: vid.to_string(),
            title: v["title"].as_str().unwrap_or("YouTube clip").to_string(),
            artist: v["channel"].as_str().or_else(|| v["uploader"].as_str()).unwrap_or("YouTube").replace(" - Topic", ""),
            dur: v["duration"].as_f64().unwrap_or(0.0) as f32,
            cover: format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", vid),
        });
    }
    if list.is_empty() {
        return Err(if out.status.success() { "nothing found at that link".to_string() } else { last_line(&out.stderr) });
    }
    Ok(list)
}

/// A SoundCloud user (name or profile link): their playlists as (title, link).
pub fn sc_sets(user: &str) -> Result<Vec<(String, String)>, String> {
    let exe = ytdlp_path().ok_or_else(|| "yt-dlp is not installed - press GET YT-DLP".to_string())?;
    let u = user.trim().trim_end_matches('/');
    let name = u.trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("www.");
    let name = name.trim_start_matches("soundcloud.com/").trim_start_matches('@');
    let name = name.split('/').next().unwrap_or("");
    if name.is_empty() {
        return Err("type a SoundCloud user name or paste a profile link".to_string());
    }
    let url = format!("https://soundcloud.com/{}/sets", name);
    let out = command(&exe)
        .args(["--no-warnings", "--flat-playlist", "--dump-json", "--playlist-end", "60"])
        .args(cookie_args())
        .arg(&url)
        .output()
        .map_err(|e| format!("could not run yt-dlp: {}", e))?;
    let mut list = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let link = v["url"].as_str().or_else(|| v["webpage_url"].as_str()).unwrap_or("");
        if link.is_empty() {
            continue;
        }
        let title = v["title"].as_str().unwrap_or(link).to_string();
        list.push((title, link.to_string()));
    }
    if list.is_empty() {
        return Err(if out.status.success() {
            "no public playlists found for that user".to_string()
        } else {
            last_line(&out.stderr)
        });
    }
    Ok(list)
}

/// Title / uploader / length / thumbnail of a clip.
pub fn yt_info(vid: &str) -> Result<Ext, String> {
    let exe = ytdlp_path().ok_or_else(|| "yt-dlp is not installed - press GET YT-DLP".to_string())?;
    let url = media_url(vid);
    let out = command(&exe)
        .args(["--no-playlist", "--no-warnings", "--dump-single-json"])
        .args(if url.contains("soundcloud.com") { cookie_args() } else { Vec::new() })
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
        .unwrap_or(if vid.starts_with("http") { "SoundCloud" } else { "YouTube" })
        .replace(" - Topic", "");
    Ok(Ext {
        kind: "yt".to_string(),
        id: hash_id(&format!("yt:{}", vid)),
        src: vid.to_string(),
        title,
        artist,
        dur: v["duration"].as_f64().unwrap_or(0.0) as f32,
        cover: if vid.starts_with("http") {
            v["thumbnail"].as_str().unwrap_or("").to_string()
        } else {
            format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", vid)
        },
    })
}

/// Download the audio of a clip (AAC in an m4a, which the player can decode) and return the bytes.
pub fn yt_fetch(src: &str) -> Result<Vec<u8>, String> {
    let exe = ytdlp_path().ok_or_else(|| "yt-dlp is not installed - press GET YT-DLP".to_string())?;
    let tmp = crate::api::config_dir().join("tmp");
    let _ = std::fs::create_dir_all(&tmp);
    let stem = file_stem(src);
    let vid = stem.as_str();
    if let Ok(rd) = std::fs::read_dir(&tmp) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().starts_with(vid) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let url = media_url(src);
    let out = command(&exe)
        .args([
            "--no-playlist",
            "--no-warnings",
            "--no-progress",
            "-f",
            "140/http_mp3_1_0/http_mp3_0_0/bestaudio[ext=mp3]/bestaudio[ext=m4a]/bestaudio",
        ])
        .args(if url.contains("soundcloud.com") { cookie_args() } else { Vec::new() })
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
    if ext == "webm" || ext == "opus" || ext == "ogg" {
        return Err("Only Opus audio was offered - press UPDATE YT-DLP and try again".to_string());
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

/// Chord changes for a tune from the open Jazz Standards data (about 1,300 songs).
/// The file is downloaded once and kept next to the other app data. On a miss the error
/// carries the closest titles, for "did you mean" buttons.
pub fn find_chart(name: &str) -> Result<(String, String, String), (String, Vec<String>)> {
    let fail = |e: String| (e, Vec::new());
    let path = crate::api::config_dir().join("charts.json");
    if !path.exists() {
        let client =
            reqwest::blocking::Client::builder().timeout(Duration::from_secs(40)).build().map_err(|e| fail(e.to_string()))?;
        let body = client
            .get("https://raw.githubusercontent.com/mikeoliphant/JazzStandards/main/JazzStandards.json")
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.text())
            .map_err(|e| fail(format!("couldn't reach the chart list ({})", e)))?;
        let _ = std::fs::create_dir_all(crate::api::config_dir());
        std::fs::write(&path, body).map_err(|e| fail(e.to_string()))?;
    }
    let text = std::fs::read_to_string(&path).map_err(|e| fail(e.to_string()))?;
    let list: Vec<serde_json::Value> = serde_json::from_str(&text).map_err(|e| {
        let _ = std::fs::remove_file(&path);
        fail(e.to_string())
    })?;
    match crate::chart::find_standard(&list, name) {
        Some(song) => crate::chart::from_standard(song).ok_or_else(|| fail("that chart has no chords".to_string())),
        None => Err(("not in the free chart list (jazz standards only)".to_string(), crate::chart::suggest(&list, name, 5))),
    }
}
