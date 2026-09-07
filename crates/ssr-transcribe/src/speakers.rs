//! Telling one voice from another.
//!
//! There are two quite different problems here and the recorder is unusually
//! well placed on one of them.
//!
//! **Between channels** the answer is exact and free. A Citrix track and a
//! microphone track are, by construction, the far side and the near side of the
//! call. No guessing is involved, and this is the case that matters most.
//!
//! **Within one channel** the answer has to be guessed, because several people
//! are coming down one wire already mixed. What is done here is deliberately
//! modest: each utterance is reduced to a few characteristics of the voice, and
//! utterances that resemble each other are given the same label. It separates
//! two people who sound different, and it will merge two people who sound
//! alike. It is a convenience, not a transcript of who said what, and the
//! interface should not pretend otherwise.

use serde::{Deserialize, Serialize};

/// A voice, as far as this can tell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Speaker {
    /// Stable number within a channel, counting from one.
    pub index: usize,
    /// What to show: the channel's own name when it is the only voice on it,
    /// otherwise "Citrix, speaker 2" and so on.
    pub label: String,
    /// True when this came from grouping voices rather than from the channel.
    pub inferred: bool,
}

/// The measurable characteristics of a voice, as a vector to compare.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoicePrint {
    /// Fundamental frequency in hertz. The strongest single cue between a
    /// typically lower and a typically higher voice.
    pub pitch_hz: f32,
    /// Rate of sign changes, which rises with the brightness of a voice.
    pub zero_crossing_rate: f32,
    /// How much the pitch moves about, which is fairly personal.
    pub pitch_spread: f32,
}

impl VoicePrint {
    /// Distance between two voices, scaled so each cue counts comparably.
    ///
    /// Pitch is divided by 80 Hz because that is roughly the gap between two
    /// adult voices one would call different; the other two are already small
    /// numbers and are weighted to match.
    pub fn distance(&self, other: &VoicePrint) -> f32 {
        let dp = (self.pitch_hz - other.pitch_hz) / 80.0;
        let dz = (self.zero_crossing_rate - other.zero_crossing_rate) * 8.0;
        let ds = (self.pitch_spread - other.pitch_spread) / 60.0;
        (dp * dp + dz * dz + ds * ds).sqrt()
    }

    /// True when the utterance carried too little voiced sound to judge.
    pub fn is_uncertain(&self) -> bool {
        self.pitch_hz <= 0.0
    }
}

/// Estimate the fundamental frequency of a frame by autocorrelation.
///
/// Returns nothing for a frame with no clear periodicity, which is the right
/// answer for a consonant, a silence, or a burst of noise.
fn frame_pitch(frame: &[f32], sample_rate: u32) -> Option<f32> {
    // Human speech sits inside this range; looking outside it invites the
    // octave errors that autocorrelation is prone to.
    let min_hz = 60.0f32;
    let max_hz = 400.0f32;
    let min_lag = (sample_rate as f32 / max_hz) as usize;
    let max_lag = (sample_rate as f32 / min_hz) as usize;
    if frame.len() < max_lag * 2 {
        return None;
    }

    let energy: f32 = frame.iter().map(|s| s * s).sum();
    if energy < 1e-6 {
        return None;
    }

    let mut best_lag = 0usize;
    let mut best = 0.0f32;
    for lag in min_lag..=max_lag {
        let mut sum = 0.0f32;
        for i in 0..(frame.len() - lag) {
            sum += frame[i] * frame[i + lag];
        }
        let normalised = sum / (frame.len() - lag) as f32;
        if normalised > best {
            best = normalised;
            best_lag = lag;
        }
    }

    // A weak peak means the frame was not really periodic.
    let mean_energy = energy / frame.len() as f32;
    if best_lag == 0 || best < mean_energy * 0.3 {
        return None;
    }
    Some(sample_rate as f32 / best_lag as f32)
}

/// Measure a voice from one utterance.
pub fn voice_print(samples: &[f32], sample_rate: u32) -> VoicePrint {
    let frame = (sample_rate as usize * 40) / 1000; // 40 ms
    let mut pitches: Vec<f32> = Vec::new();
    let mut crossings = 0usize;
    let mut counted = 0usize;

    for window in samples.chunks(frame) {
        if window.len() < frame {
            break;
        }
        if let Some(p) = frame_pitch(window, sample_rate) {
            pitches.push(p);
        }
        for pair in window.windows(2) {
            if (pair[0] < 0.0) != (pair[1] < 0.0) {
                crossings += 1;
            }
        }
        counted += window.len();
    }

    let zcr = if counted == 0 {
        0.0
    } else {
        crossings as f32 / counted as f32
    };

    if pitches.is_empty() {
        return VoicePrint {
            pitch_hz: 0.0,
            zero_crossing_rate: zcr,
            pitch_spread: 0.0,
        };
    }

    // The median resists the octave errors that a mean would absorb.
    pitches.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = pitches[pitches.len() / 2];
    let spread = {
        let mean = pitches.iter().sum::<f32>() / pitches.len() as f32;
        let var =
            pitches.iter().map(|p| (p - mean) * (p - mean)).sum::<f32>() / pitches.len() as f32;
        var.sqrt()
    };

    VoicePrint {
        pitch_hz: median,
        zero_crossing_rate: zcr,
        pitch_spread: spread,
    }
}

/// Groups the utterances of one channel into voices as they arrive.
pub struct Diariser {
    /// Name of the channel these voices are on.
    channel: String,
    /// One centroid per voice found so far, with how many utterances it holds.
    voices: Vec<(VoicePrint, usize)>,
    /// How far apart two voices must be to be considered different people.
    threshold: f32,
    /// Whether to attempt this at all.
    enabled: bool,
}

