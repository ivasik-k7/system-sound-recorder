// The session being assembled, or the one running.
//
// Track order is file order, and file order is the order the tracks appear in
// whatever the recording is opened with afterwards, so it is worth being able
// to set. Names are editable for the same reason: `wfica32.wav` is accurate
// and `client-call.wav` is what someone wants to find in a month.

import { useState } from 'react'
import { Empty, Notice, Section } from '../components/primitives'
import {
  IconArrowDown,
  IconArrowUp,
  IconFolder,
  IconLayers,
  IconTrash,
  IconWarning,
} from '../components/Icons'
import { api } from '../lib/api'
import { baseName, bytes, byteRate, duration, durationWords, kilohertz } from '../lib/format'
import { isRecording, useStore } from '../state/store'

/** Rate and channel count for the tracks where the recorder gets to choose. */
function CaptureFormat() {
  const settings = useStore((s) => s.settings)
  const update = useStore((s) => s.updateSettings)
  const live = isRecording(useStore((s) => s.snapshot))

  return (
    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 'var(--gap)' }}>
      <label className="field">
        <span className="field__label">Rate</span>
        <select
          className="select"
          value={settings.sampleRate}
          disabled={live}
          onChange={(e) => update({ sampleRate: Number(e.target.value) })}
        >
          <option value={44100}>44.1 kHz</option>
          <option value={48000}>48 kHz</option>
          <option value={96000}>96 kHz</option>
        </select>
      </label>

      <label className="field">
        <span className="field__label">Channels</span>
        <select
          className="select"
          value={settings.channels}
          disabled={live}
          onChange={(e) => update({ channels: Number(e.target.value) })}
        >
          <option value={1}>Mono</option>
          <option value={2}>Stereo</option>
        </select>
      </label>

      <label className="field">
        <span className="field__label">Buffer</span>
        <select
          className="select"
          value={settings.bufferMs}
          disabled={live}
          onChange={(e) => update({ bufferMs: Number(e.target.value) })}
        >
          <option value={100}>100 ms</option>
          <option value={200}>200 ms</option>
          <option value={500}>500 ms</option>
          <option value={1000}>1000 ms</option>
        </select>
      </label>
    </div>
  )
}

