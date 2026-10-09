//! Tidal API layer: device-code login, token refresh, browsing, stream URLs.
//! Unofficial endpoints (same ones the Tidal apps and the `tidalapi` Python lib use).

use base64::Engine as _;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub type Res<T> = Result<T, String>;

const API: &str = "https://api.tidal.com/v1";
const AUTH: &str = "https://auth.tidal.com/v1/oauth2";
// Public client credentials as published in the open-source `tidalapi` library.
// Tidal rotates these occasionally. Override without recompiling by creating
// <config dir>/tidalite/credentials.json: {"client_id":"...","client_secret":"..."}
const DEFAULT_CLIENT_ID: &str = "fX2JxdmntZWK0ixT";
const DEFAULT_CLIENT_SECRET: &str = "1Nn9AfDAjxrgJFJbKNWLeAyKGVGmINuXPPLHVXAvxAg=";
const SCOPE: &str = "r_usr w_usr w_sub";

fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("tidalite")
}

/// Carry the saved login over from the old "tidalfast" folder (first run after the rename).
fn migrate_config() {
    let old = dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("tidalfast");
    let new = config_dir();
    if !new.join("session.json").exists() {
        let _ = std::fs::create_dir_all(&new);
        for f in ["session.json", "credentials.json"] {
            let _ = std::fs::copy(old.join(f), new.join(f));
        }
    }
}

/// Append a line to %APPDATA%\tidalite\log.txt (handy when something won't play).
static LOG_RING: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn log(msg: &str) {
    use std::io::Write;
    let t = now() % 86400; // UTC time of day
    let line = format!("[{:02}:{:02}:{:02}] {}", t / 3600, (t / 60) % 60, t % 60, msg);
    if let Ok(mut r) = LOG_RING.lock() {
        r.push(line.clone());
        let n = r.len();
        if n > 400 {
            r.drain(0..n - 400);
        }
    }
    let dir = config_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("log.txt")) {
        let _ = writeln!(f, "{}", line);
    }
}

/// Everything logged this run (shown by the LOG button).
pub fn log_text() -> String {
    LOG_RING.lock().map(|r| r.join("\n")).unwrap_or_default()
}

// ---------------------------------------------------------------- data types

#[derive(Clone, Default)]
pub struct Track {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub cover: String,
    pub duration: f32,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Album,
    Playlist,
    Artist,
    Mix,
}

#[derive(Clone)]
pub struct Card {
    pub kind: Kind,
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub image: String,
}

#[derive(Clone, Default)]
pub struct Page {
    pub title: String,
    pub subtitle: String,
    pub image: String,
    pub tracks: Vec<Track>,
    pub rows: Vec<(String, Vec<Card>)>,
    pub rows_first: bool,
    /// The "Your Library" page (shown with MY TRACKS / LISTS / ALBUMS / ARTISTS tabs).
    pub library: bool,
    /// Total number of liked songs on Tidal (only set on the library page).
    pub total: usize,
}

pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub url: String,
    pub interval: u64,
    pub expires_in: u64,
}

#[derive(Clone, Default)]
struct Session {
    client_id: String,
    client_secret: String,
    access_token: String,
    refresh_token: String,
    expires_at: u64,
    user_id: i64,
    country: String,
}

impl Session {
    fn to_json(&self) -> Value {
        json!({
            "client_id": self.client_id,
            "client_secret": self.client_secret,
            "access_token": self.access_token,
            "refresh_token": self.refresh_token,
            "expires_at": self.expires_at,
            "user_id": self.user_id,
            "country": self.country,
        })
    }

    fn from_json(v: &Value) -> Option<Session> {
        Some(Session {
            client_id: v["client_id"].as_str()?.to_string(),
            client_secret: v["client_secret"].as_str()?.to_string(),
            access_token: v["access_token"].as_str()?.to_string(),
            refresh_token: v["refresh_token"].as_str().unwrap_or("").to_string(),
            expires_at: v["expires_at"].as_u64().unwrap_or(0),
            user_id: v["user_id"].as_i64().unwrap_or(0),
            country: v["country"].as_str().unwrap_or("").to_string(),
        })
    }
}

// ------------------------------------------------------------------- helpers

pub fn cover_url(uuid: &str, size: u32) -> String {
    image_url(uuid, size, size)
}

