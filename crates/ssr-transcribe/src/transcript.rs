//! The transcript model, and the files it can be written to.
//!
//! Engines disagree about their output formats, so everything is parsed into
//! this one shape and written back out from it. That way the files look the
//! same whichever engine produced them.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// One stretch of speech.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    /// Seconds from the start of the session.
    pub start: f64,
    pub end: f64,
    pub text: String,
}

impl Segment {
    pub fn duration(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }
}

/// Everything recognised in one track.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    /// Track name, matching the audio file and the session manifest.
    pub track: String,
    /// What the track was recording, in words.
    pub source: String,
    /// Language the engine detected or was told to expect.
    pub language: String,
    /// Engine that produced this, for the record.
    pub engine: String,
    /// Model the engine used, when it reported one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub segments: Vec<Segment>,
}

impl Transcript {
    /// Everything said, as flowing text.
    pub fn plain_text(&self) -> String {
        let mut out = String::new();
        for segment in &self.segments {
            let text = segment.text.trim();
            if text.is_empty() {
                continue;
            }
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(text);
        }
        out
    }

    pub fn duration(&self) -> f64 {
        self.segments.last().map(|s| s.end).unwrap_or(0.0)
    }

    /// Number of words, for a sense of how much was actually said.
    pub fn word_count(&self) -> usize {
        self.plain_text().split_whitespace().count()
    }
}

/// A file format a transcript can be written to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptFormat {
    /// Flowing text with no timings. What you paste into a document.
    Txt,
    /// SubRip subtitles, which most players and editors read.
    Srt,
    /// WebVTT subtitles, for the web.
    Vtt,
    /// The full structure, for anything that wants to process it further.
    Json,
    /// Markdown with timestamps, which reads well in notes.
    Md,
}

impl TranscriptFormat {
    pub fn extension(self) -> &'static str {
        match self {
            TranscriptFormat::Txt => "txt",
            TranscriptFormat::Srt => "srt",
            TranscriptFormat::Vtt => "vtt",
            TranscriptFormat::Json => "json",
            TranscriptFormat::Md => "md",
        }
    }

    /// A short description, for a chooser in an interface.
    pub fn describe(self) -> &'static str {
        match self {
            TranscriptFormat::Txt => "Plain text, no timings",
            TranscriptFormat::Srt => "SubRip subtitles",
            TranscriptFormat::Vtt => "WebVTT subtitles",
            TranscriptFormat::Json => "Structured, with timings",
            TranscriptFormat::Md => "Markdown with timestamps",
        }
    }

    pub fn all() -> [TranscriptFormat; 5] {
        [
            TranscriptFormat::Txt,
            TranscriptFormat::Srt,
            TranscriptFormat::Vtt,
            TranscriptFormat::Json,
            TranscriptFormat::Md,
        ]
    }

    /// Parse a format name, as typed on a command line.
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_lowercase().as_str() {
            "txt" | "text" => Some(TranscriptFormat::Txt),
            "srt" => Some(TranscriptFormat::Srt),
            "vtt" | "webvtt" => Some(TranscriptFormat::Vtt),
            "json" => Some(TranscriptFormat::Json),
            "md" | "markdown" => Some(TranscriptFormat::Md),
            _ => None,
        }
    }
}

/// `HH:MM:SS,mmm`, the form SubRip uses.
fn srt_time(seconds: f64) -> String {
    let total = seconds.max(0.0);
    let ms = (total * 1000.0).round() as u64;
    format!(
        "{:02}:{:02}:{:02},{:03}",
        ms / 3_600_000,
        (ms % 3_600_000) / 60_000,
        (ms % 60_000) / 1000,
        ms % 1000
    )
}

/// `HH:MM:SS.mmm`, the form WebVTT uses.
fn vtt_time(seconds: f64) -> String {
    srt_time(seconds).replace(',', ".")
}

