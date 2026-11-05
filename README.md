<p align="center"><img src="assets/logo_256.png" width="128" alt="Rust Shark logo"></p>

# Rust Shark

**A cleaner, faster and better-looking alternative to the official Attack Shark X6 software.**

Rust Shark is a native control app for the **Attack Shark X6** wireless mouse, written in Rust.
It does everything the official app does, in one small `.exe`: no installer, no background
service, and it starts instantly.

<p align="center"><img src="docs/screenshots/home.png" alt="Home: everything at a glance" width="820"></p>

| Performance | Macros |
| --- | --- |
| <img src="docs/screenshots/performance.png" alt="Performance page: DPI stages and polling rate"> | <img src="docs/screenshots/macros.png" alt="Macro editor: actions, delays and repeat modes"> |
| **Buttons** | **Lighting** |
| <img src="docs/screenshots/buttons.png" alt="Buttons page: remap all 7 buttons"> | <img src="docs/screenshots/lighting.png" alt="Lighting page: effects and colors"> |

> Not affiliated with or endorsed by Attack Shark. "Attack Shark" is their trademark.

## Features

- **DPI**: 6 stages, value and LED color per stage, live sync when you switch DPI on the mouse
- **Polling rate**: 125 / 250 / 500 / 1000 Hz
- **Buttons**: remap all 7 buttons to mouse functions, DPI, media, keyboard shortcuts or macros
- **Macros**: record or build sequences, edit and reorder actions and delays, repeat modes,
  import/export as JSON
- **Lighting**: effects, color, brightness and speed
- **Power**: sleep and deep-sleep timers, battery level
- **Sensor**: key response time, ripple control, angle snap, motion sync
- **Profiles**: multiple profiles, imported from the official app on first run
- **Auto-save** or an explicit Save / Reset bar

### Known limits

- **Battery** is reported by the mouse firmware in 10% steps. On some units the firmware
  always reports 100%; no app can fix that.
- **Lift-off distance** isn't offered: the official app's switch is a placebo that never
  reaches the mouse. [Here's the proof](docs/lift-off-distance.md).
- Windows only (it talks to the mouse through the Windows HID API).

## Download / build

Grab `RustShark.exe` from the [latest release](https://github.com/Abosmra/rust-shark/releases/latest),
or build it yourself. Building requires [Rust](https://rustup.rs/) (stable) on Windows.

```bash
git clone https://github.com/Abosmra/rust-shark
cd rust-shark
cargo build --release
```

The app is `target/release/rust-shark.exe`. Close the official Attack Shark app first:
both can't talk to the receiver at the same time.

Settings are stored in `%APPDATA%\RustShark\config.json`.

```bash
cargo test
```

### Releasing

CI builds and tests every push. To publish a release, bump `version` in `Cargo.toml`, then
push a tag; the workflow attaches the built `.exe`:

```bash
git tag v0.2.0
```

```bash
git push origin v0.2.0
```

## Project layout

| Path | What it does |
| --- | --- |
| `src/protocol.rs` | Settings model and the packet builders for the X6 (no I/O, unit-tested) |
| `src/device.rs` | HID connection, background writer thread, status reports (battery, DPI changes) |
| `src/store.rs` | Config file, import from the official app's `pro.data` / `macro.data` |
| `src/keys.rs` | Button functions, key codes and names |
| `src/ui.rs` | The egui interface |
| `tools/` | Scripts that generate the logo and the embedded icon data |
| `tools/capture-shim/` | Logging stand-in for the official app's HID driver, to capture what it sends |

`protocol.rs` has no dependencies on the UI, so it's the place to start if you want to
support the X6 from another app or language.

## Protocol notes

Reverse-engineered from the official app and USB captures. Useful if you're writing your own tool.

- **USB IDs**: VID `0x1D57`, PID `0xFA60` (2.4 GHz receiver), `0xFA61` (wired).
- **Writes** are HID feature reports on the interface with usage page `0x0B`, sent at their
  exact length. Packet: `[cmd][len][01][payload…][sum16 hi][sum16 lo]`, where the checksum
  is the sum of bytes from index 3.
- **Status** comes as input reports on usage page `0x0A`: `03 lo hi p0 p1`
  (`0x4010` battery state + level 0–10, `0x5010` ack, `0x1010` DPI stage changed on the mouse).

| Cmd | Length | Contents |
| --- | --- | --- |
| `04` | 56 | DPI: per-stage value (`DPI / 50 − 1`), enabled mask, active stage, 8 RGB colors, angle snap, ripple, motion sync |
| `05` | 15 | Lighting mode, speed, brightness, color, sleep timers, key response |
| `06` | — | Polling: `1` = 1000 Hz, `2` = 500, `4` = 250, `8` = 125, plus its inverse |
| `08` | 59 | Button map: 18 slots × 3 bytes (function, shortcut `[0x11, mods, key]`, macro `[0x12, 0, slot+1]`) |
| `09` | — | Macro upload: 131-byte buffer in 3 chunks; event = delay byte (`ms/10`, press `0x01` / release `0x81`) + key code; max 50 events |
| `0c` | — | Begin a full write |

See `src/protocol.rs` and its tests for the exact byte layouts.

## Contributing

Issues and pull requests are welcome. If you have a different Attack Shark model, a USB
capture from the official app is the most useful thing you can share.

## Credits

- Keyboard and mouse input icons by [amanz](https://amanz.itch.io/keyboard-mouse-icon-pack-free) (CC0).
- Built with [egui](https://github.com/emilk/egui) and [hidapi](https://github.com/ruabmbua/hidapi-rs).

## License

[MIT](LICENSE). The input icons are CC0 (see `assets/input-icons/CREDITS.md`).
