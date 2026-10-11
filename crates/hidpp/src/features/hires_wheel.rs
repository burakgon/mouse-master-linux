//! High resolution scroll wheel (feature 0x2121).

use serde::Serialize;

use crate::device::Device;
use crate::error::Result;
use crate::features::id;
use crate::report::Params;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct WheelCapability {
    /// Number of high resolution steps per ratchet notch.
    pub multiplier: u8,
    pub has_ratchet_switch: bool,
    pub has_invert: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct WheelMode {
    /// Report wheel movement as HID++ events instead of HID input.
    pub diverted: bool,
    /// High resolution (smooth) reporting.
    pub hires: bool,
    /// Invert the scroll direction ("natural" scrolling) in the device itself.
    pub inverted: bool,
}

impl WheelMode {
    fn from_byte(b: u8) -> WheelMode {
        WheelMode { diverted: b & 0x01 != 0, hires: b & 0x02 != 0, inverted: b & 0x04 != 0 }
    }

    fn to_byte(self) -> u8 {
        (self.diverted as u8) | ((self.hires as u8) << 1) | ((self.inverted as u8) << 2)
    }
}

pub async fn capability(dev: &Device) -> Result<WheelCapability> {
    let p = dev.call(id::HIRES_WHEEL, 0, &[]).await?;
    Ok(WheelCapability {
        multiplier: p.u8(0).max(1),
        has_ratchet_switch: p.u8(1) & 0x04 != 0,
        has_invert: p.u8(1) & 0x08 != 0,
    })
}

pub async fn mode(dev: &Device) -> Result<WheelMode> {
    Ok(WheelMode::from_byte(dev.call(id::HIRES_WHEEL, 1, &[]).await?.u8(0)))
}

/// Sets the wheel mode. Recent kernels watch this reply to keep their own
/// high resolution multiplier in sync, so changing `hires` is safe there.
pub async fn set_mode(dev: &Device, mode: WheelMode) -> Result<WheelMode> {
    Ok(WheelMode::from_byte(dev.call(id::HIRES_WHEEL, 2, &[mode.to_byte()]).await?.u8(0)))
}

/// True when the ratchet is engaged (as opposed to free spinning).
pub async fn ratchet_engaged(dev: &Device) -> Result<bool> {
    Ok(dev.call(id::HIRES_WHEEL, 3, &[]).await?.u8(0) & 0x01 != 0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Wheel movement while diverted.
    Movement { hires: bool, periods: u8, delta: i16 },
    /// The ratchet switched (true = engaged).
    Ratchet(bool),
}

pub fn decode_event(function: u8, p: &Params) -> Option<Event> {
    match function {
        0 => Some(Event::Movement { hires: p.u8(0) & 0x10 != 0, periods: p.u8(0) & 0x0F, delta: p.i16(1) }),
        1 if p.u8(0) < 2 => Some(Event::Ratchet(p.u8(0) == 1)),
        _ => None,
    }
}
