#![cfg_attr(not(test), windows_subsystem = "windows")]

mod device;
mod icons_data;
mod keys;
mod protocol;
mod store;
mod tray;
mod ui;
mod update;

/// `rust-shark --selftest <out.txt>`: re-sends the active profile's polling
/// rate (no actual change) and reports whether the mouse acknowledged it.
fn selftest(out: &str) {
    let (cfg, how) = store::Config::load();
    let dev = device::Device::start(egui::Context::default());
    let wait = |cond: &dyn Fn(&device::Status) -> bool, secs: u64| {
        let t = std::time::Instant::now();
        while t.elapsed().as_secs() < secs && !cond(&dev.snapshot()) {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    };
    wait(&|s| s.connected && s.battery.is_some(), 6);
    let before = dev.snapshot();
    dev.send(vec![device::Step::Packet(protocol::pkt_polling(cfg.profile()))]);
    wait(&|s| s.pending_jobs == 0, 10);
    let after = dev.snapshot();
    let report = format!(
        "config: {how}\nprofile: {}\nconnected: {} wired: {}\nbattery: {:?}\npacket: {:02x?}\nwrite_failed: {}\nacks: {} -> {}\n",
        cfg.profile().name, after.connected, after.wired, after.battery,
        protocol::pkt_polling(cfg.profile()), after.last_write_failed, before.acks, after.acks
    );
    let _ = std::fs::write(out, report);
}

/// `rust-shark --resend`: writes the saved active profile (DPI, lighting, power,
/// polling, buttons, macros) to the mouse again. Repairs a mouse that was reset.
fn resend() {
    let (cfg, _) = store::Config::load();
    let p = cfg.profile();
    let dev = device::Device::start(egui::Context::default());
    let t = std::time::Instant::now();
    while t.elapsed().as_secs() < 6 && !dev.snapshot().connected {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let mut steps = vec![
        device::Step::Packet(protocol::pkt_begin()),
        device::Step::Pause(500),
        device::Step::Packet(protocol::pkt_dpi(p)),
        device::Step::Packet(protocol::pkt_light_power(p)),
        device::Step::Packet(protocol::pkt_polling(p)),
        device::Step::Packet(protocol::pkt_buttons(p)),
    ];
    for (_, chunks) in protocol::all_macro_uploads(p) {
        steps.push(device::Step::Macro(chunks));
    }
    dev.send(steps);
    let t = std::time::Instant::now();
    while t.elapsed().as_secs() < 20 && dev.snapshot().pending_jobs > 0 {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// The rusted shark-fin logo, embedded in the binary.
pub const LOGO_PNG: &[u8] = include_bytes!("../assets/logo_256.png");

fn window_icon() -> egui::IconData {
    let img = image::load_from_memory(LOGO_PNG).expect("embedded logo").to_rgba8();
    let (width, height) = img.dimensions();
    egui::IconData { rgba: img.into_raw(), width, height }
}

/// Is the saved window's title bar on a connected monitor? (A monitor may have been
/// unplugged since.) Positions are logical points, so try the common display scales.
fn on_screen(r: &[f32; 4]) -> bool {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTONULL};
    let (x, y) = (r[0] + r[2] / 2.0, r[1] + 12.0);
    [1.0f32, 1.25, 1.5, 1.75, 2.0].iter().any(|s| {
        let pt = POINT { x: (x * s) as i32, y: (y * s) as i32 };
        !unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTONULL) }.is_null()
    })
}

/// Window size; SHARK_SIZE=WxH overrides it (used for layout testing).
fn start_size() -> [f32; 2] {
    std::env::var("SHARK_SIZE")
        .ok()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Some([w.trim().parse().ok()?, h.trim().parse().ok()?])
        })
        .unwrap_or([1180.0, 760.0])
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--selftest") {
        selftest(args.get(2).map(String::as_str).unwrap_or("selftest.txt"));
        return Ok(());
    }
    if args.get(1).map(String::as_str) == Some("--resend") {
        resend();
        return Ok(());
    }
    // started by Windows (Run key) with "start minimized" on: open straight into the tray
    let hidden = args.iter().any(|a| a == "--autostart") && store::start_minimized_setting();
    // reopen where it was last left (unless a test size is forced, or that spot is off-screen now)
    let saved = store::saved_window().filter(|r| std::env::var_os("SHARK_SIZE").is_none() && on_screen(r));
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Rust Shark")
        .with_icon(window_icon())
        .with_inner_size(saved.map(|r| [r[2], r[3]]).unwrap_or_else(start_size))
        .with_min_inner_size([960.0, 620.0])
        .with_visible(!hidden);
    if let Some(r) = saved {
        viewport = viewport.with_position([r[0], r[1]]);
    }
    let opts = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("Rust Shark", opts, Box::new(|cc| Ok(Box::new(ui::App::new(cc)))))
}

