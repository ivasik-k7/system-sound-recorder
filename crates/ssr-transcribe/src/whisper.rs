//! Running a whisper engine and reading back what it produced.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::engine::{Engine, EngineKind};
use crate::spawn::without_window;
use crate::transcript::Segment;

/// Options that change what the engine does rather than how it is invoked.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Language code such as `en`, or none to let the engine detect it.
    pub language: Option<String>,
    /// Model to use. A path for whisper.cpp, a name for the Python engine.
    pub model: Option<String>,
    /// Threads to use, or none for the engine's own default.
    pub threads: Option<u32>,
    /// Translate to English rather than transcribing in the spoken language.
    pub translate: bool,
}

/// What one run of the engine produced.
pub struct Recognised {
    pub segments: Vec<Segment>,
    pub language: String,
    pub model: Option<String>,
}

/// Transcribe a prepared 16 kHz mono WAV.
///
/// `work_dir` is where the engine is allowed to leave its output files.
pub fn run(
    engine: &Engine,
    audio: &Path,
    work_dir: &Path,
    options: &Options,
) -> Result<Recognised> {
    match engine.kind {
        EngineKind::WhisperCpp => run_whisper_cpp(engine, audio, work_dir, options),
        EngineKind::WhisperPython => run_whisper_python(engine, audio, work_dir, options),
    }
}

/// The JSON whisper.cpp writes with `--output-json`.
#[derive(Deserialize)]
struct CppOutput {
    #[serde(default)]
    result: CppResult,
    #[serde(default)]
    transcription: Vec<CppSegment>,
}

#[derive(Deserialize, Default)]
struct CppResult {
    #[serde(default)]
    language: String,
}

#[derive(Deserialize)]
struct CppSegment {
    offsets: CppOffsets,
    text: String,
}

#[derive(Deserialize)]
struct CppOffsets {
    from: i64,
    to: i64,
}

fn run_whisper_cpp(
    engine: &Engine,
    audio: &Path,
    work_dir: &Path,
    options: &Options,
) -> Result<Recognised> {
    let model = match options.model.clone() {
        Some(m) => m,
        None => engine
            .default_model()
            .map(|p| p.display().to_string())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "whisper.cpp needs a model file and none was found; \
                     set SSR_WHISPER_MODELS to the directory holding your ggml-*.bin files"
                )
            })?,
    };

    // The engine appends its own extension, so the prefix must not carry one.
    let prefix = work_dir.join("transcript");

    let mut command = Command::new(&engine.executable);
    command
        .arg("--model")
        .arg(&model)
        .arg("--file")
        .arg(audio)
        .arg("--output-json")
        .arg("--output-file")
        .arg(&prefix)
        // Without this the engine prints the transcript to the terminal as it
        // goes, which is noise when the caller is reading the JSON.
        .arg("--no-prints");

    if let Some(language) = &options.language {
        command.arg("--language").arg(language);
    }
    if let Some(threads) = options.threads {
        command.arg("--threads").arg(threads.to_string());
    }
    if options.translate {
        command.arg("--translate");
    }

    let output = without_window(&mut command)
        .output()
        .with_context(|| format!("running {}", engine.executable.display()))?;
    if !output.status.success() {
        bail!(
            "{} failed: {}",
            engine.executable.display(),
            last_meaningful_line(&String::from_utf8_lossy(&output.stderr))
        );
    }

    let json_path = prefix.with_extension("json");
    let body = std::fs::read_to_string(&json_path).with_context(|| {
        format!(
            "the engine reported success but wrote no transcript at {}",
            json_path.display()
        )
    })?;
    let parsed: CppOutput =
        serde_json::from_str(&body).context("the engine's JSON output could not be read")?;

    let segments = parsed
        .transcription
        .into_iter()
        .map(|s| Segment {
            // Offsets are milliseconds from the start of the audio.
            start: s.offsets.from as f64 / 1000.0,
            end: s.offsets.to as f64 / 1000.0,
            text: s.text,
            speaker: None,
        })
        .collect();

    Ok(Recognised {
        segments,
        language: if parsed.result.language.is_empty() {
            options.language.clone().unwrap_or_else(|| "unknown".into())
        } else {
            parsed.result.language
        },
        model: Some(model),
    })
}

