//! Onboard profiles (feature 0x8100), used by gaming mice.
//!
//! While a device runs from onboard memory it ignores most host side settings,
//! so host mode is required for DPI, report rate and button changes to stick.

use serde::Serialize;

use crate::device::Device;
use crate::error::Result;
use crate::features::id;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    Onboard,
    Host,
}

pub async fn mode(dev: &Device) -> Result<Mode> {
    let m = dev.call(id::ONBOARD_PROFILES, 2, &[]).await?.u8(0);
    Ok(if m == 2 { Mode::Host } else { Mode::Onboard })
}

pub async fn set_mode(dev: &Device, mode: Mode) -> Result<()> {
    let m = match mode {
        Mode::Onboard => 1,
        Mode::Host => 2,
    };
    dev.call(id::ONBOARD_PROFILES, 1, &[m]).await.map(|_| ())
}
