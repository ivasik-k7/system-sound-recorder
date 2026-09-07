// The bottom strip: one line that says whether anything is wrong.

import { isDesktop } from '../lib/api'
import { bytes, plural, shortPath } from '../lib/format'
import { isRecording, useStore } from '../state/store'
import { IconRadar, IconWarning } from './Icons'

export function StatusBar() {
  const snapshot = useStore((s) => s.snapshot)
  const settings = useStore((s) => s.settings)
  const prefs = useStore((s) => s.prefs)
  const discovery = useStore((s) => s.discovery)
  const disk = useStore((s) => s.disk)
  const error = useStore((s) => s.error)
  const log = useStore((s) => s.log)
  const meters = useStore((s) => s.meters)
  const rescan = useStore((s) => s.rescan)

  const live = isRecording(snapshot)
  const warnings = snapshot?.warnings.length ?? 0
  const dropouts = snapshot?.tracks.reduce((n, t) => n + t.discontinuities, 0) ?? 0
  const clipped = Object.values(meters).filter((m) => m.clipped).length
  const errors = log.filter((e) => e.level === 'error').length
  const lastLine = log.length > 0 ? log[log.length - 1] : undefined
  const playing = discovery?.sources.filter((s) => s.playing).length ?? 0

  return (
    <footer className="statusbar">
      <span className="statusbar__item">
        <span
          className={`dot${live ? ' dot--live' : error || errors > 0 ? ' dot--danger' : ' dot--ok'}`}
        />
        {live ? 'Recording' : error ? 'Problem' : 'Idle'}
      </span>

      <button
        type="button"
        className="statusbar__item statusbar__item--button"
        title="Rescan for audio now (Ctrl+D)"
        onClick={() => void rescan()}
      >
        <IconRadar size={12} />
        {playing > 0 ? `${playing} playing` : 'nothing playing'}
      </button>

      <span className="statusbar__item" title="Format used for per-application tracks">
        {settings.sampleRate / 1000} kHz · {settings.channels} ch · {settings.bufferMs} ms
      </span>

      {dropouts > 0 && (
        <span
          className="statusbar__item statusbar__item--danger"
          title="Samples the audio engine lost before the recorder saw them"
        >
          <IconWarning size={12} /> {plural(dropouts, 'dropout')}
        </span>
      )}

      {clipped > 0 && (
        <span className="statusbar__item statusbar__item--warn">
          {clipped} clipped
        </span>
      )}

      {warnings > 0 && (
        <span className="statusbar__item statusbar__item--warn">
          {plural(warnings, 'warning')}
        </span>
      )}

      <span className="statusbar__last">{lastLine?.message}</span>

      {disk && (
        <span className="statusbar__item" title={`${bytes(disk.freeBytes)} free`}>
          {bytes(disk.freeBytes)} free
        </span>
      )}

      <span className="statusbar__item" title={settings.outputDir}>
        {shortPath(settings.outputDir, 34)}
      </span>

      <span className="statusbar__item faint" title={`Theme: ${prefs.theme}`}>
        {prefs.theme}
      </span>

      {!isDesktop() && (
        <span
          className="badge badge--warn"
          title="No audio engine is present. The interface is running against its own simulator, which is how it can be worked on outside Windows."
        >
          preview
        </span>
      )}
    </footer>
  )
}