export function SessionPanel() {
  const planned = useStore((s) => s.planned)
  const snapshot = useStore((s) => s.snapshot)
  const settings = useStore((s) => s.settings)
  const disk = useStore((s) => s.disk)
  const removeSource = useStore((s) => s.removeSource)
  const moveSource = useStore((s) => s.moveSource)
  const renameSource = useStore((s) => s.renameSource)
  const clearSources = useStore((s) => s.clearSources)
  const update = useStore((s) => s.updateSettings)
  const sessionDir = useStore((s) => s.sessionDir)
  const [editing, setEditing] = useState<string | null>(null)
  const [draft, setDraft] = useState('')

  const live = isRecording(snapshot)
  const trackCount = live ? (snapshot?.tracks.length ?? 0) : planned.length

  // What the free space is worth at this format, which is the only form of
  // "how much room is left" that means anything for a multitrack recording.
  const perSecond = byteRate(settings.sampleRate, settings.channels, Math.max(trackCount, 1))
  const secondsLeft = disk ? disk.freeBytes / perSecond : null
  const tight = secondsLeft !== null && secondsLeft < 1800

  const commitRename = (key: string) => {
    renameSource(key, draft)
    setEditing(null)
  }

  return (
    <div className="panel">
      <div className="panel__toolbar">
        <span className="badge">{trackCount} track{trackCount === 1 ? '' : 's'}</span>
        {live && <span className="badge badge--record">recording</span>}
        <span className="spacer" />
        <button
          type="button"
          className="btn btn--sm"
          disabled={live || planned.length === 0}
          onClick={clearSources}
        >
          Clear
        </button>
      </div>

      <div className="panel__body">
        {planned.length === 0 ? (
          <Empty icon={IconLayers} title="Nothing selected yet">
            Add a playback endpoint to catch everything at once, or pick
            applications from Discovery to keep them on separate tracks that
            line up to the millisecond.
          </Empty>
        ) : (
          <>
            <Section count={planned.length}>Tracks · in file order</Section>
            {planned.map((source, index) => {
              const track = snapshot?.tracks[index]
              return (
                <div className="row" key={source.key}>
                  <span className="badge badge--mono badge--quiet" style={{ minWidth: 22 }}>
                    {index + 1}
                  </span>

                  <div className="row__main">
                    {editing === source.key ? (
                      <input
                        className="input input--sm"
                        style={{ width: '100%' }}
                        autoFocus
                        value={draft}
                        placeholder={source.title}
                        onChange={(e) => setDraft(e.target.value)}
                        onBlur={() => commitRename(source.key)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') commitRename(source.key)
                          if (e.key === 'Escape') setEditing(null)
                        }}
                      />
                    ) : (
                      <div
                        className="row__title"
                        title={live ? undefined : 'Click to rename. The name becomes the file name.'}
                        onDoubleClick={() => {
                          if (live) return
                          setDraft(source.request.label ?? '')
                          setEditing(source.key)
                        }}
                      >
                        <span>{source.title}</span>
                      </div>
                    )}
                    <div className="row__subtitle">{source.subtitle}</div>
                  </div>

                  {track && live && (
                    <span className="badge badge--quiet badge--mono">
                      {duration(track.durationSecs)}
                    </span>
                  )}

                  <div className="row__actions">
                    <button
                      type="button"
                      className="btn btn--ghost btn--sm btn--icon"
                      title="Move earlier in the file order"
                      disabled={live || index === 0}
                      onClick={() => moveSource(source.key, -1)}
                    >
                      <IconArrowUp size={12} />
                    </button>
                    <button
                      type="button"
                      className="btn btn--ghost btn--sm btn--icon"
                      title="Move later in the file order"
                      disabled={live || index === planned.length - 1}
                      onClick={() => moveSource(source.key, 1)}
                    >
                      <IconArrowDown size={12} />
                    </button>
                    <button
                      type="button"
                      className="btn btn--danger-ghost btn--sm btn--icon"
                      title="Remove from the session"
                      disabled={live}
                      onClick={() => removeSource(source.key)}
                    >
                      <IconTrash size={12} />
                    </button>
                  </div>
                </div>
              )
            })}
          </>
        )}

        <Section>Where it goes</Section>
        <div className="panel__body--pad stack">
          <label className="field">
            <span className="field__label">Recordings folder</span>
            <div className="rowline">
              <input
                className="input input--mono"
                style={{ flex: 1 }}
                value={settings.outputDir}
                disabled={live}
                onChange={(e) => update({ outputDir: e.target.value })}
              />
              <button
                type="button"
                className="btn btn--icon"
                title="Open this folder"
                onClick={() => void api.revealPath(settings.outputDir)}
              >
                <IconFolder size={13} />
              </button>
            </div>
            <span className="field__hint">
              Each session gets its own dated subfolder here, so recordings
              accumulate instead of replacing one another.
              {sessionDir && (
                <>
                  {' '}
                  Most recent: <span className="mono">{baseName(sessionDir)}</span>.
                </>
              )}
            </span>
          </label>

          {disk && (
            <Notice tone={tight ? 'warn' : 'neutral'} icon={tight ? IconWarning : undefined}>
              {bytes(disk.freeBytes)} free of {bytes(disk.totalBytes)}
              {trackCount > 0 && secondsLeft !== null && (
                <>
                  {' '}
                  — about <strong>{durationWords(secondsLeft)}</strong> of{' '}
                  {trackCount} track{trackCount === 1 ? '' : 's'} at{' '}
                  {kilohertz(settings.sampleRate)}, or {bytes(perSecond * 60)} a minute.
                </>
              )}
            </Notice>
          )}

          <Section>Format</Section>
          <CaptureFormat />
          <span className="field__hint">
            Rate and channel count apply to per-application tracks only. A
            playback endpoint dictates its own format, and the recorder follows
            it rather than resampling — so what an endpoint reports in Sources
            is what its track will contain.
          </span>
        </div>
      </div>
    </div>
  )
}
