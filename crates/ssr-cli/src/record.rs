//! The `rec` subcommand: a thin driver over the shared session controller.

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use ssr_session::{OutputTarget, Recorder, SessionConfig, SourceRequest};

use crate::RecArgs;

pub fn run(args: RecArgs) -> Result<()> {
    let sources = plan_sources(&args);
    let output = resolve_output(&args, sources.len())?;

    let config = SessionConfig {
        sources,
        output,
        buffer_ms: args.buffer_ms,
        sample_rate: args.rate,
        channels: args.channels,
        duration_secs: args.duration,
    };

    let mut recorder = Recorder::start(config)?;

    if !args.quiet {
        let snap = recorder.snapshot();
        eprintln!(
            "Recording {} track(s) to {}",
            snap.tracks.len(),
            snap.output_dir
        );
        for t in &snap.tracks {
            eprintln!(
                "  {:<16} {}  [{} Hz, {} ch]",
                t.name, t.description, t.sample_rate, t.channels
            );
        }
        for w in &snap.warnings {
            eprintln!("  warning: {w}");
        }
        match args.duration {
            Some(d) => eprintln!("Stopping after {d} s, or on Ctrl-C."),
            None => eprintln!("Press Ctrl-C to stop."),
        }
    }

    let interrupted = Arc::new(AtomicBool::new(false));
    {
        let flag = interrupted.clone();
        // An error here only means a handler is already installed, which cannot
        // happen in this binary; recording continues either way.
        let _ = ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst));
    }

    let mut last_draw = Instant::now();
    while recorder.is_running() {
        if interrupted.load(Ordering::SeqCst) {
            recorder.request_stop();
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        if !args.quiet && last_draw.elapsed() >= Duration::from_millis(250) {
            draw_status(&recorder.snapshot());
            last_draw = Instant::now();
        }
    }

    let manifest = recorder.stop()?;
    if !args.quiet {
        eprintln!();
    }

    if args.json {
        println!("{}", manifest.to_json());
        return Ok(());
    }

    println!(
        "Recorded {:.1} s to {}",
        manifest.duration_secs,
        recorder.output_dir().display()
    );
    for t in &manifest.tracks {
        let total = (t.stats.frames_captured + t.stats.frames_silence_filled).max(1);
        println!(
            "  {:<16} {:<28} {:.1} s, {:.0}% captured, {:.0}% silence",
            t.name,
            t.file,
            t.duration_secs,
            t.stats.frames_captured as f64 * 100.0 / total as f64,
            t.stats.frames_silence_filled as f64 * 100.0 / total as f64,
        );
        if t.stats.discontinuities > 0 {
            println!(
                "  {:<16} {} dropout(s) reported by the audio engine",
                "", t.stats.discontinuities
            );
        }
    }
    for w in &manifest.warnings {
        println!("  warning: {w}");
    }
    println!(
        "  manifest: {}",
        recorder.output_dir().join("session.json").display()
    );
    Ok(())
}

/// One-line live meter, rewritten in place.
fn draw_status(snap: &ssr_session::SessionSnapshot) {
    let mut line = format!("\r  {:>6.1}s ", snap.elapsed_secs);
    for t in &snap.tracks {
        line.push_str(&format!("| {} {} ", t.name, meter(t.peak_db)));
    }
    line.push_str("   ");
    eprint!("{line}");
    let _ = std::io::stderr().flush();
}

/// A five-cell bar plus the peak level in dBFS.
fn meter(peak_db: f32) -> String {
    const BLOCKS: [char; 5] = ['\u{2581}', '\u{2583}', '\u{2585}', '\u{2586}', '\u{2588}'];
    if peak_db <= -119.0 {
        return format!("{}  -inf dB", "\u{2581}".repeat(5));
    }
    // Map -60 dBFS upwards onto the five cells.
    let filled = (((peak_db + 60.0) / 60.0).clamp(0.0, 1.0) * 5.0).ceil() as usize;
    let mut bar = String::new();
    for (i, block) in BLOCKS.iter().enumerate() {
        bar.push(if i < filled { *block } else { '\u{2581}' });
    }
    format!("{bar} {peak_db:>6.1} dB")
}

/// Turn command line arguments into source requests.
fn plan_sources(args: &RecArgs) -> Vec<SourceRequest> {
    let mut sources: Vec<SourceRequest> = Vec::new();

    if let Some(device) = &args.system {
        sources.push(SourceRequest::System {
            device: device.clone(),
            label: None,
        });
    }
    for target in &args.app {
        sources.push(SourceRequest::App {
            target: target.clone(),
            exclude: false,
            label: None,
        });
    }
    for target in &args.not_app {
        sources.push(SourceRequest::App {
            target: target.clone(),
            exclude: true,
            label: None,
        });
    }
    for device in &args.mic {
        sources.push(SourceRequest::Input {
            device: device.clone(),
            label: None,
        });
    }

    // With no source named at all, the useful default is everything mixed.
    if sources.is_empty() {
        sources.push(SourceRequest::System {
            device: "default".to_string(),
            label: None,
        });
    }
    sources
}

/// Decide where the session goes.
fn resolve_output(args: &RecArgs, track_count: usize) -> Result<OutputTarget> {
    if let Some(root) = &args.out_root {
        return Ok(OutputTarget::NewSessionIn {
            root: PathBuf::from(root),
        });
    }

    let Some(out) = &args.out else {
        // Collect into ./recordings by default, one dated subfolder per run, so
        // repeated recordings accumulate rather than replacing one another.
        return Ok(OutputTarget::NewSessionIn {
            root: PathBuf::from("recordings"),
        });
    };

    let path = PathBuf::from(out);
    let is_wav = path
        .extension()
        .map(|e| e.eq_ignore_ascii_case("wav"))
        .unwrap_or(false);

    if !is_wav {
        return Ok(OutputTarget::Directory { path });
    }
    if track_count > 1 {
        bail!(
            "--out names a single .wav file but {track_count} tracks were requested; \
             give a directory instead"
        );
    }
    Ok(OutputTarget::File { path })
}
