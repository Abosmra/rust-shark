# Lift-off distance on the Attack Shark X6 is a placebo

The official Attack Shark X6 app has a **Lift Of Distance** switch (1 mm / 2 mm).
It does nothing. The app saves your choice to its own settings file and redraws the
switch, but **never sends it to the mouse**. The mouse keeps using its firmware's built-in
lift-off distance no matter what you pick.

That's why Rust Shark doesn't offer the setting. This page shows how that was established,
so you can check it yourself.

Tested with the official app at `C:\Program Files (x86)\Attack SharkX6Mouse\Mouse.exe`
(635,904 bytes, SHA-256 `36B80804C2447D5E1837A27D20AEC88AA27021CDBA51CAB1738F3A578929FEDF`).

## Summary of the evidence

| Check | Result |
| --- | --- |
| Live capture: what reaches the mouse when you click 2 mm → 1 mm → 2 mm | **Nothing. Zero packets.** |
| Did the click register in the app? | Yes. `pro.data` was rewritten and the stored value changed from 1 mm to 2 mm. |
| Every instruction in `Mouse.exe` that touches the stored value | 5 instructions: UI redraw, store, change check (×2), reset to default. **None builds a packet.** |
| Any other way for `Mouse.exe` to talk to the mouse? | No. It only calls `SetFeature` / `GetFeature` on its HID driver DLL, and both were logged. |
| Protocol | None of the six settings commands (`04`, `05`, `06`, `08`, `09`, `0c`) has a lift-off field. |

## 1. Live capture

The official app talks to the mouse through two DLLs that ship with it, `hiddriver_1.dll`
(2.4 GHz receiver) and `hiddriver_2.dll` (cable). They export `SetFeature`, `GetFeature`,
`Set_VIDPID`, `Open_FeatureDevice` and a few others.

[`tools/capture-shim`](../tools/capture-shim) is a stand-in for those DLLs. It logs every
`SetFeature`, `GetFeature` and `Set_VIDPID` call with the exact bytes, then passes it to the
real driver. Everything else is forwarded untouched.

With the shim in place, every real setting shows up in the log within about a second of
the click. For example, the full write the app does on startup:

```text
15:05:20.897 drv1 SET len=10 ok=1 : 0c 0a 01 fe 01 fe                       begin write
15:05:21.906 drv1 SET len=56 ok=1 : 04 38 01 01 01 3f 01 01 4a 09 09 09 …   DPI / sensor
15:05:22.911 drv1 SET len=15 ok=1 : 05 0f 01 10 13 e1 00 ff ff 0a 02 03 0e  lighting / power
15:05:23.925 drv1 SET len=9  ok=1 : 06 09 01 02 fd                          polling rate
15:05:24.932 drv1 SET len=59 ok=1 : 08 3b 01 02 00 00 03 00 00 04 00 00 …   buttons
```

Then the lift-off test: the app was reopened, and the only thing clicked was the
Lift Of Distance switch, **2 MM**, **1 MM**, **2 MM**, a couple of seconds apart.
This is the complete log for that session:

```text
===== LOD TEST START 15:48:08 =====
15:48:18.752 drv2 VIDPID len=8 ok=1 : 57 1d 00 00 61 fa    app startup: picks the cable driver
15:48:18.809 drv1 VIDPID len=8 ok=1 : 57 1d 00 00 60 fa    app startup: picks the receiver
```

Two startup lines, then nothing. Three clicks on lift-off produced **zero** packets.

The clicks did register: the app's settings file
`%APPDATA%\Attack SharkX6Mouse\ms_1\pro.data` was rewritten during the test, and the
lift-off field (offset `0x8B4` in the profile) changed from `0` (1 mm) to `1` (2 mm).

### Could it go out some other way?