/// `H:MM:SS`, for reading rather than for machines.
fn friendly_time(seconds: f64) -> String {
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

/// Render a transcript in one format.
pub fn render(transcript: &Transcript, format: TranscriptFormat) -> Result<String> {
    let mut out = String::new();
    match format {
        TranscriptFormat::Txt => {
            out.push_str(&transcript.plain_text());
            out.push('\n');
        }
        TranscriptFormat::Srt => {
            // SubRip numbers its cues from one, and every cue needs text.
            let mut n = 1;
            for segment in &transcript.segments {
                let text = segment.text.trim();
                if text.is_empty() {
                    continue;
                }
                writeln!(out, "{n}")?;
                writeln!(
                    out,
                    "{} --> {}",
                    srt_time(segment.start),
                    srt_time(segment.end)
                )?;
                writeln!(out, "{text}")?;
                out.push('\n');
                n += 1;
            }
        }
        TranscriptFormat::Vtt => {
            out.push_str("WEBVTT\n\n");
            for segment in &transcript.segments {
                let text = segment.text.trim();
                if text.is_empty() {
                    continue;
                }
                writeln!(
                    out,
                    "{} --> {}",
                    vtt_time(segment.start),
                    vtt_time(segment.end)
                )?;
                writeln!(out, "{text}")?;
                out.push('\n');
            }
        }
        TranscriptFormat::Json => {
            out.push_str(&serde_json::to_string_pretty(transcript)?);
            out.push('\n');
        }
        TranscriptFormat::Md => {
            writeln!(out, "# {}", transcript.track)?;
            out.push('\n');
            writeln!(out, "{}", transcript.source)?;
            out.push('\n');
            writeln!(
                out,
                "{} words, {} spoken, {} via {}",
                transcript.word_count(),
                friendly_time(transcript.duration()),
                transcript.language,
                transcript.engine
            )?;
            out.push('\n');
            for segment in &transcript.segments {
                let text = segment.text.trim();
                if text.is_empty() {
                    continue;
                }
                writeln!(out, "**{}** {text}", friendly_time(segment.start))?;
                out.push('\n');
            }
        }
    }
    Ok(out)
}

/// Write a transcript to `directory/<stem>.<ext>` and return the path.
pub fn write(
    transcript: &Transcript,
    directory: &Path,
    stem: &str,
    format: TranscriptFormat,
) -> Result<PathBuf> {
    std::fs::create_dir_all(directory)
        .with_context(|| format!("creating {}", directory.display()))?;
    let path = directory.join(format!("{stem}.{}", format.extension()));
    let body = render(transcript, format)?;
    std::fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

/// Combine several tracks into one transcript ordered by time.
///
/// This is the point of recording applications separately: two tracks become
/// one conversation with each line already attributed to its source, which no
/// amount of speaker separation on a mixed recording would recover reliably.
pub fn merge(transcripts: &[Transcript]) -> String {
    let mut lines: Vec<(f64, &str, &str)> = Vec::new();
    for t in transcripts {
        for segment in &t.segments {
            let text = segment.text.trim();
            if !text.is_empty() {
                lines.push((segment.start, t.track.as_str(), text));
            }
        }
    }
    lines.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut out = String::new();
    let mut last_track = "";
    for (start, track, text) in lines {
        if track != last_track {
            if !out.is_empty() {
                out.push('\n');
            }
            let _ = writeln!(out, "## {track}");
            last_track = track;
        }
        let _ = writeln!(out, "**{}** {text}", friendly_time(start));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Transcript {
        Transcript {
            track: "wfica32".into(),
            source: "process tree wfica32.exe".into(),
            language: "en".into(),
            engine: "whisper.cpp".into(),
            model: Some("base.en".into()),
            segments: vec![
                Segment {
                    start: 0.0,
                    end: 2.5,
                    text: " Good morning everyone.".into(),
                },
                Segment {
                    start: 2.5,
                    end: 4.0,
                    text: "  ".into(),
                },
                Segment {
                    start: 4.0,
                    end: 7.25,
                    text: "Shall we begin?".into(),
                },
            ],
        }
    }

    #[test]
    fn plain_text_joins_segments_and_drops_blank_ones() {
        assert_eq!(
            sample().plain_text(),
            "Good morning everyone. Shall we begin?"
        );
    }

    #[test]
    fn word_count_ignores_the_blank_segment() {
        assert_eq!(sample().word_count(), 6);
    }

    #[test]
    fn srt_numbers_cues_from_one_and_skips_blanks() {
        let out = render(&sample(), TranscriptFormat::Srt).unwrap();
        assert!(out.starts_with("1\n00:00:00,000 --> 00:00:02,500\nGood morning everyone.\n"));
        // The blank middle segment must not consume cue number two.
        assert!(out.contains("2\n00:00:04,000 --> 00:00:07,250\nShall we begin?"));
        assert!(!out.contains("3\n"));
    }

    #[test]
    fn vtt_has_its_header_and_dotted_timestamps() {
        let out = render(&sample(), TranscriptFormat::Vtt).unwrap();
        assert!(out.starts_with("WEBVTT\n"));
        assert!(out.contains("00:00:00.000 --> 00:00:02.500"));
    }

    #[test]
    fn timestamps_carry_past_an_hour() {
        assert_eq!(srt_time(3661.5), "01:01:01,500");
        assert_eq!(friendly_time(3661.0), "1:01:01");
        assert_eq!(friendly_time(75.0), "1:15");
    }

    #[test]
    fn json_round_trips() {
        let out = render(&sample(), TranscriptFormat::Json).unwrap();
        let back: Transcript = serde_json::from_str(&out).unwrap();
        assert_eq!(back.track, "wfica32");
        assert_eq!(back.segments.len(), 3);
    }

    #[test]
    fn merging_orders_by_time_and_labels_each_track() {
        let mut mic = sample();
        mic.track = "mic".into();
        mic.segments = vec![Segment {
            start: 3.0,
            end: 3.8,
            text: "Yes, go ahead.".into(),
        }];

        let merged = merge(&[sample(), mic]);
        let citrix_first = merged.find("Good morning").unwrap();
        let mic_reply = merged.find("Yes, go ahead").unwrap();
        let citrix_second = merged.find("Shall we begin").unwrap();
        assert!(citrix_first < mic_reply && mic_reply < citrix_second);
        assert!(merged.contains("## wfica32") && merged.contains("## mic"));
    }

    #[test]
    fn format_names_parse_both_ways() {
        for f in TranscriptFormat::all() {
            assert_eq!(TranscriptFormat::parse(f.extension()), Some(f));
        }
        assert_eq!(
            TranscriptFormat::parse("markdown"),
            Some(TranscriptFormat::Md)
        );
        assert_eq!(TranscriptFormat::parse("nonsense"), None);
    }
}
