# MiddleClick Scroll for TrackPoint

A Windows 11 tray-resident tool that lets you scroll by dragging in any direction while holding the middle button on a TrackPoint (e.g., on ThinkPads).

- Hold the middle button and move to scroll (the cursor stays frozen while pressed)
- Release without moving and it acts as a normal middle click
- Can be enabled/disabled per device

<br>

## Quick Start

### Download

Download `middleclick-scroll.exe` from the [Releases](https://github.com/ryomo/middleclick-scroll/releases) page and place it anywhere you like (e.g., `C:\Tools\middleclick-scroll.exe`).

### SmartScreen warning

Because this binary is not code-signed, Windows SmartScreen may block it from running.

To unblock the file:

1. Right-click `middleclick-scroll.exe` → **Properties**.
2. At the bottom of the **General** tab, check **Unblock**.
3. Click **OK**.

### Usage

When launched, it sits in the system tray. Clicking the tray icon opens a menu where you can toggle individual devices from the list of connected mice.

To run it automatically at Windows startup, place a shortcut to the exe in the folder opened by `Win+R` → `shell:startup`.

<br>

## Configuration file

Open the config file via "Open config file" in the tray menu (or edit `%APPDATA%\middleclick-scroll\config.toml` directly).
Changes take effect after restarting the tool.

| Key | Default | Meaning |
|---|---|---|
| `scroll_speed` | `4.0` | Scroll speed; higher is faster |
| `horizontal_scroll` | `true` | Enable horizontal scrolling |
| `invert_vertical` | `false` | Invert the vertical scroll direction |
| `drag_threshold` | `3` | Pointer movement (counts) before the press is treated as a drag instead of a click |
| `[devices."..."]` | — | Per-device `enabled` flag and display name |

<br>

## How it works

### Device detection

There is no reliable way to tell whether a device is a TrackPoint using OS APIs alone, so per-device on/off is left to the user.
A newly discovered device is enabled by default only if its name or device path contains `trackpoint` (e.g., the built-in ThinkPad `TrackPoint Device` is enabled automatically).

### Limitations

- Has no effect on windows of apps running as administrator, unless this tool is also run as administrator.

<br>

## Development

### Building

```powershell
cargo build --release
```

The binary will be at `target\release\middleclick-scroll.exe`.

### Release Process

TBD
