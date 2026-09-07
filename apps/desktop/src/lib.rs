//! The desktop shell.
//!
//! A thin layer: it owns at most one recording session and exposes the capture
//! engine to the interface. Anything that could be a decision rather than
//! plumbing belongs in `ssr-session`, so the command line tool and this
//! application cannot drift apart.

mod transcribe;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use ssr_core::com::Apartment;
use ssr_core::{Discovery, SessionMonitor};
use ssr_session::library::LibrarySession;
use ssr_session::{Recorder, SessionConfig, SessionManifest, SessionSnapshot};
use ssr_timeline::manifest::Marker;
use tauri::{Manager, State, WebviewUrl, WebviewWindowBuilder};

/// A running application whose audio can be recorded on its own track.
#[derive(Debug, Clone, Serialize)]
pub struct KnownApp {
    /// Friendly name, such as "Citrix HDX session".
    pub label: String,
    /// Executable that carries the audio, such as `wfica32.exe`.
    pub exe: String,
    /// Process ids currently running under that name.
    pub pids: Vec<u32>,
}

/// How often discovery refreshes when nothing is watching it closely.
///
/// The interface asks for a faster rate while a live meter is on screen and
/// drops back to this when it is not, so an idle window costs almost nothing.
const IDLE_DISCOVERY_MS: u64 = 1_000;

/// At most one session at a time. Recording two overlapping sessions would give
/// two sets of files with different origins, which is never what anyone means.
struct AppState {
    recorder: Mutex<Option<Recorder>>,
    /// Autodiscovery runs for the life of the application rather than being
    /// started and stopped, because its whole value is answering "what is
    /// playing?" the instant someone asks, and because its first pass is the
    /// expensive one. Its thread owns the COM apartment that all of its
    /// interfaces belong to.
    discovery: SessionMonitor,
    /// At most one live transcription, for the same reason as recording: two at
    /// once would produce two transcripts of one conversation.
    live: Mutex<Option<transcribe::LiveSession>>,
    /// At most one batch transcription, so a queue of them cannot saturate a
    /// machine that is also recording.
    batch: Mutex<Option<transcribe::BatchJob>>,
    /// At most one model download. They are hundreds of megabytes each and
    /// two at once would only make both slower.
    download: Mutex<Option<transcribe::DownloadJob>>,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            recorder: Mutex::new(None),
            discovery: SessionMonitor::start(IDLE_DISCOVERY_MS),
            live: Mutex::new(None),
            batch: Mutex::new(None),
            download: Mutex::new(None),
        }
    }
}

/// Commands return a plain string on failure, which is what the interface shows.
type CmdResult<T> = Result<T, String>;

fn fail(context: &str, e: impl std::fmt::Display) -> String {
    format!("{context}: {e}")
}

#[tauri::command]
fn list_devices() -> CmdResult<Vec<ssr_core::DeviceInfo>> {
    let _apartment = Apartment::new();
    // Detailed: each endpoint's channel layout comes with it, because the
    // interface shows what a recording of that endpoint would contain, and a
    // channel count without a layout does not say that.
    ssr_core::Devices::new()
        .map(|d| d.list_detailed())
        .map_err(|e| fail("could not read the audio endpoints", e))
}

#[tauri::command]
fn list_processes() -> CmdResult<Vec<ssr_core::ProcessInfo>> {
    ssr_core::list_processes().map_err(|e| fail("could not list processes", e))
}

/// The recognised audio applications that are actually running.
#[tauri::command]
fn list_known_apps() -> CmdResult<Vec<KnownApp>> {
    let procs = ssr_core::list_processes().map_err(|e| fail("could not list processes", e))?;
    let mut out = Vec::new();
    for (label, exe) in ssr_core::known_targets() {
        let pids: Vec<u32> = procs
            .iter()
            .filter(|p| p.name.eq_ignore_ascii_case(exe))
            .map(|p| p.pid)
            .collect();
        if pids.is_empty() {
            continue;
        }
        out.push(KnownApp {
            label: label.to_string(),
            exe: exe.to_string(),
            pids,
        });
    }
    Ok(out)
}

#[tauri::command]
fn start_recording(
    state: State<'_, AppState>,
    config: SessionConfig,
) -> CmdResult<SessionSnapshot> {
    let mut slot = state
        .recorder
        .lock()
        .map_err(|_| "recorder state is poisoned".to_string())?;
    if slot.as_ref().is_some_and(|r| r.is_running()) {
        return Err("a session is already recording".to_string());
    }
    let recorder = Recorder::start(config).map_err(|e| fail("could not start recording", e))?;
    let snapshot = recorder.snapshot();
    *slot = Some(recorder);
    Ok(snapshot)
}

