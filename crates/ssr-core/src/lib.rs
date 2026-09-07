//! WASAPI capture engine for the system sound recorder.
//!
//! Three kinds of source are supported, all of them producing the same
//! [`Packet`] stream so the timeline layer can treat them uniformly:
//!
//! * **Endpoint loopback** — everything mixed into one render endpoint. This is
//!   the safety net: whatever reaches the speakers is recorded, including Citrix
//!   HDX session audio and WSLg audio arriving over RDP.
//! * **Process loopback** — the audio of one process tree, isolated. This is what
//!   lets a Citrix session, a browser and a chat client land in separate files.
//! * **Input capture** — microphones and other capture endpoints.

#![cfg(windows)]

pub mod catalog;
pub mod channels;
pub mod com;
pub mod device;
pub mod disk;
pub mod error;
pub mod event;
pub mod format;
pub mod loopback;
pub mod packet;
pub mod process;
pub mod procfind;
pub mod sessions;
pub mod source;
pub mod trace;

pub use catalog::{Category, CATALOG};
pub use channels::{channels_of, layout_name, ChannelInfo};
pub use device::{
    describe_format, endpoint_format, DeviceInfo, DeviceKind, Devices, EndpointFormat,
};
pub use disk::{space_for, DiskSpace};
pub use error::{Error, Result};
pub use format::AudioFormat;
pub use packet::{Packet, PacketFlags};
pub use process::ProcessMode;
pub use procfind::{known_targets, list_processes, resolve_pid, ProcessInfo};
pub use sessions::{
    discover_once, AudioSessionInfo, DiscoveredSource, Discoverer, Discovery, SessionMonitor,
    SessionState,
};
pub use source::{start, CaptureStats, SourceHandle, SourceSpec};

/// The current local date and time as `YYYY-MM-DD_HH-MM-SS`.
///
/// Local rather than UTC because this names folders that people read. A
/// recording made at half past one in the afternoon should be in a folder that
/// says so. Timestamps inside the manifest stay in UTC, where being unambiguous
/// matters more than being familiar.
pub fn local_timestamp() -> String {
    use windows::Win32::System::SystemInformation::GetLocalTime;
    let t = unsafe { GetLocalTime() };
    format!(
        "{:04}-{:02}-{:02}_{:02}-{:02}-{:02}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
    )
}

/// Frequency of the Windows performance counter, in ticks per second.
///
/// WASAPI reports buffer positions in QPC units of 100 ns regardless of this
/// value, but the session clock is read straight from `QueryPerformanceCounter`,
/// so the raw frequency is still needed to convert.
pub fn qpc_frequency() -> i64 {
    use windows::Win32::System::Performance::QueryPerformanceFrequency;
    let mut freq = 0i64;
    unsafe {
        let _ = QueryPerformanceFrequency(&mut freq);
    }
    if freq == 0 {
        10_000_000
    } else {
        freq
    }
}

/// Current performance counter value, converted to the 100 ns units that
/// `IAudioCaptureClient::GetBuffer` reports positions in.
pub fn qpc_now_100ns() -> i64 {
    use windows::Win32::System::Performance::QueryPerformanceCounter;
    let mut now = 0i64;
    unsafe {
        let _ = QueryPerformanceCounter(&mut now);
    }
    let freq = qpc_frequency();
    // Do the multiply in i128 so a machine that has been up for a long time
    // cannot overflow the intermediate value.
    ((now as i128 * 10_000_000i128) / freq as i128) as i64
}
