//! A thin, sound wrapper around a Win32 auto-reset event.
//!
//! WASAPI wakes capture threads through an event handle, and the engine needs a
//! second event to interrupt that wait when a recording is stopped. Both are
//! this type.

use std::sync::Arc;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::System::Threading::{CreateEventW, SetEvent, WaitForMultipleObjects};

use crate::error::Result;

struct Inner(HANDLE);

// The handle is only ever used with SetEvent and WaitForMultipleObjects, both of
// which are thread-safe, and it is closed exactly once when the last Arc drops.
unsafe impl Send for Inner {}
unsafe impl Sync for Inner {}

impl Drop for Inner {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

/// A cloneable handle to an auto-reset event.
#[derive(Clone)]
pub struct Event(Arc<Inner>);

impl Event {
    /// Create an auto-reset event that starts unsignalled.
    pub fn new() -> Result<Self> {
        let handle = unsafe { CreateEventW(None, false, false, None)? };
        Ok(Event(Arc::new(Inner(handle))))
    }

    /// Raw handle, for passing to `IAudioClient::SetEventHandle`.
    pub fn raw(&self) -> HANDLE {
        self.0 .0
    }

    /// Signal the event, releasing one waiter.
    pub fn set(&self) {
        unsafe {
            let _ = SetEvent(self.0 .0);
        }
    }
}

/// Which of the two events a [`wait_either`] call woke on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Woken {
    /// The first event, conventionally the stop signal.
    First,
    /// The second event, conventionally the audio buffer signal.
    Second,
    /// Neither event fired before the deadline.
    Timeout,
    /// The wait failed or was abandoned; the caller should shut down.
    Failed,
}

/// Block until either event is signalled or `timeout_ms` elapses.
///
/// `first` is checked with priority by `WaitForMultipleObjects`, so passing the
/// stop event first means a stop is never starved by a busy audio stream.
///
/// The timeout matters more than it looks. A loopback stream on an idle endpoint
/// raises no events at all, so a wait without a deadline would park the capture
/// thread until sound resumed and the recording would lose the silence entirely.
pub fn wait_either(first: &Event, second: &Event, timeout_ms: u32) -> Woken {
    let handles = [first.raw(), second.raw()];
    let r = unsafe { WaitForMultipleObjects(&handles, false, timeout_ms) };
    if r == WAIT_OBJECT_0 {
        Woken::First
    } else if r.0 == WAIT_OBJECT_0.0 + 1 {
        Woken::Second
    } else if r == WAIT_TIMEOUT {
        Woken::Timeout
    } else {
        Woken::Failed
    }
}
