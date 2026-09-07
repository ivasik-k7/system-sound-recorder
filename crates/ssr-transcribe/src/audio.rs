//! Reading recorded tracks and preparing them for a speech engine.
//!
//! Speech models want 16 kHz mono 16-bit audio. The recorder writes 32-bit
//! float at whatever rate the endpoint runs at, usually 48 kHz stereo. The
//! conversion is done here rather than by shelling out to ffmpeg, so that
//! transcription needs nothing installed beyond the speech engine itself.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};

/// What a speech model expects.
pub const TARGET_RATE: u32 = 16_000;

/// A decoded track: mono samples and the rate they are at.
pub struct Audio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl Audio {
    pub fn duration_secs(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.samples.len() as f64 / self.sample_rate as f64
        }
    }

    /// True when nothing in the track rises above the noise floor.
    ///
    /// Worth knowing before spending minutes of processing on a track that
    /// holds nothing but manufactured silence, which is a common outcome for an
    /// application that never made a sound.
    pub fn is_silent(&self) -> bool {
        const FLOOR: f32 = 0.0005; // about -66 dBFS
        !self.samples.iter().any(|s| s.abs() > FLOOR)
    }
}

/// Read a WAV file, downmixing to mono.
///
/// Handles the formats the recorder writes and the common ones it does not:
/// 32-bit float, and 16, 24 or 32-bit integer, in either the plain or the
/// extensible header form.
pub fn read_wav_mono(path: &Path) -> Result<Audio> {
    let data = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        bail!("{} is not a RIFF WAVE file", path.display());
    }

    let mut fmt: Option<(u16, u16, u32, u16)> = None; // tag, channels, rate, bits
    let mut audio: Option<&[u8]> = None;

    let mut pos = 12usize;
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let size = u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
            as usize;
        let body_start = pos + 8;
        let body_end = body_start.saturating_add(size).min(data.len());

        if id == b"fmt " && size >= 16 {
            let b = &data[body_start..body_end];
            let mut tag = u16::from_le_bytes([b[0], b[1]]);
            let channels = u16::from_le_bytes([b[2], b[3]]);
            let rate = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
            let bits = u16::from_le_bytes([b[14], b[15]]);
            // An extensible header carries the real format in its subformat
            // GUID; only the first four bytes of that GUID distinguish them.
            if tag == 0xFFFE && size >= 40 {
                tag = u16::from_le_bytes([b[24], b[25]]);
            }
            fmt = Some((tag, channels, rate, bits));
        } else if id == b"data" {
            audio = Some(&data[body_start..body_end]);
        }

        // Chunks are padded to an even length.
        pos = body_start + size + (size & 1);
    }

    let (tag, channels, rate, bits) =
        fmt.ok_or_else(|| anyhow::anyhow!("{} has no fmt chunk", path.display()))?;
    let bytes = audio.ok_or_else(|| anyhow::anyhow!("{} has no data chunk", path.display()))?;
    if channels == 0 {
        bail!("{} claims zero channels", path.display());
    }

    let container = (bits / 8) as usize;
    if container == 0 {
        bail!("{} claims {bits} bits per sample", path.display());
    }
    let frame = container * channels as usize;
    let frames = bytes.len() / frame;

    let mut mono = Vec::with_capacity(frames);
    for f in 0..frames {
        let base = f * frame;
        let mut sum = 0.0f32;
        for c in 0..channels as usize {
            let o = base + c * container;
            let v = match (tag, bits) {
                (3, 32) => f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]),
                (1, 16) => i16::from_le_bytes([bytes[o], bytes[o + 1]]) as f32 / 32_768.0,
                (1, 24) => {
                    (i32::from_le_bytes([0, bytes[o], bytes[o + 1], bytes[o + 2]]) >> 8) as f32
                        / 8_388_608.0
                }
                (1, 32) => {
                    i32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]) as f32
                        / 2_147_483_648.0
                }
                _ => bail!(
                    "{} uses an unsupported sample format (tag {tag}, {bits} bit)",
                    path.display()
                ),
            };
            sum += v;
        }
        // Averaging rather than summing, so a stereo track does not come out
        // twice as loud as a mono one and clip the model's input.
        mono.push(sum / channels as f32);
    }

    Ok(Audio {
        samples: mono,
        sample_rate: rate,
    })
}

