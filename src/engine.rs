use std::collections::VecDeque;
use std::time::Instant;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_MIDDLEDOWN,
    MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_WHEEL, MOUSEINPUT, MOUSE_EVENT_FLAGS,
};

use crate::config::{self, Config, DeviceConfig};
use crate::devices::MouseDevice;

/// Marker (dwExtraInfo) for events we injected ourselves. "TPSS"
pub const MAGIC_EXTRA: usize = 0x5450_5353;

/// Action the hook should take when the middle button is released.
pub enum UpAction {
    /// Not ours. Let it through unchanged.
    Pass,
    /// Swallow it and do nothing (scroll finished).
    Swallow,
    /// Swallow it and synthesize the original middle click.
    SynthClick,
}

enum State {
    Idle,
    /// Middle button just pressed; not yet known whether it's a click or a drag.
    Pending { device: isize, moved: i32 },
    /// Confirmed as a drag; scrolling.
    Scrolling { device: isize },
}

struct PendingDown {
    device: isize,
    at: Instant,
}

pub struct Engine {
    pub config: Config,
    pub devices: Vec<MouseDevice>,
    /// Middle-button presses observed via Raw Input. The hook matches and consumes them.
    pending_downs: VecDeque<PendingDown>,
    state: State,
    acc_v: f64,
    acc_h: f64,
}

const PENDING_DOWN_TTL_MS: u128 = 500;

impl Engine {
    pub fn new(config: Config, devices: Vec<MouseDevice>) -> Self {
        let mut e = Engine {
            config,
            devices,
            pending_downs: VecDeque::new(),
            state: State::Idle,
            acc_v: 0.0,
            acc_h: 0.0,
        };
        e.sync_config_with_devices();
        e
    }

    pub fn refresh_devices(&mut self, devices: Vec<MouseDevice>) {
        self.devices = devices;
        self.sync_config_with_devices();
    }

    /// Register connected devices in the config. A device seen for the first
    /// time is enabled by default only if its name contains "trackpoint".
    fn sync_config_with_devices(&mut self) {
        let mut changed = false;
        for d in &self.devices {
            if !self.config.devices.contains_key(&d.path) {
                let haystack = format!("{} {}", d.name, d.path).to_lowercase();
                let enabled = haystack.contains("trackpoint");
                self.config
                    .devices
                    .insert(d.path.clone(), DeviceConfig { enabled, name: d.name.clone() });
                changed = true;
            }
        }
        if changed {
            config::save(&self.config);
        }
    }

    /// A middle-button press was observed via Raw Input (arrives before the hook).
    pub fn push_middle_down(&mut self, device: isize) {
        self.pending_downs.push_back(PendingDown { device, at: Instant::now() });
        if self.pending_downs.len() > 8 {
            self.pending_downs.pop_front();
        }
    }

    pub fn has_pending_down(&self) -> bool {
        self.pending_downs
            .iter()
            .any(|p| p.at.elapsed().as_millis() < PENDING_DOWN_TTL_MS)
    }

    /// The hook received WM_MBUTTONDOWN. Returns true to swallow it (= scroll candidate).
    pub fn on_middle_down(&mut self) -> bool {
        while let Some(front) = self.pending_downs.front() {
            if front.at.elapsed().as_millis() >= PENDING_DOWN_TTL_MS {
                self.pending_downs.pop_front();
            } else {
                break;
            }
        }
        let Some(p) = self.pending_downs.pop_front() else {
            // If the source device cannot be determined, let the original behavior through.
            return false;
        };
        if !matches!(self.state, State::Idle) {
            return false;
        }
        if self.device_enabled(p.device) {
            self.state = State::Pending { device: p.device, moved: 0 };
            self.acc_v = 0.0;
            self.acc_h = 0.0;
            true
        } else {
            false
        }
    }

    pub fn on_middle_up(&mut self) -> UpAction {
        match self.state {
            State::Idle => UpAction::Pass,
            State::Pending { .. } => {
                self.state = State::Idle;
                UpAction::SynthClick
            }
            State::Scrolling { .. } => {
                self.state = State::Idle;
                UpAction::Swallow
            }
        }
    }

    /// Cursor movement is frozen while Pending or Scrolling.
    pub fn is_active(&self) -> bool {
        !matches!(self.state, State::Idle)
    }

    /// Relative motion from Raw Input. Returns the (vertical, horizontal) deltas to convert into scrolling.
    pub fn on_raw_move(&mut self, device: isize, dx: i32, dy: i32) -> Option<(i32, i32)> {
        match &mut self.state {
            State::Pending { device: d, moved } if *d == device => {
                *moved += dx.abs() + dy.abs();
                if *moved > self.config.drag_threshold as i32 {
                    self.state = State::Scrolling { device };
                }
                None
            }
            State::Scrolling { device: d } if *d == device => {
                let sign = if self.config.invert_vertical { 1.0 } else { -1.0 };
                self.acc_v += dy as f64 * self.config.scroll_speed * sign;
                if self.config.horizontal_scroll {
                    self.acc_h += dx as f64 * self.config.scroll_speed;
                }
                let v = self.acc_v as i32;
                let h = self.acc_h as i32;
                self.acc_v -= v as f64;
                self.acc_h -= h as f64;
                if v != 0 || h != 0 { Some((v, h)) } else { None }
            }
            _ => None,
        }
    }

    fn device_enabled(&self, device: isize) -> bool {
        let Some(d) = self.devices.iter().find(|d| d.handle == device) else {
            return false;
        };
        self.config
            .devices
            .get(&d.path)
            .map(|c| c.enabled)
            .unwrap_or(false)
    }

    pub fn toggle_device(&mut self, path: &str) {
        if let Some(c) = self.config.devices.get_mut(path) {
            c.enabled = !c.enabled;
            config::save(&self.config);
        }
    }
}

fn mouse_input(flags: MOUSE_EVENT_FLAGS, data: i32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: data as u32,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: MAGIC_EXTRA,
            },
        },
    }
}

pub fn send_wheel(v: i32, h: i32) {
    let mut inputs: Vec<INPUT> = Vec::with_capacity(2);
    if v != 0 {
        inputs.push(mouse_input(MOUSEEVENTF_WHEEL, v));
    }
    if h != 0 {
        inputs.push(mouse_input(MOUSEEVENTF_HWHEEL, h));
    }
    if !inputs.is_empty() {
        unsafe {
            SendInput(&inputs, size_of::<INPUT>() as i32);
        }
    }
}

/// Re-send a swallowed middle click as the original click.
pub fn send_middle_click() {
    let inputs = [
        mouse_input(MOUSEEVENTF_MIDDLEDOWN, 0),
        mouse_input(MOUSEEVENTF_MIDDLEUP, 0),
    ];
    unsafe {
        SendInput(&inputs, size_of::<INPUT>() as i32);
    }
}
