// The mixer: one vertical strip per track.
//
// This is the view for a session that is already running. Horizontal meters
// are better for reading a number off one track; a rack of vertical strips is
// better for the question a multitrack recording actually raises, which is
// whether any of the eight tracks has quietly gone dead while you were
// looking somewhere else.

import { MeterStrip } from '../components/Meter'
import { Empty } from '../components/primitives'
import { IconMixer } from '../components/Icons'
import { fraction } from '../lib/format'
import { emptyMeter } from '../lib/meters'
import { isRecording, useStore } from '../state/store'

export function MixerPanel() {
  const snapshot = useStore((s) => s.snapshot)
  const meters = useStore((s) => s.meters)
  const planned = useStore((s) => s.planned)
  const floor = useStore((s) => s.prefs.meterFloor)
  const clearClip = useStore((s) => s.clearClip)
  const tracks = snapshot?.tracks ?? []

  if (tracks.length === 0) {
    return (
      <div className="panel">
        <Empty icon={IconMixer} title="No tracks running">
          {planned.length > 0
            ? `${planned.length} track(s) are armed. The strips appear here the moment recording starts.`
            : 'Arm some sources first. Every track in a session shares one timeline, so the strips here are directly comparable.'}
        </Empty>
      </div>
    )
  }

  const live = isRecording(snapshot)

  return (
    <div className="panel">
      <div className="mixer">
        {tracks.map((track) => {
          const meter = meters[track.name] ?? emptyMeter()
          const total = track.framesCaptured + track.framesSilence
          const silence = total > 0 ? track.framesSilence / total : 0
          return (
            <MeterStrip
              key={track.name}
              name={track.name}
              sub={`${(track.sampleRate / 1000).toFixed(0)}k · ${track.channels}ch`}
              meter={meter}
              floor={floor}
              onClearClip={() => clearClip(track.name)}
              footer={
                <>
                  {/* The proportion of the track that had to be manufactured.
                      A track that is almost all filled silence has recorded
                      almost nothing, however plausible its length looks. */}
                  <span
                    className={`badge badge--mono ${
                      silence > 0.8 ? 'badge--warn' : 'badge--quiet'
                    }`}
                    style={{ justifyContent: 'center' }}
                    title={`${fraction(silence)} of this track is silence the recorder manufactured to keep the timeline honest. High is normal for a source that is quiet most of the time, and a warning for one that should not be.`}
                  >
                    {fraction(1 - silence)} audio
                  </span>
                  {track.discontinuities > 0 && (
                    <span
                      className="badge badge--danger"
                      style={{ justifyContent: 'center' }}
                      title="The audio engine reported dropped samples upstream of the recorder."
                    >
                      {track.discontinuities} drop
                    </span>
                  )}
                </>
              }
            />
          )
        })}
      </div>
      <div className="panel__footer">
        <span className={`dot${live ? ' dot--live' : ' dot--ok'}`} />
        <span>
          {tracks.length} track{tracks.length === 1 ? '' : 's'}
          {live ? ' recording' : ' finished'}
        </span>
        <span className="spacer" />
        <button type="button" className="btn btn--ghost btn--sm" onClick={() => clearClip()}>
          Clear clip indicators
        </button>
      </div>
    </div>
  )
}
