//! System tray icon, hide/show of the main window, and "start with Windows".
//!
//! The window is hidden and shown with Win32 directly: once egui's own window is
//! invisible it stops running frames, so it couldn't bring itself back.

use std::sync::atomic::{AtomicIsize, Ordering};
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, ShowWindow, SW_HIDE, SW_RESTORE, SW_SHOW};

static HWND_: AtomicIsize = AtomicIsize::new(0);

/// Remember the main window (from eframe's creation context).
pub fn set_window(cc: &eframe::CreationContext<'_>) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if let Ok(h) = cc.window_handle() {
        if let RawWindowHandle::Win32(w) = h.as_raw() {
            HWND_.store(w.hwnd.get(), Ordering::Relaxed);
        }
    }
}

fn hwnd() -> HWND {
    HWND_.load(Ordering::Relaxed) as HWND
}

pub fn hide() {
    if !hwnd().is_null() {
        unsafe { ShowWindow(hwnd(), SW_HIDE) };
    }
}

pub fn show() {
    if !hwnd().is_null() {
        unsafe {
            ShowWindow(hwnd(), SW_SHOW);
            ShowWindow(hwnd(), SW_RESTORE);
            SetForegroundWindow(hwnd());
        }
    }
}

/// Tray icon: left click (or double click) opens the window; right click shows Open / Quit.
pub fn create(ctx: egui::Context) -> Option<TrayIcon> {
    let img = image::load_from_memory(crate::LOGO_PNG).ok()?.resize(32, 32, image::imageops::FilterType::Lanczos3).to_rgba8();
    let (w, h) = img.dimensions();
    let icon = tray_icon::Icon::from_rgba(img.into_raw(), w, h).ok()?;

    let open = MenuItem::new("Open Rust Shark", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let menu = Menu::new();
    menu.append_items(&[&open, &quit]).ok()?;
    let (open_id, quit_id) = (open.id().clone(), quit.id().clone());

    let c = ctx.clone();
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| match e {
        TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. }
        | TrayIconEvent::DoubleClick { .. } => {
            show();
            c.request_repaint();
        }
        _ => {}
    }));
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        if e.id == open_id {
            show();
            ctx.request_repaint();
        } else if e.id == quit_id {
            // settings are saved on every change, so nothing is lost
            std::process::exit(0);
        }
    }));

    TrayIconBuilder::new()
        .with_tooltip("Rust Shark")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .ok()
}

// ---- start with Windows: HKCU\...\Run\Rust Shark = "<exe>" --autostart

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE: &str = "Rust Shark";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

pub fn autostart_enabled() -> bool {
    let (k, v) = (wide(RUN_KEY), wide(VALUE));
    unsafe {
        RegGetValueW(HKEY_CURRENT_USER, k.as_ptr(), v.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) == 0
    }
}

/// Returns false if Windows refused the change.
pub fn set_autostart(on: bool) -> bool {
    let (k, v) = (wide(RUN_KEY), wide(VALUE));
    unsafe {
        if on {
            let Ok(exe) = std::env::current_exe() else { return false };
            let cmd = wide(&format!("\"{}\" --autostart", exe.display()));
            RegSetKeyValueW(HKEY_CURRENT_USER, k.as_ptr(), v.as_ptr(), REG_SZ, cmd.as_ptr() as _, (cmd.len() * 2) as u32) == 0
        } else {
            let r = RegDeleteKeyValueW(HKEY_CURRENT_USER, k.as_ptr(), v.as_ptr());
            r == 0 || r == 2 // 2 = it wasn't there
        }
    }
}
