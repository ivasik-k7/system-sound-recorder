//! Endpoint enumeration and selection.

use windows::core::PCWSTR;
use windows::Win32::Devices::FunctionDiscovery::{
    PKEY_Device_DeviceDesc, PKEY_Device_FriendlyName,
};
use windows::Win32::Media::Audio::{
    eAll, eCapture, eConsole, eRender, IAudioClient, IMMDevice, IMMDeviceEnumerator,
    MMDeviceEnumerator, PKEY_AudioEndpoint_FormFactor, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::StructuredStorage::{
    PropVariantToStringAlloc, PropVariantToUInt32,
};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL, STGM_READ};

use crate::channels::{self, ChannelInfo};
use crate::error::{Error, Result};
use crate::format::{AudioFormat, SampleType};

/// Which direction an endpoint moves audio in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceKind {
    /// A playback endpoint. Loopback capture taps these.
    Render,
    /// A recording endpoint, such as a microphone.
    Capture,
}

/// The resolved stream format of an endpoint, with its channels named.
///
/// This is read from the endpoint's mix format, which is what a shared-mode
/// capture is obliged to use. It is therefore not a preference but a fact: it
/// is what the recorded file will contain.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointFormat {
    pub sample_rate: u32,
    pub channels: u16,
    /// Container size in bits, which can exceed the meaningful bits.
    pub bits: u16,
    pub valid_bits: u16,
    /// `32-bit float`, `24-bit integer`, and so on.
    pub sample_format: String,
    /// The raw speaker mask, kept so the interface can show it in diagnostics.
    pub channel_mask: u32,
    /// A short name for the arrangement, such as `5.1`.
    pub layout: String,
    /// One entry per channel, in the order they are interleaved.
    pub channels_detail: Vec<ChannelInfo>,
}

/// A discovered audio endpoint.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    /// Stable endpoint id, usable as a selector across reboots.
    pub id: String,
    /// Human-readable name as shown in the Windows sound control panel.
    pub name: String,
    pub kind: DeviceKind,
    /// True when this is the current default for the console role.
    pub is_default: bool,
    /// What the endpoint physically is: speakers, headphones, a headset, HDMI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_factor: Option<String>,
    /// The adapter's own description, which names the hardware behind a
    /// generic endpoint name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    /// The stream format, once resolved. `None` when it could not be read,
    /// which happens on an endpoint that is present but not usable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<EndpointFormat>,
}

/// An endpoint that has been found and identified, but not yet described.
///
/// Holding the `IMMDevice` alongside its id is what lets a caller decide how
/// much to ask of it: a name, a channel layout, its audio sessions, or nothing
/// at all.
pub struct EndpointHandle {
    pub device: IMMDevice,
    /// Stable endpoint id.
    pub id: String,
    pub kind: DeviceKind,
    pub is_default: bool,
}

/// Read the describable properties of an endpoint.
///
/// `with_format` activates an audio client to read the mix format, which is the
/// expensive part; without it the format is left empty for the caller to fill
/// from elsewhere.
pub fn describe_endpoint(endpoint: &EndpointHandle, with_format: bool) -> DeviceInfo {
    DeviceInfo {
        id: endpoint.id.clone(),
        name: device_name(&endpoint.device).unwrap_or_else(|_| "<unnamed>".to_string()),
        kind: endpoint.kind,
        is_default: endpoint.is_default,
        form_factor: form_factor(&endpoint.device),
        adapter: adapter_name(&endpoint.device),
        format: if with_format {
            endpoint_format(&endpoint.device).ok()
        } else {
            None
        },
    }
}

/// Handle to the system endpoint enumerator.
///
/// Requires a live COM apartment on the calling thread; see [`crate::com::Apartment`].
pub struct Devices {
    enumerator: IMMDeviceEnumerator,
}

impl Devices {
    pub fn new() -> Result<Self> {
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
        Ok(Devices { enumerator })
    }

    /// The underlying enumerator, for callers that need to walk endpoints
    /// themselves rather than take the summarised list.
    pub fn enumerator(&self) -> &IMMDeviceEnumerator {
        &self.enumerator
    }

    /// The default endpoint for the console role in the given direction.
    pub fn default_endpoint(&self, kind: DeviceKind) -> Result<IMMDevice> {
        let flow = match kind {
            DeviceKind::Render => eRender,
            DeviceKind::Capture => eCapture,
        };
        Ok(unsafe { self.enumerator.GetDefaultAudioEndpoint(flow, eConsole)? })
    }