/// The JSON the Python implementations write.
#[derive(Deserialize)]
struct PyOutput {
    #[serde(default)]
    language: String,
    #[serde(default)]
    segments: Vec<PySegment>,
}

#[derive(Deserialize)]
struct PySegment {
    start: f64,
    end: f64,
    text: String,
}

fn run_whisper_python(
    engine: &Engine,
    audio: &Path,
    work_dir: &Path,
    options: &Options,
) -> Result<Recognised> {
    let model = options.model.clone().unwrap_or_else(|| "base".to_string());

    let mut command = Command::new(&engine.executable);
    command
        .arg(audio)
        .arg("--model")
        .arg(&model)
        .arg("--output_format")
        .arg("json")
        .arg("--output_dir")
        .arg(work_dir)
        .arg("--task")
        .arg(if options.translate {
            "translate"
        } else {
            "transcribe"
        });

    if let Some(language) = &options.language {
        command.arg("--language").arg(language);
    }
    if let Some(threads) = options.threads {
        command.arg("--threads").arg(threads.to_string());
    }

    let output = without_window(&mut command)
        .output()
        .with_context(|| format!("running {}", engine.executable.display()))?;
    if !output.status.success() {
        bail!(
            "{} failed: {}",
            engine.executable.display(),
            last_meaningful_line(&String::from_utf8_lossy(&output.stderr))
        );
    }

    // It names the output after the input file rather than taking a prefix.
    let stem = audio
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "audio".into());
    let json_path = work_dir.join(format!("{stem}.json"));
    let body = std::fs::read_to_string(&json_path).with_context(|| {
        format!(
            "the engine reported success but wrote no transcript at {}",
            json_path.display()
        )
    })?;
    let parsed: PyOutput =
        serde_json::from_str(&body).context("the engine's JSON output could not be read")?;

    Ok(Recognised {
        segments: parsed
            .segments
            .into_iter()
            .map(|s| Segment {
                start: s.start,
                end: s.end,
                text: s.text,
                speaker: None,
            })
            .collect(),
        language: if parsed.language.is_empty() {
            options.language.clone().unwrap_or_else(|| "unknown".into())
        } else {
            parsed.language
        },
        model: Some(model),
    })
}

/// The last line of engine output that says anything.
///
/// These tools print progress bars and banners to standard error, so the tail
/// is where the actual complaint is, and the whole thing is too long to show.
fn last_meaningful_line(stderr: &str) -> String {
    stderr
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .unwrap_or("no output")
        .chars()
        .take(300)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whisper_cpp_json_becomes_segments_in_seconds() {
        let body = r#"{
          "result": { "language": "en" },
          "transcription": [
            { "offsets": { "from": 0, "to": 2500 }, "text": " Good morning." },
            { "offsets": { "from": 2500, "to": 4000 }, "text": " Shall we begin?" }
          ]
        }"#;
        let parsed: CppOutput = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.result.language, "en");
        assert_eq!(parsed.transcription.len(), 2);
        assert_eq!(parsed.transcription[1].offsets.from, 2500);
    }

    #[test]
    fn python_json_becomes_segments() {
        let body = r#"{
          "text": "Good morning.",
          "language": "en",
          "segments": [ { "start": 0.0, "end": 2.5, "text": " Good morning." } ]
        }"#;
        let parsed: PyOutput = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.language, "en");
        assert_eq!(parsed.segments[0].end, 2.5);
    }

    #[test]
    fn missing_fields_do_not_break_parsing() {
        // Engines vary between versions; absent fields must not be fatal.
        let parsed: CppOutput = serde_json::from_str("{}").unwrap();
        assert!(parsed.transcription.is_empty());
        assert!(parsed.result.language.is_empty());
    }

    #[test]
    fn the_error_shown_is_the_last_thing_the_engine_said() {
        let stderr = "whisper_init\n\nprogress 10%\nprogress 90%\nerror: failed to load model\n\n";
        assert_eq!(last_meaningful_line(stderr), "error: failed to load model");
        assert_eq!(last_meaningful_line("   \n\n"), "no output");
    }
}
