"""Worst-case and edge-case settings files for UI stress testing (break-ui).

Writes tests/fixtures/worst_case.json and tests/fixtures/empty.json.
Run the app against one without touching your real settings or mouse:

    SHARK_CONFIG=<copy of fixture> SHARK_OFFLINE=1 SHARK_FAKE_BATTERY=1,1 rust-shark.exe
"""
import json
import pathlib

HERE = pathlib.Path(__file__).resolve().parent


def code(b0, b1=0, b2=0):
    return {"Code": [b0, b1, b2]}


DEFAULT_BUTTONS = [code(2), code(3), code(4), code(0x0D), code(0x0E), code(0x0F), code(6), code(5), code(0x3C)] + \
    [code(1)] * 7 + [code(0x0A), code(9)]


def profile(name, **kw):
    p = {
        "name": name,
        "dpi_units": [16, 24, 32, 64, 112, 520, 0, 0],
        "dpi_mask": 0x3F,
        "active_dpi": 1,
        "dpi_colors": [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0], [0, 255, 255], [255, 0, 255], [255, 64, 0], [255, 255, 255]],
        "ripple": False, "angle_snap": False, "motion_sync": True, "lod_mm": 1,
        "key_response": 4, "polling": 1,
        "light_mode": 5, "light_brightness": 8, "light_speed": 6, "light_color": [0, 255, 0],
        "sleep_half_min": 1, "deep_sleep_min": 10,
        "buttons": list(DEFAULT_BUTTONS),
        "macros": [],
    }
    p.update(kw)
    return p


def ev(c, down, ms):
    return {"code": c, "down": down, "delay_ms": ms}


# A full 50-action macro: typing + modifiers + mouse, with a couple of very long waits.
full = []
seq = [0x04, 0x05, 0xE0, 0x06, 0x2C, 0x52, 0x3A, 0xF1, 0xF4, 0x46, 0x29, 0x16, 0x17, 0xE2, 0x4F, 0xF5, 0x1E, 0x2B, 0xE3, 0x45, 0x28, 0x2A, 0x50, 0x4C, 0xF3]
for i, c in enumerate(seq):
    full.append(ev(c, True, 65535 if i in (3, 17) else 10))
    full.append(ev(c, False, 1270 if i == 9 else 1))

macros = [
    {"name": "Wiśniewska burstfire v12", "events": [ev(0xF1, True, 35), ev(0xF1, False, 10)] * 6, "repeat": 2, "loops": 1, "kind": 2},
    {"name": "", "events": [ev(0x16, True, 10), ev(0x16, False, 10)], "repeat": 0, "loops": 255, "kind": 2},
    {"name": "ماكرو الهجوم السريع", "events": [ev(0x14, True, 10), ev(0x14, False, 10)], "repeat": 1, "loops": 1, "kind": 2},
    {"name": "Jo", "events": full, "repeat": 0, "loops": 1, "kind": 2},
    {"name": "Copy + paste spam (WIP)", "events": [], "repeat": 0, "loops": 1, "kind": 2},
    {"name": "🦈 afk timer", "events": [ev(0xF2, True, 65535), ev(0xF2, False, 65535)], "repeat": 1, "loops": 1, "kind": 2},
] + [{"name": f"Macro {i}", "events": [ev(0x04, True, 10), ev(0x04, False, 10)], "repeat": 0, "loops": 1, "kind": 2} for i in range(7, 41)]

worst_buttons = list(DEFAULT_BUTTONS)
worst_buttons[1] = {"Macro": 0}                                   # Right -> long macro name
worst_buttons[2] = {"Shortcut": {"mods": 15, "key": 0x46}}        # Wheel -> Ctrl+Shift+Alt+Win+Print Screen
worst_buttons[6] = code(0x2B)                                     # Forward -> function the app has no name for
worst_buttons[7] = code(1)                                        # Back -> Button off
worst_buttons[4] = {"Macro": 1}                                   # Top front -> macro with empty name
worst_buttons[5] = {"Macro": 2}                                   # Top rear -> Arabic macro name

worst = profile(
    "Aleksandra's Valorant v3",
    dpi_units=[520, 520, 519, 1, 247, 520, 0, 0],
    active_dpi=3,
    dpi_colors=[[255, 0, 0], [0, 0, 0], [255, 255, 255], [20, 20, 30], [0, 255, 255], [255, 0, 255], [255, 64, 0], [255, 255, 255]],
    ripple=True, angle_snap=True, motion_sync=False,
    key_response=25, polling=8,
    light_mode=4, light_brightness=1, light_speed=8, light_color=[0, 0, 0],
    sleep_half_min=60, deep_sleep_min=60,
    buttons=worst_buttons,
    macros=macros,
)
others = [profile(n) for n in ["Jo", "ملف الألعاب", "🦈 Shark aim", "Valorant (low sens, 400)", "CS2", "Photoshop – brushes", "Work",
                                "Aim trainer", "Minecraft", "Fortnite build mode", "Default"]]

worst_cfg = {"profiles": [worst] + others, "active": 0, "library_imported": True,
             "use_default_delay": True, "default_delay": 65535, "auto_save": False}
empty_cfg = {"profiles": [profile("D", dpi_units=[16, 0, 0, 0, 0, 0, 0, 0], dpi_mask=1, active_dpi=0)], "active": 0,
             "library_imported": True, "use_default_delay": False, "default_delay": 1, "auto_save": True}

(HERE / "worst_case.json").write_text(json.dumps(worst_cfg, ensure_ascii=False, indent=1), encoding="utf-8")
(HERE / "empty.json").write_text(json.dumps(empty_cfg, ensure_ascii=False, indent=1), encoding="utf-8")
print("wrote worst_case.json, empty.json")
