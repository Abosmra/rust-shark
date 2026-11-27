//! System tray icon, hide/show of the main window, profile switching, and "start with Windows".
//!
//! The window is hidden and shown with Win32 directly: once egui's own window is
//! invisible it stops running frames, so it couldn't bring itself back.

use crate::device::Device;
use crate::i18n::{t, Lang};
use crate::protocol::Profile;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    IsWindowVisible, PostMessageW, SetForegroundWindow, ShowWindow, SW_HIDE, SW_RESTORE, SW_SHOW, WM_CLOSE,
};

static HWND_: AtomicIsize = AtomicIsize::new(0);
static FORCE_QUIT: AtomicBool = AtomicBool::new(false);

const TRAY_GUID: u128 = 0x7a9e_4c21_8b3d_4f12_a6e5_91c3_0b4d_8e72;

pub enum TrayCmd {
    SwitchProfile { idx: usize, already_sent: bool },
    Quit,
}

#[derive(Clone, Copy)]
enum MenuAction {
    Open,
    Profile(usize),
    Quit,
}

pub struct TrayState {
    pub icon: TrayIcon,
    pub rx: mpsc::Receiver<TrayCmd>,
    menu_map: Arc<Mutex<HashMap<MenuId, MenuAction>>>,
    profiles_snap: Arc<Mutex<Vec<Profile>>>,
}

impl TrayState {
    pub fn sync_menu(&self, profiles: &[Profile], active: usize, lang: Lang) {
        if let Ok(mut snap) = self.profiles_snap.lock() {
            *snap = profiles.to_vec();
        }
        let (menu, map) = build_menu(profiles, active, lang);
        if let Ok(mut m) = self.menu_map.lock() {
            *m = map;
        }
        self.icon.set_menu(Some(Box::new(menu)));
    }

    pub fn set_tooltip(&self, text: &str) {
        let _ = self.icon.set_tooltip(Some(text));
    }
}

pub fn is_force_quit() -> bool {
    FORCE_QUIT.load(Ordering::SeqCst)
}

pub fn is_window_visible() -> bool {
    let h = hwnd();
    !h.is_null() && unsafe { IsWindowVisible(h) != 0 }
}

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

fn build_menu(profiles: &[Profile], active: usize, lang: Lang) -> (Menu, HashMap<MenuId, MenuAction>) {
    let menu = Menu::new();
    let mut map = HashMap::new();

    let open = MenuItem::new(t(lang, "Open Rust Shark"), true, None);
    map.insert(open.id().clone(), MenuAction::Open);
    let _ = menu.append(&open);

    if !profiles.is_empty() {
        let _ = menu.append(&PredefinedMenuItem::separator());
        for (i, p) in profiles.iter().enumerate() {
            let item = CheckMenuItem::new(&p.name, true, i == active, None);
            map.insert(item.id().clone(), MenuAction::Profile(i));
            let _ = menu.append(&item);
        }
    }

    let _ = menu.append(&PredefinedMenuItem::separator());
    let quit = MenuItem::new(t(lang, "Quit"), true, None);
    map.insert(quit.id().clone(), MenuAction::Quit);
    let _ = menu.append(&quit);

    (menu, map)
}

