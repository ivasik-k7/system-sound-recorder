//! A capture source: one thread, one WASAPI stream, one packet channel.
//!
//! Every source looks the same from the outside regardless of whether it is
//! tapping a whole endpoint, one process tree, or a microphone. That uniformity
//! is what lets the timeline layer stay ignorant of WASAPI.

use std::thread::JoinHandle;

use crossbeam_channel::{bounded, SendTimeoutError, Sender};
use windows::Win32::Media::Audio::{
    AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY, AUDCLNT_BUFFERFLAGS_SILENT,
    AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR, AUDCLNT_E_DEVICE_INVALIDATED,
};

use crate::com::Apartment;
use crate::device::{DeviceKind, Devices};
use crate::error::{Error, Result};
use crate::event::{wait_either, Event, Woken};
use crate::format::AudioFormat;
use crate::loopback::{open_endpoint, OpenClient};

/// The key a packet is routed by.
///
/// Shared rather than owned: a capture thread hands over a packet every few
/// milliseconds and each one used to allocate a fresh copy of the track name
/// purely to say where it belonged. Cloning this is an atomic increment.
pub type TrackKey = std::sync::Arc<str>;

/// How long a capture thread waits for the writer to take a packet.
///
/// Long enough to ride out a disk hiccup or an antivirus scan, short enough
/// that a genuinely stuck writer does not accumulate an unbounded backlog of
/// threads waiting on it. The buffer is already released by this point, so
/// waiting here costs latency rather than audio.
const HANDOVER_PATIENCE: std::time::Duration = std::time::Duration::from_millis(250);
use crate::packet::{Packet, PacketFlags};
use crate::process::{open_process, ProcessMode};
use crate::procfind;

/// What a source should record.
#[derive(Debug, Clone)]
pub enum SourceSpec {
    /// Everything played through a render endpoint, mixed.
    ///
    /// This catches Citrix HDX audio and WSLg audio without needing to know
    /// which process produced them, because by the time audio reaches an
    /// endpoint it has already been mixed.
    EndpointLoopback {
        /// `default`, an endpoint id, or a substring of the endpoint name.
        device: String,
    },
    /// A recording endpoint such as a microphone.
    Input { device: String },
    /// One process tree, isolated from everything else on the machine.
    Process {
        /// A PID, or a case-insensitive executable name such as `wfica32.exe`.
        selector: String,
        mode: ProcessMode,
        sample_rate: u32,
        channels: u16,
    },
}

impl SourceSpec {
    /// A short human description, recorded in the session manifest.
    pub fn describe(&self) -> String {
        match self {
            SourceSpec::EndpointLoopback { device } => format!("loopback on {device}"),
            SourceSpec::Input { device } => format!("input from {device}"),
            SourceSpec::Process {
                selector,
                mode: ProcessMode::Include,
                ..
            } => format!("process tree {selector}"),
            SourceSpec::Process {
                selector,
                mode: ProcessMode::Exclude,
                ..
            } => format!("everything except process tree {selector}"),
        }
    }
}

/// Counters a source accumulates over its lifetime.
#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct CaptureStats {
    pub packets: u64,
    pub frames: u64,
    /// Times WASAPI reported that samples were lost upstream of the recorder.
    pub discontinuities: u64,
    /// Packets the writer could not take in time, and which were therefore
    /// thrown away. Should be zero; anything else means the disk could not
    /// keep up and the recording has holes the timeline will fill with
    /// silence.
    pub dropped: u64,
}

/// A running source. Dropping this stops and joins the capture thread.
pub struct SourceHandle {
    /// Track name, used for the output filename.
    pub name: String,
    /// The format the capture thread actually negotiated.
    pub format: AudioFormat,
    /// Human description of what is being captured.
    pub description: String,
    /// Resolved PID, when the source is a process loopback.
    pub pid: Option<u32>,
    stop: Event,
    join: Option<JoinHandle<Result<CaptureStats>>>,
}

impl SourceHandle {
    /// Ask the capture thread to finish, without waiting for it.
    ///
    /// Separate from [`SourceHandle::join`] on purpose. A capture thread can be
    /// blocked trying to hand over a packet, so the consumer has to keep
    /// draining the channel while threads wind down. Signalling and joining in
    /// one call would deadlock against its own receiver.
    pub fn signal_stop(&self) {
        self.stop.set();
    }

