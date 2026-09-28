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

To run it automatically at Windows startup, use **either** of the following methods.

#### Option 1: Startup folder (simple)

Place a shortcut to the exe in the folder opened by `Win+R` → `shell:startup`.

This is enough for most apps. However, Windows does not let a normal process send input to apps running as administrator (e.g., PowerToys with "Always run as administrator"), so scrolling does not work over their windows; the middle button behaves as a normal middle click there.

#### Option 2: Task Scheduler (run as administrator)

Use this if you also want to scroll in apps running as administrator. It starts the tool elevated at logon:

1. Open **Task Scheduler** → **Create Task...**.
2. **General** tab: check **Run with highest privileges**.
3. **Triggers** tab: **New...** → **At log on**.
4. **Actions** tab: **New...** → **Start a program** → select `middleclick-scroll.exe`.
5. **Conditions** tab: uncheck **Start the task only if the computer is on AC power**.
6. **Settings** tab: uncheck **Stop the task if it runs longer than**.

<br>

## Configuration file

Open the config file via "Open config file" in the tray menu (or edit `%APPDATA%\middleclick-scroll\config.toml` directly).
Changes take effect after restarting the tool.

| Key | Default | Meaning |
|---|---|---|
| `scroll_speed` | `10.0` | Scroll speed; higher is faster |
| `quantize_wheel` | `true` | Send wheel events only in whole notches (120). Needed for UWP/WinUI apps (Microsoft Store apps, PowerToys); set to `false` for smoother scrolling in apps that handle fine-grained wheel input, such as browsers |
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

1. Bump `version` in `Cargo.toml` (e.g. `0.3.0`), run `cargo build` to update `Cargo.lock`, then commit and push to `main`:

   ```powershell
   git commit -m "chore(release): v0.3.0"
   git push
   ```

2. On GitHub, open **Actions → Release → Run workflow** on `main`.
3. The workflow builds the binary and creates a draft release for `v<version>`. Its notes are generated from `feat:`/`fix:`/other commits since the previous tag (`docs:` and `chore(release):` are omitted).
4. Review and edit the draft, then publish it. Publishing creates the `v<version>` tag.

To preview the notes locally: `bash .github/scripts/release-notes.sh v0.3.0`

<br>

## License

MIT
