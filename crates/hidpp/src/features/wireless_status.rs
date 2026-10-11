//! Wireless device status (feature 0x1D4B).
//!
//! Devices broadcast this event when they reconnect; when the "software
//! reconfiguration needed" flag is set, all volatile settings (diverted
//! controls, DPI, ...) have been lost and must be applied again.

use crate::report::Params;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub reconnection: bool,
    pub reconfiguration_needed: bool,
    pub powered_on: bool,
}

pub fn decode_event(function: u8, p: &Params) -> Option<Event> {
    (function == 0).then(|| Event {
        reconnection: p.u8(0) == 1,
        reconfiguration_needed: p.u8(1) == 1,
        powered_on: p.u8(2) == 1,
    })
}