    /// Wait for the capture thread to finish and collect its counters.
    pub fn join(&mut self) -> Result<CaptureStats> {
        self.stop.set();
        match self.join.take() {
            Some(handle) => handle
                .join()
                .unwrap_or_else(|_| Err(Error::CaptureFailed("capture thread panicked".into()))),
            None => Ok(CaptureStats::default()),
        }
    }

    /// Signal the capture thread and wait for it to finish.
    pub fn stop(mut self) -> Result<CaptureStats> {
        self.join()
    }
}

impl Drop for SourceHandle {
    fn drop(&mut self) {
        self.stop.set();
        if let Some(handle) = self.join.take() {
            let _ = handle.join();
        }
    }
}

/// What the capture thread reports back once its stream is running.
struct Ready {
    format: AudioFormat,
    pid: Option<u32>,
}

/// Start capturing.
///
/// Returns once the stream is open and running, so a failure to open a device
/// surfaces here rather than silently producing an empty file.
pub fn start(
    name: impl Into<String>,
    spec: SourceSpec,
    buffer_ms: u32,
    packets: Sender<(TrackKey, Packet)>,
) -> Result<SourceHandle> {
    let name = name.into();
    let description = spec.describe();
    let stop = Event::new()?;

    let (ready_tx, ready_rx) = bounded::<Result<Ready>>(1);
    let thread_stop = stop.clone();
    let thread_key: TrackKey = TrackKey::from(name.as_str());
    let thread_spec = spec.clone();

    let join = std::thread::Builder::new()
        .name(format!("ssr-capture-{name}"))
        .spawn(move || {
            let _apartment = Apartment::new();
            let audio_event = match Event::new() {
                Ok(e) => e,
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return Err(Error::CaptureFailed("could not create audio event".into()));
                }
            };

            crate::trace!("opening source {thread_key:?}: {thread_spec:?}");
            let opened = open_for_spec(&thread_spec, buffer_ms, &audio_event);
            let (open, pid) = match opened {
                Ok(v) => v,
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return Err(Error::CaptureFailed(format!(
                        "{thread_key}: stream never opened"
                    )));
                }
            };

            crate::trace!("source {thread_key:?} opened as {}", open.format);
            if let Err(e) = unsafe { open.client.Start() } {
                let _ = ready_tx.send(Err(Error::Windows(e.clone())));
                return Err(Error::Windows(e));
            }

            let _ = ready_tx.send(Ok(Ready {
                format: open.format,
                pid,
            }));

            crate::trace!("source {thread_key:?} started");
            let result = run_capture(&thread_key, &open, &thread_stop, &audio_event, &packets);
            unsafe {
                let _ = open.client.Stop();
            }
            result
        })
        .map_err(|e| Error::CaptureFailed(format!("could not spawn capture thread: {e}")))?;

    crate::trace!("waiting for source {name:?} to report ready");
    let ready = ready_rx
        .recv()
        .map_err(|_| Error::CaptureFailed("capture thread exited before starting".into()))??;

    Ok(SourceHandle {
        name,
        format: ready.format,
        description,
        pid: ready.pid,
        stop,
        join: Some(join),
    })
}

/// Open the right kind of client for a spec, resolving selectors as we go.
fn open_for_spec(
    spec: &SourceSpec,
    buffer_ms: u32,
    event: &Event,
) -> Result<(OpenClient, Option<u32>)> {
    match spec {
        SourceSpec::EndpointLoopback { device } => {
            let devices = Devices::new()?;
            let endpoint = devices.resolve(device, DeviceKind::Render)?;
            Ok((open_endpoint(&endpoint, true, buffer_ms, event)?, None))
        }
        SourceSpec::Input { device } => {
            let devices = Devices::new()?;
            let endpoint = devices.resolve(device, DeviceKind::Capture)?;
            Ok((open_endpoint(&endpoint, false, buffer_ms, event)?, None))
        }
        SourceSpec::Process {
            selector,
            mode,
            sample_rate,
            channels,
        } => {
            let pid = procfind::resolve_pid(selector)?;
            let open = open_process(pid, *mode, *sample_rate, *channels, buffer_ms, event)?;
            Ok((open, Some(pid)))
        }
    }
}

