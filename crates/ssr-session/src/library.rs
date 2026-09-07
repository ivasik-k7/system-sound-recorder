//! The library of recordings already made.
//!
//! Sessions accumulate: each one is a dated folder holding a WAV per track and
//! a `session.json`. That is a good format to write and a poor one to browse,
//! so this reads the folder back into something the interface can show — what
//! was recorded, when, how long, how big, and whether the recorder finished
//! writing it.
//!
//! The last of those matters more than it sounds. A session whose folder has
//! WAV files but no manifest was interrupted: the machine lost power, or the
//! program was killed. The audio is usually still there and still playable, but
//! nothing recorded what the tracks were or that they were meant to line up. A
//! library that quietly listed it alongside the complete ones would be hiding
//! the single most important thing about it.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ssr_timeline::manifest::{Marker, SessionManifest};

/// One file inside a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTrack {
    pub name: String,
    pub file: String,
    /// Size on disk in bytes. Read from the filesystem rather than the
    /// manifest, so a truncated file shows its real size.
    pub size_bytes: u64,
    /// Length in seconds, from the manifest when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<f64>,
    pub sample_rate: u32,
    pub channels: u16,
    /// What was tapped, in words.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Fraction of the track the recorder had to manufacture as silence, 0..1.
    ///
    /// The number that says whether a recording is trustworthy. A track that is
    /// nine tenths manufactured silence recorded almost nothing, however
    /// plausible its length looks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silence_fraction: Option<f64>,
    /// Dropouts the audio engine reported during this track.
    pub discontinuities: u64,
}

/// One session on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySession {
    /// Full path to the session directory.
    pub dir: String,
    /// The directory's own name, which is the timestamp it was recorded at.
    pub name: String,
    /// When recording began, from the manifest, in UTC.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_utc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<f64>,
    pub tracks: Vec<LibraryTrack>,
    /// Total size of every file in the folder.
    pub size_bytes: u64,
    /// False when there is no readable manifest: the session was interrupted,
    /// or is being written right now.
    pub complete: bool,
    /// Why the manifest could not be read, when it could not be.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
    /// Warnings the recorder attached to the session.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// Cue points dropped while it was recorded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<Marker>,
    /// Modification time of the folder, as seconds since the Unix epoch. Used
    /// for ordering when there is no manifest to read a start time from.
    pub modified_unix: i64,
}

impl LibrarySession {
    /// Total number of dropouts across every track.
    pub fn discontinuities(&self) -> u64 {
        self.tracks.iter().map(|t| t.discontinuities).sum()
    }
}

/// The manifest file name inside a session directory.
pub const MANIFEST_NAME: &str = "session.json";

