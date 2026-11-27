//! Single-instance lock using a Windows named mutex.
//! If another instance is already running, brings its window to the foreground.

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
};

pub struct SingleInstance(HANDLE);

impl Drop for SingleInstance {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CloseHandle(self.0) };
        }
    }
}

pub enum Lock {
    Acquired(#[allow(dead_code)] SingleInstance),
    AlreadyRunning,
    Unavailable,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn focus_existing_window() {
    let title = wide("Rust Shark");
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_SHOW);
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);
        }
    }
}

pub fn acquire() -> Lock {
    let name = wide("Local\\RustSharkSingleInstanceMutex");
    let h = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if h.is_null() {
        return Lock::Unavailable;
    }
    let err = unsafe { GetLastError() };
    if err == ERROR_ALREADY_EXISTS {
        unsafe { CloseHandle(h) };
        focus_existing_window();
        return Lock::AlreadyRunning;
    }
    Lock::Acquired(SingleInstance(h))
}
