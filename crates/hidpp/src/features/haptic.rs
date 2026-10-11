//! Haptic feedback motor (feature 0x19B0), e.g. on the MX Master 4.

use serde::Serialize;

use crate::device::Device;
use crate::error::Result;
use crate::features::id;

/// Waveforms a device may support, as advertised in its capability bitmap.
pub const WAVEFORMS: &[(u8, &str)] = &[
    (0x00, "Sharp state change"),
    (0x01, "Damp state change"),
    (0x02, "Sharp collision"),
    (0x03, "Damp collision"),
    (0x04, "Subtle collision"),
    (0x05, "Happy alert"),
    (0x06, "Angry alert"),
    (0x07, "Completed"),
    (0x08, "Square"),
    (0x09, "Wave"),
    (0x0A, "Firework"),
    (0x0B, "Mad"),
    (0x0C, "Knock"),
    (0x0D, "Jingle"),
    (0x0E, "Ringing"),
    (0x1B, "Whisper collision"),
];

#[derive(Debug, Clone, Serialize)]
pub struct HapticConfig {
    pub enabled: bool,
    /// Intensity in percent.
    pub level: u8,
    /// The device only supports four discrete levels (25/50/75/100).
    pub discrete_levels: bool,
    /// Supported waveform ids.
    pub waveforms: Vec<u8>,
}

pub async fn read(dev: &Device) -> Result<HapticConfig> {
    let caps = dev.call(id::HAPTIC, 0, &[]).await?;
    let bitmap = caps.u32(4);
    let waveforms = WAVEFORMS.iter().map(|(id, _)| *id).filter(|id| *id < 32 && bitmap & (1 << id) != 0).collect();
    let cfg = dev.call(id::HAPTIC, 1, &[]).await?;
    Ok(HapticConfig {
        enabled: cfg.u8(0) & 0x01 != 0,
        level: cfg.u8(1).min(100),
        discrete_levels: cfg.u8(2) & 0x01 != 0,
        waveforms,
    })
}

pub async fn set(dev: &Device, enabled: bool, level: u8) -> Result<()> {
    let level = if enabled { level.clamp(1, 100) } else { level.min(100) };
    dev.call(id::HAPTIC, 2, &[enabled as u8, level]).await.map(|_| ())
}

pub async fn play(dev: &Device, waveform: u8) -> Result<()> {
    dev.call(id::HAPTIC, 4, &[waveform]).await.map(|_| ())
}
