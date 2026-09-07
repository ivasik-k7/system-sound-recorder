//! Autodiscovery: what is actually making sound right now.
//!
//! The rest of the engine records what it is told to record. This module
//! answers the question that comes first — what is there to record? — by asking
//! the Windows audio engine directly rather than guessing from a list of
//! executable names.
//!
//! Every stream reaching an endpoint is an *audio session*, and
//! `IAudioSessionManager2` will enumerate them. Each one carries the process id
//! that owns it, whether it is currently active, and, through
//! `IAudioMeterInformation`, its live peak level — per channel, not just
//! overall. That is a complete picture of the machine's audio: a browser tab
//! playing video, a Citrix session carrying a remote meeting, and the Windows
//! notification sounds all appear as separate sessions on the same endpoint,
//! each one meterable before a single frame has been recorded.
//!
//! Two things follow from this that a name-matching approach cannot do. A
//! process nobody thought to catalogue is discovered anyway, because it has a
//! session. And a catalogued process that happens to be running but silent is
//! visibly silent, rather than being offered as though it were a live source.
//!
//! Sessions are enumerated on capture endpoints too, which is how the interface
//! can say which application currently holds the microphone.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use serde::Serialize;
use windows::core::Interface;
use windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
use windows::Win32::Media::Audio::{
    AudioSessionStateActive, AudioSessionStateExpired, IAudioSessionControl, IAudioSessionControl2,
    IAudioSessionManager2, ISimpleAudioVolume,
};
use windows::Win32::System::Com::CLSCTX_ALL;

use crate::catalog::{self, Category};
use crate::com::Apartment;
use crate::device::{describe_endpoint, DeviceInfo, DeviceKind, Devices};
use crate::error::Result;
use crate::procfind::{list_processes, ProcessInfo};

/// Where a session is in its lifecycle, as the audio engine sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionState {
    /// Rendering or capturing audio right now.
    Active,
    /// The stream exists but is not moving audio. A paused player, or an
    /// application that opened its output at launch and has been quiet since.
    Inactive,
    /// The owning process has gone. Windows keeps the entry around briefly.
    Expired,
}

/// One audio stream, as the Windows audio engine describes it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioSessionInfo {
    /// An identity that survives across refreshes, so the interface can keep a
    /// row in place and animate its meter rather than rebuilding the list.
    pub key: String,
    /// Owning process. Zero for the system sounds session, which belongs to the
    /// audio service rather than to any one application.
    pub pid: u32,
    /// Executable name, when the process could still be found.
    pub exe: String,
    /// The name to show: the catalogue's, the session's own, or the executable's.
    pub label: String,
    pub category: Category,
    /// Windows' own name for the session, when it set a usable one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Endpoint this stream is attached to.
    pub endpoint_id: String,
    pub endpoint_name: String,
    pub endpoint_kind: DeviceKind,
    pub state: SessionState,
    /// Loudest channel, as a linear amplitude in 0..1.
    pub peak: f32,
    /// Peak per channel, in the endpoint's channel order.
    pub channel_peaks: Vec<f32>,
    /// The application's own volume slider, 0..1.
    pub volume: f32,
    pub muted: bool,
    /// True for the Windows notification sounds session.
    pub is_system_sounds: bool,
    /// True when this process is in the catalogue.
    pub known: bool,
    /// True for the sources this tool exists to separate, such as Citrix.
    pub recommended: bool,
}

/// A candidate for recording, aggregated across every session a process owns.
///
/// A browser has one session per tab that makes noise, and Citrix can have
/// several. Recording is by process tree, so the interface offers one row per
/// executable and this is that row: the union of what the process is doing.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredSource {
    /// Executable name, which is also the selector `SourceRequest::App` takes.
    pub exe: String,
    pub label: String,
    pub category: Category,
    /// Every process id currently running under this name.
    pub pids: Vec<u32>,
    /// How many audio sessions these processes own.
    pub sessions: u32,
    /// How many of those are actively moving audio.
    pub active_sessions: u32,
    /// Loudest channel across all of them.
    pub peak: f32,
    /// True when any session is active. The difference between "running" and
    /// "worth recording".
    pub playing: bool,
    /// True when at least one session is on a capture endpoint: the process is
    /// listening as well as, or instead of, playing.
    pub capturing: bool,
    pub known: bool,
    pub recommended: bool,
    /// Endpoint names this process is playing to, for the subtitle.
    pub endpoints: Vec<String>,
}

