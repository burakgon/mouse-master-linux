//! Asynchronous access to Linux `/dev/hidraw*` nodes and udev based discovery.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tokio::io::unix::AsyncFd;

pub const LOGITECH_VENDOR_ID: u16 = 0x046D;
/// Lenovo branded Logitech nano receiver.
pub const LENOVO_VENDOR_ID: u16 = 0x17EF;

/// The physical bus a hidraw node is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Bus {
    Usb,
    Bluetooth,
    Other(u16),
}

impl Bus {
    pub fn from_hid_bus(bus: u16) -> Bus {
        match bus {
            0x0003 => Bus::Usb,
            0x0005 => Bus::Bluetooth,
            other => Bus::Other(other),
        }
    }
}

/// Which HID++ related reports a hidraw node declares in its report descriptor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ReportSupport {
    pub short: bool,
    pub long: bool,
    pub very_long: bool,
    pub dj: bool,
}

impl ReportSupport {
    pub fn is_hidpp(&self) -> bool {
        self.short || self.long || self.very_long
    }

    /// Parses a HID report descriptor and records the vendor specific report
    /// ids used by HID++ (0x10, 0x11, 0x12) and DJ (0x20, 0x21).
    pub fn from_descriptor(desc: &[u8]) -> ReportSupport {
        let mut support = ReportSupport::default();
        let mut usage_page: u32 = 0;
        let mut stack: Vec<u32> = Vec::new();
        let mut i = 0usize;
        while i < desc.len() {
            let prefix = desc[i];
            if prefix == 0xFE {
                // Long item: 0xFE, size, tag, data...
                let Some(&size) = desc.get(i + 1) else { break };
                i += 3 + size as usize;
                continue;
            }
            let size = match prefix & 0x03 {
                0 => 0,
                1 => 1,
                2 => 2,
                _ => 4,
            };
            if i + 1 + size > desc.len() {
                break;
            }
            let mut value: u32 = 0;
            for (shift, byte) in desc[i + 1..i + 1 + size].iter().enumerate() {
                value |= (*byte as u32) << (8 * shift);
            }
            let item_type = (prefix >> 2) & 0x03;
            let tag = prefix >> 4;
            if item_type == 1 {
                match tag {
                    0x0 => usage_page = value,
                    0x8 => {
                        if usage_page >= 0xFF00 {
                            match value {
                                0x10 => support.short = true,
                                0x11 => support.long = true,
                                0x12 => support.very_long = true,
                                0x20 | 0x21 => support.dj = true,
                                _ => {}
                            }
                        }
                    }
                    0xA => stack.push(usage_page),
                    0xB => usage_page = stack.pop().unwrap_or(usage_page),
                    _ => {}
                }
            }
            i += 1 + size;
        }
        support
    }
}

/// Static information about a hidraw node, gathered from udev and sysfs.
#[derive(Debug, Clone, Serialize)]
pub struct NodeInfo {
    pub devnode: PathBuf,
    pub syspath: PathBuf,
    pub bus: Bus,
    pub vendor_id: u16,
    pub product_id: u16,
    pub name: String,
    /// Bluetooth address or USB serial, when available.
    pub uniq: String,
    pub reports: ReportSupport,
    /// True for the per-device nodes the kernel's `hid-logitech-dj` driver
    /// creates below a receiver. Those devices are managed through the
    /// receiver node instead.
    pub is_dj_child: bool,
}

impl NodeInfo {
    pub fn is_logitech(&self) -> bool {
        self.vendor_id == LOGITECH_VENDOR_ID
            || (self.vendor_id == LENOVO_VENDOR_ID && self.product_id == 0x6042)
    }

    /// Reads node information for a hidraw device from udev.
    pub fn from_udev(dev: &udev::Device) -> Option<NodeInfo> {
        let devnode = dev.devnode()?.to_path_buf();
        let hid = dev.parent_with_subsystem("hid").ok()??;
        let hid_id = hid.property_value("HID_ID")?.to_str()?;
        let (bus, vendor_id, product_id) = parse_hid_id(hid_id)?;
        let name = hid
            .property_value("HID_NAME")
            .and_then(|v| v.to_str())
            .unwrap_or_default()
            .to_string();
        let uniq = hid
            .property_value("HID_UNIQ")
            .and_then(|v| v.to_str())
            .unwrap_or_default()
            .to_string();
        let is_dj_child = hid
            .parent()
            .and_then(|p| p.subsystem().map(|s| s == "hid"))
            .unwrap_or(false);
        let descriptor = std::fs::read(hid.syspath().join("report_descriptor")).unwrap_or_default();
        Some(NodeInfo {
            devnode,
            syspath: dev.syspath().to_path_buf(),
            bus: Bus::from_hid_bus(bus),
            vendor_id,
            product_id,
            name,
            uniq,
            reports: ReportSupport::from_descriptor(&descriptor),
            is_dj_child,
        })
    }
}

