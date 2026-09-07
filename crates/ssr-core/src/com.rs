//! COM apartment management.
//!
//! Every capture thread initialises COM for itself and uninitialises on the way
//! out. `Apartment` makes that pairing hard to get wrong.

use windows::Win32::System::Com::{
    CoInitializeEx, CoUninitialize, COINIT_DISABLE_OLE1DDE, COINIT_MULTITHREADED,
};

/// RAII guard for a multithreaded COM apartment on the current thread.
pub struct Apartment {
    /// True when this guard performed the initialisation and therefore owns the
    /// matching `CoUninitialize`.
    owns: bool,
}

impl Apartment {
    /// Join the multithreaded apartment on this thread.
    ///
    /// If the thread is already in a compatible apartment this is a no-op and no
    /// `CoUninitialize` will be issued on drop.
    pub fn new() -> Self {
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED | COINIT_DISABLE_OLE1DDE) };
        // S_FALSE means the apartment already existed; we still hold a reference
        // and must balance it. Only a hard failure means we own nothing.
        Apartment { owns: hr.is_ok() }
    }
}

impl Default for Apartment {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        if self.owns {
            unsafe { CoUninitialize() };
        }
    }
}
