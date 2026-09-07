//! A streaming WAV writer for 32-bit float audio.
//!
//! Written by hand rather than pulled from a crate because the recorder needs
//! `WAVE_FORMAT_EXTENSIBLE` with a real channel mask for surround endpoints, and
//! needs to patch the chunk sizes on close after streaming for hours.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// `KSDATAFORMAT_SUBTYPE_IEEE_FLOAT`, little-endian byte order.
const SUBTYPE_IEEE_FLOAT: [u8; 16] = [
    0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
];

/// Largest data payload a classic RIFF file can address.
const RIFF_LIMIT: u64 = u32::MAX as u64 - 128;

/// Byte offsets of the fields that have to be back-patched when the file closes.
const OFF_RIFF_SIZE: u64 = 4;
const OFF_FACT_SAMPLES: u64 = 12 + 8 + 40 + 8;
const OFF_DATA_SIZE: u64 = OFF_FACT_SAMPLES + 4 + 4;

/// A WAV file being streamed to disk.
pub struct WavWriter {
    file: BufWriter<File>,
    path: PathBuf,
    channels: u16,
    sample_rate: u32,
    frames_written: u64,
    /// Set once the payload passes what a RIFF header can describe.
    overflowed: bool,
}

impl WavWriter {
    /// Create a file and write its header, leaving sizes to be patched later.
    pub fn create(
        path: impl AsRef<Path>,
        sample_rate: u32,
        channels: u16,
        channel_mask: u32,
    ) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = File::create(&path).with_context(|| format!("creating {}", path.display()))?;
        let mut w = WavWriter {
            file: BufWriter::with_capacity(1 << 18, file),
            path,
            channels,
            sample_rate,
            frames_written: 0,
            overflowed: false,
        };
        w.write_header(channel_mask)?;
        Ok(w)
    }

    fn write_header(&mut self, channel_mask: u32) -> Result<()> {
        let block_align = self.channels * 4;
        let byte_rate = self.sample_rate * block_align as u32;
        let mask = if channel_mask != 0 {
            channel_mask
        } else {
            default_channel_mask(self.channels)
        };

        let f = &mut self.file;
        f.write_all(b"RIFF")?;
        f.write_all(&0u32.to_le_bytes())?; // patched on close
        f.write_all(b"WAVE")?;

        // fmt chunk, 40 bytes: WAVEFORMATEXTENSIBLE
        f.write_all(b"fmt ")?;
        f.write_all(&40u32.to_le_bytes())?;
        f.write_all(&0xFFFEu16.to_le_bytes())?; // WAVE_FORMAT_EXTENSIBLE
        f.write_all(&self.channels.to_le_bytes())?;
        f.write_all(&self.sample_rate.to_le_bytes())?;
        f.write_all(&byte_rate.to_le_bytes())?;
        f.write_all(&block_align.to_le_bytes())?;
        f.write_all(&32u16.to_le_bytes())?; // bits per sample
        f.write_all(&22u16.to_le_bytes())?; // cbSize
        f.write_all(&32u16.to_le_bytes())?; // valid bits
        f.write_all(&mask.to_le_bytes())?;
        f.write_all(&SUBTYPE_IEEE_FLOAT)?;

        // fact chunk: required for non-PCM formats.
        f.write_all(b"fact")?;
        f.write_all(&4u32.to_le_bytes())?;
        f.write_all(&0u32.to_le_bytes())?; // patched on close

        f.write_all(b"data")?;
        f.write_all(&0u32.to_le_bytes())?; // patched on close
        Ok(())
    }

    /// Append interleaved float samples.
    pub fn write_samples(&mut self, samples: &[f32]) -> Result<()> {
        if samples.is_empty() {
            return Ok(());
        }
        // Reinterpreting the slice avoids a per-sample byte conversion on what
        // is the hottest path in the writer. f32 and u8 have no padding or
        // invalid bit patterns, and WAV is little-endian like every platform
        // this runs on.
        #[cfg(target_endian = "little")]
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                samples.as_ptr() as *const u8,
                std::mem::size_of_val(samples),
            )
        };
        #[cfg(target_endian = "big")]
        let owned: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        #[cfg(target_endian = "big")]
        let bytes: &[u8] = &owned;

        self.file.write_all(bytes)?;
        self.frames_written += (samples.len() / self.channels.max(1) as usize) as u64;
        if self.data_bytes() > RIFF_LIMIT {
            self.overflowed = true;
        }
        Ok(())
    }

    /// Append `frames` frames of digital silence.
    pub fn write_silence(&mut self, frames: u64) -> Result<()> {
        if frames == 0 {
            return Ok(());
        }
        // Write in chunks so a long gap does not allocate a huge buffer.
        const CHUNK_FRAMES: u64 = 4096;
        let zeros = vec![0f32; (CHUNK_FRAMES as usize) * self.channels as usize];
        let mut left = frames;
        while left > 0 {
            let n = left.min(CHUNK_FRAMES) as usize;
            self.write_samples(&zeros[..n * self.channels as usize])?;
            left -= n as u64;
        }
        Ok(())
    }

    pub fn frames_written(&self) -> u64 {
        self.frames_written
    }

    pub fn duration_secs(&self) -> f64 {
        self.frames_written as f64 / self.sample_rate as f64
    }

    /// True when the payload exceeded what a RIFF header can describe.
    ///
    /// The audio on disk is still intact and readable by tools that tolerate a
    /// wrong size field, but the file is past the format's limit.
    pub fn overflowed(&self) -> bool {
        self.overflowed
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn data_bytes(&self) -> u64 {
        self.frames_written * self.channels as u64 * 4
    }

    /// Flush, patch the size fields, and close.
    pub fn finish(mut self) -> Result<u64> {
        self.file.flush()?;
        let data_bytes = self.data_bytes();
        let riff_size = (OFF_DATA_SIZE + 4 + data_bytes).saturating_sub(8);
        let total_samples = self.frames_written * self.channels as u64;

        let f = self.file.get_mut();
        f.seek(SeekFrom::Start(OFF_RIFF_SIZE))?;
        f.write_all(&(riff_size.min(u32::MAX as u64) as u32).to_le_bytes())?;
        f.seek(SeekFrom::Start(OFF_FACT_SAMPLES))?;
        f.write_all(&(total_samples.min(u32::MAX as u64) as u32).to_le_bytes())?;
        f.seek(SeekFrom::Start(OFF_DATA_SIZE))?;
        f.write_all(&(data_bytes.min(u32::MAX as u64) as u32).to_le_bytes())?;
        f.flush()?;
        Ok(self.frames_written)
    }
}

