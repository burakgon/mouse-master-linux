//! Firmware pointer speed multiplier (feature 0x2205). 256 means 1.0.

use crate::device::Device;
use crate::error::Result;
use crate::features::id;

pub const MIN: u16 = 0x002E;
pub const MAX: u16 = 0x01FF;

pub async fn read(dev: &Device) -> Result<u16> {
    Ok(dev.call(id::POINTER_SPEED, 0, &[]).await?.u16(0))
}

pub async fn set(dev: &Device, speed: u16) -> Result<()> {
    let b = speed.clamp(MIN, MAX).to_be_bytes();
    dev.call(id::POINTER_SPEED, 1, &b).await.map(|_| ())
}
