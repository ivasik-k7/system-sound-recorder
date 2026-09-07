# Plan and status

The original plan is preserved below with each phase marked. What was built
differs from it in three ways worth noting: the WSL-native PulseAudio tap was
dropped because the Windows engine already captures WSL audio; a desktop
application was added; and autodiscovery was added, which the plan did not
anticipate at all and which turned out to answer the question that comes before
every recording — what is there to record?

## Status

| Phase | State |
|---|---|
| 0. Spike: confirm Citrix and WSL reach the mix | done |
| 1. Core engine, mixed capture | done |
| 2. Per-source tracks, process loopback | done |
| 3. Timeline correctness | done |
| 4. Control plane and ergonomics | mostly done |
| 5. Output pipeline | not started |
| 6. Native WSL tap | dropped, see below |
| 7. Desktop application | added, builds pending verification on Windows |
| 8. Autodiscovery and the interface | added, done |

### Done

**The capture engine.** Endpoint loopback, per-process loopback via
`ActivateAudioInterfaceAsync`, and input capture, all producing one packet
stream. Format negotiation covers 16, 24 and 32-bit integer and 32-bit float,
mono through surround.

**Timeline correctness.** Silence fill for idle streams, placement by the
performance counter timestamp each buffer carries, overlap trimming, stale block
rejection, and end padding so every track in a session is the same length. All
of it unit tested against an in-memory sink.

**Measured accuracy.** A system-mix track and a single-process track recorded
together for ten seconds ended one frame apart and aligned to about 5 ms, with
the per-process track leading, which is the right sign for a tap upstream of the
endpoint mix. A 440 Hz reference tone was recovered at 440.000 Hz.

Getting that number right took two corrections worth remembering. Cross
correlating the waveforms is ambiguous by a multiple of the tone's period, which
produced a confident and wrong 0.75 ms; the envelopes have to be correlated
instead. And whatever else the machine is playing lands in the system mix, so
the comparison has to be band limited around the reference tone.
`scripts/verify-session.py` does both.

**A timing bug the measurement found.** The idle silence fill stopped a fixed
300 ms behind the present. When a buffer arrived later than that, its place had
already been filled, the overlap was trimmed, and the start of the sound was
dragged about 40 ms earlier than it happened. The lookbehind is now sized from
the capture buffer and grows itself whenever a trim occurs, which brought the
two tracks from 41 ms apart to 5 ms.

**The command line tool.** `devices`, `apps`, and `rec` with live meters, a
session manifest, and process selection by name or process id.

**The interface.** React with real docking: panels drag between groups, tab
together, float over the window, and tear off onto a second display. Runs in a
plain browser against a simulator when the engine is not present — one that
models moving levels, sessions coming and going, a library and a filling disk,
because a layout only checked against empty data has not been checked.

**The toolchain.** Everything except the desktop shell cross-compiles from WSL
to Windows with no root access required.

**Autodiscovery, phase 8.** The machine's audio sessions are enumerated
directly, so what can be recorded is discovered rather than guessed from a list
of executable names: which process owns each stream, whether it is active, and
its live peak per channel. Endpoint mix formats and channel masks are resolved
alongside, which is what lets the interface say what a track will contain
before recording it.

Making that cheap enough to drive a meter was the work. A naive pass costs
about fourteen cross-process calls per audio session and measured 70 ms; the
`Discoverer` keeps the endpoint handles, the session managers and the session
objects themselves, so a refresh is two calls per session and about 3 ms, with
a full rescan roughly once a second at about 45 ms. `ssr discover --watch` is
the same engine on the command line, and is the quickest way to confirm on a
new machine that the thing you care about really does surface as a session.

**The interface, phase 8.** Twelve panels in a real docking workspace, six
named arrangements plus any the user saves, a command palette, four themes,
three densities, meters with proper ballistics and a latching clip indicator,
markers written into the manifest, a library of past sessions that distinguishes
a finished recording from an interrupted one, and a diagnostics panel reporting
dropouts, manufactured silence and what the free space is worth in minutes.

### Mostly done, phase 4

Done: automatic stop after a duration, live level meters with peak hold and clip
latching, a manifest, cue points, keyboard transport and a command palette,
panels that can be torn off onto another screen, disk capacity in minutes rather
than bytes, and auto-arming a remote session the moment one starts playing —
which is the auto-start rule that mattered, arrived at through discovery rather
than through a rules engine.

Not done:

* A background service with a control socket, so recording survives closing the
  interface.
