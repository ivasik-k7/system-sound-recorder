// Cue points.
//
// A marker is worth nothing at the moment it is dropped and a great deal an
// hour later, which is why they go into the manifest rather than staying in
// the interface: whatever the audio is opened with afterwards can read them.
//
// The shortcut matters more than this panel does. Pressing M during a call
// takes no attention; typing a note does, and by the time you have typed it
// the moment you wanted to mark has passed. So M marks the instant
// immediately, and the label can be filled in here afterwards.

import { useState } from 'react'
import { Empty, Kbd, Notice } from '../components/primitives'
import { IconFlag, IconPlus } from '../components/Icons'
import { duration } from '../lib/format'
import { isRecording, useStore } from '../state/store'

export function MarkersPanel() {
  const snapshot = useStore((s) => s.snapshot)
  const manifest = useStore((s) => s.manifest)
  const addMarker = useStore((s) => s.addMarker)
  const [label, setLabel] = useState('')

  const live = isRecording(snapshot)
  // While recording, the live snapshot is the truth. Afterwards the manifest
  // is, because it is what was actually written to disk.
  const markers = live ? (snapshot?.markers ?? []) : (manifest?.markers ?? snapshot?.markers ?? [])

  const drop = () => {
    void addMarker(label)
    setLabel('')
  }

  return (
    <div className="panel">
      <div className="panel__toolbar">
        <input
          className="input"
          style={{ flex: 1 }}
          placeholder={live ? 'Label for the next marker (optional)' : 'Recording is not running'}
          value={label}
          disabled={!live}
          onChange={(e) => setLabel(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') drop()
          }}
        />
        <button
          type="button"
          className="btn btn--sm"
          disabled={!live}
          title="Mark this instant"
          onClick={drop}
        >
          <IconPlus size={12} /> Mark
        </button>
      </div>

      <div className="panel__body">
        {markers.length === 0 ? (
          <Empty icon={IconFlag} title="No markers yet">
            Press <Kbd keys="M" /> while recording to mark the instant something
            happens. Markers are written into the session manifest, so they are
            still there when the audio is opened somewhere else.
          </Empty>
        ) : (
          <>
            {markers.map((marker, i) => (
              <div className="row row--interactive" key={`${marker.at_secs}-${i}`}>
                <span className="row__icon row__icon--accent">
                  <IconFlag size={13} />
                </span>
                <div className="row__main">
                  <div className="row__title">
                    <span>{marker.label || `Marker ${i + 1}`}</span>
                  </div>
                  <div className="row__subtitle">
                    {duration(marker.at_secs)} into the session
                  </div>
                </div>
                <span className="badge badge--mono badge--quiet">
                  {duration(marker.at_secs)}
                </span>
              </div>
            ))}
            {!live && markers.length > 0 && (
              <Notice>
                These markers are in the session manifest at{' '}
                <span className="mono">session.json</span>, as offsets in seconds
                from the shared timeline origin — the same instant on every track.
              </Notice>
            )}
          </>
        )}
      </div>

      {live && (
        <div className="panel__footer">
          <Kbd keys="M" />
          <span>marks the moment · a label can be added first, or left empty</span>
        </div>
      )}
    </div>
  )
}