/// A windowed-sinc low-pass, applied before rate conversion.
///
/// Resampling without this folds everything above the new Nyquist frequency
/// back into the audible band as noise, which speech models handle badly.
fn low_pass(samples: &[f32], rate: u32, cutoff_hz: f32) -> Vec<f32> {
    if samples.is_empty() || cutoff_hz >= rate as f32 / 2.0 {
        return samples.to_vec();
    }

    // 64 taps is enough for speech and cheap enough to run on a long recording.
    const HALF: isize = 32;
    let fc = cutoff_hz / rate as f32;
    let mut kernel = [0.0f32; (HALF * 2 + 1) as usize];
    let mut sum = 0.0f32;
    for (i, k) in kernel.iter_mut().enumerate() {
        let n = i as isize - HALF;
        let sinc = if n == 0 {
            2.0 * fc
        } else {
            let x = std::f32::consts::PI * n as f32;
            (2.0 * fc * x).sin() / x
        };
        // Hamming window, to keep the stop band clean.
        let w = 0.54 - 0.46 * (2.0 * std::f32::consts::PI * i as f32 / (HALF * 2) as f32).cos();
        *k = sinc * w;
        sum += *k;
    }
    for k in kernel.iter_mut() {
        *k /= sum;
    }

    let n = samples.len() as isize;
    let mut out = vec![0.0f32; samples.len()];
    for (i, o) in out.iter_mut().enumerate() {
        let mut acc = 0.0f32;
        for (j, k) in kernel.iter().enumerate() {
            // Clamp at the edges rather than wrapping or zero-padding, which
            // would put a click at the start and end of every track.
            let idx = (i as isize + j as isize - HALF).clamp(0, n - 1) as usize;
            acc += samples[idx] * k;
        }
        *o = acc;
    }
    out
}

/// Convert mono audio to the rate a speech model expects.
pub fn resample(samples: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || samples.is_empty() || from == 0 {
        return samples.to_vec();
    }

    let filtered = if to < from {
        // Leave a little headroom below the new Nyquist for the filter to roll
        // off in; speech carries nothing useful up there anyway.
        low_pass(samples, from, to as f32 * 0.45)
    } else {
        samples.to_vec()
    };

    let ratio = from as f64 / to as f64;
    let out_len = ((samples.len() as f64) / ratio).floor() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 * ratio;
        let idx = src.floor() as usize;
        let frac = (src - idx as f64) as f32;
        let a = filtered[idx.min(filtered.len() - 1)];
        let b = filtered[(idx + 1).min(filtered.len() - 1)];
        out.push(a + (b - a) * frac);
    }
    out
}

/// Builds the low-pass kernel used before rate conversion.
fn build_kernel(rate: u32, cutoff_hz: f32, half: usize) -> Vec<f32> {
    let fc = cutoff_hz / rate as f32;
    let n = half * 2 + 1;
    let mut kernel = vec![0.0f32; n];
    let mut sum = 0.0f32;
    for (i, k) in kernel.iter_mut().enumerate() {
        let m = i as isize - half as isize;
        let sinc = if m == 0 {
            2.0 * fc
        } else {
            let x = std::f32::consts::PI * m as f32;
            (2.0 * fc * x).sin() / x
        };
        let w = 0.54 - 0.46 * (2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32).cos();
        *k = sinc * w;
        sum += *k;
    }
    for k in kernel.iter_mut() {
        *k /= sum;
    }
    kernel
}

