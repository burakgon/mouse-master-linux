//! Sensor resolution: adjustable DPI (0x2201) and extended adjustable DPI (0x2202).

use serde::Serialize;

use crate::device::Device;
use crate::error::{Error, Result};
use crate::features::id;

/// The DPI values a sensor accepts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DpiRange {
    /// A contiguous range with a fixed step.
    Range { min: u16, max: u16, step: u16 },
    /// An explicit list of values.
    List { values: Vec<u16> },
}

impl DpiRange {
    pub fn min(&self) -> u16 {
        match self {
            DpiRange::Range { min, .. } => *min,
            DpiRange::List { values } => values.first().copied().unwrap_or(0),
        }
    }

    pub fn max(&self) -> u16 {
        match self {
            DpiRange::Range { max, .. } => *max,
            DpiRange::List { values } => values.last().copied().unwrap_or(0),
        }
    }

    /// Snaps an arbitrary value to the closest supported one.
    pub fn snap(&self, dpi: u16) -> u16 {
        match self {
            DpiRange::Range { min, max, step } => {
                let clamped = dpi.clamp(*min, *max);
                let step = (*step).max(1);
                let steps = ((clamped - min) as u32 + step as u32 / 2) / step as u32;
                (*min as u32 + steps * step as u32).min(*max as u32) as u16
            }
            DpiRange::List { values } => {
                values.iter().copied().min_by_key(|v| (*v as i32 - dpi as i32).unsigned_abs()).unwrap_or(dpi)
            }
        }
    }

    /// Parses the packed list format shared by 0x2201 and 0x2202: big endian
    /// u16 values, where a value with the top three bits set (0xE000) is a step
    /// between the previous and the following value, terminated by 0.
    pub fn parse(bytes: &[u8]) -> Option<DpiRange> {
        let mut values = Vec::new();
        let mut range: Option<(u16, u16, u16)> = None;
        let mut i = 0;
        while i + 1 < bytes.len() {
            let v = u16::from_be_bytes([bytes[i], bytes[i + 1]]);
            if v == 0 {
                break;
            }
            if v >> 13 == 0b111 {
                let step = v & 0x1FFF;
                let lo = *values.last()?;
                let hi = u16::from_be_bytes([*bytes.get(i + 2)?, *bytes.get(i + 3)?]);
                if hi <= lo || step == 0 {
                    return None;
                }
                if values.len() == 1 && range.is_none() {
                    range = Some((lo, hi, step));
                    values.clear();
                } else {
                    let mut x = lo + step;
                    while x <= hi {
                        values.push(x);
                        x = x.checked_add(step)?;
                    }
                }
                i += 4;
            } else {
                values.push(v);
                i += 2;
            }
        }
        match (range, values.is_empty()) {
            (Some((min, max, step)), true) => Some(DpiRange::Range { min, max, step }),
            (Some((min, max, step)), false) => {
                // A range followed by extra values: expand everything.
                let mut all: Vec<u16> = (min..=max).step_by(step as usize).collect();
                all.extend(values);
                all.sort_unstable();
                all.dedup();
                Some(DpiRange::List { values: all })
            }
            (None, false) => Some(DpiRange::List { values }),
            (None, true) => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DpiInfo {
    pub feature: u16,
    pub range: DpiRange,
    pub current: u16,
    pub default: u16,
}

/// Which DPI feature the device implements, if any.
pub fn dpi_feature(dev: &Device) -> Option<u16> {
    [id::EXTENDED_ADJUSTABLE_DPI, id::ADJUSTABLE_DPI].into_iter().find(|f| dev.has(*f))
}

pub async fn read(dev: &Device) -> Result<DpiInfo> {
    match dpi_feature(dev) {
        Some(id::ADJUSTABLE_DPI) => {
            // getSensorDpiList(sensor) -> [sensor, packed list...]
            let p = dev.call(id::ADJUSTABLE_DPI, 1, &[0]).await?;
            let bytes = p.bytes(1, p.len().saturating_sub(1)).to_vec();
            let range = DpiRange::parse(&bytes).ok_or_else(|| Error::Protocol("invalid DPI list".into()))?;
            let cur = dev.call(id::ADJUSTABLE_DPI, 2, &[0]).await?;
            let (current, default) = (cur.u16(1), cur.u16(3));
            Ok(DpiInfo { feature: id::ADJUSTABLE_DPI, range, current: if current == 0 { default } else { current }, default })
        }
        Some(id::EXTENDED_ADJUSTABLE_DPI) => {
            let mut bytes = Vec::new();
            for page in 0..16u8 {
                // getSensorDpiRanges(sensor, direction X, page) -> [sensor, direction, page, list...]
                let p = dev.call(id::EXTENDED_ADJUSTABLE_DPI, 2, &[0, 0, page]).await?;
                let payload = p.bytes(3, p.len().saturating_sub(3));
                bytes.extend_from_slice(payload);
                // Lists are zero terminated and zero padded: a page ending in zeros is the last one.
                if payload.len() < 2 || payload[payload.len() - 2..] == [0, 0] {
                    break;
                }
            }
            let range = DpiRange::parse(&bytes).ok_or_else(|| Error::Protocol("invalid DPI ranges".into()))?;
            // getSensorDpiParameters -> [sensor, dpiX, defaultDpiX, dpiY, defaultDpiY, lod]
            let cur = dev.call(id::EXTENDED_ADJUSTABLE_DPI, 5, &[0]).await?;
            let (current, default) = (cur.u16(1), cur.u16(3));
            Ok(DpiInfo {
                feature: id::EXTENDED_ADJUSTABLE_DPI,
                range,
                current: if current == 0 { default } else { current },
                default,
            })
        }
        _ => Err(Error::FeatureNotSupported(id::ADJUSTABLE_DPI)),
    }
}

pub async fn set(dev: &Device, dpi: u16) -> Result<()> {
    let b = dpi.to_be_bytes();
    match dpi_feature(dev) {
        Some(id::ADJUSTABLE_DPI) => dev.call(id::ADJUSTABLE_DPI, 3, &[0, b[0], b[1]]).await.map(|_| ()),
        Some(id::EXTENDED_ADJUSTABLE_DPI) => {
            // setSensorDpiParameters(sensor, dpiX, dpiY, lod): use the same value for both axes, keep LOD.
            let cur = dev.call(id::EXTENDED_ADJUSTABLE_DPI, 5, &[0]).await?;
            let lod = cur.u8(9);
            dev.call(id::EXTENDED_ADJUSTABLE_DPI, 6, &[0, b[0], b[1], b[0], b[1], lod]).await.map(|_| ())
        }
        _ => Err(Error::FeatureNotSupported(id::ADJUSTABLE_DPI)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mx_master_3s_range() {
        // Real reply (after the sensor index byte): 200..8000 step 50.
        let r = DpiRange::parse(&[0x00, 0xc8, 0xe0, 0x32, 0x1f, 0x40, 0x00, 0x00]).unwrap();
        assert_eq!(r, DpiRange::Range { min: 200, max: 8000, step: 50 });
        assert_eq!(r.snap(1234), 1250);
        assert_eq!(r.snap(10), 200);
        assert_eq!(r.snap(9000), 8000);
    }

    #[test]
    fn explicit_list() {
        let r = DpiRange::parse(&[0x01, 0x90, 0x03, 0x20, 0x06, 0x40, 0x00, 0x00]).unwrap();
        assert_eq!(r, DpiRange::List { values: vec![400, 800, 1600] });
        assert_eq!(r.snap(1000), 800);
    }
}
