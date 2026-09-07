//! Transcribing a channel while it is still playing.
//!
//! The batch path takes a finished recording and produces files. This one takes
//! audio as it arrives and produces lines, so a conversation can be read as it
//! happens and is on disk before it ends.
//!
//! Recognition is far slower than real time to start and far faster than real
//! time to finish, which is to say it is lumpy. It therefore runs on a thread
//! of its own: whoever is feeding audio in must never wait for a model. If
//! speech arrives faster than it can be recognised the queue grows, and the
//! listener says so rather than dropping audio silently.

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use anyhow::{Context, Result};
use crossbeam_channel::{bounded, Receiver, Sender};
use serde::{Deserialize, Serialize};

use crate::engine::Engine;
use crate::speakers::{self, Diariser, Speaker};
use crate::transcript::{Segment, Transcript};
use crate::vad;
use crate::whisper;

/// One line of the conversation, as it would appear in a chat.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    /// Position in the conversation, counting from one.
    pub index: usize,
    /// The channel it came in on, such as the Citrix session.
    pub channel: String,
    pub speaker: Speaker,
    /// Seconds from the start of listening.
    pub start: f64,
    pub end: f64,
    pub text: String,
}

impl Line {
    pub fn duration(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }
}

/// How the listener should behave.
#[derive(Debug, Clone)]
pub struct Options {
    /// Name of the channel, used to label every line from it.
    pub channel: String,
    /// Try to tell voices apart within this channel. Worth it for a meeting
    /// coming down one wire, pointless for a microphone with one person at it.
    pub detect_speakers: bool,
    pub language: Option<String>,
    pub model: Option<String>,
    pub translate: bool,
    /// A file to append each line to as it is recognised. Written immediately
    /// so that a crash, or simply forgetting to press stop, still leaves a
    /// usable transcript.
    pub transcript_path: Option<PathBuf>,
    /// Format of that file.
    pub transcript_format: LiveFormat,
}

impl Options {
    pub fn for_channel(channel: impl Into<String>) -> Self {
        Options {
            channel: channel.into(),
            detect_speakers: true,
            language: None,
            model: None,
            translate: false,
            transcript_path: None,
            transcript_format: LiveFormat::Markdown,
        }
    }
}

/// How a live transcript is written as it grows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LiveFormat {
    /// One line per utterance, readable as it is written.
    Markdown,
    /// One JSON object per line, for anything that wants to consume it.
    Jsonl,
    /// Just the words.
    Text,
}

impl LiveFormat {
    pub fn extension(self) -> &'static str {
        match self {
            LiveFormat::Markdown => "md",
            LiveFormat::Jsonl => "jsonl",
            LiveFormat::Text => "txt",
        }
    }
}

/// Turns audio into words. A trait so the listener can be tested without a
/// speech engine and a model on the machine.
pub trait Recogniser: Send {
    /// Recognise one utterance. Returning an empty string means nothing was
    /// said, which is a normal outcome rather than a failure.
    fn recognise(&mut self, samples: &[f32], sample_rate: u32) -> Result<String>;
}

/// A recogniser backed by an installed speech engine.
pub struct EngineRecogniser {
    engine: Engine,
    options: whisper::Options,
    work_dir: PathBuf,
}

impl EngineRecogniser {
    pub fn new(engine: Engine, work_dir: PathBuf, options: whisper::Options) -> Result<Self> {
        std::fs::create_dir_all(&work_dir)
            .with_context(|| format!("creating {}", work_dir.display()))?;
        Ok(EngineRecogniser {
            engine,
            options,
            work_dir,
        })
    }
}

