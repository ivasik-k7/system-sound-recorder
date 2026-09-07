//! The unit of audio handed from a capture thread to the timeline layer.

/// Flags WASAPI raises on a captured buffer, normalised into something the
/// timeline layer can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PacketFlags {
    /// The device position jumped: samples were dropped somewhere upstream.
    pub discontinuity: bool,
    /// The buffer holds digital silence; the payload may be all zeros.
    pub silent: bool,
    /// The device timestamp is not trustworthy for this buffer.
    pub timestamp_error: bool,
}

/// A block of interleaved 32-bit float samples with the timestamp of its first frame.
#[derive(Debug)]
pub struct Packet {
    /// Performance-counter value of the first frame, in 100 ns units.
    ///
    /// This is the anchor that keeps independently clocked tracks aligned. It
    /// comes from `IAudioCaptureClient::GetBuffer`, not from a wall clock read
    /// after the fact.
    pub qpc_100ns: i64,
    /// Device stream position of the first frame, in frames.
    pub device_position: u64,
    /// Interleaved samples, `frames * channels` long.
    pub samples: Vec<f32>,
    /// Channel count, so `samples.len() / channels` gives the frame count.
    pub channels: usize,
    pub flags: PacketFlags,
}

impl Packet {
    pub fn frames(&self) -> usize {
        // A zero channel count should be impossible, but dividing by it would
        // be a panic rather than a wrong answer, so it is guarded.
        self.samples.len().checked_div(self.channels).unwrap_or(0)
    }
}