/// The part of a session that actually changes.
///
/// A full discovery snapshot is mostly text that is fixed for the life of a
/// session: its key, the executable, the endpoint's name and GUID, the label,
/// the category. Sending all of it several times a second to move a meter is
/// most of the traffic and all of the garbage. This is what a meter needs.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLevel {
    pub key: String,
    pub state: SessionState,
    pub peak: f32,
    pub channel_peaks: Vec<f32>,
}

/// Everything discovery found in one pass.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Discovery {
    /// Endpoints, with their channel layouts resolved.
    pub devices: Vec<DeviceInfo>,
    /// Every audio session on every endpoint.
    pub sessions: Vec<AudioSessionInfo>,
    /// One row per process, ready to offer as a recordable source.
    pub sources: Vec<DiscoveredSource>,
    /// How long the pass took, in milliseconds. Shown in diagnostics, and the
    /// number to look at if the interface ever feels sluggish while polling.
    pub elapsed_ms: f64,
    /// Monotonic counter, incremented once per completed pass. Lets the
    /// interface tell a stale snapshot from a fresh one with no clock involved.
    pub generation: u64,
    /// Set when a pass failed. Discovery is best-effort: a machine with a
    /// misbehaving endpoint should still show every other endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Take ownership of a `PWSTR` COM handed back, freeing the allocation.
///
/// # Safety
/// `raw` must be a string allocated with `CoTaskMemAlloc`, which is what every
/// WASAPI getter returning `PWSTR` produces.
unsafe fn take_pwstr(raw: windows::core::PWSTR) -> String {
    if raw.is_null() {
        return String::new();
    }
    let s = raw.to_string().unwrap_or_default();
    windows::Win32::System::Com::CoTaskMemFree(Some(raw.0 as *const _));
    s
}

/// Windows' display name for a session, when it is worth showing.
///
/// Most applications never set one, and the ones Windows sets for its own
/// sessions are indirect resource references such as
/// `@%SystemRoot%\System32\AudioSrv.Dll,-202`. Showing that to a user would be
/// worse than showing nothing, so those are dropped.
fn usable_display_name(raw: String) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with('@') {
        return None;
    }
    Some(trimmed.to_string())
}

/// A session the discoverer is holding on to between passes.
///
/// The expensive facts about a session — which process owns it, what it is
/// called, how many channels it meters — do not change for as long as the
/// session exists. Only its state and its levels do. Keeping the COM objects
/// alive means a refresh reads the two things that change and nothing else,
/// which is the difference between a pass costing fourteen cross-process calls
/// per session and costing two.
struct LiveSession {
    control: IAudioSessionControl,
    /// Only needed on the pass that creates the entry, but kept because
    /// dropping it and re-deriving it later would cost more than holding it.
    #[allow(dead_code)]
    control2: IAudioSessionControl2,
    meter: Option<IAudioMeterInformation>,
    volume: Option<ISimpleAudioVolume>,
    /// Channels this session meters, read once.
    metering_channels: usize,
    /// The published row. State, peaks, volume and mute are refreshed in
    /// place; everything else was resolved when the session was first seen.
    info: AudioSessionInfo,
}

impl Discovery {
    /// Just the levels, for the high-rate path.
    pub fn levels(&self) -> Vec<SessionLevel> {
        self.sessions
            .iter()
            .map(|s| SessionLevel {
                key: s.key.clone(),
                state: s.state,
                peak: s.peak,
                channel_peaks: s.channel_peaks.clone(),
            })
            .collect()
    }
}

