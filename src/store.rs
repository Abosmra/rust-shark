//! Saved profiles (%APPDATA%\RustShark\config.json) and import from the
//! official app's pro.data.

use crate::keys;
use crate::protocol::{Binding, Macro, MacroEvent, Profile};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub profiles: Vec<Profile>,
    pub active: usize,
    /// Set once the official macro library (macro.data) has been merged in.
    #[serde(default)]
    pub library_imported: bool,
    /// Macro editor: give every newly recorded/added action this delay.
    #[serde(default = "yes")]
    pub use_default_delay: bool,
    #[serde(default = "ten")]
    pub default_delay: u16,
    /// Send every change to the mouse right away. When off, changes wait for "Save".
    #[serde(default = "yes")]
    pub auto_save: bool,
}

fn yes() -> bool {
    true
}
fn ten() -> u16 {
    10
}

fn appdata() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

fn config_path() -> PathBuf {
    // Test hook: SHARK_CONFIG points at a throwaway settings file (worst-case UI testing).
    if let Some(p) = std::env::var_os("SHARK_CONFIG") {
        return PathBuf::from(p);
    }
    let path = appdata().join("RustShark").join("config.json");
    // Carry settings over from the app's earlier name ("Shark Control").
    let old = appdata().join("SharkControl").join("config.json");
    if !path.exists() && old.exists() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::copy(&old, &path);
    }
    path
}

pub fn official_data_path() -> PathBuf {
    appdata().join("Attack SharkX6Mouse").join("ms_1").join("pro.data")
}

pub fn official_library_path() -> PathBuf {
    appdata().join("Attack SharkX6Mouse").join("ms_1").join("macro.data")
}

/// The official Macro Manager library: 240-byte records of
/// [name: UTF-16, 26 chars][count: u32 @0x34][events @0x38: code, 1=down/2=up, delay u16].
pub fn import_official_library() -> Vec<Macro> {
    let Ok(d) = std::fs::read(official_library_path()) else { return Vec::new() };
    d.as_chunks::<0xF0>().0.iter()
        .filter_map(|r| {
            let name = utf16_at(r, 0, 26);
            if name.is_empty() {
                return None;
            }
            let count = (u32_at(r, 0x34)? as usize).min((0xF0 - 0x38) / 4);
            let events = (0..count)
                .map(|k| {
                    let e = 0x38 + k * 4;
                    MacroEvent { code: r[e], down: r[e + 1] == 1, delay_ms: u16_at(r, e + 2).unwrap_or(10).max(1) }
                })
                .collect();
            Some(Macro { name, events, repeat: 0, loops: 1, kind: 2 })
        })
        .collect()
}

/// Add macros whose names aren't in the profile yet.
pub fn merge_macros(p: &mut Profile, lib: &[Macro]) -> usize {
    let mut added = 0;
    for m in lib {
        if !p.macros.iter().any(|x| x.name == m.name) {
            p.macros.push(m.clone());
            added += 1;
        }
    }
    added
}

impl Config {
    /// Saved config, else your settings imported from the official app, else defaults.
    pub fn load() -> (Config, &'static str) {
        let (mut cfg, how) = Self::load_inner();
        if !cfg.library_imported {
            let lib = import_official_library();
            for p in cfg.profiles.iter_mut() {
                merge_macros(p, &lib);
            }
            cfg.library_imported = true;
            cfg.save();
            if !lib.is_empty() && how == "loaded" {
                return (cfg, "library");
            }
        }
        (cfg, how)
    }

    fn load_inner() -> (Config, &'static str) {
        let path = config_path();
        let mut recovered = false;
        if let Ok(s) = std::fs::read_to_string(&path) {
            match serde_json::from_str::<Config>(&s) {
                Ok(c) if !c.profiles.is_empty() => {
                    let active = c.active.min(c.profiles.len() - 1);
                    return (Config { active, ..c }, "loaded");
                }
                // unreadable: keep it as config.json.bad so the next save can't destroy it
                _ => recovered = std::fs::rename(&path, path.with_extension("json.bad")).is_ok(),
            }
        }
        if recovered {
            let profiles = import_official().unwrap_or_else(|| vec![Profile::default()]);
            return (Config { profiles, active: 0, library_imported: false, use_default_delay: true, default_delay: 10, auto_save: true }, "recovered");
        }
        if let Some(profiles) = import_official() {
            return (Config { profiles, active: 0, library_imported: false, use_default_delay: true, default_delay: 10, auto_save: true }, "imported");
        }
        (Config { profiles: vec![Profile::default()], active: 0, library_imported: false, use_default_delay: true, default_delay: 10, auto_save: true }, "default")
    }

    pub fn save(&self) {
        let p = config_path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
    }

