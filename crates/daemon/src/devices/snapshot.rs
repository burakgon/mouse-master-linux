//! Device state as published to clients (serialized to JSON over D-Bus).

use hidpp::features::battery::Battery;
use hidpp::features::device_info::{DeviceKind, FirmwareEntity};
use hidpp::features::dpi::DpiRange;
use hidpp::features::hosts_info::HostBus;
use hidpp::features::smartshift::WheelMode;
use hidpp::hidraw::Bus;
use serde::Serialize;

use crate::config::DeviceConfig;

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionInfo {
    pub bus: Bus,
    /// Receiver the device is paired with, if any.
    pub receiver: Option<String>,
    pub receiver_kind: Option<hidpp::receiver::ReceiverKind>,
    pub slot: Option<u8>,
    pub node: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ControlInfo {
    pub cid: u16,
    /// Configuration key (`gesture`, `middle`, `cid-01a0`, ...).
    pub key: String,
    pub name: String,
    pub role: hidpp::controls::Role,
    pub divertable: bool,
    pub gestures: bool,
    pub is_virtual: bool,
    pub mouse_button: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DpiCaps {
    pub range: DpiRange,
    pub min: u16,
    pub max: u16,
    pub default: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct SmartShiftCaps {
    pub torque: bool,
    pub default_threshold: u8,
    pub default_torque: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WheelCaps {
    pub invert: bool,
    pub ratchet_switch: bool,
    pub multiplier: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThumbwheelCaps {
    pub touch: bool,
    pub tap: bool,
    pub proximity: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HapticCaps {
    pub waveforms: Vec<(u8, String)>,
    pub discrete_levels: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Capabilities {
    pub dpi: Option<DpiCaps>,
    pub smartshift: Option<SmartShiftCaps>,
    pub wheel: Option<WheelCaps>,
    pub thumbwheel: Option<ThumbwheelCaps>,
    pub hosts: Option<u8>,
    pub host_names: bool,
    pub haptics: Option<HapticCaps>,
    pub report_rates_us: Vec<u32>,
    pub onboard_profiles: bool,
    pub gestures: bool,
    pub battery: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostEntry {
    pub index: u8,
    pub paired: bool,
    pub bus: HostBus,
    pub name: String,
    pub current: bool,
}

/// Live values read from (or last written to) the device.
#[derive(Debug, Clone, Default, Serialize)]
pub struct HardwareState {
    pub dpi: Option<u16>,
    pub wheel_mode: Option<WheelMode>,
    pub smartshift_threshold: Option<u8>,
    pub smartshift_torque: Option<u8>,
    pub hires_scroll: Option<bool>,
    pub invert_scroll: Option<bool>,
    pub ratchet_engaged: Option<bool>,
    pub thumbwheel_invert: Option<bool>,
    pub haptics: Option<bool>,
    pub haptics_level: Option<u8>,
    pub report_rate_us: Option<u32>,
    pub host_mode: Option<bool>,
    pub hosts: Vec<HostEntry>,
    pub current_host: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeatureInfo {
    pub id: u16,
    pub name: &'static str,
    pub version: u8,
    pub hidden: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceSnapshot {
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,
    /// Product id used to pick artwork and layouts.
    pub model_id: String,
    pub model_ids: Vec<String>,
    pub unit_id: String,
    pub serial: Option<String>,
    pub firmware: Vec<FirmwareEntity>,
    pub protocol: String,
    pub connection: ConnectionInfo,
    pub online: bool,
    /// Initialization is still running.
    pub busy: bool,
    pub battery: Option<Battery>,
    pub capabilities: Capabilities,
    pub state: HardwareState,
    pub controls: Vec<ControlInfo>,
    pub features: Vec<FeatureInfo>,
    pub config: DeviceConfig,
    /// Id of the application profile currently in effect.
    pub active_profile: Option<String>,
    pub error: Option<String>,
}
