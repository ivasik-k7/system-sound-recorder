//! The session manifest.
//!
//! A multitrack recording is only useful if you can tell later what each file
//! was, when it started relative to the others, and whether the recorder had to
//! paper over anything. That is what this file records.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::track::TrackStats;

/// One output file.
///
/// Deserializable as well as serializable: the manifest is not only written at
/// the end of a session but read back afterwards, which is what lets the
/// interface show a library of past recordings rather than a list of folders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackManifest {
    /// Track name, matching the file stem.
    pub name: String,
    /// File name, relative to the session directory.
    pub file: String,
    /// Human description of what was tapped.
    pub source: String,
    /// Resolved process id, for process loopback tracks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_secs: f64,
    /// Frames the device delivered, versus frames the recorder manufactured.
    pub stats: TrackStats,
    /// Set when the file grew past what a RIFF header can describe.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub size_overflow: bool,
}

/// A point in time someone marked while recording.
///
/// Recorded in the manifest rather than kept in the interface, because a cue
/// point is only worth anything later, in whatever tool the audio is opened
/// with. The offset is from the session origin, so it is the same instant on
/// every track in the session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Marker {
    /// Seconds from the start of the session.
    pub at_secs: f64,
    /// What the marker is for. Empty when it was dropped without a note.
    #[serde(default)]
    pub label: String,
}

/// Everything about one recording session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionManifest {
    pub tool: String,
    pub version: String,
    /// When recording began, as an RFC 3339 timestamp in UTC.
    pub started_utc: String,
    pub ended_utc: String,
    pub duration_secs: f64,
    /// The shared timeline origin. Every track's frame zero is this instant, so
    /// the files can be loaded into a DAW as-is and will line up.
    pub session_t0_qpc_100ns: i64,
    pub tracks: Vec<TrackManifest>,
    /// Anything the operator should know about this recording.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// Cue points dropped while recording, in the order they happened.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<Marker>,
}

impl SessionManifest {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
    }
}

/// Format the current instant as an RFC 3339 UTC timestamp.
pub fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    rfc3339_from_unix(secs)
}

/// Convert a Unix timestamp to `YYYY-MM-DDTHH:MM:SSZ`.
///
/// Hand-rolled rather than pulling in a date library, since the recorder needs
/// exactly this one conversion and nothing else.
pub fn rfc3339_from_unix(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let time_of_day = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        time_of_day / 3600,
        (time_of_day % 3600) / 60,
        time_of_day % 60
    )
}

/// Days since the Unix epoch to a proleptic Gregorian calendar date.
///
/// This is Howard Hinnant's `civil_from_days`, which is exact for any date the
/// calendar can express.
fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    // Shift the epoch to 0000-03-01 so leap days land at the end of the cycle.
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11], March-based
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// A filesystem-safe name derived from an arbitrary label.
pub fn sanitize_filename(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "track".to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_formats_correctly() {
        assert_eq!(rfc3339_from_unix(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn known_instants_match_the_reference_calendar() {
        assert_eq!(rfc3339_from_unix(1_788_784_496), "2026-09-07T12:34:56Z");
        assert_eq!(rfc3339_from_unix(1_788_006_896), "2026-08-29T12:34:56Z");
        assert_eq!(rfc3339_from_unix(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339_from_unix(4_102_444_799), "2099-12-31T23:59:59Z");
    }

    #[test]
    fn leap_day_is_handled() {
        // 2024-02-29T00:00:00Z
        assert_eq!(rfc3339_from_unix(1_709_164_800), "2024-02-29T00:00:00Z");
    }

    #[test]
    fn century_non_leap_year_is_handled() {
        // 1900 was not a leap year; 1900-03-01T00:00:00Z
        assert_eq!(rfc3339_from_unix(-2_203_891_200), "1900-03-01T00:00:00Z");
    }

    #[test]
    fn filenames_lose_path_separators_and_spaces() {
        assert_eq!(
            sanitize_filename("Speakers (Senary Audio)"),
            "Speakers-Senary-Audio"
        );
        assert_eq!(sanitize_filename("a/b\\c"), "a-b-c");
        assert_eq!(sanitize_filename("???"), "track");
    }
}