    pub fn profile(&self) -> &Profile {
        &self.profiles[self.active]
    }
    pub fn profile_mut(&mut self) -> &mut Profile {
        &mut self.profiles[self.active]
    }
}

fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    d.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
}
fn utf16_at(d: &[u8], o: usize, max_chars: usize) -> String {
    let mut v = Vec::new();
    for i in 0..max_chars {
        match u16_at(d, o + i * 2) {
            Some(0) | None => break,
            Some(c) => v.push(c),
        }
    }
    String::from_utf16_lossy(&v)
}

/// Profile records in pro.data use the same layout as the official app's
/// in-memory settings struct (field offsets decoded from Mouse.exe).
fn looks_like_profile(d: &[u8], b: usize) -> bool {
    let ok = |o: usize, lo: u32, hi: u32| u32_at(d, b + o).is_some_and(|v| v >= lo && v <= hi);
    let name = utf16_at(d, b, 24);
    !name.is_empty()
        && name.chars().all(|c| !c.is_control())
        && ok(0x8C4, 1, 520)
        && ok(0x934, 0, 7)
        && ok(0x938, 0, 3)
        && ok(0x94C, 0, 11)
        && ok(0x930, 1, 60)
        && ok(0x92C, 1, 60)
        && ok(0x958, 2, 25)
        && ok(0x924, 1, 8)
        && (0..8).all(|i| ok(0x904 + i * 4, 0, 1))
}

fn read_profile(d: &[u8], b: usize) -> Profile {
    let g = |o: usize| u32_at(d, b + o).unwrap_or(0);
    let mut p = Profile::default();
    p.name = utf16_at(d, b, 24);
    for i in 0..8 {
        p.dpi_units[i] = g(0x8C4 + i * 4).min(520) as u16;
        let c = b + 0x960 + i * 4;
        p.dpi_colors[i] = [d[c], d[c + 1], d[c + 2]];
    }
    p.dpi_mask = (0..8).fold(0u8, |m, i| if g(0x904 + i * 4) == 1 { m | (1 << i) } else { m });
    if p.dpi_mask == 0 {
        p.dpi_mask = 0x3F;
    }
    p.active_dpi = g(0x934) as u8;
    p.polling = [8u8, 4, 2, 1][g(0x938).min(3) as usize];
    p.ripple = g(0x940) == 1;
    p.angle_snap = g(0x944) == 1;
    p.motion_sync = g(0x948) == 1;
    p.light_mode = g(0x94C) as u8;
    p.light_brightness = g(0x924) as u8;
    p.light_speed = (g(0x928) as u8).clamp(4, 8);
    p.deep_sleep_min = g(0x92C) as u8;
    p.sleep_half_min = g(0x930) as u8;
    p.key_response = g(0x958) as u8;
    let c = b + 0x95C;
    p.light_color = [d[c], d[c + 1], d[c + 2]];

    for slot in 0..18 {
        let m = b + 0x9A0 + slot * 0x4F8;
        let Some(fid) = d.get(m + 1).copied() else { break };
        p.buttons[slot] = match fid {
            16 => Binding::Shortcut { mods: d[m + 7], key: d[m + 3] },
            17 => {
                let count = u16_at(d, m + 0x44).unwrap_or(0) as usize;
                let events = (0..count.min(crate::protocol::MAX_MACRO_EVENTS))
                    .filter_map(|k| {
                        let e = m + 0x440 + k * 4;
                        Some(MacroEvent {
                            code: *d.get(e)?,
                            down: *d.get(e + 1)? == 1,
                            delay_ms: u16_at(d, e + 2)?,
                        })
                    })
                    .collect();
                p.macros.push(Macro {
                    name: {
                        let n = utf16_at(d, m + 0x10, 24);
                        if n.is_empty() { format!("Macro {}", p.macros.len() + 1) } else { n }
                    },
                    events,
                    repeat: d[m + 0x43C].min(2),
                    loops: d[m + 0x40].max(1),
                    kind: d[m + 0x0C],
                });
                Binding::Macro(p.macros.len() - 1)
            }
            id => match keys::FUNCTION_IDS.iter().find(|f| f.0 == id) {
                Some(f) => Binding::Code(f.1),
                None => p.buttons[slot],
            },
        };
    }
    p
}

pub fn import_official() -> Option<Vec<Profile>> {
    let d = std::fs::read(official_data_path()).ok()?;
    let mut out = Vec::new();
    let mut b = 0;
    while b + 0x9A0 + 18 * 0x4F8 <= d.len() {
        if looks_like_profile(&d, b) {
            out.push(read_profile(&d, b));
            b += 0x9A0 + 18 * 0x4F8;
        } else {
            b += 4;
        }
    }
    if out.is_empty() { None } else { Some(out) }
}
