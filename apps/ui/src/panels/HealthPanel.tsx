// Diagnostics: whether this recording can be trusted.
//
// A long recording is either trustworthy or merely plausible, and the
// difference is not audible. Four things decide it, and they are all here.
//
// **Dropouts** are samples the audio engine lost upstream of the recorder.
// They are the one failure that cannot be repaired or reasoned about after the
// fact, so they are reported first and never hidden.
//
// **Manufactured silence** is the proportion of a track the recorder filled in
// because the stream delivered nothing. High is entirely normal for a source
// that is quiet most of the time and alarming for one that should not be.
//
// **Capacity** is how long the free space lasts at this format, which is the
// only form of "disk space" that means anything for multitrack float audio.
//
// **Discovery cost** is how long an autodiscovery pass takes. It is here
// because if the interface ever feels heavy, this is the number to look at.

import { Empty, Notice, Section, Stat } from '../components/primitives'
import { IconPulse, IconWarning } from '../components/Icons'
import {
  bytes,
  byteRate,
  duration,
  durationWords,
  fraction,
  kilohertz,
  plural,
} from '../lib/format'
import { isRecording, useStore } from '../state/store'

export function HealthPanel() {
  const snapshot = useStore((s) => s.snapshot)
  const discovery = useStore((s) => s.discovery)
  const disk = useStore((s) => s.disk)
  const settings = useStore((s) => s.settings)
  const prefs = useStore((s) => s.prefs)
  const meters = useStore((s) => s.meters)
  const manifest = useStore((s) => s.manifest)

  const tracks = snapshot?.tracks ?? []
  const live = isRecording(snapshot)

  const dropouts = tracks.reduce((n, t) => n + t.discontinuities, 0)
  const clipped = Object.values(meters).filter((m) => m.clipped).length
  const capturedFrames = tracks.reduce((n, t) => n + t.framesCaptured, 0)
  const silenceFrames = tracks.reduce((n, t) => n + t.framesSilence, 0)
  const totalFrames = capturedFrames + silenceFrames

  const perSecond = byteRate(
    settings.sampleRate,
    settings.channels,
    Math.max(tracks.length, 1),
  )
  const secondsLeft = disk ? disk.freeBytes / perSecond : null

  return (
    <div className="panel">
      <div className="panel__body panel__body--pad stack">
        <Section>This session</Section>
        {tracks.length === 0 ? (
          <Empty icon={IconPulse} title="Nothing running">
            While a session records, this panel reports the four things that
            decide whether it can be trusted: dropouts, how much of each track
            was manufactured silence, the room left on disk, and what the
            recorder itself is costing.
          </Empty>
        ) : (
          <>
            <div className="card-grid">
              <Stat
                label="Elapsed"
                value={duration(snapshot?.elapsedSecs ?? 0)}
                tone={live ? 'ok' : 'neutral'}
              />
              <Stat label="Tracks" value={tracks.length} />
              <Stat
                label="Dropouts"
                value={dropouts}
                tone={dropouts > 0 ? 'danger' : 'ok'}
                title="Samples the audio engine lost before the recorder saw them. Not repairable afterwards."
              />
              <Stat
                label="Clipped tracks"
                value={clipped}
                tone={clipped > 0 ? 'warn' : 'ok'}
                title="Tracks that have reached full scale at some point in this session."
              />
              <Stat
                label="Real audio"
                value={fraction(totalFrames > 0 ? capturedFrames / totalFrames : 0)}
                title="The rest was silence the recorder manufactured so the files match elapsed time."
              />
              <Stat
                label="Written"
                value={bytes((snapshot?.elapsedSecs ?? 0) * perSecond)}
              />
            </div>

            {dropouts > 0 && (
              <Notice tone="danger" icon={IconWarning}>
                The audio engine reported {plural(dropouts, 'dropout')} on this
                session. That is lost audio, not a display problem, and closing
                other work on the machine is the only remedy while it is
                happening.
              </Notice>
            )}

            <Section>Per track</Section>
            {tracks.map((track) => {
              const total = track.framesCaptured + track.framesSilence
              const real = total > 0 ? track.framesCaptured / total : 0
              return (
                <div className="row" key={track.name}>
                  <div className="row__main">
                    <div className="row__title">
                      <span>{track.name}</span>
                      {track.discontinuities > 0 && (
                        <span className="badge badge--danger">
                          {plural(track.discontinuities, 'dropout')}
                        </span>
                      )}
                      {real < 0.2 && total > 0 && (
                        <span
                          className="badge badge--warn"
                          title="Almost everything on this track is manufactured silence. If the source should have been playing, it was not reaching the recorder."
                        >
                          mostly silence
                        </span>
                      )}
                    </div>
                    <div className="row__subtitle">{track.description}</div>
                  </div>
                  <span className="badge badge--mono badge--quiet">
                    {kilohertz(track.sampleRate)} · {track.channels} ch
                  </span>
                  <span className="badge badge--mono badge--quiet">{fraction(real)} audio</span>
                </div>
              )
            })}
          </>
        )}

        <Section>Capacity</Section>
        <div className="card-grid">
          <Stat
            label="Free space"
            value={disk ? bytes(disk.freeBytes) : '—'}
            tone={secondsLeft !== null && secondsLeft < 1800 ? 'warn' : 'neutral'}
          />
          <Stat
            label="Room left"
            value={secondsLeft !== null ? durationWords(secondsLeft) : '—'}
            title="At the current format and track count."
          />
          <Stat label="Rate on disk" value={`${bytes(perSecond * 60)}/min`} />
          <Stat label="Buffer" value={`${settings.bufferMs} ms`} />
        </div>

        <Section>Autodiscovery</Section>
        <div className="card-grid">
          <Stat
            label="Pass time"
            value={discovery ? `${discovery.elapsedMs.toFixed(1)} ms` : '—'}
            tone={discovery && discovery.elapsedMs > 60 ? 'warn' : 'ok'}
            title="A refresh reads a state and a set of channel peaks per session. A rescan, which happens about once a second, costs more."
          />
          <Stat label="Passes" value={discovery?.generation ?? 0} />
          <Stat label="Refresh" value={`${prefs.discoveryMs} ms`} />
          <Stat label="Sessions seen" value={discovery?.sessions.length ?? 0} />
        </div>
        {discovery?.error && <Notice tone="warn">{discovery.error}</Notice>}

        {manifest && (
          <>
            <Section>Last finished session</Section>
            <div className="card-grid">
              <Stat label="Duration" value={durationWords(manifest.duration_secs)} />
              <Stat label="Tracks" value={manifest.tracks.length} />
              <Stat
                label="Dropouts"
                value={manifest.tracks.reduce((n, t) => n + t.stats.discontinuities, 0)}
              />
              <Stat
                label="Longest gap"
                value={`${Math.max(
                  0,
                  ...manifest.tracks.map((t) => t.stats.longest_gap_ms),
                ).toFixed(0)} ms`}
                title="The longest single stretch the recorder had to fill with manufactured silence."
              />
            </div>
          </>
        )}

        <Notice>
          Every track in a session is placed against one shared origin taken
          from the performance counter, not appended as buffers arrive. That is
          what makes the files line up in an editor, and it is why the
          manufactured-silence figure matters: it is the recorder keeping each
          file's length equal to elapsed time.
        </Notice>
      </div>
    </div>
  )
}
