# Architecture

## The shape of the problem

Everything worth recording already meets in one place. Citrix HDX redirects
remote session audio to the local Windows playback endpoint. WSLg carries Linux
PulseAudio over RDP into the same audio engine. Browsers, chat clients and
native programs go there directly.

That gives one universal tap, WASAPI loopback, and one way to pull the mix apart
again, per-process loopback. Both live on the Windows side, which is why the
engine is a Windows binary even when it is driven from WSL.

## Layers

```
  ssr-cli                  apps/desktop (Tauri)  ──  apps/ui (React)
      └──────────────┬──────────────┘
                     │
                 ssr-session          one session, one worker thread,
                     │                a snapshot anyone can poll
                     │                plus the library of past sessions
                     │
      ┌──────────────┴──────────────┐
  ssr-core                     ssr-timeline
  WASAPI, COM, process         placement on a shared timeline,
  discovery, autodiscovery,    silence fill, WAV writing, manifests
  channel layouts, packets
```

`ssr-session` exists so the command line tool and the desktop application cannot
drift apart. Both start a `Recorder`, poll `snapshot()`, and stop it. Neither
contains any recording logic of its own.

## Autodiscovery

Recording is the second question. The first is what there is to record, and
name matching answers it badly: a list of executables misses anything nobody
thought of, and cannot tell a process that is running from one that is
actually making a sound.

Windows will answer properly. Every stream reaching an endpoint is an *audio
session*, and `IAudioSessionManager2` enumerates them. Each carries the owning
process id, its state, and — through `IAudioMeterInformation` — a live peak per
channel. `crates/ssr-core/src/sessions.rs` turns that into two views: one row
per stream, and one row per executable, which is what recording by process tree
actually takes.

The same pass reads each endpoint's mix format and decodes its channel mask
into named channels (`channels.rs`). That is not decoration. A shared-mode
capture, loopback included, must use the endpoint's mix format, so the layout
read here is exactly what the recorded file will contain, and it is what the
WAV writer stamps into the file so an editor opens the tracks with the speakers
in the right order.

### Why it is a thread, and why it caches

The interesting cost is not obvious. These interfaces are proxies to the audio
service, so every call is a cross-process round trip. A naive pass —
enumerate endpoints, activate a session manager on each, enumerate sessions,
then ask each session for its process, name, identifier, channel count, peaks,
volume and mute — is about fourteen round trips per session, and measured 70 ms
on the development machine with ten sessions open. Ten times a second, that is
most of a core spent asking questions whose answers had not changed.

Almost none of it does change. The endpoints, their names and their channel
layouts are the same as last time; so are the sessions and the processes that
own them. Only the levels move. So `Discoverer` keeps it all: the endpoint
handles, the activated session managers, and the session objects themselves. A
refresh reads a state and a set of channel peaks per session — two calls — and
takes about 3 ms. A full rescan, which is what notices a newly started stream,
runs about once a second and costs about 45 ms.

Holding COM interfaces means one thread owns them for their whole life, which
is what `SessionMonitor` provides: a worker, an apartment, and a snapshot
anyone can poll. It is deliberately the same shape as the recorder. The
interface polls a memory copy and never touches COM, which is what makes a
live meter cost nothing.

The cadences are all time-based rather than counted in passes, so they hold
whether the interface is refreshing at 10 Hz or once a second — and the desktop
shell asks for a slow rate when no panel needs discovery, so an idle window is
nearly free.

## Capture

Three source kinds, one packet stream.

| Source | Mechanism | Format chosen by |
|---|---|---|
| Endpoint loopback | `AUDCLNT_STREAMFLAGS_LOOPBACK` on a render endpoint | the endpoint |
| Process loopback | `ActivateAudioInterfaceAsync` with `VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK` | the caller |
| Input | ordinary shared-mode capture | the endpoint |

Each source owns a thread. The thread joins the multithreaded apartment, opens
its client, and loops on `WaitForMultipleObjects` over a stop event and the
audio event. Every buffer is converted to interleaved 32-bit float and handed to
the session worker over a bounded channel, carrying the performance counter
timestamp WASAPI reported for its first frame.

The wait carries a deadline. A loopback stream on an idle endpoint raises no
event at all, so an unbounded wait would park the thread until sound resumed and
the recording would lose the silence entirely.

## Placement on the timeline

This is where a recorder is won or lost. `ssr-timeline` places each block by its
timestamp against one session origin, rather than appending it in arrival order.

* **A gap** between the write head and a block's position is filled with real
  silence, so the file's length always matches elapsed time.
