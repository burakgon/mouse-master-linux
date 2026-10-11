//! Easy-Switch host selection (feature 0x1814).

use serde::Serialize;

use crate::device::Device;
use crate::error::Result;
use crate::features::id;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct HostInfo {
    pub count: u8,
    /// Zero based index of the host the device is currently connected to.
    pub current: u8,
}

pub async fn info(dev: &Device) -> Result<HostInfo> {
    let reply = dev.call(id::CHANGE_HOST, 0, &[]).await?;
    Ok(HostInfo { count: reply.u8(0), current: reply.u8(1) })
}

/// Switches the device to another host. The device disconnects immediately,
/// so no reply is awaited.
pub async fn switch_to(dev: &Device, host: u8) -> Result<()> {
    dev.send(id::CHANGE_HOST, 1, &[host]).await
}
