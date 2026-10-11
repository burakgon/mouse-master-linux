//! HID++ 2.0 features.
//!
//! Each module implements the requests and event parsing for one feature (or a
//! family of closely related features). Functions take a [`Device`] and the
//! feature is looked up in its feature table.
//!
//! [`Device`]: crate::Device

pub mod battery;
pub mod change_host;
pub mod device_info;
pub mod dpi;
pub mod haptic;
pub mod hires_wheel;
pub mod hosts_info;
pub mod onboard_profiles;
pub mod pointer_speed;
pub mod report_rate;
pub mod reprog_controls;
pub mod smartshift;
pub mod thumbwheel;
pub mod wireless_status;

/// Well known HID++ 2.0 feature ids.
pub mod id {
    pub const ROOT: u16 = 0x0000;
    pub const FEATURE_SET: u16 = 0x0001;
    pub const DEVICE_FW_VERSION: u16 = 0x0003;
    pub const DEVICE_NAME: u16 = 0x0005;
    pub const DEVICE_FRIENDLY_NAME: u16 = 0x0007;
    pub const CONFIG_CHANGE: u16 = 0x0020;
    pub const BATTERY_STATUS: u16 = 0x1000;
    pub const BATTERY_VOLTAGE: u16 = 0x1001;
    pub const UNIFIED_BATTERY: u16 = 0x1004;
    pub const CHANGE_HOST: u16 = 0x1814;
    pub const HOSTS_INFO: u16 = 0x1815;
    pub const BACKLIGHT2: u16 = 0x1982;
    pub const HAPTIC: u16 = 0x19B0;
    pub const FORCE_SENSING_BUTTON: u16 = 0x19C0;
    pub const REPROG_CONTROLS_V4: u16 = 0x1B04;
    pub const PERSISTENT_REMAPPABLE_ACTION: u16 = 0x1C00;
    pub const WIRELESS_DEVICE_STATUS: u16 = 0x1D4B;
    pub const SMART_SHIFT: u16 = 0x2110;
    pub const SMART_SHIFT_ENHANCED: u16 = 0x2111;
    pub const HI_RES_SCROLLING: u16 = 0x2120;
    pub const HIRES_WHEEL: u16 = 0x2121;
    pub const LOWRES_WHEEL: u16 = 0x2130;
    pub const THUMB_WHEEL: u16 = 0x2150;
    pub const ADJUSTABLE_DPI: u16 = 0x2201;
    pub const EXTENDED_ADJUSTABLE_DPI: u16 = 0x2202;
    pub const POINTER_SPEED: u16 = 0x2205;
    pub const ANGLE_SNAPPING: u16 = 0x2230;
    pub const FN_INVERSION: u16 = 0x40A0;
    pub const NEW_FN_INVERSION: u16 = 0x40A2;
    pub const K375S_FN_INVERSION: u16 = 0x40A3;
    pub const REPORT_RATE: u16 = 0x8060;
    pub const EXTENDED_REPORT_RATE: u16 = 0x8061;
    pub const COLOR_LED_EFFECTS: u16 = 0x8070;
    pub const RGB_EFFECTS: u16 = 0x8071;
    pub const ONBOARD_PROFILES: u16 = 0x8100;
}

/// Human readable name of a feature id, for diagnostics.
pub fn feature_name(feature: u16) -> &'static str {
    match feature {
        0x0000 => "Root",
        0x0001 => "Feature set",
        0x0002 => "Feature info",
        0x0003 => "Device firmware version",
        0x0004 => "Device unit id",
        0x0005 => "Device name",
        0x0006 => "Device groups",
        0x0007 => "Device friendly name",
        0x0008 => "Keep alive",
        0x0020 => "Configuration change",
        0x0021 => "Crypto id",
        0x0030 => "Target software",
        0x0080 => "Wireless signal strength",
        0x00C0 => "DFU control (legacy)",
        0x00C1 => "DFU control (unsigned)",
        0x00C2 => "DFU control (signed)",
        0x00C3 => "DFU control",
        0x00D0 => "DFU",
        0x1000 => "Battery status",
        0x1001 => "Battery voltage",
        0x1004 => "Unified battery",
        0x1010 => "Charging control",
        0x1300 => "LED control",
        0x1500 => "Force pairing",
        0x1802 => "Device reset",
        0x1805 => "OOB state",
        0x1806 => "Configurable device properties",
        0x1814 => "Change host",
        0x1815 => "Hosts info",
        0x1816 => "BLE Pro pre-pairing",
        0x1981 => "Backlight",
        0x1982 => "Backlight 2",
        0x1983 => "Backlight 3",
        0x1990 => "Illumination",
        0x19B0 => "Haptic feedback",
        0x19C0 => "Force sensing button",
        0x1A00 => "Presenter control",
        0x1B00 => "Reprogrammable controls",
        0x1B01 => "Reprogrammable controls v2",
        0x1B02 => "Reprogrammable controls v2.2",
        0x1B03 => "Reprogrammable controls v3",
        0x1B04 => "Reprogrammable controls v4",
        0x1B05 => "Full key customization",
        0x1B10 => "Control list",
        0x1C00 => "Persistent remappable action",
        0x1D4B => "Wireless device status",
        0x1DF0 => "Remaining pairing",
        0x1E00 => "Enable hidden features",
        0x1F20 => "ADC measurement",
        0x2001 => "Left/right swap",
        0x2006 => "Pointer axis orientation",
        0x2100 => "Vertical scrolling",
        0x2110 => "SmartShift",
        0x2111 => "SmartShift enhanced",
        0x2120 => "Hi-res scrolling",
        0x2121 => "Hi-res wheel",
        0x2130 => "Low-res wheel",
        0x2150 => "Thumb wheel",
        0x2200 => "Mouse pointer",
        0x2201 => "Adjustable DPI",
        0x2202 => "Extended adjustable DPI",
        0x2205 => "Pointer speed",
        0x2230 => "Angle snapping",
        0x2240 => "Surface tuning",
        0x2250 => "XY statistics",
        0x2251 => "Wheel statistics",
        0x2400 => "Hybrid tracking",
        0x40A0 => "Fn inversion",
        0x40A2 => "Fn inversion (new)",
        0x40A3 => "Fn inversion (K375s)",
        0x4100 => "Encryption",
        0x4220 => "Lock key state",
        0x4301 => "Solar dashboard",
        0x4520 => "Keyboard layout",
        0x4521 => "Disable keys",
        0x4522 => "Disable keys by usage",
        0x4530 => "Dual platform",
        0x4531 => "Multi platform",
        0x4540 => "Keyboard layout 2",
        0x4600 => "Crown",
        0x6010 => "Touchpad firmware items",
        0x6011 => "Touchpad software items",
        0x6020 => "Tap enable",
        0x6100 => "Touchpad raw XY",
        0x6500 => "Gestures",
        0x6501 => "Gestures 2",
        0x8010 => "G-keys",
        0x8020 => "M-keys",
        0x8030 => "Macro record",
        0x8040 => "Brightness control",
        0x8060 => "Report rate",
        0x8061 => "Extended report rate",
        0x8070 => "Color LED effects",
        0x8071 => "RGB effects",
        0x8080 => "Per-key lighting",
        0x8081 => "Per-key lighting v2",
        0x8090 => "Mode status",
        0x8100 => "Onboard profiles",
        0x8110 => "Mouse button spy",
        0x8111 => "Latency monitoring",
        _ => "Unknown",
    }
}
