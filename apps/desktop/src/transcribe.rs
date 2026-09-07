//! Transcription, as the desktop application uses it.
//!
//! Two shapes, matching the two things a person wants. Listening turns a
//! channel into a conversation as it happens. Transcribing turns a finished
//! recording into files. Both run on threads of their own, because recognition
//! is far too slow to happen while an interface is waiting.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use crossbeam_channel::{bounded, RecvTimeoutError};
use serde::{Deserialize, Serialize};

use ssr_core::Packet;
use ssr_session::SourceRequest;
use ssr_transcribe::audio::StreamResampler;
use ssr_transcribe::engine::{self, Engine};
use ssr_transcribe::live::{EngineRecogniser, Line, Listener, LiveFormat, Options};
use ssr_transcribe::whisper;

/// Everything a speech engine reads is at this rate.
const ENGINE_RATE: u32 = 16_000;

/// What the interface is told about the engines on this machine.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineReport {
    pub engines: Vec<Engine>,
    /// True when at least one engine could actually run.
    pub ready: bool,
    /// What to do about it when nothing can. Shown verbatim.
    pub advice: String,
}

pub fn report_engines() -> EngineReport {
    let engines = engine::detect();
    let ready = engines.iter().any(|e| e.is_usable());
    EngineReport {
        ready,
        advice: engine::installation_advice().to_string(),
        engines,
    }
}

/// Pick an engine by name, or the best usable one.
fn choose(wanted: Option<&str>) -> Result<Engine> {
    let found = engine::detect();
    if found.is_empty() {
        bail!("{}", engine::installation_advice());
    }
    if let Some(name) = wanted {
        let needle = name.to_lowercase();
        return found
            .into_iter()
            .find(|e| e.label.to_lowercase().contains(&needle))
            .ok_or_else(|| anyhow!("no installed engine matches {name:?}"));
    }
    found
        .iter()
        .find(|e| e.is_usable())
        .cloned()
        .ok_or_else(|| {
            anyhow!(
                "{} is installed but has no model to work with.\n\n{}",
                found[0].label,
                engine::installation_advice()
            )
        })
}

// --- listening ---------------------------------------------------------------

/// What to listen to, and how.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenConfig {
    /// The one channel to transcribe.
    pub source: SourceRequest,
    /// Name to label the lines with.
    pub label: String,
    #[serde(default = "yes")]
    pub detect_speakers: bool,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub translate: bool,
    #[serde(default)]
    pub engine: Option<String>,
    /// Where the running transcript is appended. Defaults beside the others.
    #[serde(default)]
    pub transcript_path: Option<PathBuf>,
    #[serde(default = "default_buffer")]
    pub buffer_ms: u32,
}

fn yes() -> bool {
    true
}
fn default_buffer() -> u32 {
    200
}

/// How the listening session is going.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenStatus {
    pub active: bool,
    pub channel: String,
    pub engine: String,
    /// Seconds since listening began.
    pub elapsed_secs: f64,
    /// Utterances heard but not yet recognised. A number that keeps climbing
    /// means the machine cannot keep up with the conversation.
    pub backlog: usize,
    pub transcript_path: String,
    /// Lines produced so far.
    pub total_lines: usize,
    /// Distinct voices heard, when speaker detection is on.
    pub speakers: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ListenStatus {
    pub fn idle() -> Self {
        ListenStatus {
            active: false,
            channel: String::new(),
            engine: String::new(),
            elapsed_secs: 0.0,
            backlog: 0,
            transcript_path: String::new(),
            total_lines: 0,
            speakers: 0,
            error: None,
        }
    }
}

/// How many lines the session keeps in memory.
///
/// The transcript file is the complete record and is appended to a line at a
/// time, so nothing is lost by forgetting the beginning of a long meeting here.
/// Holding all of it would mean an hour of talk sitting in memory twice over,
/// once in this process and once in the interface, for the sake of scrollback
/// nobody reads while the meeting is still running.
const LINES_KEPT: usize = 2_000;

/// Shared between the listening thread and whoever is watching it.
struct Shared {
    lines: Vec<Line>,
    /// Lines dropped from the front, so indexing stays correct after trimming.
    dropped: usize,
    status: ListenStatus,
}

impl Shared {
    /// Add lines, forgetting the oldest once there are too many.
    fn extend(&mut self, fresh: impl IntoIterator<Item = Line>) {
        self.lines.extend(fresh);
        if self.lines.len() > LINES_KEPT {
            let excess = self.lines.len() - LINES_KEPT;
            self.lines.drain(0..excess);
            self.dropped += excess;
        }
    }

    /// Total produced, including those no longer held.
    fn total(&self) -> usize {
        self.dropped + self.lines.len()
    }
}

