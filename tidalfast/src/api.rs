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
// <config dir>/tidalfast/credentials.json: {"client_id":"...","client_secret":"..."}
const DEFAULT_CLIENT_ID: &str = "fX2JxdmntZWK0ixT";
const DEFAULT_CLIENT_SECRET: &str = "1Nn9AfDAjxrgJFJbKNWLeAyKGVGmINuXPPLHVXAvxAg=";
const SCOPE: &str = "r_usr w_usr w_sub";

fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tidalfast")
}

/// Append a line to %APPDATA%\tidalfast\log.txt (handy when something won't play).
pub fn log(msg: &str) {
    use std::io::Write;
    let dir = config_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("log.txt")) {
        let _ = writeln!(f, "[{}] {}", now(), msg);
    }
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
    format!(
        "https://resources.tidal.com/images/{}/{}x{}.jpg",
        uuid.replace('-', "/"),
        w,
        h
    )
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
    let artist = t["artist"]["name"]
        .as_str()
        .or_else(|| t["artists"][0]["name"].as_str())
        .unwrap_or("")
        .to_string();
    Some(Track {
        id,
        title,
        artist,
        album: t["album"]["title"].as_str().unwrap_or("").to_string(),
        cover: t["album"]["cover"].as_str().unwrap_or("").to_string(),
        duration: t["duration"].as_f64().unwrap_or(0.0) as f32,
    })
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
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (tidalfast)")
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
            let r = self
                .http
                .get(format!("{}{}", API, path))
                .bearer_auth(&tok)
                .query(&q)
                .send()
                .map_err(e2s)?;
            let st = r.status();
            if st.as_u16() == 401 && attempt == 0 {
                self.refresh()?;
                continue;
            }
            if !st.is_success() {
                let t = r.text().unwrap_or_default();
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
        let r = self
            .http
            .get(url)
            .timeout(Duration::from_secs(600))
            .send()
            .map_err(e2s)?
            .error_for_status()
            .map_err(e2s)?;
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
        let v = self.get(
            &format!("/users/{}/playlistsAndFavoritePlaylists", uid),
            &[("limit", "100"), ("order", "DATE"), ("orderDirection", "DESC")],
        )?;
        Ok(cards_from(arr(&v["items"])))
    }

    pub fn library(&self) -> Res<Page> {
        let uid = self.uid();
        let mut p = Page { title: "Your Library".to_string(), rows_first: true, ..Default::default() };
        if let Ok(pl) = self.my_playlists() {
            if !pl.is_empty() {
                p.rows.push(("Playlists".to_string(), pl));
            }
        }
        if let Ok(v) = self.get(&format!("/users/{}/favorites/albums", uid), &[("limit", "50")]) {
            let c = cards_from(arr(&v["items"]));
            if !c.is_empty() {
                p.rows.push(("Albums".to_string(), c));
            }
        }
        if let Ok(v) = self.get(&format!("/users/{}/favorites/artists", uid), &[("limit", "50")]) {
            let c = cards_from(arr(&v["items"]));
            if !c.is_empty() {
                p.rows.push(("Artists".to_string(), c));
            }
        }
        let v = self.get(
            &format!("/users/{}/favorites/tracks", uid),
            &[("limit", "100"), ("order", "DATE"), ("orderDirection", "DESC")],
        )?;
        p.tracks = tracks_from(arr(&v["items"]));
        Ok(p)
    }

    pub fn search(&self, q: &str) -> Res<Page> {
        let v = self.get(
            "/search",
            &[("query", q), ("limit", "30"), ("types", "TRACKS,ALBUMS,ARTISTS,PLAYLISTS")],
        )?;
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

    pub fn open(&self, c: &Card) -> Res<Page> {
        let mut p = Page {
            title: c.title.clone(),
            subtitle: c.subtitle.clone(),
            image: c.image.clone(),
            ..Default::default()
        };
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