/// A sensible `dwChannelMask` when the endpoint did not supply one.
fn default_channel_mask(channels: u16) -> u32 {
    match channels {
        1 => 0x4,   // front centre
        2 => 0x3,   // front left + right
        4 => 0x33,  // quad
        6 => 0x3F,  // 5.1
        8 => 0x63F, // 7.1
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_offsets_match_the_bytes_written() {
        let dir = std::env::temp_dir().join("ssr-wav-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("offsets.wav");
        let mut w = WavWriter::create(&path, 48_000, 2, 0).unwrap();
        w.write_samples(&[0.0, 0.0]).unwrap();
        w.finish().unwrap();

        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[12..16], b"fmt ");
        // The patched offsets must land on the chunks they claim to.
        assert_eq!(
            &bytes[(OFF_FACT_SAMPLES - 8) as usize..(OFF_FACT_SAMPLES - 4) as usize],
            b"fact"
        );
        assert_eq!(
            &bytes[(OFF_DATA_SIZE - 4) as usize..OFF_DATA_SIZE as usize],
            b"data"
        );
        let data_size = u32::from_le_bytes(
            bytes[OFF_DATA_SIZE as usize..OFF_DATA_SIZE as usize + 4]
                .try_into()
                .unwrap(),
        );
        assert_eq!(data_size, 8);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn silence_writes_the_requested_frame_count() {
        let dir = std::env::temp_dir().join("ssr-wav-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("silence.wav");
        let mut w = WavWriter::create(&path, 48_000, 2, 0).unwrap();
        w.write_silence(10_000).unwrap();
        assert_eq!(w.frames_written(), 10_000);
        w.finish().unwrap();
        let _ = std::fs::remove_file(&path);
    }
}
