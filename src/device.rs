//! Background thread that owns the USB handles.
//!
//! - Status channel (usage page 0x0A): read continuously for battery, acks and
//!   DPI-level changes made on the mouse itself.
//! - Feature channel (usage page 0x0B): settings are written with
//!   HidD_SetFeature at their exact length, like the official driver does.

use crate::protocol::{self, StatusMsg};
use std::ffi::CStr;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};
use windows_sys::Win32::Devices::HumanInterfaceDevice::HidD_SetFeature;
use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};

#[derive(Clone, Default)]
pub struct Status {
    pub connected: bool,
    pub wired: bool,
    /// (state, level 1..=10)
    pub battery: Option<(u8, u8)>,
    /// DPI level the mouse switched to on its own; taken by the UI.
    pub dpi_event: Option<u8>,
    pub pending_jobs: usize,
    pub last_write_failed: bool,
    /// A write failed; the mouse may be out of sync until the profile is re-sent.
    pub resync_needed: bool,
    /// Number of write acknowledgements received from the receiver.
    pub acks: u32,
}

pub enum Step {
    Packet(Vec<u8>),
    Macro(Vec<Vec<u8>>),
    Pause(u64),
}

pub struct Device {
    tx: mpsc::Sender<Vec<Step>>,
    pub status: Arc<Mutex<Status>>,
}

impl Device {
    pub fn start(ctx: egui::Context) -> Self {
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(Mutex::new(Status::default()));
        // Test hook: SHARK_OFFLINE never touches USB; SHARK_FAKE_BATTERY="state,level"
        // pretends a mouse is connected with that battery report.
        if std::env::var_os("SHARK_OFFLINE").is_some() {
            if let Some((a, b)) = std::env::var("SHARK_FAKE_BATTERY").ok().and_then(|v| {
                let (a, b) = v.split_once(',')?;
                Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
            }) {
                let mut s = status.lock().unwrap();
                s.connected = true;
                s.battery = Some((a, b));
            }
            return Device { tx, status };
        }
        let st = status.clone();
        std::thread::Builder::new()
            .name("shark-usb".into())
            .spawn(move || worker(rx, st, ctx))
            .expect("spawn usb thread");
        Device { tx, status }
    }

    pub fn send(&self, steps: Vec<Step>) {
        if steps.is_empty() {
            return;
        }
        self.status.lock().unwrap().pending_jobs += 1;
        let _ = self.tx.send(steps);
    }

    pub fn snapshot(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    /// True once after a failed write, when the receiver is connected again.
    pub fn take_resync(&self) -> bool {
        let mut s = self.status.lock().unwrap();
        let r = s.resync_needed && s.connected && s.pending_jobs == 0;
        if r {
            s.resync_needed = false;
        }
        r
    }

    pub fn take_dpi_event(&self) -> Option<u8> {
        self.status.lock().unwrap().dpi_event.take()
    }
}

struct Feature(HANDLE);
unsafe impl Send for Feature {}

impl Feature {
    fn open(path: &CStr) -> Option<Self> {
        let wide: Vec<u16> = path.to_string_lossy().encode_utf16().chain(Some(0)).collect();
        let h = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        if h == INVALID_HANDLE_VALUE || h.is_null() {
            None
        } else {
            Some(Feature(h))
        }
    }

