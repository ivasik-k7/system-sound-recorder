// Level meters.
//
// Two shapes for two questions. The horizontal meter is for reading a level:
// it has room for a number, a scale and a name. The vertical strip is for
// comparing levels: eight of them side by side answer "is anything dead?" in
// one glance, which no amount of horizontal bars will do.
//
// Both share the ballistics in lib/meters.ts, so a track reads the same
// wherever it is shown.

import { bandColour, meterFraction, METER_TICKS, type MeterState } from '../lib/meters'
import { decibels } from '../lib/format'

interface MeterProps {
  name: string
  meter: MeterState
  /** Bottom of the scale, in dBFS. */
  floor?: number
  /** A second line under the name. */
  detail?: string
  /** Hide the numbered scale, for a compact list. */
  compact?: boolean
  onClearClip?: () => void
  badge?: React.ReactNode
}

/** The latching clip indicator. Clickable, because it has to be clearable. */
export function ClipLed({ lit, onClear }: { lit: boolean; onClear?: () => void }) {
  return (
    <button
      type="button"
      className={`clip-led${lit ? ' clip-led--lit' : ''}`}
      onClick={onClear}
      disabled={!lit}
      title={
        lit
          ? 'This track reached full scale. Click to clear, and lower the source before recording anything you need to keep.'
          : 'Lights when a track reaches full scale, and stays lit until cleared.'
      }
    >
      CLIP
    </button>
  )
}

export function Meter({
  name,
  meter,
  floor = -60,
  detail,
  compact,
  onClearClip,
  badge,
}: MeterProps) {
  const rms = meterFraction(meter.rmsDb, floor)
  const peak = meterFraction(meter.peakDb, floor)
  const hold = meterFraction(meter.holdDb, floor)

  return (
    <div className="meter">
      <div className="meter__head">
        <span className="meter__name">{name}</span>
        {badge}
      </div>
      <div className="meter__readout">
        <strong>{decibels(meter.peakDb)}</strong>
        <span>dB</span>
        <ClipLed lit={meter.clipped} onClear={onClearClip} />
      </div>

      <div
        className="meter__bar"
        role="meter"
        aria-label={`${name} level`}
        aria-valuenow={Math.round(meter.peakDb)}
        aria-valuemin={floor}
        aria-valuemax={0}
        aria-valuetext={`${decibels(meter.peakDb)} decibels`}
      >
        {/* The gradient is sized to the whole bar and clipped by the fill, so
            a given colour always sits at the same decibel value. */}
        <div
          className="meter__fill"
          style={{
            width: `${rms * 100}%`,
            ['--meter-width' as string]: `${(1 / Math.max(rms, 0.001)) * 100}%`,
          }}
        />
        {hold > 0 && <div className="meter__hold" style={{ left: `calc(${hold * 100}% - 2px)` }} />}
        {peak > 0 && <div className="meter__peak" style={{ left: `calc(${peak * 100}% - 2px)` }} />}
      </div>

      {!compact && (
        <div className="meter__scale" aria-hidden="true">
          {METER_TICKS.filter((db) => db >= floor).map((db, i, all) => (
            <span
              key={db}
              className={`meter__tick${
                i === 0 ? ' meter__tick--edge-left' : i === all.length - 1 ? ' meter__tick--edge-right' : ''
              }`}
              style={
                i === 0 || i === all.length - 1
                  ? undefined
                  : { left: `${meterFraction(db, floor) * 100}%` }
              }
            >
              {db}
            </span>
          ))}
        </div>
      )}

      {detail && <div className="meter__detail">{detail}</div>}
    </div>
  )
}

// ---------------------------------------------------------------------------

interface StripProps {
  name: string
  sub?: string
  meter: MeterState
  /** Per-channel peaks as linear amplitudes, when the source reports them.
   *  One column is drawn per channel, so a 5.1 endpoint shows six. */
  channelPeaks?: number[]
  channelLabels?: string[]
  floor?: number
  onClearClip?: () => void
  /** Show the decibel scale beside the columns. Off for a crowded rack. */
  scale?: boolean
  footer?: React.ReactNode
}