/// Tray icon: left click (or double click) opens the window; right click shows Open / Profiles / Quit.
pub fn create(
    ctx: egui::Context,
    dev: Device,
    profiles: &[Profile],
    active: usize,
    lang: Lang,
) -> Option<TrayState> {
    let img = image::load_from_memory(crate::LOGO_PNG)
        .ok()?
        .resize(32, 32, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    let (w, h) = img.dimensions();
    let icon = tray_icon::Icon::from_rgba(img.into_raw(), w, h).ok()?;

    let (menu, initial_map) = build_menu(profiles, active, lang);
    let menu_map = Arc::new(Mutex::new(initial_map));
    let profiles_snap = Arc::new(Mutex::new(profiles.to_vec()));
    let (tx, rx) = mpsc::channel();

    let c = ctx.clone();
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| match e {
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        }
        | TrayIconEvent::DoubleClick { .. } => {
            show();
            c.request_repaint();
        }
        _ => {}
    }));

    let map_ref = menu_map.clone();
    let snap_ref = profiles_snap.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        let action = map_ref.lock().ok().and_then(|m| m.get(&e.id).copied());
        match action {
            Some(MenuAction::Open) => {
                show();
                ctx.request_repaint();
            }
            Some(MenuAction::Profile(idx)) => {
                if is_window_visible() {
                    let _ = tx.send(TrayCmd::SwitchProfile {
                        idx,
                        already_sent: false,
                    });
                    ctx.request_repaint();
                } else if let Some(p) = snap_ref.lock().ok().and_then(|s| s.get(idx).cloned()) {
                    dev.send(crate::device::full_profile_steps(&p));
                    let _ = tx.send(TrayCmd::SwitchProfile {
                        idx,
                        already_sent: true,
                    });
                    ctx.request_repaint();
                }
            }
            Some(MenuAction::Quit) => {
                FORCE_QUIT.store(true, Ordering::SeqCst);
                let _ = tx.send(TrayCmd::Quit);
                let h = hwnd();
                if !h.is_null() {
                    unsafe { PostMessageW(h, WM_CLOSE, 0, 0) };
                }
                ctx.request_repaint();
                let dev_exit = dev.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    dev_exit.shutdown();
                    std::process::exit(0);
                });
            }
            None => {}
        }
    }));

    let tray_icon = TrayIconBuilder::new()
        .with_guid(TRAY_GUID)
        .with_tooltip("Rust Shark")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .ok()?;

    Some(TrayState {
        icon: tray_icon,
        rx,
        menu_map,
        profiles_snap,
    })
}

pub fn format_tooltip(st: &crate::device::Status, profile_name: &str, lang: Lang) -> String {
    if !st.connected {
        return format!("Rust Shark · {}", t(lang, "Not connected"));
    }
    let link = if st.wired { "USB" } else { "2.4G" };
    match st.battery {
        Some((3, level)) => {
            let pct = level.min(10) as u32 * 10;
            format!("Rust Shark · {} {pct}% ({link}) · {profile_name}", t(lang, "Charging"))
        }
        Some((_, level)) => {
            let pct = level.min(10) as u32 * 10;
            format!("Rust Shark · {pct}% ({link}) · {profile_name}")
        }
        None => format!("Rust Shark · {} · {profile_name}", t(lang, if st.wired { "Connected · USB" } else { "Connected · 2.4G" })),
    }
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
        RegGetValueW(
            HKEY_CURRENT_USER,
            k.as_ptr(),
            v.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ) == 0
    }
}

/// Returns false if Windows refused the change.
pub fn set_autostart(on: bool) -> bool {
    let (k, v) = (wide(RUN_KEY), wide(VALUE));
    unsafe {
        if on {
            let Ok(exe) = std::env::current_exe() else { return false };
            let cmd = wide(&format!("\"{}\" --autostart", exe.display()));
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                k.as_ptr(),
                v.as_ptr(),
                REG_SZ,
                cmd.as_ptr() as _,
                (cmd.len() * 2) as u32,
            ) == 0
        } else {
            let r = RegDeleteKeyValueW(HKEY_CURRENT_USER, k.as_ptr(), v.as_ptr());
            r == 0 || r == 2 // 2 = it wasn't there
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::Status;

    #[test]
    fn tooltip_formats_states_in_en_and_pt() {
        let mut st = Status::default();
        assert_eq!(format_tooltip(&st, "Default", Lang::En), "Rust Shark · Not connected");
        assert_eq!(format_tooltip(&st, "Default", Lang::Pt), "Rust Shark · Desconectado");

        st.connected = true;
        st.wired = false;
        st.battery = Some((1, 8));
        assert_eq!(format_tooltip(&st, "Gaming", Lang::En), "Rust Shark · 80% (2.4G) · Gaming");

        st.wired = true;
        st.battery = Some((3, 9));
        assert_eq!(format_tooltip(&st, "Work", Lang::Pt), "Rust Shark · Carregando 90% (USB) · Work");
    }
}
