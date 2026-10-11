//! Special keys and mouse buttons (feature 0x1B04, "REPROG_CONTROLS_V4").
//!
//! Controls (buttons, special keys) can be *diverted*: instead of producing
//! their native HID input, the device reports presses as HID++ events, so
//! software can attach any action to them. Controls with the raw XY capability
//! can additionally report pointer motion while held, which is what mouse
//! gestures are built on.

use bitflags::bitflags;
use serde::Serialize;

use crate::device::Device;
use crate::error::Result;
use crate::features::id;
use crate::report::Params;

bitflags! {
    /// Capabilities of a control (getCidInfo bytes 4 and 8 combined).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ControlFlags: u16 {
        const MOUSE_BUTTON = 0x0001;
        const FN_KEY = 0x0002;
        const NON_STANDARD = 0x0004;
        const FN_SENSITIVE = 0x0008;
        const REPROGRAMMABLE = 0x0010;
        const DIVERTABLE = 0x0020;
        const PERSISTENTLY_DIVERTABLE = 0x0040;
        const VIRTUAL = 0x0080;
        const RAW_XY = 0x0100;
        const FORCE_RAW_XY = 0x0200;
        const ANALYTICS_KEY_EVENTS = 0x0400;
        const RAW_WHEEL = 0x0800;
    }
}

bitflags! {
    /// Reporting state of a control (get/setCidReporting bytes 2 and 5 combined).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct ReportingFlags: u16 {
        const DIVERTED = 0x0001;
        const PERSISTENTLY_DIVERTED = 0x0004;
        const RAW_XY_DIVERTED = 0x0010;
        const FORCE_RAW_XY_DIVERTED = 0x0040;
        const ANALYTICS_KEY_EVENTS = 0x0100;
        const RAW_WHEEL = 0x0400;
    }
}

impl Serialize for ControlFlags {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u16(self.bits())
    }
}

impl Serialize for ReportingFlags {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u16(self.bits())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ControlInfo {
    /// Control id (CID), see [`crate::controls`].
    pub cid: u16,
    /// Task id of the control's native function.
    pub task: u16,
    pub flags: ControlFlags,
    /// Position of F-keys (1..16), otherwise 0.
    pub position: u8,
    pub group: u8,
    /// Groups this control can be remapped into.
    pub group_mask: u8,
}

impl ControlInfo {
    pub fn is_divertable(&self) -> bool {
        self.flags.contains(ControlFlags::DIVERTABLE)
    }

    pub fn supports_raw_xy(&self) -> bool {
        self.flags.contains(ControlFlags::RAW_XY)
    }

    pub fn is_virtual(&self) -> bool {
        self.flags.contains(ControlFlags::VIRTUAL)
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ControlReporting {
    pub cid: u16,
    pub flags: ReportingFlags,
    /// Control id this control is remapped to (its own cid when not remapped).
    pub remapped_to: u16,
}

pub async fn count(dev: &Device) -> Result<u8> {
    Ok(dev.call(id::REPROG_CONTROLS_V4, 0, &[]).await?.u8(0))
}

pub async fn control_info(dev: &Device, index: u8) -> Result<ControlInfo> {
    let p = dev.call(id::REPROG_CONTROLS_V4, 1, &[index]).await?;
    Ok(ControlInfo {
        cid: p.u16(0),
        task: p.u16(2),
        flags: ControlFlags::from_bits_retain(p.u8(4) as u16 | ((p.u8(8) as u16) << 8)),
        position: p.u8(5),
        group: p.u8(6),
        group_mask: p.u8(7),
    })
}

/// Reads every control of the device.
pub async fn controls(dev: &Device) -> Result<Vec<ControlInfo>> {
    let n = count(dev).await?;
    let mut out = Vec::with_capacity(n as usize);
    for i in 0..n {
        out.push(control_info(dev, i).await?);
    }
    Ok(out)
}

pub async fn reporting(dev: &Device, cid: u16) -> Result<ControlReporting> {
    let p = dev.call(id::REPROG_CONTROLS_V4, 2, &cid.to_be_bytes()).await?;
    let remapped = p.u16(3);
    Ok(ControlReporting {
        cid: p.u16(0),
        flags: ReportingFlags::from_bits_retain(p.u8(2) as u16 | ((p.u8(5) as u16) << 8)),
        remapped_to: if remapped == 0 { cid } else { remapped },
    })
}

/// Desired reporting changes for [`set_reporting`]. `None` leaves a flag untouched.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReportingChange {
    pub divert: Option<bool>,
    pub persist: Option<bool>,
    pub raw_xy: Option<bool>,
    pub force_raw_xy: Option<bool>,
    /// Remap target cid; `None` keeps the current mapping.
    pub remap: Option<u16>,
}

impl ReportingChange {
    pub fn divert(divert: bool, raw_xy: bool) -> ReportingChange {
        ReportingChange { divert: Some(divert), raw_xy: Some(raw_xy), ..Default::default() }
    }