#[tauri::command]
fn stop_recording(state: State<'_, AppState>) -> CmdResult<SessionManifest> {
    let mut slot = state
        .recorder
        .lock()
        .map_err(|_| "recorder state is poisoned".to_string())?;
    match slot.as_mut() {
        Some(recorder) => recorder
            .stop()
            .map_err(|e| fail("could not stop cleanly", e)),
        None => Err("no session is running".to_string()),
    }
}

#[tauri::command]
fn snapshot(state: State<'_, AppState>) -> CmdResult<SessionSnapshot> {
    let slot = state
        .recorder
        .lock()
        .map_err(|_| "recorder state is poisoned".to_string())?;
    Ok(slot
        .as_ref()
        .map(|r| r.snapshot())
        .unwrap_or_else(SessionSnapshot::idle))
}

/// Where recordings go unless the user says otherwise.
///
/// Documents rather than the home directory itself. Recordings are ordinary
/// user files and belong somewhere the user already treats as theirs, not
/// scattered at the root of a profile alongside application state.
#[tauri::command]
fn default_output_dir(app: tauri::AppHandle) -> String {
    app.path()
        .document_dir()
        .or_else(|_| app.path().audio_dir())
        .map(|d| d.join("System Sound Recorder"))
        .unwrap_or_else(|_| PathBuf::from("recordings"))
        .display()
        .to_string()
}

/// Show a folder or file in the file manager.
#[tauri::command]
fn reveal_path(app: tauri::AppHandle, path: String) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| fail("could not open that location", e))
}

/// Tear a panel off into a window of its own, for a second display.
#[tauri::command]
fn open_panel_window(app: tauri::AppHandle, panel_id: String, title: String) -> CmdResult<()> {
    // The label has to be stable so asking twice focuses the existing window
    // rather than stacking duplicates on top of each other.
    let label = format!("panel-{panel_id}");
    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.set_focus();
        return Ok(());
    }

    let url = WebviewUrl::App(format!("index.html?panel={panel_id}").into());
    WebviewWindowBuilder::new(&app, &label, url)
        .title(title)
        .inner_size(560.0, 800.0)
        .min_inner_size(320.0, 320.0)
        .decorations(false)
        .resizable(true)
        .build()
        .map_err(|e| fail("could not open the panel window", e))?;
    Ok(())
}

#[tauri::command]
fn set_always_on_top(window: tauri::Window, enabled: bool) -> CmdResult<()> {
    window
        .set_always_on_top(enabled)
        .map_err(|e| fail("could not change the window level", e))
}

// ---------------------------------------------------------------------------
// Autodiscovery
// ---------------------------------------------------------------------------

/// The latest discovery snapshot: endpoints with their channel layouts, every
/// audio session on the machine, and one recordable row per application.
///
/// A memory copy, not an enumeration. The monitor thread does the work on its
/// own schedule, so this can be polled at whatever rate the interface is
/// animating at.
#[tauri::command]
fn discover(state: State<'_, AppState>) -> Discovery {
    state.discovery.latest()
}

/// Only the levels of the sessions already reported.
///
/// The meters want ten of these a second; the descriptions they belong to do
/// not change and are not worth resending with them. About a twelfth the size
/// of a full snapshot.
#[tauri::command]
fn discover_levels(state: State<'_, AppState>) -> Vec<ssr_core::SessionLevel> {
    state.discovery.latest().levels()
}

/// Ask discovery to refresh faster or slower.
///
/// Called by the interface as panels come and go: a visible level meter wants
/// ten passes a second, and a window with the discovery panel closed wants one.
#[tauri::command]
fn set_discovery_rate(state: State<'_, AppState>, interval_ms: u64) {
    state.discovery.set_interval_ms(interval_ms);
}

/// Rebuild the discovery picture now, rather than at the next scheduled rescan.
///
/// For the moment someone has just started something playing and pressed
/// Rescan, and does not want to wait even a second to see it.
#[tauri::command]
fn rescan(state: State<'_, AppState>) -> Discovery {
    state.discovery.refresh_now();
    state.discovery.latest()
}

// ---------------------------------------------------------------------------
// Session library, markers and capacity
// ---------------------------------------------------------------------------

/// Read past sessions out of a library folder, newest first.
#[tauri::command]
fn list_sessions(root: String, limit: Option<usize>) -> Vec<LibrarySession> {
    // A cap, because a library is allowed to be large and the interface only
    // ever shows a page of it. The manifests of older sessions are not parsed
    // at all until they are asked for.
    ssr_session::list_sessions(Path::new(&root), limit.unwrap_or(60))
}

