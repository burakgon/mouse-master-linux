//! Battery reporting: unified battery (0x1004), battery status (0x1000) and
//! battery voltage (0x1001).

use serde::Serialize;

use crate::device::Device;
use crate::error::{Error, Result};
use crate::features::id;
use crate::report::Params;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChargeState {
    Discharging,
    Charging,
    ChargingSlow,
    Full,
    Error,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BatteryLevel {
    Critical,
    Low,
    Good,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Battery {
    /// State of charge in percent, when the device reports it.
    pub percent: Option<u8>,
    pub level: Option<BatteryLevel>,
    pub state: ChargeState,
    pub voltage_mv: Option<u16>,
    pub external_power: bool,
}

impl Battery {
    pub fn is_charging(&self) -> bool {
        matches!(self.state, ChargeState::Charging | ChargeState::ChargingSlow)
    }

    /// Best effort percentage, falling back to the coarse level.
    pub fn approximate_percent(&self) -> Option<u8> {
        self.percent.or(match self.level? {
            BatteryLevel::Critical => Some(5),
            BatteryLevel::Low => Some(20),
            BatteryLevel::Good => Some(60),
            BatteryLevel::Full => Some(95),
        })
    }
}

/// The battery feature a device implements, in order of preference.
pub fn battery_feature(dev: &Device) -> Option<u16> {
    [id::UNIFIED_BATTERY, id::BATTERY_STATUS, id::BATTERY_VOLTAGE].into_iter().find(|f| dev.has(*f))
}

pub async fn read(dev: &Device) -> Result<Battery> {
    match battery_feature(dev) {
        Some(id::UNIFIED_BATTERY) => Ok(decode_unified(&dev.call(id::UNIFIED_BATTERY, 1, &[]).await?)),
        Some(id::BATTERY_STATUS) => Ok(decode_status(&dev.call(id::BATTERY_STATUS, 0, &[]).await?)),
        Some(id::BATTERY_VOLTAGE) => Ok(decode_voltage(&dev.call(id::BATTERY_VOLTAGE, 0, &[]).await?)),
        _ => Err(Error::FeatureNotSupported(id::UNIFIED_BATTERY)),
    }
}

/// Decodes a battery event (function 0 of any of the battery features).
pub fn decode_event(feature: u16, params: &Params) -> Option<Battery> {
    match feature {
        id::UNIFIED_BATTERY => Some(decode_unified(params)),
        id::BATTERY_STATUS => Some(decode_status(params)),
        id::BATTERY_VOLTAGE => Some(decode_voltage(params)),
        _ => None,
    }
}

/// 0x1004 getStatus / event: [state of charge %, level bits, charging status, external power].
pub fn decode_unified(p: &Params) -> Battery {
    let level = match p.u8(1) {
        l if l & 0x08 != 0 => Some(BatteryLevel::Full),
        l if l & 0x04 != 0 => Some(BatteryLevel::Good),
        l if l & 0x02 != 0 => Some(BatteryLevel::Low),
        l if l & 0x01 != 0 => Some(BatteryLevel::Critical),
        _ => None,
    };
    let state = match p.u8(2) {
        0 => ChargeState::Discharging,
        1 => ChargeState::Charging,
        2 => ChargeState::ChargingSlow,
        3 => ChargeState::Full,
        4 => ChargeState::Error,
        _ => ChargeState::Unknown,
    };
    let percent = match p.u8(0) {
        0 => None,
        v => Some(v.min(100)),
    };
    Battery { percent, level, state, voltage_mv: None, external_power: p.u8(3) != 0 }
}

/// 0x1000 getBatteryLevelStatus / event: [level %, next level %, status].
pub fn decode_status(p: &Params) -> Battery {
    let state = match p.u8(2) {
        0 => ChargeState::Discharging,
        1 => ChargeState::Charging,
        2 => ChargeState::Charging, // almost full
        3 => ChargeState::Full,
        4 => ChargeState::ChargingSlow,
        5 | 6 => ChargeState::Error,
        _ => ChargeState::Unknown,
    };
    let percent = match p.u8(0) {
        0 => None,
        v => Some(v.min(100)),
    };
    Battery {
        percent,
        level: None,
        state,
        voltage_mv: None,
        external_power: matches!(state, ChargeState::Charging | ChargeState::ChargingSlow | ChargeState::Full),
    }
}

/// 0x1001 getBatteryInfo / event: [voltage mV (2), flags].
pub fn decode_voltage(p: &Params) -> Battery {
    let voltage = p.u16(0);
    let flags = p.u8(2);
    let charging = flags & 0x80 != 0;
    let state = if charging {
        match flags & 0x07 {
            1 => ChargeState::Full,
            0 if flags & 0x10 != 0 => ChargeState::ChargingSlow,
            0 => ChargeState::Charging,
            _ => ChargeState::Error,
        }
    } else {
        ChargeState::Discharging
    };
    Battery {
        percent: Some(voltage_to_percent(voltage)),
        level: None,
        state,
        voltage_mv: Some(voltage),
        external_power: charging,
    }
}

/// Approximates the state of charge of a single cell Li-ion battery.
pub fn voltage_to_percent(mv: u16) -> u8 {
    const CURVE: [(u16, u8); 11] = [
        (4186, 100),
        (4067, 90),
        (3989, 80),
        (3922, 70),
        (3859, 60),
        (3811, 50),
        (3778, 40),
        (3751, 30),
        (3717, 20),
        (3671, 10),
        (3646, 5),
    ];
    if mv >= CURVE[0].0 {
        return 100;
    }
    for pair in CURVE.windows(2) {
        let (hi_v, hi_p) = pair[0];
        let (lo_v, lo_p) = pair[1];
        if mv >= lo_v {
            let span = (hi_v - lo_v) as u32;
            let pos = (mv - lo_v) as u32;
            return lo_p + ((hi_p - lo_p) as u32 * pos / span) as u8;
        }
    }
    if mv > 3500 { 2 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unified_from_real_device() {
        // MX Master 3S: 65 %, level "full", discharging.
        let b = decode_unified(&Params::new(&[0x41, 0x08, 0x00, 0x00]));
        assert_eq!(b.percent, Some(65));
        assert_eq!(b.level, Some(BatteryLevel::Full));
        assert_eq!(b.state, ChargeState::Discharging);
        assert!(!b.is_charging());
    }

    #[test]
    fn voltage_curve() {
        assert_eq!(voltage_to_percent(4200), 100);
        assert_eq!(voltage_to_percent(3811), 50);
        assert!(voltage_to_percent(3830) > 50 && voltage_to_percent(3830) < 60);
        assert_eq!(voltage_to_percent(3000), 0);
    }
}
