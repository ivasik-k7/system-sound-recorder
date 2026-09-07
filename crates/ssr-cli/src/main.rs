//! `ssr` — record every audio stream reaching this machine.

mod discover;
mod listen;
mod record;
mod transcribe;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "ssr",
    // The tag and the commit, not the tag alone: between two tags there are
    // many builds, and a bug report needs to name one of them.
    version = ssr_core::version::FULL,
    about = "Record system audio: everything mixed, or one track per application.",
    long_about = "Record system audio on Windows.\n\n\
Audio from Citrix sessions, WSL, browsers and native applications all converge \
on the Windows audio engine, so a loopback capture records all of it. Naming \
applications instead splits them into separate, time-aligned tracks."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List audio endpoints that can be recorded from.
    Devices {
        /// Emit JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
    /// List running processes whose audio can be captured separately.
    Apps {
        /// Show every process, not just the ones making sound.
        #[arg(long)]
        all: bool,
        /// Emit JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
    /// Show what is playing audio right now, with live levels.
    ///
    /// Asks the Windows audio engine which streams exist rather than matching
    /// process names, so anything making a sound is found whether the recorder
    /// recognises it or not.
    Discover {
        /// Redraw continuously until interrupted.
        #[arg(short, long)]
        watch: bool,
        /// List every audio session individually instead of one row per
        /// application. Shows per-channel levels.
        #[arg(short, long)]
        sessions: bool,
        /// Refresh interval for --watch, in milliseconds.
        #[arg(long, default_value_t = 250, value_name = "MS")]
        interval: u64,
        /// Emit JSON instead of a table. Ignored with --watch.
        #[arg(long)]
        json: bool,
    },
    /// Record one or more tracks.
    Rec(RecArgs),

    /// Transcribe a channel while it is playing, as a conversation.
    ///
    /// The other way round from `rec`: instead of leaving audio files behind,
    /// this reads what is being said as it is said, prints it like a chat, and
    /// appends each line to a file the moment it is recognised.
    ///
    /// Needs a speech engine installed on the machine. Recognition is local, so
    /// nothing leaves this computer.
    Listen(ListenArgs),

    /// Turn a finished recording into text.
    ///
    /// Because the recorder keeps applications on separate tracks, each track
    /// transcribes into its own file already attributed to its source.
    Transcribe(TranscribeArgs),
}

#[derive(Args, Debug)]
pub struct ListenArgs {
    /// Listen to one application, by executable name or process id.
    #[arg(long, value_name = "PROCESS")]
    pub app: Option<String>,

    /// Listen to everything this machine plays. The default.
    #[arg(long, num_args = 0..=1, default_missing_value = "default", value_name = "DEVICE")]
    pub system: Option<String>,

    /// Listen to a microphone.
    #[arg(long, num_args = 0..=1, default_missing_value = "default", value_name = "DEVICE")]
    pub mic: Option<String>,

    /// Name to label this channel with. Defaults to the application or device.
    #[arg(long, value_name = "NAME")]
    pub label: Option<String>,

    /// Where to write the transcript. Defaults to ./transcripts/<time>-<channel>.
    #[arg(short, long, value_name = "FILE")]
    pub out: Option<String>,

    /// Transcript format: md, txt or jsonl.
    #[arg(long, default_value = "md")]
    pub format: String,

    /// Do not try to tell voices apart within the channel.
    ///
    /// Worth setting for a microphone with one person at it, where every line
    /// is that person and the guessing can only introduce mistakes.
    #[arg(long)]
    pub no_speakers: bool,

    /// Language code such as en, or leave it out to let the engine decide.
    #[arg(long, value_name = "CODE")]
    pub language: Option<String>,

    /// Model to use, overriding the engine's own choice.
    #[arg(long, value_name = "NAME")]
    pub model: Option<String>,

    /// Translate into English rather than transcribing as spoken.
    #[arg(long)]
    pub translate: bool,

    /// Speech engine to use when more than one is installed.
    #[arg(long, value_name = "NAME")]
    pub engine: Option<String>,

    /// Recognition threads. Defaults to the engine's own choice.
    #[arg(long, value_name = "N")]
    pub threads: Option<u32>,

    /// Capture buffer size in milliseconds.
    #[arg(long, default_value_t = 200, value_name = "MS")]
    pub buffer_ms: u32,

    /// Stop after this many seconds.
    #[arg(short, long, value_name = "SECONDS")]
    pub duration: Option<f64>,
}

#[derive(Args, Debug)]
pub struct TranscribeArgs {
    /// A session directory, the sort `rec` writes.
    pub session: String,

    /// Where the transcripts go. Defaults to the session directory.
    #[arg(short, long, value_name = "DIR")]
    pub out: Option<String>,

    /// Tracks to transcribe. Repeatable, or comma separated. Default is all.
    #[arg(long, value_name = "NAME", action = clap::ArgAction::Append)]
    pub track: Vec<String>,

    /// Formats to write, comma separated: txt, srt, vtt, json, md.
    #[arg(long, default_value = "txt,srt")]
    pub format: String,

    /// Language code such as en, or leave it out to let the engine decide.
    #[arg(long, value_name = "CODE")]
    pub language: Option<String>,

    /// Model to use, overriding the engine's own choice.
    #[arg(long, value_name = "NAME")]
    pub model: Option<String>,

    /// Translate into English rather than transcribing as spoken.
    #[arg(long)]
    pub translate: bool,

    /// Speech engine to use when more than one is installed.
    #[arg(long, value_name = "NAME")]
    pub engine: Option<String>,

    /// Do not write the combined file that interleaves every track by time.
    #[arg(long)]
    pub no_combine: bool,

    /// Transcribe silent tracks too, instead of passing over them.
    #[arg(long)]
    pub include_silent: bool,
}

#[derive(Args, Debug)]
pub struct RecArgs {
    /// Where to write this session. A directory holding one WAV per track plus
    /// session.json, or a single .wav path when recording exactly one track.
    ///
    /// The directory is used exactly as given, so recording twice to the same
    /// one replaces the first session. Use --out-root to keep both.
    #[arg(short, long)]
    pub out: Option<String>,

    /// A folder to collect sessions in, each in its own dated subfolder.
    ///
    /// Defaults to ./recordings when neither this nor --out is given.
    #[arg(long, value_name = "DIR", conflicts_with = "out")]
    pub out_root: Option<String>,

    /// Record everything played through a render endpoint, mixed together.
    ///
    /// Give an endpoint name or id to pick a specific one. This is the source
    /// that catches Citrix and WSL audio without naming any process.
    #[arg(long, num_args = 0..=1, default_missing_value = "default", value_name = "DEVICE")]
    pub system: Option<String>,

    /// Record one application's audio on its own track. Repeatable.
    ///
    /// Takes a process name such as wfica32.exe, or a numeric process id. The
    /// whole process tree is captured, so helper processes are included.
    #[arg(long, value_name = "PROCESS", action = clap::ArgAction::Append)]
    pub app: Vec<String>,

    /// Record everything except one application's audio. Repeatable.
    #[arg(long = "not-app", value_name = "PROCESS", action = clap::ArgAction::Append)]
    pub not_app: Vec<String>,

    /// Record a microphone or other input endpoint. Repeatable.
    #[arg(long, num_args = 0..=1, default_missing_value = "default", value_name = "DEVICE", action = clap::ArgAction::Append)]
    pub mic: Vec<String>,

    /// Stop after this many seconds instead of waiting for Ctrl-C.
    #[arg(short, long, value_name = "SECONDS")]
    pub duration: Option<f64>,

    /// Capture buffer size in milliseconds. Larger is more tolerant of a busy
    /// machine, at the cost of a longer flush at the end.
    #[arg(long, default_value_t = 200, value_name = "MS")]
    pub buffer_ms: u32,

    /// Sample rate for per-application tracks, which is the one case where the
    /// recorder chooses rather than the endpoint.
    #[arg(long, default_value_t = 48_000, value_name = "HZ")]
    pub rate: u32,

    /// Channel count for per-application tracks.
    #[arg(long, default_value_t = 2)]
    pub channels: u16,

    /// Print the session manifest as JSON when recording finishes.
    #[arg(long)]
    pub json: bool,

    /// Do not print the live status line.
    #[arg(short, long)]
    pub quiet: bool,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let _apartment = ssr_core::com::Apartment::new();
    // Transcription spawns a speech worker. Ctrl-C, or a terminal window being
    // closed, must not leave it behind holding a model in memory.
    let _kill_children_with_me = ssr_core::KillOnExit::install();

    match cli.command {
        Command::Devices { json } => devices(json),
        Command::Apps { all, json } => apps(all, json),
        Command::Discover {
            watch,
            sessions,
            interval,
            json,
        } => {
            if watch {
                discover::watch(interval, sessions)
            } else {
                discover::run(json, sessions)
            }
        }
        Command::Rec(args) => record::run(args),
        Command::Listen(args) => listen::run(args),
        Command::Transcribe(args) => transcribe::run(args),
    }
}

fn devices(json: bool) -> Result<()> {
    // The detailed listing resolves each endpoint's mix format, which is what
    // the recording will actually contain: the sample rate, the sample type and
    // the channel layout are the endpoint's to choose, not the recorder's.
    let devices = ssr_core::Devices::new()?.list_detailed();
    if json {
        println!("{}", serde_json::to_string_pretty(&devices)?);
        return Ok(());
    }
    let discovery = ssr_core::sessions::Discovery {
        devices,
        ..Default::default()
    };
    discover::print_endpoints(&discovery);
    Ok(())
}

fn apps(all: bool, json: bool) -> Result<()> {
    if all {
        let mut procs = ssr_core::list_processes()?;
        if json {
            println!("{}", serde_json::to_string_pretty(&procs)?);
            return Ok(());
        }
        procs.sort_by_key(|p| p.name.to_lowercase());
        for p in procs {
            println!("  {:>7}  {}", p.pid, p.name);
        }
        return Ok(());
    }

    // Discovery rather than name matching: the list is what the audio engine
    // says is there, so an application nobody catalogued still turns up.
    let discovery = ssr_core::discover_once()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&discovery.sources)?);
        return Ok(());
    }
    discover::print_sources(&discovery);
    println!();
    println!("Any other process works too: pass its executable name or pid to --app.");
    println!("Run with --all to list every process, or `ssr discover --watch` for live levels.");
    Ok(())
}