/// Read one session directory.
///
/// Returns `None` only when the path is not a directory. A directory with
/// nothing recognisable in it still comes back, marked incomplete, because an
/// empty session folder is itself worth seeing.
pub fn read_session(dir: &Path) -> Option<LibrarySession> {
    if !dir.is_dir() {
        return None;
    }

    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut size_bytes = 0u64;
    let mut wav_sizes: Vec<(String, u64)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            size_bytes += meta.len();
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name.to_lowercase().ends_with(".wav") {
                wav_sizes.push((file_name, meta.len()));
            }
        }
    }

    let modified_unix = std::fs::metadata(dir)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let manifest_path = dir.join(MANIFEST_NAME);
    let parsed: Result<SessionManifest, String> = std::fs::read_to_string(&manifest_path)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "no session.json: the recording was interrupted before it could be finished"
                    .to_string()
            } else {
                format!("session.json could not be read: {e}")
            }
        })
        .and_then(|raw| {
            serde_json::from_str::<SessionManifest>(&raw)
                .map_err(|e| format!("session.json is not readable: {e}"))
        });

    let size_of = |file: &str| -> u64 {
        wav_sizes
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(file))
            .map(|(_, s)| *s)
            .unwrap_or(0)
    };

    match parsed {
        Ok(manifest) => {
            let tracks = manifest
                .tracks
                .iter()
                .map(|t| {
                    let total = t.stats.frames_captured + t.stats.frames_silence_filled;
                    LibraryTrack {
                        name: t.name.clone(),
                        size_bytes: size_of(&t.file),
                        file: t.file.clone(),
                        duration_secs: Some(t.duration_secs),
                        sample_rate: t.sample_rate,
                        channels: t.channels,
                        source: Some(t.source.clone()),
                        silence_fraction: if total > 0 {
                            Some(t.stats.frames_silence_filled as f64 / total as f64)
                        } else {
                            None
                        },
                        discontinuities: t.stats.discontinuities,
                    }
                })
                .collect();
            Some(LibrarySession {
                dir: dir.display().to_string(),
                name,
                started_utc: Some(manifest.started_utc.clone()),
                duration_secs: Some(manifest.duration_secs),
                tracks,
                size_bytes,
                complete: true,
                problem: None,
                warnings: manifest.warnings.clone(),
                markers: manifest.markers.clone(),
                modified_unix,
            })
        }
        Err(problem) => {
            // No manifest to describe the tracks, so describe the files
            // instead. The format cannot be known without parsing the WAV
            // headers, and claiming a rate that might be wrong would be worse
            // than admitting the gap.
            let tracks = wav_sizes
                .iter()
                .map(|(file, size)| LibraryTrack {
                    name: file
                        .trim_end_matches(".wav")
                        .trim_end_matches(".WAV")
                        .to_string(),
                    file: file.clone(),
                    size_bytes: *size,
                    duration_secs: None,
                    sample_rate: 0,
                    channels: 0,
                    source: None,
                    silence_fraction: None,
                    discontinuities: 0,
                })
                .collect();
            Some(LibrarySession {
                dir: dir.display().to_string(),
                name,
                started_utc: None,
                duration_secs: None,
                tracks,
                size_bytes,
                complete: false,
                problem: Some(problem),
                warnings: Vec::new(),
                markers: Vec::new(),
                modified_unix,
            })
        }
    }
}

