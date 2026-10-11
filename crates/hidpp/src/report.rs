//! Raw HID++ reports and helpers to build and inspect them.
//!
//! Every HID++ report starts with the same four bytes:
//!
//! | byte | HID++ 1.0          | HID++ 2.0                          |
//! |------|--------------------|------------------------------------|
//! | 0    | report id          | report id                          |
//! | 1    | device index       | device index                       |
//! | 2    | sub id             | feature index                      |
//! | 3    | address (register) | function (high nibble) + software id |
//!
//! followed by the parameters (3, 16 or 60 bytes depending on the report type).

use std::fmt;

pub const REPORT_SHORT: u8 = 0x10;
pub const REPORT_LONG: u8 = 0x11;
pub const REPORT_VERY_LONG: u8 = 0x12;
pub const REPORT_DJ_SHORT: u8 = 0x20;
pub const REPORT_DJ_LONG: u8 = 0x21;

pub const SHORT_LEN: usize = 7;
pub const LONG_LEN: usize = 20;
pub const VERY_LONG_LEN: usize = 64;
pub const DJ_SHORT_LEN: usize = 15;
pub const DJ_LONG_LEN: usize = 32;

/// Device index addressing a receiver itself, or a device connected directly over USB or Bluetooth.
pub const DEVICE_INDEX_SELF: u8 = 0xFF;

/// Sub id used by HID++ 1.0 error replies.
pub const SUB_ID_ERROR_10: u8 = 0x8F;
/// Feature index used by HID++ 2.0 error replies.
pub const FEATURE_INDEX_ERROR_20: u8 = 0xFF;

/// The kind of HID++ report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReportKind {
    Short,
    Long,
    VeryLong,
}

impl ReportKind {
    pub const fn id(self) -> u8 {
        match self {
            ReportKind::Short => REPORT_SHORT,
            ReportKind::Long => REPORT_LONG,
            ReportKind::VeryLong => REPORT_VERY_LONG,
        }
    }

    pub const fn size(self) -> usize {
        match self {
            ReportKind::Short => SHORT_LEN,
            ReportKind::Long => LONG_LEN,
            ReportKind::VeryLong => VERY_LONG_LEN,
        }
    }

    /// Number of parameter bytes carried by this report kind.
    pub const fn param_len(self) -> usize {
        self.size() - 4
    }
}

/// A single HID++ or DJ report, exactly as read from or written to hidraw.
#[derive(Clone, PartialEq, Eq)]
pub struct Report {
    buf: [u8; VERY_LONG_LEN],
    len: usize,
}

impl Report {
    /// Builds a HID++ report, zero padding the parameters.
    ///
    /// Parameters that do not fit into the report are truncated; callers are
    /// expected to pick a report kind large enough for their payload.
    pub fn new(kind: ReportKind, device_index: u8, sub_id: u8, address: u8, params: &[u8]) -> Report {
        let mut buf = [0u8; VERY_LONG_LEN];
        buf[0] = kind.id();
        buf[1] = device_index;
        buf[2] = sub_id;
        buf[3] = address;
        let n = params.len().min(kind.param_len());
        buf[4..4 + n].copy_from_slice(&params[..n]);
        Report { buf, len: kind.size() }
    }

    /// Parses a report read from hidraw. Returns `None` for reports that are
    /// neither HID++ nor DJ, or that have an unexpected length.
    pub fn parse(data: &[u8]) -> Option<Report> {
        let id = *data.first()?;
        let expected: &[usize] = match id {
            REPORT_SHORT => &[SHORT_LEN],
            REPORT_LONG => &[LONG_LEN],
            REPORT_VERY_LONG => &[VERY_LONG_LEN],
            REPORT_DJ_SHORT => &[DJ_SHORT_LEN],
            REPORT_DJ_LONG => &[DJ_LONG_LEN],
            _ => return None,
        };
        // Some receivers send short reports padded to long length and vice versa;
        // accept anything at least as long as the nominal size and trim.
        let nominal = expected[0];
        if data.len() < nominal {
            return None;
        }
        let mut buf = [0u8; VERY_LONG_LEN];
        buf[..nominal].copy_from_slice(&data[..nominal]);
        Some(Report { buf, len: nominal })
    }

    pub fn report_id(&self) -> u8 {
        self.buf[0]
    }

    pub fn device_index(&self) -> u8 {
        self.buf[1]
    }

    pub fn sub_id(&self) -> u8 {
        self.buf[2]
    }

    pub fn address(&self) -> u8 {
        self.buf[3]
    }

    /// HID++ 2.0 feature index (same byte as the HID++ 1.0 sub id).
    pub fn feature_index(&self) -> u8 {
        self.buf[2]
    }

    /// HID++ 2.0 function (or event) number.
    pub fn function(&self) -> u8 {
        self.buf[3] >> 4
    }

    /// HID++ 2.0 software id. Unsolicited events carry a software id of 0.
    pub fn software_id(&self) -> u8 {
        self.buf[3] & 0x0F
    }