/// A running live transcription.
pub struct LiveSession {
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl LiveSession {
    pub fn start(config: ListenConfig) -> Result<Self> {
        let engine = choose(config.engine.as_deref())?;

        let transcript_path = config.transcript_path.clone().unwrap_or_else(|| {
            let stamp = ssr_core::local_timestamp();
            let safe = ssr_timeline::sanitize_filename(&config.label);
            PathBuf::from("transcripts").join(format!("{stamp}-{safe}.md"))
        });

        let work_dir = std::env::temp_dir().join(format!("ssr-listen-{}", std::process::id()));
        let recogniser = EngineRecogniser::new(
            engine.clone(),
            work_dir.clone(),
            whisper::Options {
                language: config.language.clone(),
                model: config.model.clone(),
                threads: None,
                translate: config.translate,
            },
        )?;

        // Capture first, so the endpoint's real format is known before the
        // listener is built around it.
        let (tx, rx) = bounded::<(ssr_core::source::TrackKey, Packet)>(512);
        let spec = config.source.to_spec(48_000, 2);
        let handle = ssr_core::start("listen", spec, config.buffer_ms, tx)
            .map_err(|e| anyhow!("that channel could not be opened: {e}"))?;
        let source_rate = handle.format.sample_rate;
        let source_channels = handle.format.channels.max(1) as usize;

        let mut options = Options::for_channel(config.label.clone());
        options.detect_speakers = config.detect_speakers;
        options.language = config.language.clone();
        options.model = config.model.clone();
        options.translate = config.translate;
        options.transcript_path = Some(transcript_path.clone());
        options.transcript_format = LiveFormat::Markdown;

        let mut listener = Listener::start(ENGINE_RATE, options, Box::new(recogniser))?;

        let shared = Arc::new(Mutex::new(Shared {
            lines: Vec::new(),
            dropped: 0,
            status: ListenStatus {
                active: true,
                channel: config.label.clone(),
                engine: engine.label.clone(),
                elapsed_secs: 0.0,
                backlog: 0,
                transcript_path: transcript_path.display().to_string(),
                total_lines: 0,
                speakers: 0,
                error: None,
            },
        }));
        let stop = Arc::new(AtomicBool::new(false));

        let worker_shared = shared.clone();
        let worker_stop = stop.clone();
        let worker = std::thread::Builder::new()
            .name("ssr-live".into())
            .spawn(move || {
                let _apartment = ssr_core::com::Apartment::new();
                let mut resampler = StreamResampler::new(source_rate, ENGINE_RATE);
                let started = Instant::now();
                let mut mono: Vec<f32> = Vec::new();

                // A loopback stream sends nothing while its endpoint is quiet,
                // so without feeding real silence in the clock would only move
                // while someone was talking, and an utterance would never be
                // closed by the pause that ended it.
                let top_up = |listener: &mut Listener, elapsed: f64| {
                    let behind = (elapsed - 0.3) - listener.heard_secs();
                    if behind > 0.05 {
                        let n = (behind.min(5.0) * ENGINE_RATE as f64) as usize;
                        listener.push(&vec![0.0f32; n]);
                    }
                };

                while !worker_stop.load(Ordering::SeqCst) {
                    match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok((_, packet)) => {
                            mono.clear();
                            downmix(&packet.samples, source_channels, &mut mono);
                            let converted = resampler.push(&mono);
                            if !converted.is_empty() {
                                listener.push(&converted);
                            }
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }

                    top_up(&mut listener, started.elapsed().as_secs_f64());

                    let fresh = listener.drain();
                    let backlog = listener.backlog();
                    if let Ok(mut s) = worker_shared.lock() {
                        s.extend(fresh);
                        s.status.elapsed_secs = started.elapsed().as_secs_f64();
                        s.status.backlog = backlog;
                        s.status.total_lines = s.total();
                        s.status.speakers = distinct_speakers(&s.lines);
                    }
                }

                handle.signal_stop();
                let tail = resampler.flush();
                if !tail.is_empty() {
                    listener.push(&tail);
                }
                top_up(&mut listener, started.elapsed().as_secs_f64());

                // The last thing said is usually the part someone wanted, so
                // the queue is drained rather than abandoned.
                let remaining = listener.finish();
                if let Ok(mut s) = worker_shared.lock() {
                    s.extend(remaining);
                    s.status.active = false;
                    s.status.backlog = 0;
                    s.status.total_lines = s.total();
                    s.status.speakers = distinct_speakers(&s.lines);
                }
                let _ = std::fs::remove_dir_all(&work_dir);
            })
            .context("starting the listening thread")?;

        Ok(LiveSession {
            shared,
            stop,
            worker: Some(worker),
        })
    }

    pub fn status(&self) -> ListenStatus {
        self.shared
            .lock()
            .map(|s| s.status.clone())
            .unwrap_or_else(|_| ListenStatus::idle())
    }

    /// Lines after the given index, so the interface can ask for only what it
    /// has not seen. Sending the whole conversation on every poll would grow
    /// without bound over a long meeting.
    pub fn lines_after(&self, after: usize) -> Vec<Line> {
        self.shared
            .lock()
            .map(|s| {
                // `after` counts from the start of the conversation, but the
                // oldest lines may have been forgotten. Anything the caller
                // missed entirely is gone; the file still has it.
                let skip = after.saturating_sub(s.dropped);
                s.lines.iter().skip(skip).cloned().collect()
            })
            .unwrap_or_default()
    }

    pub fn is_running(&self) -> bool {
        self.status().active
    }

    pub fn stop(&mut self) -> ListenStatus {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        self.status()
    }
}

impl Drop for LiveSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn distinct_speakers(lines: &[Line]) -> usize {
    let mut seen: Vec<&str> = Vec::new();
    for line in lines {
        if !seen.contains(&line.speaker.label.as_str()) {
            seen.push(&line.speaker.label);
        }
    }
    seen.len()
}

/// Average the channels of an interleaved buffer into mono.
fn downmix(samples: &[f32], channels: usize, out: &mut Vec<f32>) {
    if channels <= 1 {
        out.extend_from_slice(samples);
        return;
    }
    out.reserve(samples.len() / channels);
    for frame in samples.chunks_exact(channels) {
        out.push(frame.iter().sum::<f32>() / channels as f32);
    }
}

// --- transcribing a finished recording ---------------------------------------

/// How a batch transcription is going.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchStatus {
    pub active: bool,
    pub engine: String,
    /// Track being worked on, and how far through the list it is.
    pub track: String,
    pub index: usize,
    pub total: usize,
    /// What is happening to it: preparing, recognising, writing.
    pub stage: String,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<ssr_transcribe::job::TranscribeResult>,
}