/// Read every session under a library root, newest first.
///
/// A missing root is not an error: it simply has no sessions in it yet, which
/// is the state every new installation starts in.
pub fn list_sessions(root: &Path, limit: usize) -> Vec<LibrarySession> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };

    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();

    // Sort by name before reading anything. Session folders are named for the
    // time they were recorded, so this is chronological, and it means a library
    // of a thousand sessions only parses the manifests of the newest few.
    dirs.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

    let mut out: Vec<LibrarySession> = dirs
        .iter()
        .take(limit)
        .filter_map(|d| read_session(d))
        .collect();

    // The names are usually chronological, but a session written with an
    // explicit --out name is not, so order the result by what it actually says.
    out.sort_by(|a, b| {
        b.started_utc
            .cmp(&a.started_utc)
            .then(b.modified_unix.cmp(&a.modified_unix))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ssr-library-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_session(root: &Path, name: &str, started: &str, silence: u64, captured: u64) {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("system.wav"), vec![0u8; 128]).unwrap();
        let manifest = format!(
            r#"{{
              "tool": "system-sound-recorder",
              "version": "0.1.0",
              "started_utc": "{started}",
              "ended_utc": "{started}",
              "duration_secs": 12.5,
              "session_t0_qpc_100ns": 0,
              "tracks": [
                {{
                  "name": "system",
                  "file": "system.wav",
                  "source": "loopback on default",
                  "sample_rate": 48000,
                  "channels": 2,
                  "duration_secs": 12.5,
                  "stats": {{
                    "frames_captured": {captured},
                    "frames_silence_filled": {silence},
                    "frames_overlap_trimmed": 0,
                    "blocks_dropped": 0,
                    "discontinuities": 3,
                    "untimed_blocks": 0,
                    "lookbehind_growths": 0,
                    "longest_gap_ms": 0.0
                  }}
                }}
              ],
              "warnings": ["one endpoint was busy"]
            }}"#
        );
        std::fs::write(dir.join(MANIFEST_NAME), manifest).unwrap();
    }

    #[test]
    fn a_finished_session_is_read_back_in_full() {
        let root = temp_root("complete");
        write_session(
            &root,
            "2026-09-07_10-00-00",
            "2026-09-07T08:00:00Z",
            100,
            900,
        );

        let sessions = list_sessions(&root, 50);
        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        assert!(s.complete);
        assert_eq!(s.name, "2026-09-07_10-00-00");
        assert_eq!(s.started_utc.as_deref(), Some("2026-09-07T08:00:00Z"));
        assert_eq!(s.duration_secs, Some(12.5));
        assert_eq!(s.tracks.len(), 1);
        assert_eq!(s.tracks[0].sample_rate, 48_000);
        assert_eq!(s.tracks[0].size_bytes, 128);
        assert_eq!(s.warnings, vec!["one endpoint was busy"]);
        assert_eq!(s.discontinuities(), 3);
        // 100 filled of 1000 total.
        let fraction = s.tracks[0].silence_fraction.unwrap();
        assert!((fraction - 0.1).abs() < 1e-9, "got {fraction}");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_interrupted_session_is_listed_and_says_so() {
        let root = temp_root("interrupted");
        let dir = root.join("2026-09-07_11-00-00");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("system.wav"), vec![0u8; 64]).unwrap();
        std::fs::write(dir.join("mic.wav"), vec![0u8; 32]).unwrap();

        let sessions = list_sessions(&root, 50);
        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        assert!(!s.complete);
        assert!(s.problem.as_ref().unwrap().contains("interrupted"));
        // The audio is still described, from the files themselves.
        assert_eq!(s.tracks.len(), 2);
        assert_eq!(s.size_bytes, 96);
        assert!(s.tracks.iter().all(|t| t.duration_secs.is_none()));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_corrupt_manifest_does_not_hide_the_session() {
        let root = temp_root("corrupt");
        let dir = root.join("2026-09-07_12-00-00");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("system.wav"), vec![0u8; 16]).unwrap();
        std::fs::write(dir.join(MANIFEST_NAME), "{ this is not json").unwrap();

        let sessions = list_sessions(&root, 50);
        assert_eq!(sessions.len(), 1);
        assert!(!sessions[0].complete);
        assert!(sessions[0]
            .problem
            .as_ref()
            .unwrap()
            .contains("not readable"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sessions_come_back_newest_first() {
        let root = temp_root("order");
        write_session(&root, "2026-09-05_09-00-00", "2026-09-05T07:00:00Z", 0, 10);
        write_session(&root, "2026-09-07_09-00-00", "2026-09-07T07:00:00Z", 0, 10);
        write_session(&root, "2026-09-06_09-00-00", "2026-09-06T07:00:00Z", 0, 10);

        let names: Vec<String> = list_sessions(&root, 50)
            .iter()
            .map(|s| s.name.clone())
            .collect();
        assert_eq!(
            names,
            vec![
                "2026-09-07_09-00-00",
                "2026-09-06_09-00-00",
                "2026-09-05_09-00-00"
            ]
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_limit_takes_the_newest_and_not_an_arbitrary_few() {
        let root = temp_root("limit");
        for day in 1..=5 {
            write_session(
                &root,
                &format!("2026-09-0{day}_09-00-00"),
                &format!("2026-09-0{day}T07:00:00Z"),
                0,
                10,
            );
        }
        let sessions = list_sessions(&root, 2);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].name, "2026-09-05_09-00-00");
        assert_eq!(sessions[1].name, "2026-09-04_09-00-00");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_library_that_does_not_exist_yet_is_simply_empty() {
        let missing = std::env::temp_dir().join("ssr-library-test-nonexistent-xyz");
        let _ = std::fs::remove_dir_all(&missing);
        assert!(list_sessions(&missing, 50).is_empty());
    }

    #[test]
    fn a_file_in_the_library_root_is_not_mistaken_for_a_session() {
        let root = temp_root("stray");
        std::fs::write(root.join("notes.txt"), "hello").unwrap();
        assert!(list_sessions(&root, 50).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