/// Read one session directory, for when the interface has been given a path
/// directly rather than finding it in the library.
#[tauri::command]
fn read_session(dir: String) -> CmdResult<LibrarySession> {
    ssr_session::library::read_session(Path::new(&dir))
        .ok_or_else(|| format!("{dir} is not a session directory"))
}

/// Mark this instant in the running session.
///
/// Returns the offset that was recorded, or nothing when no session is running.
#[tauri::command]
fn add_marker(state: State<'_, AppState>, label: String) -> CmdResult<Option<Marker>> {
    let slot = state
        .recorder
        .lock()
        .map_err(|_| "recorder state is poisoned".to_string())?;
    Ok(slot.as_ref().and_then(|r| r.add_marker(&label)))
}

/// Free space on the volume a path lives on, and what that is worth in seconds.
#[tauri::command]
fn disk_space(path: String) -> CmdResult<ssr_core::DiskSpace> {
    ssr_core::space_for(Path::new(&path))
        .ok_or_else(|| format!("could not read the free space on the volume holding {path}"))
}

/// Undo the webview's double application of the display scale.
///
/// On a display scaled above 100% the webview and its window disagree, and the
/// disagreement is not subtle: the interface is drawn a quarter larger than the
/// frame holding it, so the rightmost column and the status bar are simply
/// outside the window. It is not a layout fault. Measured from inside the page,
/// nothing overflows: the document is exactly the size of the viewport, and the
/// viewport is exactly the window's physical client area. The webview then
/// rasterises that at the display scale, and the result no longer fits.
///
/// Zooming out by the same factor cancels it exactly. The interface then
/// occupies the whole window at the size it was designed for.
///
/// Windows is asked for the scale directly rather than the window being asked,
/// because the window reports 1.0 here, which is the root of the problem.
#[cfg(windows)]
fn apply_scale_correction(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::HiDpi::GetDpiForWindow;

    let Ok(handle) = window.hwnd() else {
        return;
    };
    let dpi = unsafe { GetDpiForWindow(HWND(handle.0 as _)) };
    if dpi == 0 {
        return;
    }
    let scale = dpi as f64 / 96.0;
    if (scale - 1.0).abs() < 0.01 {
        return;
    }
    let _ = window.set_zoom(1.0 / scale);
}

#[cfg(not(windows))]
fn apply_scale_correction(_window: &tauri::WebviewWindow) {}

// --- transcription -----------------------------------------------------------

/// What speech engines are installed, and what to do if none are.
#[tauri::command]
fn transcription_engines() -> transcribe::EngineReport {
    transcribe::report_engines()
}

#[tauri::command]
fn start_listening(
    state: State<'_, AppState>,
    config: transcribe::ListenConfig,
) -> CmdResult<transcribe::ListenStatus> {
    let mut slot = state
        .live
        .lock()
        .map_err(|_| "listening state is poisoned".to_string())?;
    if slot.as_ref().is_some_and(|l| l.is_running()) {
        return Err("already listening to a channel".to_string());
    }
    let session =
        transcribe::LiveSession::start(config).map_err(|e| fail("could not start listening", e))?;
    let status = session.status();
    *slot = Some(session);
    Ok(status)
}

/// Status plus whatever lines the caller has not seen yet.
#[tauri::command]
fn listening_poll(state: State<'_, AppState>, after: usize) -> CmdResult<serde_json::Value> {
    let slot = state
        .live
        .lock()
        .map_err(|_| "listening state is poisoned".to_string())?;
    let (status, lines) = match slot.as_ref() {
        Some(session) => (session.status(), session.lines_after(after)),
        None => (transcribe::ListenStatus::idle(), Vec::new()),
    };
    Ok(serde_json::json!({ "status": status, "lines": lines }))
}

#[tauri::command]
fn stop_listening(state: State<'_, AppState>) -> CmdResult<transcribe::ListenStatus> {
    let mut slot = state
        .live
        .lock()
        .map_err(|_| "listening state is poisoned".to_string())?;
    match slot.as_mut() {
        Some(session) => Ok(session.stop()),
        None => Err("nothing is being listened to".to_string()),
    }
}