    /// The endpoint ids that are currently default, render then capture.
    ///
    /// Read once and compared by id, because asking each endpoint whether it is
    /// the default is not a question the API answers.
    pub fn default_ids(&self) -> (Option<String>, Option<String>) {
        let render = self
            .default_endpoint(DeviceKind::Render)
            .ok()
            .and_then(|d| device_id(&d).ok());
        let capture = self
            .default_endpoint(DeviceKind::Capture)
            .ok()
            .and_then(|d| device_id(&d).ok());
        (render, capture)
    }

    /// Every active endpoint, identified but not yet described.
    ///
    /// This is deliberately the cheap half of enumeration: it costs one walk
    /// and two calls per endpoint, and reads no property stores and activates
    /// no clients. Naming an endpoint and resolving its format are separate
    /// steps because a caller polling for levels already knows both and should
    /// not pay for them again.
    pub fn endpoints(&self) -> Result<Vec<EndpointHandle>> {
        let (default_render_id, default_capture_id) = self.default_ids();
        let collection = unsafe {
            self.enumerator
                .EnumAudioEndpoints(eAll, DEVICE_STATE_ACTIVE)?
        };
        let count = unsafe { collection.GetCount()? };

        let mut out = Vec::with_capacity(count as usize);
        for i in 0..count {
            let Ok(device) = (unsafe { collection.Item(i) }) else {
                continue;
            };
            let Ok(id) = device_id(&device) else { continue };
            let Ok(kind) = endpoint_kind(&device) else {
                continue;
            };
            let is_default = match kind {
                DeviceKind::Render => default_render_id.as_deref() == Some(id.as_str()),
                DeviceKind::Capture => default_capture_id.as_deref() == Some(id.as_str()),
            };
            out.push(EndpointHandle {
                device,
                id,
                kind,
                is_default,
            });
        }
        Ok(out)
    }

    /// Every active endpoint, both directions, with defaults marked.
    ///
    /// The stream format is not resolved; see [`Devices::list_detailed`].
    pub fn list(&self) -> Result<Vec<DeviceInfo>> {
        Ok(self
            .endpoints()?
            .iter()
            .map(|e| describe_endpoint(e, false))
            .collect())
    }

    /// Every active endpoint with its stream format resolved.
    ///
    /// Best-effort by design: an endpoint whose client will not activate still
    /// appears, with `format` left empty. Half-connected Bluetooth headsets sit
    /// in exactly that state, and dropping them from the list would be a worse
    /// answer than listing them without their format.
    pub fn list_detailed(&self) -> Vec<DeviceInfo> {
        self.endpoints()
            .map(|list| list.iter().map(|e| describe_endpoint(e, true)).collect())
            .unwrap_or_default()
    }

    /// Resolve a user-supplied selector to an endpoint.
    ///
    /// A selector is either the literal `default`, a full endpoint id, or a
    /// case-insensitive substring of the friendly name.
    pub fn resolve(&self, selector: &str, kind: DeviceKind) -> Result<IMMDevice> {
        if selector.eq_ignore_ascii_case("default") {
            return self.default_endpoint(kind);
        }

        let collection = unsafe {
            self.enumerator
                .EnumAudioEndpoints(eAll, DEVICE_STATE_ACTIVE)?
        };
        let count = unsafe { collection.GetCount()? };
        let needle = selector.to_lowercase();

        let mut fallback: Option<IMMDevice> = None;
        for i in 0..count {
            let device = unsafe { collection.Item(i)? };
            if endpoint_kind(&device)? != kind {
                continue;
            }
            let id = device_id(&device)?;
            if id == selector {
                return Ok(device);
            }
            let name = device_name(&device).unwrap_or_default();
            if fallback.is_none() && name.to_lowercase().contains(&needle) {
                fallback = Some(device);
            }
        }
        fallback.ok_or_else(|| Error::NoSuchDevice(selector.to_string()))
    }
}

/// Read an endpoint's stable id.
pub fn device_id(device: &IMMDevice) -> Result<String> {
    unsafe {
        let raw = device.GetId()?;
        let s = raw.to_string().unwrap_or_default();
        windows::Win32::System::Com::CoTaskMemFree(Some(raw.0 as *const _));
        Ok(s)
    }
}

/// Read a string out of an endpoint's property store.
fn string_property(
    device: &IMMDevice,
    key: &windows::Win32::Foundation::PROPERTYKEY,
) -> Result<String> {
    unsafe {
        let store = device.OpenPropertyStore(STGM_READ)?;
        let value = store.GetValue(key)?;
        let raw = PropVariantToStringAlloc(&value)?;
        let s = raw.to_string().unwrap_or_default();
        CoTaskMemFree(Some(raw.0 as *const _));
        Ok(s)
    }
}

