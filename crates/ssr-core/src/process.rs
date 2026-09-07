//! Per-process loopback capture.
//!
//! This is the capability that makes the recorder useful rather than merely
//! functional. Instead of one mixed file, a Citrix session, a browser tab and a
//! chat client can each land on their own track, because Windows can synthesise
//! a virtual endpoint carrying only the audio of one process tree.
//!
//! Requires Windows 10 build 20348 or later.

use std::mem::{size_of, ManuallyDrop};
use std::sync::{Arc, Mutex};

use windows::core::Interface;
use windows::Win32::Media::Audio::{
    ActivateAudioInterfaceAsync, IAudioCaptureClient, IAudioClient, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK, AUDIOCLIENT_ACTIVATION_PARAMS,
    AUDIOCLIENT_ACTIVATION_PARAMS_0, AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
    AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS, PROCESS_LOOPBACK_MODE,
    PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE,
    PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE, VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
    WAVEFORMATEX,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::BLOB;
use windows::Win32::System::Variant::VT_BLOB;

use crate::error::{Error, Result};
use crate::event::Event;
use crate::format::{AudioFormat, SampleType, WAVE_FORMAT_IEEE_FLOAT, WAVE_FORMAT_PCM};
use crate::loopback::OpenClient;

/// Whether to record the target process tree or everything except it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessMode {
    /// Capture only the target process and its children.
    Include,
    /// Capture everything the machine plays except the target process tree.
    /// Useful for recording "the system, minus my own playback".
    Exclude,
}

impl From<ProcessMode> for PROCESS_LOOPBACK_MODE {
    fn from(m: ProcessMode) -> Self {
        match m {
            ProcessMode::Include => PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
            ProcessMode::Exclude => PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE,
        }
    }
}

/// The outcome of an activation, filled in by the completion handler.
///
/// `IAudioClient` is not `Send`, but every thread involved here lives in the
/// same multithreaded apartment, so the pointer is valid on whichever thread
/// picks it up. That is the whole justification for the unsafe impls below, and
/// it is why the engine insists on an MTA everywhere.
struct ActivationSlot {
    hr: windows::core::HRESULT,
    client: Option<IAudioClient>,
}

struct SharedSlot(Mutex<ActivationSlot>);

unsafe impl Send for SharedSlot {}
unsafe impl Sync for SharedSlot {}

impl SharedSlot {
    fn new() -> Arc<Self> {
        Arc::new(SharedSlot(Mutex::new(ActivationSlot {
            hr: windows::core::HRESULT(0),
            client: None,
        })))
    }
}

mod handler {
    use super::*;
    use windows::core::implement;
    use windows::Win32::Media::Audio::{
        IActivateAudioInterfaceAsyncOperation, IActivateAudioInterfaceCompletionHandler,
        IActivateAudioInterfaceCompletionHandler_Impl,
    };

    /// Completion handler for `ActivateAudioInterfaceAsync`.
    ///
    /// The result has to be collected here, inside the callback, rather than by
    /// the waiting thread afterwards. Calling `GetActivateResult` from another
    /// thread once the callback has returned deadlocks against the audio
    /// service, which is a memorable way to discover that the documented
    /// pattern exists for a reason.
    #[implement(IActivateAudioInterfaceCompletionHandler)]
    pub(super) struct CompletionHandler {
        pub(super) done: Event,
        pub(super) slot: Arc<SharedSlot>,
    }

    impl IActivateAudioInterfaceCompletionHandler_Impl for CompletionHandler_Impl {
        fn ActivateCompleted(
            &self,
            operation: windows::core::Ref<IActivateAudioInterfaceAsyncOperation>,
        ) -> windows::core::Result<()> {
            {
                let mut slot = match self.slot.0.lock() {
                    Ok(g) => g,
                    // A poisoned lock means a previous activation panicked. The
                    // waiter will time out, which is the right outcome.
                    Err(_) => {
                        self.done.set();
                        return Ok(());
                    }
                };

                match operation.as_ref() {
                    Some(op) => {
                        let mut hr = windows::core::HRESULT(0);
                        let mut unknown: Option<windows::core::IUnknown> = None;
                        match unsafe { op.GetActivateResult(&mut hr, &mut unknown) } {
                            Ok(()) => {
                                slot.hr = hr;
                                if hr.is_ok() {
                                    slot.client = unknown.and_then(|u| u.cast().ok());
                                }
                            }
                            Err(e) => slot.hr = e.code(),
                        }
                    }
                    None => slot.hr = windows::core::HRESULT(-2147467261), // E_POINTER
                }
            }
            self.done.set();
            Ok(())
        }
    }
}

