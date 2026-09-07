// A rolling level history per track, with the session's markers on it.
//
// Not a waveform. It is the level over the last minute or few, which answers
// the question a waveform cannot at this scale: has this track been receiving
// audio the whole time, or did it go quiet twenty minutes ago and nobody
// noticed? Two traces are drawn — the average as a filled area and the peak as
// a line above it — because the gap between them is what distinguishes speech
// from music from a steady tone.

import { useEffect, useMemo, useRef, useState } from 'react'
import { Empty, Segmented } from '../components/primitives'
import { IconPulse } from '../components/Icons'
import { decibels, duration } from '../lib/format'
import { meterFraction } from '../lib/meters'
import { HISTORY_LENGTH, historyAt, useStore, type History } from '../state/store'
import type { Marker } from '../lib/types'

type Window = '30' | '90' | '300'

const WINDOW_OPTIONS = [
  { value: '30' as Window, label: '30 s' },
  { value: '90' as Window, label: '90 s' },
  { value: '300' as Window, label: '5 min' },
]

interface PlotProps {
  history: History | undefined
  /** How many of the most recent samples to draw. */
  samples: number
  /** Seconds the plot spans, ending at `elapsed`. */
  windowSecs: number
  /** Samples per second in the history arrays. */
  sampleRate: number
  elapsed: number
  floor: number
  live: boolean
  tall: boolean
}

function TrackPlot({
  history,
  samples,
  windowSecs,
  sampleRate,
  elapsed,
  floor,
  live,
  tall,
}: PlotProps) {
  const ref = useRef<HTMLCanvasElement>(null)

  useEffect(() => {
    // One frame, not one per poll. The effect can fire far more often than the
    // display refreshes — five tracks at ten polls a second is fifty redraws,
    // each walking up to nine hundred points twice — and everything but the
    // last of those is thrown away by the compositor anyway.
    let frame = 0
    const draw = () => {
    const canvas = ref.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    // Match the backing store to the device pixel ratio, or the drawing is
    // soft on exactly the high resolution displays this is most likely to sit
    // on.
    const dpr = window.devicePixelRatio || 1
    const width = canvas.clientWidth
    const height = canvas.clientHeight
    if (width === 0 || height === 0) return
    if (canvas.width !== Math.round(width * dpr) || canvas.height !== Math.round(height * dpr)) {
      canvas.width = Math.round(width * dpr)
      canvas.height = Math.round(height * dpr)
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, width, height)

    const style = getComputedStyle(canvas)
    const read = (name: string, fallback: string) =>
      style.getPropertyValue(name).trim() || fallback
    const grid = read('--border', '#232a33')
    const low = read('--meter-low', '#3ecf8e')
    const high = read('--meter-high', '#f0b356')
    const clip = read('--meter-clip', '#f0666d')
    const faint = read('--text-3', '#67717f')

    const y = (db: number) => height - meterFraction(db, floor) * height

    // Reference lines at the levels anyone actually aims between.
    ctx.strokeStyle = grid
    ctx.lineWidth = 1
    ctx.setLineDash([2, 3])
    for (const db of [-6, -18, -36]) {
      if (db < floor) continue
      const line = Math.round(y(db)) + 0.5
      ctx.beginPath()
      ctx.moveTo(0, line)
      ctx.lineTo(width, line)
      ctx.stroke()
    }
    ctx.setLineDash([])

    const available = Math.min(history?.count ?? 0, samples)
    if (!history || available === 0) return

    // The window is a fixed span of time, so a new session scrolls in from
    // the right rather than stretching a handful of samples across the width.
    const total = windowSecs * sampleRate
    const step = width / total
    // Index 0 is the oldest sample being drawn, `available - 1` the newest.
    const xFor = (index: number, length: number) => width - (length - 1 - index) * step
    const sampleAt = (index: number) => historyAt(history, available - 1 - index)

    ctx.globalAlpha = live ? 1 : 0.5

    // The average, as an area. Read left to right so the path is monotonic and
    // the fill cannot self-intersect.
    ctx.beginPath()
    ctx.moveTo(xFor(0, available), height)
    for (let i = 0; i < available; i += 1) {
      ctx.lineTo(xFor(i, available), y(sampleAt(i).rms))
    }
    ctx.lineTo(xFor(available - 1, available), height)
    ctx.closePath()
    const fill = ctx.createLinearGradient(0, 0, 0, height)
    fill.addColorStop(0, high)
    fill.addColorStop(0.45, low)
    fill.addColorStop(1, 'transparent')
    ctx.fillStyle = fill
    ctx.globalAlpha = live ? 0.4 : 0.22
    ctx.fill()

    // The peak, as a line. Drawn over the area so the headroom between them
    // is visible, which is the whole point of showing both.
    ctx.globalAlpha = live ? 0.95 : 0.5
    ctx.beginPath()
    for (let i = 0; i < available; i += 1) {
      const px = xFor(i, available)
      const py = y(sampleAt(i).peak)
      if (i === 0) ctx.moveTo(px, py)
      else ctx.lineTo(px, py)
    }
    ctx.strokeStyle = low
    ctx.lineWidth = 1.25
    ctx.stroke()

    // Anywhere the peak reached full scale, marked in the clip colour. A
    // single frame of it is one pixel wide and would otherwise be invisible.
    ctx.globalAlpha = 1
    ctx.fillStyle = clip
    for (let i = 0; i < available; i += 1) {
      if (sampleAt(i).peak < -0.1) continue
      ctx.fillRect(xFor(i, available) - 1, 0, 2, height)
    }

    // Where the history does not yet fill the window, say so rather than
    // leaving an empty stretch that looks like silence.
    if (available < total) {
      const edge = xFor(0, available)
      ctx.globalAlpha = 0.5
      ctx.strokeStyle = faint
      ctx.setLineDash([2, 4])
      ctx.beginPath()
      ctx.moveTo(edge, 0)
      ctx.lineTo(edge, height)
      ctx.stroke()
      ctx.setLineDash([])
    }
    ctx.globalAlpha = 1
    }

    frame = requestAnimationFrame(draw)
    return () => cancelAnimationFrame(frame)
  }, [history, samples, windowSecs, sampleRate, floor, live, tall, elapsed])

  return <canvas ref={ref} className={`timeline__canvas${tall ? ' timeline__canvas--tall' : ''}`} />
}

