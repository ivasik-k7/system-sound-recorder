//! Finding where speech starts and stops in a stream of audio.
//!
//! Live transcription cannot hand a speech model an endless stream; it has to
//! give it utterances. Cutting on a fixed timer would slice words in half, so
//! the cut is made where the talking stops.
//!
//! The detector is energy based, with a noise floor it learns as it goes. That
//! matters because the floor is not the same for a headset microphone and a
//! Citrix session with comfort noise on the line, and a fixed threshold that
//! suits one will either deafen or trigger constantly on the other.

/// Audio is examined in frames of this length.
pub const FRAME_MS: usize = 20;

/// What the detector decided about the stream.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Speech began this many samples into the stream.
    SpeechStart { at: u64 },
    /// An utterance finished. The samples are the speech itself, with a little
    /// of the silence either side kept so the model has room to work.
    Utterance {
        start_sample: u64,
        end_sample: u64,
        samples: Vec<f32>,
    },
}

/// How the detector behaves. The defaults suit speech over a call.
#[derive(Debug, Clone)]
pub struct Config {
    pub sample_rate: u32,
    /// How far above the learned noise floor a frame must be to count as speech.
    pub threshold_over_floor: f32,
    /// A floor beneath which nothing counts as speech however quiet the room.
    pub absolute_floor: f32,
    /// Consecutive speech frames needed to open an utterance. Rejects clicks.
    pub onset_frames: usize,
    /// Silent frames tolerated before an utterance is closed. Long enough to
    /// ride over the pause in the middle of a sentence.
    pub hangover_frames: usize,
    /// An utterance is cut here even if the speaker has not paused, so that a
    /// monologue still produces text while it is happening.
    pub max_utterance_secs: f32,
    /// Utterances shorter than this are discarded as noise.
    pub min_utterance_secs: f32,
    /// Audio kept before the onset and after the end, so words are not clipped.
    pub padding_ms: usize,
}

impl Config {
    pub fn for_rate(sample_rate: u32) -> Self {
        Config {
            sample_rate,
            threshold_over_floor: 3.0,
            absolute_floor: 0.004,
            onset_frames: 3, // 60 ms
            // How long a pause has to last before an utterance is considered
            // finished. Every millisecond here is added to how long the speaker
            // waits to see their words. Long enough not to cut a breath in the
            // middle of a sentence, and no longer.
            hangover_frames: 22, // 440 ms
            // Short on purpose. This is the ceiling on how late a line can be:
            // nothing is recognised until the utterance closes, so twenty
            // seconds of uninterrupted speech meant twenty seconds of silence
            // from the transcript followed by a paragraph. Seven keeps a
            // monologue arriving in readable pieces while it is still being
            // spoken, and the recogniser splits each piece further by its own
            // sentence boundaries.
            max_utterance_secs: 7.0,
            min_utterance_secs: 0.35,
            padding_ms: 200,
        }
    }

    fn frame_len(&self) -> usize {
        (self.sample_rate as usize * FRAME_MS) / 1000
    }
}

/// Splits a stream of audio into utterances.
pub struct Detector {
    config: Config,
    frame_len: usize,
    /// Samples not yet forming a whole frame.
    pending: Vec<f32>,
    /// Everything since the utterance opened, including its leading padding.
    buffer: Vec<f32>,
    /// A rolling tail of recent silence, kept so an utterance can be given a
    /// run-up rather than starting on the first loud sample.
    lookbehind: Vec<f32>,
    speaking: bool,
    speech_run: usize,
    silence_run: usize,
    /// Learned noise level, as a root mean square amplitude.
    floor: f32,
    /// Samples consumed so far, which is the clock everything is timed against.
    position: u64,
    utterance_start: u64,
}

impl Detector {
    pub fn new(config: Config) -> Self {
        let frame_len = config.frame_len().max(1);
        Detector {
            frame_len,
            pending: Vec::with_capacity(frame_len),
            buffer: Vec::new(),
            lookbehind: Vec::new(),
            speaking: false,
            speech_run: 0,
            silence_run: 0,
            // Starts pessimistic and comes down; better to miss the first half
            // second than to transcribe a room's hum.
            floor: 0.01,
            position: 0,
            utterance_start: 0,
            config,
        }
    }

    fn padding_samples(&self) -> usize {
        (self.config.sample_rate as usize * self.config.padding_ms) / 1000
    }

