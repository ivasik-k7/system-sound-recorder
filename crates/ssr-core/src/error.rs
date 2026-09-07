use std::fmt;

/// Errors raised by the capture engine.
#[derive(Debug)]
pub enum Error {
    /// A COM or WASAPI call failed.
    Windows(windows::core::Error),
    /// No device matched the requested selector.
    NoSuchDevice(String),
    /// No running process matched the requested selector.
    NoSuchProcess(String),
    /// The engine was asked for something the platform cannot do.
    Unsupported(String),
    /// Activation of a process-loopback client timed out.
    ActivationTimeout,
    /// A capture thread stopped unexpectedly.
    CaptureFailed(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Windows(e) => write!(f, "windows error: {e}"),
            Error::NoSuchDevice(s) => write!(f, "no audio device matched {s:?}"),
            Error::NoSuchProcess(s) => write!(f, "no running process matched {s:?}"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::ActivationTimeout => {
                write!(f, "timed out activating a process-loopback audio client")
            }
            Error::CaptureFailed(s) => write!(f, "capture failed: {s}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Windows(e) => Some(e),
            _ => None,
        }
    }
}

impl From<windows::core::Error> for Error {
    fn from(e: windows::core::Error) -> Self {
        Error::Windows(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
