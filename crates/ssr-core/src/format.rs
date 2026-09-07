//! Audio format description and conversion to the engine's internal
//! representation, which is always interleaved 32-bit float.
//!
//! Endpoints hand back whatever their mix format happens to be: 16-bit integer
//! at 44.1 kHz on one machine, 32-bit float at 48 kHz with a 7.1 channel mask on
//! the next. Normalising once, here, keeps every layer above this one simple.

use std::fmt;
use windows::core::GUID;
use windows::Win32::Media::Audio::{WAVEFORMATEX, WAVEFORMATEXTENSIBLE, WAVEFORMATEXTENSIBLE_0};

use crate::error::{Error, Result};

pub const WAVE_FORMAT_PCM: u16 = 0x0001;
pub const WAVE_FORMAT_IEEE_FLOAT: u16 = 0x0003;
pub const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

/// `KSDATAFORMAT_SUBTYPE_PCM`. Declared here rather than imported so the crate
/// does not depend on which windows-rs feature happens to re-export it.
pub const SUBTYPE_PCM: GUID = GUID::from_values(
    0x0000_0001,
    0x0000,
    0x0010,
    [0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71],
);

/// `KSDATAFORMAT_SUBTYPE_IEEE_FLOAT`.
pub const SUBTYPE_IEEE_FLOAT: GUID = GUID::from_values(
    0x0000_0003,
    0x0000,
    0x0010,
    [0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71],
);

/// How samples are laid out in the bytes WASAPI returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleType {
    /// 32-bit IEEE float, already the engine's internal representation.
    F32,
    /// 16-bit signed integer.
    I16,
    /// 24-bit signed integer packed into three bytes.
    I24,
    /// 32-bit signed integer container, `valid_bits` of which are meaningful.
    I32,
}

/// A fully resolved capture format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_type: SampleType,
    /// Bytes occupied by one sample of one channel.
    pub container_bytes: u16,
    /// Meaningful bits within the container, which can be smaller than the
    /// container for 24-in-32 formats.
    pub valid_bits: u16,
    /// Speaker positions, or zero when the endpoint did not say.
    pub channel_mask: u32,
}

impl AudioFormat {
    /// Bytes per frame across all channels.
    pub fn block_align(&self) -> usize {
        self.container_bytes as usize * self.channels as usize
    }

    /// The format the engine asks for when it gets to choose, which is the case
    /// for process loopback. Float avoids a conversion and cannot clip.
    pub fn float_stereo(sample_rate: u32) -> Self {
        AudioFormat {
            sample_rate,
            channels: 2,
            sample_type: SampleType::F32,
            container_bytes: 4,
            valid_bits: 32,
            // SPEAKER_FRONT_LEFT | SPEAKER_FRONT_RIGHT
            channel_mask: 0x3,
        }
    }

    /// Read a format out of the `WAVEFORMATEX` WASAPI handed us.
    ///
    /// # Safety
    /// `ptr` must point at a valid `WAVEFORMATEX`, and if its tag is
    /// `WAVE_FORMAT_EXTENSIBLE` the allocation must be large enough for a full
    /// `WAVEFORMATEXTENSIBLE`.
    pub unsafe fn from_wave_format(ptr: *const WAVEFORMATEX) -> Result<Self> {
        if ptr.is_null() {
            return Err(Error::Unsupported("null WAVEFORMATEX".into()));
        }
        // WAVEFORMATEX is a packed struct, so every field has to be copied out
        // by value before it can be borrowed (by `format!`, for instance).
        let wf = std::ptr::read_unaligned(ptr);
        let format_tag = wf.wFormatTag;
        let bits_per_sample = wf.wBitsPerSample;
        let sample_rate = wf.nSamplesPerSec;
        let channels = wf.nChannels;
        let cb_size = wf.cbSize;

        let container_bytes = bits_per_sample / 8;
        let mut valid_bits = bits_per_sample;
        let mut channel_mask = 0u32;

        let subtype = if format_tag == WAVE_FORMAT_EXTENSIBLE {
            if cb_size < 22 {
                return Err(Error::Unsupported(
                    "WAVE_FORMAT_EXTENSIBLE with a truncated extension".into(),
                ));
            }
            let ext = std::ptr::read_unaligned(ptr as *const WAVEFORMATEXTENSIBLE);
            channel_mask = ext.dwChannelMask;
            let vb = ext.Samples.wValidBitsPerSample;
            if vb != 0 {
                valid_bits = vb;
            }
            ext.SubFormat
        } else if format_tag == WAVE_FORMAT_IEEE_FLOAT {
            SUBTYPE_IEEE_FLOAT
        } else if format_tag == WAVE_FORMAT_PCM {
            SUBTYPE_PCM
        } else {
            return Err(Error::Unsupported(format!(
                "unhandled wFormatTag 0x{format_tag:04x}"
            )));
        };

        let sample_type = if subtype == SUBTYPE_IEEE_FLOAT {
            if bits_per_sample != 32 {
                return Err(Error::Unsupported(format!(
                    "{bits_per_sample}-bit float is not supported"
                )));
            }
            SampleType::F32
        } else if subtype == SUBTYPE_PCM {
            match bits_per_sample {
                16 => SampleType::I16,
                24 => SampleType::I24,
                32 => SampleType::I32,
                other => {
                    return Err(Error::Unsupported(format!(
                        "{other}-bit PCM is not supported"
                    )))
                }
            }
        } else {
            return Err(Error::Unsupported(format!(
                "unhandled subformat {subtype:?}; the endpoint may be using a compressed format"
            )));
        };

        Ok(AudioFormat {
            sample_rate,
            channels,
            sample_type,
            container_bytes,
            valid_bits,
            channel_mask,
        })
    }

