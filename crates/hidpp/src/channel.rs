//! A HID++ channel: one hidraw node shared by all devices reachable through it
//! (a receiver and its paired devices, or a single directly connected device).
//!
//! The channel owns a background reader task. Replies are matched against the
//! single in-flight request; everything else (HID++ 1.0 notifications, HID++
//! 2.0 feature events, DJ reports) is forwarded to the event receiver returned
//! by [`Channel::open`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use tracing::{debug, trace, warn};

use crate::error::{Error, Hidpp10Error, Hidpp20Error, Result};
use crate::hidraw::{Hidraw, NodeInfo, ReportSupport};
use crate::report::{FEATURE_INDEX_ERROR_20, Params, Report, ReportKind, SUB_ID_ERROR_10};

/// Software id used by Mouse Master for its requests.
///
/// Other known users: kernel 0x01, OpenRGB 0x07, LGSTrayEx 0x0A, Solaar 0x0B,
/// G HUB 0x0D, firmware 0x0F. Keeping bit 3 set distinguishes replies from
/// unsolicited events, which always use software id 0.
pub const SOFTWARE_ID: u8 = 0x0E;

pub const RECEIVER_TIMEOUT: Duration = Duration::from_millis(1500);
pub const DEVICE_TIMEOUT: Duration = Duration::from_millis(3000);

#[derive(Debug, Clone, Copy)]
enum Matcher {
    Feature { device_index: u8, feature_index: u8, address: u8 },
    Register { device_index: u8, sub_id: u8, address: u8, first_param: Option<u8> },
    Ping { device_index: u8, address: u8, mark: u8 },
}

fn same_device(expected: u8, actual: u8) -> bool {
    // Some Bluetooth devices answer requests sent to 0xFF with index 0x00.
    actual == expected || actual == expected ^ 0xFF
}

impl Matcher {
    fn device_index(&self) -> u8 {
        match *self {
            Matcher::Feature { device_index, .. }
            | Matcher::Register { device_index, .. }
            | Matcher::Ping { device_index, .. } => device_index,
        }
    }

    /// Returns `Some` if the report answers the request this matcher describes.
    fn check(&self, r: &Report) -> Option<Result<Report>> {
        if !r.is_hidpp() || !same_device(self.device_index(), r.device_index()) {
            return None;
        }
        let p = r.params();
        match *self {
            Matcher::Feature { feature_index, address, .. } => {
                if r.sub_id() == FEATURE_INDEX_ERROR_20 && r.address() == feature_index && p[0] == address {
                    return Some(Err(Error::Hidpp20(Hidpp20Error::from(p[1]))));
                }
                if r.sub_id() == SUB_ID_ERROR_10 && r.address() == feature_index && p[0] == address {
                    return Some(Err(map_hidpp10_error(p[1])));
                }
                if r.sub_id() == feature_index && r.address() == address {
                    return Some(Ok(r.clone()));
                }
                None
            }
            Matcher::Register { sub_id, address, first_param, .. } => {
                if r.sub_id() == SUB_ID_ERROR_10 && r.address() == sub_id && p[0] == address {
                    return Some(Err(map_hidpp10_error(p[1])));
                }
                if r.sub_id() == sub_id && r.address() == address && first_param.is_none_or(|f| p[0] == f) {
                    return Some(Ok(r.clone()));
                }
                None
            }
            Matcher::Ping { address, mark, .. } => {
                if r.sub_id() == 0x00 && r.address() == address && p[2] == mark {
                    return Some(Ok(r.clone()));
                }
                if r.sub_id() == SUB_ID_ERROR_10 && r.address() == 0x00 && p[0] == address {
                    return Some(Err(Error::Hidpp10(Hidpp10Error::from(p[1]))));
                }
                if r.sub_id() == FEATURE_INDEX_ERROR_20 && r.address() == 0x00 && p[0] == address {
                    return Some(Err(Error::Hidpp20(Hidpp20Error::from(p[1]))));
                }
                None
            }
        }
    }
}

fn map_hidpp10_error(code: u8) -> Error {
    match Hidpp10Error::from(code) {
        Hidpp10Error::ResourceError | Hidpp10Error::ConnectionRequestFailed => Error::Unreachable,
        other => Error::Hidpp10(other),
    }
}

