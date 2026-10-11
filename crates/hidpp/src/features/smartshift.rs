//! SmartShift: the electromagnetic ratchet of MX scroll wheels.
//! Feature 0x2110 (SmartShift) and 0x2111 (SmartShift enhanced, adds tunable torque).

use serde::Serialize;

use crate::device::Device;
use crate::error::{Error, Result};
use crate::features::id;

/// Auto disengage value meaning "never switch to free spin automatically".
pub const ALWAYS_RATCHET: u8 = 0xFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WheelMode {
    FreeSpin,
    Ratchet,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SmartShift {
    pub feature: u16,
    pub mode: WheelMode,
    /// Scroll speed above which the wheel disengages into free spin
    /// (1..=254), or [`ALWAYS_RATCHET`] when SmartShift is off.
    pub auto_disengage: u8,
    pub default_auto_disengage: u8,
    /// Ratchet force in percent (0x2111 with tunable torque only).
    pub torque: Option<u8>,
    pub default_torque: Option<u8>,
}

impl SmartShift {
    pub fn enabled(&self) -> bool {
        self.auto_disengage != ALWAYS_RATCHET && self.auto_disengage != 0
    }
}

pub fn smartshift_feature(dev: &Device) -> Option<u16> {
    [id::SMART_SHIFT_ENHANCED, id::SMART_SHIFT].into_iter().find(|f| dev.has(*f))
}

fn mode_from(b: u8) -> WheelMode {
    if b == 1 { WheelMode::FreeSpin } else { WheelMode::Ratchet }
}

pub async fn read(dev: &Device) -> Result<SmartShift> {
    match smartshift_feature(dev) {
        Some(id::SMART_SHIFT) => {
            let p = dev.call(id::SMART_SHIFT, 0, &[]).await?;
            Ok(SmartShift {
                feature: id::SMART_SHIFT,
                mode: mode_from(p.u8(0)),
                auto_disengage: p.u8(1),
                default_auto_disengage: p.u8(2),
                torque: None,
                default_torque: None,
            })
        }
        Some(id::SMART_SHIFT_ENHANCED) => {
            let caps = dev.call(id::SMART_SHIFT_ENHANCED, 0, &[]).await?;
            let tunable = caps.u8(0) & 0x01 != 0;
            let p = dev.call(id::SMART_SHIFT_ENHANCED, 1, &[]).await?;
            Ok(SmartShift {
                feature: id::SMART_SHIFT_ENHANCED,
                mode: mode_from(p.u8(0)),
                auto_disengage: p.u8(1),
                default_auto_disengage: caps.u8(1),
                torque: tunable.then(|| p.u8(2)),
                default_torque: tunable.then(|| caps.u8(2)),
            })
        }
        _ => Err(Error::FeatureNotSupported(id::SMART_SHIFT)),
    }
}

/// Changes the wheel configuration. `None` leaves a value unchanged.
pub async fn write(
    dev: &Device,
    mode: Option<WheelMode>,
    auto_disengage: Option<u8>,
    torque: Option<u8>,
) -> Result<()> {
    let mode_byte = match mode {
        None => 0,
        Some(WheelMode::FreeSpin) => 1,
        Some(WheelMode::Ratchet) => 2,
    };
    let disengage = auto_disengage.unwrap_or(0);
    match smartshift_feature(dev) {
        Some(id::SMART_SHIFT) => dev.call(id::SMART_SHIFT, 1, &[mode_byte, disengage, 0]).await.map(|_| ()),
        Some(id::SMART_SHIFT_ENHANCED) => {
            // Some devices (MX Master 4) only apply a new torque when the mode is
            // written along with it, so always send the current mode.
            let mode_byte = if mode_byte == 0 && torque.is_some() {
                let cur = dev.call(id::SMART_SHIFT_ENHANCED, 1, &[]).await?;
                cur.u8(0)
            } else {
                mode_byte
            };
            dev.call(id::SMART_SHIFT_ENHANCED, 2, &[mode_byte, disengage, torque.unwrap_or(0)]).await.map(|_| ())
        }
        _ => Err(Error::FeatureNotSupported(id::SMART_SHIFT)),
    }
}
