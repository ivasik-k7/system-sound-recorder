//! Describing a session before it starts.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ssr_core::{ProcessMode, SourceSpec};
use ssr_timeline::sanitize_filename;

/// One thing the user wants recorded, in user-facing terms.
///
/// Distinct from [`ssr_core::SourceSpec`], which is the engine's view. Keeping
/// them apart means the interface layer never has to know about loopback flags
/// or process trees.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SourceRequest {
    /// Everything the machine plays, mixed.
    #[serde(rename_all = "camelCase")]
    System {
        /// `default`, an endpoint id, or part of an endpoint name.
        #[serde(default = "default_device")]
        device: String,
        /// Optional label; a sensible one is derived when absent.
        #[serde(default)]
        label: Option<String>,
    },
    /// One application's audio, on its own track.
    #[serde(rename_all = "camelCase")]
    App {
        /// Executable name or numeric process id.
        target: String,
        /// Record everything except this application instead.
        #[serde(default)]
        exclude: bool,
        #[serde(default)]
        label: Option<String>,
    },
    /// A microphone or other input endpoint.
    #[serde(rename_all = "camelCase")]
    Input {
        #[serde(default = "default_device")]
        device: String,
        #[serde(default)]
        label: Option<String>,
    },
}

fn default_device() -> String {
    "default".to_string()
}

impl SourceRequest {
    /// The track name, which is also the output file stem.
    pub fn track_name(&self) -> String {
        let raw = match self {
            SourceRequest::System { label: Some(l), .. }
            | SourceRequest::App { label: Some(l), .. }
            | SourceRequest::Input { label: Some(l), .. } => l.clone(),
            SourceRequest::System { .. } => "system".to_string(),
            SourceRequest::App {
                target, exclude, ..
            } => {
                let stem = target.trim_end_matches(".exe").trim_end_matches(".EXE");
                if *exclude {
                    format!("except-{stem}")
                } else {
                    stem.to_string()
                }
            }
            SourceRequest::Input { .. } => "mic".to_string(),
        };
        sanitize_filename(&raw)
    }

    /// Translate into the engine's own description of a source.
    pub fn to_spec(&self, sample_rate: u32, channels: u16) -> SourceSpec {
        match self {
            SourceRequest::System { device, .. } => SourceSpec::EndpointLoopback {
                device: device.clone(),
            },
            SourceRequest::Input { device, .. } => SourceSpec::Input {
                device: device.clone(),
            },
            SourceRequest::App {
                target, exclude, ..
            } => SourceSpec::Process {
                selector: target.clone(),
                mode: if *exclude {
                    ProcessMode::Exclude
                } else {
                    ProcessMode::Include
                },
                sample_rate,
                channels,
            },
        }
    }
}

/// Where the session's files go.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OutputTarget {
    /// A directory holding one WAV per track plus `session.json`.
    ///
    /// Writing two sessions to the same directory overwrites the first, so this
    /// is for when the caller has already chosen a name.
    Directory { path: PathBuf },
    /// A library folder. Each session gets a fresh, timestamped subdirectory of
    /// its own, so recordings accumulate instead of replacing one another.
    NewSessionIn { root: PathBuf },
    /// A single WAV file. Only valid when the session has exactly one track.
    File { path: PathBuf },
}

/// A session directory under `root` that does not exist yet.
///
/// The timestamp is to the second, which is not quite enough on its own: two
/// sessions started in the same second, or a clock that steps backwards, would
/// collide and one would overwrite the other. The suffix makes that impossible.
pub fn unique_session_dir(root: &std::path::Path) -> PathBuf {
    let stamp = ssr_core::local_timestamp();
    let first = root.join(&stamp);
    if !first.exists() {
        return first;
    }
    for n in 2..1000 {
        let candidate = root.join(format!("{stamp}-{n}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    // A thousand sessions in one second is not a real situation, but returning
    // something is better than looping.
    root.join(format!("{stamp}-{}", std::process::id()))
}

impl OutputTarget {
    /// Directory to write into, and a fixed file name when one was requested.
    ///
    /// Touches the filesystem for [`OutputTarget::NewSessionIn`], which has to
    /// look for an unused name.
    pub fn resolve(&self) -> (PathBuf, Option<String>) {
        match self {
            OutputTarget::Directory { path } => (path.clone(), None),
            OutputTarget::NewSessionIn { root } => (unique_session_dir(root), None),
            OutputTarget::File { path } => {
                let name = path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| "recording.wav".to_string());
                let dir = path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("."));
                (dir, Some(name))
            }
        }
    }
}

/// A complete description of a session, ready to start.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionConfig {
    pub sources: Vec<SourceRequest>,
    pub output: OutputTarget,
    /// Capture buffer size. Larger survives a busier machine.
    #[serde(default = "default_buffer_ms")]
    pub buffer_ms: u32,
    /// Sample rate for per-application tracks, the one case where the recorder
    /// chooses rather than reading it off an endpoint.
    #[serde(default = "default_rate")]
    pub sample_rate: u32,
    /// Channel count for per-application tracks.
    #[serde(default = "default_channels")]
    pub channels: u16,
    /// Stop automatically after this many seconds.
    #[serde(default)]
    pub duration_secs: Option<f64>,
}

fn default_buffer_ms() -> u32 {
    200
}
fn default_rate() -> u32 {
    48_000
}
fn default_channels() -> u16 {
    2
}

impl SessionConfig {
    /// Track names in order, with duplicates disambiguated.
    ///
    /// Two microphones or two loopback taps would otherwise collide on the same
    /// filename and silently overwrite each other.
    pub fn track_names(&self) -> Vec<String> {
        let mut seen: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
        self.sources
            .iter()
            .map(|s| {
                let base = s.track_name();
                let n = seen.entry(base.clone()).or_insert(0);
                *n += 1;
                if *n == 1 {
                    base
                } else {
                    format!("{base}-{n}")
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_track_names_are_disambiguated() {
        let cfg = SessionConfig {
            sources: vec![
                SourceRequest::Input {
                    device: "default".into(),
                    label: None,
                },
                SourceRequest::Input {
                    device: "other".into(),
                    label: None,
                },
            ],
            output: OutputTarget::Directory {
                path: PathBuf::from("out"),
            },
            buffer_ms: 200,
            sample_rate: 48_000,
            channels: 2,
            duration_secs: None,
        };
        assert_eq!(cfg.track_names(), vec!["mic", "mic-2"]);
    }

    #[test]
    fn an_excluded_app_gets_a_distinct_name() {
        let s = SourceRequest::App {
            target: "wfica32.exe".into(),
            exclude: true,
            label: None,
        };
        assert_eq!(s.track_name(), "except-wfica32");
    }

    #[test]
    fn a_library_root_produces_a_fresh_directory_each_time() {
        let root = std::env::temp_dir().join("ssr-session-dir-test");
        std::fs::create_dir_all(&root).unwrap();
        let target = OutputTarget::NewSessionIn { root: root.clone() };

        let (first, name) = target.resolve();
        assert!(name.is_none());
        assert!(first.starts_with(&root));
        std::fs::create_dir_all(&first).unwrap();

        // With the first one now on disk, the next must not collide with it
        // even though the timestamp is identical.
        let (second, _) = target.resolve();
        assert_ne!(first, second);
        assert!(!second.exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_single_file_target_splits_into_directory_and_name() {
        let t = OutputTarget::File {
            path: PathBuf::from("/tmp/session/take1.wav"),
        };
        let (dir, name) = t.resolve();
        assert_eq!(dir, PathBuf::from("/tmp/session"));
        assert_eq!(name.as_deref(), Some("take1.wav"));
    }
}
