//! Everything you build up while practicing, saved in `%APPDATA%\tidalite\library.json`:
//! your tune list (with the versions worth studying), named sections, the practice diary,
//! and the files / YouTube clips you added.

use crate::api::Track;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

/// A playlist you built inside the player: any mix of Tidal, SoundCloud, YouTube and file tracks.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Playlist {
    pub name: String,
    pub items: Vec<Version>,
}

/// A playlist link kept for later: SoundCloud or YouTube (`kind` = "sc" | "yt").
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct SavedList {
    pub kind: String,
    pub name: String,
    pub url: String,
}

/// A recording of a tune: from Tidal, a file on this computer, or a YouTube clip.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Version {
    pub kind: String, // "tidal" | "file" | "yt"
    pub id: i64,
    pub src: String, // file path or YouTube video id
    pub title: String,
    pub artist: String,
    pub album: String,
    pub cover: String,
    pub dur: f32,
    pub stars: u8,
    pub note: String,
}

impl Version {
    pub fn to_track(&self) -> Track {
        Track {
            id: self.id,
            title: self.title.clone(),
            artist: self.artist.clone(),
            artists: Vec::new(),
            album: self.album.clone(),
            cover: self.cover.clone(),
            duration: self.dur,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Tune {
    pub name: String,
    /// 0 learning, 1 working on it, 2 ready to play
    pub status: u8,
    pub key: String,
    pub bpm: u32,
    pub notes: String,
    /// lead sheet as plain text, e.g. `T44 *A | Dm7 G7 | Cmaj7 | ...`
    pub chart: String,
    /// who wrote it etc. (filled by LOOK UP, kept so it is only searched once)
    pub info: String,
    pub versions: Vec<Version>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Section {
    pub name: String,
    pub a: f32,
    pub b: f32,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Day {
    pub date: String,
    pub secs: BTreeMap<String, u32>,
    pub loops: u32,
    pub pomos: u32,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Entry {
    pub date: String,
    pub tune: String,
    pub mins: u32,
    pub bpm: u32,
    pub note: String,
}

/// A file or YouTube clip in the FILES / YT lists.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Ext {
    pub kind: String, // "file" | "yt"
    pub id: i64,
    pub src: String,
    pub title: String,
    pub artist: String,
    pub dur: f32,
    pub cover: String,
}

impl Ext {
    pub fn to_track(&self) -> Track {
        Track {
            id: self.id,
            title: self.title.clone(),
            artist: self.artist.clone(),
            artists: Vec::new(),
            album: String::new(),
            cover: self.cover.clone(),
            duration: self.dur,
        }
    }

    pub fn to_version(&self) -> Version {
        Version {
            kind: self.kind.clone(),
            id: self.id,
            src: self.src.clone(),
            title: self.title.clone(),
            artist: self.artist.clone(),
            cover: self.cover.clone(),
            dur: self.dur,
            ..Default::default()
        }
    }
}

/// What was playing when the app was closed ("Continue where I left off").
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Last {
    pub version: Version,
    pub pos: f32,
    pub speed: u32,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Store {
    pub tunes: Vec<Tune>,
    pub sections: HashMap<i64, Vec<Section>>,
    pub days: Vec<Day>,
    pub entries: Vec<Entry>,
    pub folders: Vec<String>,
    pub files: Vec<Ext>,
    pub yt: Vec<Ext>,
    /// SoundCloud tracks you kept (same kind of entry as a YouTube clip, with a link instead of a video id)
    pub sc: Vec<Ext>,
    /// Hearted tracks that are not on Tidal (files, YouTube, SoundCloud); kept on this computer only
    pub hearts: Vec<Ext>,
    /// SoundCloud / YouTube playlists you saved by link
    pub lists: Vec<SavedList>,
    pub playlists: Vec<Playlist>,
    pub bpm: HashMap<i64, u32>,
    /// tempo and key found by listening to the audio (see meta.rs)
    pub meta: HashMap<i64, crate::meta::Info>,
    pub last: Option<Last>,
}

fn path() -> PathBuf {
    crate::api::config_dir().join("library.json")
}

impl Store {
    pub fn load() -> Store {
        std::fs::read_to_string(path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        let dir = crate::api::config_dir();
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(t) = serde_json::to_string(self) {
            let tmp = dir.join("library.json.tmp");
            if std::fs::write(&tmp, t).is_ok() {
                let _ = std::fs::rename(&tmp, path());
            }
        }
    }

    pub fn day_mut(&mut self, date: &str) -> &mut Day {
        if let Some(i) = self.days.iter().position(|d| d.date == date) {
            return &mut self.days[i];
        }
        self.days.push(Day { date: date.to_string(), ..Default::default() });
        let n = self.days.len();
        &mut self.days[n - 1]
    }

    pub fn secs_on(&self, date: &str) -> u32 {
        self.days.iter().find(|d| d.date == date).map(|d| d.secs.values().sum()).unwrap_or(0)
    }

    /// Consecutive practice days up to today (a day you haven't practiced *yet* doesn't break it).
    pub fn streak(&self, today: i64) -> u32 {
        let has = |day: i64| self.days.iter().any(|d| parse_date(&d.date) == Some(day) && d.secs.values().sum::<u32>() >= 60);
        let mut n = 0;
        let mut day = if has(today) { today } else { today - 1 };
        while has(day) {
            n += 1;
            day -= 1;
        }
        n
    }

    pub fn tune_of(&self, id: i64) -> Option<usize> {
        self.tunes.iter().position(|t| t.versions.iter().any(|v| v.id == id))
    }

    pub fn tune_secs(&self, name: &str) -> u32 {
        self.days.iter().map(|d| d.secs.get(name).copied().unwrap_or(0)).sum()
    }
}

// ------------------------------------------------------------------ dates
/// Local calendar date (year, month, day).
#[cfg(windows)]
pub fn today() -> (i32, u32, u32) {
    #[repr(C)]
    struct St {
        y: u16,
        mo: u16,
        dow: u16,
        d: u16,
        h: u16,
        mi: u16,
        s: u16,
        ms: u16,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetLocalTime(st: *mut St);
    }
    unsafe {
        let mut st: St = std::mem::zeroed();
        GetLocalTime(&mut st);
        (st.y as i32, st.mo as u32, st.d as u32)
    }
}

/// Mac / Linux: ask the system for the local date (so the diary rolls over at local midnight, not UTC midnight).
#[cfg(not(windows))]
pub fn today() -> (i32, u32, u32) {
    #[repr(C)]
    struct Tm {
        sec: i32,
        min: i32,
        hour: i32,
        mday: i32,
        mon: i32,
        year: i32,
        wday: i32,
        yday: i32,
        isdst: i32,
        gmtoff: i64,
        zone: *const u8,
    }
    extern "C" {
        fn localtime_r(t: *const i64, out: *mut Tm) -> *mut Tm;
    }
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let mut tm =
        Tm { sec: 0, min: 0, hour: 0, mday: 0, mon: 0, year: 0, wday: 0, yday: 0, isdst: 0, gmtoff: 0, zone: std::ptr::null() };
    let ok = unsafe { !localtime_r(&secs, &mut tm).is_null() };
    if ok && tm.mday > 0 {
        (tm.year + 1900, (tm.mon + 1) as u32, tm.mday as u32)
    } else {
        civil_from_days(secs.div_euclid(86400))
    }
}

pub fn today_str() -> String {
    let (y, m, d) = today();
    format!("{:04}-{:02}-{:02}", y, m, d)
}

pub fn today_days() -> i64 {
    let (y, m, d) = today();
    days_from_civil(y, m, d)
}

/// Days since 1970-01-01.
pub fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y as i64 - 1 } else { y as i64 };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m: u32 = if mp < 10 { (mp + 3) as u32 } else { (mp - 9) as u32 };
    let yy: i64 = if m <= 2 { y + 1 } else { y };
    (yy as i32, m, d)
}

pub fn parse_date(s: &str) -> Option<i64> {
    let mut it = s.split('-');
    let y = it.next()?.parse::<i32>().ok()?;
    let m = it.next()?.parse::<u32>().ok()?;
    let d = it.next()?.parse::<u32>().ok()?;
    Some(days_from_civil(y, m, d))
}

pub fn date_str(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

pub fn weekday(days: i64) -> &'static str {
    ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"][((days + 4).rem_euclid(7)) as usize]
}

/// Stable id for a file path / YouTube clip. Always negative so it can't clash with Tidal ids.
pub fn hash_id(s: &str) -> i64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    -((h & 0x3fff_ffff_ffff_ffff) as i64) - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_round_trip() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11017);
        assert_eq!(date_str(0), "1970-01-01");
        // every day for ~60 years survives the trip, leap days included
        for z in -3650..20000 {
            let (y, m, d) = civil_from_days(z);
            assert_eq!(days_from_civil(y, m, d), z);
        }
        assert_eq!(date_str(parse_date("2024-02-29").unwrap()), "2024-02-29");
        assert_eq!(parse_date("2024-03-01").unwrap() - parse_date("2024-02-28").unwrap(), 2);
        assert_eq!(parse_date("2023-03-01").unwrap() - parse_date("2023-02-28").unwrap(), 1);
        assert_eq!(parse_date("garbage"), None);
        assert_eq!(parse_date("2024-02"), None);
    }

    #[test]
    fn weekdays() {
        assert_eq!(weekday(0), "THU"); // 1970-01-01
        assert_eq!(weekday(parse_date("2026-10-09").unwrap()), "FRI");
        assert_eq!(weekday(-1), "WED");
    }

    #[test]
    fn ids_for_files_never_clash_with_tidal() {
        for s in ["", "a", "C:/music/song.flac", "yt:dQw4w9WgXcQ"] {
            assert!(hash_id(s) < 0, "{}", s);
            assert_eq!(hash_id(s), hash_id(s), "stable");
        }
        assert_ne!(hash_id("a"), hash_id("b"));
    }

    fn day(date: &str, secs: u32) -> Day {
        let mut d = Day { date: date.to_string(), ..Default::default() };
        d.secs.insert("tune".to_string(), secs);
        d
    }

    #[test]
    fn practice_streak() {
        let today = parse_date("2026-10-09").unwrap();
        let mut s = Store::default();
        assert_eq!(s.streak(today), 0);
        s.days = vec![day("2026-10-06", 300), day("2026-10-07", 120), day("2026-10-08", 600)];
        // not practiced yet today: yesterday's streak still counts
        assert_eq!(s.streak(today), 3);
        s.days.push(day("2026-10-09", 90));
        assert_eq!(s.streak(today), 4);
        // under a minute is not a practice day, and it breaks the chain
        s.days[1].secs.insert("tune".to_string(), 30);
        assert_eq!(s.streak(today), 2);
        assert_eq!(s.secs_on("2026-10-08"), 600);
        assert_eq!(s.secs_on("2001-01-01"), 0);
    }

    #[test]
    fn old_library_files_still_load() {
        // fields added later default instead of failing the whole file
        let s: Store = serde_json::from_str(r#"{"folders": ["C:/music"], "bpm": {"5": 120}}"#).unwrap();
        assert_eq!(s.folders, vec!["C:/music".to_string()]);
        assert_eq!(s.bpm.get(&5), Some(&120));
        assert!(s.meta.is_empty() && s.playlists.is_empty());
    }
}