fn image_url(uuid: &str, w: u32, h: u32) -> String {
    if uuid.is_empty() {
        return String::new();
    }
    if uuid.starts_with("http") {
        return uuid.to_string(); // YouTube thumbnails are full URLs already
    }
    format!("https://resources.tidal.com/images/{}/{}x{}.jpg", uuid.replace('-', "/"), w, h)
}

fn arr(v: &Value) -> &[Value] {
    v.as_array().map(|a| a.as_slice()).unwrap_or(&[])
}

/// Many endpoints wrap the real object: {"item": {...}}, {"data": {...}}, {"playlist": {...}}
fn unwrap(v: &Value) -> &Value {
    for k in ["item", "data", "playlist"] {
        if let Some(x) = v.get(k) {
            if x.is_object() {
                return x;
            }
        }
    }
    v
}

fn parse_track(v: &Value) -> Option<Track> {
    if v["type"].as_str() == Some("video") {
        return None;
    }
    let t = unwrap(v);
    let id = t["id"].as_i64()?;
    t.get("album")?;
    let mut title = t["title"].as_str()?.to_string();
    if let Some(ver) = t["version"].as_str() {
        if !ver.is_empty() {
            title = format!("{} ({})", title, ver);
        }
    }
    let artist = t["artist"]["name"].as_str().or_else(|| t["artists"][0]["name"].as_str()).unwrap_or("").to_string();
    Some(Track {
        id,
        title,
        artist,
        album: t["album"]["title"].as_str().unwrap_or("").to_string(),
        cover: t["album"]["cover"].as_str().unwrap_or("").to_string(),
        duration: t["duration"].as_f64().unwrap_or(0.0) as f32,
    })
}

/// Parse "[mm:ss.xx] text" lines (LRC) into (seconds, text), sorted by time.
fn parse_lrc(s: &str) -> Vec<(f32, String)> {
    let mut out: Vec<(f32, String)> = Vec::new();
    for line in s.lines() {
        let mut rest = line.trim();
        let mut times: Vec<f32> = Vec::new();
        while rest.starts_with('[') {
            match rest.find(']') {
                Some(end) => {
                    if let Some((m, sec)) = rest[1..end].split_once(':') {
                        if let (Ok(m), Ok(sec)) = (m.trim().parse::<f32>(), sec.trim().parse::<f32>()) {
                            times.push(m * 60.0 + sec);
                        }
                    }
                    rest = rest[end + 1..].trim_start();
                }
                None => break,
            }
        }
        for t in times {
            out.push((t, rest.to_string()));
        }
    }
    out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    out
}

fn tracks_from(items: &[Value]) -> Vec<Track> {
    items.iter().filter_map(parse_track).collect()
}

fn parse_card(v: &Value) -> Option<Card> {
    let v = unwrap(v);
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();

    if let Some(uuid) = v["uuid"].as_str() {
        let image = if let Some(sq) = v["squareImage"].as_str() {
            image_url(sq, 320, 320)
        } else if let Some(im) = v["image"].as_str() {
            image_url(im, 320, 214)
        } else {
            String::new()
        };
        let subtitle = match v["numberOfTracks"].as_u64() {
            Some(n) => format!("{} tracks", n),
            None => "Playlist".to_string(),
        };
        return Some(Card { kind: Kind::Playlist, id: uuid.to_string(), title: s("title"), subtitle, image });
    }

    if v["mixType"].is_string() || (v["id"].is_string() && v["images"].is_object()) {
        let imgs = &v["images"];
        let image = imgs["MEDIUM"]["url"]
            .as_str()
            .or_else(|| imgs["LARGE"]["url"].as_str())
            .or_else(|| imgs["SMALL"]["url"].as_str())
            .unwrap_or("")
            .to_string();
        let title = v["title"].as_str().or_else(|| v["titleTextInfo"]["text"].as_str()).unwrap_or("Mix").to_string();
        let subtitle = v["subTitle"].as_str().or_else(|| v["subTitleTextInfo"]["text"].as_str()).unwrap_or("Mix").to_string();
        return Some(Card { kind: Kind::Mix, id: s("id"), title, subtitle, image });
    }

    if v["cover"].is_string() && v["title"].is_string() && v["id"].is_number() && v.get("album").is_none() {
        let artist = v["artist"]["name"].as_str().or_else(|| v["artists"][0]["name"].as_str()).unwrap_or("").to_string();
        return Some(Card {
            kind: Kind::Album,
            id: v["id"].as_i64()?.to_string(),
            title: s("title"),
            subtitle: artist,
            image: image_url(v["cover"].as_str().unwrap_or(""), 320, 320),
        });
    }

    if v["picture"].is_string() && v["name"].is_string() && v["id"].is_number() {
        return Some(Card {
            kind: Kind::Artist,
            id: v["id"].as_i64()?.to_string(),
            title: s("name"),
            subtitle: "Artist".to_string(),
            image: image_url(v["picture"].as_str().unwrap_or(""), 320, 320),
        });
    }
    None
}