/** Where in the plot a moment in the session falls, as a fraction 0..1. */
function positionOf(atSecs: number, elapsed: number, windowSecs: number): number {
  return 1 - (elapsed - atSecs) / windowSecs
}

export function TimelinePanel() {
  const snapshot = useStore((s) => s.snapshot)
  const history = useStore((s) => s.history)
  const refreshMs = useStore((s) => s.prefs.refreshMs)
  const floor = useStore((s) => s.prefs.meterFloor)
  const [window_, setWindow] = useState<Window>('90')
  const [hover, setHover] = useState<{ track: string; x: number; db: number; ago: number } | null>(
    null,
  )

  const tracks = snapshot?.tracks ?? []
  const elapsed = snapshot?.elapsedSecs ?? 0
  const live = snapshot?.state === 'recording'
  const sampleRate = 1000 / Math.max(refreshMs, 20)
  const windowSecs = Math.min(Number(window_), HISTORY_LENGTH / sampleRate)
  const markers: Marker[] = snapshot?.markers ?? []
  const tall = tracks.length <= 2

  const visibleMarkers = useMemo(
    () =>
      markers
        .map((marker) => ({ marker, at: positionOf(marker.at_secs, elapsed, windowSecs) }))
        .filter(({ at }) => at >= 0 && at <= 1),
    [markers, elapsed, windowSecs],
  )

  if (tracks.length === 0) {
    return (
      <div className="panel">
        <Empty icon={IconPulse} title="A level history appears here while recording">
          It shows the average level as an area and the peak as a line above it,
          across the last minute or so, which is how a track that has quietly
          stopped receiving audio becomes obvious.
        </Empty>
      </div>
    )
  }

  return (
    <div className="panel">
      <div className="panel__toolbar">
        <Segmented
          value={window_}
          options={WINDOW_OPTIONS}
          onChange={setWindow}
          ariaLabel="Time window"
        />
        <span className="spacer" />
        {markers.length > 0 && (
          <span className="badge badge--accent">
            {markers.length} marker{markers.length === 1 ? '' : 's'}
          </span>
        )}
        <span className="badge badge--mono badge--quiet">{duration(elapsed)}</span>
      </div>

      <div className="panel__body">
        <div className="timeline">
          {tracks.map((track) => {
            const entry = history[track.name]
            const samples = Math.ceil(windowSecs * sampleRate)
            const hovering = hover?.track === track.name ? hover : null

            return (
              <div className="timeline__track" key={track.name}>
                <div className="timeline__header">
                  <span className="timeline__name truncate">{track.name}</span>
                  <span className="faint mono">
                    {decibels(track.peakDb)} dB peak
                    {track.discontinuities > 0 ? ` · ${track.discontinuities} dropout` : ''}
                  </span>
                </div>

                <div
                  className="timeline__plot"
                  onMouseLeave={() => setHover(null)}
                  onMouseMove={(e) => {
                    const box = e.currentTarget.getBoundingClientRect()
                    const fractionAcross = (e.clientX - box.left) / box.width
                    const available = Math.min(entry?.count ?? 0, samples)
                    if (!entry || available === 0) return
                    const index = Math.round(fractionAcross * (available - 1))
                    const clamped = Math.max(0, Math.min(available - 1, index))
                    const db = historyAt(entry, available - 1 - clamped).peak
                    setHover({
                      track: track.name,
                      x: fractionAcross * box.width,
                      db,
                      ago: (1 - fractionAcross) * windowSecs,
                    })
                  }}
                >
                  <TrackPlot
                    history={entry}
                    samples={samples}
                    windowSecs={windowSecs}
                    sampleRate={sampleRate}
                    elapsed={elapsed}
                    floor={floor}
                    live={live}
                    tall={tall}
                  />

                  {visibleMarkers.map(({ marker, at }, i) => (
                    <span
                      className="timeline__marker"
                      key={`${marker.at_secs}-${i}`}
                      style={{ left: `${at * 100}%` }}
                      title={`${duration(marker.at_secs)}${marker.label ? ` — ${marker.label}` : ''}`}
                    />
                  ))}

                  {hovering && (
                    <>
                      <span className="timeline__cursor" style={{ left: hovering.x }} />
                      <span
                        className="timeline__hover"
                        style={{
                          left: Math.max(34, Math.min(hovering.x, 1e4)),
                        }}
                      >
                        {decibels(hovering.db)} dB · {hovering.ago.toFixed(1)} s ago
                      </span>
                    </>
                  )}
                </div>
              </div>
            )
          })}
        </div>
      </div>
    </div>
  )
}
