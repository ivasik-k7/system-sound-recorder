// The manifest of the session that just finished.
//
// This is the file that says what each track was, that they share one
// timeline, and how much of each one the recorder had to manufacture. It is
// shown two ways because it is read for two reasons: as a summary, when
// someone wants to know whether the recording is good, and as raw JSON, when
// someone is about to feed it to something else.

import { useState } from 'react'
import { Empty, Notice, Section, Segmented } from '../components/primitives'
import { IconClipboard, IconFolder, IconLayers } from '../components/Icons'
import { api } from '../lib/api'
import { duration, fraction, kilohertz, localTime, plural } from '../lib/format'
import { useStore } from '../state/store'

type View = 'summary' | 'json'

export function ManifestPanel() {
  const manifest = useStore((s) => s.manifest)
  const sessionDir = useStore((s) => s.sessionDir)
  const outputDir = useStore((s) => s.settings.outputDir)
  const notify = useStore((s) => s.notify)
  const [view, setView] = useState<View>('summary')

  if (!manifest) {
    return (
      <div className="panel">
        <Empty icon={IconLayers} title="No finished session yet">
          When a recording stops, its manifest appears here: one entry per
          track, with the shared timeline origin and what the recorder had to
          paper over.
        </Empty>
      </div>
    )
  }

  const json = JSON.stringify(manifest, null, 2)
  const dropouts = manifest.tracks.reduce((n, t) => n + t.stats.discontinuities, 0)

  return (
    <div className="panel">
      <div className="panel__toolbar panel__toolbar--wrap">
        <Segmented
          value={view}
          options={[
            { value: 'summary' as View, label: 'Summary' },
            { value: 'json' as View, label: 'JSON' },
          ]}
          onChange={setView}
          ariaLabel="Manifest view"
        />
        <span className="spacer" />
        <button
          type="button"
          className="btn btn--sm"
          onClick={() => {
            void navigator.clipboard
              .writeText(json)
              .then(() => notify('ok', 'Manifest copied'))
              .catch(() => notify('warn', 'Could not reach the clipboard'))
          }}
        >
          <IconClipboard size={12} /> Copy
        </button>
        <button
          type="button"
          className="btn btn--sm btn--icon"
          title={sessionDir ?? outputDir}
          onClick={() => void api.revealPath(sessionDir ?? outputDir)}
        >
          <IconFolder size={13} />
        </button>
      </div>

      <div className="panel__body">
        {view === 'json' ? (
          <pre className="code">{json}</pre>
        ) : (
          <>
            <Section>Session</Section>
            <div className="panel__body--pad card-grid">
              <div className="stat">
                <span className="stat__value">{duration(manifest.duration_secs)}</span>
                <span className="stat__label">Duration</span>
              </div>
              <div className="stat">
                <span className="stat__value">{manifest.tracks.length}</span>
                <span className="stat__label">Tracks</span>
              </div>
              <div className={`stat${dropouts > 0 ? ' stat--danger' : ' stat--ok'}`}>
                <span className="stat__value">{dropouts}</span>
                <span className="stat__label">Dropouts</span>
              </div>
              <div className="stat">
                <span className="stat__value" style={{ fontSize: 'var(--fs-3)' }}>
                  {localTime(manifest.started_utc)}
                </span>
                <span className="stat__label">Started</span>
              </div>
            </div>

            <Section count={manifest.tracks.length}>Tracks</Section>
            {manifest.tracks.map((track) => {
              const total = track.stats.frames_captured + track.stats.frames_silence_filled
              const real = total > 0 ? track.stats.frames_captured / total : 0
              return (
                <div className="row" key={track.file}>
                  <div className="row__main">
                    <div className="row__title">
                      <span>{track.name}</span>
                      <span className="badge badge--quiet badge--mono">{track.file}</span>
                      {track.size_overflow && (
                        <span
                          className="badge badge--warn"
                          title="The file grew past what a RIFF header can describe. The audio is all there; the header understates its length."
                        >
                          over 4 GB
                        </span>
                      )}
                    </div>
                    <div className="row__subtitle">{track.source}</div>
                  </div>
                  <span className="badge badge--mono badge--quiet">
                    {kilohertz(track.sample_rate)} · {track.channels} ch
                  </span>
                  <span className="badge badge--mono badge--quiet">
                    {duration(track.duration_secs)}
                  </span>
                  <span
                    className={`badge badge--mono ${real < 0.2 ? 'badge--warn' : 'badge--quiet'}`}
                    title={`${fraction(1 - real)} manufactured silence, longest gap ${track.stats.longest_gap_ms.toFixed(0)} ms`}
                  >
                    {fraction(real)} audio
                  </span>
                  {track.stats.discontinuities > 0 && (
                    <span className="badge badge--danger">
                      {track.stats.discontinuities} drop
                    </span>
                  )}
                </div>
              )
            })}

            {(manifest.markers?.length ?? 0) > 0 && (
              <>
                <Section count={manifest.markers?.length}>Markers</Section>
                <div className="panel__body--pad chip-list">
                  {manifest.markers?.map((marker, i) => (
                    <span className="chip" key={i}>
                      {duration(marker.at_secs)} {marker.label}
                    </span>
                  ))}
                </div>
              </>
            )}

            {(manifest.warnings?.length ?? 0) > 0 && (
              <>
                <Section>Warnings</Section>
                {manifest.warnings?.map((warning) => (
                  <Notice tone="warn" key={warning}>
                    {warning}
                  </Notice>
                ))}
              </>
            )}

            <Notice>
              Every track's frame zero is the same instant — the timeline origin
              recorded as <span className="mono">session_t0_qpc_100ns</span> — so
              the files can be dropped into an editor as they are and will line
              up. {plural(manifest.tracks.length, 'file')} plus this manifest is
              the whole session.
            </Notice>
          </>
        )}
      </div>
    </div>
  )
}