/// Activate a raw `IAudioClient` bound to one process tree.
fn activate_client(pid: u32, mode: ProcessMode) -> Result<IAudioClient> {
    use windows::Win32::Media::Audio::IActivateAudioInterfaceCompletionHandler;

    crate::trace!("activating process loopback: pid={pid} mode={mode:?}");
    let done = Event::new()?;
    let slot = SharedSlot::new();
    let handler: IActivateAudioInterfaceCompletionHandler = handler::CompletionHandler {
        done: done.clone(),
        slot: slot.clone(),
    }
    .into();

    // Must stay alive for the duration of the call: the PROPVARIANT below holds
    // a borrowed pointer to it rather than a copy.
    let mut params = AUDIOCLIENT_ACTIVATION_PARAMS {
        ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
        Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
            ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                TargetProcessId: pid,
                ProcessLoopbackMode: mode.into(),
            },
        },
    };

    // The activation parameters travel inside a PROPVARIANT holding a VT_BLOB
    // that points at `params` on the stack.
    //
    // It must be wrapped in ManuallyDrop. windows-rs gives PROPVARIANT a Drop
    // that calls PropVariantClear, which for a VT_BLOB calls CoTaskMemFree on
    // the blob pointer. Handing the allocator a stack address corrupts the heap
    // and the process wedges on its next allocation, some distance from the
    // real cause. This blob is borrowed, not owned, so it must never be cleared.
    let mut propvariant = ManuallyDrop::new(PROPVARIANT::default());
    unsafe {
        let inner = &mut *propvariant.Anonymous.Anonymous;
        inner.vt = VT_BLOB;
        inner.Anonymous.blob = BLOB {
            cbSize: size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,
            pBlobData: &mut params as *mut _ as *mut u8,
        };
    }

    crate::trace!("calling ActivateAudioInterfaceAsync");
    let operation = unsafe {
        ActivateAudioInterfaceAsync(
            VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
            &IAudioClient::IID,
            Some(&*propvariant as *const PROPVARIANT),
            &handler,
        )?
    };

    crate::trace!("activation call returned; waiting for the completion callback");
    // Activation is normally immediate; a second is a generous ceiling that
    // still fails fast if the audio service is wedged.
    let never = Event::new()?;
    if crate::event::wait_either(&done, &never, 1_000) != crate::event::Woken::First {
        return Err(Error::ActivationTimeout);
    }

    let mut guard = slot
        .0
        .lock()
        .map_err(|_| Error::CaptureFailed("activation state was poisoned".into()))?;
    crate::trace!("activation hr = 0x{:08x}", guard.hr.0);
    guard.hr.ok()?;
    let client = guard.client.take().ok_or_else(|| {
        Error::CaptureFailed("process loopback activation returned no audio client".into())
    })?;
    drop(guard);

    // Release the activation machinery before returning; only the client is
    // needed from here on.
    drop(operation);
    drop(handler);
    crate::trace!("process loopback client activated");
    Ok(client)
}

/// Formats to try, in descending order of preference.
///
/// Unlike endpoint loopback the caller picks the format here, but not every
/// build accepts every choice, so the engine negotiates downwards rather than
/// failing outright. Float first because it cannot clip; 16-bit integer last
/// because it is what the platform documents and always accepts.
fn candidate_formats(sample_rate: u32, channels: u16) -> Vec<(AudioFormat, u16)> {
    vec![
        (
            AudioFormat {
                sample_rate,
                channels,
                sample_type: SampleType::F32,
                container_bytes: 4,
                valid_bits: 32,
                channel_mask: 0,
            },
            WAVE_FORMAT_IEEE_FLOAT,
        ),
        (
            AudioFormat {
                sample_rate,
                channels,
                sample_type: SampleType::I16,
                container_bytes: 2,
                valid_bits: 16,
                channel_mask: 0,
            },
            WAVE_FORMAT_PCM,
        ),
    ]
}

/// Open a capture client that records one process tree.
pub fn open_process(
    pid: u32,
    mode: ProcessMode,
    sample_rate: u32,
    channels: u16,
    buffer_ms: u32,
    event: &Event,
) -> Result<OpenClient> {
    let mut last_err: Option<Error> = None;

    for (format, tag) in candidate_formats(sample_rate, channels) {
        // A fresh client per attempt: IAudioClient::Initialize may only be
        // called once, successfully or not.
        let client = activate_client(pid, mode)?;

        let block_align = format.block_align() as u16;
        let wfx = WAVEFORMATEX {
            wFormatTag: tag,
            nChannels: format.channels,
            nSamplesPerSec: format.sample_rate,
            nAvgBytesPerSec: format.sample_rate * block_align as u32,
            nBlockAlign: block_align,
            wBitsPerSample: format.container_bytes * 8,
            cbSize: 0,
        };

        crate::trace!("trying format {format}");
        let init = unsafe {
            client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                (buffer_ms as i64) * 10_000,
                // Process loopback requires a zero periodicity.
                0,
                &wfx,
                None,
            )
        };

        match init {
            Ok(()) => unsafe {
                crate::trace!("Initialize accepted {format}");
                client.SetEventHandle(event.raw())?;
                let buffer_frames = client.GetBufferSize()?;
                let capture: IAudioCaptureClient = client.GetService()?;
                return Ok(OpenClient {
                    client,
                    capture,
                    format,
                    buffer_frames,
                });
            },
            Err(e) => {
                crate::trace!("Initialize rejected for {format}: {e}");
                last_err = Some(Error::Windows(e));
            }
        }
    }

    Err(last_err.unwrap_or_else(|| {
        Error::CaptureFailed("no candidate format was accepted for process loopback".into())
    }))
}