/// The capture loop.
///
/// Waits for either a stop signal or an audio buffer, then drains every packet
/// WASAPI has ready. The wait carries a deadline because a loopback stream on an
/// idle endpoint never raises its event at all, and the loop still has to notice
/// a stop request in that state.
fn run_capture(
    key: &TrackKey,
    open: &OpenClient,
    stop: &Event,
    audio: &Event,
    packets: &Sender<(TrackKey, Packet)>,
) -> Result<CaptureStats> {
    let mut stats = CaptureStats::default();
    let block_align = open.format.block_align();
    let channels = open.format.channels as usize;

    loop {
        match wait_either(stop, audio, 100) {
            Woken::First => break,
            Woken::Second | Woken::Timeout => {}
            Woken::Failed => {
                return Err(Error::CaptureFailed(format!("{key}: wait failed")));
            }
        }

        loop {
            let available = match unsafe { open.capture.GetNextPacketSize() } {
                Ok(n) => n,
                Err(e) if e.code() == AUDCLNT_E_DEVICE_INVALIDATED => {
                    return Err(Error::CaptureFailed(format!(
                        "{key}: the audio endpoint went away mid-recording"
                    )));
                }
                Err(e) => return Err(Error::Windows(e)),
            };
            if available == 0 {
                break;
            }

            let mut pending: Option<Packet> = None;
            let mut data: *mut u8 = std::ptr::null_mut();
            let mut frames: u32 = 0;
            let mut raw_flags: u32 = 0;
            let mut device_position: u64 = 0;
            let mut qpc_position: u64 = 0;

            unsafe {
                open.capture.GetBuffer(
                    &mut data,
                    &mut frames,
                    &mut raw_flags,
                    Some(&mut device_position),
                    Some(&mut qpc_position),
                )?;
            }

            if frames > 0 {
                let flags = PacketFlags {
                    discontinuity: raw_flags & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32 != 0,
                    silent: raw_flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0,
                    timestamp_error: raw_flags & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 != 0,
                };

                let sample_count = frames as usize * channels;
                let mut samples = Vec::with_capacity(sample_count);
                if flags.silent || data.is_null() {
                    // The buffer contents are undefined when SILENT is set, so
                    // write real zeros rather than whatever was left there.
                    samples.resize(sample_count, 0.0);
                } else {
                    let bytes =
                        unsafe { std::slice::from_raw_parts(data, frames as usize * block_align) };
                    open.format.convert_to_f32(bytes, &mut samples);
                }

                if flags.discontinuity {
                    stats.discontinuities += 1;
                }
                stats.packets += 1;
                stats.frames += frames as u64;

                pending = Some(Packet {
                    qpc_100ns: qpc_position as i64,
                    device_position,
                    samples,
                    channels,
                    flags,
                });
            }

            // Released before the handover, always.
            //
            // The samples have already been copied out, so nothing needs the
            // buffer any more — and holding it across a send means that a
            // writer stalling on a slow disk stalls the audio engine too. The
            // engine's response to that is to overrun its own ring and lose
            // audio, which is precisely the failure this recorder exists to
            // avoid. Release first and a slow writer costs at most a dropped
            // packet, which is counted.
            unsafe { open.capture.ReleaseBuffer(frames)? };

            if let Some(packet) = pending.take() {
                match packets.send_timeout((TrackKey::clone(key), packet), HANDOVER_PATIENCE) {
                    Ok(()) => {}
                    // A brief stall is worth waiting out; a long one is not,
                    // because the audio still arriving has nowhere to go
                    // either and blocking here only moves the loss upstream
                    // where it cannot be counted.
                    Err(SendTimeoutError::Timeout(_)) => stats.dropped += 1,
                    // A closed receiver means the session is shutting down.
                    Err(SendTimeoutError::Disconnected(_)) => return Ok(stats),
                }
            }
        }
    }

    Ok(stats)
}