/// Read an endpoint's friendly name from its property store.
pub fn device_name(device: &IMMDevice) -> Result<String> {
    string_property(device, &PKEY_Device_FriendlyName)
}

/// The adapter's description, such as `Realtek High Definition Audio`.
///
/// Useful because friendly names are frequently generic: three endpoints all
/// called "Speakers" are told apart by the hardware behind them.
pub fn adapter_name(device: &IMMDevice) -> Option<String> {
    string_property(device, &PKEY_Device_DeviceDesc)
        .ok()
        .filter(|s| !s.is_empty())
}

/// What the endpoint physically is, as Windows classifies it.
///
/// Worth surfacing because it changes what a recording means. A loopback tap on
/// `Headphones` captures what one person heard; the same tap on
/// `DigitalAudioDisplayDevice` is an HDMI output that may have nobody listening
/// to it at all.
pub fn form_factor(device: &IMMDevice) -> Option<String> {
    let raw = unsafe {
        let store = device.OpenPropertyStore(STGM_READ).ok()?;
        let value = store.GetValue(&PKEY_AudioEndpoint_FormFactor).ok()?;
        PropVariantToUInt32(&value).ok()?
    };
    // The order is fixed by EndpointFormFactor in mmdeviceapi.h.
    Some(
        match raw {
            0 => "Remote network device",
            1 => "Speakers",
            2 => "Line level",
            3 => "Headphones",
            4 => "Microphone",
            5 => "Headset",
            6 => "Handset",
            7 => "Digital passthrough",
            8 => "S/PDIF",
            9 => "HDMI or DisplayPort",
            _ => return None,
        }
        .to_string(),
    )
}

/// Describe a resolved capture format in the terms the interface shows.
pub fn describe_format(format: AudioFormat) -> EndpointFormat {
    let sample_format = match format.sample_type {
        SampleType::F32 => "32-bit float".to_string(),
        SampleType::I16 => "16-bit integer".to_string(),
        SampleType::I24 => "24-bit integer".to_string(),
        SampleType::I32 => {
            // 24-in-32 is common enough that calling it "32-bit" would be
            // misleading about how much of the file is real.
            if format.valid_bits != 0 && format.valid_bits < 32 {
                format!("{}-bit in 32-bit container", format.valid_bits)
            } else {
                "32-bit integer".to_string()
            }
        }
    };
    EndpointFormat {
        sample_rate: format.sample_rate,
        channels: format.channels,
        bits: format.container_bytes * 8,
        valid_bits: format.valid_bits,
        sample_format,
        channel_mask: format.channel_mask,
        layout: channels::layout_name(format.channel_mask, format.channels),
        channels_detail: channels::channels_of(format.channel_mask, format.channels),
    }
}

/// Read an endpoint's mix format and name its channels.
///
/// The mix format is not a suggestion: a shared-mode capture, loopback
/// included, has to use it. Reading it before recording is what lets the
/// interface promise what the file will contain rather than guess.
pub fn endpoint_format(device: &IMMDevice) -> Result<EndpointFormat> {
    unsafe {
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let mix = client.GetMixFormat()?;
        let parsed = AudioFormat::from_wave_format(mix);
        // Free the allocation whichever way the parse went.
        CoTaskMemFree(Some(mix as *const _));
        Ok(describe_format(parsed?))
    }
}

/// Determine whether an endpoint renders or captures.
pub fn endpoint_kind(device: &IMMDevice) -> Result<DeviceKind> {
    use windows::core::Interface;
    use windows::Win32::Media::Audio::IMMEndpoint;

    // IMMEndpoint::GetDataFlow is the direct answer, and every endpoint
    // implements it.
    let endpoint: IMMEndpoint = device.cast()?;
    let flow = unsafe { endpoint.GetDataFlow()? };
    Ok(if flow == eRender {
        DeviceKind::Render
    } else {
        DeviceKind::Capture
    })
}

/// Convert a Rust string into a null-terminated UTF-16 buffer plus a pointer to it.
///
/// The returned `Vec` must outlive any use of the `PCWSTR`.
pub fn wide(s: &str) -> (Vec<u16>, PCWSTR) {
    let mut buf: Vec<u16> = s.encode_utf16().collect();
    buf.push(0);
    let ptr = PCWSTR(buf.as_ptr());
    (buf, ptr)
}