    /// Build a `WAVEFORMATEXTENSIBLE` describing this format, for the calls where
    /// the engine chooses rather than the endpoint.
    pub fn to_wave_format_extensible(self) -> WAVEFORMATEXTENSIBLE {
        let block_align = self.block_align() as u16;
        WAVEFORMATEXTENSIBLE {
            Format: WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_EXTENSIBLE,
                nChannels: self.channels,
                nSamplesPerSec: self.sample_rate,
                nAvgBytesPerSec: self.sample_rate * block_align as u32,
                nBlockAlign: block_align,
                wBitsPerSample: self.container_bytes * 8,
                // sizeof(WAVEFORMATEXTENSIBLE) - sizeof(WAVEFORMATEX)
                cbSize: 22,
            },
            Samples: WAVEFORMATEXTENSIBLE_0 {
                wValidBitsPerSample: self.valid_bits,
            },
            dwChannelMask: self.channel_mask,
            SubFormat: match self.sample_type {
                SampleType::F32 => SUBTYPE_IEEE_FLOAT,
                _ => SUBTYPE_PCM,
            },
        }
    }

    /// Convert one captured buffer into interleaved f32, appending to `out`.
    ///
    /// `bytes` must be a whole number of frames; WASAPI guarantees that.
    pub fn convert_to_f32(&self, bytes: &[u8], out: &mut Vec<f32>) {
        match self.sample_type {
            SampleType::F32 => {
                out.reserve(bytes.len() / 4);
                for c in bytes.chunks_exact(4) {
                    out.push(f32::from_le_bytes([c[0], c[1], c[2], c[3]]));
                }
            }
            SampleType::I16 => {
                out.reserve(bytes.len() / 2);
                for c in bytes.chunks_exact(2) {
                    let v = i16::from_le_bytes([c[0], c[1]]);
                    out.push(v as f32 / 32_768.0);
                }
            }
            SampleType::I24 => {
                out.reserve(bytes.len() / 3);
                for c in bytes.chunks_exact(3) {
                    // Sign-extend the 24-bit value into an i32 by placing it in
                    // the high three bytes and shifting back down.
                    let v = i32::from_le_bytes([0, c[0], c[1], c[2]]) >> 8;
                    out.push(v as f32 / 8_388_608.0);
                }
            }
            SampleType::I32 => {
                out.reserve(bytes.len() / 4);
                // Some endpoints deliver 24 valid bits left-aligned in a 32-bit
                // container. Scaling by the container range is still correct
                // because the unused low bits are zero.
                for c in bytes.chunks_exact(4) {
                    let v = i32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                    out.push(v as f32 / 2_147_483_648.0);
                }
            }
        }
    }
}

impl fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ty = match self.sample_type {
            SampleType::F32 => "f32",
            SampleType::I16 => "i16",
            SampleType::I24 => "i24",
            SampleType::I32 => "i32",
        };
        write!(f, "{} Hz, {} ch, {}", self.sample_rate, self.channels, ty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt_of(sample_type: SampleType, container_bytes: u16) -> AudioFormat {
        AudioFormat {
            sample_rate: 48_000,
            channels: 1,
            sample_type,
            container_bytes,
            valid_bits: container_bytes * 8,
            channel_mask: 0,
        }
    }

    #[test]
    fn i16_full_scale_round_trips() {
        let mut out = Vec::new();
        let bytes = [0x00, 0x80, 0xff, 0x7f, 0x00, 0x00];
        fmt_of(SampleType::I16, 2).convert_to_f32(&bytes, &mut out);
        assert_eq!(out[0], -1.0);
        assert!((out[1] - 1.0).abs() < 1e-4);
        assert_eq!(out[2], 0.0);
    }

    #[test]
    fn i24_sign_extends() {
        let mut out = Vec::new();
        // -8388608 (0x800000) then +8388607 (0x7FFFFF), little endian.
        let bytes = [0x00, 0x00, 0x80, 0xff, 0xff, 0x7f];
        fmt_of(SampleType::I24, 3).convert_to_f32(&bytes, &mut out);
        assert_eq!(out[0], -1.0);
        assert!((out[1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn f32_passes_through() {
        let mut out = Vec::new();
        let bytes: Vec<u8> = [0.5f32, -0.25]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        fmt_of(SampleType::F32, 4).convert_to_f32(&bytes, &mut out);
        assert_eq!(out, vec![0.5, -0.25]);
    }

    #[test]
    fn block_align_covers_all_channels() {
        let f = AudioFormat::float_stereo(48_000);
        assert_eq!(f.block_align(), 8);
    }
}
