//! `ssr discover` — what is making sound right now.
//!
//! The command line half of autodiscovery. It exists for two reasons beyond
//! convenience. It is how someone confirms, before recording anything, that the
//! thing they care about really does surface as an audio session on this
//! machine — the Citrix question the whole tool turns on. And it is how the
//! discovery engine is exercised on a real machine without the desktop
//! application in the way.

use std::io::Write;
use std::time::Duration;

use anyhow::Result;
use ssr_core::sessions::{Discovery, SessionState};
use ssr_core::{DeviceKind, SessionMonitor};

/// A horizontal level bar, in the width given.
fn bar(peak: f32, width: usize) -> String {
    // Levels are drawn on a decibel scale, not a linear one: on a linear scale
    // everything below about a third of full scale is indistinguishable, which
    // is where nearly all real programme material sits.
    let db = if peak <= 0.000_001 {
        -60.0
    } else {
        20.0 * peak.log10()
    };
    let fraction = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
    let filled = (fraction * width as f32).round() as usize;
    let mut out = String::with_capacity(width);
    for i in 0..width {
        out.push(if i < filled { '#' } else { '.' });
    }
    out
}

fn db_text(peak: f32) -> String {
    if peak <= 0.000_001 {
        "  -inf".to_string()
    } else {
        format!("{:6.1}", 20.0 * peak.log10())
    }
}

fn state_text(state: SessionState) -> &'static str {
    match state {
        SessionState::Active => "playing",
        SessionState::Inactive => "idle",
        SessionState::Expired => "gone",
    }
}

/// Print the endpoints with their channel layouts.
pub fn print_endpoints(discovery: &Discovery) {
    for kind in [DeviceKind::Render, DeviceKind::Capture] {
        let list: Vec<_> = discovery
            .devices
            .iter()
            .filter(|d| d.kind == kind)
            .collect();
        if list.is_empty() {
            continue;
        }
        println!(
            "{} endpoints (record these with {}):",
            if kind == DeviceKind::Render {
                "Playback"
            } else {
                "Input"
            },
            if kind == DeviceKind::Render {
                "--system"
            } else {
                "--mic"
            }
        );
        for d in list {
            let marker = if d.is_default { "*" } else { " " };
            println!("  {marker} {}", d.name);
            match &d.format {
                Some(f) => {
                    let labels: Vec<&str> =
                        f.channels_detail.iter().map(|c| c.label.as_str()).collect();
                    println!(
                        "      {} Hz  {}  {}  [{}]",
                        f.sample_rate,
                        f.sample_format,
                        f.layout,
                        labels.join(" ")
                    );
                }
                None => println!("      format unavailable"),
            }
            if let Some(ff) = &d.form_factor {
                let adapter = d.adapter.as_deref().unwrap_or("");
                if adapter.is_empty() {
                    println!("      {ff}");
                } else {
                    println!("      {ff} · {adapter}");
                }
            }
            println!("      {}", d.id);
        }
        if kind == DeviceKind::Render {
            println!("  (* is the current default)");
        }
        println!();
    }
}

/// Print the recordable sources discovery found, loudest and most relevant
/// first, with the flag needed to record each one.
pub fn print_sources(discovery: &Discovery) {
    println!("Applications discovered (record any of these with --app):");
    if discovery.sources.is_empty() {
        println!("  nothing is playing and no recognised application is running");
    }
    for s in &discovery.sources {
        let star = if s.recommended { "*" } else { " " };
        let what = if s.playing {
            "playing"
        } else if s.capturing {
            "listening"
        } else if s.sessions > 0 {
            "idle"
        } else {
            "running"
        };
        println!(
            "  {star} {} {}  --app {:<20} {}",
            bar(s.peak, 14),
            db_text(s.peak),
            s.exe,
            s.label
        );
        let endpoints = if s.endpoints.is_empty() {
            String::new()
        } else {
            format!(" → {}", s.endpoints.join(", "))
        };
        println!(
            "      {what}, {} session(s), {}{}",
            s.sessions,
            pid_summary(&s.pids),
            endpoints
        );
    }
    if discovery.sources.iter().any(|s| s.recommended) {
        println!("  (* is a remote session, the audio that exists nowhere else)");
    }
}