    fn max_samples(&self) -> usize {
        (self.config.sample_rate as f32 * self.config.max_utterance_secs) as usize
    }

    fn min_samples(&self) -> usize {
        (self.config.sample_rate as f32 * self.config.min_utterance_secs) as usize
    }

    /// Feed audio in. Returns whatever the detector concluded from it.
    pub fn push(&mut self, samples: &[f32]) -> Vec<Event> {
        let mut events = Vec::new();
        let mut offset = 0;

        while offset < samples.len() {
            let want = self.frame_len - self.pending.len();
            let take = want.min(samples.len() - offset);
            self.pending
                .extend_from_slice(&samples[offset..offset + take]);
            offset += take;

            if self.pending.len() < self.frame_len {
                break;
            }
            let frame: Vec<f32> = std::mem::take(&mut self.pending);
            self.consume_frame(&frame, &mut events);
        }

        events
    }

    fn consume_frame(&mut self, frame: &[f32], events: &mut Vec<Event>) {
        let rms = rms(frame);
        let is_speech =
            rms > self.config.absolute_floor && rms > self.floor * self.config.threshold_over_floor;

        // The floor only follows quiet frames, and only slowly. Letting it
        // track speech would raise it until nothing registered at all.
        if !is_speech {
            self.floor = self.floor * 0.995 + rms * 0.005;
            self.floor = self.floor.max(1e-5);
        }

        if self.speaking {
            self.buffer.extend_from_slice(frame);
        } else {
            self.lookbehind.extend_from_slice(frame);
            let keep = self.padding_samples();
            if self.lookbehind.len() > keep {
                let excess = self.lookbehind.len() - keep;
                self.lookbehind.drain(0..excess);
            }
        }

        if is_speech {
            self.speech_run += 1;
            self.silence_run = 0;
            if !self.speaking && self.speech_run >= self.config.onset_frames {
                self.speaking = true;
                // Reach back so the opening consonant is not lost.
                self.buffer = std::mem::take(&mut self.lookbehind);
                let lead = self.buffer.len() as u64;
                self.utterance_start = self.position.saturating_sub(lead);
                events.push(Event::SpeechStart {
                    at: self.utterance_start,
                });
            }
        } else {
            self.speech_run = 0;
            if self.speaking {
                self.silence_run += 1;
                if self.silence_run >= self.config.hangover_frames {
                    self.close(events);
                }
            }
        }

        self.position += frame.len() as u64;

        if self.speaking && self.buffer.len() >= self.max_samples() {
            self.close(events);
        }
    }

    fn close(&mut self, events: &mut Vec<Event>) {
        let samples = std::mem::take(&mut self.buffer);
        self.speaking = false;
        self.silence_run = 0;
        self.speech_run = 0;
        self.lookbehind.clear();

        if samples.len() >= self.min_samples() {
            events.push(Event::Utterance {
                start_sample: self.utterance_start,
                end_sample: self.utterance_start + samples.len() as u64,
                samples,
            });
        }
    }

    /// Close anything still open, at the end of a stream.
    pub fn finish(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        if self.speaking {
            self.close(&mut events);
        }
        events
    }

    /// Seconds of audio consumed so far.
    pub fn position_secs(&self) -> f64 {
        self.position as f64 / self.config.sample_rate as f64
    }

    /// The noise level the detector has settled on, for display.
    pub fn noise_floor(&self) -> f32 {
        self.floor
    }
}