impl BatchStatus {
    pub fn idle() -> Self {
        BatchStatus {
            active: false,
            engine: String::new(),
            track: String::new(),
            index: 0,
            total: 0,
            stage: String::new(),
            done: false,
            error: None,
            result: None,
        }
    }
}

/// A batch transcription running in the background.
pub struct BatchJob {
    status: Arc<Mutex<BatchStatus>>,
    worker: Option<JoinHandle<()>>,
}

impl BatchJob {
    pub fn start(
        request: ssr_transcribe::job::TranscribeRequest,
        engine_name: Option<String>,
    ) -> Result<Self> {
        let engine = choose(engine_name.as_deref())?;

        let status = Arc::new(Mutex::new(BatchStatus {
            active: true,
            engine: engine.label.clone(),
            stage: "starting".into(),
            ..BatchStatus::idle()
        }));

        let worker_status = status.clone();
        let worker = std::thread::Builder::new()
            .name("ssr-transcribe".into())
            .spawn(move || {
                let _apartment = ssr_core::com::Apartment::new();
                let progress_status = worker_status.clone();
                let outcome = ssr_transcribe::transcribe_session(&engine, &request, move |p| {
                    if let Ok(mut s) = progress_status.lock() {
                        s.track = p.track;
                        s.index = p.index;
                        s.total = p.total;
                        s.stage = p.stage;
                    }
                });

                if let Ok(mut s) = worker_status.lock() {
                    s.active = false;
                    s.done = true;
                    s.stage = "finished".into();
                    match outcome {
                        Ok(result) => s.result = Some(result),
                        Err(e) => s.error = Some(format!("{e:#}")),
                    }
                }
            })
            .context("starting the transcription thread")?;

        Ok(BatchJob {
            status,
            worker: Some(worker),
        })
    }

    pub fn status(&self) -> BatchStatus {
        self.status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| BatchStatus::idle())
    }

    pub fn is_running(&self) -> bool {
        self.status().active
    }
}

impl Drop for BatchJob {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

// --- saving ------------------------------------------------------------------

/// What the save dialog asks for.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRequest {
    /// The conversation to write.
    pub lines: Vec<Line>,
    pub channel: String,
    pub engine: String,
    /// Directory to write into.
    pub directory: PathBuf,
    /// File name without an extension; one file is written per format.
    pub stem: String,
    pub formats: Vec<ssr_transcribe::TranscriptFormat>,
    /// Prefix each line with who said it.
    #[serde(default = "yes")]
    pub include_speakers: bool,
}

/// Where a save ended up.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub directory: String,
    pub files: Vec<String>,
}

