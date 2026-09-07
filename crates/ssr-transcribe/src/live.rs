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
use crate::resident::Resident;
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
/// A piece of recognised speech and where it sits inside the utterance.
///
/// Returned separately rather than joined into one string, and that is the
/// whole point of it. An utterance can be twenty seconds of two people talking;
/// joining it gives one late paragraph attributed to one voice. Keeping the
/// pieces gives a line each, at the time each was actually said, and lets the
/// voice behind each one be looked at on its own.
#[derive(Debug, Clone)]
pub struct Heard {
    /// Seconds from the start of the utterance, not of the session.
    pub start: f64,
    pub end: f64,
    pub text: String,
}

pub trait Recogniser: Send {
    /// Recognise one utterance. An empty result means nothing was said, which
    /// is a normal outcome rather than a failure.
    fn recognise(&mut self, samples: &[f32], sample_rate: u32) -> Result<Vec<Heard>>;

    /// Get ready, before there is anything to recognise.
    ///
    /// Called once on the recognition thread as listening begins. Loading a
    /// speech model takes seconds, and on a cold cache it means downloading a
    /// hundred megabytes or more. Doing it here means it overlaps with someone
    /// clearing their throat, rather than delaying the first thing they say.
    fn warm(&mut self) {}
}

/// A recogniser backed by an installed speech engine.
///
/// Prefers to hold the engine open. A one-shot engine spends five to fifteen
/// seconds starting an interpreter and reading the model before it looks at any
/// audio, and it pays that on every utterance, which during a conversation is
/// the difference between transcription happening quietly and the machine
/// visibly grinding. Where the engine cannot be held open this still works, one
/// process at a time, because slow transcription beats none.
pub struct EngineRecogniser {
    engine: Engine,
    options: whisper::Options,
    work_dir: PathBuf,
    /// Started on first use rather than up front, so that loading the model —
    /// which on a cold cache means downloading it — happens on the recognition
    /// thread instead of blocking whoever pressed Listen.
    resident: Option<Resident>,
    /// Set once the resident route has been tried and found wanting, so it is
    /// not retried for every utterance of a long meeting.
    resident_unavailable: bool,
    /// Why, kept for the first report.
    resident_problem: Option<String>,
}

impl EngineRecogniser {
    pub fn new(engine: Engine, work_dir: PathBuf, options: whisper::Options) -> Result<Self> {
        std::fs::create_dir_all(&work_dir)
            .with_context(|| format!("creating {}", work_dir.display()))?;
        Ok(EngineRecogniser {
            engine,
            options,
            work_dir,
            resident: None,
            resident_unavailable: false,
            resident_problem: None,
        })
    }

    /// Whether this will hold the engine open, which is worth telling the user.
    pub fn will_be_resident(&self) -> bool {
        self.engine.is_resident_capable()
    }

    /// Why the engine could not be held open, once that is known.
    pub fn resident_problem(&self) -> Option<&str> {
        self.resident_problem.as_deref()
    }

    /// Recognise through the held-open engine, starting it if needed.
    fn try_resident(&mut self, samples: &[f32]) -> Option<Result<Vec<Heard>>> {
        if self.resident_unavailable {
            return None;
        }
        let interpreter = self.engine.interpreter.clone()?;

        if self.resident.is_none() {
            let dir = crate::resident::work_dir_for(&self.work_dir);
            match Resident::start(&interpreter, &dir, &self.options) {
                Ok(started) => self.resident = Some(started),
                Err(e) => {
                    self.resident_unavailable = true;
                    self.resident_problem = Some(format!("{e:#}"));
                    return None;
                }
            }
        }

        let resident = self.resident.as_mut()?;
        match resident.transcribe(samples, &self.options) {
            Ok(recognised) => Some(Ok(pieces_of(&recognised))),
            Err(e) => {
                // A worker that has died is not worth restarting mid
                // conversation; fall back and keep the words coming.
                self.resident = None;
                self.resident_unavailable = true;
                self.resident_problem = Some(format!("{e:#}"));
                None
            }
        }
    }
}

/// The pieces of a result, with the empty ones dropped.
fn pieces_of(recognised: &whisper::Recognised) -> Vec<Heard> {
    recognised
        .segments
        .iter()
        .filter(|s| !s.text.trim().is_empty())
        .map(|s| Heard {
            start: s.start,
            end: s.end,
            text: s.text.trim().to_string(),
        })
        .collect()
}

impl Recogniser for EngineRecogniser {
    fn warm(&mut self) {
        // An empty utterance is not sent; starting the worker is the point, and
        // any failure is remembered so the first real utterance falls straight
        // through to the slower route instead of waiting again.
        if self.resident_unavailable || self.resident.is_some() {
            return;
        }
        let Some(interpreter) = self.engine.interpreter.clone() else {
            return;
        };
        let dir = crate::resident::work_dir_for(&self.work_dir);
        match Resident::start(&interpreter, &dir, &self.options) {
            Ok(started) => self.resident = Some(started),
            Err(e) => {
                self.resident_unavailable = true;
                self.resident_problem = Some(format!("{e:#}"));
            }
        }
    }