* Global hotkeys that work when the application is not focused.
* A retroactive ring buffer, so "save the last five minutes" works after the
  fact. This is the most valuable remaining item.
* Auto-*start*, as opposed to auto-arm: discovery now notices a Citrix session
  the moment it plays and can add it to the plan, but starting the recording
  itself is still a keystroke.
* Device hotplug recovery. Switching to a headset mid-recording currently ends
  that track's stream rather than reopening it. `IMMNotificationClient` is the
  mechanism; the manifest already has a place to record the discontinuity.
  Discovery already notices the endpoint set changing, and drops its cached
  session manager when it does, so half of the plumbing exists.
* Per-channel metering of recorded *tracks*. Discovery reports per-channel
  peaks for a session, but a track's snapshot carries one figure, so the mixer
  strips draw a single column for a stereo track.

### Not started, phase 5

FLAC and Opus encoding, mixdown to a single stereo file, silence trimming, RF64
for sessions past four gigabytes, and Whisper transcription using the per-track
separation to attribute speech.

### Dropped, phase 6

The native WSL PulseAudio tap. Phase 2 already captures WSL audio two ways:
through the system mix, and as its own track by naming the WSLg process. A third
path would have added a Linux-side component and a second clock to reconcile,
for fidelity nobody had asked for.

## Risks, revisited

| Risk from the original plan | What happened |
|---|---|
| Citrix policy or deviceTRUST blocks capture | Not hit. Process loopback against the Citrix process activates normally. Untested against an active session with audio. |
| mingw cross-compile hits COM linking problems | Not hit for the engine. The desktop shell needed `windres` on `PATH`, which the bootstrap script now provides. |
| Citrix or WSLg audio surfaces under an unexpected process | Both appear where expected: `wfica32.exe` and `msrdc.exe`. |
| Long sessions drift | Addressed in phase 3 and measured. Still untested beyond ten seconds; a multi-hour run is the obvious next check. |
| Elevated processes are not capturable | Untested. |
| Autodiscovery is too expensive to poll | Hit, and fixed. The obvious implementation cost 70 ms a pass because every WASAPI session call is a cross-process round trip. Caching the session objects between passes brought a refresh to about 3 ms. |

## Two things learned the hard way

Both are recorded in `architecture.md` and commented at the site.

Process loopback deadlocks if the activation result is collected outside the
completion callback. And windows-rs gives `PROPVARIANT` a `Drop` that frees the
blob you lend it, which corrupts the heap when the blob is on the stack.

---

# Original plan

## 1. The central insight

Everything the user wants to record already converges in one place: the
**Windows audio engine**. Citrix HDX redirects remote session audio to the local
Windows render endpoint. WSLg pipes Linux PulseAudio over RDP into the same
engine. Browsers and native apps go there directly.

That means there is exactly one universal tap, **WASAPI loopback on Windows**,
and the tool must live on the Windows side even though the user works in WSL.

## 2. Source matrix

| Source | Where it surfaces | Tap |
|---|---|---|
| Citrix HDX remote audio | `wfica32.exe` process tree | process loopback |
| WSL / WSLg apps | WSLg to RDP to Windows engine | process loopback on the WSLg process |
| Browser, Teams, Slack | own process trees | process loopback per process |
| Microphone | capture endpoint | ordinary WASAPI capture |
| Everything, mixed | default render endpoint | endpoint loopback |

## 3. Technology choice

Rust with the `windows` crate, because process loopback needs direct COM access
to `ActivateAudioInterfaceAsync`, which the convenient wrappers do not expose.

Rejected: C# with NAudio, which cannot do process loopback without hand-rolled
interop; C++, which costs the whole Visual Studio C++ workload; Python, fine for
a spike but not for a multitrack engine; and a virtual audio cable, which
hijacks the default playback device and adds latency.

## 4. The hard problems

Idle-stream silence gaps. Clock drift across tracks. Process loopback lifecycle
and target rediscovery. Device hotplug. Format normalisation. Backpressure
between the capture thread and the writer.

## 5. Phases

Phase 0, a throwaway spike to confirm Citrix and WSL audio really do land in the
mix. Phase 1, the core engine and mixed capture. Phase 2, per-source tracks.
Phase 3, timeline correctness. Phase 4, control plane and ergonomics. Phase 5,
the output pipeline. Phase 6, an optional native WSL tap.

## 6. One non-technical note

Recording Citrix sessions and calls captures other people's voices. Check the
applicable policy and consent rules before using this on meetings.