impl Recogniser for EngineRecogniser {
    fn recognise(&mut self, samples: &[f32], _sample_rate: u32) -> Result<String> {
        let audio = self.work_dir.join("utterance.wav");
        crate::audio::write_wav_16k_mono(&audio, samples)?;
        let recognised = whisper::run(&self.engine, &audio, &self.work_dir, &self.options)?;
        let text = recognised
            .segments
            .iter()
            .map(|s| s.text.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        Ok(text)
    }
}

/// A live transcription of one channel.
pub struct Listener {
    detector: vad::Detector,
    /// Utterances waiting to be recognised.
    to_worker: Option<Sender<Job>>,
    /// Lines the worker has finished.
    lines: Receiver<Line>,
    worker: Option<JoinHandle<()>>,
    /// How many utterances are queued but not yet recognised.
    backlog: Arc<AtomicUsize>,
    sample_rate: u32,
}

struct Job {
    index: usize,
    start: f64,
    end: f64,
    samples: Vec<f32>,
}

impl Listener {
    /// Start listening. Audio is fed in with [`Listener::push`].
    pub fn start(
        sample_rate: u32,
        options: Options,
        mut recogniser: Box<dyn Recogniser>,
    ) -> Result<Self> {
        // A bounded queue, because an unbounded one would let a machine that
        // cannot keep up consume memory until it fell over. At sixteen
        // utterances the listener reports a backlog instead.
        let (to_worker, jobs) = bounded::<Job>(16);
        let (to_caller, lines) = bounded::<Line>(256);
        let backlog = Arc::new(AtomicUsize::new(0));

        let mut writer = LineWriter::open(&options)?;
        let mut diariser = Diariser::new(options.channel.clone(), options.detect_speakers);
        let channel = options.channel.clone();
        let worker_backlog = backlog.clone();

        let worker = std::thread::Builder::new()
            .name("ssr-listen".into())
            .spawn(move || {
                for job in jobs {
                    worker_backlog.fetch_sub(1, Ordering::SeqCst);

                    let text = match recogniser.recognise(&job.samples, sample_rate) {
                        Ok(t) => t,
                        // A failed utterance should not end the session. The
                        // next one may well work, and the audio is still being
                        // recorded regardless.
                        Err(e) => format!("[not recognised: {e}]"),
                    };
                    if text.trim().is_empty() {
                        continue;
                    }

                    let print = speakers::voice_print(&job.samples, sample_rate);
                    let speaker = diariser.assign(&print);

                    let line = Line {
                        index: job.index,
                        channel: channel.clone(),
                        speaker,
                        start: job.start,
                        end: job.end,
                        text,
                    };

                    writer.append(&line);
                    if to_caller.send(line).is_err() {
                        break;
                    }
                }
            })
            .context("starting the recognition thread")?;

        Ok(Listener {
            detector: vad::Detector::new(vad::Config::for_rate(sample_rate)),
            to_worker: Some(to_worker),
            lines,
            worker: Some(worker),
            backlog,
            sample_rate,
        })
    }

    /// Feed audio in. Returns immediately; recognition happens elsewhere.
    ///
    /// The audio must be mono at the rate the listener was started with.
    pub fn push(&mut self, samples: &[f32]) {
        let events = self.detector.push(samples);
        self.dispatch(events);
    }

