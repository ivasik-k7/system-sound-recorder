//! Live status, sampled by whatever is displaying the session.

use serde::{Deserialize, Serialize};
use ssr_timeline::manifest::Marker;

/// Where a session is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecorderState {
    Starting,
    Recording,
    Stopping,
    Finished,
    Failed,
}

/// A moment-in-time view of one track.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackSnapshot {
    pub name: String,
    /// What is being tapped, in words.
    pub description: String,
    /// Output file name, relative to the session directory.
    pub file: String,
    pub sample_rate: u32,
    pub channels: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// Audio written so far, in seconds.
    pub duration_secs: f64,
    /// Loudest sample in the last interval, as a linear amplitude in 0..1.
    pub peak: f32,
    /// The same, in dBFS. Negative infinity is reported as -120.
    pub peak_db: f32,
    /// Root mean square of the last interval, in dBFS.
    pub rms_db: f32,
    /// Frames that came from the device.
    pub frames_captured: u64,
    /// Frames of silence manufactured to keep the timeline honest.
    pub frames_silence: u64,
    /// Dropouts the audio engine reported.
    pub discontinuities: u64,
}

/// A moment-in-time view of the whole session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSnapshot {
    pub state: RecorderState,
    /// Seconds since recording began.
    pub elapsed_secs: f64,
    /// Directory the session is being written to.
    pub output_dir: String,
    pub tracks: Vec<TrackSnapshot>,
    /// Anything the operator should know, accumulated as the session runs.
    pub warnings: Vec<String>,
    /// Cue points dropped so far, so the interface can show them on the
    /// timeline as they are made rather than only after the session ends.
    #[serde(default)]
    pub markers: Vec<Marker>,
}

impl SessionSnapshot {
    /// An empty snapshot, for before a session exists.
    pub fn idle() -> Self {
        SessionSnapshot {
            state: RecorderState::Finished,
            elapsed_secs: 0.0,
            output_dir: String::new(),
            tracks: Vec::new(),
            warnings: Vec::new(),
            markers: Vec::new(),
        }
    }
}

/// Convert a linear amplitude to dBFS, with a floor so the value stays finite.
pub fn to_db(amplitude: f32) -> f32 {
    if amplitude <= 0.000_001 {
        -120.0
    } else {
        (20.0 * amplitude.log10()).max(-120.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_scale_is_zero_db() {
        assert!((to_db(1.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn half_scale_is_about_minus_six_db() {
        assert!((to_db(0.5) + 6.0206).abs() < 1e-3);
    }

    #[test]
    fn silence_hits_the_floor_rather_than_negative_infinity() {
        assert_eq!(to_db(0.0), -120.0);
        assert!(to_db(0.0).is_finite());
    }
}