/// Write a live conversation out in the chosen formats.
pub fn save_conversation(request: SaveRequest) -> Result<SaveResult> {
    if request.lines.is_empty() {
        bail!("there is nothing to save yet");
    }
    if request.formats.is_empty() {
        bail!("choose at least one format");
    }

    let mut transcript =
        ssr_transcribe::live::to_transcript(&request.lines, &request.channel, &request.engine);
    if !request.include_speakers {
        // Strip the attribution the conversion adds, for someone who wants the
        // words and nothing else.
        for (segment, line) in transcript.segments.iter_mut().zip(request.lines.iter()) {
            segment.text = line.text.clone();
        }
    }

    let stem = ssr_timeline::sanitize_filename(&request.stem);
    let mut files = Vec::new();
    for format in &request.formats {
        let path =
            ssr_transcribe::transcript::write(&transcript, &request.directory, &stem, *format)?;
        files.push(path.display().to_string());
    }

    Ok(SaveResult {
        directory: request.directory.display().to_string(),
        files,
    })
}

// --- models ------------------------------------------------------------------

/// A model the user can choose, and whether it is already here.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub label: String,
    /// What choosing it means, in plain terms.
    pub hint: String,
    /// Roughly what it will cost to fetch, in bytes.
    pub approximate_bytes: u64,
    /// True when it is on the machine and will load without a download.
    pub cached: bool,
    /// What it actually occupies, once it is here.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
}

/// The models worth offering, smallest first.
///
/// Sizes are the download, not the memory it needs while running, which is
/// larger. The advice matters more than the numbers: for anything but clear
/// English the jump from base to small is the one that changes the answer.
const CATALOGUE: &[(&str, &str, &str, u64)] = &[
    (
        "tiny",
        "Tiny",
        "Fastest, roughest. Usable for English only.",
        75_000_000,
    ),
    (
        "base",
        "Base",
        "Fine for clear English. Weak on other languages.",
        145_000_000,
    ),
    (
        "small",
        "Small",
        "The first size worth using for Ukrainian and other languages.",
        480_000_000,
    ),
    (
        "medium",
        "Medium",
        "Better again, and noticeably slower on a processor.",
        1_500_000_000,
    ),
    (
        "large-v3",
        "Large",
        "Best available. Slow without a graphics card.",
        3_100_000_000,
    ),
];

#[tauri::command]
pub fn list_speech_models() -> Vec<ModelInfo> {
    CATALOGUE
        .iter()
        .map(|(id, label, hint, approximate)| ModelInfo {
            id: (*id).to_string(),
            label: (*label).to_string(),
            hint: (*hint).to_string(),
            approximate_bytes: *approximate,
            cached: ssr_transcribe::resident::is_model_cached(id),
            bytes: ssr_transcribe::resident::cached_model_size(id),
        })
        .collect()
}

/// How a model download is going.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownload {
    pub active: bool,
    pub model: String,
    pub fraction: f64,
    pub bytes: u64,
    pub total: u64,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A download running in the background.
pub struct DownloadJob {
    status: Arc<Mutex<ModelDownload>>,
    worker: Option<JoinHandle<()>>,
}

impl DownloadJob {
    pub fn start(model: String, engine_name: Option<String>) -> Result<Self> {
        let engine = choose(engine_name.as_deref())?;
        let interpreter = engine.interpreter.clone().ok_or_else(|| {
            anyhow!(
                "downloading needs the Python interpreter that owns the whisper \
                 install, and it could not be found next to {}",
                engine.executable.display()
            )
        })?;

        let status = Arc::new(Mutex::new(ModelDownload {
            active: true,
            model: model.clone(),
            ..ModelDownload::default()
        }));

        let work_dir = std::env::temp_dir().join(format!("ssr-download-{}", std::process::id()));
        let worker_status = status.clone();
        let worker = std::thread::Builder::new()
            .name("ssr-model-download".into())
            .spawn(move || {
                let progress_status = worker_status.clone();
                let outcome = ssr_transcribe::resident::download_model(
                    &interpreter,
                    &work_dir,
                    &model,
                    move |p| {
                        if let Ok(mut s) = progress_status.lock() {
                            s.fraction = p.fraction;
                            s.bytes = p.bytes;
                            s.total = p.total;
                        }
                    },
                );

                if let Ok(mut s) = worker_status.lock() {
                    s.active = false;
                    s.done = true;
                    match outcome {
                        Ok(()) => s.fraction = 1.0,
                        Err(e) => s.error = Some(format!("{e:#}")),
                    }
                }
                let _ = std::fs::remove_dir_all(&work_dir);
            })
            .context("starting the download")?;

        Ok(DownloadJob {
            status,
            worker: Some(worker),
        })
    }

    pub fn status(&self) -> ModelDownload {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn is_running(&self) -> bool {
        self.status().active
    }
}

impl Drop for DownloadJob {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
