use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    /// Raw Inputの移動量1カウントあたりのホイールdelta(120で1ノッチ相当)。
    pub scroll_speed: f64,
    /// 水平スクロールを有効にするか。
    pub horizontal_scroll: bool,
    /// 垂直スクロールの方向を反転するか。
    pub invert_vertical: bool,
    /// ミドルボタン押下中にこのカウント以上動いたら「ドラッグ」とみなす閾値。
    pub drag_threshold: u32,
    /// デバイスのインターフェイスパス → 設定。
    pub devices: BTreeMap<String, DeviceConfig>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DeviceConfig {
    pub enabled: bool,
    /// 表示用の名前(キャッシュ)。判定には使わない。
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
