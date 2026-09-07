//! The recording session controller.
//!
//! A session owns a worker thread. The worker holds the tracks and does all the
//! writing; everyone else observes it through [`Recorder::snapshot`]. Keeping
//! the writing on one thread means neither the command line nor the desktop
//! application can stall a capture thread by being slow to draw.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use crossbeam_channel::{bounded, RecvTimeoutError};

use ssr_core::{Packet, SourceHandle};
use ssr_timeline::manifest::{now_rfc3339, Marker};
use ssr_timeline::{SessionManifest, TimedBlock, Track, TrackManifest, WavWriter};

use crate::config::SessionConfig;
use crate::status::{to_db, RecorderState, SessionSnapshot, TrackSnapshot};

/// How often the worker tops every track up with silence and republishes status.
const TICK: Duration = Duration::from_millis(50);
/// How long the drain phase tolerates silence before deciding the capture
/// threads have handed over everything they had.
const DRAIN_QUIET: Duration = Duration::from_millis(400);
/// Metering window. Peaks are held for this long so short sounds stay visible.
const METER_DECAY: f32 = 0.75;

/// Status shared between the worker thread and observers.
struct Shared {
    state: RecorderState,
    tracks: Vec<TrackSnapshot>,
    warnings: Vec<String>,
    output_dir: String,
    started: Instant,
    elapsed_secs: f64,
    /// Cue points dropped while recording. Held here rather than on the worker
    /// so that marking a moment never waits on the worker's next tick: pressing
    /// the key at the moment something happens has to record that moment, not
    /// the moment the recorder got round to noticing.
    markers: Vec<Marker>,
}

/// A running or finished recording session.
pub struct Recorder {
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<SessionManifest>>>,
    output_dir: PathBuf,
}

