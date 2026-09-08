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

## [0.3.0] - 2026-09-08

A release about believing the transcript. Recognition was producing text that
nothing downstream could distinguish from speech, writing it to whichever
directory the program happened to be started from, and describing it with
metadata it had thrown away. The minor version rather than the patch, because
the transcript format, the speaker labels and two public functions all change
shape.

### Added

- **The engine's loops are detected and removed.** A speech model occasionally
  falls into repeating one phrase until its window ends — forty copies of a
  stumble, or a minute of bare full stops over silence. `sanity.rs` collapses
  any phrase repeated three times or more to a single copy, and drops what is
  left if it turns out to be nothing but punctuation. Measured against a
  forty-five minute interview recorded with this tool, that is 3,378 words out
  of 10,256: a third of what came back was never said. The number removed is
  shown while listening, because a transcript that was cleaned should say so.
- **A warning before saving somewhere a transcript should not go.** The save
  dialog now checks the chosen folder and says so if it sits inside a git
  repository or a sync folder. It warns; it does not refuse.
- Transcripts record the language and the model that produced them. Saved files
  used to say `"language": "unknown"` and name no model at all, which makes a
  transcript impossible to judge or to reproduce.

### Changed

- **Speaker attribution is a field of its own, not a prefix on the words.** It
  used to be pasted onto the front of the text, so every consumer — the plain
  text export, the subtitles, the word count, anything reading the JSON —
  received it as though it were something somebody had said. One transcript
  carried four thousand words of attribution inside its own prose and reported
  a word count half again as large as the words in it.
- **Labels are `Speaker 2`, not `<channel>, speaker 2`.** The channel is already
  on the line and in the header. The qualifier also used to depend on how many
  voices had been heard *so far*, so the first line of a conversation was named
  after the channel and everything after it was numbered — and that first line
  then appeared in the speaker list as a person of its own.
- **The plain text export is a conversation again.** One paragraph per turn.
  For a forty-five minute recording it had been a single line of nine thousand
  words with no breaks in it.
- **Meeting notes distinguish what a phrase means from what it might mean.**
  Cues are in two tiers: `action item` and `we agreed` are reported as said,
  `let's` and `can you` as guesses and only when the line has some substance
  behind it. Near-duplicates collapse, and each section is capped and says how
  many it left out. On the interview above this turns ninety-eight bullets,
  nearly all of them noise, into forty-one with the certain ones first.
- The speech worker decodes with a short temperature fallback again, and the
  thresholds that trigger it. Disabling it kept a hard passage from taking many
  times as long as an easy one, but it is also the mechanism that catches the
  model looping, and that trade was not worth making.
- `notes::classify` returns a `Match` rather than a tuple, and `Segment` has a
  `speaker` field. Both are breaking changes for anything built against them.

### Fixed

- **Transcripts were written to a relative path.** The default was
  `transcripts/`, which resolves against wherever the program was started from
  rather than anywhere anyone chose. In practice that was a source checkout, so
  transcripts of real meetings accumulated inside a git repository. The default
  is now an absolute path under `Documents`, and two tests pin that it is
  absolute and never the working directory.
- **A stalled utterance was primed into the next one.** `condition_on_previous_text`
  is off precisely so that a bad decode cannot propagate, and then the previous
  line was handed back as a prompt, which propagates it by another route. That
  is why loops came in runs of three and four consecutive utterances.
- The activity log had no styles of its own, so each line rendered as two
  adjacent spans and read `10:25:12 AMReady.` — the timestamp welded to the
  message. Timestamps are now a dim monospace column with the messages aligned
  beside them.
- **Segment end times could not be true.** The model reports the end of its
  padded window rather than the end of the speech, so a two-word answer came
  back stamped as twenty-nine seconds and ran past the start of the next two
  utterances. Speaking times for a meeting added up to fifty-two minutes inside
  a forty-five minute recording. Ends are now clamped to the utterance they came
  from and trimmed back to the next start. Nothing invents a better time; it
  only declines to state an impossible one.

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
