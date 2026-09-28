# AGENTS.md

This file provides guidance to AI coding agents when working with code in this repository.

## Project

Windows-only tray app (Rust, edition 2024, `windows` crate) that turns a middle-button drag on a TrackPoint into wheel scrolling. Holding the middle button and moving scrolls, with the cursor frozen. Releasing without moving sends a normal middle click. Scrolling can be turned on or off for each device.

## Commands

```powershell
cargo build                         # debug build: keeps the console window, so println!/panics are visible
cargo build --release               # release build: GUI subsystem, no console (see cfg_attr in main.rs)
cargo run                           # run the app (only one instance allowed; exit the tray copy first)
cargo run --example list_mice       # print what devices::enumerate_mice() returns (handle/path/name)
cargo clippy
```

There are no tests. `build.rs` embeds `assets/icon.ico` as resource ID 1 via `winres`, and `tray::add_icon` loads it by that ID.

## Architecture

The core trick is described in the header comment of `src/main.rs`. It combines two Win32 input mechanisms because neither can do the job alone:

- **Raw Input (`WM_INPUT`)** tells which physical device produced an event, but it cannot block input.
- **Low-level mouse hook (`WH_MOUSE_LL`)** can swallow or replace input, but it cannot tell which device produced it.

When the hook receives `WM_MBUTTONDOWN`, `pump_raw_input` runs `PeekMessage` to pump pending `WM_INPUT` messages from inside the hook, for up to 20ms. The matching Raw Input event is already queued ahead of the hook. `handle_raw_input` records the source device with `Engine::push_middle_down`, and then `Engine::on_middle_down` consumes that record to decide whether to swallow the press. `IN_PUMP` guards against the hook being re-entered during the pump.

`src/engine.rs` holds all the state, in a global `Mutex<Engine>` (`main::engine()`):
- A state machine: `Idle` → `Pending { device, moved }` → `Scrolling { device }`. Once motion passes `drag_threshold`, `Pending` becomes `Scrolling`. On release, `Pending` synthesizes a click (`UpAction::SynthClick`) and `Scrolling` ends with `UpAction::Swallow`.
- While the state is not `Idle`, the hook drops `WM_MOUSEMOVE`, which freezes the cursor. Scrolling is driven by Raw Input deltas, not by hook events.
- Wheel output is accumulated and flushed at most once every `FLUSH_INTERVAL_MS`. The sub-unit remainder is kept for the next flush, and a final `flush_and_send` runs on release.
- Events the app injects itself carry `MAGIC_EXTRA` in `dwExtraInfo`, so the hook passes them through instead of handling them again. Raw Input events with `hDevice == 0` are injected and are ignored when matching devices.

Devices and config:
- `devices.rs` enumerates mice through Raw Input and resolves display names (HID product string, with a fallback to the devnode name). The Raw Input `handle` changes on reconnect. The device interface `path` is stable, so config is keyed by `path`.
- `config.rs` reads and writes `%APPDATA%\middleclick-scroll\config.toml` (serde, `#[serde(default)]`). `Engine::sync_config_with_devices` adds newly seen devices and saves. A new device is enabled only if its name or path contains "trackpoint". Tray toggles also save immediately. Other config edits take effect only after a restart.
- `WM_INPUT_DEVICE_CHANGE` (from registering with `RIDEV_DEVNOTIFY`) re-enumerates devices.

Other notes:
- `tray.rs`: tray icon and popup menu, re-added when the `TaskbarCreated` message arrives (Explorer restart). It snapshots engine state before calling `TrackPopupMenu` so the lock is not held while the menu is open.
- A named mutex (`Local\middleclick-scroll-instance`) prevents running two instances.
- The panic hook writes `panic.log` next to the config file and shows a message box.
- The hook callback must return quickly. Keep `Engine` lock sections short and never hold the lock across `SendInput` or message pumping.

## Code style

Recent commits prefer calling functions before `if let`/`let else` rather than inside the condition (for example, `let x = f(); if let Some(v) = x {...}`), and they keep constants as associated consts on `Engine`. Commit messages follow Conventional Commits (`feat:`, `fix:`, `refactor:`, `docs:`).