struct Pending {
    matcher: Matcher,
    tx: oneshot::Sender<Result<Report>>,
}

struct Inner {
    hidraw: Hidraw,
    info: NodeInfo,
    request_lock: tokio::sync::Mutex<()>,
    pending: Mutex<Option<Pending>>,
    closed: AtomicBool,
}

impl Inner {
    fn complete(&self, report: &Report) -> bool {
        let mut slot = self.pending.lock().unwrap();
        let Some(pending) = slot.as_ref() else { return false };
        match pending.matcher.check(report) {
            Some(result) => {
                let pending = slot.take().expect("pending request");
                let _ = pending.tx.send(result);
                true
            }
            None => false,
        }
    }

    fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        if let Some(pending) = self.pending.lock().unwrap().take() {
            let _ = pending.tx.send(Err(Error::Disconnected));
        }
    }
}

/// Events delivered by a channel's reader task.
pub type EventReceiver = mpsc::UnboundedReceiver<Report>;

/// A cheaply clonable handle to an open hidraw node speaking HID++.
#[derive(Clone)]
pub struct Channel {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Channel").field("node", &self.inner.info.devnode).finish()
    }
}

impl Channel {
    /// Opens the node and spawns its reader task. Must be called within a tokio runtime.
    pub fn open(info: NodeInfo) -> Result<(Channel, EventReceiver)> {
        let hidraw = Hidraw::open(&info.devnode)?;
        let inner = Arc::new(Inner {
            hidraw,
            info,
            request_lock: tokio::sync::Mutex::new(()),
            pending: Mutex::new(None),
            closed: AtomicBool::new(false),
        });
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(reader_task(inner.clone(), tx));
        Ok((Channel { inner }, rx))
    }

    pub fn info(&self) -> &NodeInfo {
        &self.inner.info
    }

    pub fn reports(&self) -> ReportSupport {
        self.inner.info.reports
    }

    pub fn is_closed(&self) -> bool {
        self.inner.closed.load(Ordering::SeqCst)
    }

    /// Picks the smallest report type the node supports that fits `param_len` bytes.
    fn pick_kind(&self, param_len: usize, force_long: bool) -> ReportKind {
        let s = self.reports();
        if !force_long && param_len <= ReportKind::Short.param_len() && s.short {
            ReportKind::Short
        } else if param_len <= ReportKind::Long.param_len() && s.long {
            ReportKind::Long
        } else if s.very_long {
            ReportKind::VeryLong
        } else if s.long {
            ReportKind::Long
        } else {
            ReportKind::Short
        }
    }

