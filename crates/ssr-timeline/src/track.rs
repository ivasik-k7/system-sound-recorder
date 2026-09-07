//! Placing captured audio on a shared session timeline.
//!
//! Two problems are solved here, and both of them are invisible until a
//! recording is played back next to another one.
//!
//! **Idle gaps.** A loopback stream on a quiet endpoint delivers no buffers at
//! all. Appending packets as they arrive would splice out every silence, so a
//! ten minute recording of an occasional notification sound would come back as
//! four seconds of beeps. Silence has to be manufactured to fill the gap.
//!
//! **Independent clocks.** Each endpoint counts time with its own oscillator, so
//! two tracks recorded side by side drift apart. Every buffer carries a
//! performance-counter timestamp, and placing buffers by that timestamp rather
//! than by arrival order keeps tracks locked to one another.

use anyhow::Result;

/// Where a track writes its audio.
///
/// An abstraction rather than a concrete file so the placement logic above can
/// be tested without touching the disk or the audio stack.
pub trait Sink {
    fn write_samples(&mut self, samples: &[f32]) -> Result<()>;
    fn write_silence(&mut self, frames: u64) -> Result<()>;
}

impl Sink for crate::wav::WavWriter {
    fn write_samples(&mut self, samples: &[f32]) -> Result<()> {
        crate::wav::WavWriter::write_samples(self, samples)
    }
    fn write_silence(&mut self, frames: u64) -> Result<()> {
        crate::wav::WavWriter::write_silence(self, frames)
    }
}

/// One block of audio with the timestamp of its first frame.
#[derive(Debug, Clone)]
pub struct TimedBlock {
    /// Performance counter value of the first frame, in 100 ns units.
    pub qpc_100ns: i64,
    pub samples: Vec<f32>,
    pub channels: usize,
    /// WASAPI reported that samples were lost before this block.
    pub discontinuity: bool,
    /// The timestamp on this block cannot be trusted.
    pub timestamp_error: bool,
}

impl TimedBlock {
    pub fn frames(&self) -> usize {
        // A zero channel count should be impossible, but dividing by it would
        // be a panic rather than a wrong answer, so it is guarded.
        self.samples.len().checked_div(self.channels).unwrap_or(0)
    }
}

/// What happened to a track over the course of a session.
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct TrackStats {
    /// Frames that came from the device.
    pub frames_captured: u64,
    /// Frames of silence manufactured to keep the timeline honest.
    pub frames_silence_filled: u64,
    /// Frames trimmed from the front of a block that overlapped what was
    /// already written.
    pub frames_overlap_trimmed: u64,
    /// Blocks discarded entirely because they landed wholly in the past.
    pub blocks_dropped: u64,
    /// Times the device reported dropped samples upstream of the recorder.
    pub discontinuities: u64,
    /// Blocks whose timestamp was flagged unreliable and so were appended
    /// rather than placed.
    pub untimed_blocks: u64,
    /// Times the silence fill had to back off because it had run ahead of a
    /// buffer still in flight. Repeated growth points at a stream that delivers
    /// far later than its buffer size suggests.
    pub lookbehind_growths: u64,
    /// Longest single gap that had to be filled, in milliseconds.
    pub longest_gap_ms: f64,
}

/// Writes one track, keeping it aligned to the session timeline.
pub struct Track<S: Sink> {
    name: String,
    sink: S,
    sample_rate: u32,
    channels: usize,
    /// Session origin, shared by every track, in 100 ns performance counter units.
    session_t0_100ns: i64,
    frames_written: u64,
    /// How far behind the present to stop filling silence.
    ///
    /// Audio arrives in buffers, so the most recent few tens of milliseconds are
    /// always still in flight. Filling silence right up to the current instant
    /// races those buffers, and a buffer that arrives after its place has
    /// already been filled has to be trimmed, which drags the start of a sound
    /// earlier than it happened. This value is what prevents that, and it grows
    /// itself when it turns out to have been too small.
    silence_lookbehind_100ns: i64,
    /// Ceiling on that growth, so a pathological stream cannot push the write
    /// head arbitrarily far into the past.
    max_lookbehind_100ns: i64,
    stats: TrackStats,
}