/// Print every session, including the ones that are not offered as sources.
pub fn print_sessions(discovery: &Discovery) {
    println!("Audio sessions on this machine:");
    if discovery.sessions.is_empty() {
        println!("  none");
        return;
    }
    for s in &discovery.sessions {
        let direction = if s.endpoint_kind == DeviceKind::Render {
            "out"
        } else {
            "in "
        };
        println!(
            "  {} {} {:<7} {direction} {:<28} {}",
            bar(s.peak, 14),
            db_text(s.peak),
            state_text(s.state),
            truncate(&s.label, 28),
            truncate(&s.endpoint_name, 30),
        );
        if s.channel_peaks.len() > 1 {
            let per: Vec<String> = s
                .channel_peaks
                .iter()
                .map(|p| format!("{:>6}", db_text(*p).trim()))
                .collect();
            println!("      channels: {}", per.join(" "));
        }
        if s.muted {
            println!("      muted by the application");
        }
    }
}

/// Process ids, abbreviated.
///
/// A browser can have thirty processes under one name and listing them all
/// buries the row that matters. The count is the useful part; the first few ids
/// are there for anyone who wants to pin one with `--app PID`.
fn pid_summary(pids: &[u32]) -> String {
    match pids.len() {
        0 => "not running".to_string(),
        1 => format!("pid {}", pids[0]),
        n if n <= 4 => format!(
            "pids {}",
            pids.iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        n => format!(
            "{n} processes, pids {} and {} more",
            pids.iter()
                .take(3)
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            n - 3
        ),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// One-shot discovery report.
pub fn run(json: bool, sessions: bool) -> Result<()> {
    let discovery = ssr_core::discover_once()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&discovery)?);
        return Ok(());
    }
    print_endpoints(&discovery);
    print_sources(&discovery);
    if sessions {
        println!();
        print_sessions(&discovery);
    }
    if let Some(e) = &discovery.error {
        println!();
        println!("note: one endpoint could not be read: {e}");
    }
    println!();
    println!("Discovery took {:.1} ms.", discovery.elapsed_ms);
    Ok(())
}

/// Live discovery, redrawn until interrupted.
///
/// This is the mode that answers "is Citrix audio really visible to this tool":
/// start a call, watch the row light up.
pub fn watch(interval_ms: u64, sessions: bool) -> Result<()> {
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    {
        let flag = std::sync::Arc::clone(&running);
        ctrlc::set_handler(move || flag.store(false, std::sync::atomic::Ordering::Relaxed))?;
    }

    // The monitor owns its own thread and apartment, so the terminal thread
    // does nothing but draw.
    let monitor = SessionMonitor::start(interval_ms);
    let mut stdout = std::io::stdout();

    while running.load(std::sync::atomic::Ordering::Relaxed) {
        let discovery = monitor.latest();
        // Nothing to draw until the first pass has completed.
        if discovery.generation > 0 {
            // Home the cursor and clear forwards, rather than clearing the
            // whole screen first: clearing makes the display flicker.
            print!("\x1b[H\x1b[J");
            println!("ssr discover — watching, Ctrl-C to stop");
            println!(
                "pass {} in {:.1} ms, refreshing every {} ms",
                discovery.generation,
                discovery.elapsed_ms,
                monitor.interval_ms()
            );
            println!();
            if sessions {
                print_sessions(&discovery);
            } else {
                print_sources(&discovery);
            }
            let _ = stdout.flush();
        }
        std::thread::sleep(Duration::from_millis(interval_ms.max(50)));
    }

    println!();
    Ok(())
}