No. `Mouse.exe` loads the driver DLL by name and the only functions it looks up are
`SetFeature`, `GetFeature` and `Open_FeatureDevice`. The strings `Hid_Write`,
`HidD_SetOutputReport` and `WriteFile` don't appear in `Mouse.exe` at all, and the only
`hidapi.dll` functions it imports are `hid_init`, `hid_enumerate` and
`hid_free_enumeration`, which list devices but can't write to them. So `SetFeature` is the
only way the app can change anything on the mouse, and it was logged.

## 2. Code check

The lift-off value lives at offset `0x8B4` of the app's profile structure (the same
structure that's saved to `pro.data`). Disassembling `Mouse.exe` (29,898 instructions) and
searching for every access to `+ 0x8b4` finds exactly five:

| Address | Instruction | What it is |
| --- | --- | --- |
| `0x411220` | `mov dword ptr [eax + 0x8b4], ecx` | The switch's click handler stores 0 or 1, then posts window message `0x531` |
| `0x40f53d` | `mov eax, dword ptr [eax + 0x8b4]` | The `0x531` handler reads it back to tick the matching radio button (UI only) |
| `0x413a01` | `mov eax, dword ptr [edx + 0x8b4]` | "Has anything changed?" comparison… |
| `0x413a07` | `cmp eax, dword ptr [ecx + 0x8b4]` | …which only sets a dirty flag so the profile gets saved to disk |
| `0x4156b1` | `mov dword ptr [edi + 0x8b4], 0` | Restore defaults: back to 1 mm |

The packet builders read their fields from neighbouring offsets (`0x8C4` DPI, `0x904`
DPI mask, `0x924`–`0x960` lighting, power, polling, ripple, angle snap, motion sync, key
response, colors). None of them reads `0x8B4`.

The click handler, for reference:

```text
411211: mov eax, dword ptr [esi + 0x5dba4]   ; current profile
411217: push 0
411219: push 0
41121b: push 0x531                           ; "lift-off changed" window message
411220: mov dword ptr [eax + 0x8b4], ecx     ; store 0 (1 mm) or 1 (2 mm)
411226: push dword ptr [esi + 0x168]         ; window handle
41122c: call edi                             ; PostMessage
```

and the `0x531` handler, which only updates the radio buttons:

```text
40f533: mov eax, dword ptr [esi + 0x5dba4]   ; current profile
40f53d: mov eax, dword ptr [eax + 0x8b4]     ; 0 or 1
40f543: mov ecx, dword ptr [esi + eax*4 + 0x3b0]   ; the 1 mm or 2 mm radio button
40f54c: call dword ptr [eax + 0x1c0]         ; radio->Selected(true): UI only
```

## 3. The protocol has no field for it

Every setting the official app can change is sent with one of six commands (see the
[protocol notes](../README.md#protocol-notes)). Rust Shark builds those packets from a
settings model that has **no lift-off value at all**, and its tests check the output byte
for byte against packets captured from the official app. They match, so none of the
official packets can depend on the lift-off switch.

Whether the sensor *could* be told a different lift-off distance through some command
that this app never sends is a separate question; nothing in the official software
does it.

## Check it yourself

### Quick feel test

Move the mouse slowly while lifting it off the pad, and note when the cursor stops.
Switch between 1 mm and 2 mm in the official app and repeat. The cutoff height doesn't change.

### Capture test

1. Build the shim: run `tools\capture-shim\build.bat` (needs Visual Studio with the C++
   workload). It produces `hiddriver_1.dll` and `hiddriver_2.dll`.
2. Copy the official app's folder somewhere you can write to, for example
   `C:\x6-capture`. Don't modify the installed copy.
3. In the copy, rename the real `hiddriver_1.dll` to `hiddriver_1o.dll` and
   `hiddriver_2.dll` to `hiddriver_2o.dll`, then put the two shim DLLs in their place.
4. Close the official app (including its tray icon) and Rust Shark, then run `Mouse.exe`
   from the copy.
5. Change any normal setting, such as polling rate, and watch `capture.log` next to
   `Mouse.exe`: a `SET` line appears. Then click the lift-off switch a few times: no line
   appears.
6. Delete the copy when you're done.
