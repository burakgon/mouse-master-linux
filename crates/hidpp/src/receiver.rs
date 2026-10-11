//! Logitech receivers (Unifying, Bolt, Lightspeed, Nano) and the HID++ 1.0
//! registers and notifications used to manage their paired devices.

use serde::Serialize;

use crate::channel::Channel;
use crate::error::Result;
use crate::features::device_info::DeviceKind;
use crate::report::{DEVICE_INDEX_SELF, Params, Report};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReceiverKind {
    Unifying,
    Bolt,
    Nano,
    Lightspeed,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ReceiverModel {
    pub product_id: u16,
    pub kind: ReceiverKind,
    pub name: &'static str,
    pub max_devices: u8,
}

/// Looks up a known receiver by USB vendor and product id.
pub fn receiver_model(vendor_id: u16, product_id: u16) -> Option<ReceiverModel> {
    use ReceiverKind::*;
    let (kind, name, max_devices) = match (vendor_id, product_id) {
        (0x046D, 0xC548) => (Bolt, "Bolt Receiver", 6),
        (0x046D, 0xC52B | 0xC532) => (Unifying, "Unifying Receiver", 6),
        (0x046D, 0xC52F | 0xC518 | 0xC51A | 0xC51B | 0xC521 | 0xC525 | 0xC526 | 0xC52E | 0xC531 | 0xC535 | 0xC537) => {
            (Nano, "Nano Receiver", 1)
        }
        (0x046D, 0xC534) => (Nano, "Nano Receiver", 2),
        (0x17EF, 0x6042) => (Nano, "Nano Receiver", 1),
        (0x046D, 0xC539 | 0xC53A | 0xC53D | 0xC53F | 0xC541 | 0xC545 | 0xC547 | 0xC54D) => {
            (Lightspeed, "Lightspeed Receiver", 2)
        }
        _ => return None,
    };
    Some(ReceiverModel { product_id, kind, name, max_devices })
}

mod reg {
    pub const NOTIFICATIONS: u16 = 0x00;
    pub const CONNECTION_STATE: u16 = 0x02;
    pub const RECEIVER_INFO: u16 = 0x2B5;
    pub const BOLT_UNIQUE_ID: u16 = 0x2FB;
}

/// Notification flags: report device (dis)connections, software present.
const NOTIFY_WIRELESS: u32 = 0x00_01_00;
const NOTIFY_SOFTWARE_PRESENT: u32 = 0x00_08_00;

#[derive(Debug, Clone, Serialize)]
pub struct PairedDevice {
    pub slot: u8,
    pub wireless_pid: u16,
    pub kind: DeviceKind,
    pub serial: Option<String>,
    pub codename: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Receiver {
    channel: Channel,
    model: ReceiverModel,
}

impl Receiver {
    pub fn new(channel: Channel, model: ReceiverModel) -> Receiver {
        Receiver { channel, model }
    }

    pub fn channel(&self) -> &Channel {
        &self.channel
    }

    pub fn model(&self) -> &ReceiverModel {
        &self.model
    }

    /// Asks the receiver to report device connections and disconnections.
    pub async fn enable_notifications(&self) -> Result<()> {
        let flags = NOTIFY_WIRELESS | NOTIFY_SOFTWARE_PRESENT;
        let current = self.channel.read_register(DEVICE_INDEX_SELF, reg::NOTIFICATIONS, &[]).await?;
        let current = (current.u8(0) as u32) << 16 | (current.u8(1) as u32) << 8 | current.u8(2) as u32;
        let wanted = current | flags;
        let bytes = [(wanted >> 16) as u8, (wanted >> 8) as u8, wanted as u8];
        self.channel.write_register(DEVICE_INDEX_SELF, reg::NOTIFICATIONS, &bytes).await.map(|_| ())
    }

    /// Makes the receiver send a connection notification for every paired device.
    pub async fn trigger_device_arrival(&self) -> Result<()> {
        self.channel.write_register(DEVICE_INDEX_SELF, reg::CONNECTION_STATE, &[0x02]).await.map(|_| ())
    }

    pub async fn connected_devices(&self) -> Result<u8> {
        Ok(self.channel.read_register(DEVICE_INDEX_SELF, reg::CONNECTION_STATE, &[]).await?.u8(1))
    }

    pub async fn serial(&self) -> Result<String> {
        if self.model.kind == ReceiverKind::Bolt {
            let p = self.channel.read_register(DEVICE_INDEX_SELF, reg::BOLT_UNIQUE_ID, &[]).await?;
            // Replies are zero padded; the unique id is what precedes the padding.
            let bytes = p.as_slice();
            let end = bytes.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
            Ok(hex(&bytes[..end]))
        } else {
            let p = self.channel.read_register(DEVICE_INDEX_SELF, reg::RECEIVER_INFO, &[0x03]).await?;
            Ok(hex(p.bytes(1, 4)))
        }
    }

    /// Reads what the receiver knows about the device paired in `slot` (1 based).
    pub async fn paired_device(&self, slot: u8) -> Result<PairedDevice> {
        if self.model.kind == ReceiverKind::Bolt {
            let p = self.channel.read_register(DEVICE_INDEX_SELF, reg::RECEIVER_INFO, &[0x50 + slot]).await?;
            let wireless_pid = u16::from_be_bytes([p.u8(3), p.u8(2)]);
            let kind = DeviceKind::from_hidpp10(p.u8(1));
            let serial = Some(hex(p.bytes(4, 4)));
            let codename = self
                .channel
                .read_register(DEVICE_INDEX_SELF, reg::RECEIVER_INFO, &[0x60 + slot, 0x01])
                .await
                .ok()
                .map(|n| ascii(n.bytes(3, (n.u8(2) as usize).min(14))));
            Ok(PairedDevice { slot, wireless_pid, kind, serial, codename })
        } else {
            let p = self.channel.read_register(DEVICE_INDEX_SELF, reg::RECEIVER_INFO, &[0x20 + slot - 1]).await?;
            let wireless_pid = p.u16(3);
            let kind = DeviceKind::from_hidpp10(p.u8(7));
            let serial = self
                .channel
                .read_register(DEVICE_INDEX_SELF, reg::RECEIVER_INFO, &[0x30 + slot - 1])
                .await
                .ok()
                .map(|e| hex(e.bytes(1, 4)));
            let codename = self
                .channel
                .read_register(DEVICE_INDEX_SELF, reg::RECEIVER_INFO, &[0x40 + slot - 1])
                .await
                .ok()
                .map(|n| ascii(n.bytes(2, n.u8(1) as usize)));
            Ok(PairedDevice { slot, wireless_pid, kind, serial, codename })
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

fn ascii(bytes: &[u8]) -> String {
    bytes.iter().take_while(|&&b| b != 0).map(|&b| b as char).collect()
}

/// HID++ 1.0 notifications sent by receivers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiverEvent {
    /// A paired device went on- or offline (sub id 0x41).
    Connection { slot: u8, online: bool, encrypted: bool, kind: DeviceKind, wireless_pid: u16 },
    /// A device was unpaired (sub id 0x40, address 0x02).
    Unpaired { slot: u8 },
}

pub fn decode_event(report: &Report) -> Option<ReceiverEvent> {
    if !report.is_hidpp10_notification() {
        return None;
    }
    let p = Params::from(report);
    let slot = report.device_index();
    match report.sub_id() {
        0x41 if report.address() != 0x02 => {
            let flags = p.u8(0);
            Some(ReceiverEvent::Connection {
                slot,
                online: flags & 0x40 == 0,
                encrypted: flags & 0x20 != 0 || report.address() == 0x10,
                kind: DeviceKind::from_hidpp10(flags),
                wireless_pid: u16::from_be_bytes([p.u8(2), p.u8(1)]),
            })
        }
        0x40 if report.address() == 0x02 => Some(ReceiverEvent::Unpaired { slot }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_notification() {
        // Device 1 online, mouse, wpid 0x4082.
        let r = Report::parse(&[0x10, 0x01, 0x41, 0x04, 0x02, 0x82, 0x40]).unwrap();
        assert_eq!(
            decode_event(&r),
            Some(ReceiverEvent::Connection {
                slot: 1,
                online: true,
                encrypted: false,
                kind: DeviceKind::Mouse,
                wireless_pid: 0x4082
            })
        );
        // Same device going offline (bit 6 set).
        let r = Report::parse(&[0x10, 0x01, 0x41, 0x04, 0x42, 0x82, 0x40]).unwrap();
        assert!(matches!(decode_event(&r), Some(ReceiverEvent::Connection { online: false, .. })));
    }

    #[test]
    fn known_receivers() {
        assert_eq!(receiver_model(0x046D, 0xC548).unwrap().kind, ReceiverKind::Bolt);
        assert!(receiver_model(0x046D, 0xB034).is_none());
    }
}
