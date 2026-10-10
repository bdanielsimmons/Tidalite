//! The song the tour plays: "Local Forecast - Elevator" by Kevin MacLeod (incompetech.com), licensed under
//! Creative Commons: By Attribution 4.0 (creativecommons.org/licenses/by/4.0/). It is kept in the repository
//! (assets/tour.mp3) and fetched once, the first time a tour runs, so the app itself stays small.

use std::path::PathBuf;

const URL: &str = "https://raw.githubusercontent.com/bdanielsimmons/Tidalite/main/tidalfast/assets/tour.mp3";

/// The credit the licence asks for, shown in the tour.
pub const CREDIT: &str =
    "\"Local Forecast - Elevator\" Kevin MacLeod (incompetech.com). Licensed under Creative Commons: By Attribution 4.0.";

/// Named "artist - title", so it shows up properly in the queue.
fn file() -> PathBuf {
    crate::api::config_dir().join("Kevin MacLeod - Local Forecast - Elevator.mp3")
}

/// The song, when it has been fetched before.
pub fn ready() -> Option<PathBuf> {
    let f = file();
    (std::fs::metadata(&f).map_or(0, |m| m.len()) > 100_000).then_some(f)
}

/// Fetch the song once (call off the main thread).
pub fn fetch() -> Result<PathBuf, String> {
    if let Some(f) = ready() {
        return Ok(f);
    }
    let b = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .and_then(|c| c.get(URL).send())
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.bytes())
        .map_err(|e| e.to_string())?;
    let f = file();
    std::fs::write(&f, &b).map_err(|e| e.to_string())?;
    Ok(f)
}

/// Where the tour loops it: a short phrase of about six seconds.
pub fn loop_span() -> (f32, f32) {
    (40.0, 46.0)
}