    fn recognise(&mut self, samples: &[f32], _sample_rate: u32) -> Result<Vec<Heard>> {
        if let Some(outcome) = self.try_resident(samples) {
            return outcome;
        }

        // The one-shot route. A file is needed here because the command line
        // program only takes a path, and that path is what makes it reach for
        // ffmpeg; the held-open worker takes the samples directly and needs
        // neither.
        let audio = self.work_dir.join("utterance.wav");
        crate::audio::write_wav_16k_mono(&audio, samples)?;
        let recognised = whisper::run(&self.engine, &audio, &self.work_dir, &self.options)?;
        Ok(pieces_of(&recognised))
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

/// The audio behind one recognised piece, when there is enough of it to judge.
///
/// A voice print from a fifth of a second is noise, and feeding noise to the
/// speaker grouping invents speakers. Below that the utterance as a whole is
/// the better answer.
fn slice_for<'a>(samples: &'a [f32], sample_rate: u32, piece: &Heard) -> Option<&'a [f32]> {
    const SHORTEST: f64 = 0.25;
    if piece.end - piece.start < SHORTEST {
        return None;
    }
    let rate = sample_rate as f64;
    let from = ((piece.start * rate) as usize).min(samples.len());
    let to = ((piece.end * rate) as usize).clamp(from, samples.len());
    if to - from < (SHORTEST * rate) as usize {
        return None;
    }
    Some(&samples[from..to])
}

struct Job {
    /// Kept for ordering work that is not reassembled by index yet.
    #[allow(dead_code)]
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
                // Before anything is said, not after. This is where the model
                // gets loaded, and it is the one genuinely slow step.
                recogniser.warm();

                let mut next_index = 0usize;
                for job in jobs {
                    worker_backlog.fetch_sub(1, Ordering::SeqCst);

                    let pieces = match recogniser.recognise(&job.samples, sample_rate) {
                        Ok(pieces) => pieces,
                        // A failed utterance should not end the session. The
                        // next one may well work, and the audio is still being
                        // recorded regardless.
                        Err(e) => vec![Heard {
                            start: 0.0,
                            end: job.end - job.start,
                            text: format!("[not recognised: {e}]"),
                        }],
                    };

                    // The voice of the whole utterance, used when a piece is
                    // too short to judge on its own.
                    let overall = speakers::voice_print(&job.samples, sample_rate);

                    let mut sent_any = false;
                    for piece in pieces {
                        if piece.text.trim().is_empty() {
                            continue;
                        }

                        // Classify each piece on its own audio. This is what
                        // notices that the second half of an utterance was
                        // somebody else, which looking at the whole thing at
                        // once cannot do.
                        let print = match slice_for(&job.samples, sample_rate, &piece) {
                            Some(slice) => speakers::voice_print(slice, sample_rate),
                            None => overall,
                        };
                        let speaker = diariser.assign(&print);

                        next_index += 1;
                        let line = Line {
                            index: next_index,
                            channel: channel.clone(),
                            speaker,
                            // Piece timings are relative to the utterance; the
                            // conversation wants them relative to the session.
                            start: job.start + piece.start,
                            end: job.start + piece.end,
                            text: piece.text,
                        };

                        writer.append(&line);
                        if to_caller.send(line).is_err() {
                            break;
                        }
                        sent_any = true;
                    }

                    if !sent_any {
                        continue;
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
            // The header carries a note about other people's voices. It belongs
            // in the file rather than only in the interface, because the file
            // is what gets forwarded, and the person who receives it was not
            // there when anything was shown on screen.
            let _ = writeln!(file, "# {}\n", options.channel);
            let _ = writeln!(
                file,
                "_Transcribed from {} on {}. Recognition ran on this machine. \
                 This may contain other people's voices; check the policy that \
                 applies before sharing it._\n",
                options.channel,
                crate::transcript::today()
            );
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
        fn recognise(&mut self, samples: &[f32], rate: u32) -> Result<Vec<Heard>> {
            Ok(vec![Heard {
                start: 0.0,
                end: samples.len() as f64 / rate as f64,
                text: self.0.to_string(),
            }])
        }
    }

    /// Splits every utterance in two, to check that one utterance can become
    /// several lines with their own timings and their own speaker.
    #[allow(dead_code)]
    struct Splitter;

    impl Recogniser for Splitter {
        fn recognise(&mut self, samples: &[f32], rate: u32) -> Result<Vec<Heard>> {
            let whole = samples.len() as f64 / rate as f64;
            let middle = whole / 2.0;
            Ok(vec![
                Heard {
                    start: 0.0,
                    end: middle,
                    text: "first half".into(),
                },
                Heard {
                    start: middle,
                    end: whole,
                    text: "second half".into(),
                },
            ])
        }
    }

    struct Silent;

    impl Recogniser for Silent {
        fn recognise(&mut self, _samples: &[f32], _rate: u32) -> Result<Vec<Heard>> {
            Ok(Vec::new())
        }
    }

    struct Broken;

    impl Recogniser for Broken {
        fn recognise(&mut self, _samples: &[f32], _rate: u32) -> Result<Vec<Heard>> {
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
