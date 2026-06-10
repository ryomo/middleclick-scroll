# MiddleClick Scroll for TrackPoint

A Windows 11 tray-resident tool that lets you scroll by dragging in any direction while holding the middle button on a TrackPoint (e.g. on ThinkPads).

- Hold the middle button and move to scroll (the cursor stays frozen while pressed)
- Release without moving and it acts as a normal middle click
- Can be enabled/disabled per device

## Usage

```
cargo build --release
.\target\release\middleclick-scroll.exe
```

When launched, it sits in the system tray. Clicking the tray icon opens a menu
where you can toggle individual devices from the list of connected mice.

To run it automatically at Windows startup, place a shortcut to the exe in the
folder opened by `Win+R` → `shell:startup`.

## Device detection

There is no reliable way to tell whether a device is a TrackPoint using OS APIs
alone, so per-device on/off is left to the user. A newly discovered device is
enabled by default only if its name or device path contains `trackpoint`
(e.g. the built-in ThinkPad `TrackPoint Device` is enabled automatically).

## Configuration file

`%APPDATA%\middleclick-scroll\config.toml` (also reachable via "Open config file" in the tray menu).
Changes take effect after restarting the tool (device on/off is immediate when done from the tray menu).

| Key | Default | Meaning |
|---|---|---|
| `scroll_speed` | `4.0` | Wheel delta per count of movement (120 equals one notch). Higher is faster |
| `horizontal_scroll` | `true` | Whether to enable horizontal scrolling |
| `invert_vertical` | `false` | Whether to invert the vertical scroll direction |
| `drag_threshold` | `3` | If the pointer moves more than this many counts, the press is treated as a drag instead of a click |
| `[devices."..."]` | — | Per-device `enabled` flag and display name (auto-generated) |

## How it works

- Raw Input (`WM_INPUT`) identifies which device a mouse event came from (Raw Input cannot block input).
- A low-level mouse hook (`WH_MOUSE_LL`) blocks and replaces input (the hook cannot identify the source device).
- When the hook receives a middle-button press, it pumps the Raw Input events
  already waiting in the queue and matches them against the press to determine
  which device it came from.
- If the press came from an enabled device, it is swallowed. If the button is
  released before moving more than `drag_threshold`, the original middle click
  is synthesized and sent; if it moves beyond the threshold, the Raw Input
  motion is converted into wheel events
  (`MOUSEEVENTF_WHEEL` / `MOUSEEVENTF_HWHEEL`).

## Limitations

- Has no effect on windows of apps running as administrator, unless this tool is also run as administrator (a limitation of low-level hooks).
- While scrolling (middle button held), cursor movement from other mice is also temporarily frozen.
- If an older app ignores wheel deltas smaller than 120, raising `scroll_speed` may help.