impl<S: Sink> Track<S> {
    pub fn new(
        name: impl Into<String>,
        sink: S,
        sample_rate: u32,
        channels: usize,
        session_t0_100ns: i64,
    ) -> Self {
        Track {
            name: name.into(),
            sink,
            sample_rate,
            channels,
            session_t0_100ns,
            frames_written: 0,
            // 300 ms comfortably exceeds the buffer sizes the engine requests.
            silence_lookbehind_100ns: 3_000_000,
            max_lookbehind_100ns: 20_000_000,
            stats: TrackStats::default(),
        }
    }

    /// Set the initial distance behind the present at which silence stops.
    ///
    /// Worth raising when the capture buffers are large, since a buffer cannot
    /// arrive sooner than the time it takes to fill.
    pub fn with_lookbehind(mut self, lookbehind_100ns: i64) -> Self {
        self.silence_lookbehind_100ns =
            lookbehind_100ns.clamp(1_000_000, self.max_lookbehind_100ns);
        self
    }

    /// Current lookbehind, in 100 ns units. Exposed for the manifest.
    pub fn lookbehind_100ns(&self) -> i64 {
        self.silence_lookbehind_100ns
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn stats(&self) -> TrackStats {
        self.stats
    }

    pub fn frames_written(&self) -> u64 {
        self.frames_written
    }

    pub fn duration_secs(&self) -> f64 {
        self.frames_written as f64 / self.sample_rate as f64
    }

    pub fn sink_mut(&mut self) -> &mut S {
        &mut self.sink
    }

    pub fn into_sink(self) -> S {
        self.sink
    }

    /// Frame index on the session timeline for a given timestamp.
    ///
    /// Computed in `i128` because 100 ns units times a sample rate overflows
    /// `i64` after a few hours of uptime.
    fn frame_at(&self, qpc_100ns: i64) -> i64 {
        let delta = qpc_100ns as i128 - self.session_t0_100ns as i128;
        ((delta * self.sample_rate as i128) / 10_000_000i128) as i64
    }

    /// Place a captured block on the timeline.
    pub fn push(&mut self, block: &TimedBlock) -> Result<()> {
        if block.discontinuity {
            self.stats.discontinuities += 1;
        }
        let frames = block.frames();
        if frames == 0 {
            return Ok(());
        }

        // Without a trustworthy timestamp the only safe thing is to append and
        // accept that this block may sit a few milliseconds off.
        if block.timestamp_error {
            self.stats.untimed_blocks += 1;
            self.sink.write_samples(&block.samples)?;
            self.frames_written += frames as u64;
            self.stats.frames_captured += frames as u64;
            return Ok(());
        }

        let target = self.frame_at(block.qpc_100ns).max(0) as u64;

        if target > self.frames_written {
            let gap = target - self.frames_written;
            self.note_gap(gap);
            self.sink.write_silence(gap)?;
            self.frames_written += gap;
            self.stats.frames_silence_filled += gap;
            self.sink.write_samples(&block.samples)?;
            self.frames_written += frames as u64;
            self.stats.frames_captured += frames as u64;
            return Ok(());
        }

        // The block starts at or before the write head, so its leading frames
        // duplicate audio already on disk.
        let overlap = (self.frames_written - target) as usize;
        if overlap >= frames {
            self.stats.blocks_dropped += 1;
            return Ok(());
        }
        if overlap > 0 {
            self.stats.frames_overlap_trimmed += overlap as u64;
            // The fill got ahead of a buffer that was still in flight, so the
            // lookbehind was too small for this stream. Grow it past the
            // overshoot with a margin, and the same sound will not be clipped
            // at its start again.
            let overshoot = (overlap as i64 * 10_000_000) / self.sample_rate as i64;
            self.silence_lookbehind_100ns =
                (self.silence_lookbehind_100ns + overshoot * 2).min(self.max_lookbehind_100ns);
            self.stats.lookbehind_growths += 1;
        }
        let kept = &block.samples[overlap * self.channels..];
        self.sink.write_samples(kept)?;
        let kept_frames = (frames - overlap) as u64;
        self.frames_written += kept_frames;
        self.stats.frames_captured += kept_frames;
        Ok(())
    }

    /// Extend the track with silence up to shortly before `now`.
    ///
    /// Called on a timer while a stream is idle, which is the only way an idle
    /// loopback stream contributes anything to its file at all.
    pub fn fill_idle(&mut self, now_qpc_100ns: i64) -> Result<()> {
        let safe_point = now_qpc_100ns - self.silence_lookbehind_100ns;
        let target = self.frame_at(safe_point);
        if target <= 0 {
            return Ok(());
        }
        let target = target as u64;
        if target <= self.frames_written {
            return Ok(());
        }
        let gap = target - self.frames_written;
        self.note_gap(gap);
        self.sink.write_silence(gap)?;
        self.frames_written += gap;
        self.stats.frames_silence_filled += gap;
        Ok(())
    }

    /// Extend the track with silence right up to `end`, ignoring the lookbehind.
    ///
    /// Called once when a session stops, so that every track in the session ends
    /// at the same instant and therefore has the same length. Without this a
    /// track that fell silent before the stop would come out short and appear
    /// to have been trimmed from the end.
    pub fn finalize_to(&mut self, end_qpc_100ns: i64) -> Result<()> {
        let target = self.frame_at(end_qpc_100ns);
        if target <= 0 {
            return Ok(());
        }
        let target = target as u64;
        if target <= self.frames_written {
            return Ok(());
        }
        let gap = target - self.frames_written;
        self.sink.write_silence(gap)?;
        self.frames_written += gap;
        self.stats.frames_silence_filled += gap;
        Ok(())
    }

    fn note_gap(&mut self, frames: u64) {
        let ms = frames as f64 * 1000.0 / self.sample_rate as f64;
        if ms > self.stats.longest_gap_ms {
            self.stats.longest_gap_ms = ms;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Recorder {
        /// Every write in order: `Ok(n)` for audio frames, `Err(n)` for silence.
        events: Vec<(bool, u64)>,
        channels: usize,
    }

    impl Sink for Recorder {
        fn write_samples(&mut self, samples: &[f32]) -> Result<()> {
            self.events
                .push((true, (samples.len() / self.channels) as u64));
            Ok(())
        }
        fn write_silence(&mut self, frames: u64) -> Result<()> {
            self.events.push((false, frames));
            Ok(())
        }
    }

    fn block(qpc: i64, frames: usize) -> TimedBlock {
        TimedBlock {
            qpc_100ns: qpc,
            samples: vec![0.25; frames * 2],
            channels: 2,
            discontinuity: false,
            timestamp_error: false,
        }
    }

    fn track() -> Track<Recorder> {
        Track::new(
            "t",
            Recorder {
                channels: 2,
                ..Default::default()
            },
            48_000,
            2,
            0,
        )
    }

    #[test]
    fn contiguous_blocks_produce_no_silence() {
        let mut t = track();
        // 480 frames at 48 kHz is exactly 10 ms, or 100_000 units of 100 ns.
        t.push(&block(0, 480)).unwrap();
        t.push(&block(100_000, 480)).unwrap();
        t.push(&block(200_000, 480)).unwrap();
        assert_eq!(t.frames_written(), 1440);
        assert_eq!(t.stats().frames_silence_filled, 0);
        assert!(t.sink.events.iter().all(|(is_audio, _)| *is_audio));
    }

    #[test]
    fn a_gap_is_filled_with_exactly_the_missing_frames() {
        let mut t = track();
        t.push(&block(0, 480)).unwrap();
        // Next block starts at 500 ms, so 24000 frames should exist by then and
        // 23520 of them must be manufactured.
        t.push(&block(5_000_000, 480)).unwrap();
        assert_eq!(t.stats().frames_silence_filled, 24_000 - 480);
        assert_eq!(t.frames_written(), 24_000 + 480);
        assert!((t.stats().longest_gap_ms - 490.0).abs() < 0.01);
    }

    #[test]
    fn a_late_starting_track_gets_leading_silence() {
        let mut t = track();
        // The source opened two seconds after the session began.
        t.push(&block(20_000_000, 480)).unwrap();
        assert_eq!(t.stats().frames_silence_filled, 96_000);
        assert_eq!(t.frames_written(), 96_480);
    }

    #[test]
    fn overlapping_blocks_are_trimmed_not_duplicated() {
        let mut t = track();
        t.push(&block(0, 480)).unwrap();
        // Claims to start at 5 ms, but 10 ms is already written: 240 frames of
        // this block are already on disk.
        t.push(&block(50_000, 480)).unwrap();
        assert_eq!(t.stats().frames_overlap_trimmed, 240);
        assert_eq!(t.frames_written(), 720);
    }

    #[test]
    fn an_overlap_makes_the_fill_back_off_next_time() {
        let mut t = track();
        let before = t.lookbehind_100ns();
        t.push(&block(0, 480)).unwrap();
        // 240 frames of overlap is 5 ms, so the lookbehind should grow by 10 ms.
        t.push(&block(50_000, 480)).unwrap();
        assert_eq!(t.stats().lookbehind_growths, 1);
        assert_eq!(t.lookbehind_100ns(), before + 100_000);
    }

    #[test]
    fn lookbehind_growth_is_capped() {
        let mut t = track().with_lookbehind(19_500_000);
        t.push(&block(0, 48_000)).unwrap();
        // A whole second of overlap would otherwise push it far past the cap.
        t.push(&block(0, 96_000)).unwrap();
        assert_eq!(t.lookbehind_100ns(), 20_000_000);
    }

    #[test]
    fn a_configured_lookbehind_moves_the_fill_boundary() {
        let mut t = track().with_lookbehind(5_000_000); // 500 ms
        t.push(&block(0, 480)).unwrap();
        // One second in, with a 500 ms lookbehind, silence reaches 500 ms.
        t.fill_idle(10_000_000).unwrap();
        assert_eq!(t.frames_written(), 24_000);
    }

    #[test]
    fn a_block_wholly_in_the_past_is_dropped() {
        let mut t = track();
        t.push(&block(0, 4800)).unwrap(); // 100 ms
        t.push(&block(100_000, 480)).unwrap(); // starts at 10 ms, all stale
        assert_eq!(t.stats().blocks_dropped, 1);
        assert_eq!(t.frames_written(), 4800);
    }

    #[test]
    fn idle_fill_stops_short_of_the_present() {
        let mut t = track();
        t.push(&block(0, 480)).unwrap();
        // One second in. The 300 ms lookbehind means silence is written only up
        // to the 700 ms mark.
        t.fill_idle(10_000_000).unwrap();
        assert_eq!(t.frames_written(), 33_600);
        assert_eq!(t.stats().frames_silence_filled, 33_600 - 480);
    }

    #[test]
    fn idle_fill_never_rewinds() {
        let mut t = track();
        t.push(&block(0, 48_000)).unwrap(); // a full second already written
        t.fill_idle(10_000_000).unwrap();
        assert_eq!(t.frames_written(), 48_000);
        assert_eq!(t.stats().frames_silence_filled, 0);
    }

    #[test]
    fn finalize_pads_every_track_to_the_same_length() {
        let t0 = 500i64;
        let end = t0 + 20_000_000; // two seconds later
        let mut busy = Track::new(
            "a",
            Recorder {
                channels: 2,
                ..Default::default()
            },
            48_000,
            2,
            t0,
        );
        let mut quiet = Track::new(
            "b",
            Recorder {
                channels: 2,
                ..Default::default()
            },
            48_000,
            2,
            t0,
        );

        busy.push(&block(t0 + 19_000_000, 480)).unwrap();
        quiet.push(&block(t0, 480)).unwrap();

        busy.finalize_to(end).unwrap();
        quiet.finalize_to(end).unwrap();
        assert_eq!(busy.frames_written(), 96_000);
        assert_eq!(quiet.frames_written(), 96_000);
    }

    #[test]
    fn untrustworthy_timestamps_fall_back_to_appending() {
        let mut t = track();
        let mut b = block(999_999_999, 480);
        b.timestamp_error = true;
        t.push(&b).unwrap();
        assert_eq!(t.frames_written(), 480);
        assert_eq!(t.stats().untimed_blocks, 1);
        assert_eq!(t.stats().frames_silence_filled, 0);
    }

    #[test]
    fn two_tracks_sharing_an_origin_stay_aligned() {
        // The point of the whole module: a track that starts late and a track
        // that starts on time must agree on where a given instant lands.
        let t0 = 1_234_567_890i64;
        let mut early = Track::new(
            "a",
            Recorder {
                channels: 2,
                ..Default::default()
            },
            48_000,
            2,
            t0,
        );
        let mut late = Track::new(
            "b",
            Recorder {
                channels: 2,
                ..Default::default()
            },
            48_000,
            2,
            t0,
        );

        early.push(&block(t0, 480)).unwrap();
        // Both receive a block stamped exactly five seconds after the origin.
        let mark = t0 + 50_000_000;
        early.push(&block(mark, 480)).unwrap();
        late.push(&block(mark, 480)).unwrap();

        // The five second mark must sit at frame 240000 in both files.
        assert_eq!(early.frames_written() - 480, 240_000);
        assert_eq!(late.frames_written() - 480, 240_000);
    }
}