impl Recorder {
    /// Open every source and begin recording.
    ///
    /// Returns once the streams are running, so a device that cannot be opened
    /// is reported here rather than discovered later as an empty file.
    pub fn start(config: SessionConfig) -> Result<Recorder> {
        if config.sources.is_empty() {
            bail!("a session needs at least one source");
        }
        let names = config.track_names();
        let (out_dir, single_file) = config.output.resolve();
        if single_file.is_some() && names.len() > 1 {
            bail!(
                "the output is a single file but {} tracks were requested; \
                 choose a directory instead",
                names.len()
            );
        }
        std::fs::create_dir_all(&out_dir)
            .with_context(|| format!("creating {}", out_dir.display()))?;

        let shared = Arc::new(Mutex::new(Shared {
            state: RecorderState::Starting,
            tracks: Vec::new(),
            warnings: Vec::new(),
            output_dir: out_dir.display().to_string(),
            started: Instant::now(),
            elapsed_secs: 0.0,
            markers: Vec::new(),
        }));
        let stop = Arc::new(AtomicBool::new(false));

        let (ready_tx, ready_rx) = bounded::<Result<()>>(1);
        let worker_shared = shared.clone();
        let worker_stop = stop.clone();
        let worker_dir = out_dir.clone();

        let worker = std::thread::Builder::new()
            .name("ssr-session".to_string())
            .spawn(move || {
                run_session(
                    config,
                    names,
                    worker_dir,
                    single_file,
                    worker_shared,
                    worker_stop,
                    ready_tx,
                )
            })
            .context("spawning the session worker")?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Recorder {
                shared,
                stop,
                worker: Some(worker),
                output_dir: out_dir,
            }),
            Ok(Err(e)) => {
                let _ = worker.join();
                Err(e)
            }
            Err(_) => {
                let _ = worker.join();
                Err(anyhow!("the session worker exited before it started"))
            }
        }
    }

    /// Current state of the session, cheap enough to poll at frame rate.
    pub fn snapshot(&self) -> SessionSnapshot {
        let s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        SessionSnapshot {
            state: s.state,
            elapsed_secs: s.elapsed_secs,
            output_dir: s.output_dir.clone(),
            tracks: s.tracks.clone(),
            warnings: s.warnings.clone(),
            markers: s.markers.clone(),
        }
    }

    /// Mark this instant, to be written into the manifest.
    ///
    /// Returns the offset that was recorded, so the caller can show it.
    /// Marking a session that is not recording is a no-op returning `None`,
    /// rather than an error: the shortcut is a reflex and pressing it a moment
    /// too late should do nothing, not raise something.
    pub fn add_marker(&self, label: &str) -> Option<Marker> {
        let mut s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        if !matches!(s.state, RecorderState::Recording | RecorderState::Starting) {
            return None;
        }
        let marker = Marker {
            at_secs: s.started.elapsed().as_secs_f64(),
            label: label.trim().to_string(),
        };
        s.markers.push(marker.clone());
        Some(marker)
    }

    /// Every marker dropped so far.
    pub fn markers(&self) -> Vec<Marker> {
        self.shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .markers
            .clone()
    }

    pub fn is_running(&self) -> bool {
        matches!(
            self.snapshot().state,
            RecorderState::Starting | RecorderState::Recording
        )
    }

    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    /// Ask the worker to finish, without waiting.
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    /// Stop and collect the manifest.
    pub fn stop(&mut self) -> Result<SessionManifest> {
        self.request_stop();
        match self.worker.take() {
            Some(w) => w
                .join()
                .map_err(|_| anyhow!("the session worker panicked"))?,
            None => bail!("this session has already been stopped"),
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.request_stop();
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

/// Per-track state the worker owns.
struct LiveTrack {
    track: Track<WavWriter>,
    file: String,
    description: String,
    pid: Option<u32>,
    sample_rate: u32,
    channels: u16,
    peak: f32,
    /// Sum of squares and sample count since the last status publish.
    sum_squares: f64,
    sample_count: u64,
    rms_db: f32,
}

#[allow(clippy::too_many_arguments)]
fn run_session(
    config: SessionConfig,
    names: Vec<String>,
    out_dir: PathBuf,
    single_file: Option<String>,
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    ready_tx: crossbeam_channel::Sender<Result<()>>,
) -> Result<SessionManifest> {
    let _apartment = ssr_core::com::Apartment::new();

    // The shared origin for every track, taken before any stream opens so a
    // slow source gets leading silence rather than being shifted earlier.
    let session_t0 = ssr_core::qpc_now_100ns();
    let started_utc = now_rfc3339();
    let started_at = Instant::now();

    let (tx, rx) = bounded::<(String, Packet)>(2048);
    let mut handles: Vec<SourceHandle> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for (request, name) in config.sources.iter().zip(names.iter()) {
        let spec = request.to_spec(config.sample_rate, config.channels);
        match ssr_core::start(name.clone(), spec, config.buffer_ms, tx.clone()) {
            Ok(h) => handles.push(h),
            Err(e) => {
                // One unavailable source should not discard the rest of the
                // session. Citrix audio redirection being switched off by policy
                // is the expected case here.
                warnings.push(format!("source {name:?} could not start: {e}"));
            }
        }
    }
    // The worker must not keep a sender, or the channel would never report that
    // every capture thread has finished.
    drop(tx);

    if handles.is_empty() {
        let detail = if warnings.is_empty() {
            "no source could be started".to_string()
        } else {
            warnings.join("; ")
        };
        set_state(&shared, RecorderState::Failed);
        push_warnings(&shared, &warnings);
        let _ = ready_tx.send(Err(anyhow!(detail.clone())));
        bail!(detail);
    }

    let mut tracks: Vec<LiveTrack> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();

    for h in &handles {
        let file = single_file
            .clone()
            .unwrap_or_else(|| format!("{}.wav", h.name));
        let wav = match WavWriter::create(
            out_dir.join(&file),
            h.format.sample_rate,
            h.format.channels,
            h.format.channel_mask,
        ) {
            Ok(w) => w,
            Err(e) => {
                set_state(&shared, RecorderState::Failed);
                let _ = ready_tx.send(Err(e));
                bail!("could not create an output file");
            }
        };
        index.insert(h.name.clone(), tracks.len());
        // Silence must stop filling further behind the present than a buffer
        // takes to arrive, or the first sound after a quiet stretch is trimmed
        // and appears to have started early. Twice the buffer plus a margin is
        // a safe starting point; the track raises it further if it turns out
        // this stream delivers later than that.
        let lookbehind = (config.buffer_ms as i64 * 2 + 200) * 10_000;
        tracks.push(LiveTrack {
            track: Track::new(
                h.name.clone(),
                wav,
                h.format.sample_rate,
                h.format.channels as usize,
                session_t0,
            )
            .with_lookbehind(lookbehind),
            file,
            description: h.description.clone(),
            pid: h.pid,
            sample_rate: h.format.sample_rate,
            channels: h.format.channels,
            peak: 0.0,
            sum_squares: 0.0,
            sample_count: 0,
            rms_db: -120.0,
        });
    }

    {
        let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
        s.state = RecorderState::Recording;
        s.started = Instant::now();
        s.warnings.extend(warnings.iter().cloned());
    }
    publish(&shared, &mut tracks, started_at);
    let _ = ready_tx.send(Ok(()));

    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        if let Some(limit) = config.duration_secs {
            if started_at.elapsed().as_secs_f64() >= limit {
                break;
            }
        }

        match rx.recv_timeout(TICK) {
            Ok((name, packet)) => {
                if let Some(&i) = index.get(&name) {
                    write_packet(&mut tracks[i], packet)?;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        // Every track is topped up on a timer, not only when its own stream goes
        // quiet, because an idle loopback stream sends nothing at all.
        let now = ssr_core::qpc_now_100ns();
        for t in &mut tracks {
            t.track.fill_idle(now)?;
        }
        publish(&shared, &mut tracks, started_at);
    }

    set_state(&shared, RecorderState::Stopping);

    // Ask the capture threads to finish, then keep draining: a thread blocked
    // handing over its last packet cannot exit until someone receives it.
    for h in &handles {
        h.signal_stop();
    }
    let mut quiet_since = Instant::now();
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok((name, packet)) => {
                if let Some(&i) = index.get(&name) {
                    write_packet(&mut tracks[i], packet)?;
                }
                quiet_since = Instant::now();
            }
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {
                if quiet_since.elapsed() >= DRAIN_QUIET {
                    break;
                }
            }
        }
    }

    for h in &mut handles {
        if let Err(e) = h.join() {
            warnings.push(format!("source {:?} ended with an error: {e}", h.name));
        }
    }

    // Pad every track to the longest so the files are interchangeable in an
    // editor and none of them looks truncated.
    let longest = tracks
        .iter()
        .map(|t| t.track.duration_secs())
        .fold(0.0f64, f64::max);
    let end_qpc = session_t0 + (longest * 10_000_000.0) as i64;
    for t in &mut tracks {
        t.track.finalize_to(end_qpc)?;
    }

    let mut track_manifests = Vec::new();
    for t in tracks.drain(..) {
        let stats = t.track.stats();
        let duration_secs = t.track.duration_secs();
        let name = t.track.name().to_string();
        let wav = t.track.into_sink();
        let size_overflow = wav.overflowed();
        if size_overflow {
            warnings.push(format!(
                "track {name:?} passed 4 GB; its WAV header cannot describe the full length"
            ));
        }
        wav.finish()?;
        track_manifests.push(TrackManifest {
            name,
            file: t.file,
            source: t.description,
            pid: t.pid,
            sample_rate: t.sample_rate,
            channels: t.channels,
            duration_secs,
            stats,
            size_overflow,
        });
    }

    let manifest = SessionManifest {
        tool: "system-sound-recorder".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        started_utc,
        ended_utc: now_rfc3339(),
        duration_secs: longest,
        session_t0_qpc_100ns: session_t0,
        tracks: track_manifests,
        warnings: warnings.clone(),
        markers: shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .markers
            .clone(),
    };

    let manifest_path = out_dir.join("session.json");
    std::fs::write(&manifest_path, manifest.to_json())
        .with_context(|| format!("writing {}", manifest_path.display()))?;

    {
        let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
        s.state = RecorderState::Finished;
        s.elapsed_secs = longest;
        s.warnings = warnings;
        for (snapshot, m) in s.tracks.iter_mut().zip(manifest.tracks.iter()) {
            snapshot.duration_secs = m.duration_secs;
            snapshot.peak = 0.0;
            snapshot.peak_db = -120.0;
            snapshot.rms_db = -120.0;
        }
    }

    Ok(manifest)
}

/// Place one packet on its track and fold it into the meters.
fn write_packet(t: &mut LiveTrack, packet: Packet) -> Result<()> {
    let mut peak = 0.0f32;
    let mut sum = 0.0f64;
    for s in &packet.samples {
        let a = s.abs();
        if a > peak {
            peak = a;
        }
        sum += (*s as f64) * (*s as f64);
    }
    if peak > t.peak {
        t.peak = peak;
    }
    t.sum_squares += sum;
    t.sample_count += packet.samples.len() as u64;

    let block = TimedBlock {
        qpc_100ns: packet.qpc_100ns,
        samples: packet.samples,
        channels: packet.channels,
        discontinuity: packet.flags.discontinuity,
        timestamp_error: packet.flags.timestamp_error,
    };
    t.track.push(&block)
}

fn set_state(shared: &Arc<Mutex<Shared>>, state: RecorderState) {
    let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
    s.state = state;
}

fn push_warnings(shared: &Arc<Mutex<Shared>>, warnings: &[String]) {
    let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
    s.warnings.extend(warnings.iter().cloned());
}

/// Copy the worker's view of every track into the shared snapshot.
fn publish(shared: &Arc<Mutex<Shared>>, tracks: &mut [LiveTrack], started: Instant) {
    let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
    s.elapsed_secs = started.elapsed().as_secs_f64();
    s.tracks.clear();
    for t in tracks.iter_mut() {
        if t.sample_count > 0 {
            let rms = (t.sum_squares / t.sample_count as f64).sqrt() as f32;
            t.rms_db = to_db(rms);
            t.sum_squares = 0.0;
            t.sample_count = 0;
        }
        let stats = t.track.stats();
        s.tracks.push(TrackSnapshot {
            name: t.track.name().to_string(),
            description: t.description.clone(),
            file: t.file.clone(),
            sample_rate: t.sample_rate,
            channels: t.channels,
            pid: t.pid,
            duration_secs: t.track.duration_secs(),
            peak: t.peak,
            peak_db: to_db(t.peak),
            rms_db: t.rms_db,
            frames_captured: stats.frames_captured,
            frames_silence: stats.frames_silence_filled,
            discontinuities: stats.discontinuities,
        });
        // Decay rather than reset, so a brief sound stays on the meter long
        // enough to be seen.
        t.peak *= METER_DECAY;
    }
}
