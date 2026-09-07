//! Transcribing a whole recorded session.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::audio;
use crate::engine::Engine;
use crate::transcript::{self, Transcript, TranscriptFormat};
use crate::whisper;

/// What the caller wants transcribed, and where the result should go.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeRequest {
    /// A session directory containing `session.json` and its WAV files.
    pub session_dir: PathBuf,
    /// Where the transcripts go. Defaults to the session directory.
    #[serde(default)]
    pub output_dir: Option<PathBuf>,
    /// Track names to transcribe. Empty means every track.
    #[serde(default)]
    pub tracks: Vec<String>,
    /// Formats to write. Empty means plain text.
    #[serde(default)]
    pub formats: Vec<TranscriptFormat>,
    /// Language code, or none to let the engine decide.
    #[serde(default)]
    pub language: Option<String>,
    /// Model to use, overriding the engine's default.
    #[serde(default)]
    pub model: Option<String>,
    /// Translate into English instead of transcribing as spoken.
    #[serde(default)]
    pub translate: bool,
    /// Also write one file interleaving every track by time.
    #[serde(default)]
    pub combine: bool,
    /// Skip tracks that contain no audible sound.
    #[serde(default = "yes")]
    pub skip_silent: bool,
}

fn yes() -> bool {
    true
}

/// Progress, reported as each track is dealt with.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub track: String,
    /// Which track this is, counting from one.
    pub index: usize,
    pub total: usize,
    /// What is happening to it right now.
    pub stage: String,
}

/// What one track produced.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackOutcome {
    pub track: String,
    /// Files written for this track.
    pub files: Vec<PathBuf>,
    pub words: usize,
    /// Set when the track was passed over rather than transcribed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
    /// Set when the track failed. One bad track does not stop the rest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// What the whole run produced.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeResult {
    pub output_dir: PathBuf,
    pub engine: String,
    pub tracks: Vec<TrackOutcome>,
    /// Every file written, for showing or opening afterwards.
    pub files: Vec<PathBuf>,
}

/// The parts of a session manifest this needs.
#[derive(Deserialize)]
struct Manifest {
    #[serde(default)]
    tracks: Vec<ManifestTrack>,
}

#[derive(Deserialize)]
struct ManifestTrack {
    name: String,
    file: String,
    #[serde(default)]
    source: String,
}