    fn set(&self, data: &[u8]) -> bool {
        unsafe { HidD_SetFeature(self.0, data.as_ptr() as _, data.len() as u32) != 0 }
    }
}

impl Drop for Feature {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

struct Conn {
    status_dev: hidapi::HidDevice,
    feature: Feature,
    wired: bool,
    ack: bool,
}

fn connect(api: &mut hidapi::HidApi) -> Option<Conn> {
    let _ = api.refresh_devices();
    for pid in protocol::PIDS {
        let devs: Vec<_> = api
            .device_list()
            .filter(|d| d.vendor_id() == protocol::VID && d.product_id() == pid)
            .collect();
        let status = devs.iter().find(|d| d.usage_page() == protocol::STATUS_PAGE);
        let feat = devs.iter().find(|d| d.usage_page() == protocol::FEATURE_PAGE);
        if let (Some(s), Some(f)) = (status, feat) {
            let status_dev = match api.open_path(s.path()) {
                Ok(d) => d,
                Err(_) => continue,
            };
            if let Some(feature) = Feature::open(f.path()) {
                return Some(Conn { status_dev, feature, wired: pid == 0xFA61, ack: false });
            }
        }
    }
    None
}

impl Conn {
    /// Read status reports for up to `ms`. Returns false if the device is gone.
    fn pump(&mut self, ms: u64, st: &Mutex<Status>, ctx: &egui::Context) -> bool {
        let end = Instant::now() + Duration::from_millis(ms);
        let mut buf = [0u8; 64];
        loop {
            let left = end.saturating_duration_since(Instant::now()).as_millis() as i32;
            match self.status_dev.read_timeout(&mut buf, left.clamp(1, 50)) {
                Ok(n) if n > 0 => {
                    if let Some(msg) = protocol::parse_status(&buf[..n]) {
                        let mut s = st.lock().unwrap();
                        match msg {
                            StatusMsg::Battery { state, level } => {
                                if s.battery != Some((state, level)) {
                                    s.battery = Some((state, level));
                                    ctx.request_repaint();
                                }
                            }
                            StatusMsg::Ack { ok } => {
                                self.ack = ok;
                                if ok {
                                    s.acks += 1;
                                }
                            }
                            StatusMsg::DpiLevel(l) => {
                                s.dpi_event = Some(l);
                                ctx.request_repaint();
                            }
                            StatusMsg::Other(_) => {}
                        }
                    }
                }
                Ok(_) => {}
                Err(_) => return false,
            }
            if Instant::now() >= end {
                return true;
            }
        }
    }

    /// Write one packet, then give the receiver time to relay it to the mouse.
    /// (This receiver doesn't send acks for settings writes — verified by
    /// sniffing — so we don't retry; an ack, if one arrives, ends the wait early.)
    fn send_acked(&mut self, data: &[u8], st: &Mutex<Status>, ctx: &egui::Context) -> bool {
        self.ack = false;
        if !self.feature.set(data) {
            return false;
        }
        let t = Instant::now();
        while t.elapsed() < Duration::from_millis(250) && !self.ack {
            if !self.pump(25, st, ctx) {
                return false;
            }
        }
        true
    }

    fn send_macro(&mut self, chunks: &[Vec<u8>], st: &Mutex<Status>, ctx: &egui::Context) -> bool {
        for c in chunks {
            if !self.feature.set(c) {
                return false;
            }
            // The official app paces macro chunks a full second apart.
            if !self.pump(1000, st, ctx) {
                return false;
            }
        }
        true
    }
}

fn worker(rx: mpsc::Receiver<Vec<Step>>, st: Arc<Mutex<Status>>, ctx: egui::Context) {
    let mut api = match hidapi::HidApi::new() {
        Ok(a) => a,
        Err(_) => return,
    };
    let mut conn: Option<Conn> = None;
    let mut last_try = Instant::now() - Duration::from_secs(60);

    loop {
        if conn.is_none() && last_try.elapsed() > Duration::from_secs(2) {
            last_try = Instant::now();
            conn = connect(&mut api);
            let mut s = st.lock().unwrap();
            s.connected = conn.is_some();
            s.wired = conn.as_ref().map(|c| c.wired).unwrap_or(false);
            if conn.is_none() {
                s.battery = None;
            }
            ctx.request_repaint();
        }

        match rx.try_recv() {
            Ok(steps) => {
                let mut failed = false;
                if let Some(c) = conn.as_mut() {
                    for step in steps {
                        let ok = match &step {
                            Step::Packet(p) => c.send_acked(p, &st, &ctx),
                            Step::Macro(chunks) => c.send_macro(chunks, &st, &ctx),
                            Step::Pause(ms) => c.pump(*ms, &st, &ctx),
                        };
                        if !ok {
                            failed = true;
                            break;
                        }
                    }
                } else {
                    failed = true;
                }
                if failed {
                    conn = None;
                }
                let mut s = st.lock().unwrap();
                s.pending_jobs = s.pending_jobs.saturating_sub(1);
                s.last_write_failed = failed;
                s.resync_needed |= failed;
                if failed {
                    s.connected = false;
                }
                ctx.request_repaint();
                continue;
            }
            Err(mpsc::TryRecvError::Disconnected) => return,
            Err(mpsc::TryRecvError::Empty) => {}
        }

        match conn.as_mut() {
            Some(c) => {
                if !c.pump(100, &st, &ctx) {
                    conn = None;
                    let mut s = st.lock().unwrap();
                    s.connected = false;
                    s.battery = None;
                    ctx.request_repaint();
                }
            }
            None => std::thread::sleep(Duration::from_millis(200)),
        }
    }
}