/// Rate conversion for audio that arrives in pieces.
///
/// [`resample`] is for a whole file. A live stream arrives in capture buffers,
/// and resampling each one on its own puts a discontinuity at every boundary,
/// because the filter has no idea what came before or after. Forty of those a
/// second is an audible buzz and a measurable drop in recognition quality.
///
/// This keeps enough history either side of each boundary that every output
/// sample is computed from a full filter's worth of input, and carries the
/// fractional read position across calls so the output rate does not drift.
pub struct StreamResampler {
    from: u32,
    to: u32,
    kernel: Vec<f32>,
    half: usize,
    /// Tail of the input already seen, kept so the next block has context.
    history: Vec<f32>,
    /// Stream position of `history[0]`.
    input_base: u64,
    /// Next output sample to produce, counted from the start of the stream.
    next_out: u64,
    /// Set while draining the tail, which relaxes the right-context rule.
    finishing: bool,
}

impl StreamResampler {
    pub fn new(from: u32, to: u32) -> Self {
        const HALF: usize = 32;
        let kernel = if to < from && from > 0 {
            build_kernel(from, to as f32 * 0.45, HALF)
        } else {
            Vec::new()
        };
        StreamResampler {
            from,
            to,
            kernel,
            half: HALF,
            history: Vec::new(),
            input_base: 0,
            next_out: 0,
            finishing: false,
        }
    }

    /// True when this is doing nothing, which is worth knowing before copying.
    pub fn is_passthrough(&self) -> bool {
        self.from == self.to || self.from == 0 || self.to == 0
    }

    /// Emit whatever is left at the end of the stream.
    ///
    /// During a stream the last half-window is held back because the filter has
    /// no input to its right yet. At the end there never will be any, and
    /// silence after the final sample is a fact rather than an assumption, so
    /// the remainder can be produced.
    pub fn flush(&mut self) -> Vec<f32> {
        if self.is_passthrough() {
            return Vec::new();
        }
        self.finishing = true;
        let out = self.push(&[]);
        self.finishing = false;
        out
    }

    /// Convert one block. Output length varies; that is expected.
    pub fn push(&mut self, input: &[f32]) -> Vec<f32> {
        if self.is_passthrough() {
            return input.to_vec();
        }
        if input.is_empty() && !self.finishing {
            return Vec::new();
        }

        let mut buffer = Vec::with_capacity(self.history.len() + input.len());
        buffer.extend_from_slice(&self.history);
        buffer.extend_from_slice(input);

        // Only the middle of the buffer has a full filter either side of it.
        let filtered: Vec<f32> = if self.kernel.is_empty() {
            buffer.clone()
        } else {
            (0..buffer.len())
                .map(|i| {
                    let mut acc = 0.0f32;
                    for (j, k) in self.kernel.iter().enumerate() {
                        let idx = i as isize + j as isize - self.half as isize;
                        if idx >= 0 && (idx as usize) < buffer.len() {
                            acc += buffer[idx as usize] * k;
                        }
                    }
                    acc
                })
                .collect()
        };

        // At the very start of the stream there is nothing to the left, and
        // silence before the first sample is not an assumption but a fact. Any
        // later block does need real context, or the boundary shows.
        let valid_start = if self.input_base == 0 {
            0
        } else {
            self.input_base + self.half as u64
        };
        let valid_end = if self.finishing {
            self.input_base + buffer.len() as u64
        } else {
            self.input_base + buffer.len().saturating_sub(self.half) as u64
        };

        let ratio = self.from as f64 / self.to as f64;
        let mut out = Vec::new();
        loop {
            let src = self.next_out as f64 * ratio;
            let idx = src.floor() as u64;
            // Needs the sample after it too, for the interpolation.
            if idx < valid_start || idx + 1 >= valid_end {
                break;
            }
            let local = (idx - self.input_base) as usize;
            if local >= filtered.len() {
                break;
            }
            let frac = (src - idx as f64) as f32;
            let a = filtered[local];
            let b = filtered[(local + 1).min(filtered.len() - 1)];
            out.push(a + (b - a) * frac);
            self.next_out += 1;
        }

        // Keep enough context for the next block, and remember where it starts.
        let keep = (self.half * 2 + 1).min(buffer.len());
        let dropped = buffer.len() - keep;
        self.history = buffer[dropped..].to_vec();
        self.input_base += dropped as u64;

        out
    }
}