/// Transcribe a session, writing the requested formats.
///
/// `on_progress` is called before each track, so a caller can show which one is
/// being worked on. Recognition is slow enough that saying nothing for minutes
/// would look like a hang.
pub fn transcribe_session(
    engine: &Engine,
    request: &TranscribeRequest,
    mut on_progress: impl FnMut(Progress),
) -> Result<TranscribeResult> {
    let manifest_path = request.session_dir.join("session.json");
    let body = std::fs::read_to_string(&manifest_path).with_context(|| {
        format!(
            "{} does not look like a session directory: no session.json",
            request.session_dir.display()
        )
    })?;
    let manifest: Manifest = serde_json::from_str(&body)
        .with_context(|| format!("reading {}", manifest_path.display()))?;

    if manifest.tracks.is_empty() {
        bail!("that session has no tracks");
    }

    let wanted: Vec<&ManifestTrack> = manifest
        .tracks
        .iter()
        .filter(|t| request.tracks.is_empty() || request.tracks.iter().any(|w| w == &t.name))
        .collect();
    if wanted.is_empty() {
        bail!("none of the named tracks are in that session");
    }

    let output_dir = request
        .output_dir
        .clone()
        .unwrap_or_else(|| request.session_dir.clone());
    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("creating {}", output_dir.display()))?;

    let formats = if request.formats.is_empty() {
        vec![TranscriptFormat::Txt]
    } else {
        request.formats.clone()
    };

    // Converted audio and engine output go in a directory of their own so the
    // session is not littered with intermediate files.
    let work_dir = output_dir.join(".ssr-transcribe");
    std::fs::create_dir_all(&work_dir)
        .with_context(|| format!("creating {}", work_dir.display()))?;

    let options = whisper::Options {
        language: request.language.clone(),
        model: request.model.clone(),
        threads: None,
        translate: request.translate,
    };

    let total = wanted.len();
    let mut outcomes = Vec::new();
    let mut all_files = Vec::new();
    let mut transcripts = Vec::new();

    for (i, track) in wanted.iter().enumerate() {
        let index = i + 1;
        let source_path = request.session_dir.join(&track.file);

        let report = |stage: &str, on_progress: &mut dyn FnMut(Progress)| {
            on_progress(Progress {
                track: track.name.clone(),
                index,
                total,
                stage: stage.to_string(),
            })
        };

        report("preparing audio", &mut on_progress);
        let prepared = work_dir.join(format!("{}-16k.wav", track.name));
        let decoded = match audio::prepare_for_engine(&source_path, &prepared) {
            Ok(a) => a,
            Err(e) => {
                outcomes.push(TrackOutcome {
                    track: track.name.clone(),
                    files: Vec::new(),
                    words: 0,
                    skipped: None,
                    error: Some(format!("{e:#}")),
                });
                continue;
            }
        };

        if request.skip_silent && decoded.is_silent() {
            outcomes.push(TrackOutcome {
                track: track.name.clone(),
                files: Vec::new(),
                words: 0,
                skipped: Some("the track is silent".into()),
                error: None,
            });
            continue;
        }

        report("recognising speech", &mut on_progress);
        let recognised = match whisper::run(engine, &prepared, &work_dir, &options) {
            Ok(r) => r,
            Err(e) => {
                outcomes.push(TrackOutcome {
                    track: track.name.clone(),
                    files: Vec::new(),
                    words: 0,
                    skipped: None,
                    error: Some(format!("{e:#}")),
                });
                continue;
            }
        };

        let transcript = Transcript {
            track: track.name.clone(),
            source: track.source.clone(),
            language: recognised.language,
            engine: engine.label.clone(),
            model: recognised.model,
            segments: recognised.segments,
        };

        report("writing files", &mut on_progress);
        let mut files = Vec::new();
        let mut failure = None;
        for format in &formats {
            match transcript::write(&transcript, &output_dir, &track.name, *format) {
                Ok(path) => files.push(path),
                Err(e) => failure = Some(format!("{e:#}")),
            }
        }

        outcomes.push(TrackOutcome {
            track: track.name.clone(),
            files: files.clone(),
            words: transcript.word_count(),
            skipped: None,
            error: failure,
        });
        all_files.extend(files);
        transcripts.push(transcript);
    }

    if request.combine && transcripts.len() > 1 {
        let combined = transcript::merge(&transcripts);
        let path = output_dir.join("conversation.md");
        std::fs::write(&path, combined).with_context(|| format!("writing {}", path.display()))?;
        all_files.push(path);
    }

    // The intermediate files have served their purpose; leaving them behind
    // would double the size of every session on disk.
    let _ = std::fs::remove_dir_all(&work_dir);

    Ok(TranscribeResult {
        output_dir,
        engine: engine.label.clone(),
        tracks: outcomes,
        files: all_files,
    })
}

/// Track names in a session, for offering a choice before starting.
pub fn session_tracks(session_dir: &Path) -> Result<Vec<String>> {
    let body = std::fs::read_to_string(session_dir.join("session.json"))
        .with_context(|| format!("reading the manifest in {}", session_dir.display()))?;
    let manifest: Manifest = serde_json::from_str(&body)?;
    Ok(manifest.tracks.into_iter().map(|t| t.name).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_session(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("session.json"),
            r#"{"tracks":[
                {"name":"system","file":"system.wav","source":"loopback on default"},
                {"name":"mic","file":"mic.wav","source":"input from default"}
            ]}"#,
        )
        .unwrap();
    }

    #[test]
    fn track_names_come_back_from_the_manifest() {
        let dir = std::env::temp_dir().join("ssr-job-tracks");
        write_session(&dir);
        assert_eq!(session_tracks(&dir).unwrap(), vec!["system", "mic"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_without_a_manifest_is_reported_clearly() {
        let dir = std::env::temp_dir().join("ssr-job-empty");
        std::fs::create_dir_all(&dir).unwrap();
        let err = session_tracks(&dir).unwrap_err().to_string();
        assert!(err.contains("manifest"), "unhelpful message: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn requesting_an_unknown_track_is_an_error_rather_than_silence() {
        let dir = std::env::temp_dir().join("ssr-job-unknown");
        write_session(&dir);
        let engine = Engine {
            kind: crate::engine::EngineKind::WhisperPython,
            label: "test".into(),
            executable: PathBuf::from("does-not-exist"),
            models: Vec::new(),
        };
        let request = TranscribeRequest {
            session_dir: dir.clone(),
            output_dir: None,
            tracks: vec!["nonexistent".into()],
            formats: Vec::new(),
            language: None,
            model: None,
            translate: false,
            combine: false,
            skip_silent: true,
        };
        let err = transcribe_session(&engine, &request, |_| {})
            .unwrap_err()
            .to_string();
        assert!(err.contains("none of the named tracks"), "got: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
