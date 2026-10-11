//! Device identity: firmware/ids (0x0003), name and type (0x0005) and the
//! user editable friendly name (0x0007).

use serde::Serialize;

use crate::device::Device;
use crate::error::Result;
use crate::features::id;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceKind {
    Keyboard,
    RemoteControl,
    Numpad,
    Mouse,
    Touchpad,
    Trackball,
    Presenter,
    Receiver,
    Headset,
    Webcam,
    SteeringWheel,
    Joystick,
    Gamepad,
    Dock,
    Speaker,
    Microphone,
    Unknown,
}

impl DeviceKind {
    /// Decodes the HID++ 2.0 device type (feature 0x0005, function 2).
    pub fn from_hidpp20(t: u8) -> DeviceKind {
        match t {
            0 => DeviceKind::Keyboard,
            1 => DeviceKind::RemoteControl,
            2 => DeviceKind::Numpad,
            3 => DeviceKind::Mouse,
            4 => DeviceKind::Touchpad,
            5 => DeviceKind::Trackball,
            6 => DeviceKind::Presenter,
            7 => DeviceKind::Receiver,
            8 => DeviceKind::Headset,
            9 => DeviceKind::Webcam,
            10 => DeviceKind::SteeringWheel,
            11 => DeviceKind::Joystick,
            12 => DeviceKind::Gamepad,
            13 => DeviceKind::Dock,
            14 => DeviceKind::Speaker,
            15 => DeviceKind::Microphone,
            _ => DeviceKind::Unknown,
        }
    }

    /// Decodes the HID++ 1.0 device kind used in receiver notifications and pairing registers.
    pub fn from_hidpp10(kind: u8) -> DeviceKind {
        match kind & 0x0F {
            0x01 => DeviceKind::Keyboard,
            0x02 => DeviceKind::Mouse,
            0x03 => DeviceKind::Numpad,
            0x04 => DeviceKind::Presenter,
            0x07 => DeviceKind::RemoteControl,
            0x08 => DeviceKind::Trackball,
            0x09 => DeviceKind::Touchpad,
            0x0B => DeviceKind::Gamepad,
            0x0C => DeviceKind::Joystick,
            0x0D => DeviceKind::Headset,
            _ => DeviceKind::Unknown,
        }
    }

    pub fn is_pointer(self) -> bool {
        matches!(self, DeviceKind::Mouse | DeviceKind::Trackball | DeviceKind::Touchpad)
    }
}

/// Transport bits reported by getDeviceInfo.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ModelIds {
    pub bluetooth: Option<u16>,
    pub ble: Option<u16>,
    /// Wireless product id used by Unifying / Bolt / Lightspeed receivers.
    pub wireless: Option<u16>,
    pub usb: Option<u16>,
}

impl ModelIds {
    /// The most specific id to identify the model with.
    pub fn primary(&self) -> Option<u16> {
        self.ble.or(self.bluetooth).or(self.wireless).or(self.usb)
    }

