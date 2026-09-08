//! A speech engine held open between utterances.
//!
//! The command line whisper program is a one-shot. Each call pays for starting
//! Python, importing torch and reading the model from disk before it looks at
//! any audio: five to fifteen seconds, whether the utterance is two seconds or
//! twenty. During a conversation that is not a delay, it is a different mode of
//! operation, and it is why transcription felt like something the machine was
//! visibly doing rather than something happening quietly.
//!
//! This keeps one interpreter alive with the model already in memory and feeds
//! it audio down a pipe. The first utterance still waits for the model to load;
//! every one after it is the recognition alone.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;

use crate::spawn::without_window;
use crate::transcript::Segment;
use crate::whisper::{Options, Recognised};

/// The worker, embedded so there is nothing to install alongside the recorder.
const WORKER_SOURCE: &str = include_str!("whisper_worker.py");

/// How long to wait for the model to be ready.
///
/// Generous on purpose. The first run downloads the model, which is well over a
/// hundred megabytes, and killing it partway leaves a corrupt cache and a
/// confusing error. Every later start reads from that cache in a few seconds.
const READY_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// How long one utterance may take before the worker is assumed wedged.
const JOB_TIMEOUT: Duration = Duration::from_secs(120);

/// The last few lines the worker wrote to its error stream.
///
/// Kept because a failure inside Python arrives there and nowhere else; without
/// it a broken install shows up as "the worker stopped answering".
#[derive(Clone, Default)]
struct Diagnostics(Arc<Mutex<Vec<String>>>);

impl Diagnostics {
    fn push(&self, line: String) {
        if let Ok(mut lines) = self.0.lock() {
            lines.push(line);
            // Only the tail is useful, and a download progress bar can produce
            // thousands of lines.
            let len = lines.len();
            if len > 40 {
                lines.drain(0..len - 40);
            }
        }
    }

    fn tail(&self) -> String {
        self.0
            .lock()
            .map(|lines| {
                lines
                    .iter()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .cloned()
                    .unwrap_or_default()
            })
            .unwrap_or_default()
    }
}

#[derive(Deserialize)]
struct Ready {
    #[serde(default)]
    ready: bool,
    #[serde(default)]
    model: String,
    /// Which implementation answered. faster-whisper is several times quicker
    /// on a processor than the reference one for the same model, so it is worth
    /// telling the user which they got.
    #[serde(default)]
    backend: String,
    #[serde(default)]
    threads: u32,
}

#[derive(Deserialize)]
struct Reply {
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    language: String,
    #[serde(default)]
    segments: Vec<ReplySegment>,
    #[serde(default)]
    error: String,
}

#[derive(Deserialize)]
struct ReplySegment {
    #[serde(default)]
    start: f64,
    #[serde(default)]
    end: f64,
    #[serde(default)]
    text: String,
}

/// A live interpreter with the model loaded.
pub struct Resident {
    child: Child,
    stdin: ChildStdin,
    replies: Receiver<String>,
    diagnostics: Diagnostics,
    model: String,
    backend: String,
    threads: u32,
    /// The previous line, offered back as context. Whisper uses it to keep
    /// names and jargon consistent across utterances of one conversation.
    previous: String,
}

