//! macOS only: the real menu bar at the top of the screen (Backline / File / Playback / View / Window / Help),
//! so the app behaves like other Mac apps. Clicks arrive as `Msg::Menu(id)` and are handled in extras.rs.
//! Shortcuts use the Command key and never a bare letter, so typing in the search box is never stolen.

use crate::Msg;
use eframe::egui;
use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use std::sync::mpsc::Sender;

fn it(id: &str, text: &str, accel: Option<(Modifiers, Code)>) -> MenuItem {
    MenuItem::with_id(id, text, true, accel.map(|(m, c)| Accelerator::new(Some(m), c)))
}

/// Builds the menu and installs it. The returned value must be kept alive for as long as the app runs.
pub fn install(tx: &Sender<Msg>, ctx: &egui::Context) -> Option<Box<dyn std::any::Any>> {
    let cmd = Modifiers::SUPER;
    let shift_cmd = Modifiers::SUPER | Modifiers::SHIFT;
    let ctrl_cmd = Modifiers::SUPER | Modifiers::CONTROL;
    let menu = Menu::new();
    let app = Submenu::with_items(
        "Backline",
        true,
        &[
            &it("about", "About Backline", None),
            &PredefinedMenuItem::separator(),
            &it("prefs", "Preferences...", Some((cmd, Code::Comma))),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::separator(),
            &it("quit", "Quit Backline", Some((cmd, Code::KeyQ))),
        ],
    )
    .ok()?;
    let file = Submenu::with_items(
        "File",
        true,
        &[
            &it("add_files", "Add Files...", Some((cmd, Code::KeyO))),
            &it("add_folder", "Add Folder...", Some((shift_cmd, Code::KeyO))),
        ],
    )
    .ok()?;
    let playback = Submenu::with_items(
        "Playback",
        true,
        &[
            &it("toggle", "Play / Pause", None),
            &it("next", "Next Song", Some((cmd, Code::ArrowRight))),
            &it("prev", "Previous Song", Some((cmd, Code::ArrowLeft))),
            &PredefinedMenuItem::separator(),
            &it("like", "Like Current Song", None),
        ],
    )
    .ok()?;
    let view = Submenu::with_items(
        "View",
        true,
        &[
            &it("art", "Album View", Some((cmd, Code::Digit1))),
            &it("mini", "Mini Player", Some((cmd, Code::Digit2))),
            &it("full", "Enter / Exit Full Screen", Some((ctrl_cmd, Code::KeyF))),
            &PredefinedMenuItem::separator(),
            &it("skin", "Next Skin", Some((shift_cmd, Code::KeyS))),
            &it("eq", "Equalizer", None),
            &PredefinedMenuItem::separator(),
            &it("palette", "Command Palette...", Some((cmd, Code::KeyK))),
        ],
    )
    .ok()?;
    let window = Submenu::with_items("Window", true, &[&PredefinedMenuItem::minimize(None)]).ok()?;
    let help =
        Submenu::with_items("Help", true, &[&it("help", "Backline Help", None), &it("tour", "Take the Tour", None)]).ok()?;
    menu.append_items(&[&app, &file, &playback, &view, &window, &help]).ok()?;
    menu.init_for_nsapp();
    let (tx, ctx) = (tx.clone(), ctx.clone());
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        let _ = tx.send(Msg::Menu(e.id.0.clone()));
        ctx.request_repaint();
    }));
    Some(Box::new(menu))
}
