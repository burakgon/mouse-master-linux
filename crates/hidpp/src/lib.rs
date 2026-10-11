//! Logitech HID++ protocol over Linux hidraw.
//!
//! * [`hidraw`] discovers and opens hidraw nodes,
//! * [`Channel`] multiplexes requests and events on one node,
//! * [`Device`] wraps a HID++ 2.0 device and its feature table,
//! * [`features`] implements the individual HID++ 2.0 features,
//! * [`receiver`] handles Unifying / Bolt / Lightspeed receivers.

pub mod channel;
pub mod controls;
pub mod device;
pub mod error;
pub mod features;
pub mod hidraw;
pub mod receiver;
pub mod report;

pub use channel::{Channel, EventReceiver};
pub use device::{Device, FeatureEntry, FeatureTable};
pub use error::{Error, Result};
pub use report::{DEVICE_INDEX_SELF, Params, Report, ReportKind};