/** A vertical mixer strip. */
export function MeterStrip({
  name,
  sub,
  meter,
  channelPeaks,
  channelLabels,
  floor = -60,
  onClearClip,
  scale = true,
  footer,
}: StripProps) {
  const columns =
    channelPeaks && channelPeaks.length > 0
      ? channelPeaks.map((peak) => (peak <= 0.000001 ? -120 : 20 * Math.log10(peak)))
      : [meter.rmsDb]
  const hold = meterFraction(meter.holdDb, floor)

  return (
    <div className={`strip${columns.length > 3 ? ' strip--wide' : ''}`}>
      <span className="strip__name" title={name}>
        {name}
      </span>

      <div className="strip__meters">
        {scale && (
          <div className="strip__scale" aria-hidden="true">
            {[0, -12, -24, -36, -48, -60]
              .filter((db) => db >= floor)
              .map((db) => (
                <span key={db}>{db === 0 ? '0' : db}</span>
              ))}
          </div>
        )}
        {columns.map((db, i) => {
          const level = meterFraction(db, floor)
          return (
            <div
              className={`vmeter${columns.length === 1 ? ' vmeter--single' : ''}`}
              key={i}
              role="meter"
              aria-label={`${name} ${channelLabels?.[i] ?? `channel ${i + 1}`}`}
              aria-valuenow={Math.round(db)}
              aria-valuemin={floor}
              aria-valuemax={0}
              title={`${channelLabels?.[i] ?? `Channel ${i + 1}`}: ${decibels(db)} dB`}
            >
              {/* Reference lines at -6 and -18, the two levels anyone
                  actually aims between. */}
              {[-6, -18].map((db2) => (
                <span
                  key={db2}
                  className="vmeter__grid"
                  style={{ bottom: `${meterFraction(db2, floor) * 100}%` }}
                />
              ))}
              <div
                className="vmeter__fill"
                style={{
                  height: `${level * 100}%`,
                  ['--meter-height' as string]: `${(1 / Math.max(level, 0.001)) * 100}%`,
                }}
              />
              {hold > 0 && (
                <span className="vmeter__hold" style={{ bottom: `${hold * 100}%` }} />
              )}
            </div>
          )
        })}
      </div>

      {channelLabels && channelLabels.length > 1 && (
        <div className="chip-list" style={{ justifyContent: 'center' }}>
          {channelLabels.map((label) => (
            <span className="chip" key={label} style={{ padding: '0 3px' }}>
              {label}
            </span>
          ))}
        </div>
      )}

      <span className="strip__readout" style={{ color: bandColour(meter.peakDb) }}>
        {decibels(meter.peakDb)}
      </span>

      <div className="strip__buttons">
        <ClipLed lit={meter.clipped} onClear={onClearClip} />
      </div>

      {sub && (
        <span className="strip__sub" title={sub}>
          {sub}
        </span>
      )}
      {footer}
    </div>
  )
}

// ---------------------------------------------------------------------------

/**
 * A single small bar, for a list row.
 *
 * Used in discovery, where twenty rows each need a level and none of them has
 * room for a meter. Peak hold still applies: without it a notification sound
 * that lasted a third of a second leaves no trace at all.
 */
export function MiniMeter({
  meter,
  floor = -60,
  width = 56,
  title,
}: {
  meter: MeterState
  floor?: number
  width?: number
  title?: string
}) {
  const level = meterFraction(meter.rmsDb, floor)
  const hold = meterFraction(meter.holdDb, floor)
  return (
    <div
      className="meter__bar"
      style={{ width, height: 7, flex: 'none' }}
      role="meter"
      aria-valuenow={Math.round(meter.peakDb)}
      aria-valuemin={floor}
      aria-valuemax={0}
      aria-label={title ?? 'level'}
      title={title ?? `${decibels(meter.peakDb)} dB`}
    >
      <div
        className="meter__fill"
        style={{
          width: `${level * 100}%`,
          ['--meter-width' as string]: `${(1 / Math.max(level, 0.001)) * 100}%`,
        }}
      />
      {hold > 0 && <div className="meter__hold" style={{ left: `calc(${hold * 100}% - 1px)` }} />}
    </div>
  )
}
