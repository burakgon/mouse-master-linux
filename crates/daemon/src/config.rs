//! Persistent configuration, stored as TOML in `$XDG_CONFIG_HOME/mouse-master/config.toml`.
//!
//! Hardware settings are `Option`s: `None` means "leave whatever the device
//! has", so Mouse Master never changes anything the user did not touch.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::actions::Action;

pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub general: General,
    pub devices: BTreeMap<String, DeviceConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Config { version: CONFIG_VERSION, general: General::default(), devices: BTreeMap::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct General {
    /// Desktop notifications for low battery and connection changes.
    pub notifications: bool,
    /// Battery percentage below which a low battery notification is shown.
    pub low_battery_percent: u8,
    /// On screen display when DPI or SmartShift change from the device.
    pub osd: bool,
    /// Switch bindings automatically based on the focused application.
    pub app_profiles: bool,
}

impl Default for General {
    fn default() -> Self {
        General { notifications: true, low_battery_percent: 15, osd: true, app_profiles: true }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct DeviceConfig {
    /// Last known name, for humans reading the file.
    pub name: String,
    pub settings: DeviceSettings,
    /// Bindings used when no application profile matches.
    pub bindings: Bindings,
    /// Application specific overrides, checked in order.
    pub profiles: Vec<AppProfile>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WheelModeSetting {
    Ratchet,
    FreeSpin,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct DeviceSettings {
    pub dpi: Option<u16>,
    /// DPI values cycled through by the DPI cycle actions.
    pub dpi_presets: Vec<u16>,
    /// Wheel mode the device starts in.
    pub wheel_mode: Option<WheelModeSetting>,
    /// SmartShift on (automatic switch to free spin when scrolling fast) or off.
    pub smartshift: Option<bool>,
    /// SmartShift sensitivity, 1 (very sensitive) ..= 50 (almost never).
    pub smartshift_threshold: Option<u8>,
    /// Ratchet force in percent (devices with tunable torque).
    pub smartshift_torque: Option<u8>,
    pub hires_scroll: Option<bool>,
    pub invert_scroll: Option<bool>,
    pub thumbwheel_invert: Option<bool>,
    pub haptics: Option<bool>,
    pub haptics_level: Option<u8>,
    pub report_rate_us: Option<u32>,
    /// Keep gaming mice in host mode so settings take effect.
    pub host_mode: Option<bool>,
}

/// What the thumb wheel does.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ThumbwheelBinding {
    /// Native horizontal scrolling (not diverted).
    #[default]
    Default,
    /// Horizontal scrolling with a custom speed. Direction follows `thumbwheel_invert`.
    Scroll {
        #[serde(default = "one")]
        speed: f32,
    },
    /// Smooth zoom like a touchpad pinch: right zooms in, left zooms out.
    Zoom {
        #[serde(default = "one")]
        speed: f32,
    },
    /// Run one action per step to the left and another per step to the right.
    Actions {
        left: Action,
        right: Action,
        /// Rotation (in degrees of the wheel) per step.
        #[serde(default = "default_step_degrees")]
        step_degrees: f32,
    },
}

fn one() -> f32 {
    1.0
}

fn default_step_degrees() -> f32 {
    20.0
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Bindings {
    /// Button bindings keyed by control key (`gesture`, `middle`, `back`, `cid-01a0`, ...).
    pub buttons: BTreeMap<String, Action>,
    pub thumbwheel: Option<ThumbwheelBinding>,
    /// Action for a single tap on the thumb wheel (touch sensitive wheels only).
    pub thumbwheel_tap: Option<Action>,
}

impl Bindings {
    /// Overlays `other` on top of `self`: entries in `other` win.
    pub fn overlay(&self, other: &Bindings) -> Bindings {
        let mut buttons = self.buttons.clone();
        for (k, v) in &other.buttons {
            buttons.insert(k.clone(), v.clone());
        }
        Bindings {
            buttons,
            thumbwheel: other.thumbwheel.clone().or_else(|| self.thumbwheel.clone()),
            thumbwheel_tap: other.thumbwheel_tap.clone().or_else(|| self.thumbwheel_tap.clone()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct AppProfile {
    /// Stable identifier (generated).
    pub id: String,
    /// Display name, usually the application's name.
    pub name: String,
    /// Application ids (desktop file ids, Wayland app ids or X11 classes, case insensitive).
    pub apps: Vec<String>,
    pub enabled: bool,
    pub bindings: Bindings,
    /// Optional DPI while the application is focused.
    pub dpi: Option<u16>,
}

impl Default for AppProfile {
    fn default() -> Self {
        AppProfile {
            id: String::new(),
            name: String::new(),
            apps: Vec::new(),
            enabled: true,
            bindings: Bindings::default(),
            dpi: None,
        }
    }
}

impl AppProfile {
    pub fn matches(&self, app: &crate::window::ActiveApp) -> bool {
        self.enabled && self.apps.iter().any(|a| app.matches(a))
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".config")).join("mouse-master")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

impl Config {
    pub fn load(path: &Path) -> Result<Config> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let cfg: Config = toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
                Ok(cfg)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let dir = path.parent().context("config path has no parent")?;
        std::fs::create_dir_all(dir)?;
        let text = toml::to_string_pretty(self)?;
        let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
        std::io::Write::write_all(
            &mut tmp,
            b"# Mouse Master configuration. Edit with the app, or by hand while the service is stopped.\n\n",
        )?;
        std::io::Write::write_all(&mut tmp, text.as_bytes())?;
        tmp.persist(path)?;
        Ok(())
    }
}

/// Shared, debounced-persisted configuration.
#[derive(Clone)]
pub struct ConfigStore {
    inner: Arc<Mutex<Config>>,
    path: Arc<PathBuf>,
    dirty: tokio::sync::mpsc::UnboundedSender<()>,
}

impl ConfigStore {
    pub fn open(path: PathBuf) -> ConfigStore {
        let config = match Config::load(&path) {
            Ok(c) => c,
            Err(e) => {
                // Keep the broken file for the user and start fresh.
                warn!("{e:#}; starting with defaults");
                let backup = path.with_extension("toml.broken");
                let _ = std::fs::rename(&path, &backup);
                Config::default()
            }
        };
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        let store = ConfigStore { inner: Arc::new(Mutex::new(config)), path: Arc::new(path), dirty: tx };
        let writer = store.clone();
        tokio::spawn(async move {
            while rx.recv().await.is_some() {
                // Coalesce bursts of changes (slider drags) into one write.
                tokio::time::sleep(Duration::from_millis(400)).await;
                while rx.try_recv().is_ok() {}
                writer.flush();
            }
        });
        store
    }

    pub fn get(&self) -> Config {
        self.inner.lock().clone()
    }

    pub fn general(&self) -> General {
        self.inner.lock().general.clone()
    }

    pub fn device(&self, id: &str) -> DeviceConfig {
        self.inner.lock().devices.get(id).cloned().unwrap_or_default()
    }

    pub fn has_device(&self, id: &str) -> bool {
        self.inner.lock().devices.contains_key(id)
    }

    /// Mutates the configuration and schedules a save.
    pub fn update<R>(&self, f: impl FnOnce(&mut Config) -> R) -> R {
        let r = f(&mut self.inner.lock());
        let _ = self.dirty.send(());
        r
    }

    pub fn update_device<R>(&self, id: &str, f: impl FnOnce(&mut DeviceConfig) -> R) -> R {
        self.update(|c| f(c.devices.entry(id.to_string()).or_default()))
    }

    pub fn replace(&self, config: Config) {
        *self.inner.lock() = config;
        let _ = self.dirty.send(());
    }

    pub fn flush(&self) {
        let snapshot = self.get();
        match snapshot.save(&self.path) {
            Ok(()) => info!("saved configuration to {}", self.path.display()),
            Err(e) => warn!("failed to save configuration: {e:#}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::{Action, GestureSet};

    #[test]
    fn roundtrip_toml() {
        let mut cfg = Config::default();
        let mut dev = DeviceConfig { name: "MX Master 3S".into(), ..Default::default() };
        dev.settings.dpi = Some(1600);
        dev.settings.smartshift = Some(true);
        dev.bindings.buttons.insert("gesture".into(), Action::Gestures(GestureSet::default()));
        dev.bindings.buttons.insert("middle".into(), Action::Builtin { id: "overview".into() });
        dev.bindings.thumbwheel = Some(ThumbwheelBinding::Scroll { speed: 1.5 });
        dev.profiles.push(AppProfile {
            id: "p1".into(),
            name: "Firefox".into(),
            apps: vec!["firefox".into()],
            ..Default::default()
        });
        cfg.devices.insert("b034-5C8F7AF8".into(), dev);
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn empty_file_is_default() {
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn overlay_prefers_profile() {
        let mut base = Bindings::default();
        base.buttons.insert("back".into(), Action::Default);
        base.buttons.insert("forward".into(), Action::None);
        let mut over = Bindings::default();
        over.buttons.insert("back".into(), Action::Builtin { id: "copy".into() });
        let merged = base.overlay(&over);
        assert_eq!(merged.buttons["back"], Action::Builtin { id: "copy".into() });
        assert_eq!(merged.buttons["forward"], Action::None);
    }
}
