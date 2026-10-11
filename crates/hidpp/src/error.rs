use std::io;

use thiserror::Error;

/// HID++ 1.0 error codes (returned by receivers and legacy devices).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hidpp10Error {
    InvalidSubId,
    InvalidAddress,
    InvalidValue,
    ConnectionRequestFailed,
    TooManyDevices,
    AlreadyExists,
    Busy,
    UnknownDevice,
    ResourceError,
    RequestUnavailable,
    UnsupportedParameterValue,
    WrongPinCode,
    Other(u8),
}

impl From<u8> for Hidpp10Error {
    fn from(code: u8) -> Self {
        match code {
            0x01 => Self::InvalidSubId,
            0x02 => Self::InvalidAddress,
            0x03 => Self::InvalidValue,
            0x04 => Self::ConnectionRequestFailed,
            0x05 => Self::TooManyDevices,
            0x06 => Self::AlreadyExists,
            0x07 => Self::Busy,
            0x08 => Self::UnknownDevice,
            0x09 => Self::ResourceError,
            0x0A => Self::RequestUnavailable,
            0x0B => Self::UnsupportedParameterValue,
            0x0C => Self::WrongPinCode,
            other => Self::Other(other),
        }
    }
}

/// HID++ 2.0 error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hidpp20Error {
    Unknown,
    InvalidArgument,
    OutOfRange,
    HardwareError,
    LogitechInternal,
    InvalidFeatureIndex,
    InvalidFunction,
    Busy,
    Unsupported,
    Other(u8),
}

impl From<u8> for Hidpp20Error {
    fn from(code: u8) -> Self {
        match code {
            0x01 => Self::Unknown,
            0x02 => Self::InvalidArgument,
            0x03 => Self::OutOfRange,
            0x04 => Self::HardwareError,
            0x05 => Self::LogitechInternal,
            0x06 => Self::InvalidFeatureIndex,
            0x07 => Self::InvalidFunction,
            0x08 => Self::Busy,
            0x09 => Self::Unsupported,
            other => Self::Other(other),
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("the hidraw device was disconnected")]
    Disconnected,

    #[error("no reply from device {device_index:#04x} within {timeout_ms} ms")]
    Timeout { device_index: u8, timeout_ms: u64 },

    #[error("device is not reachable (asleep, switched off or out of range)")]
    Unreachable,

    #[error("HID++ 1.0 error: {0:?}")]
    Hidpp10(Hidpp10Error),

    #[error("HID++ 2.0 error: {0:?}")]
    Hidpp20(Hidpp20Error),

    #[error("feature {0:#06x} is not supported by this device")]
    FeatureNotSupported(u16),

    #[error("protocol error: {0}")]
    Protocol(String),
}

impl Error {
    /// Errors that mean the device is (temporarily) gone, as opposed to a
    /// request being invalid.
    pub fn is_offline(&self) -> bool {
        matches!(
            self,
            Error::Disconnected
                | Error::Unreachable
                | Error::Timeout { .. }
                | Error::Hidpp10(Hidpp10Error::ResourceError)
                | Error::Hidpp10(Hidpp10Error::UnknownDevice)
                | Error::Hidpp10(Hidpp10Error::ConnectionRequestFailed)
        )
    }

    pub fn is_busy(&self) -> bool {
        matches!(self, Error::Hidpp20(Hidpp20Error::Busy) | Error::Hidpp10(Hidpp10Error::Busy))
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
