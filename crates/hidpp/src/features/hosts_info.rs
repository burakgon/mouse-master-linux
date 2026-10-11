//! Information about the Easy-Switch hosts a device is paired with (feature 0x1815).

use serde::Serialize;

use crate::device::Device;
use crate::error::Result;
use crate::features::id;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HostBus {
    Unknown,
    Receiver,
    Usb,
    Bluetooth,
    BluetoothLe,
}

impl HostBus {
    fn from_byte(b: u8) -> HostBus {
        match b {
            1 => HostBus::Receiver,
            2 => HostBus::Usb,
            3 => HostBus::Receiver,
            4 => HostBus::Bluetooth,
            5 => HostBus::BluetoothLe,
            _ => HostBus::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Host {
    pub index: u8,
    pub paired: bool,
    pub bus: HostBus,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hosts {
    pub current: u8,
    pub hosts: Vec<Host>,
    pub can_read_names: bool,
    pub can_write_names: bool,
}

pub async fn read(dev: &Device) -> Result<Hosts> {
    let info = dev.call(id::HOSTS_INFO, 0, &[]).await?;
    let capabilities = info.u8(0);
    let count = info.u8(2);
    let current = info.u8(3);
    let can_read_names = capabilities & 0x01 != 0;
    let can_write_names = capabilities & 0x02 != 0;
    let mut hosts = Vec::with_capacity(count as usize);
    for host in 0..count {
        let h = dev.call(id::HOSTS_INFO, 1, &[host]).await?;
        let paired = h.u8(1) != 0;
        let bus = HostBus::from_byte(h.u8(2));
        let name_len = h.u8(4) as usize;
        let mut name = Vec::new();
        if can_read_names && paired {
            while name.len() < name_len {
                let chunk = dev.call(id::HOSTS_INFO, 3, &[host, name.len() as u8]).await?;
                // Reply: [host, offset, up to 14 bytes of name].
                let take = (name_len - name.len()).min(14);
                if take == 0 {
                    break;
                }
                name.extend_from_slice(chunk.bytes(2, take));
            }
        }
        let name = String::from_utf8_lossy(&name).trim_end_matches('\0').to_string();
        hosts.push(Host { index: host, paired, bus, name });
    }
    Ok(Hosts { current, hosts, can_read_names, can_write_names })
}

/// Sets the name the device stores for one of its hosts.
pub async fn set_host_name(dev: &Device, host: u8, name: &str) -> Result<()> {
    let h = dev.call(id::HOSTS_INFO, 1, &[host]).await?;
    let max_len = h.u8(5).max(1) as usize;
    let mut bytes = name.as_bytes().to_vec();
    bytes.truncate(max_len);
    let mut offset = 0usize;
    while offset < bytes.len() {
        let end = (offset + 14).min(bytes.len());
        let mut params = vec![host, offset as u8];
        params.extend_from_slice(&bytes[offset..end]);
        dev.call(id::HOSTS_INFO, 4, &params).await?;
        offset = end;
    }
    Ok(())
}