impl Resident {
    /// Start a worker and wait for the model to be in memory.
    pub fn start(interpreter: &Path, work_dir: &Path, options: &Options) -> Result<Self> {
        std::fs::create_dir_all(work_dir)
            .with_context(|| format!("creating {}", work_dir.display()))?;
        let script = work_dir.join("ssr_whisper_worker.py");
        std::fs::write(&script, WORKER_SOURCE)
            .with_context(|| format!("writing {}", script.display()))?;

        let settings = serde_json::json!({
            "model": options.model.clone().unwrap_or_else(|| "base".to_string()),
        });

        let mut command = Command::new(interpreter);
        command
            // Unbuffered, or the replies sit in Python's buffer and the caller
            // waits for something that has already been written.
            .arg("-u")
            .arg(&script)
            .arg(settings.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = without_window(&mut command)
            .spawn()
            .with_context(|| format!("starting {}", interpreter.display()))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("the worker has no input stream"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("the worker has no output stream"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("the worker has no error stream"))?;

        let diagnostics = Diagnostics::default();

        // Both streams are drained by threads. A full pipe blocks the child,
        // and the model download writes a great deal to the error stream.
        let (tx, replies) = mpsc::channel();
        std::thread::Builder::new()
            .name("ssr-whisper-out".into())
            .spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            })
            .context("starting the worker's reader")?;

        let error_diagnostics = diagnostics.clone();
        std::thread::Builder::new()
            .name("ssr-whisper-err".into())
            .spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    error_diagnostics.push(line);
                }
            })
            .context("starting the worker's error reader")?;

        let mut resident = Resident {
            child,
            stdin,
            replies,
            diagnostics,
            model: String::new(),
            backend: String::new(),
            threads: 0,
            previous: String::new(),
        };

        match resident.replies.recv_timeout(READY_TIMEOUT) {
            Ok(line) => {
                let ready: Ready = serde_json::from_str(&line).with_context(|| {
                    format!("the worker's first reply was not understood: {line}")
                })?;
                if !ready.ready {
                    bail!("the worker started but reported it was not ready");
                }
                resident.model = ready.model;
                resident.backend = ready.backend;
                resident.threads = ready.threads;
                Ok(resident)
            }
            Err(RecvTimeoutError::Timeout) => {
                let _ = resident.child.kill();
                bail!(
                    "the model did not finish loading within {} minutes. {}",
                    READY_TIMEOUT.as_secs() / 60,
                    resident.diagnostics.tail()
                )
            }
            Err(RecvTimeoutError::Disconnected) => {
                let _ = resident.child.wait();
                let detail = resident.diagnostics.tail();
                if detail.is_empty() {
                    bail!("the worker stopped before it was ready")
                }
                bail!("{detail}")
            }
        }
    }

    /// The model that was actually loaded.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Which implementation is doing the work.
    pub fn backend(&self) -> &str {
        &self.backend
    }

    /// How many cores it was told it could use.
    pub fn threads(&self) -> u32 {
        self.threads
    }

    /// Transcribe one utterance of 16 kHz mono audio.
    pub fn transcribe(&mut self, samples: &[f32], options: &Options) -> Result<Recognised> {
        let header = serde_json::json!({
            "samples": samples.len(),
            "language": options.language,
            "translate": options.translate,
            "prompt": self.previous,
        });

        // The header names the length, then exactly that many samples follow.
        // Framing it this way means the worker never has to guess where one
        // utterance ends, which a delimiter in binary audio cannot promise.
        self.stdin
            .write_all(header.to_string().as_bytes())
            .and_then(|()| self.stdin.write_all(b"\n"))
            .and_then(|()| {
                let mut bytes = Vec::with_capacity(samples.len() * 4);
                for sample in samples {
                    bytes.extend_from_slice(&sample.to_le_bytes());
                }
                self.stdin.write_all(&bytes)
            })
            .and_then(|()| self.stdin.flush())
            .map_err(|e| anyhow!("the worker stopped accepting audio: {e}"))?;

        let line = match self.replies.recv_timeout(JOB_TIMEOUT) {
            Ok(line) => line,
            Err(RecvTimeoutError::Timeout) => {
                bail!(
                    "the engine did not answer within {} seconds",
                    JOB_TIMEOUT.as_secs()
                )
            }
            Err(RecvTimeoutError::Disconnected) => {
                let detail = self.diagnostics.tail();
                bail!(
                    "the engine stopped{}",
                    if detail.is_empty() {
                        String::new()
                    } else {
                        format!(": {detail}")
                    }
                )
            }
        };

        let reply: Reply = serde_json::from_str(&line)
            .with_context(|| format!("the engine's reply was not understood: {line}"))?;
        if !reply.ok {
            bail!("{}", reply.error);
        }

        let segments: Vec<Segment> = reply
            .segments
            .into_iter()
            .map(|s| Segment {
                start: s.start,
                end: s.end,
                text: s.text,
                speaker: None,
            })
            .collect();

        // Remember what was said, to give the next utterance some context.
        let text = segments
            .iter()
            .map(|s| s.text.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        // Unless the model was repeating itself, in which case handing it back
        // is how a stall in one utterance becomes a stall in the next four.
        // `condition_on_previous_text` is off for exactly this reason and then
        // this prompt puts the feedback straight back; a stalled model gets a
        // clean slate instead.
        let mended = crate::sanity::repair(&text);
        if !mended.is_clean() {
            self.previous.clear();
        } else if !text.is_empty() {
            // Only the tail matters, and a long prompt costs time.
            self.previous = text
                .chars()
                .rev()
                .take(220)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
        }

        Ok(Recognised {
            segments,
            language: if reply.language.is_empty() {
                options.language.clone().unwrap_or_else(|| "unknown".into())
            } else {
                reply.language
            },
            model: Some(self.model.clone()),
        })
    }
}