impl LiveSession {
    /// Read the parts that change. Two cross-process calls in the common case.
    ///
    /// Returns false when the session has gone, which is how a stopped stream
    /// leaves the list without waiting for the next full rescan.
    fn refresh(&mut self, with_volume: bool) -> bool {
        unsafe {
            self.info.state = match self.control.GetState() {
                Ok(s) if s == AudioSessionStateActive => SessionState::Active,
                Ok(s) if s == AudioSessionStateExpired => SessionState::Expired,
                Ok(_) => SessionState::Inactive,
                // The owning process has gone and the proxy is dead with it.
                Err(_) => return false,
            };
            if self.info.state == SessionState::Expired {
                return false;
            }

            if let Some(meter) = &self.meter {
                if self.metering_channels > 0 {
                    self.info.channel_peaks.resize(self.metering_channels, 0.0);
                    if meter
                        .GetChannelsPeakValues(&mut self.info.channel_peaks)
                        .is_err()
                    {
                        self.info.channel_peaks.clear();
                    }
                }
                // The overall peak is the loudest channel, which saves a call
                // to GetPeakValue for exactly the same answer. Only a session
                // that refuses to report per-channel values needs to be asked.
                self.info.peak = if self.info.channel_peaks.is_empty() {
                    meter.GetPeakValue().unwrap_or(0.0)
                } else {
                    self.info
                        .channel_peaks
                        .iter()
                        .copied()
                        .fold(0.0f32, f32::max)
                };
            }

            // The application's own volume slider moves rarely and is not worth
            // two calls per pass, so it rides along with the slow refresh.
            if with_volume {
                if let Some(v) = &self.volume {
                    self.info.volume = v.GetMasterVolume().unwrap_or(1.0);
                    self.info.muted = v.GetMute().map(|m| m.as_bool()).unwrap_or(false);
                }
            }
        }
        true
    }
}

/// Adopt one enumerated session, reading everything static about it.
///
/// `None` when the session cannot be described, which happens when the owning
/// process exits between being enumerated and being interrogated.
fn adopt(
    control: IAudioSessionControl,
    endpoint: &DeviceInfo,
    index: i32,
    procs: &[ProcessInfo],
) -> Option<LiveSession> {
    unsafe {
        let control2 = control.cast::<IAudioSessionControl2>().ok()?;

        let pid = control2.GetProcessId().unwrap_or(0);
        // S_OK means yes, S_FALSE means no. Anything else is a failure and is
        // treated as no.
        let is_system_sounds = control2.IsSystemSoundsSession().0 == 0;

        let exe = procs
            .iter()
            .find(|p| p.pid == pid)
            .map(|p| p.name.clone())
            .unwrap_or_default();

        let display_name = control
            .GetDisplayName()
            .ok()
            .map(|raw| take_pwstr(raw))
            .and_then(usable_display_name);

        // The identifier is per-application; the instance identifier is per
        // stream, which is what distinguishes two tabs of the same browser.
        let instance = control2
            .GetSessionInstanceIdentifier()
            .ok()
            .map(|raw| take_pwstr(raw))
            .unwrap_or_default();

        let meter = control.cast::<IAudioMeterInformation>().ok();
        let metering_channels = meter
            .as_ref()
            .and_then(|m| m.GetMeteringChannelCount().ok())
            .unwrap_or(0) as usize;
        let volume = control.cast::<ISimpleAudioVolume>().ok();

        let (label, category) = if is_system_sounds {
            ("Windows system sounds".to_string(), Category::System)
        } else if !exe.is_empty() {
            (catalog::label_for(&exe), catalog::category_for(&exe))
        } else if let Some(name) = display_name.clone() {
            (name, Category::Other)
        } else {
            (format!("Process {pid}"), Category::Other)
        };

        // Prefer the instance identifier: it is stable for the life of the
        // stream, which is what keeps a row from jumping about in the list and
        // what lets a later rescan recognise a session it has already adopted.
        let key = if instance.is_empty() {
            format!("{}#{pid}#{index}", endpoint.id)
        } else {
            instance
        };

        let info = AudioSessionInfo {
            key,
            pid,
            known: catalog::lookup(&exe).is_some(),
            recommended: catalog::is_recommended(&exe),
            exe,
            label,
            category,
            display_name,
            endpoint_id: endpoint.id.clone(),
            endpoint_name: endpoint.name.clone(),
            endpoint_kind: endpoint.kind,
            state: SessionState::Inactive,
            peak: 0.0,
            channel_peaks: Vec::new(),
            volume: 1.0,
            muted: false,
            is_system_sounds,
        };

        let mut live = LiveSession {
            control,
            control2,
            meter,
            volume,
            metering_channels,
            info,
        };
        // Fill in the state and levels immediately, so a session discovered
        // mid-pass is published with a real level rather than silence.
        if !live.refresh(true) {
            return None;
        }
        Some(live)
    }
}

