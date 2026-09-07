// Transport.
//
// The one strip that is never docked away, because a recorder with a hidden
// record button is not a recorder. It carries the transport, the clock, and
// the three facts that have to be true at a glance: how many tracks, where
// they are going, and whether there is room.

import { api } from '../lib/api'
import {
  bytes,
  byteRate,
  durationParts,
  durationWords,
  shortPath,
} from '../lib/format'
import { isRecording, useStore } from '../state/store'
import { IconFlag, IconFolder, IconRecord, IconStop } from './Icons'
import { Switch } from './primitives'

export function CommandBar() {
  const snapshot = useStore((s) => s.snapshot)
  const planned = useStore((s) => s.planned)
  const settings = useStore((s) => s.settings)
  const prefs = useStore((s) => s.prefs)
  const setPref = useStore((s) => s.setPref)
  const disk = useStore((s) => s.disk)
  const busy = useStore((s) => s.busy)
  const start = useStore((s) => s.start)
  const stop = useStore((s) => s.stop)
  const addMarker = useStore((s) => s.addMarker)
  const update = useStore((s) => s.updateSettings)
  const sessionDir = useStore((s) => s.sessionDir)

  const live = isRecording(snapshot)
  const elapsed = live ? (snapshot?.elapsedSecs ?? 0) : 0
  const trackCount = live ? (snapshot?.tracks.length ?? 0) : planned.length
  const clock = durationParts(elapsed)

  const perSecond = byteRate(settings.sampleRate, settings.channels, Math.max(trackCount, 1))
  const written = elapsed * perSecond
  const secondsLeft = disk ? disk.freeBytes / perSecond : null

  return (
    <div className="commandbar">
      <div className="commandbar__group">
        {live ? (
          <button
            type="button"
            className="btn btn--stop"
            onClick={() => void stop()}
            disabled={busy}
            title="Stop recording and write the manifest (Space)"
          >
            <IconStop size={13} /> Stop
          </button>
        ) : (
          <button
            type="button"
            className="btn btn--record"
            onClick={() => void start()}
            disabled={busy || planned.length === 0}
            title={
              planned.length === 0
                ? 'Add a source first — an output endpoint, or an application from Discovery'
                : 'Begin recording (Space)'
            }
          >
            <IconRecord size={13} /> Record
          </button>
        )}

        <button
          type="button"
          className="btn btn--icon"
          disabled={!live}
          title="Mark this instant (M)"
          onClick={() => void addMarker('')}
        >
          <IconFlag size={13} />
        </button>
      </div>

      <div className={`clock${live ? ' clock--live' : ''}`}>
        <span>{clock.whole}</span>
        <span className="clock__frac">{clock.frac}</span>
      </div>

      <div className="commandbar__meta">
        <span>
          <strong>{trackCount}</strong> track{trackCount === 1 ? '' : 's'}
          {live ? ' recording' : planned.length > 0 ? ' armed' : ' — nothing selected'}
          {(snapshot?.markers.length ?? 0) > 0 && live
            ? ` · ${snapshot?.markers.length} marker${
                snapshot?.markers.length === 1 ? '' : 's'
              }`
            : ''}
        </span>
        {prefs.showEstimates && (
          <span>
            {live ? `${bytes(written)} written` : `${bytes(perSecond * 60)} a minute`}
            {secondsLeft !== null && trackCount > 0
              ? ` · room for ${durationWords(secondsLeft)}`
              : ''}
          </span>
        )}
      </div>

      <div className="commandbar__divider" />

      <button
        type="button"
        className="pathbutton"
        title={
          sessionDir
            ? `This session: ${sessionDir}`
            : `Recordings folder: ${settings.outputDir}`
        }
        onClick={() => void api.revealPath(sessionDir ?? settings.outputDir)}
      >
        <IconFolder size={13} />
        <span className="pathbutton__path">
          {shortPath(sessionDir ?? settings.outputDir ?? 'Choose a folder', 40)}
        </span>
      </button>

      <div className="commandbar__divider" />

      <div className="commandbar__group">
        <label className="switch" title="Stop automatically after this many seconds">
          <span>Stop after</span>
          <input
            className="input input--mono input--sm"
            style={{ width: 66 }}
            type="number"
            min={0}
            step={30}
            disabled={live}
            value={settings.durationSecs ?? ''}
            placeholder="∞"
            onChange={(e) => {
              const v = e.target.value.trim()
              update({ durationSecs: v === '' ? null : Number(v) })
            }}
          />
          <span>s</span>
        </label>
      </div>

      <div className="commandbar__spacer" />

      <div className="commandbar__group">
        <Switch
          checked={prefs.alwaysOnTop}
          onChange={(v) => setPref('alwaysOnTop', v)}
          label="On top"
          title="Keep this window above other windows"
        />
      </div>
    </div>
  )
}
