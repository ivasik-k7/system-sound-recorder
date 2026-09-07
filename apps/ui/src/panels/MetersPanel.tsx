// Levels, read one track at a time.
//
// The companion to the mixer: where the strips are for comparing tracks, this
// is for reading one — a full scale, a decibel figure, and what proportion of
// the track has actually been audio rather than manufactured silence.

import { Meter } from '../components/Meter'
import { Empty } from '../components/primitives'
import { IconWave } from '../components/Icons'
import { duration, fraction, kilohertz } from '../lib/format'
import { emptyMeter } from '../lib/meters'
import { useStore } from '../state/store'

export function MetersPanel() {
  const snapshot = useStore((s) => s.snapshot)
  const meters = useStore((s) => s.meters)
  const floor = useStore((s) => s.prefs.meterFloor)
  const clearClip = useStore((s) => s.clearClip)
  const tracks = snapshot?.tracks ?? []

  if (tracks.length === 0) {
    return (
      <div className="panel">
        <Empty icon={IconWave} title="Levels appear here once recording starts">
          Each meter holds its peak for a moment and latches if a track ever
          reaches full scale, so a transient that clipped is still visible
          afterwards.
        </Empty>
      </div>
    )
  }

  return (
    <div className="panel">
      <div className="panel__body">
        <div className="meter-list">
          {tracks.map((track) => {
            const total = track.framesCaptured + track.framesSilence
            return (
              <Meter
                key={track.name}
                name={track.name}
                meter={meters[track.name] ?? emptyMeter()}
                floor={floor}
                onClearClip={() => clearClip(track.name)}
                detail={`${duration(track.durationSecs)} · ${kilohertz(
                  track.sampleRate,
                )} · ${track.channels} ch · ${fraction(
                  total > 0 ? track.framesCaptured / total : 0,
                )} audio`}
                badge={
                  track.discontinuities > 0 ? (
                    <span className="badge badge--danger">
                      {track.discontinuities} dropout
                      {track.discontinuities === 1 ? '' : 's'}
                    </span>
                  ) : undefined
                }
              />
            )
          })}
        </div>
      </div>
    </div>
  )
}