impl Diariser {
    /// A diariser for one channel.
    ///
    /// `enabled` being false makes every utterance the channel itself, which is
    /// the right behaviour for a microphone with one person in front of it.
    pub fn new(channel: impl Into<String>, enabled: bool) -> Self {
        Diariser {
            channel: channel.into(),
            voices: Vec::new(),
            // Chosen so two clearly different voices separate while the same
            // person sounding tired does not become a second speaker.
            threshold: 1.15,
            enabled,
        }
    }

    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold;
        self
    }

    /// How many distinct voices have been heard on this channel.
    pub fn voice_count(&self) -> usize {
        self.voices.len()
    }

    /// Assign an utterance to a voice.
    pub fn assign(&mut self, print: &VoicePrint) -> Speaker {
        if !self.enabled || print.is_uncertain() {
            // Without a usable measurement, attributing this to a particular
            // person would be invention. It belongs to the channel.
            return Speaker {
                index: 1,
                label: self.channel.clone(),
                inferred: false,
            };
        }

        let mut best: Option<(usize, f32)> = None;
        for (i, (centroid, _)) in self.voices.iter().enumerate() {
            let d = centroid.distance(print);
            if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                best = Some((i, d));
            }
        }

        let index = match best {
            Some((i, d)) if d <= self.threshold => {
                // Fold the new measurement into the running average, so a voice
                // is described by everything heard of it rather than by
                // whichever utterance happened to come first.
                let (centroid, count) = &mut self.voices[i];
                let n = *count as f32;
                centroid.pitch_hz = (centroid.pitch_hz * n + print.pitch_hz) / (n + 1.0);
                centroid.zero_crossing_rate =
                    (centroid.zero_crossing_rate * n + print.zero_crossing_rate) / (n + 1.0);
                centroid.pitch_spread =
                    (centroid.pitch_spread * n + print.pitch_spread) / (n + 1.0);
                *count += 1;
                i + 1
            }
            _ => {
                self.voices.push((*print, 1));
                self.voices.len()
            }
        };

        Speaker {
            index,
            // Only worth qualifying once there is more than one voice; until
            // then "Citrix" reads better than "Citrix, speaker 1".
            label: if self.voices.len() > 1 {
                format!("{}, speaker {}", self.channel, index)
            } else {
                self.channel.clone()
            },
            inferred: self.voices.len() > 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 16_000;

    /// A voiced sound at a given pitch, with harmonics so it is not a bare sine.
    fn voice(pitch: f32, secs: f32, brightness: f32) -> Vec<f32> {
        let n = (RATE as f32 * secs) as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                let w = 2.0 * std::f32::consts::PI * pitch * t;
                (w.sin() + 0.5 * (2.0 * w).sin() + brightness * (4.0 * w).sin()) * 0.25
            })
            .collect()
    }

    #[test]
    fn pitch_is_recovered_from_a_voiced_sound() {
        let print = voice_print(&voice(120.0, 1.0, 0.2), RATE);
        assert!(
            (print.pitch_hz - 120.0).abs() < 12.0,
            "measured {} Hz",
            print.pitch_hz
        );
    }

    #[test]
    fn silence_yields_no_pitch_and_is_marked_uncertain() {
        let print = voice_print(&vec![0.0; RATE as usize], RATE);
        assert!(print.is_uncertain());
    }

    #[test]
    fn two_different_voices_are_further_apart_than_one_voice_twice() {
        let low_a = voice_print(&voice(110.0, 1.0, 0.2), RATE);
        let low_b = voice_print(&voice(114.0, 1.0, 0.2), RATE);
        let high = voice_print(&voice(220.0, 1.0, 0.5), RATE);

        assert!(
            low_a.distance(&low_b) < low_a.distance(&high),
            "same voice {:.2} was not closer than a different one {:.2}",
            low_a.distance(&low_b),
            low_a.distance(&high)
        );
    }

    #[test]
    fn a_single_voice_stays_one_speaker_named_after_the_channel() {
        let mut d = Diariser::new("Citrix", true);
        for pitch in [110.0, 116.0, 108.0, 113.0] {
            let print = voice_print(&voice(pitch, 0.8, 0.2), RATE);
            let speaker = d.assign(&print);
            assert_eq!(speaker.label, "Citrix");
        }
        assert_eq!(d.voice_count(), 1);
    }

    #[test]
    fn two_distinct_voices_become_two_speakers() {
        let mut d = Diariser::new("Citrix", true);
        let low = voice_print(&voice(105.0, 1.0, 0.15), RATE);
        let high = voice_print(&voice(240.0, 1.0, 0.6), RATE);

        d.assign(&low);
        d.assign(&high);
        d.assign(&low);

        assert_eq!(d.voice_count(), 2, "voices were not separated");
        assert!(d.assign(&high).label.contains("speaker"));
    }

    #[test]
    fn disabling_it_keeps_everything_on_the_channel() {
        let mut d = Diariser::new("Microphone", false);
        let low = voice_print(&voice(105.0, 1.0, 0.15), RATE);
        let high = voice_print(&voice(240.0, 1.0, 0.6), RATE);
        assert_eq!(d.assign(&low).label, "Microphone");
        assert_eq!(d.assign(&high).label, "Microphone");
        assert_eq!(d.voice_count(), 0);
    }

    #[test]
    fn an_unmeasurable_utterance_is_attributed_to_the_channel_not_invented() {
        let mut d = Diariser::new("Citrix", true);
        let noise = voice_print(&vec![0.0; RATE as usize], RATE);
        let speaker = d.assign(&noise);
        assert_eq!(speaker.label, "Citrix");
        assert!(!speaker.inferred);
    }
}
