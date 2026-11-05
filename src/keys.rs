//! Button functions, lighting modes and key names.

/// (group, label, bytes) — bytes come from the official app's function table.
pub const FUNCTIONS: &[(&str, &str, [u8; 3])] = &[
    ("Mouse", "Left click", [0x02, 0, 0]),
    ("Mouse", "Right click", [0x03, 0, 0]),
    ("Mouse", "Middle click", [0x04, 0, 0]),
    ("Mouse", "Forward", [0x06, 0, 0]),
    ("Mouse", "Backward", [0x05, 0, 0]),
    ("Mouse", "Double click", [0x07, 0, 0]),
    ("Mouse", "Fire button", [0x08, 0, 0]),
    ("Mouse", "Scroll up", [0x09, 0, 0]),
    ("Mouse", "Scroll down", [0x0A, 0, 0]),
    ("Mouse", "Tilt left", [0x0B, 0, 0]),
    ("Mouse", "Tilt right", [0x0C, 0, 0]),
    ("Mouse", "Easy aim", [0x10, 0x00, 0x03]),
    ("Mouse", "LED loop", [0x29, 0x00, 0x03]),
    ("Mouse", "Button off", [0x01, 0, 0]),
    ("DPI", "DPI cycle", [0x0D, 0, 0]),
    ("DPI", "DPI up", [0x0E, 0, 0]),
    ("DPI", "DPI down", [0x0F, 0, 0]),
    ("Media", "Media player", [0x15, 0, 0]),
    ("Media", "Play / pause", [0x18, 0, 0]),
    ("Media", "Stop", [0x19, 0, 0]),
    ("Media", "Previous track", [0x16, 0, 0]),
    ("Media", "Next track", [0x17, 0, 0]),
    ("Media", "Volume up", [0x1B, 0, 0]),
    ("Media", "Volume down", [0x1C, 0, 0]),
    ("Media", "Mute", [0x1A, 0, 0]),
    ("Browser", "Home", [0x25, 0, 0]),
    ("Browser", "Favorites", [0x11, 0x03, 0x12]),
    ("Browser", "Forward", [0x20, 0, 0]),
    ("Browser", "Back", [0x21, 0, 0]),
    ("Browser", "Stop", [0x22, 0, 0]),
    ("Browser", "Refresh", [0x24, 0, 0]),
    ("Browser", "Search", [0x26, 0, 0]),
    ("Apps", "Email", [0x1E, 0, 0]),
    ("Apps", "Calculator", [0x1D, 0, 0]),
    ("Apps", "This PC", [0x23, 0, 0]),
    ("Shortcut", "Cut", [0x11, 0x01, 0x1B]),
    ("Shortcut", "Copy", [0x11, 0x01, 0x06]),
    ("Shortcut", "Paste", [0x11, 0x01, 0x19]),
    ("Shortcut", "Open", [0x11, 0x01, 0x12]),
    ("Shortcut", "Save", [0x11, 0x01, 0x16]),
    ("Shortcut", "Find", [0x11, 0x01, 0x09]),
    ("Shortcut", "Redo", [0x11, 0x01, 0x1C]),
    ("Shortcut", "Select all", [0x11, 0x01, 0x04]),
    ("Shortcut", "Print", [0x11, 0x01, 0x13]),
    ("Shortcut", "Close window", [0x11, 0x04, 0x3D]),
    ("Shortcut", "Switch window", [0x11, 0x04, 0x2B]),
    ("Shortcut", "Show desktop", [0x11, 0x08, 0x07]),
    ("Shortcut", "Run", [0x11, 0x08, 0x15]),
    ("Shortcut", "Lock PC", [0x11, 0x08, 0x0F]),
    ("Shortcut", "Screen capture", [0x11, 0x0A, 0x16]),
];