    async fn transact(&self, report: Report, matcher: Matcher, timeout: Duration) -> Result<Report> {
        let _guard = self.inner.request_lock.lock().await;
        if self.is_closed() {
            return Err(Error::Disconnected);
        }
        let (tx, rx) = oneshot::channel();
        *self.inner.pending.lock().unwrap() = Some(Pending { matcher, tx });
        trace!(node = ?self.inner.info.devnode, "-> {report:?}");
        if let Err(e) = self.inner.hidraw.write(report.as_bytes()).await {
            self.inner.pending.lock().unwrap().take();
            if e.raw_os_error() == Some(libc::ENODEV) {
                self.inner.close();
                return Err(Error::Disconnected);
            }
            return Err(e.into());
        }
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(Error::Disconnected),
            Err(_) => {
                self.inner.pending.lock().unwrap().take();
                Err(Error::Timeout { device_index: report.device_index(), timeout_ms: timeout.as_millis() as u64 })
            }
        }
    }

    /// Sends a report without waiting for a reply.
    pub async fn send(&self, report: Report) -> Result<()> {
        let _guard = self.inner.request_lock.lock().await;
        if self.is_closed() {
            return Err(Error::Disconnected);
        }
        trace!(node = ?self.inner.info.devnode, "-> {report:?} (no reply)");
        self.inner.hidraw.write(report.as_bytes()).await.map_err(Error::from)
    }

    fn default_timeout(device_index: u8, is_receiver_register: bool) -> Duration {
        if is_receiver_register && device_index == 0xFF { RECEIVER_TIMEOUT } else { DEVICE_TIMEOUT }
    }

    /// Calls a HID++ 2.0 feature function and returns the reply parameters.
    pub async fn feature_call(&self, device_index: u8, feature_index: u8, function: u8, params: &[u8]) -> Result<Params> {
        let address = (function << 4) | SOFTWARE_ID;
        let kind = self.pick_kind(params.len(), false);
        let report = Report::new(kind, device_index, feature_index, address, params);
        let matcher = Matcher::Feature { device_index, feature_index, address };
        let reply = self.transact(report, matcher, Self::default_timeout(device_index, false)).await?;
        Ok(Params::from(&reply))
    }

    /// Calls a HID++ 2.0 feature function without waiting for the reply.
    pub async fn feature_send(&self, device_index: u8, feature_index: u8, function: u8, params: &[u8]) -> Result<()> {
        let address = (function << 4) | SOFTWARE_ID;
        let kind = self.pick_kind(params.len(), false);
        self.send(Report::new(kind, device_index, feature_index, address, params)).await
    }

    /// Reads a HID++ 1.0 register. Registers above 0xFF are "long" registers.
    pub async fn read_register(&self, device_index: u8, register: u16, params: &[u8]) -> Result<Params> {
        let long = register > 0xFF;
        let sub_id = if long { 0x83 } else { 0x81 };
        let address = (register & 0xFF) as u8;
        let kind = self.pick_kind(params.len(), false);
        let report = Report::new(kind, device_index, sub_id, address, params);
        // Replies to the multi purpose info registers must also echo the sub register.
        let first_param = if (long && address == 0xB5) || (!long && address == 0xF1) { params.first().copied() } else { None };
        let matcher = Matcher::Register { device_index, sub_id, address, first_param };
        let mut timeout = Self::default_timeout(device_index, true);
        if long {
            timeout *= 2;
        }
        let reply = self.transact(report, matcher, timeout).await?;
        Ok(Params::from(&reply))
    }

    /// Writes a HID++ 1.0 register.
    pub async fn write_register(&self, device_index: u8, register: u16, params: &[u8]) -> Result<Params> {
        let long = register > 0xFF || params.len() > ReportKind::Short.param_len();
        let sub_id = if long { 0x82 } else { 0x80 };
        let address = (register & 0xFF) as u8;
        let kind = self.pick_kind(params.len(), long);
        let report = Report::new(kind, device_index, sub_id, address, params);
        let matcher = Matcher::Register { device_index, sub_id, address, first_param: None };
        let reply = self.transact(report, matcher, Self::default_timeout(device_index, true)).await?;
        Ok(Params::from(&reply))
    }

    /// Pings a device. Returns its HID++ protocol version as `(major, minor)`;
    /// HID++ 1.0 devices report `(1, 0)`.
    pub async fn ping(&self, device_index: u8) -> Result<(u8, u8)> {
        let mark = ping_mark();
        let address = (1 << 4) | SOFTWARE_ID; // root feature, function 1: getProtocolVersion
        let kind = self.pick_kind(3, false);
        let report = Report::new(kind, device_index, 0x00, address, &[0, 0, mark]);
        let matcher = Matcher::Ping { device_index, address, mark };
        match self.transact(report, matcher, Self::default_timeout(device_index, false)).await {
            Ok(reply) => Ok((reply.params()[0], reply.params()[1])),
            Err(Error::Hidpp10(Hidpp10Error::InvalidSubId)) => Ok((1, 0)),
            Err(Error::Hidpp10(Hidpp10Error::UnknownDevice)) => Err(Error::Unreachable),
            Err(e) => Err(e),
        }
    }
}

fn ping_mark() -> u8 {
    use std::sync::atomic::AtomicU8;
    static COUNTER: AtomicU8 = AtomicU8::new(0x5A);
    COUNTER.fetch_add(0x3D, Ordering::Relaxed) | 1
}

