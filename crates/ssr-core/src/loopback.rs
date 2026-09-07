//! Opening capture clients on ordinary audio endpoints.
//!
//! Two shapes are handled here and they differ only by one flag: loopback
//! capture on a render endpoint, which records whatever the machine is playing,
//! and ordinary capture on an input endpoint, which records a microphone.

use windows::Win32::Media::Audio::{
    IAudioCaptureClient, IAudioClient, IMMDevice, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK,
};
use windows::Win32::System::Com::{CoTaskMemFree, CLSCTX_ALL};

use crate::error::Result;
use crate::event::Event;
use crate::format::AudioFormat;

/// An initialised, not yet started, capture client.
pub struct OpenClient {
    pub client: IAudioClient,
    pub capture: IAudioCaptureClient,
    pub format: AudioFormat,
    /// Buffer capacity the engine actually got, in frames.
    pub buffer_frames: u32,
}

/// Open a capture client on an endpoint.
///
/// The endpoint dictates the format: shared-mode capture and loopback both have
/// to use the endpoint mix format, so the returned [`AudioFormat`] is a readout,
/// not a request.
pub fn open_endpoint(
    device: &IMMDevice,
    loopback: bool,
    buffer_ms: u32,
    event: &Event,
) -> Result<OpenClient> {
    unsafe {
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;

        let mix = client.GetMixFormat()?;
        let format = AudioFormat::from_wave_format(mix);
        // Read the format before anything can fail, then always release the
        // allocation WASAPI handed us.
        let format = match format {
            Ok(f) => f,
            Err(e) => {
                CoTaskMemFree(Some(mix as *const _));
                return Err(e);
            }
        };

        let mut flags = AUDCLNT_STREAMFLAGS_EVENTCALLBACK;
        if loopback {
            flags |= AUDCLNT_STREAMFLAGS_LOOPBACK;
        }

        let result = client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            flags,
            (buffer_ms as i64) * 10_000,
            0,
            mix,
            None,
        );
        CoTaskMemFree(Some(mix as *const _));
        result?;

        client.SetEventHandle(event.raw())?;
        let buffer_frames = client.GetBufferSize()?;
        let capture: IAudioCaptureClient = client.GetService()?;

        Ok(OpenClient {
            client,
            capture,
            format,
            buffer_frames,
        })
    }
}