impl Drop for Resident {
    fn drop(&mut self) {
        // Closing the input stream is the polite signal; the worker's read
        // returns nothing and it exits on its own.
        let _ = self.stdin.flush();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Where the worker's scratch files live, given a session's working directory.
pub fn work_dir_for(base: &Path) -> PathBuf {
    base.join("engine")
}

/// How a model download is going.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    /// How far along, between zero and one, where that can be known.
    pub fraction: f64,
    pub bytes: u64,
    pub total: u64,
    /// Set when it has finished, successfully or not.
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Deserialize)]
struct DownloadLine {
    #[serde(default)]
    stage: String,
    #[serde(default)]
    fraction: f64,
    #[serde(default)]
    bytes: u64,
    #[serde(default)]
    total: u64,
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    error: String,
}

/// Fetch a model, reporting progress as it goes.
///
/// Exists so the wait is explainable. Left to itself the library downloads on
/// first use, silently, which makes the recorder look as though it has hung the
/// first time anybody speaks — and a download killed halfway leaves a corrupt
/// file that then looks like a broken installation.
pub fn download_model(
    interpreter: &Path,
    work_dir: &Path,
    model: &str,
    mut on_progress: impl FnMut(DownloadProgress),
) -> Result<()> {
    std::fs::create_dir_all(work_dir)
        .with_context(|| format!("creating {}", work_dir.display()))?;
    let script = work_dir.join("ssr_whisper_worker.py");
    std::fs::write(&script, WORKER_SOURCE)
        .with_context(|| format!("writing {}", script.display()))?;

    let settings = serde_json::json!({ "action": "download", "model": model });

    let mut command = Command::new(interpreter);
    command
        .arg("-u")
        .arg(&script)
        .arg(settings.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = without_window(&mut command)
        .spawn()
        .with_context(|| format!("starting {}", interpreter.display()))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("the downloader has no output stream"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("the downloader has no error stream"))?;

    let diagnostics = Diagnostics::default();
    let error_diagnostics = diagnostics.clone();
    std::thread::Builder::new()
        .name("ssr-download-err".into())
        .spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                error_diagnostics.push(line);
            }
        })
        .context("starting the downloader's error reader")?;

    let mut finished = false;
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        let Ok(parsed) = serde_json::from_str::<DownloadLine>(&line) else {
            continue;
        };
        if parsed.ok {
            finished = true;
            on_progress(DownloadProgress {
                fraction: 1.0,
                bytes: parsed.bytes,
                total: parsed.total,
                done: true,
                error: None,
            });
            break;
        }
        if parsed.stage == "downloading" {
            on_progress(DownloadProgress {
                fraction: parsed.fraction,
                bytes: parsed.bytes,
                total: parsed.total,
                done: false,
                error: None,
            });
        }
        if !parsed.error.is_empty() {
            bail!("{}", parsed.error);
        }
    }

    let status = child.wait().context("waiting for the downloader")?;
    if !finished || !status.success() {
        let detail = diagnostics.tail();
        bail!(
            "the model could not be downloaded{}",
            if detail.is_empty() {
                String::new()
            } else {
                format!(": {detail}")
            }
        );
    }
    Ok(())
}

/// Where the reference implementation keeps its models.
fn openai_cache() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)?;
    Some(home.join(".cache").join("whisper"))
}

/// Whether a model is already on the machine, so it can be used immediately.
pub fn is_model_cached(model: &str) -> bool {
    let Some(root) = openai_cache() else {
        return false;
    };
    // The reference implementation names them plainly. faster-whisper keeps
    // its own copies under the Hugging Face cache, which is why this reports
    // only what can be answered for certain.
    root.join(format!("{model}.pt")).is_file()
}

/// Bytes a cached model occupies, for showing what is on disk.
pub fn cached_model_size(model: &str) -> Option<u64> {
    let root = openai_cache()?;
    std::fs::metadata(root.join(format!("{model}.pt")))
        .ok()
        .map(|m| m.len())
}
