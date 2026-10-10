//! Local copy of every track that gets played, so it can be replayed (and looped) without
//! downloading again. Files live in `<config dir>/cache/<track id>.<ext>`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

fn set() -> &'static Mutex<HashSet<i64>> {
    static S: OnceLock<Mutex<HashSet<i64>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashSet::new()))
}

pub fn dir() -> PathBuf {
    crate::api::config_dir().join("cache")
}

/// Is this track stored on this computer?
pub fn has(id: i64) -> bool {
    set().lock().map(|s| s.contains(&id)).unwrap_or(false)
}

/// Read the folder once at startup.
pub fn scan() {
    let mut found = HashSet::new();
    if let Ok(rd) = std::fs::read_dir(dir()) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if let Some(stem) = name.split('.').next() {
                if let Ok(id) = stem.parse::<i64>() {
                    if !name.ends_with(".part") {
                        found.insert(id);
                    }
                }
            }
        }
    }
    if let Ok(mut s) = set().lock() {
        *s = found;
    }
}

pub fn path_for(id: i64) -> Option<PathBuf> {
    for ext in ["m4a", "flac", "mp3"] {
        let p = dir().join(format!("{}.{}", id, ext));
        if p.exists() {
            return Some(p);
        }
    }
    None
}

pub fn get(id: i64) -> Option<Vec<u8>> {
    let p = path_for(id)?;
    match std::fs::read(&p) {
        Ok(b) if b.len() > 1024 => Some(b),
        _ => None,
    }
}

/// Save a downloaded track. Written under a temporary name first, so a crash never
/// leaves a half-written file that looks complete.
pub fn put(id: i64, bytes: &[u8]) -> Option<PathBuf> {
    let ext = if bytes.starts_with(b"fLaC") {
        "flac"
    } else if bytes.starts_with(b"ID3") || (bytes.len() > 2 && bytes[0] == 0xFF && bytes[1] & 0xE0 == 0xE0) {
        "mp3"
    } else {
        "m4a"
    };
    let d = dir();
    std::fs::create_dir_all(&d).ok()?;
    let tmp = d.join(format!("{}.{}.part", id, ext));
    let fin = d.join(format!("{}.{}", id, ext));
    std::fs::write(&tmp, bytes).ok()?;
    std::fs::rename(&tmp, &fin).ok()?;
    if let Ok(mut s) = set().lock() {
        s.insert(id);
    }
    Some(fin)
}

/// (number of files, total bytes)
pub fn stats() -> (usize, u64) {
    let mut n = 0;
    let mut total = 0;
    if let Ok(rd) = std::fs::read_dir(dir()) {
        for e in rd.flatten() {
            if let Ok(m) = e.metadata() {
                if m.is_file() {
                    n += 1;
                    total += m.len();
                }
            }
        }
    }
    (n, total)
}

pub fn remove(id: i64) {
    if let Some(p) = path_for(id) {
        let _ = std::fs::remove_file(p);
    }
    if let Ok(mut s) = set().lock() {
        s.remove(&id);
    }
}

pub fn clear() {
    if let Ok(rd) = std::fs::read_dir(dir()) {
        for e in rd.flatten() {
            let _ = std::fs::remove_file(e.path());
        }
    }
    if let Ok(mut s) = set().lock() {
        s.clear();
    }
}

pub fn open_folder() {
    let d = dir();
    let _ = std::fs::create_dir_all(&d);
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer").arg(&d).spawn();
    }
    #[cfg(not(windows))]
    {
        let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        let _ = std::process::Command::new(opener).arg(&d).spawn();
    }
}

pub fn fmt_size(b: u64) -> String {
    if b >= 1 << 30 {
        format!("{:.1} GB", b as f64 / (1u64 << 30) as f64)
    } else if b >= 1 << 20 {
        format!("{:.0} MB", b as f64 / (1u64 << 20) as f64)
    } else {
        format!("{} KB", b / 1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(fmt_size(0), "0 KB");
        assert_eq!(fmt_size(1536), "1 KB");
        assert_eq!(fmt_size(5 << 20), "5 MB");
        assert_eq!(fmt_size(3 << 29), "1.5 GB");
    }
}