async fn reader_task(inner: Arc<Inner>, events: mpsc::UnboundedSender<Report>) {
    let mut buf = [0u8; 64];
    loop {
        match inner.hidraw.read(&mut buf).await {
            Ok(0) => continue,
            Ok(n) => {
                let Some(report) = Report::parse(&buf[..n]) else { continue };
                if inner.complete(&report) {
                    trace!(node = ?inner.info.devnode, "<- {report:?} (reply)");
                    continue;
                }
                if report.is_dj() {
                    // DJ input reports (type < 0x10) are just regular input.
                    if report.sub_id() < 0x10 {
                        continue;
                    }
                } else if report.is_register_message() {
                    // Replies to somebody else's register requests.
                    continue;
                } else if report.is_hidpp() && report.sub_id() < 0x40 && report.software_id() != 0 {
                    // Replies to other software's feature calls (Solaar, the kernel, ...).
                    continue;
                }
                trace!(node = ?inner.info.devnode, "<- {report:?} (event)");
                if events.send(report).is_err() {
                    debug!(node = ?inner.info.devnode, "event receiver dropped; stopping reader");
                    break;
                }
            }
            Err(e) => {
                match e.raw_os_error() {
                    Some(libc::ENODEV) | Some(libc::EIO) => debug!(node = ?inner.info.devnode, "hidraw node removed"),
                    _ => warn!(node = ?inner.info.devnode, "hidraw read failed: {e}"),
                }
                break;
            }
        }
    }
    inner.close();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn long(bytes: &[u8]) -> Report {
        let mut buf = [0u8; 20];
        buf[..bytes.len()].copy_from_slice(bytes);
        Report::parse(&buf).unwrap()
    }

    #[test]
    fn feature_reply_matching() {
        let m = Matcher::Feature { device_index: 0xFF, feature_index: 0x09, address: 0x1E };
        assert!(m.check(&long(&[0x11, 0xFF, 0x09, 0x1E, 0x00, 0x50])).unwrap().is_ok());
        // Bluetooth quirk: device index 0x00 in replies.
        assert!(m.check(&long(&[0x11, 0x00, 0x09, 0x1E, 0x00, 0x50])).unwrap().is_ok());
        // Other software id.
        assert!(m.check(&long(&[0x11, 0xFF, 0x09, 0x1B, 0x00, 0x50])).is_none());
        // Event with the same feature index.
        assert!(m.check(&long(&[0x11, 0xFF, 0x09, 0x00, 0x00, 0xC3])).is_none());
        // HID++ 2.0 error.
        match m.check(&long(&[0x11, 0xFF, 0xFF, 0x09, 0x1E, 0x02])).unwrap() {
            Err(Error::Hidpp20(Hidpp20Error::InvalidArgument)) => {}
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn hidpp10_error_on_feature_call_means_unreachable() {
        let m = Matcher::Feature { device_index: 0x01, feature_index: 0x05, address: 0x0E };
        let short = Report::parse(&[0x10, 0x01, 0x8F, 0x05, 0x0E, 0x09, 0x00]).unwrap();
        assert!(matches!(m.check(&short), Some(Err(Error::Unreachable))));
    }

    #[test]
    fn register_reply_must_echo_subregister() {
        let m = Matcher::Register { device_index: 0xFF, sub_id: 0x83, address: 0xB5, first_param: Some(0x20) };
        assert!(m.check(&long(&[0x11, 0xFF, 0x83, 0xB5, 0x21])).is_none());
        assert!(m.check(&long(&[0x11, 0xFF, 0x83, 0xB5, 0x20])).unwrap().is_ok());
    }

    #[test]
    fn ping_matching() {
        let m = Matcher::Ping { device_index: 0x02, address: 0x1E, mark: 0x77 };
        assert!(m.check(&long(&[0x11, 0x02, 0x00, 0x1E, 0x04, 0x05, 0x77])).unwrap().is_ok());
        assert!(m.check(&long(&[0x11, 0x02, 0x00, 0x1E, 0x04, 0x05, 0x78])).is_none());
        let old = Report::parse(&[0x10, 0x02, 0x8F, 0x00, 0x1E, 0x01, 0x00]).unwrap();
        assert!(matches!(m.check(&old), Some(Err(Error::Hidpp10(Hidpp10Error::InvalidSubId)))));
    }
}
