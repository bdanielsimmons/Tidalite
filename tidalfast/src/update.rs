//! Self-update. Every green build is published as a GitHub Release (tag `v9.N`, N = the Actions run number,
//! which the build stamps into the program as TIDALITE_BUILD). The app looks for a newer release, downloads
//! the file for this system next to itself, and swaps it in on the next start (or right away on click).
//! The previous file is kept as `.old` for one start so a bad swap can be undone by hand.

use crate::{api, retro_btn, Action, App, Msg, Tip};
use eframe::egui;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const REPO: &str = "bdanielsimmons/Tidalite";

#[derive(Clone)]
pub struct Info {
    pub build: u64,
    pub tag: String,
    pub url: String,
}

fn es<E: ToString>(e: E) -> String {
    e.to_string()
}

/// This program's build number (0 for a local build that was not made by the release workflow).
pub fn build() -> u64 {
    option_env!("TIDALITE_BUILD").and_then(|s| s.parse().ok()).unwrap_or(0)
}

/// Name of the release file that matches this program.
fn asset_name() -> &'static str {
    match (cfg!(windows), crate::PRACTICE) {
        (true, true) => "tidalite-practice.exe",
        (true, false) => "tidalite-simple.exe",
        (false, true) => "tidalite-practice-mac.tar.gz",
        (false, false) => "tidalite-simple-mac.tar.gz",
    }
}

fn http() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder().timeout(Duration::from_secs(600)).user_agent("Tidalite-updater").build().map_err(es)
}

fn staged_path() -> Result<PathBuf, String> {
    Ok(std::env::current_exe().map_err(es)?.with_extension("new"))
}

fn marker() -> PathBuf {
    api::config_dir().join("update.txt")
}

/// Ask GitHub for the newest release; Some when it is newer than this build.
pub fn check() -> Result<Option<Info>, String> {
    let me = build();
    if me == 0 {
        return Ok(None);
    }
    let url = format!("https://api.github.com/repos/{}/releases/latest", REPO);
    let v: serde_json::Value = http()?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(20))
        .send()
        .map_err(es)?
        .error_for_status()
        .map_err(es)?
        .json()
        .map_err(es)?;
    let tag = v["tag_name"].as_str().ok_or("release has no tag")?.to_string();
    let Some(n) = tag.rsplit('.').next().and_then(|s| s.parse::<u64>().ok()) else { return Ok(None) };
    if n <= me {
        return Ok(None);
    }
    let want = asset_name();
    let url = v["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|x| x["name"].as_str() == Some(want)))
        .and_then(|x| x["browser_download_url"].as_str())
        .ok_or("the release has no file for this system")?
        .to_string();
    Ok(Some(Info { build: n, tag, url }))
}

/// Download the new program and leave it next to this one as `<name>.new`.
pub fn stage(info: &Info) -> Result<(), String> {
    let bytes = http()?.get(&info.url).send().map_err(es)?.error_for_status().map_err(es)?.bytes().map_err(es)?;
    let dest = staged_path()?;
    let _ = std::fs::remove_file(&dest);
    if cfg!(windows) {
        std::fs::write(&dest, &bytes).map_err(es)?;
    } else {
        // the Mac / Linux file is a .tar.gz holding one program called `tidalite`
        let dir = api::config_dir().join("update");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(es)?;
        let tgz = dir.join("new.tar.gz");
        std::fs::write(&tgz, &bytes).map_err(es)?;
        let st = std::process::Command::new("tar").arg("-xzf").arg(&tgz).arg("-C").arg(&dir).status().map_err(es)?;
        if !st.success() {
            return Err("could not unpack the update".to_string());
        }
        std::fs::copy(dir.join("tidalite"), &dest).map_err(es)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755)).map_err(es)?;
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
    if std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0) < 1_000_000 {
        let _ = std::fs::remove_file(&dest);
        return Err("the downloaded update looks incomplete".to_string());
    }
    std::fs::write(marker(), info.build.to_string()).map_err(es)?;
    Ok(())
}

/// Swap the staged program in and start it. The caller exits afterwards.
pub fn apply() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(es)?;
    let new = staged_path()?;
    let old = exe.with_extension("old");
    if !new.exists() {
        return Err("no update has been downloaded".to_string());
    }
    let _ = std::fs::remove_file(&old);
    // a running program can be renamed (not overwritten) on Windows, and on Mac / Linux too
    std::fs::rename(&exe, &old).map_err(es)?;
    if let Err(e) = std::fs::rename(&new, &exe) {
        let _ = std::fs::rename(&old, &exe);
        return Err(es(e));
    }
    let _ = std::fs::remove_file(marker());
    std::process::Command::new(&exe).spawn().map_err(es)?;
    Ok(())
}

/// Called first thing in main(): finish an update that was downloaded during the last session.
/// True when the new program has been started and this one should just exit.
pub fn apply_staged_at_start() -> bool {
    if let Ok(old) = std::env::current_exe().map(|e| e.with_extension("old")) {
        let _ = std::fs::remove_file(old);
    }
    let Ok(txt) = std::fs::read_to_string(marker()) else { return false };
    if txt.trim().parse::<u64>().unwrap_or(0) <= build() {
        // already running that build (or newer): drop the leftovers
        let _ = std::fs::remove_file(marker());
        if let Ok(p) = staged_path() {
            let _ = std::fs::remove_file(p);
        }
        return false;
    }
    match apply() {
        Ok(()) => true,
        Err(e) => {
            api::log(&format!("update at start: {}", e));
            false
        }
    }
}

impl App {
    pub(crate) fn start_update_check(&mut self) {
        self.upd_checked = Some(Instant::now());
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(Msg::UpdateFound(check()));
            ctx.request_repaint();
        });
    }

    /// Checks every few hours; once a new build is downloaded, shows one button at the top to restart into it.
    pub(crate) fn update_banner(&mut self, ctx: &egui::Context, acts: &mut Vec<Action>) {
        let due = self.upd_checked.map_or(true, |t| t.elapsed() > Duration::from_secs(3 * 3600));
        if due && matches!(self.upd_state, 0 | 3) {
            self.start_update_check();
        }
        if self.upd_state != 2 {
            return;
        }
        let Some(info) = self.upd.clone() else { return };
        egui::Area::new(egui::Id::new("update_banner"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 4.0])
            .show(ctx, |ui| {
                if retro_btn(ui, &format!("UPDATE {} READY - CLICK TO RESTART", info.tag.to_uppercase()), true)
                    .tip("Saves everything and reopens Tidalite on the new version. If you don't click, it installs the next time you open the app.")
                    .clicked()
                {
                    acts.push(Action::ApplyUpdate);
                }
            });
    }
}