/// Write mono 16-bit audio at 16 kHz, which is what the engines read.
pub fn write_wav_16k_mono(path: &Path, samples: &[f32]) -> Result<()> {
    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut w = BufWriter::new(file);

    let data_bytes = (samples.len() * 2) as u32;
    w.write_all(b"RIFF")?;
    w.write_all(&(36 + data_bytes).to_le_bytes())?;
    w.write_all(b"WAVE")?;
    w.write_all(b"fmt ")?;
    w.write_all(&16u32.to_le_bytes())?;
    w.write_all(&1u16.to_le_bytes())?; // PCM
    w.write_all(&1u16.to_le_bytes())?; // mono
    w.write_all(&TARGET_RATE.to_le_bytes())?;
    w.write_all(&(TARGET_RATE * 2).to_le_bytes())?; // byte rate
    w.write_all(&2u16.to_le_bytes())?; // block align
    w.write_all(&16u16.to_le_bytes())?; // bits
    w.write_all(b"data")?;
    w.write_all(&data_bytes.to_le_bytes())?;

    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32_767.0) as i16;
        w.write_all(&v.to_le_bytes())?;
    }
    w.flush()?;
    Ok(())
}

/// Read a recorded track and produce the temporary file a speech engine reads.
pub fn prepare_for_engine(source: &Path, destination: &Path) -> Result<Audio> {
    let audio = read_wav_mono(source)?;
    let resampled = resample(&audio.samples, audio.sample_rate, TARGET_RATE);
    write_wav_16k_mono(destination, &resampled)?;
    Ok(audio)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, hz: f32, secs: f32) -> Vec<f32> {
        let n = (rate as f32 * secs) as usize;
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * hz * i as f32 / rate as f32).sin() * 0.5)
            .collect()
    }

    /// Dominant frequency of a single clean tone.
    ///
    /// Zero crossings, but interpolated to find where each one actually fell.
    /// Counting whole samples quantises the answer to several hertz, which is
    /// larger than the error being looked for.
    fn peak_hz(samples: &[f32], rate: u32) -> f32 {
        let mut crossings: Vec<f64> = Vec::new();
        for (i, w) in samples.windows(2).enumerate() {
            if w[0] < 0.0 && w[1] >= 0.0 {
                let frac = (-w[0] as f64) / ((w[1] - w[0]) as f64);
                crossings.push(i as f64 + frac);
            }
        }
        if crossings.len() < 2 {
            return 0.0;
        }
        let span = crossings[crossings.len() - 1] - crossings[0];
        ((crossings.len() - 1) as f64 * rate as f64 / span) as f32
    }

    #[test]
    fn resampling_preserves_pitch() {
        let input = tone(48_000, 440.0, 1.0);
        let out = resample(&input, 48_000, TARGET_RATE);
        assert!(
            (out.len() as i64 - 16_000).abs() <= 2,
            "got {} samples",
            out.len()
        );
        let hz = peak_hz(&out, TARGET_RATE);
        assert!((hz - 440.0).abs() < 1.0, "recovered {hz} Hz");
    }

    #[test]
    fn resampling_removes_content_above_the_new_nyquist() {
        // 12 kHz cannot exist at 16 kHz and must not fold back as a lower tone.
        let input = tone(48_000, 12_000.0, 0.5);
        let out = resample(&input, 48_000, TARGET_RATE);
        // Measured away from the ends. A tone that begins at full amplitude is
        // a step, and a step genuinely contains low frequencies that the filter
        // is right to pass; real audio does not start that way.
        let interior = &out[200..out.len() - 200];
        let peak = interior.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak < 0.01, "aliased through at amplitude {peak}");
    }

    #[test]
    fn the_same_rate_is_left_alone() {
        let input = tone(16_000, 300.0, 0.25);
        let out = resample(&input, 16_000, 16_000);
        assert_eq!(out, input);
    }

    #[test]
    fn streaming_matches_resampling_the_whole_thing() {
        let input = tone(48_000, 440.0, 1.0);
        let whole = resample(&input, 48_000, TARGET_RATE);

        let mut streamer = StreamResampler::new(48_000, TARGET_RATE);
        let mut streamed = Vec::new();
        // Deliberately awkward sizes: capture buffers do not arrive in neat
        // multiples of anything.
        for chunk in input.chunks(479) {
            streamed.extend(streamer.push(chunk));
        }
        streamed.extend(streamer.flush());

        assert!(
            (streamed.len() as i64 - whole.len() as i64).abs() <= 2,
            "lengths differ: {} streamed against {} whole",
            streamed.len(),
            whole.len()
        );

        // Compare the interior, where both have full filter support.
        let n = streamed.len().min(whole.len());
        let worst = (200..n - 200)
            .map(|i| (streamed[i] - whole[i]).abs())
            .fold(0.0f32, f32::max);
        assert!(worst < 0.02, "streamed output diverged by {worst}");
    }

    #[test]
    fn streaming_keeps_pitch_across_block_boundaries() {
        let input = tone(48_000, 440.0, 2.0);
        let mut streamer = StreamResampler::new(48_000, TARGET_RATE);
        let mut out = Vec::new();
        for chunk in input.chunks(961) {
            out.extend(streamer.push(chunk));
        }
        let hz = peak_hz(&out[100..out.len() - 100], TARGET_RATE);
        assert!((hz - 440.0).abs() < 1.0, "recovered {hz} Hz");
    }

    #[test]
    fn streaming_at_the_same_rate_copies_through() {
        let mut streamer = StreamResampler::new(16_000, 16_000);
        assert!(streamer.is_passthrough());
        let input = tone(16_000, 300.0, 0.1);
        assert_eq!(streamer.push(&input), input);
    }

    #[test]
    fn silence_is_recognised() {
        let quiet = Audio {
            samples: vec![0.0; 1000],
            sample_rate: 16_000,
        };
        assert!(quiet.is_silent());
        let loud = Audio {
            samples: tone(16_000, 200.0, 0.1),
            sample_rate: 16_000,
        };
        assert!(!loud.is_silent());
    }

    #[test]
    fn a_written_file_reads_back_with_the_same_content() {
        let dir = std::env::temp_dir().join("ssr-transcribe-audio-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("round-trip.wav");
        let samples = tone(16_000, 440.0, 0.2);

        write_wav_16k_mono(&path, &samples).unwrap();
        let back = read_wav_mono(&path).unwrap();

        assert_eq!(back.sample_rate, TARGET_RATE);
        assert_eq!(back.samples.len(), samples.len());
        let hz = peak_hz(&back.samples, back.sample_rate);
        assert!((hz - 440.0).abs() < 1.0, "recovered {hz} Hz");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stereo_is_averaged_rather_than_summed() {
        // Two identical channels must not come back at twice the amplitude.
        let dir = std::env::temp_dir().join("ssr-transcribe-stereo-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("stereo.wav");

        let frames = 100usize;
        let mut body = Vec::new();
        for _ in 0..frames {
            for _ in 0..2 {
                body.extend_from_slice(&0.5f32.to_le_bytes());
            }
        }
        let mut file = Vec::new();
        file.extend_from_slice(b"RIFF");
        file.extend_from_slice(&(36u32 + body.len() as u32).to_le_bytes());
        file.extend_from_slice(b"WAVE");
        file.extend_from_slice(b"fmt ");
        file.extend_from_slice(&16u32.to_le_bytes());
        file.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
        file.extend_from_slice(&2u16.to_le_bytes()); // stereo
        file.extend_from_slice(&48_000u32.to_le_bytes());
        file.extend_from_slice(&(48_000u32 * 8).to_le_bytes());
        file.extend_from_slice(&8u16.to_le_bytes());
        file.extend_from_slice(&32u16.to_le_bytes());
        file.extend_from_slice(b"data");
        file.extend_from_slice(&(body.len() as u32).to_le_bytes());
        file.extend_from_slice(&body);
        std::fs::write(&path, &file).unwrap();

        let audio = read_wav_mono(&path).unwrap();
        assert_eq!(audio.samples.len(), frames);
        assert!(
            (audio.samples[0] - 0.5).abs() < 1e-6,
            "got {}",
            audio.samples[0]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
