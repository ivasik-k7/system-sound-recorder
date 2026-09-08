//! The `listen` subcommand: transcribe a channel while it is playing.
//!
//! The other mode records audio and leaves files behind. This one reads a
//! conversation as it happens, prints it like a chat, and writes each line to
//! disk the moment it is recognised.

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use crossbeam_channel::{bounded, RecvTimeoutError};

use ssr_core::{Packet, SourceHandle};
use ssr_session::SourceRequest;
use ssr_transcribe::audio::StreamResampler;
use ssr_transcribe::live::{EngineRecogniser, Line, Listener, LiveFormat, Options};
use ssr_transcribe::{engine, whisper};

use crate::ListenArgs;

/// Everything the engine reads is at this rate.
const ENGINE_RATE: u32 = 16_000;

pub fn run(args: ListenArgs) -> Result<()> {
    let engine = choose_engine(args.engine.as_deref())?;

    let request = source_request(&args)?;
    let channel = args
        .label
        .clone()
        .unwrap_or_else(|| default_label(&request));

    let transcript_path = resolve_output(&args, &channel);
    let format = match args.format.as_str() {
        "md" | "markdown" => LiveFormat::Markdown,
        "txt" | "text" => LiveFormat::Text,
        "jsonl" | "json" => LiveFormat::Jsonl,
        other => bail!("unknown transcript format {other:?}; use md, txt or jsonl"),
    };

    // Engine scratch files go somewhere temporary, not next to the transcript.
    let work_dir = std::env::temp_dir().join(format!("ssr-listen-{}", std::process::id()));

    let recogniser = EngineRecogniser::new(
        engine.clone(),
        work_dir.clone(),
        whisper::Options {
            language: args.language.clone(),
            model: args.model.clone(),
            threads: args.threads,
            translate: args.translate,
        },
    )?;

    let mut options = Options::for_channel(channel.clone());
    options.detect_speakers = !args.no_speakers;
    options.language = args.language.clone();
    options.model = args.model.clone();
    options.translate = args.translate;
    options.transcript_path = Some(transcript_path.clone());
    options.transcript_format = format;

    // Capture first, so the endpoint's real format is known before the
    // listener is built around it.
    let (tx, rx) = bounded::<(ssr_core::source::TrackKey, Packet)>(512);
    let spec = request.to_spec(48_000, 2);
    let handle: SourceHandle = ssr_core::start("listen", spec, args.buffer_ms, tx)
        .context("that channel could not be opened")?;

    let source_rate = handle.format.sample_rate;
    let source_channels = handle.format.channels.max(1) as usize;

    let mut listener = Listener::start(ENGINE_RATE, options, Box::new(recogniser))?;
    let mut resampler = StreamResampler::new(source_rate, ENGINE_RATE);

    eprintln!("Listening to {channel}  [{}]", handle.description);
    eprintln!(
        "Engine: {}{} · writing to {}",
        engine.label,
        if engine.is_resident_capable() {
            " · held open, so the model loads once"
        } else {
            ""
        },
        transcript_path.display()
    );
    if !engine.is_resident_capable() {
        // Worth saying plainly. Restarting the engine for every utterance costs
        // five to fifteen seconds each time, which is the difference between
        // transcription happening quietly and the machine visibly grinding.
        eprintln!(
            "  Note: this engine is restarted for every utterance. Installing \
             whisper.cpp,\n  or an openai-whisper whose interpreter sits beside \
             it, would keep it loaded."
        );
    }
    if args.no_speakers {
        eprintln!("Speaker detection off; every line is attributed to the channel.");
    }
    match args.duration {
        Some(d) => eprintln!("Stopping after {d} s, or on Ctrl-C.\n"),
        None => eprintln!("Press Ctrl-C to stop.\n"),
    }

    let interrupted = Arc::new(AtomicBool::new(false));
    {
        let flag = interrupted.clone();
        let _ = ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst));
    }

    let started = Instant::now();
    let mut mono = Vec::new();
    let mut printed = 0usize;

    // A loopback stream delivers nothing at all while the endpoint is quiet, so
    // without this the detector's clock would only advance when someone was
    // speaking. Two things would go wrong: timestamps would collapse towards
    // zero, and an utterance would never close, because the silence that ends
    // it would never arrive. Feeding real silence in fixes both.
    let top_up_silence = |listener: &mut Listener, elapsed: f64| {
        // Stay behind the present by more than a capture buffer, or this races
        // audio still in flight and inserts silence where speech belongs.
        // Stay behind the present by a little more than a capture buffer. Any
        // further back and the silence that ends an utterance arrives late,
        // which is added directly to how long the speaker waits.
        let target = elapsed - 0.3;
        let behind = target - listener.heard_secs();
        if behind <= 0.05 {
            return;
        }
        let samples = (behind.min(5.0) * ENGINE_RATE as f64) as usize;
        listener.push(&vec![0.0f32; samples]);
    };

    loop {
        if interrupted.load(Ordering::SeqCst) {
            break;
        }
        if let Some(limit) = args.duration {
            if started.elapsed().as_secs_f64() >= limit {
                break;
            }
        }

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

        top_up_silence(&mut listener, started.elapsed().as_secs_f64());

        for line in listener.drain() {
            print_line(&line);
            printed += 1;
        }
    }

    handle.signal_stop();
    // Anything the capture threads still hold has to be received, or they
    // cannot exit; and the tail of the audio is often the interesting part.
    let drain_until = Instant::now() + Duration::from_millis(400);
    while Instant::now() < drain_until {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok((_, packet)) => {
                mono.clear();
                downmix(&packet.samples, source_channels, &mut mono);
                let converted = resampler.push(&mono);
                if !converted.is_empty() {
                    listener.push(&converted);
                }
            }
            Err(_) => break,
        }
    }
    let tail = resampler.flush();
    if !tail.is_empty() {
        listener.push(&tail);
    }
    // Bring the clock up to the moment recording stopped, so a final utterance
    // is closed by silence rather than by the stream simply ending.
    top_up_silence(&mut listener, started.elapsed().as_secs_f64());

    let backlog = listener.backlog();
    if backlog > 0 {
        eprintln!("\nRecognising the last {backlog} utterance(s)…");
    }
    let heard = listener.heard_secs();
    for line in listener.finish() {
        print_line(&line);
        printed += 1;
    }

    let _ = std::fs::remove_dir_all(&work_dir);

    println!();
    println!(
        "Listened for {:.0} s, {} line(s) transcribed.",
        heard, printed
    );
    println!("Transcript: {}", transcript_path.display());
    Ok(())
}