    pub fn all(&self) -> Vec<u16> {
        [self.ble, self.bluetooth, self.wireless, self.usb].into_iter().flatten().collect()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FirmwareEntity {
    /// 0 main application, 1 bootloader, 2 hardware, 3 touchpad, 4 optical sensor, 5 softdevice, ...
    pub kind: u8,
    pub name: String,
    pub version: String,
    pub build: u16,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceIdentity {
    pub unit_id: String,
    pub model_ids: ModelIds,
    pub extended_model_id: u8,
    pub serial_supported: bool,
    pub firmware: Vec<FirmwareEntity>,
}

/// Reads unit id, model ids and firmware versions (feature 0x0003).
pub async fn identity(dev: &Device) -> Result<DeviceIdentity> {
    let info = dev.call(id::DEVICE_FW_VERSION, 0, &[]).await?;
    let count = info.u8(0);
    let unit_id = format!("{:08X}", info.u32(1));
    let transport = info.u16(5);
    let mut model_ids = ModelIds::default();
    let mut offset = 7;
    // Bits: 0 Bluetooth, 1 BLE, 2 eQuad (receiver), 3 USB.
    for (bit, slot) in [
        (0x01u16, &mut model_ids.bluetooth),
        (0x02, &mut model_ids.ble),
        (0x04, &mut model_ids.wireless),
        (0x08, &mut model_ids.usb),
    ] {
        if transport & bit != 0 {
            let pid = info.u16(offset);
            if pid != 0 {
                *slot = Some(pid);
            }
            offset += 2;
        }
    }
    let extended_model_id = info.u8(13);
    let serial_supported = info.u8(14) & 0x01 != 0;
    let mut firmware = Vec::new();
    for i in 0..count {
        let fw = dev.call(id::DEVICE_FW_VERSION, 1, &[i]).await?;
        let kind = fw.u8(0) & 0x0F;
        let name: String = fw.bytes(1, 3).iter().filter(|b| b.is_ascii_graphic()).map(|&b| b as char).collect();
        let (major, minor, build) = (fw.u8(4), fw.u8(5), fw.u16(6));
        // Versions are BCD encoded, so hexadecimal formatting yields the decimal digits.
        let version = match kind {
            2 => format!("{}", fw.u8(1)),
            _ if build != 0 => format!("{major:02X}.{minor:02X}.B{build:04X}"),
            _ => format!("{major:02X}.{minor:02X}"),
        };
        firmware.push(FirmwareEntity { kind, name, version, build, active: fw.u8(8) & 0x01 != 0 });
    }
    Ok(DeviceIdentity { unit_id, model_ids, extended_model_id, serial_supported, firmware })
}

/// Reads the serial number when the device supports it (feature 0x0003 v4+).
pub async fn serial_number(dev: &Device) -> Result<String> {
    let reply = dev.call(id::DEVICE_FW_VERSION, 2, &[]).await?;
    Ok(reply.bytes(0, 12).iter().filter(|b| b.is_ascii_graphic()).map(|&b| b as char).collect())
}

/// Reads the marketing name (feature 0x0005).
pub async fn name(dev: &Device) -> Result<String> {
    let len = dev.call(id::DEVICE_NAME, 0, &[]).await?.u8(0) as usize;
    let mut bytes = Vec::with_capacity(len);
    while bytes.len() < len {
        let chunk = dev.call(id::DEVICE_NAME, 1, &[bytes.len() as u8]).await?;
        let take = (len - bytes.len()).min(chunk.len());
        if take == 0 {
            break;
        }
        bytes.extend_from_slice(chunk.bytes(0, take));
    }
    Ok(String::from_utf8_lossy(&bytes).trim_end_matches('\0').trim().to_string())
}

/// Reads the device type (feature 0x0005).
pub async fn kind(dev: &Device) -> Result<DeviceKind> {
    Ok(DeviceKind::from_hidpp20(dev.call(id::DEVICE_NAME, 2, &[]).await?.u8(0)))
}

#[derive(Debug, Clone, Serialize)]
pub struct FriendlyName {
    pub name: String,
    pub max_len: u8,
}

/// Reads the friendly (user editable, Bluetooth advertised) name (feature 0x0007).
pub async fn friendly_name(dev: &Device) -> Result<FriendlyName> {
    let info = dev.call(id::DEVICE_FRIENDLY_NAME, 0, &[]).await?;
    let (len, max_len) = (info.u8(0) as usize, info.u8(1));
    let mut bytes = Vec::with_capacity(len);
    while bytes.len() < len {
        let chunk = dev.call(id::DEVICE_FRIENDLY_NAME, 1, &[bytes.len() as u8]).await?;
        // Replies echo the offset in byte 0.
        let take = (len - bytes.len()).min(chunk.len().saturating_sub(1));
        if take == 0 {
            break;
        }
        bytes.extend_from_slice(chunk.bytes(1, take));
    }
    Ok(FriendlyName { name: String::from_utf8_lossy(&bytes).trim_end_matches('\0').to_string(), max_len })
}
