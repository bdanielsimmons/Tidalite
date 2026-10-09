//! Global media keys (Windows): Play/Pause, Next, Previous and Stop work even while Tidalite
//! is in the background. Uses plain user32 hotkeys, so there is nothing extra to build.

use crate::Msg;
use eframe::egui;
use std::sync::mpsc::Sender;

#[cfg(windows)]
mod imp {
    use super::*;
    use std::ffi::c_void;

    #[repr(C)]
    struct RawMsg {
        hwnd: *mut c_void,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        pt_x: i32,
        pt_y: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn RegisterHotKey(hwnd: *mut c_void, id: i32, modifiers: u32, vk: u32) -> i32;
        fn GetMessageW(msg: *mut RawMsg, hwnd: *mut c_void, min: u32, max: u32) -> i32;
    }

    const WM_HOTKEY: u32 = 0x0312;
    const MOD_NOREPEAT: u32 = 0x4000;

    pub fn spawn(tx: Sender<Msg>, ctx: egui::Context) {
        std::thread::spawn(move || unsafe {
            // id -> virtual key: 1 play/pause, 2 next, 3 previous, 4 stop
            for (id, vk) in [(1i32, 0xB3u32), (2, 0xB0), (3, 0xB1), (4, 0xB2)] {
                let ok = RegisterHotKey(std::ptr::null_mut(), id, MOD_NOREPEAT, vk);
                if ok == 0 {
                    crate::api::log(&format!("media key {} could not be registered (another app owns it?)", id));
                }
            }
            let mut m: RawMsg = std::mem::zeroed();
            while GetMessageW(&mut m, std::ptr::null_mut(), 0, 0) > 0 {
                if m.message == WM_HOTKEY {
                    let _ = tx.send(Msg::Media(m.wparam as u8));
                    ctx.request_repaint();
                }
            }
        });
    }
}

#[cfg(windows)]
pub fn spawn(tx: Sender<Msg>, ctx: egui::Context) {
    imp::spawn(tx, ctx);
}

#[cfg(not(windows))]
pub fn spawn(_tx: Sender<Msg>, _ctx: egui::Context) {}
