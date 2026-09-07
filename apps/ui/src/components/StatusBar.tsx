// The footer.
//
// A strip of small readouts, and nearly all of them do something. The ones
// that report a problem open the panel that explains it; the ones that report a
// place open it. An indicator you cannot act on is a decoration, so the few
// that really are inert say so by being flat and unclickable.
//
// Order is by how often it is wanted, left to right, and the message takes the
// slack in the middle so the numbers keep their positions as the window
// resizes.

import type { ComponentType } from 'react'
import type { DockviewApi } from 'dockview'
import { isDesktop } from '../lib/api'
import { api } from '../lib/api'
import { bytes, duration, plural, shortPath } from '../lib/format'
import { isRecording, useStore } from '../state/store'
import { useConversation } from '../state/conversation'
import { showPanel } from '../workspace/layout'
import {
  IconFolder,
  IconList,
  IconPulse,
  IconMessage,
  IconRadar,
  IconWarning,
  IconWave,
  type IconProps,
} from './Icons'

type Tone = 'normal' | 'live' | 'ok' | 'warn' | 'danger'

interface PillProps {
  icon?: ComponentType<IconProps>
  /** The number or word being reported. Monospaced, so it stops jittering. */
  value?: string | number
  label?: string
  title: string
  tone?: Tone
  onClick?: () => void
}

function Pill({ icon: Icon, value, label, title, tone = 'normal', onClick }: PillProps) {
  const className = `pill${tone === 'normal' ? '' : ` pill--${tone}`}${onClick ? '' : ' pill--static'}`
  const body = (
    <>
      {Icon && (
        <span className="pill__icon">
          <Icon size={12} />
        </span>
      )}
      {value !== undefined && <span className="pill__value">{value}</span>}
      {label && <span>{label}</span>}
    </>
  )
  return onClick ? (
    <button type="button" className={className} title={title} onClick={onClick}>
      {body}
    </button>
  ) : (
    <span className={className} title={title}>
      {body}
    </span>
  )
}

