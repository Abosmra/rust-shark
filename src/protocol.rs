//! Attack Shark X6 wire protocol.
//!
//! Decoded from the official app (Mouse.exe) and verified against packets it
//! actually sent (see the tests at the bottom). Every settings packet is a HID
//! feature report on the vendor collection (usage page 0x0B):
//!   [cmd][len][0x01][payload...][checksum hi][checksum lo]
//! The checksum is a 16-bit sum of the payload bytes starting at index 3.

use serde::{Deserialize, Serialize};

pub const VID: u16 = 0x1D57;
/// 0xFA60 = 2.4G receiver, 0xFA61 = mouse on USB cable.
pub const PIDS: [u16; 2] = [0xFA60, 0xFA61];
pub const STATUS_PAGE: u16 = 0x0A;
pub const FEATURE_PAGE: u16 = 0x0B;

pub const MAX_MACRO_EVENTS: usize = 50;
pub const MAX_MACRO_DELAY_MS: u16 = 1270;

/// What a physical button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Binding {
    /// A built-in function, sent as-is (see `keys::FUNCTIONS`).
    Code([u8; 3]),
    /// Keyboard shortcut: modifier bits (1 ctrl, 2 shift, 4 alt, 8 win) + HID key.
    Shortcut { mods: u8, key: u8 },
    /// Plays macro `index` from `Profile::macros`.
    Macro(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroEvent {
    /// HID usage code (keyboard), or 0xF1/0xF2 for left/right mouse button.
    pub code: u8,
    pub down: bool,
    /// Wait after this event, 1..=1270 ms (10 ms resolution on the mouse).
    pub delay_ms: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Macro {
    pub name: String,
    pub events: Vec<MacroEvent>,
    /// 0 = play N times, 1 = repeat until any key, 2 = repeat while held.
    pub repeat: u8,
    pub loops: u8,
    /// Header byte the official app always writes (2).
    #[serde(default = "two")]
    pub kind: u8,
}
fn two() -> u8 {
    2
}
fn one() -> u8 {
    1
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    /// DPI per level in units of 50 (1..=520 -> 50..=26000). 0 = unused slot.
    pub dpi_units: [u16; 8],
    /// Bitmask of enabled DPI levels (0x3F = 6 levels).
    pub dpi_mask: u8,
    /// Active DPI level, 0-based.
    pub active_dpi: u8,
    pub dpi_colors: [[u8; 3]; 8],
    /// How the LED shows a DPI change: 1 blink (official app's fixed value), 2 static, 3 off.
    #[serde(default = "one")]
    pub dpi_indicator: u8,
    pub ripple: bool,
    /// Lift-off distance, firmware code: 0 = 1 mm (default), 1 = 2 mm, 2 = 0.7 mm.
    /// High nibble of byte 3 of the DPI packet; the official app never sets it.
    #[serde(default)]
    pub lod: u8,
    pub angle_snap: bool,
    pub motion_sync: bool,
    /// Key response raw value 2..=25 (x2 = milliseconds).
    pub key_response: u8,
    /// Polling code: 1 = 1000 Hz, 2 = 500, 4 = 250, 8 = 125.
    pub polling: u8,
    /// Lighting mode index 0..=11 (see `keys::LIGHT_FAMILIES`).
    pub light_mode: u8,
    pub light_brightness: u8, // 1..=8
    pub light_speed: u8,      // 4..=8
    pub light_color: [u8; 3],
    /// Sleep timer in half minutes (1..=60).
    pub sleep_half_min: u8,
    /// Deep sleep in minutes (1..=60).
    pub deep_sleep_min: u8,
    /// 18 button slots in firmware order (see `keys::UI_BUTTONS`).
    pub buttons: [Binding; 18],
    pub macros: Vec<Macro>,
}

impl Default for Profile {
    /// Factory defaults, identical to the official app's "restore defaults".
    fn default() -> Self {
        use Binding::Code as C;
        let o = C([0x01, 0, 0]);
        Profile {
            name: "Default".into(),
            dpi_units: [16, 24, 32, 64, 112, 520, 0, 0],
            dpi_mask: 0x3F,
            active_dpi: 1,
            dpi_colors: DEFAULT_DPI_COLORS,
            dpi_indicator: 1,
            lod: 0,
            ripple: false,
            angle_snap: false,
            motion_sync: true,
            key_response: 4,
            polling: 1,
            light_mode: 5,
            light_brightness: 8,
            light_speed: 6,
            light_color: [0x00, 0xFF, 0x00],
            sleep_half_min: 1,
            deep_sleep_min: 10,
            buttons: [
                C([0x02, 0, 0]),
                C([0x03, 0, 0]),
                C([0x04, 0, 0]),
                C([0x0D, 0, 0]),
                C([0x0E, 0, 0]),
                C([0x0F, 0, 0]),
                C([0x06, 0, 0]),
                C([0x05, 0, 0]),
                C([0x3C, 0, 0]),
                o,
                o,
                o,
                o,
                o,
                o,
                o,
                C([0x0A, 0, 0]),
                C([0x09, 0, 0]),
            ],
            macros: Vec::new(),
        }
    }
}

pub const DEFAULT_DPI_COLORS: [[u8; 3]; 8] = [
    [0xFF, 0x00, 0x00],
    [0x00, 0xFF, 0x00],
    [0x00, 0x00, 0xFF],
    [0xFF, 0xFF, 0x00],
    [0x00, 0xFF, 0xFF],
    [0xFF, 0x00, 0xFF],
    [0xFF, 0x40, 0x00],
    [0xFF, 0xFF, 0xFF],
];

impl Profile {
    pub fn dpi_levels(&self) -> usize {
        (self.dpi_mask.count_ones() as usize).clamp(1, 8)
    }
    pub fn set_dpi_levels(&mut self, n: usize) {
        let n = n.clamp(1, 8);
        self.dpi_mask = ((1u16 << n) - 1) as u8;
        for i in 0..n {
            if self.dpi_units[i] == 0 {
                self.dpi_units[i] = 16;
            }
        }
        if self.active_dpi as usize >= n {
            self.active_dpi = 0;
        }
    }
}

fn sum16(bytes: &[u8]) -> u16 {
    bytes.iter().fold(0u16, |a, &b| a.wrapping_add(b as u16))
}

fn put_sum(b: &mut [u8], from: usize, to: usize) {
    let s = sum16(&b[from..to]);
    b[to] = (s >> 8) as u8;
    b[to + 1] = s as u8;
}

/// Sent before a full profile write.
pub fn pkt_begin() -> Vec<u8> {
    vec![0x0C, 0x0A, 0x01, 0xFE, 0x01, 0xFE, 0, 0, 0, 0]
}

/// DPI levels, colors, active level, ripple / angle snap / motion sync.
pub fn pkt_dpi(p: &Profile) -> Vec<u8> {
    let mut b = vec![0u8; 56];
    b[..3].copy_from_slice(&[0x04, 0x38, 0x01]);
    b[3] = (p.lod.min(2) << 4) | p.angle_snap as u8;
    b[4] = p.ripple as u8;
    b[5] = p.dpi_mask;
    b[6] = p.angle_snap as u8;
    b[7] = p.motion_sync as u8;
    for (i, &u) in p.dpi_units.iter().enumerate() {
        if u >= 1 {
            let raw = u.min(520) - 1;
            b[8 + i] = raw as u8;
            b[16 + i] = (raw >> 8) as u8;
        }
    }
    b[24] = p.active_dpi + 1;
    for (i, c) in p.dpi_colors.iter().enumerate() {
        b[25 + i * 3..28 + i * 3].copy_from_slice(c);
    }
    b[49] = p.dpi_indicator.clamp(1, 3);
    put_sum(&mut b, 3, 50);
    b
}

/// Lighting, sleep timers and key response time.
pub fn pkt_light_power(p: &Profile) -> Vec<u8> {
    let mut b = vec![0u8; 15];
    b[..3].copy_from_slice(&[0x05, 0x0F, 0x01]);
    let deep = p.deep_sleep_min;
    let mode = p.light_mode.min(15);
    b[3] = mode << 4;
    b[4] = (deep & 0xF0) | (9 - p.light_speed.clamp(1, 9));
    let low = if mode == 1 || mode == 9 { p.light_brightness.min(15) } else { 8 };
    b[5] = ((deep & 0x0F) << 4) | low;
    b[6..9].copy_from_slice(&p.light_color);
    b[9] = p.sleep_half_min;
    b[10] = p.key_response;
    put_sum(&mut b, 3, 11);
    b
}

pub fn pkt_polling(p: &Profile) -> Vec<u8> {
    vec![0x06, 0x09, 0x01, p.polling, !p.polling, 0, 0, 0, 0]
}

pub fn binding_bytes(slot: usize, b: &Binding) -> [u8; 3] {
    match *b {
        Binding::Code(c) => c,
        Binding::Shortcut { mods, key } => [0x11, mods, key],
        Binding::Macro(_) => [0x12, 0x00, slot as u8 + 1],
    }
}

pub fn pkt_buttons(p: &Profile) -> Vec<u8> {
    let mut b = vec![0u8; 59];
    b[..3].copy_from_slice(&[0x08, 0x3B, 0x01]);
    for (i, bind) in p.buttons.iter().enumerate() {
        b[3 + i * 3..6 + i * 3].copy_from_slice(&binding_bytes(i, bind));
    }
    put_sum(&mut b, 3, 57);
    b
}

fn macro_buffer(slot: u8, m: &Macro) -> [u8; 131] {
    let mut b = [0u8; 131];
    b[0] = 0x09;
    b[1] = 0x83;
    b[2] = slot;
    b[3] = m.kind;
    b[4] = m.repeat;
    b[5] = m.repeat;
    b[6] = m.repeat;
    b[7] = m.loops.max(1);
    let entries = encode_events(&m.events);
    let entries = &entries[..entries.len().min(MAX_MACRO_EVENTS)];
    for (k, (t, code)) in entries.iter().enumerate() {
        b[29 + k * 2] = *t;
        b[30 + k * 2] = *code;
    }
    b[28] = entries.len() as u8;
    put_sum(&mut b, 3, 129);
    b
}

/// Max "long wait" filler entries per macro (same budget as the official app).
pub const MAX_LONG_WAITS: usize = 10;

/// Encode events into (timing byte, code) pairs.
///
/// Delays up to 1270 ms fit in the event itself (10 ms units). Longer delays
/// work like the official app: the event carries `delay % 200` and is followed
/// by a filler entry `(n, 0x03)` meaning "wait n x 200 ms". Unlike the official
/// app (which truncates n to 8 bits, so a 65.5 s wait became 14.2 s), waits
/// above 51 s are split across several fillers.
pub fn encode_events(events: &[MacroEvent]) -> Vec<(u8, u8)> {
    let enc = |ms: u16, down: bool| {
        // Bits 0-6 = delay in 10 ms units (min 1), bit 7 = release. The official app
        // ORs 0x01 into every byte, so even unit counts play 10 ms long (its "40 ms"
        // was sent as 0x05 and measured 50 ms on the X6).
        let units = ((ms as f32 / 10.0 + 0.5) as u8).clamp(1, 0x7F);
        units | if down { 0x00 } else { 0x80 }
    };
    let mut out = Vec::new();
    let mut fillers = 0;
    for e in events {
        if e.delay_ms <= MAX_MACRO_DELAY_MS {
            out.push((enc(e.delay_ms, e.down), e.code));
        } else {
            let mut q = e.delay_ms / 200;
            let r = e.delay_ms - q * 200;
            let need = (q as usize).div_ceil(255);
            if fillers + need > MAX_LONG_WAITS {
                out.push((enc(MAX_MACRO_DELAY_MS, e.down), e.code));
                continue;
            }
            out.push((enc(r, e.down), e.code));
            while q > 0 {
                let part = q.min(255);
                out.push((part as u8, 0x03));
                q -= part;
                fillers += 1;
            }
        }
    }
    out
}

/// A macro is sent as three 64-byte chunks.
pub fn macro_chunks(slot: u8, m: &Macro) -> Vec<Vec<u8>> {
    let b = macro_buffer(slot, m);
    let mut out = Vec::new();
    for i in 0..3u8 {
        let mut c = vec![0u8; 64];
        c[0] = 0x09;
        c[1] = if i == 2 { 0x0C } else { 0x40 };
        c[2] = slot;
        c[3] = i;
        let start = 3 + 60 * i as usize;
        let end = (start + 60).min(131);
        c[4..4 + (end - start)].copy_from_slice(&b[start..end]);
        out.push(c);
    }
    out
}

/// Macro uploads for every button bound to a macro: (slot number, chunks).
pub fn all_macro_uploads(p: &Profile) -> Vec<(u8, Vec<Vec<u8>>)> {
    p.buttons
        .iter()
        .enumerate()
        .filter_map(|(i, b)| match b {
            Binding::Macro(m) => p.macros.get(*m).map(|mac| {
                let slot = i as u8 + 1;
                (slot, macro_chunks(slot, mac))
            }),
            _ => None,
        })
        .collect()
}

/// Messages pushed by the receiver on the status collection (report 0x03).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatusMsg {
    /// state: 1 on battery, 2 full, 3 charging; level 0..=10
    Battery { state: u8, level: u8 },
    /// Write result for `report` (0x50 event): accepted or rejected (e.g. bad checksum).
    Ack { ok: bool, report: u8 },
    /// The mouse switched DPI level itself (0-based).
    DpiLevel(u8),
    Other(u8),
}

pub fn parse_status(r: &[u8]) -> Option<StatusMsg> {
    if r.len() < 5 || r[0] != 0x03 {
        return None;
    }
    // r[1] is the firmware's device ID: 0x10 on the X6 tested here, 0x85 on other X6
    // batches, so it is not checked
    Some(match r[2] {
        0x40 => StatusMsg::Battery { state: r[3], level: r[4] },
        0x50 => StatusMsg::Ack { ok: r[3] == 0, report: r[4] },
        0x10 if (1..=8).contains(&r[3]) => StatusMsg::DpiLevel(r[3] - 1),
        other => StatusMsg::Other(other),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace().map(|h| u8::from_str_radix(h, 16).unwrap()).collect()
    }
    fn padded(s: &str, len: usize) -> Vec<u8> {
        let mut v = hex(s);
        v.resize(len, 0);
        v
    }

    /// The user's profile as captured from the official app at 15:10.
    fn user() -> Profile {
        let mut p = Profile::default();
        p.dpi_units = [10, 101, 96, 83, 48, 520, 0, 0];
        p.active_dpi = 0;
        p.ripple = true;
        p.angle_snap = true;
        p.motion_sync = true;
        p.light_mode = 6;
        p.light_speed = 6;
        p.light_color = [0x00, 0xFF, 0xFF];
        p.deep_sleep_min = 15;
        p.sleep_half_min = 11;
        p.key_response = 2;
        p.polling = 2;
        p.buttons[4] = Binding::Macro(0);
        p.buttons[5] = Binding::Macro(1);
        p
    }

    #[test]
    fn begin_matches_capture() {
        assert_eq!(pkt_begin(), padded("0c 0a 01 fe 01 fe", 10));
    }

    #[test]
    fn defaults_match_restore_defaults_capture() {
        let p = Profile::default();
        assert_eq!(pkt_dpi(&p), padded("04 38 01 00 00 3f 00 01 0f 17 1f 3f 6f 07 00 00 00 00 00 00 00 02 00 00 02 ff 00 00 00 ff 00 00 00 ff ff ff 00 00 ff ff ff 00 ff ff 40 00 ff ff ff 01 0e 72", 56));
        assert_eq!(pkt_light_power(&p), padded("05 0f 01 50 03 a8 00 ff 00 01 04 01 ff", 15));
        assert_eq!(pkt_polling(&p), padded("06 09 01 01 fe", 9));
        assert_eq!(pkt_buttons(&p), padded("08 3b 01 02 00 00 03 00 00 04 00 00 0d 00 00 0e 00 00 0f 00 00 06 00 00 05 00 00 3c 00 00 01 00 00 01 00 00 01 00 00 01 00 00 01 00 00 01 00 00 01 00 00 0a 00 00 09 00 00 00 94", 59));
    }

    #[test]
    fn user_profile_matches_capture() {
        let p = user();
        assert_eq!(pkt_dpi(&p), padded("04 38 01 01 01 3f 01 01 09 64 5f 52 2f 07 00 00 00 00 00 00 00 02 00 00 01 ff 00 00 00 ff 00 00 00 ff ff ff 00 00 ff ff ff 00 ff ff 40 00 ff ff ff 01 0e ce", 56));
        assert_eq!(pkt_light_power(&p), padded("05 0f 01 60 03 f8 00 ff ff 0b 02 03 66", 15));
        assert_eq!(pkt_polling(&p), padded("06 09 01 02 fd", 9));
        assert_eq!(pkt_buttons(&p), padded("08 3b 01 02 00 00 03 00 00 04 00 00 0d 00 00 12 00 05 12 00 06 06 00 00 05 00 00 3c 00 00 01 00 00 01 00 00 01 00 00 01 00 00 01 00 00 01 00 00 01 00 00 0a 00 00 09 00 00 00 a6", 59));
    }

    #[test]
    fn lighting_mode_1_uses_brightness() {
        let mut p = user();
        p.light_mode = 1;
        p.deep_sleep_min = 30;
        p.light_brightness = 1;
        p.sleep_half_min = 10;
        p.light_color = [0x00, 0xFF, 0xFF];
        assert_eq!(pkt_light_power(&p), padded("05 0f 01 10 13 e1 00 ff ff 0a 02 03 0e", 15));
    }

    #[test]
    fn macro_matches_capture() {
        let m = Macro {
            name: "spamLC".into(),
            events: vec![
                MacroEvent { code: 0xF1, down: true, delay_ms: 40 },
                MacroEvent { code: 0xF1, down: false, delay_ms: 1 },
            ],
            repeat: 0,
            loops: 1,
            kind: 2,
        };
        let c = macro_chunks(5, &m);
        // the official app sent "05 f1" here (its +10 ms bug); 40 ms is 4 units
        assert_eq!(c[0], padded("09 40 05 00 02 00 00 00 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 04 f1 81 f1", 64));
        assert_eq!(c[1], padded("09 40 05 01", 64));
        assert_eq!(c[2], padded("09 0c 05 02 00 00 00 00 00 00 02 6c", 64));
    }

    #[test]
    fn typed_macro_matches_capture() {
        let ev = |code, down| MacroEvent { code, down, delay_ms: 1 };
        let m = Macro {
            name: "gs".into(),
            events: vec![
                ev(0x0A, true), ev(0x0A, false), ev(0x16, true), ev(0x16, false),
                ev(0x0A, true), ev(0x16, true), ev(0x0A, false), ev(0x16, false),
                ev(0x09, true), ev(0x09, false),
            ],
            repeat: 0,
            loops: 1,
            kind: 2,
        };
        let c = macro_chunks(6, &m);
        assert_eq!(c[0], padded("09 40 06 00 02 00 00 00 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 0a 01 0a 81 0a 01 16 81 16 01 0a 01 16 81 0a 81 16 01 09 81 09", 64));
        assert_eq!(c[2], padded("09 0c 06 02 00 00 00 00 00 00 03 29", 64));
    }

    #[test]
    fn long_delays_use_filler_entries() {
        let ev = |code, down, delay_ms| MacroEvent { code, down, delay_ms };
        // 1900 ms -> event carries 100 ms, then "wait 9 x 200 ms"
        assert_eq!(encode_events(&[ev(0x52, false, 1900)]), vec![(0x8A, 0x52), (9, 0x03)]);
        // 65535 ms (afktycoon) -> 135 ms (14 units) + 255 x 200 + 72 x 200
        assert_eq!(encode_events(&[ev(0xF1, true, 65535)]), vec![(0x0E, 0xF1), (255, 0x03), (72, 0x03)]);
        // short delays: plain 10 ms units, release adds 0x80, minimum 1 unit
        assert_eq!(encode_events(&[ev(0xF1, true, 40)]), vec![(0x04, 0xF1)]);
        assert_eq!(encode_events(&[ev(0xF1, false, 30)]), vec![(0x83, 0xF1)]);
        assert_eq!(encode_events(&[ev(0xF1, true, 35)]), vec![(0x04, 0xF1)]);
        assert_eq!(encode_events(&[ev(0xF1, true, 0)]), vec![(0x01, 0xF1)]);
    }

    #[test]
    fn status_parsing() {
        assert_eq!(parse_status(&hex("03 10 40 01 0a")), Some(StatusMsg::Battery { state: 1, level: 10 }));
        assert_eq!(parse_status(&hex("03 85 40 03 09")), Some(StatusMsg::Battery { state: 3, level: 9 }));
        // captured on the X6: accepted polling write, and a rejected one (bad checksum)
        assert_eq!(parse_status(&hex("03 10 50 00 06")), Some(StatusMsg::Ack { ok: true, report: 0x06 }));
        assert_eq!(parse_status(&hex("03 10 50 01 06")), Some(StatusMsg::Ack { ok: false, report: 0x06 }));
        assert_eq!(parse_status(&hex("03 10 10 03 00")), Some(StatusMsg::DpiLevel(2)));
    }
}