/// Parses udev's `HID_ID` property, e.g. `0005:0000046D:0000B034`.
pub fn parse_hid_id(value: &str) -> Option<(u16, u16, u16)> {
    let mut parts = value.split(':');
    let bus = u32::from_str_radix(parts.next()?, 16).ok()?;
    let vendor = u32::from_str_radix(parts.next()?, 16).ok()?;
    let product = u32::from_str_radix(parts.next()?, 16).ok()?;
    Some((bus as u16, vendor as u16, product as u16))
}

/// Lists all hidraw nodes that look like Logitech HID++ interfaces.
pub fn enumerate() -> io::Result<Vec<NodeInfo>> {
    let mut enumerator = udev::Enumerator::new()?;
    enumerator.match_subsystem("hidraw")?;
    let mut nodes = Vec::new();
    for dev in enumerator.scan_devices()? {
        if let Some(info) = NodeInfo::from_udev(&dev) {
            if info.is_logitech() && info.reports.is_hidpp() && !info.is_dj_child {
                nodes.push(info);
            }
        }
    }
    nodes.sort_by(|a, b| a.devnode.cmp(&b.devnode));
    Ok(nodes)
}

/// A non-blocking hidraw file registered with the tokio reactor.
pub struct Hidraw {
    fd: AsyncFd<File>,
    path: PathBuf,
}

impl Hidraw {
    pub fn open(path: &Path) -> io::Result<Hidraw> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(path)?;
        Ok(Hidraw { fd: AsyncFd::new(file)?, path: path.to_path_buf() })
    }

    /// Wraps an already open descriptor, e.g. one end of a `SOCK_SEQPACKET`
    /// socket pair connected to a simulated device. It must be non-blocking.
    pub fn from_fd(fd: std::os::fd::OwnedFd, path: &Path) -> io::Result<Hidraw> {
        Ok(Hidraw { fd: AsyncFd::new(File::from(fd))?, path: path.to_path_buf() })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads a single report.
    pub async fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let mut guard = self.fd.readable().await?;
            match guard.try_io(|inner| {
                let mut file: &File = inner.get_ref();
                file.read(buf)
            }) {
                Ok(result) => return result,
                Err(_would_block) => continue,
            }
        }
    }

    /// Writes a single report.
    pub async fn write(&self, data: &[u8]) -> io::Result<()> {
        loop {
            let mut guard = self.fd.writable().await?;
            match guard.try_io(|inner| {
                let mut file: &File = inner.get_ref();
                file.write(data)
            }) {
                Ok(Ok(n)) if n == data.len() => return Ok(()),
                Ok(Ok(n)) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        format!("short hidraw write ({n} of {} bytes)", data.len()),
                    ));
                }
                Ok(Err(e)) => return Err(e),
                Err(_would_block) => continue,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Report descriptor of an MX Master 3S connected over Bluetooth.
    const MX_MASTER_3S_BT: &[u8] = &[
        0x05, 0x01, 0x09, 0x02, 0xa1, 0x01, 0x85, 0x02, 0x09, 0x01, 0xa1, 0x00, 0x95, 0x10, 0x75, 0x01, 0x15, 0x00,
        0x25, 0x01, 0x05, 0x09, 0x19, 0x01, 0x29, 0x10, 0x81, 0x02, 0x05, 0x01, 0x16, 0x01, 0xf8, 0x26, 0xff, 0x07,
        0x75, 0x0c, 0x95, 0x02, 0x09, 0x30, 0x09, 0x31, 0x81, 0x06, 0x15, 0x81, 0x25, 0x7f, 0x75, 0x08, 0x95, 0x01,
        0x09, 0x38, 0x81, 0x06, 0x95, 0x01, 0x05, 0x0c, 0x0a, 0x38, 0x02, 0x81, 0x06, 0xc0, 0xc0, 0x06, 0x43, 0xff,
        0x0a, 0x02, 0x02, 0xa1, 0x01, 0x85, 0x11, 0x75, 0x08, 0x95, 0x13, 0x15, 0x00, 0x26, 0xff, 0x00, 0x09, 0x02,
        0x81, 0x00, 0x09, 0x02, 0x91, 0x00, 0xc0,
    ];

    #[test]
    fn bluetooth_descriptor_is_long_only() {
        let s = ReportSupport::from_descriptor(MX_MASTER_3S_BT);
        assert!(s.long);
        assert!(!s.short);
        assert!(!s.very_long);
        assert!(!s.dj);
        assert!(s.is_hidpp());
    }

    #[test]
    fn mouse_report_id_is_not_hidpp() {
        // Report id 0x10 under the generic desktop page must not count as HID++.
        let desc = [0x05, 0x01, 0x09, 0x02, 0xa1, 0x01, 0x85, 0x10, 0xc0];
        assert!(!ReportSupport::from_descriptor(&desc).is_hidpp());
    }

    #[test]
    fn hid_id_parsing() {
        assert_eq!(parse_hid_id("0005:0000046D:0000B034"), Some((5, 0x046D, 0xB034)));
        assert_eq!(parse_hid_id("garbage"), None);
    }
}