/// Official app's internal function id -> bytes (used when importing pro.data).
pub const FUNCTION_IDS: &[(u8, [u8; 3])] = &[
    (0, [0x01, 0, 0]), (1, [0x02, 0, 0]), (2, [0x03, 0, 0]), (3, [0x04, 0, 0]), (4, [0x06, 0, 0]),
    (5, [0x05, 0, 0]), (9, [0x07, 0, 0]), (10, [0x08, 0, 0]), (11, [0x09, 0, 0]), (12, [0x0A, 0, 0]),
    (13, [0x0B, 0, 0]), (14, [0x0C, 0, 0]), (6, [0x0D, 0, 0]), (7, [0x0E, 0, 0]), (8, [0x0F, 0, 0]),
    (19, [0x13, 0, 0]), (115, [0x14, 0, 0]), (21, [0x2B, 0, 0]), (22, [0x10, 0, 0x03]),
    (23, [0x29, 0, 0x03]), (54, [0x34, 0, 0]), (55, [0x35, 0, 0]), (56, [0x36, 0, 0]), (52, [0x3C, 0, 0]),
    (64, [0x15, 0, 0]), (67, [0x16, 0, 0]), (68, [0x17, 0, 0]), (65, [0x18, 0, 0]), (66, [0x19, 0, 0]),
    (71, [0x1A, 0, 0]), (69, [0x1B, 0, 0]), (70, [0x1C, 0, 0]), (88, [0x1D, 0, 0]), (87, [0x1E, 0, 0]),
    (81, [0x11, 0x03, 0x12]), (82, [0x20, 0, 0]), (83, [0x21, 0, 0]), (84, [0x22, 0, 0]), (89, [0x23, 0, 0]),
    (85, [0x24, 0, 0]), (80, [0x25, 0, 0]), (86, [0x26, 0, 0]), (32, [0x11, 0x01, 0x1B]),
    (33, [0x11, 0x01, 0x06]), (34, [0x11, 0x01, 0x19]), (35, [0x11, 0x01, 0x12]), (47, [0x11, 0x01, 0x11]),
    (36, [0x11, 0x01, 0x16]), (41, [0x11, 0x01, 0x13]), (37, [0x11, 0x01, 0x09]), (38, [0x11, 0x01, 0x1C]),
    (39, [0x11, 0x01, 0x1D]), (48, [0x11, 0x01, 0x2E]), (49, [0x11, 0x01, 0x2D]), (40, [0x11, 0x01, 0x04]),
    (43, [0x11, 0x04, 0x2B]), (44, [0x11, 0x08, 0x07]), (42, [0x11, 0x04, 0x3D]), (45, [0x11, 0x08, 0x15]),
    (46, [0x11, 0x08, 0x0F]), (50, [0x11, 0x0A, 0x16]),
];

/// The 7 physical buttons shown in the app -> firmware slot index.
/// Icon code for each physical button in `UI_BUTTONS` order
/// (left, right, wheel, side front, side rear, top front, top rear).
pub const UI_BUTTON_ICONS: [u8; 7] = [0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7];

pub const UI_BUTTONS: [(&str, usize); 7] = [
    ("Left", 0),
    ("Right", 1),
    ("Wheel", 2),
    ("Side front", 6),
    ("Side rear", 7),
    ("Top front", 4),
    ("Top rear", 5),
];

/// Lighting effects grouped for the UI: (family, [(mode index, variant label)]).
pub const LIGHT_FAMILIES: [(&str, &[(u8, &str)]); 7] = [
    ("Off", &[(0, "Off")]),
    ("Static", &[(1, "One color"), (9, "Mixed colors"), (5, "Per DPI level")]),
    ("Breathing", &[(2, "One color"), (4, "Color cycle"), (6, "Per DPI level")]),
    ("Neon", &[(3, "Neon")]),
    ("Rainbow wave", &[(7, "Rainbow wave")]),
    ("Lightning", &[(8, "Lightning")]),
    ("Marquee", &[(10, "Style 1"), (11, "Style 2")]),
];

/// "Static · Per DPI level" style name for any mode.
pub fn light_mode_name(mode: u8) -> String {
    for (fam, vars) in LIGHT_FAMILIES {
        if let Some(v) = vars.iter().find(|v| v.0 == mode) {
            return if vars.len() > 1 { format!("{fam} · {}", v.1) } else { fam.to_string() };
        }
    }
    format!("Mode {mode}")
}

pub const POLLING: [(u8, &str); 4] = [(8, "125 Hz"), (4, "250 Hz"), (2, "500 Hz"), (1, "1000 Hz")];

pub fn function_label(bytes: [u8; 3]) -> Option<&'static str> {
    FUNCTIONS.iter().find(|f| f.2 == bytes).map(|f| f.1)
}

