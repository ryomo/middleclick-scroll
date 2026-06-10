use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    /// Wheel delta per count of Raw Input motion (120 equals one notch).
    pub scroll_speed: f64,
    /// Whether to enable horizontal scrolling.
    pub horizontal_scroll: bool,
    /// Whether to invert the vertical scroll direction.
    pub invert_vertical: bool,
    /// If the pointer moves more than this many counts while the middle button
    /// is held, the press is treated as a drag instead of a click.
    pub drag_threshold: u32,
    /// Device interface path → settings.
    pub devices: BTreeMap<String, DeviceConfig>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DeviceConfig {
    pub enabled: bool,
    /// Display name (cached). Not used for matching.
    pub name: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            scroll_speed: 4.0,
            horizontal_scroll: true,
            invert_vertical: false,
            drag_threshold: 3,
            devices: BTreeMap::new(),
        }
    }
}

pub fn path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("middleclick-scroll")
        .join("config.toml")
}

pub fn load() -> Config {
    fs::read_to_string(path())
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(config: &Config) {
    let p = path();
    if let Some(dir) = p.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(s) = toml::to_string_pretty(config) {
        let _ = fs::write(p, s);
    }
}
