//! The horizontal thumb wheel of MX mice (feature 0x2150).

use serde::Serialize;

use crate::device::Device;
use crate::error::Result;
use crate::features::id;
use crate::report::Params;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ThumbwheelInfo {
    /// Increments per full turn reported as native HID scrolling.
    pub native_resolution: u16,
    /// Increments per full turn reported when diverted.
    pub diverted_resolution: u16,
    /// +1 when a positive rotation means "right", -1 otherwise.
    pub default_direction: i8,
    pub has_timestamp: bool,
    pub has_touch: bool,
    pub has_proximity: bool,
    pub has_single_tap: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ThumbwheelStatus {
    pub diverted: bool,
    pub inverted: bool,
    pub touched: bool,
    pub proximity: bool,
}

pub async fn info(dev: &Device) -> Result<ThumbwheelInfo> {
    let p = dev.call(id::THUMB_WHEEL, 0, &[]).await?;
    let caps = p.u8(5);
    Ok(ThumbwheelInfo {
        native_resolution: p.u16(0),
        diverted_resolution: p.u16(2).max(1),
        default_direction: if p.u8(4) != 0 { 1 } else { -1 },
        has_timestamp: caps & 0x01 != 0,
        has_touch: caps & 0x02 != 0,
        has_proximity: caps & 0x04 != 0,
        has_single_tap: caps & 0x08 != 0,
    })
}

pub async fn status(dev: &Device) -> Result<ThumbwheelStatus> {
    let p = dev.call(id::THUMB_WHEEL, 1, &[]).await?;
    Ok(ThumbwheelStatus {
        diverted: p.u8(0) & 0x01 != 0,
        inverted: p.u8(1) & 0x01 != 0,
        touched: p.u8(1) & 0x02 != 0,
        proximity: p.u8(1) & 0x04 != 0,
    })
}

pub async fn set_reporting(dev: &Device, diverted: bool, inverted: bool) -> Result<()> {
    dev.call(id::THUMB_WHEEL, 2, &[diverted as u8, inverted as u8]).await.map(|_| ())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RotationStatus {
    Inactive,
    Start,
    Active,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    /// Rotation in diverted resolution units, in the device's native direction.
    pub rotation: i16,
    pub timestamp: u16,
    pub status: RotationStatus,
    pub touch: bool,
    pub proximity: bool,
    pub single_tap: bool,
}

pub fn decode_event(function: u8, p: &Params) -> Option<Event> {
    if function != 0 {
        return None;
    }
    let status = match p.u8(4) {
        1 => RotationStatus::Start,
        2 => RotationStatus::Active,
        3 => RotationStatus::Stop,
        _ => RotationStatus::Inactive,
    };
    let flags = p.u8(5);
    Some(Event {
        rotation: p.i16(0),
        timestamp: p.u16(2),
        status,
        touch: flags & 0x02 != 0,
        proximity: flags & 0x04 != 0,
        single_tap: flags & 0x08 != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mx_master_3s_info_layout() {
        let p = Params::new(&[0x00, 0x12, 0x00, 0x78, 0x00, 0x0f, 0x03, 0xe8]);
        // Decode the same way `info` does.
        assert_eq!(p.u16(0), 18);
        assert_eq!(p.u16(2), 120);
        assert_eq!(p.u8(5) & 0x08, 0x08);
    }

    #[test]
    fn event_decoding() {
        let e = decode_event(0, &Params::new(&[0xff, 0xf6, 0x01, 0x02, 0x02, 0x02])).unwrap();
        assert_eq!(e.rotation, -10);
        assert_eq!(e.status, RotationStatus::Active);
        assert!(e.touch);
        assert!(!e.single_tap);
    }
}