#[cfg(test)]
mod import_tests {
    /// Importing the official app's saved data must reproduce the exact bytes
    /// the official app sent for the same settings.
    #[test]
    fn official_import_matches_capture() {
        let Some(ps) = crate::store::import_official() else { return };
        let p = &ps[0];
        let dpi = crate::protocol::pkt_dpi(p);
        assert_eq!(&dpi[3..8], &[1, 1, 0x3f, 1, 1]);
        assert_eq!(&dpi[8..14], &[0x09, 0x64, 0x5f, 0x52, 0x2f, 0x07]);
        assert_eq!(crate::protocol::pkt_polling(p)[3], 2);
        let b = crate::protocol::pkt_buttons(p);
        assert_eq!(&b[57..59], &[0x00, 0xa6]);
        let macros = crate::protocol::all_macro_uploads(p);
        assert_eq!(macros.len(), 2);
        assert_eq!(&macros[0].1[2][..12], &[0x09, 0x0c, 0x05, 0x02, 0, 0, 0, 0, 0, 0, 0x02, 0x6d]);
    }
}

#[cfg(test)]
mod recovery_tests {
    /// Tests that point SHARK_CONFIG somewhere take turns (it's process-wide).
    static ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// An unreadable settings file is kept as config.json.bad, never silently overwritten.
    #[test]
    fn unreadable_config_is_backed_up() {
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("rust-shark-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("config.json");
        std::fs::write(&cfg, "{ this is not json").unwrap();
        std::env::set_var("SHARK_CONFIG", &cfg);
        let (_, how) = crate::store::Config::load();
        std::env::remove_var("SHARK_CONFIG");
        assert_eq!(how, "recovered");
        assert_eq!(std::fs::read_to_string(dir.join("config.json.bad")).unwrap(), "{ this is not json");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Snapshots land in backups\, skip exact repeats, and read back as the same settings.
    #[test]
    fn snapshots_round_trip() {
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("rust-shark-backup-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("SHARK_CONFIG", dir.join("config.json"));
        let (mut cfg, _) = crate::store::Config::load();
        cfg.snapshot("one");
        cfg.snapshot("same"); // identical: skipped
        cfg.window = Some([1.0, 2.0, 3.0, 4.0]);
        cfg.snapshot("moved"); // only the window moved: skipped
        let list = crate::store::list_backups();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].why, "one");
        cfg.profile_mut().name = "Changed".into();
        let back = crate::store::Config::from_file(&list[0].path).unwrap();
        assert_ne!(back.profile().name, "Changed");
        assert!(crate::store::Config::from_file(&dir.join("config.json.missing")).is_none());
        std::env::remove_var("SHARK_CONFIG");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Over-long macros: every slot is counted, but the upload stays a valid 50-slot block.
    #[test]
    fn long_macro_is_counted_then_cut() {
        use crate::protocol::*;
        let ev = |down| MacroEvent { code: 0x04, down, delay_ms: 10 };
        let m = Macro { name: "x".into(), events: (0..60).map(|i| ev(i % 2 == 0)).collect(), repeat: 0, loops: 1, kind: 2 };
        assert_eq!(encode_events(&m.events).len(), 60);
        let chunks = macro_chunks(1, &m);
        assert_eq!(chunks[0][4 + 25], 50); // event count byte (buffer index 28)
    }
}