fn rms(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    let sum: f64 = frame.iter().map(|s| (*s as f64) * (*s as f64)).sum();
    (sum / frame.len() as f64).sqrt() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 16_000;

    fn silence(secs: f32) -> Vec<f32> {
        vec![0.0; (RATE as f32 * secs) as usize]
    }

    fn speech(secs: f32) -> Vec<f32> {
        // A tone modulated in amplitude stands in for speech well enough for
        // the detector, which only looks at energy.
        let n = (RATE as f32 * secs) as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                let envelope = 0.5 + 0.5 * (2.0 * std::f32::consts::PI * 4.0 * t).sin();
                (2.0 * std::f32::consts::PI * 200.0 * t).sin() * 0.3 * envelope
            })
            .collect()
    }

    fn utterances(events: &[Event]) -> Vec<&Event> {
        events
            .iter()
            .filter(|e| matches!(e, Event::Utterance { .. }))
            .collect()
    }

    #[test]
    fn silence_alone_produces_nothing() {
        let mut d = Detector::new(Config::for_rate(RATE));
        let mut events = d.push(&silence(3.0));
        events.extend(d.finish());
        assert!(utterances(&events).is_empty());
    }

    #[test]
    fn one_burst_of_speech_becomes_one_utterance() {
        let mut d = Detector::new(Config::for_rate(RATE));
        let mut events = d.push(&silence(1.0));
        events.extend(d.push(&speech(2.0)));
        events.extend(d.push(&silence(1.5)));
        events.extend(d.finish());

        let found = utterances(&events);
        assert_eq!(found.len(), 1, "got {} utterances", found.len());
    }

    #[test]
    fn a_pause_between_sentences_does_not_split_them() {
        // 300 ms is a breath, not the end of a turn.
        let mut d = Detector::new(Config::for_rate(RATE));
        let mut events = d.push(&silence(0.8));
        events.extend(d.push(&speech(1.0)));
        events.extend(d.push(&silence(0.3)));
        events.extend(d.push(&speech(1.0)));
        events.extend(d.push(&silence(1.5)));
        events.extend(d.finish());

        assert_eq!(utterances(&events).len(), 1);
    }

    #[test]
    fn a_real_gap_does_split_them() {
        let mut d = Detector::new(Config::for_rate(RATE));
        let mut events = d.push(&silence(0.8));
        events.extend(d.push(&speech(1.0)));
        events.extend(d.push(&silence(1.5)));
        events.extend(d.push(&speech(1.0)));
        events.extend(d.push(&silence(1.5)));
        events.extend(d.finish());

        assert_eq!(utterances(&events).len(), 2);
    }

    #[test]
    fn a_monologue_is_cut_rather_than_buffered_forever() {
        let mut config = Config::for_rate(RATE);
        config.max_utterance_secs = 2.0;
        let mut d = Detector::new(config);

        let mut events = d.push(&silence(0.5));
        events.extend(d.push(&speech(9.0)));
        events.extend(d.finish());

        let found = utterances(&events);
        assert!(
            found.len() >= 4,
            "a long stretch was not cut: {} pieces",
            found.len()
        );
    }

    #[test]
    fn a_click_is_not_an_utterance() {
        let mut d = Detector::new(Config::for_rate(RATE));
        let mut events = d.push(&silence(1.0));
        events.extend(d.push(&speech(0.02)));
        events.extend(d.push(&silence(1.5)));
        events.extend(d.finish());
        assert!(utterances(&events).is_empty());
    }

    #[test]
    fn timings_line_up_with_the_stream() {
        let mut d = Detector::new(Config::for_rate(RATE));
        let mut events = d.push(&silence(2.0));
        events.extend(d.push(&speech(1.0)));
        events.extend(d.push(&silence(1.5)));
        events.extend(d.finish());

        let Event::Utterance {
            start_sample,
            end_sample,
            ..
        } = utterances(&events)[0]
        else {
            panic!("expected an utterance")
        };
        let start = *start_sample as f64 / RATE as f64;
        let end = *end_sample as f64 / RATE as f64;
        // Speech begins at 2 s; padding may reach back a little before that.
        assert!((1.7..=2.1).contains(&start), "started at {start}");
        assert!(end > start + 0.8, "ended too soon at {end}");
    }

    #[test]
    fn a_noisy_line_does_not_trigger_constantly() {
        // Steady hum well above zero, but with no speech in it at all.
        let n = (RATE as f32 * 4.0) as usize;
        let hum: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 60.0 * i as f32 / RATE as f32).sin() * 0.02)
            .collect();

        let mut d = Detector::new(Config::for_rate(RATE));
        let mut events = d.push(&hum);
        events.extend(d.finish());
        assert!(
            utterances(&events).len() <= 1,
            "hum produced {} utterances",
            utterances(&events).len()
        );
    }

    #[test]
    fn audio_can_arrive_in_any_sized_pieces() {
        // Capture buffers do not align to frames, so the detector must not care.
        let signal = [silence(1.0), speech(1.5), silence(1.5)].concat();

        let mut whole = Detector::new(Config::for_rate(RATE));
        let mut a = whole.push(&signal);
        a.extend(whole.finish());

        let mut split = Detector::new(Config::for_rate(RATE));
        let mut b = Vec::new();
        for chunk in signal.chunks(577) {
            b.extend(split.push(chunk));
        }
        b.extend(split.finish());

        assert_eq!(utterances(&a).len(), utterances(&b).len());
    }
}