/// Collapse sessions into one row per executable.
fn group_sources(sessions: &[AudioSessionInfo], procs: &[ProcessInfo]) -> Vec<DiscoveredSource> {
    let mut out: Vec<DiscoveredSource> = Vec::new();

    for session in sessions {
        // The system sounds session has no executable to record by name, so it
        // is not offered as a source of its own. It is still in `sessions`,
        // where it explains where a stray notification beep came from.
        if session.is_system_sounds || session.exe.is_empty() {
            continue;
        }
        let existing = out
            .iter_mut()
            .find(|s| s.exe.eq_ignore_ascii_case(&session.exe));
        let entry = match existing {
            Some(e) => e,
            None => {
                out.push(DiscoveredSource {
                    exe: session.exe.clone(),
                    label: catalog::label_for(&session.exe),
                    category: session.category,
                    pids: Vec::new(),
                    sessions: 0,
                    active_sessions: 0,
                    peak: 0.0,
                    playing: false,
                    capturing: false,
                    known: session.known,
                    recommended: session.recommended,
                    endpoints: Vec::new(),
                });
                out.last_mut().expect("just pushed")
            }
        };

        entry.sessions += 1;
        if session.state == SessionState::Active {
            entry.active_sessions += 1;
            match session.endpoint_kind {
                DeviceKind::Render => entry.playing = true,
                DeviceKind::Capture => entry.capturing = true,
            }
        }
        entry.peak = entry.peak.max(session.peak);
        if !entry.endpoints.contains(&session.endpoint_name) {
            entry.endpoints.push(session.endpoint_name.clone());
        }
    }

    // Fill in every process id running under each name, not only the ones that
    // happen to own a session. Recording is by process tree, so a browser whose
    // audio lives in one child process still has to show the whole family.
    for entry in &mut out {
        entry.pids = procs
            .iter()
            .filter(|p| p.name.eq_ignore_ascii_case(&entry.exe))
            .map(|p| p.pid)
            .collect();
        entry.pids.sort_unstable();
    }

    // Recommended first, then whatever is actually making sound, then loudest,
    // then alphabetically so the order does not jitter between refreshes.
    out.sort_by(|a, b| {
        b.recommended
            .cmp(&a.recommended)
            .then(b.playing.cmp(&a.playing))
            .then(
                b.peak
                    .partial_cmp(&a.peak)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
    });
    out
}

/// Add catalogued applications that are running but own no audio session.
///
/// A Citrix client that has not yet been asked to play anything has no session,
/// and it is still the thing the user wants armed and waiting. These rows are
/// marked as not playing, so the interface can show them as available rather
/// than as live.
fn add_silent_catalogued(sources: &mut Vec<DiscoveredSource>, procs: &[ProcessInfo]) {
    for entry in catalog::CATALOG {
        if entry.category == Category::System {
            continue;
        }
        if sources
            .iter()
            .any(|s| s.exe.eq_ignore_ascii_case(entry.exe))
        {
            continue;
        }
        let pids: Vec<u32> = procs
            .iter()
            .filter(|p| p.name.eq_ignore_ascii_case(entry.exe))
            .map(|p| p.pid)
            .collect();
        if pids.is_empty() {
            continue;
        }
        sources.push(DiscoveredSource {
            exe: entry.exe.to_string(),
            label: entry.label.to_string(),
            category: entry.category,
            pids,
            sessions: 0,
            active_sessions: 0,
            peak: 0.0,
            playing: false,
            capturing: false,
            known: true,
            recommended: entry.recommended,
            endpoints: Vec::new(),
        });
    }
}

/// How long a discovered set of sessions is trusted before it is rebuilt.
///
/// Between rescans a pass only refreshes what it already knows about, which is
/// what makes it cheap. The cost of that is latency in noticing a stream that
/// has just started, so this is the delay before something newly playing shows
/// up — short enough not to be noticed, long enough that the expensive work
/// happens once a second rather than ten times a second.
const RESCAN_AFTER: Duration = Duration::from_millis(900);

/// How long endpoint names and channel layouts are trusted.
///
/// Longer than a session rescan because these only change when someone opens
/// the Windows sound control panel, and reading them means activating an audio
/// client per endpoint.
const REDESCRIBE_AFTER: Duration = Duration::from_secs(10);

/// Reusable discovery state.
///
/// A pass over the machine's audio is mostly the same work as the last one: the
/// same endpoints with the same names and channel layouts, and the same
/// sessions owned by the same processes. Only the levels really change.
///
/// So this holds on to everything that does not. The endpoints, their session
/// managers, and the session objects themselves are kept alive between passes,
/// and a pass reads a state and a set of channel peaks per session — two
/// cross-process calls instead of fourteen. Every so often it rescans, which is
/// how a newly started stream is noticed.
///
/// It owns COM interfaces, so it belongs to one thread for its whole life,
/// which is what [`SessionMonitor`] provides. Callers who want a single answer
/// and no thread want [`discover_once`].
pub struct Discoverer {
    devices: Devices,
    endpoints: Vec<EndpointCache>,
    /// The process table. Only consulted when adopting a new session, so it is
    /// refreshed on the same cycle as a rescan.
    procs: Vec<ProcessInfo>,
    last_rescan: Option<std::time::Instant>,
    last_describe: Option<std::time::Instant>,
    generation: u64,
    /// Set when the last rescan could not read an endpoint. Discovery is
    /// best-effort: one bad endpoint should not hide every good one.
    error: Option<String>,
}

struct EndpointCache {
    id: String,
    info: DeviceInfo,
    /// Activated once. This is the single most expensive call in a pass, and
    /// the manager stays valid for as long as the endpoint does.
    manager: Option<IAudioSessionManager2>,
    sessions: Vec<LiveSession>,
}

impl Discoverer {
    /// Requires a COM apartment on the calling thread; see [`Apartment`].
    pub fn new() -> Result<Self> {
        Ok(Discoverer {
            devices: Devices::new()?,
            endpoints: Vec::new(),
            procs: Vec::new(),
            last_rescan: None,
            last_describe: None,
            generation: 0,
            error: None,
        })
    }

    /// Force the next pass to rebuild everything.
    ///
    /// For the moment a person presses Rescan, having just started something
    /// playing, and does not want to wait even a second for it to appear.
    pub fn invalidate(&mut self) {
        self.last_rescan = None;
        self.last_describe = None;
    }

    /// Run one pass.
    pub fn pass(&mut self) -> Result<Discovery> {
        let started = std::time::Instant::now();
        self.generation += 1;

        let rescan = self.last_rescan.is_none_or(|t| t.elapsed() >= RESCAN_AFTER);
        let redescribe = self
            .last_describe
            .is_none_or(|t| t.elapsed() >= REDESCRIBE_AFTER);

        if rescan {
            self.rescan(redescribe)?;
            self.last_rescan = Some(started);
            if redescribe {
                self.last_describe = Some(started);
            }
        }

        // The cheap part, and the only part most passes do: ask each session
        // it already knows about for its state and its levels.
        let mut sessions = Vec::new();
        for endpoint in &mut self.endpoints {
            endpoint.sessions.retain_mut(|s| s.refresh(rescan));
            sessions.extend(endpoint.sessions.iter().map(|s| s.info.clone()));
        }

        let devices: Vec<DeviceInfo> = self.endpoints.iter().map(|e| e.info.clone()).collect();
        let error = self.error.clone();

        let mut sources = group_sources(&sessions, &self.procs);
        add_silent_catalogued(&mut sources, &self.procs);

        // Loudest first within the sessions list as well, since that is the
        // order a person scans it in when looking for what is making a noise.
        sessions.sort_by(|a, b| {
            b.peak
                .partial_cmp(&a.peak)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
        });

        Ok(Discovery {
            devices,
            sessions,
            sources,
            elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
            generation: self.generation,
            error,
        })
    }

    /// Rebuild the endpoint and session lists.
    ///
    /// Sessions already being tracked are kept as they are, matched by their
    /// instance identifier, so a rescan does not reset a level that is being
    /// watched or make a row flicker.
    fn rescan(&mut self, redescribe: bool) -> Result<()> {
        let handles = self.devices.endpoints()?;
        self.procs = list_processes().unwrap_or_default();
        self.error = None;

        // Drop cache entries for endpoints that have gone. Unplugging a headset
        // must not leave a manager behind that fails on every future pass.
        self.endpoints
            .retain(|c| handles.iter().any(|h| h.id == c.id));

        for handle in &handles {
            let index = match self.endpoints.iter().position(|c| c.id == handle.id) {
                Some(i) => {
                    if redescribe {
                        self.endpoints[i].info = describe_endpoint(handle, true);
                    }
                    i
                }
                None => {
                    self.endpoints.push(EndpointCache {
                        id: handle.id.clone(),
                        info: describe_endpoint(handle, true),
                        manager: None,
                        sessions: Vec::new(),
                    });
                    self.endpoints.len() - 1
                }
            };

            // The default endpoint can change at any moment without the
            // endpoint set changing at all, so this one field is never cached.
            self.endpoints[index].info.is_default = handle.is_default;

            if self.endpoints[index].manager.is_none() {
                self.endpoints[index].manager =
                    unsafe { handle.device.Activate(CLSCTX_ALL, None).ok() };
            }

            let endpoint_info = self.endpoints[index].info.clone();
            let Some(manager) = self.endpoints[index].manager.clone() else {
                self.note_error(&endpoint_info.name, "no session manager");
                continue;
            };

            let enumerated = unsafe {
                // A fresh enumerator every rescan: it is a snapshot taken when
                // it is created, so a kept one would report a machine that had
                // stopped changing.
                match manager.GetSessionEnumerator() {
                    Ok(e) => e,
                    Err(e) => {
                        // One endpoint refusing to enumerate must not cost the
                        // others; half-connected Bluetooth endpoints do exactly
                        // this. Drop the manager so the next rescan retries.
                        self.endpoints[index].manager = None;
                        self.note_error(&endpoint_info.name, &e.to_string());
                        continue;
                    }
                }
            };

            let count = unsafe { enumerated.GetCount().unwrap_or(0) };
            let mut seen: Vec<String> = Vec::with_capacity(count as usize);

            for i in 0..count {
                let Ok(control) = (unsafe { enumerated.GetSession(i) }) else {
                    continue;
                };
                // Identify before adopting, so a session already being tracked
                // is left alone rather than rebuilt.
                let key = match unsafe { session_key(&control, &endpoint_info.id, i) } {
                    Some(k) => k,
                    None => continue,
                };
                seen.push(key.clone());
                if self.endpoints[index]
                    .sessions
                    .iter()
                    .any(|s| s.info.key == key)
                {
                    continue;
                }
                if let Some(live) = adopt(control, &endpoint_info, i, &self.procs) {
                    self.endpoints[index].sessions.push(live);
                }
            }

            // Anything the enumeration no longer mentions has ended.
            self.endpoints[index]
                .sessions
                .retain(|s| seen.contains(&s.info.key));
        }
        Ok(())
    }

    fn note_error(&mut self, endpoint: &str, message: &str) {
        if self.error.is_none() {
            self.error = Some(format!("{endpoint}: {message}"));
        }
    }
}

/// The identity of an enumerated session, without adopting it.
///
/// # Safety
/// `control` must be a live session control.
unsafe fn session_key(
    control: &IAudioSessionControl,
    endpoint_id: &str,
    index: i32,
) -> Option<String> {
    let control2 = control.cast::<IAudioSessionControl2>().ok()?;
    let instance = control2
        .GetSessionInstanceIdentifier()
        .ok()
        .map(|raw| take_pwstr(raw))
        .unwrap_or_default();
    if !instance.is_empty() {
        return Some(instance);
    }
    // No instance identifier: fall back to the same composite key `adopt`
    // builds, so the two agree and a session is not adopted twice.
    let pid = control2.GetProcessId().unwrap_or(0);
    Some(format!("{endpoint_id}#{pid}#{index}"))
}

/// Run one discovery pass and throw the machinery away.
///
/// Requires a COM apartment on the calling thread; see [`Apartment`]. Use
/// [`SessionMonitor`] or a long-lived [`Discoverer`] when this is going to be
/// called repeatedly: a first pass pays for everything a later one reuses.
pub fn discover_once() -> Result<Discovery> {
    Discoverer::new()?.pass()
}

// ---------------------------------------------------------------------------
// Continuous monitoring
// ---------------------------------------------------------------------------

struct Shared {
    latest: Mutex<Discovery>,
    /// Refresh interval in milliseconds.
    interval_ms: AtomicU64,
    stop: AtomicBool,
    /// Signalled to wake the thread early, for a changed interval or a manual
    /// refresh. Polling on a short timer instead would make an idle interface
    /// pay for responsiveness it is not using.
    wake: Condvar,
    wake_lock: Mutex<()>,
}

/// A background thread that keeps a discovery snapshot fresh.
///
/// The COM interfaces involved belong to the thread that created them, so this
/// owns a thread, an apartment, and everything derived from them, and publishes
/// plain data. Callers read [`SessionMonitor::latest`] as often as they like
/// without touching COM at all.
///
/// This is the same shape as the recorder: one worker, one snapshot anyone can
/// poll. It is what makes a live meter in the interface cost a memory copy
/// rather than an endpoint enumeration.
pub struct SessionMonitor {
    shared: Arc<Shared>,
    handle: Option<JoinHandle<()>>,
}

impl SessionMonitor {
    /// Start monitoring, refreshing every `interval_ms`.
    pub fn start(interval_ms: u64) -> Self {
        let shared = Arc::new(Shared {
            latest: Mutex::new(Discovery::default()),
            interval_ms: AtomicU64::new(interval_ms.clamp(50, 60_000)),
            stop: AtomicBool::new(false),
            wake: Condvar::new(),
            wake_lock: Mutex::new(()),
        });

        let worker = Arc::clone(&shared);
        let handle = std::thread::Builder::new()
            .name("ssr-discovery".into())
            .spawn(move || run(worker))
            .ok();

        SessionMonitor { shared, handle }
    }

    /// The most recent snapshot. Never blocks on a refresh in progress for
    /// longer than the copy takes.
    pub fn latest(&self) -> Discovery {
        self.shared
            .latest
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    /// Change the refresh rate, effective immediately.
    ///
    /// The interface asks for a fast rate while a live meter is on screen and a
    /// slow one when it is not, so an idle window is nearly free.
    pub fn set_interval_ms(&self, ms: u64) {
        self.shared
            .interval_ms
            .store(ms.clamp(50, 60_000), Ordering::Relaxed);
        self.shared.wake.notify_all();
    }

    /// Current refresh interval.
    pub fn interval_ms(&self) -> u64 {
        self.shared.interval_ms.load(Ordering::Relaxed)
    }

    /// Refresh now rather than at the next tick.
    pub fn refresh_now(&self) {
        self.shared.wake.notify_all();
    }
}

impl Drop for SessionMonitor {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.shared.wake.notify_all();
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn run(shared: Arc<Shared>) {
    // The apartment, the discoverer, and every COM interface inside it belong
    // to this thread and live as long as it does. That single-threaded
    // ownership is the whole reason discovery has a thread of its own: it is
    // what allows a session manager to be activated once and reused, instead
    // of every pass starting again from nothing.
    let _apartment = Apartment::new();

    let mut discoverer = match Discoverer::new() {
        Ok(d) => d,
        Err(e) => {
            // Nothing can be discovered at all. Publish the reason rather than
            // leaving the interface with a permanently empty list and no
            // explanation for it.
            if let Ok(mut slot) = shared.latest.lock() {
                slot.error = Some(e.to_string());
                slot.generation = 1;
            }
            return;
        }
    };

    while !shared.stop.load(Ordering::Relaxed) {
        let snapshot = match discoverer.pass() {
            Ok(d) => d,
            Err(e) => {
                // Keep the previous device and session lists rather than
                // blanking the interface over one failed pass, and say why.
                let mut previous = shared.latest.lock().map(|g| g.clone()).unwrap_or_default();
                previous.generation += 1;
                previous.error = Some(e.to_string());
                previous
            }
        };
        if let Ok(mut slot) = shared.latest.lock() {
            *slot = snapshot;
        }

        let wait = Duration::from_millis(shared.interval_ms.load(Ordering::Relaxed));
        if let Ok(guard) = shared.wake_lock.lock() {
            let _ = shared.wake.wait_timeout(guard, wait);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(exe: &str, state: SessionState, peak: f32, kind: DeviceKind) -> AudioSessionInfo {
        AudioSessionInfo {
            key: format!("{exe}-{peak}"),
            pid: 100,
            exe: exe.to_string(),
            label: catalog::label_for(exe),
            category: catalog::category_for(exe),
            display_name: None,
            endpoint_id: "ep".into(),
            endpoint_name: "Speakers".into(),
            endpoint_kind: kind,
            state,
            peak,
            channel_peaks: vec![peak, peak],
            volume: 1.0,
            muted: false,
            is_system_sounds: false,
            known: catalog::lookup(exe).is_some(),
            recommended: catalog::is_recommended(exe),
        }
    }

    #[test]
    fn sessions_of_one_process_collapse_into_one_source() {
        let sessions = vec![
            session("chrome.exe", SessionState::Active, 0.4, DeviceKind::Render),
            session(
                "chrome.exe",
                SessionState::Inactive,
                0.9,
                DeviceKind::Render,
            ),
        ];
        let sources = group_sources(&sessions, &[]);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].sessions, 2);
        assert_eq!(sources[0].active_sessions, 1);
        // The peak is the loudest of them, whatever their states.
        assert!((sources[0].peak - 0.9).abs() < 1e-6);
        assert!(sources[0].playing);
        assert!(!sources[0].capturing);
    }

    #[test]
    fn a_capture_session_marks_the_source_as_listening() {
        let sessions = vec![session(
            "ms-teams.exe",
            SessionState::Active,
            0.2,
            DeviceKind::Capture,
        )];
        let sources = group_sources(&sessions, &[]);
        assert!(sources[0].capturing);
        assert!(!sources[0].playing);
    }

    #[test]
    fn the_recommended_source_is_offered_first_even_when_quieter() {
        let sessions = vec![
            session("chrome.exe", SessionState::Active, 0.9, DeviceKind::Render),
            session("wfica32.exe", SessionState::Active, 0.1, DeviceKind::Render),
        ];
        let sources = group_sources(&sessions, &[]);
        assert_eq!(sources[0].exe, "wfica32.exe");
        assert_eq!(sources[0].label, "Citrix HDX session");
    }

    #[test]
    fn playing_beats_silent_and_loud_beats_quiet() {
        let sessions = vec![
            session("vlc.exe", SessionState::Inactive, 0.8, DeviceKind::Render),
            session("spotify.exe", SessionState::Active, 0.2, DeviceKind::Render),
            session("firefox.exe", SessionState::Active, 0.7, DeviceKind::Render),
        ];
        let sources = group_sources(&sessions, &[]);
        let order: Vec<&str> = sources.iter().map(|s| s.exe.as_str()).collect();
        assert_eq!(order, vec!["firefox.exe", "spotify.exe", "vlc.exe"]);
    }

    #[test]
    fn system_sounds_is_reported_but_not_offered_as_a_source() {
        let mut s = session("", SessionState::Active, 0.5, DeviceKind::Render);
        s.is_system_sounds = true;
        assert!(group_sources(&[s], &[]).is_empty());
    }

    #[test]
    fn a_running_but_silent_catalogued_application_is_still_offered() {
        let procs = vec![ProcessInfo {
            pid: 42,
            parent_pid: 1,
            name: "wfica32.exe".to_string(),
        }];
        let mut sources = Vec::new();
        add_silent_catalogued(&mut sources, &procs);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].exe, "wfica32.exe");
        assert_eq!(sources[0].pids, vec![42]);
        assert!(!sources[0].playing);
        assert_eq!(sources[0].sessions, 0);
    }

    #[test]
    fn a_process_already_discovered_is_not_added_twice() {
        let procs = vec![ProcessInfo {
            pid: 42,
            parent_pid: 1,
            name: "wfica32.exe".to_string(),
        }];
        let mut sources = group_sources(
            &[session(
                "wfica32.exe",
                SessionState::Active,
                0.3,
                DeviceKind::Render,
            )],
            &procs,
        );
        add_silent_catalogued(&mut sources, &procs);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].sessions, 1);
    }

    #[test]
    fn every_pid_of_a_process_family_is_listed_not_only_the_noisy_one() {
        let procs = vec![
            ProcessInfo {
                pid: 10,
                parent_pid: 1,
                name: "chrome.exe".into(),
            },
            ProcessInfo {
                pid: 11,
                parent_pid: 10,
                name: "chrome.exe".into(),
            },
            ProcessInfo {
                pid: 12,
                parent_pid: 1,
                name: "other.exe".into(),
            },
        ];
        let sources = group_sources(
            &[session(
                "chrome.exe",
                SessionState::Active,
                0.1,
                DeviceKind::Render,
            )],
            &procs,
        );
        assert_eq!(sources[0].pids, vec![10, 11]);
    }

    #[test]
    fn windows_resource_display_names_are_dropped_rather_than_shown() {
        assert_eq!(
            usable_display_name(r"@%SystemRoot%\System32\AudioSrv.Dll,-202".into()),
            None
        );
        assert_eq!(usable_display_name("   ".into()), None);
        assert_eq!(
            usable_display_name(" Teams call ".into()).as_deref(),
            Some("Teams call")
        );
    }
}