* **An overlap** has its leading frames trimmed rather than duplicated.
* **A block wholly in the past** is dropped and counted.
* **An idle stream** is topped up on a timer, stopping short of the present so
  the fill does not race buffers still in flight. That distance is sized from
  the capture buffer and grows itself whenever a trim proves it was too small.
  Getting this wrong is subtle and costly: with a fixed 300 ms the first sound
  after a silence was dragged about 40 ms early, because its place had already
  been filled by the time it arrived.
* **An untrustworthy timestamp**, which WASAPI flags, falls back to appending.
* **At the end**, every track is padded to the longest, so the files are
  interchangeable in an editor and none looks truncated.

All of that logic is behind a `Sink` trait, so it is tested against an in-memory
recorder with no audio stack and no disk.

## Two mistakes worth remembering

**Process loopback deadlocks if you collect the result yourself.**
`ActivateAudioInterfaceAsync` hands the audio client to a completion callback.
Signalling an event from the callback and then calling `GetActivateResult` on
the waiting thread looks reasonable and hangs every time. The result has to be
collected inside the callback.

**`PROPVARIANT` frees the blob you lend it.** The activation parameters travel in
a `PROPVARIANT` holding a `VT_BLOB` that points at a stack structure. windows-rs
gives `PROPVARIANT` a `Drop` that calls `PropVariantClear`, which calls
`CoTaskMemFree` on that pointer. Handing the allocator a stack address corrupts
the heap, and the process wedges later, nowhere near the cause. The variant must
be wrapped in `ManuallyDrop`.

Both are commented at the site in `crates/ssr-core/src/process.rs`.

## Output

A session directory holds one WAV per track and a `session.json`. The WAVs are
`WAVE_FORMAT_EXTENSIBLE`, 32-bit float, with the endpoint's channel mask, and
their chunk sizes are patched when the file closes. Past four gigabytes a RIFF
header can no longer describe the payload; the audio is still written and the
manifest says so.

The manifest records, per track, how many frames came from the device, how many
were manufactured, how many were trimmed, the longest gap, and any dropouts the
audio engine reported. That is what makes a long recording trustworthy after the
fact rather than merely plausible.

## Markers

Cue points live in the manifest rather than in the interface. A marker is worth
nothing at the moment it is dropped and a great deal an hour later, in whatever
tool the audio is opened with, so it has to be in the file. The offset is from
the session origin, which makes it the same instant on every track.

The recorder holds them rather than the worker thread, so marking a moment
never waits for the worker's next tick: pressing the key at the moment
something happens has to record that moment, not the moment the recorder got
round to noticing.

## The interface

React, with `dockview` providing real docking: panels can be dragged between
groups, tabbed, floated over the window, or torn off into a window of their own
on a second display. Every panel reads what it needs from one store, which is
what makes them freely rearrangeable. The layout is saved as it changes.

### Panels are interchangeable on purpose

There is no panel hierarchy and no panel owns state another one needs. That is
the whole reason the docking works: a panel can be moved, tabbed, floated or
torn onto another screen without anything being arranged around it. Adding one
means adding an entry to `panels/registry.tsx` and nothing else.

The registry also declares which panels need live discovery, and the shell uses
that to tell the engine how fast to refresh. A window showing only the mixer
lets discovery idle.

### Named arrangements

Twelve panels can be arranged in more ways than anyone wants to arrange them,
and the arrangement that suits setting a session up is not the one that suits
watching it run. So six are named and one click away, and the docking is there
for anyone who wants something else. Custom arrangements can be saved by name.

Each preset carries a drawn schematic, because layouts are recognised by shape
rather than by name.

A stored layout that mentions a panel this build does not have is rejected
rather than restored: falling back to a preset is far better than a workspace
with holes in it that cannot be recovered from without knowing to reset it.

### Meters have memory

A meter driven straight from the samples is useless. At ten frames a second a
transient that clipped the recording occupies one frame and the eye does not
catch it. So `lib/meters.ts` gives every meter three behaviours: the bar
follows the average, rising fast and falling slowly; a hairline follows the
instantaneous peak; and a mark holds the loudest recent peak for a second and a
half before falling at a fixed rate.

The clip indicator latches and stays lit until cleared. A clipped recording
cannot be repaired afterwards, and a warning that fades on its own is a warning
that can be missed while you are looking at something else.

Levels are drawn linear in decibels, not in amplitude. On a linear amplitude
scale everything below about a third of full scale is indistinguishable, which
is where nearly all real programme material sits.

### The simulator

Outside the desktop shell the interface talks to a simulator instead of the
engine, so it runs in an ordinary browser. It is not a stub: it models moving
levels, a machine's worth of audio sessions coming and going, a library of past
recordings and a filling disk, because a layout that only looks right against
empty data is a layout that has not been checked.
