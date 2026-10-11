//! Polling rate: report rate (0x8060) and extended report rate (0x8061).

use serde::Serialize;

use crate::device::Device;
use crate::error::{Error, Result};
use crate::features::id;

#[derive(Debug, Clone, Serialize)]
pub struct ReportRate {
    pub feature: u16,
    /// Supported report intervals in microseconds.
    pub supported_us: Vec<u32>,
    pub current_us: u32,
}

/// Rates of the extended feature, indexed by their wire value.
const EXTENDED_RATES_US: [u32; 7] = [8000, 4000, 2000, 1000, 500, 250, 125];

pub fn report_rate_feature(dev: &Device) -> Option<u16> {
    [id::EXTENDED_REPORT_RATE, id::REPORT_RATE].into_iter().find(|f| dev.has(*f))
}

pub async fn read(dev: &Device) -> Result<ReportRate> {
    match report_rate_feature(dev) {
        Some(id::REPORT_RATE) => {
            let flags = dev.call(id::REPORT_RATE, 0, &[]).await?.u8(0);
            let supported_us = (0..8).filter(|i| flags & (1 << i) != 0).map(|i| (i as u32 + 1) * 1000).collect();
            let current_us = dev.call(id::REPORT_RATE, 1, &[]).await?.u8(0) as u32 * 1000;
            Ok(ReportRate { feature: id::REPORT_RATE, supported_us, current_us })
        }
        Some(id::EXTENDED_REPORT_RATE) => {
            let flags = dev.call(id::EXTENDED_REPORT_RATE, 1, &[]).await?.u16(0);
            let supported_us = (0..7).filter(|i| flags & (1 << i) != 0).map(|i| EXTENDED_RATES_US[i]).collect();
            let idx = dev.call(id::EXTENDED_REPORT_RATE, 2, &[]).await?.u8(0) as usize;
            Ok(ReportRate {
                feature: id::EXTENDED_REPORT_RATE,
                supported_us,
                current_us: EXTENDED_RATES_US.get(idx).copied().unwrap_or(1000),
            })
        }
        _ => Err(Error::FeatureNotSupported(id::REPORT_RATE)),
    }
}

pub async fn set(dev: &Device, interval_us: u32) -> Result<()> {
    match report_rate_feature(dev) {
        Some(id::REPORT_RATE) => {
            let ms = (interval_us / 1000).clamp(1, 8) as u8;
            dev.call(id::REPORT_RATE, 2, &[ms]).await.map(|_| ())
        }
        Some(id::EXTENDED_REPORT_RATE) => {
            let idx = EXTENDED_RATES_US
                .iter()
                .position(|&r| r == interval_us)
                .ok_or_else(|| Error::Protocol(format!("unsupported report interval {interval_us} µs")))?;
            dev.call(id::EXTENDED_REPORT_RATE, 3, &[idx as u8]).await.map(|_| ())
        }
        _ => Err(Error::FeatureNotSupported(id::REPORT_RATE)),
    }
}