/// One line of conversation, printed the way a chat reads.
fn print_line(line: &Line) {
    // Bold the speaker so the eye can follow who is talking down the left.
    println!(
        "\x1b[1m{:>8}\x1b[0m  \x1b[2m{}\x1b[0m",
        clock(line.start),
        line.speaker.label
    );
    println!("          {}", line.text);
    println!();
    let _ = std::io::stdout().flush();
}

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

/// Pick the engine to use, or explain what to install.
pub fn choose_engine(wanted: Option<&str>) -> Result<engine::Engine> {
    let found = engine::detect();
    if found.is_empty() {
        bail!("{}", engine::installation_advice());
    }

    if let Some(name) = wanted {
        let needle = name.to_lowercase();
        return found
            .into_iter()
            .find(|e| e.label.to_lowercase().contains(&needle))
            .ok_or_else(|| anyhow::anyhow!("no installed engine matches {name:?}"));
    }

    let usable: Vec<_> = found.iter().filter(|e| e.is_usable()).cloned().collect();
    match usable.into_iter().next() {
        Some(e) => Ok(e),
        // Something is installed but cannot run, which is almost always a
        // missing model. Saying so beats saying nothing was found.
        None => bail!(
            "{} is installed but has no model to work with.\n\n{}",
            found[0].label,
            engine::installation_advice()
        ),
    }
}

/// Turn the command line arguments into the one channel to listen to.
fn source_request(args: &ListenArgs) -> Result<SourceRequest> {
    let chosen = [
        args.app.is_some(),
        args.system.is_some(),
        args.mic.is_some(),
    ]
    .iter()
    .filter(|c| **c)
    .count();

    if chosen > 1 {
        bail!("listen takes one channel at a time; choose --app, --system or --mic");
    }

    if let Some(target) = &args.app {
        return Ok(SourceRequest::App {
            target: target.clone(),
            exclude: false,
            label: None,
        });
    }
    if let Some(device) = &args.mic {
        return Ok(SourceRequest::Input {
            device: device.clone(),
            label: None,
        });
    }
    // Defaulting to the system mix means `ssr listen` on its own does the
    // obvious thing: transcribe whatever this machine is playing.
    Ok(SourceRequest::System {
        device: args.system.clone().unwrap_or_else(|| "default".into()),
        label: None,
    })
}

fn default_label(request: &SourceRequest) -> String {
    match request {
        SourceRequest::App { target, .. } => target
            .trim_end_matches(".exe")
            .trim_end_matches(".EXE")
            .to_string(),
        SourceRequest::Input { .. } => "Microphone".to_string(),
        SourceRequest::System { .. } => "System".to_string(),
    }
}

fn resolve_output(args: &ListenArgs, channel: &str) -> PathBuf {
    if let Some(out) = &args.out {
        return PathBuf::from(out);
    }
    let stamp = ssr_core::local_timestamp();
    let safe = ssr_timeline::sanitize_filename(channel);
    let extension = match args.format.as_str() {
        "txt" | "text" => "txt",
        "jsonl" | "json" => "jsonl",
        _ => "md",
    };
    // Absolute, not relative to wherever this was run from. A transcript is
    // the content of a conversation, and dropping one into whatever directory
    // happened to be current is how it ends up somewhere it should not be.
    ssr_transcribe::default_transcript_dir().join(format!("{stamp}-{safe}.{extension}"))
}