/// HID keyboard usage -> name.
pub fn key_name(code: u8) -> String {
    match code {
        0x04..=0x1D => ((b'A' + code - 0x04) as char).to_string(),
        0x1E..=0x26 => ((b'1' + code - 0x1E) as char).to_string(),
        0x27 => "0".into(),
        0x28 => "Enter".into(),
        0x29 => "Esc".into(),
        0x2A => "Backspace".into(),
        0x2B => "Tab".into(),
        0x2C => "Space".into(),
        0x2D => "-".into(),
        0x2E => "=".into(),
        0x2F => "[".into(),
        0x30 => "]".into(),
        0x31 => "\\".into(),
        0x33 => ";".into(),
        0x34 => "'".into(),
        0x35 => "`".into(),
        0x36 => ",".into(),
        0x37 => ".".into(),
        0x38 => "/".into(),
        0x39 => "Caps Lock".into(),
        0x3A..=0x45 => format!("F{}", code - 0x3A + 1),
        0x46 => "Print Screen".into(),
        0x49 => "Insert".into(),
        0x4A => "Home".into(),
        0x4B => "Page Up".into(),
        0x4C => "Delete".into(),
        0x4D => "End".into(),
        0x4E => "Page Down".into(),
        0x4F => "Right".into(),
        0x50 => "Left".into(),
        0x51 => "Down".into(),
        0x52 => "Up".into(),
        0xE0 => "Ctrl".into(),
        0xE1 => "Shift".into(),
        0xE2 => "Alt".into(),
        0xE3 => "Win".into(),
        0xF1 => "Left click".into(),
        0xF2 => "Right click".into(),
        0xF3 => "Middle click".into(),
        0xF4 => "Forward button".into(),
        0xF5 => "Back button".into(),
        0x03 => "Long wait".into(),
        other => format!("Key 0x{other:02X}"),
    }
}

/// Mouse actions for macros (codes from the official recorder).
pub const MOUSE_ACTIONS: [(u8, &str); 5] =
    [(0xF1, "Left click"), (0xF2, "Right click"), (0xF3, "Middle click"), (0xF4, "Forward"), (0xF5, "Backward")];

/// Everything a macro event can be: keys, modifiers and mouse buttons.
pub fn macro_codes() -> Vec<u8> {
    let mut v = shortcut_keys();
    v.extend([0xE0, 0xE1, 0xE2, 0xE3]);
    v.extend(MOUSE_ACTIONS.iter().map(|m| m.0));
    v
}

/// Keys offered in the shortcut picker.
pub fn shortcut_keys() -> Vec<u8> {
    let mut v: Vec<u8> = (0x04..=0x2C).collect();
    v.extend(0x3A..=0x45);
    v.extend([0x46, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52]);
    v
}

/// egui key -> HID usage, for macro recording.
pub fn egui_to_hid(k: egui::Key) -> Option<u8> {
    use egui::Key::*;
    Some(match k {
        A => 0x04, B => 0x05, C => 0x06, D => 0x07, E => 0x08, F => 0x09, G => 0x0A, H => 0x0B,
        I => 0x0C, J => 0x0D, K => 0x0E, L => 0x0F, M => 0x10, N => 0x11, O => 0x12, P => 0x13,
        Q => 0x14, R => 0x15, S => 0x16, T => 0x17, U => 0x18, V => 0x19, W => 0x1A, X => 0x1B,
        Y => 0x1C, Z => 0x1D,
        Num1 => 0x1E, Num2 => 0x1F, Num3 => 0x20, Num4 => 0x21, Num5 => 0x22, Num6 => 0x23,
        Num7 => 0x24, Num8 => 0x25, Num9 => 0x26, Num0 => 0x27,
        Enter => 0x28, Escape => 0x29, Backspace => 0x2A, Tab => 0x2B, Space => 0x2C,
        Minus => 0x2D, Equals => 0x2E, OpenBracket => 0x2F, CloseBracket => 0x30, Backslash => 0x31,
        Semicolon => 0x33, Quote => 0x34, Backtick => 0x35, Comma => 0x36, Period => 0x37, Slash => 0x38,
        F1 => 0x3A, F2 => 0x3B, F3 => 0x3C, F4 => 0x3D, F5 => 0x3E, F6 => 0x3F, F7 => 0x40,
        F8 => 0x41, F9 => 0x42, F10 => 0x43, F11 => 0x44, F12 => 0x45,
        Insert => 0x49, Home => 0x4A, PageUp => 0x4B, Delete => 0x4C, End => 0x4D, PageDown => 0x4E,
        ArrowRight => 0x4F, ArrowLeft => 0x50, ArrowDown => 0x51, ArrowUp => 0x52,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_lighting_mode_appears_once() {
        let mut seen: Vec<u8> = super::LIGHT_FAMILIES.iter().flat_map(|f| f.1.iter().map(|v| v.0)).collect();
        seen.sort();
        assert_eq!(seen, (0..=11).collect::<Vec<u8>>());
    }
}