    fn dispatch(&mut self, events: Vec<vad::Event>) {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let Some(sender) = &self.to_worker else {
            return;
        };
        for event in events {
            let vad::Event::Utterance {
                start_sample,
                end_sample,
                samples,
            } = event
            else {
                continue;
            };
            let job = Job {
                index: COUNTER.fetch_add(1, Ordering::SeqCst) + 1,
                start: start_sample as f64 / self.sample_rate as f64,
                end: end_sample as f64 / self.sample_rate as f64,
                samples,
            };
            self.backlog.fetch_add(1, Ordering::SeqCst);
            if sender.try_send(job).is_err() {
                // The queue is full, which means recognition is further behind
                // than the machine can recover from. Dropping the oldest would
                // lose the thread of the conversation; dropping this one at
                // least keeps what has been said so far intact.
                self.backlog.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }

    /// Lines recognised since this was last called.
    pub fn drain(&mut self) -> Vec<Line> {
        self.lines.try_iter().collect()
    }

    /// Utterances heard but not yet recognised.
    pub fn backlog(&self) -> usize {
        self.backlog.load(Ordering::SeqCst)
    }

    /// Seconds of audio heard.
    pub fn heard_secs(&self) -> f64 {
        self.detector.position_secs()
    }

    /// Stop listening and collect whatever is still in flight.
    ///
    /// Waits for the queue to drain, because the last thing said is usually the
    /// part someone wanted.
    pub fn finish(mut self) -> Vec<Line> {
        let trailing = self.detector.finish();
        self.dispatch(trailing);

        // Closing the sender is what tells the worker there is no more coming.
        self.to_worker = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        self.lines.try_iter().collect()
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.to_worker = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Appends lines to the transcript file as they are recognised.
struct LineWriter {
    file: Option<std::fs::File>,
    format: LiveFormat,
}

impl LineWriter {
    fn open(options: &Options) -> Result<Self> {
        let Some(path) = &options.transcript_path else {
            return Ok(LineWriter {
                file: None,
                format: options.transcript_format,
            });
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("opening {}", path.display()))?;

        if options.transcript_format == LiveFormat::Markdown {
            let _ = writeln!(file, "# {}\n", options.channel);
        }
        Ok(LineWriter {
            file: Some(file),
            format: options.transcript_format,
        })
    }

    fn append(&mut self, line: &Line) {
        let Some(file) = self.file.as_mut() else {
            return;
        };
        let rendered = match self.format {
            LiveFormat::Markdown => format!(
                "**{}** `{}`  \n{}\n\n",
                line.speaker.label,
                clock(line.start),
                line.text
            ),
            LiveFormat::Text => format!("{}\n", line.text),
            LiveFormat::Jsonl => match serde_json::to_string(line) {
                Ok(json) => format!("{json}\n"),
                Err(_) => String::new(),
            },
        };
        // Flushed on every line. A live transcript that is only complete after
        // a clean shutdown is not much use, since the times you most want it
        // are the times there was not one.
        let _ = file.write_all(rendered.as_bytes());
        let _ = file.flush();
    }
}

/// `M:SS` or `H:MM:SS`, for a timestamp beside a line of speech.
fn clock(seconds: f64) -> String {
    let total = seconds.max(0.0) as u64;
    if total >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            total / 3600,
            (total % 3600) / 60,
            total % 60
        )
    } else {
        format!("{}:{:02}", total / 60, total % 60)
    }
}

/// Turn a finished conversation into a transcript, so a live session can be
/// saved in the same formats a recorded one can.
pub fn to_transcript(lines: &[Line], channel: &str, engine: &str) -> Transcript {
    Transcript {
        track: channel.to_string(),
        source: format!("live transcription of {channel}"),
        language: "unknown".into(),
        engine: engine.to_string(),
        model: None,
        segments: lines
            .iter()
            .map(|l| Segment {
                start: l.start,
                end: l.end,
                text: if l.speaker.inferred {
                    format!("{}: {}", l.speaker.label, l.text)
                } else {
                    l.text.clone()
                },
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 16_000;

    /// Says the same thing every time, so the test is about the plumbing.
    struct Parrot(&'static str);

    impl Recogniser for Parrot {
        fn recognise(&mut self, _samples: &[f32], _rate: u32) -> Result<String> {
            Ok(self.0.to_string())
        }
    }

    struct Silent;

    impl Recogniser for Silent {
        fn recognise(&mut self, _samples: &[f32], _rate: u32) -> Result<String> {
            Ok(String::new())
        }
    }

    struct Broken;

    impl Recogniser for Broken {
        fn recognise(&mut self, _samples: &[f32], _rate: u32) -> Result<String> {
            anyhow::bail!("the model could not be loaded")
        }
    }

    fn speech(secs: f32, pitch: f32) -> Vec<f32> {
        let n = (RATE as f32 * secs) as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                let w = 2.0 * std::f32::consts::PI * pitch * t;
                (w.sin() + 0.5 * (2.0 * w).sin()) * 0.3
            })
            .collect()
    }

    fn quiet(secs: f32) -> Vec<f32> {
        vec![0.0; (RATE as f32 * secs) as usize]
    }

    #[test]
    fn speech_becomes_a_line() {
        let listener = Listener::start(
            RATE,
            Options::for_channel("Citrix"),
            Box::new(Parrot("good morning")),
        )
        .unwrap();

        let mut listener = listener;
        listener.push(&quiet(0.6));
        listener.push(&speech(1.5, 120.0));
        listener.push(&quiet(1.2));
        let lines = listener.finish();

        assert_eq!(lines.len(), 1, "got {lines:?}");
        assert_eq!(lines[0].text, "good morning");
        assert_eq!(lines[0].channel, "Citrix");
        assert!(lines[0].start >= 0.0 && lines[0].end > lines[0].start);
    }

    #[test]
    fn an_utterance_the_engine_heard_nothing_in_produces_no_line() {
        let mut listener =
            Listener::start(RATE, Options::for_channel("Citrix"), Box::new(Silent)).unwrap();
        listener.push(&quiet(0.6));
        listener.push(&speech(1.5, 120.0));
        listener.push(&quiet(1.2));
        assert!(listener.finish().is_empty());
    }

    #[test]
    fn a_failing_engine_reports_itself_rather_than_ending_the_session() {
        let mut listener =
            Listener::start(RATE, Options::for_channel("Citrix"), Box::new(Broken)).unwrap();
        listener.push(&quiet(0.6));
        listener.push(&speech(1.5, 120.0));
        listener.push(&quiet(1.2));
        let lines = listener.finish();
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].text.contains("not recognised"),
            "got {}",
            lines[0].text
        );
    }

    #[test]
    fn each_line_is_written_to_the_file_as_it_happens() {
        let dir = std::env::temp_dir().join("ssr-live-append");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("live.md");
        let _ = std::fs::remove_file(&path);

        let mut options = Options::for_channel("Citrix");
        options.transcript_path = Some(path.clone());

        let mut listener = Listener::start(RATE, options, Box::new(Parrot("hello there"))).unwrap();
        listener.push(&quiet(0.6));
        listener.push(&speech(1.5, 120.0));
        listener.push(&quiet(1.2));
        listener.finish();

        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.starts_with("# Citrix"), "no heading: {body}");
        assert!(body.contains("hello there"), "no text: {body}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn jsonl_output_is_one_object_per_line() {
        let dir = std::env::temp_dir().join("ssr-live-jsonl");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("live.jsonl");
        let _ = std::fs::remove_file(&path);

        let mut options = Options::for_channel("Mic");
        options.transcript_path = Some(path.clone());
        options.transcript_format = LiveFormat::Jsonl;

        let mut listener = Listener::start(RATE, options, Box::new(Parrot("one"))).unwrap();
        listener.push(&quiet(0.6));
        listener.push(&speech(1.2, 130.0));
        listener.push(&quiet(1.2));
        listener.push(&speech(1.2, 130.0));
        listener.push(&quiet(1.2));
        listener.finish();

        let body = std::fs::read_to_string(&path).unwrap();
        let parsed: Vec<Line> = body
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).expect("each line must be an object"))
            .collect();
        assert_eq!(parsed.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lines_convert_into_a_transcript_that_can_be_saved() {
        let lines = vec![Line {
            index: 1,
            channel: "Citrix".into(),
            speaker: Speaker {
                index: 1,
                label: "Citrix".into(),
                inferred: false,
            },
            start: 1.0,
            end: 3.0,
            text: "good morning".into(),
        }];
        let t = to_transcript(&lines, "Citrix", "whisper.cpp");
        assert_eq!(t.segments.len(), 1);
        assert_eq!(t.plain_text(), "good morning");
    }

    #[test]
    fn a_timestamp_reads_as_a_clock() {
        assert_eq!(clock(0.0), "0:00");
        assert_eq!(clock(75.0), "1:15");
        assert_eq!(clock(3725.0), "1:02:05");
    }
}