    pub fn params(&self) -> &[u8] {
        &self.buf[4..self.len]
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn is_hidpp(&self) -> bool {
        matches!(self.report_id(), REPORT_SHORT | REPORT_LONG | REPORT_VERY_LONG)
    }

    pub fn is_dj(&self) -> bool {
        matches!(self.report_id(), REPORT_DJ_SHORT | REPORT_DJ_LONG)
    }

    /// True for HID++ 1.0 register replies and errors (sub ids with the top bit set).
    pub fn is_register_message(&self) -> bool {
        self.is_hidpp() && self.sub_id() & 0x80 == 0x80 && self.sub_id() != FEATURE_INDEX_ERROR_20
    }

    /// True for HID++ 1.0 notifications sent by receivers (sub ids 0x40..=0x7F).
    pub fn is_hidpp10_notification(&self) -> bool {
        self.is_hidpp() && (0x40..0x80).contains(&self.sub_id())
    }

    /// True for HID++ 2.0 feature events (unsolicited, software id 0).
    pub fn is_feature_event(&self) -> bool {
        self.is_hidpp() && self.sub_id() < 0x40 && self.software_id() == 0
    }
}

impl fmt::Debug for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = self.as_bytes();
        write!(f, "[{:02x} {:02x} {:02x} {:02x}", bytes[0], bytes[1], bytes[2], bytes[3])?;
        write!(f, " |")?;
        // Trim trailing zero padding to keep logs readable.
        let params = self.params();
        let end = params.iter().rposition(|&b| b != 0).map_or(0, |p| p + 1);
        for b in &params[..end] {
            write!(f, " {b:02x}")?;
        }
        if end < params.len() {
            write!(f, " ..")?;
        }
        write!(f, "]")
    }
}

/// The parameter bytes of a HID++ reply, with convenient accessors.
///
/// Reading past the end yields zeros, mirroring how devices zero-pad replies.
#[derive(Clone, PartialEq, Eq)]
pub struct Params {
    buf: [u8; VERY_LONG_LEN - 4],
    len: usize,
}

impl Params {
    pub fn new(bytes: &[u8]) -> Params {
        let mut buf = [0u8; VERY_LONG_LEN - 4];
        let n = bytes.len().min(buf.len());
        buf[..n].copy_from_slice(&bytes[..n]);
        Params { buf, len: n }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.buf[..self.len]
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn u8(&self, i: usize) -> u8 {
        self.buf.get(i).copied().unwrap_or(0)
    }

    pub fn i8(&self, i: usize) -> i8 {
        self.u8(i) as i8
    }

    pub fn u16(&self, i: usize) -> u16 {
        u16::from_be_bytes([self.u8(i), self.u8(i + 1)])
    }

    pub fn i16(&self, i: usize) -> i16 {
        self.u16(i) as i16
    }

    pub fn u32(&self, i: usize) -> u32 {
        u32::from_be_bytes([self.u8(i), self.u8(i + 1), self.u8(i + 2), self.u8(i + 3)])
    }

    /// Returns `len` bytes starting at `start`, clamped to the available data.
    pub fn bytes(&self, start: usize, len: usize) -> &[u8] {
        let start = start.min(self.buf.len());
        let end = (start + len).min(self.buf.len());
        &self.buf[start..end]
    }
}

impl fmt::Debug for Params {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Params(")?;
        for (i, b) in self.as_slice().iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{b:02x}")?;
        }
        write!(f, ")")
    }
}

impl From<&Report> for Params {
    fn from(report: &Report) -> Params {
        Params::new(report.params())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_parse_long() {
        let r = Report::new(ReportKind::Long, 0xFF, 0x09, 0x1E, &[0x00, 0xC3]);
        assert_eq!(r.len(), LONG_LEN);
        assert_eq!(r.as_bytes()[..6], [0x11, 0xFF, 0x09, 0x1E, 0x00, 0xC3]);
        let p = Report::parse(r.as_bytes()).unwrap();
        assert_eq!(p, r);
        assert_eq!(p.function(), 1);
        assert_eq!(p.software_id(), 0xE);
    }

    #[test]
    fn parse_rejects_unknown_and_short() {
        assert!(Report::parse(&[0x02, 0x00, 0x00]).is_none());
        assert!(Report::parse(&[0x11, 0xFF, 0x00]).is_none());
    }

    #[test]
    fn params_accessors() {
        let p = Params::new(&[0x00, 0x03, 0xe8, 0x03, 0xe8, 0xff, 0x9c]);
        assert_eq!(p.u16(1), 1000);
        assert_eq!(p.i16(5), -100);
        assert_eq!(p.u8(40), 0);
    }

    #[test]
    fn classify() {
        let event = Report::parse(&[0x11, 0xFF, 0x09, 0x00, 0x00, 0xC3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).unwrap();
        assert!(event.is_feature_event());
        let notif = Report::parse(&[0x10, 0x01, 0x41, 0x04, 0x02, 0x82, 0x40]).unwrap();
        assert!(notif.is_hidpp10_notification());
        let reg = Report::parse(&[0x10, 0xFF, 0x81, 0x00, 0x00, 0x09, 0x00]).unwrap();
        assert!(reg.is_register_message());
    }
}
