//! The `transcribe` subcommand: turn a finished recording into text.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};

use ssr_transcribe::job::{TranscribeRequest, TranscribeResult};
use ssr_transcribe::TranscriptFormat;

use crate::TranscribeArgs;

pub fn run(args: TranscribeArgs) -> Result<()> {
    let engine = crate::listen::choose_engine(args.engine.as_deref())?;

    let session_dir = PathBuf::from(&args.session);
    if !session_dir.join("session.json").exists() {
        bail!(
            "{} does not look like a recorded session; expected a session.json in it",
            session_dir.display()
        );
    }

    let mut formats = Vec::new();
    for name in args.format.split(',') {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        match TranscriptFormat::parse(name) {
            Some(f) => formats.push(f),
            None => bail!("unknown format {name:?}; choose from txt, srt, vtt, json and md"),
        }
    }

    let request = TranscribeRequest {
        session_dir,
        output_dir: args.out.map(PathBuf::from),
        tracks: args
            .track
            .iter()
            .flat_map(|t| t.split(','))
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        formats,
        language: args.language,
        model: args.model,
        translate: args.translate,
        combine: !args.no_combine,
        skip_silent: !args.include_silent,
    };

    eprintln!("Transcribing with {}.", engine.label);
    eprintln!("This is slower than listening live; a long track takes minutes.\n");

    let result = ssr_transcribe::transcribe_session(&engine, &request, |p| {
        eprintln!("[{}/{}] {} — {}", p.index, p.total, p.track, p.stage);
    })
    .context("transcription failed")?;

    report(&result);
    Ok(())
}

fn report(result: &TranscribeResult) {
    println!();
    for track in &result.tracks {
        if let Some(reason) = &track.skipped {
            println!("  {:<16} skipped, {reason}", track.track);
            continue;
        }
        if let Some(error) = &track.error {
            println!("  {:<16} failed: {error}", track.track);
            continue;
        }
        println!(
            "  {:<16} {} words, {} file(s)",
            track.track,
            track.words,
            track.files.len()
        );
    }
    println!();
    println!("Written to {}", result.output_dir.display());
    for file in &result.files {
        if let Some(name) = file.file_name() {
            println!("  {}", name.to_string_lossy());
        }
    }
}
