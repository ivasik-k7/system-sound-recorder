# Changelog

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project uses [semantic versioning](https://semver.org/spec/v2.0.0.html).
While the version is below 1.0, the interfaces — including the manifest format —
may change between minor versions.

The release workflow reads the section for the version being tagged and uses it
as the release notes, so a section that says nothing produces a release that
says nothing.

## [Unreleased]

Nothing yet.

## [0.2.1] - 2026-09-07

### Added

- The version now names the build it came from. The command line, the About
  box and every session manifest report `v0.2.1-<commit>`; the package
  manifests keep the bare `0.2.1`, which is all Cargo, npm and an MSI product
  version will accept. Between two tags there are many builds, and a bug report
  or a recording from six months ago needs to name one of them.
- Continuous integration: formatting, clippy at `-D warnings`, the tests on
  Windows, the interface typecheck and build, a cross-compile from Linux
  matching the documented WSL workflow, the declared minimum Rust versions, and
  a licence and advisory check.
- A release workflow. Tagging `vX.Y.Z` builds the command line recorder and the
  MSI and NSIS installers on a Windows runner, writes `SHA256SUMS.txt`, and
  publishes a GitHub Release. The installers exist only here: the local
  cross-compile from WSL cannot bundle them.
- `scripts/check-versions.sh`, because the version lives in five files and
  nothing was checking that they agreed.
- `scripts/third-party-notices.py`, which generates `THIRD-PARTY-NOTICES.md`
  from the dependency graphs rather than from memory.
- Legal and policy documents: `LICENSE`, `LEGAL.md` on recording other people,
  `PRIVACY.md` on what the program does and does not do with data,
  `SECURITY.md`, `CONTRIBUTING.md` and `CODE_OF_CONDUCT.md`.
- `deny.toml`, a dependency policy whose licence allow list is the set the tree
  actually uses.

### Changed

- The declared minimum Rust version was 1.82 and was not true. The engine needs
  1.85, because clap requires edition 2024; the desktop shell needs 1.88,
  because of the bundler's dependencies. Both are now declared correctly and
  checked on every push.
- The whole tree is `rustfmt`-clean, which it had never been.
- The crates are marked `publish = false`, which they are: they depend on each
  other by path, and crates.io does not allow that.
- A root `package-lock.json` now exists, so the interface can be installed
  reproducibly. The root declared npm workspaces but carried no lockfile, which
  meant `npm ci` could not work anywhere.

### Fixed

- **A write failure part way through a session no longer loses the whole
  recording.** An I/O error — the disk filling is the realistic one — used to
  propagate out of the session worker, skipping finalisation: the WAV headers
  were never patched and no manifest was written, so an hour of captured audio
  was left in files that read as empty. A failing track is now retired with a
  warning and everything else is closed properly.
- **A stalled writer can no longer stall the audio engine.** The capture thread
  handed each packet to the writer *before* releasing the WASAPI buffer, so a
  slow disk blocked the buffer, overran the engine's ring and lost real audio.
  The buffer is released first, and a handover that cannot complete within
  250 ms drops the packet and counts it in the session warnings rather than
  waiting.
- Closing the window stopped the recording but left live transcription and its
  speech worker running behind it.
- Child processes are placed in a job object that dies with the program, so a
  crash no longer orphans a speech worker holding a model in memory.
- `PRIVACY.md` claimed nothing is written to a temporary directory. Recording
  writes none, but transcription does — it chunks audio through `%TEMP%` — and
  the document now says exactly what, where, and when it is not cleaned up.
- Five clippy findings, all mechanical.

### Performance

- Status is published on the tick rather than on every packet, and updated in
  place: three string allocations per track per publish became none.
- Packets are routed by a shared key instead of allocating a copy of the track
  name for every buffer.
- The WAV writer reuses one silence buffer instead of allocating 32 KB each
  time an idle track is topped up.
- The interface keeps level history in ring buffers. It was copying two
  nine-hundred-element arrays per track per poll — about a third of a megabyte
  a second of garbage on a five-track session.
- Meters poll a levels-only call, about a twelfth the size of a full discovery
  snapshot, with a full pass on the slow cadence that finds new sessions.
- Timeline redraws are coalesced into one animation frame instead of one per
  poll per track.
- Polling stops while the window is hidden. The recording is on the engine's
  own thread and is unaffected.