export function StatusBar({ api: dock }: { api?: DockviewApi | null }) {
  const snapshot = useStore((s) => s.snapshot)
  const settings = useStore((s) => s.settings)
  const discovery = useStore((s) => s.discovery)
  const disk = useStore((s) => s.disk)
  const error = useStore((s) => s.error)
  const log = useStore((s) => s.log)
  const meters = useStore((s) => s.meters)
  const rescan = useStore((s) => s.rescan)
  const listenStatus = useConversation((s) => s.status)
  const listenLabel = useConversation((s) => s.targetLabel)
  const lines = useConversation((s) => s.lines)

  const live = isRecording(snapshot)
  const tracks = snapshot?.tracks ?? []
  const warnings = snapshot?.warnings.length ?? 0
  const dropouts = tracks.reduce((n, t) => n + t.discontinuities, 0)
  const clipped = Object.values(meters).filter((m) => m.clipped).length
  const errors = log.filter((e) => e.level === 'error').length
  const lastLine = log.length > 0 ? log[log.length - 1] : undefined
  const playing = discovery?.sources.filter((s) => s.playing).length ?? 0

  /** Fraction of the volume in use, for the capacity gauge. */
  const used = disk && disk.totalBytes > 0 ? 1 - disk.freeBytes / disk.totalBytes : null
  const diskTone: Tone =
    used === null ? 'normal' : used > 0.95 ? 'danger' : used > 0.85 ? 'warn' : 'normal'

  const open = (panel: string) => (dock ? () => showPanel(dock, panel) : undefined)

  return (
    <footer className="statusbar">
      <Pill
        icon={undefined}
        value={live ? duration(snapshot?.elapsedSecs ?? 0) : undefined}
        label={live ? undefined : error || errors > 0 ? 'Problem' : 'Idle'}
        tone={live ? 'live' : error || errors > 0 ? 'danger' : 'ok'}
        title={
          live
            ? `Recording ${plural(tracks.length, 'track')} to ${snapshot?.outputDir}`
            : error ?? 'No session is running'
        }
        onClick={open('session')}
      />

      {live && (
        <Pill
          icon={IconWave}
          value={tracks.length}
          label={tracks.length === 1 ? 'track' : 'tracks'}
          title="Open the mixer"
          tone="normal"
          onClick={open('mixer')}
        />
      )}

      <span className="statusbar__rule" />

      <Pill
        icon={IconRadar}
        value={playing}
        label={playing === 1 ? 'playing' : 'playing'}
        title="Applications making a sound right now. Click to rescan."
        tone={playing > 0 ? 'ok' : 'normal'}
        onClick={() => void rescan()}
      />

      <Pill
        icon={IconPulse}
        value={discovery ? `${discovery.elapsedMs.toFixed(1)}` : '—'}
        label="ms"
        title="How long the last autodiscovery pass took. Open diagnostics."
        tone={discovery && discovery.elapsedMs > 60 ? 'warn' : 'normal'}
        onClick={open('health')}
      />

      {/* Live transcription runs alongside recording rather than instead of
          it, so it reports itself here rather than owning the window. */}
      {listenStatus.active && (
        <Pill
          icon={IconMessage}
          value={lines.length}
          label={lines.length === 1 ? 'line' : 'lines'}
          title={`Reading ${listenLabel || 'a channel'} as it is spoken. Open the conversation.`}
          tone="ok"
          onClick={open('conversation')}
        />
      )}

      <span className="statusbar__rule" />

      {dropouts > 0 && (
        <Pill
          icon={IconWarning}
          value={dropouts}
          label={dropouts === 1 ? 'dropout' : 'dropouts'}
          title="Samples the audio engine lost before the recorder saw them. Open diagnostics."
          tone="danger"
          onClick={open('health')}
        />
      )}

      {clipped > 0 && (
        <Pill
          value={clipped}
          label="clipped"
          title="Tracks that reached full scale. Open the mixer to clear the indicators."
          tone="warn"
          onClick={open('mixer')}
        />
      )}

      {warnings > 0 && (
        <Pill
          icon={IconWarning}
          value={warnings}
          label={warnings === 1 ? 'warning' : 'warnings'}
          title="Warnings from this session. Open the activity log."
          tone="warn"
          onClick={open('log')}
        />
      )}

      {errors > 0 && (
        <Pill
          icon={IconList}
          value={errors}
          label={errors === 1 ? 'error' : 'errors'}
          title="Errors reported. Open the activity log."
          tone="danger"
          onClick={open('log')}
        />
      )}

      <span className="statusbar__message" title={lastLine?.message}>
        {lastLine?.message}
      </span>

      <Pill
        value={`${settings.sampleRate / 1000}k`}
        label={`· ${settings.channels}ch · ${settings.bufferMs}ms`}
        title="Format used for per-application tracks, and the capture buffer. Open the session panel to change it."
        onClick={open('session')}
      />

      <span className="statusbar__rule" />

      {disk && (
        <button
          type="button"
          className={`pill${diskTone === 'normal' ? '' : ` pill--${diskTone}`}`}
          title={`${bytes(disk.freeBytes)} free of ${bytes(disk.totalBytes)}. Open diagnostics for what that is worth in minutes.`}
          onClick={open('health')}
        >
          <span className="pill__gauge">
            <span
              className={`pill__gaugeFill${diskTone === 'normal' ? '' : ` pill__gaugeFill--${diskTone}`}`}
              style={{ width: `${Math.min(100, Math.max(2, (used ?? 0) * 100))}%` }}
            />
          </span>
          <span className="pill__value">{bytes(disk.freeBytes)}</span>
          <span>free</span>
        </button>
      )}

      <Pill
        icon={IconFolder}
        label={shortPath(settings.outputDir, 26)}
        title={`Open ${settings.outputDir}`}
        onClick={() => void api.revealPath(settings.outputDir)}
      />

      {!isDesktop() && (
        <>
          <span className="statusbar__rule" />
          <Pill
            label="preview"
            tone="warn"
            title="No audio engine is present. The interface is running against its own simulator, which is how it can be worked on outside Windows."
          />
        </>
      )}
    </footer>
  )
}