    fn encode(&self, cid: u16) -> [u8; 5] {
        let mut bits = 0u8;
        let mut apply = |value: Option<bool>, flag: u8| {
            if let Some(v) = value {
                bits |= flag << 1; // the "valid" bit
                if v {
                    bits |= flag;
                }
            }
        };
        apply(self.divert, 0x01);
        apply(self.persist, 0x04);
        apply(self.raw_xy, 0x10);
        apply(self.force_raw_xy, 0x40);
        let remap = self.remap.unwrap_or(0).to_be_bytes();
        let cid = cid.to_be_bytes();
        [cid[0], cid[1], bits, remap[0], remap[1]]
    }
}

pub async fn set_reporting(dev: &Device, cid: u16, change: ReportingChange) -> Result<()> {
    // Only the first five bytes are sent: older firmware misbehaves when given
    // the v4+ extension byte, and the flags it carries are not needed here.
    dev.call(id::REPROG_CONTROLS_V4, 3, &change.encode(cid)).await.map(|_| ())
}

/// Turns analytics key events on or off for a control. The control keeps its
/// normal behaviour; presses and releases are additionally reported as
/// [`Event::Analytics`]. Requires the `ANALYTICS_KEY_EVENTS` capability.
pub async fn set_analytics(dev: &Device, cid: u16, enabled: bool) -> Result<()> {
    let c = cid.to_be_bytes();
    // Byte 2 has no valid bits set and the remap target is 0: only the v4
    // extension byte (bit 0 analytics, bit 1 its valid flag) changes.
    dev.call(id::REPROG_CONTROLS_V4, 3, &[c[0], c[1], 0, 0, 0, 0x02 | enabled as u8]).await.map(|_| ())
}

/// Events emitted by the feature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The set of diverted controls currently held down (up to four).
    DivertedButtons(Vec<u16>),
    /// Pointer motion while a raw XY diverted control is held.
    RawXy { dx: i16, dy: i16 },
    /// Analytics key event: (cid, pressed) pairs.
    Analytics(Vec<(u16, bool)>),
    /// Raw wheel movement while a control with raw wheel reporting is held.
    RawWheel { delta: i16 },
}

pub fn decode_event(function: u8, p: &Params) -> Option<Event> {
    match function {
        0 => {
            let cids = (0..4).map(|i| p.u16(i * 2)).filter(|&c| c != 0).collect();
            Some(Event::DivertedButtons(cids))
        }
        1 => Some(Event::RawXy { dx: p.i16(0), dy: p.i16(2) }),
        2 => {
            // Up to five (control id, event) entries; event bit 0 set means pressed.
            let mut events = Vec::new();
            for i in 0..5 {
                let cid = p.u16(i * 3);
                if cid != 0 {
                    events.push((cid, p.u8(i * 3 + 2) & 0x01 != 0));
                }
            }
            Some(Event::Analytics(events))
        }
        3 => Some(Event::RawWheel { delta: p.i16(1) }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_divert_with_raw_xy() {
        let change = ReportingChange::divert(true, true);
        assert_eq!(change.encode(0x00C3), [0x00, 0xC3, 0x33, 0x00, 0x00]);
        let undo = ReportingChange::divert(false, false);
        assert_eq!(undo.encode(0x00C3), [0x00, 0xC3, 0x22, 0x00, 0x00]);
    }

    #[test]
    fn decode_buttons_and_motion() {
        let p = Params::new(&[0x00, 0xC3, 0x00, 0x53, 0, 0, 0, 0]);
        assert_eq!(decode_event(0, &p), Some(Event::DivertedButtons(vec![0xC3, 0x53])));
        let p = Params::new(&[0xFF, 0xFE, 0x00, 0x05]);
        assert_eq!(decode_event(1, &p), Some(Event::RawXy { dx: -2, dy: 5 }));
    }

    #[test]
    fn real_device_flags() {
        // Gesture button of an MX Master 3S: flags 0x0531.
        let f = ControlFlags::from_bits_retain(0x0531);
        assert!(f.contains(ControlFlags::DIVERTABLE | ControlFlags::RAW_XY | ControlFlags::MOUSE_BUTTON));
        assert!(!f.contains(ControlFlags::VIRTUAL));
    }
}