#[tauri::command]
fn start_transcription(
    state: State<'_, AppState>,
    request: ssr_transcribe::job::TranscribeRequest,
    engine: Option<String>,
) -> CmdResult<transcribe::BatchStatus> {
    let mut slot = state
        .batch
        .lock()
        .map_err(|_| "transcription state is poisoned".to_string())?;
    if slot.as_ref().is_some_and(|j| j.is_running()) {
        return Err("a transcription is already running".to_string());
    }
    let job = transcribe::BatchJob::start(request, engine)
        .map_err(|e| fail("could not start transcribing", e))?;
    let status = job.status();
    *slot = Some(job);
    Ok(status)
}

#[tauri::command]
fn transcription_status(state: State<'_, AppState>) -> CmdResult<transcribe::BatchStatus> {
    let slot = state
        .batch
        .lock()
        .map_err(|_| "transcription state is poisoned".to_string())?;
    Ok(slot
        .as_ref()
        .map(|j| j.status())
        .unwrap_or_else(transcribe::BatchStatus::idle))
}

/// Write a live conversation to files, as the save dialog asks.
#[tauri::command]
fn save_conversation(request: transcribe::SaveRequest) -> CmdResult<transcribe::SaveResult> {
    transcribe::save_conversation(request).map_err(|e| fail("could not save", e))
}

/// Start fetching a speech model, reporting progress as it goes.
#[tauri::command]
fn download_speech_model(
    state: State<'_, AppState>,
    model: String,
    engine: Option<String>,
) -> CmdResult<transcribe::ModelDownload> {
    let mut slot = state
        .download
        .lock()
        .map_err(|_| "download state is poisoned".to_string())?;
    if slot.as_ref().is_some_and(|j| j.is_running()) {
        return Err("a model is already being downloaded".to_string());
    }
    let job = transcribe::DownloadJob::start(model, engine)
        .map_err(|e| fail("could not start the download", e))?;
    let status = job.status();
    *slot = Some(job);
    Ok(status)
}

#[tauri::command]
fn model_download_status(state: State<'_, AppState>) -> CmdResult<transcribe::ModelDownload> {
    let slot = state
        .download
        .lock()
        .map_err(|_| "download state is poisoned".to_string())?;
    Ok(slot.as_ref().map(|j| j.status()).unwrap_or_default())
}

/// Track names in a recorded session, for choosing what to transcribe.
#[tauri::command]
fn session_tracks(path: String) -> CmdResult<Vec<String>> {
    ssr_transcribe::job::session_tracks(std::path::Path::new(&path))
        .map_err(|e| fail("could not read that session", e))
}

/// Build and run the application.
pub fn run() {
    // Before anything can spawn a child. Transcription starts a speech worker
    // that holds a model in memory, and without this it survives a crash of
    // this process with nothing left to stop it.
    let _kill_children_with_me = ssr_core::KillOnExit::install();

    tauri::Builder::default()
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                apply_scale_correction(&window);
            }
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            list_devices,
            list_processes,
            list_known_apps,
            start_recording,
            stop_recording,
            snapshot,
            default_output_dir,
            reveal_path,
            open_panel_window,
            set_always_on_top,
            discover,
            discover_levels,
            set_discovery_rate,
            rescan,
            list_sessions,
            read_session,
            add_marker,
            disk_space,
            transcription_engines,
            start_listening,
            listening_poll,
            stop_listening,
            start_transcription,
            transcription_status,
            save_conversation,
            session_tracks,
            transcribe::list_speech_models,
            download_speech_model,
            model_download_status,
        ])
        .on_window_event(|window, event| {
            // Closing the main window ends the session rather than leaving a
            // capture thread writing to a file nobody is watching.
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if window.label() != "main" {
                    return;
                }
                // Everything the window started, not only the recording.
                //
                // Each of these owns a capture thread, and the two
                // transcription slots also own a speech worker holding a model
                // in memory. Stopping the recorder alone used to leave those
                // running behind a closed window, with a half-written
                // transcript and no way to reach either.
                if let Some(state) = window.try_state::<AppState>() {
                    if let Ok(mut slot) = state.recorder.lock() {
                        if let Some(recorder) = slot.as_mut() {
                            let _ = recorder.stop();
                        }
                    }
                    if let Ok(mut slot) = state.live.lock() {
                        if let Some(session) = slot.as_mut() {
                            session.stop();
                        }
                        // Dropped here rather than at process exit, so the
                        // speech worker is killed while there is still a
                        // runtime to do it.
                        *slot = None;
                    }
                    if let Ok(mut slot) = state.batch.lock() {
                        // A batch transcription has no cancellation protocol
                        // yet, so this only drops the handle. The job object
                        // installed at startup is what stops its engine
                        // processes when this one exits.
                        *slot = None;
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("the desktop shell failed to start");
}