fn cards_from(items: &[Value]) -> Vec<Card> {
    items.iter().filter_map(parse_card).collect()
}

// ----------------------------------------------------------------------- Api

#[derive(Clone)]
pub struct Api {
    http: reqwest::blocking::Client,
    session: Arc<Mutex<Session>>,
}

impl Api {
    pub fn new() -> Api {
        migrate_config();
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (tidalite)")
            .build()
            .expect("http client");

        if let Ok(m) = std::fs::metadata(config_dir().join("log.txt")) {
            if m.len() > 200_000 {
                let _ = std::fs::remove_file(config_dir().join("log.txt"));
            }
        }
        let mut s = Session {
            client_id: DEFAULT_CLIENT_ID.to_string(),
            client_secret: DEFAULT_CLIENT_SECRET.to_string(),
            ..Default::default()
        };
        if let Some(saved) = std::fs::read_to_string(config_dir().join("session.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| Session::from_json(&v))
        {
            s = saved;
        }
        if let Some(c) = std::fs::read_to_string(config_dir().join("credentials.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        {
            if let (Some(i), Some(sec)) = (c["client_id"].as_str(), c["client_secret"].as_str()) {
                s.client_id = i.to_string();
                s.client_secret = sec.to_string();
            }
        }
        Api { http, session: Arc::new(Mutex::new(s)) }
    }

    pub fn has_token(&self) -> bool {
        !self.session.lock().unwrap().access_token.is_empty()
    }

    fn uid(&self) -> i64 {
        self.session.lock().unwrap().user_id
    }

    fn creds(&self) -> (String, String) {
        let s = self.session.lock().unwrap();
        (s.client_id.clone(), s.client_secret.clone())
    }

    fn save(&self) {
        let s = self.session.lock().unwrap().clone();
        let dir = config_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("session.json"), s.to_json().to_string());
    }

    pub fn logout(&self) {
        {
            let mut s = self.session.lock().unwrap();
            s.access_token.clear();
            s.refresh_token.clear();
            s.expires_at = 0;
        }
        let _ = std::fs::remove_file(config_dir().join("session.json"));
    }

    // ---------------------------------------------------------------- login

    pub fn start_login(&self) -> Res<DeviceCode> {
        let (cid, _) = self.creds();
        let r = self
            .http
            .post(format!("{}/device_authorization", AUTH))
            .form(&[("client_id", cid.as_str()), ("scope", SCOPE)])
            .send()
            .map_err(e2s)?;
        let st = r.status();
        let v: Value = r.json().map_err(e2s)?;
        if !st.is_success() {
            return Err(format!("Device login failed ({}): {}", st.as_u16(), v));
        }
        let mut url = v["verificationUriComplete"]
            .as_str()
            .or_else(|| v["verificationUri"].as_str())
            .unwrap_or("link.tidal.com")
            .to_string();
        if !url.starts_with("http") {
            url = format!("https://{}", url);
        }
        Ok(DeviceCode {
            device_code: v["deviceCode"].as_str().ok_or("no deviceCode in reply")?.to_string(),
            user_code: v["userCode"].as_str().unwrap_or("").to_string(),
            url,
            interval: v["interval"].as_u64().unwrap_or(2),
            expires_in: v["expiresIn"].as_u64().unwrap_or(300),
        })
    }

    /// Blocks, polling until the user approves in the browser.
    pub fn finish_login(&self, dc: &DeviceCode) -> Res<()> {
        let deadline = now() + dc.expires_in;
        loop {
            if now() > deadline {
                return Err("Login timed out - try again.".to_string());
            }
            std::thread::sleep(Duration::from_secs(dc.interval.max(2)));
            let (cid, sec) = self.creds();
            let r = self
                .http
                .post(format!("{}/token", AUTH))
                .basic_auth(&cid, Some(&sec))
                .form(&[
                    ("client_id", cid.as_str()),
                    ("device_code", dc.device_code.as_str()),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ("scope", SCOPE),
                ])
                .send()
                .map_err(e2s)?;
            let st = r.status();
            let v: Value = r.json().unwrap_or(Value::Null);
            if st.is_success() {
                self.apply_token(&v)?;
                self.save();
                return Ok(());
            }
            let err = v["error"].as_str().unwrap_or("");
            if err == "authorization_pending" || err == "slow_down" {
                continue;
            }
            return Err(format!("Login failed: {}", v));
        }
    }

    fn apply_token(&self, v: &Value) -> Res<()> {
        let at = v["access_token"].as_str().ok_or("no access_token in reply")?;
        let mut s = self.session.lock().unwrap();
        s.access_token = at.to_string();
        if let Some(rt) = v["refresh_token"].as_str() {
            s.refresh_token = rt.to_string();
        }
        s.expires_at = now() + v["expires_in"].as_u64().unwrap_or(3600);
        if let Some(u) = v["user"]["userId"].as_i64().or_else(|| v["user_id"].as_i64()) {
            s.user_id = u;
        }
        if let Some(c) = v["user"]["countryCode"].as_str() {
            s.country = c.to_string();
        }
        Ok(())
    }

    fn refresh(&self) -> Res<()> {
        let (cid, sec) = self.creds();
        let rt = self.session.lock().unwrap().refresh_token.clone();
        if rt.is_empty() {
            return Err("Session expired - log in again.".to_string());
        }
        let r = self
            .http
            .post(format!("{}/token", AUTH))
            .basic_auth(&cid, Some(&sec))
            .form(&[
                ("client_id", cid.as_str()),
                ("refresh_token", rt.as_str()),
                ("grant_type", "refresh_token"),
                ("scope", SCOPE),
            ])
            .send()
            .map_err(e2s)?;
        let st = r.status();
        let v: Value = r.json().unwrap_or(Value::Null);
        if !st.is_success() {
            return Err(format!("Session expired ({}). Log in again.", st.as_u16()));
        }
        self.apply_token(&v)?;
        self.save();
        Ok(())
    }

    /// Fills in user id + country. Doubles as "is my token still good?".
    pub fn refresh_info(&self) -> Res<()> {
        let v = self.get("/sessions", &[])?;
        {
            let mut s = self.session.lock().unwrap();
            if let Some(c) = v["countryCode"].as_str() {
                s.country = c.to_string();
            }
            if let Some(u) = v["userId"].as_i64() {
                s.user_id = u;
            }
        }
        self.save();
        Ok(())
    }

    // -------------------------------------------------------------- requests

    pub fn get(&self, path: &str, params: &[(&str, &str)]) -> Res<Value> {
        let (exp, has_rt) = {
            let s = self.session.lock().unwrap();
            (s.expires_at, !s.refresh_token.is_empty())
        };
        if has_rt && exp > 0 && exp < now() + 60 {
            let _ = self.refresh();
        }
        for attempt in 0..2 {
            let (tok, country) = {
                let s = self.session.lock().unwrap();
                (s.access_token.clone(), s.country.clone())
            };
            let mut q: Vec<(&str, &str)> = params.to_vec();
            if !country.is_empty() {
                q.push(("countryCode", country.as_str()));
            }
            let t0 = std::time::Instant::now();
            let r = match self.http.get(format!("{}{}", API, path)).bearer_auth(&tok).query(&q).send() {
                Ok(r) => r,
                Err(e) => {
                    log(&format!("GET {} failed to send: {}", path, e));
                    return Err(e.to_string());
                }
            };
            let st = r.status();
            log(&format!("GET {} -> {} ({} ms)", path, st.as_u16(), t0.elapsed().as_millis()));
            if st.as_u16() == 401 && attempt == 0 {
                self.refresh()?;
                continue;
            }
            if !st.is_success() {
                let t = r.text().unwrap_or_default();
                log(&format!("  error body: {}", t.chars().take(300).collect::<String>()));
                return Err(format!("{} {}: {}", st.as_u16(), path, t.chars().take(160).collect::<String>()));
            }
            return r.json::<Value>().map_err(e2s);
        }
        Err("Unauthorized".to_string())
    }

    fn paged(&self, path: &str, extra: &[(&str, &str)], max: usize) -> Res<Vec<Value>> {
        let mut out: Vec<Value> = Vec::new();
        let mut off = 0usize;
        loop {
            let offs = off.to_string();
            let mut p: Vec<(&str, &str)> = extra.to_vec();
            p.push(("limit", "100"));
            p.push(("offset", offs.as_str()));
            let v = self.get(path, &p)?;
            let items = arr(&v["items"]).to_vec();
            let n = items.len();
            out.extend(items);
            off += n;
            let total = v["totalNumberOfItems"].as_u64().unwrap_or(0) as usize;
            if n == 0 || off >= max || (total > 0 && off >= total) || (total == 0 && n < 100) {
                break;
            }
        }
        Ok(out)
    }

    /// Raw bytes of any URL (cover art, audio file).
    pub fn fetch(&self, url: &str) -> Res<Vec<u8>> {
        let r = self.http.get(url).timeout(Duration::from_secs(600)).send().map_err(e2s)?.error_for_status().map_err(e2s)?;
        r.bytes().map(|b| b.to_vec()).map_err(e2s)
    }

    // ----------------------------------------------------------------- pages

    pub fn home(&self) -> Res<Page> {
        let mut p = Page { title: "Home".to_string(), ..Default::default() };
        if let Ok(v) = self.get("/pages/home", &[("deviceType", "BROWSER"), ("locale", "en_US")]) {
            for row in arr(&v["rows"]) {
                for m in arr(&row["modules"]) {
                    let title = m["title"].as_str().unwrap_or("");
                    let cards = cards_from(arr(&m["pagedList"]["items"]));
                    if !title.is_empty() && !cards.is_empty() {
                        p.rows.push((title.to_string(), cards));
                    }
                }
            }
        }
        if p.rows.is_empty() {
            // Home feed format changed or unavailable: fall back to the library.
            return self.library();
        }
        Ok(p)
    }

    pub fn my_playlists(&self) -> Res<Vec<Card>> {
        let uid = self.uid();
        if let Ok(v) = self.get(&format!("/users/{}/playlistsAndFavoritePlaylists", uid), &[("limit", "100")]) {
            let c = cards_from(arr(&v["items"]));
            if !c.is_empty() {
                return Ok(c);
            }
        }
        let mut out: Vec<Card> = Vec::new();
        let mut last_err = String::new();
        for p in ["playlists", "favorites/playlists"] {
            match self.get(&format!("/users/{}/{}", uid, p), &[("limit", "100")]) {
                Ok(v) => out.extend(cards_from(arr(&v["items"]))),
                Err(e) => last_err = e,
            }
        }
        if out.is_empty() && !last_err.is_empty() {
            return Err(last_err);
        }
        Ok(out)
    }

    /// Like (true) or unlike (false) a track.
    pub fn set_liked(&self, id: i64, like: bool) -> Res<()> {
        let (exp, has_rt) = {
            let s = self.session.lock().unwrap();
            (s.expires_at, !s.refresh_token.is_empty())
        };
        if has_rt && exp > 0 && exp < now() + 60 {
            let _ = self.refresh();
        }
        let (tok, country, uid) = {
            let s = self.session.lock().unwrap();
            (s.access_token.clone(), s.country.clone(), s.user_id)
        };
        let base = format!("{}/users/{}/favorites/tracks", API, uid);
        let idstr = id.to_string();
        let r = if like {
            self.http
                .post(&base)
                .bearer_auth(&tok)
                .query(&[("countryCode", country.as_str())])
                .form(&[("trackIds", idstr.as_str()), ("onArtifactNotFound", "SKIP")])
                .send()
        } else {
            self.http.delete(format!("{}/{}", base, idstr)).bearer_auth(&tok).query(&[("countryCode", country.as_str())]).send()
        };
        let r = r.map_err(e2s)?;
        let st = r.status();
        log(&format!("{} favorite {} -> {}", if like { "POST" } else { "DELETE" }, id, st.as_u16()));
        if st.is_success() {
            Ok(())
        } else {
            let t = r.text().unwrap_or_default();
            log(&format!("  error body: {}", t.chars().take(300).collect::<String>()));
            Err(format!("{}: {}", st.as_u16(), t.chars().take(120).collect::<String>()))
        }
    }

    /// One page (100) of liked songs starting at `offset`.
    /// Returns (tracks, total liked songs, raw item count in this page).
    pub fn fav_tracks(&self, offset: usize) -> Res<(Vec<Track>, usize, usize)> {
        let uid = self.uid();
        let off = offset.to_string();
        let v = self.get(
            &format!("/users/{}/favorites/tracks", uid),
            &[("limit", "100"), ("offset", off.as_str()), ("order", "DATE"), ("orderDirection", "DESC")],
        )?;
        let total = v["totalNumberOfItems"].as_u64().unwrap_or(0) as usize;
        let raw = arr(&v["items"]);
        Ok((tracks_from(raw), total, raw.len()))
    }

    pub fn library(&self) -> Res<Page> {
        let uid = self.uid();
        let mut p = Page { title: "Your Library".to_string(), library: true, ..Default::default() };
        let mut errs: Vec<String> = Vec::new();
        match self.fav_tracks(0) {
            Ok((t, total, _)) => {
                p.total = total.max(t.len());
                p.tracks = t;
            }
            Err(e) => errs.push(e),
        }
        match self.my_playlists() {
            Ok(pl) => {
                if !pl.is_empty() {
                    p.rows.push(("Playlists".to_string(), pl));
                }
            }
            Err(e) => errs.push(e),
        }
        if let Ok(items) = self.paged(&format!("/users/{}/favorites/albums", uid), &[], 1000) {
            let c = cards_from(&items);
            if !c.is_empty() {
                p.rows.push(("Albums".to_string(), c));
            }
        }
        if let Ok(items) = self.paged(&format!("/users/{}/favorites/artists", uid), &[], 1000) {
            let c = cards_from(&items);
            if !c.is_empty() {
                p.rows.push(("Artists".to_string(), c));
            }
        }
        if p.tracks.is_empty() && p.rows.is_empty() {
            if let Some(e) = errs.into_iter().next() {
                return Err(e);
            }
        }
        Ok(p)
    }

    /// Lyrics for a track: (lines with start time in seconds, true if the timings are real).
    pub fn lyrics(&self, id: i64) -> Res<(Vec<(f32, String)>, bool)> {
        let v = self.get(&format!("/tracks/{}/lyrics", id), &[("deviceType", "PHONE"), ("locale", "en_US")])?;
        if let Some(s) = v["subtitles"].as_str() {
            let l = parse_lrc(s);
            if !l.is_empty() {
                return Ok((l, true));
            }
        }
        if let Some(s) = v["lyrics"].as_str() {
            let l: Vec<(f32, String)> = s.lines().map(|x| (-1.0, x.trim().to_string())).collect();
            if l.iter().any(|(_, t)| !t.is_empty()) {
                return Ok((l, false));
            }
        }
        Err("no lyrics".to_string())
    }

    pub fn search(&self, q: &str) -> Res<Page> {
        let v = self.get("/search", &[("query", q), ("limit", "30"), ("types", "TRACKS,ALBUMS,ARTISTS,PLAYLISTS")])?;
        let mut p = Page { title: format!("Results for \"{}\"", q), ..Default::default() };
        p.tracks = tracks_from(arr(&v["tracks"]["items"]));
        for (label, key) in [("Albums", "albums"), ("Artists", "artists"), ("Playlists", "playlists")] {
            let c = cards_from(arr(&v[key]["items"]));
            if !c.is_empty() {
                p.rows.push((label.to_string(), c));
            }
        }
        Ok(p)
    }

    /// Recordings worth studying, best first: Tidal's 0-100 popularity, adjusted for practice. Karaoke, tribute and
    /// compilation tracks sink; artists already in your tunes float up. With a style (1 jazz, 2 gospel) the search is
    /// widened and players of that world (and the composer's own recording) float above chart pop. One per artist.
    pub fn popular(&self, name: &str, known: &[String], style: u8, composer: &str) -> Res<Vec<(Track, u32)>> {
        let mut queries = vec![name.to_string()];
        match style {
            1 => {
                queries.push(format!("{} jazz", name));
                if !composer.trim().is_empty() {
                    queries.push(format!("{} {}", name, composer.trim()));
                }
            }
            2 => {
                queries.push(format!("{} gospel", name));
                queries.push(format!("{} choir", name));
            }
            _ => {}
        }
        let mut all: Vec<(Track, u32)> = Vec::new();
        for (qi, q) in queries.iter().enumerate() {
            let v = match self.get("/search", &[("query", q.as_str()), ("limit", "50"), ("types", "TRACKS")]) {
                Ok(v) => v,
                Err(e) if qi == 0 => return Err(e),
                Err(_) => continue,
            };
            for it in arr(&v["tracks"]["items"]) {
                let pop = unwrap(it)["popularity"].as_u64().unwrap_or(0) as u32;
                if let Some(t) = parse_track(it) {
                    if !all.iter().any(|(x, _)| x.id == t.id) {
                        all.push((t, pop));
                    }
                }
            }
        }
        const JUNK: [&str; 14] = [
            "karaoke",
            "tribute",
            "made famous",
            "originally performed",
            "backing track",
            "in the style of",
            "lullaby",
            "remix",
            "sing-along",
            "play along",
            "cover version",
            "workout",
            "ringtone",
            "8-bit",
        ];
        const COMP: [&str; 9] =
            ["greatest hits", "best of", "essential", "collection", "anthology", "very best", "ultimate", "playlist", "hits"];
        const JAZZ: [&str; 62] = [
            "miles davis",
            "coltrane",
            "bill evans",
            "thelonious monk",
            "charlie parker",
            "dizzy gillespie",
            "duke ellington",
            "count basie",
            "louis armstrong",
            "ella fitzgerald",
            "sarah vaughan",
            "billie holiday",
            "art blakey",
            "sonny rollins",
            "cannonball adderley",
            "wes montgomery",
            "oscar peterson",
            "dave brubeck",
            "herbie hancock",
            "wayne shorter",
            "stan getz",
            "chet baker",
            "charles mingus",
            "ahmad jamal",
            "keith jarrett",
            "pat metheny",
            "joe pass",
            "wynton kelly",
            "hank mobley",
            "lee morgan",
            "freddie hubbard",
            "horace silver",
            "jimmy smith",
            "kenny burrell",
            "dexter gordon",
            "lester young",
            "coleman hawkins",
            "ben webster",
            "clifford brown",
            "max roach",
            "tommy flanagan",
            "red garland",
            "mccoy tyner",
            "brad mehldau",
            "kenny barron",
            "chick corea",
            "joe henderson",
            "woody shaw",
            "phil woods",
            "paul chambers",
            "ron carter",
            "jim hall",
            "bud powell",
            "nat king cole",
            "frank sinatra",
            "tony bennett",
            "joe williams",
            "carmen mcrae",
            "john scofield",
            "michael brecker",
            "gerry mulligan",
            "jackie mclean",
        ];
        const JAZZ_WORDS: [&str; 8] = ["quartet", "quintet", "trio", "sextet", "orchestra", "big band", "jazz", "sessions"];
        const GOSPEL: [&str; 34] = [
            "mahalia jackson",
            "kirk franklin",
            "yolanda adams",
            "winans",
            "marvin sapp",
            "tamela mann",
            "mcclurkin",
            "fred hammond",
            "john p. kee",
            "hezekiah walker",
            "tye tribbett",
            "israel houghton",
            "james cleveland",
            "andrae crouch",
            "shirley caesar",
            "clark sisters",
            "richard smallwood",
            "hawkins",
            "commissioned",
            "take 6",
            "mary mary",
            "smokie norful",
            "earnest pugh",
            "jonathan mcreynolds",
            "travis greene",
            "charles jenkins",
            "anthony brown",
            "kim burrell",
            "karen clark",
            "myron butler",
            "gospel",
            "choir",
            "chorale",
            "mass choir",
        ];
        let last_name = composer.split_whitespace().last().unwrap_or("").to_lowercase();
        let score = |t: &Track, pop: u32| -> i32 {
            let (ti, al, ar) = (t.title.to_lowercase(), t.album.to_lowercase(), t.artist.to_lowercase());
            let mut s = pop as i32;
            if JUNK.iter().any(|j| ti.contains(j) || al.contains(j)) {
                s -= 60;
            }
            if COMP.iter().any(|c| al.contains(c)) {
                s -= 20;
            }
            if ti.contains("live") || al.contains("live") {
                s -= 5;
            }
            if known.iter().any(|k| k.eq_ignore_ascii_case(&t.artist)) {
                s += 15;
            }
            match style {
                1 => {
                    let world = JAZZ.iter().any(|j| ar.contains(j));
                    let words = JAZZ_WORDS.iter().any(|j| ar.contains(j) || al.contains(j));
                    let own = last_name.len() > 3 && ar.contains(&last_name);
                    if world {
                        s += 35;
                    }
                    if words {
                        s += 20;
                    }
                    if own {
                        s += 25;
                    }
                    if !world && !words && !own {
                        s -= 25;
                    }
                    if ti.contains("feat.") || ar.contains("feat.") || ar.contains(" & ") && !words && !world {
                        s -= 10;
                    }
                }
                2 => {
                    let world = GOSPEL.iter().any(|g| ar.contains(g) || al.contains(g));
                    s += if world { 35 } else { -20 };
                }
                _ => {}
            }
            s
        };
        let want = name.to_lowercase();
        let mut all: Vec<(i32, Track, u32)> = all.into_iter().map(|(t, p)| (score(&t, p), t, p)).collect();
        all.sort_by(|a, b| b.0.cmp(&a.0));
        let (named, rest): (Vec<_>, Vec<_>) = all.into_iter().partition(|(_, t, _)| t.title.to_lowercase().contains(&want));
        let mut seen: Vec<String> = Vec::new();
        let mut out = Vec::new();
        for (s, t, p) in if named.len() >= 3 { named } else { named.into_iter().chain(rest).collect() } {
            if s > -25 && !seen.contains(&t.artist) {
                seen.push(t.artist.clone());
                out.push((t, p));
            }
        }
        out.truncate(12);
        Ok(out)
    }

    pub fn open(&self, c: &Card) -> Res<Page> {
        let mut p = Page { title: c.title.clone(), subtitle: c.subtitle.clone(), image: c.image.clone(), ..Default::default() };
        match c.kind {
            Kind::Album => {
                let items = self.paged(&format!("/albums/{}/items", c.id), &[], 500)?;
                p.tracks = tracks_from(&items);
            }
            Kind::Playlist => {
                let items = self.paged(&format!("/playlists/{}/items", c.id), &[], 1000)?;
                p.tracks = tracks_from(&items);
            }
            Kind::Artist => {
                let v = self.get(&format!("/artists/{}/toptracks", c.id), &[("limit", "30")])?;
                p.tracks = tracks_from(arr(&v["items"]));
                if let Ok(a) = self.get(&format!("/artists/{}/albums", c.id), &[("limit", "30")]) {
                    let cards = cards_from(arr(&a["items"]));
                    if !cards.is_empty() {
                        p.rows.push(("Albums".to_string(), cards));
                    }
                }
            }
            Kind::Mix => {
                let v = self.get("/pages/mix", &[("mixId", c.id.as_str()), ("deviceType", "BROWSER")])?;
                for row in arr(&v["rows"]) {
                    for m in arr(&row["modules"]) {
                        p.tracks.extend(tracks_from(arr(&m["pagedList"]["items"])));
                    }
                }
            }
        }
        Ok(p)
    }

    // -------------------------------------------------------------- playback

    /// Direct audio URL for a track. Lossless first (if allowed), then 320k AAC, then low.
    /// Every failure reason is collected and logged so a "won't play" is diagnosable.
    pub fn stream_url(&self, id: i64, lossless: bool) -> Res<String> {
        let quals: &[&str] = if lossless { &["LOSSLESS", "HIGH", "LOW"] } else { &["HIGH", "LOW"] };
        let mut errs: Vec<String> = Vec::new();

        for q in quals {
            let r = self.get(
                &format!("/tracks/{}/playbackinfopostpaywall", id),
                &[("audioquality", *q), ("playbackmode", "STREAM"), ("assetpresentation", "FULL")],
            );
            match r {
                Ok(v) => {
                    let mime = v["manifestMimeType"].as_str().unwrap_or("").to_string();
                    if mime.contains("bts") {
                        let raw = v["manifest"].as_str().unwrap_or("");
                        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(raw) {
                            if let Ok(m) = serde_json::from_slice::<Value>(&bytes) {
                                if let Some(u) = m["urls"][0].as_str() {
                                    log(&format!("track {} -> {} ({})", id, q, m["codecs"].as_str().unwrap_or("?")));
                                    return Ok(u.to_string());
                                }
                            }
                        }
                        errs.push(format!("{}: manifest unreadable", q));
                    } else {
                        errs.push(format!("{}: unsupported stream type {}", q, mime));
                    }
                }
                Err(e) => errs.push(format!("{}: {}", q, e)),
            }
        }

        // Older endpoint that returns the URL directly.
        for q in quals {
            let r = self.get(
                &format!("/tracks/{}/urlpostpaywall", id),
                &[("urlusagemode", "STREAM"), ("audioquality", *q), ("assetpresentation", "FULL")],
            );
            match r {
                Ok(v) => {
                    if let Some(u) = v["urls"][0].as_str() {
                        log(&format!("track {} -> {} via urlpostpaywall", id, q));
                        return Ok(u.to_string());
                    }
                    errs.push(format!("{}: no url in reply", q));
                }
                Err(e) => errs.push(format!("url {}: {}", q, e)),
            }
        }

        let msg = errs.join(" | ");
        log(&format!("track {} FAILED: {}", id, msg));
        Err(msg)
    }
}
