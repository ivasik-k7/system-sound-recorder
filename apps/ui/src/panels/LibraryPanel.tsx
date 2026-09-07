// Recordings already made.
//
// Sessions accumulate as dated folders, which is a good shape to write and a
// poor one to browse. What this panel adds beyond a file listing is the two
// judgements a folder cannot make for itself.
//
// The first is whether the session finished. A folder with WAV files but no
// manifest was interrupted: the audio is usually still playable, but nothing
// recorded what the tracks were or that they were meant to line up. Listing
// that quietly alongside the complete ones would hide the most important thing
// about it.
//
// The second is how much of each track is real. A track that is nine tenths
// manufactured silence recorded almost nothing, however plausible its length
// looks in a file browser.

import { useEffect, useState } from 'react'
import { Empty, Notice, Section } from '../components/primitives'
import {
  IconChevronDown,
  IconChevronRight,
  IconFlag,
  IconFolder,
  IconLibrary,
  IconRefresh,
  IconWarning,
} from '../components/Icons'
import { api } from '../lib/api'
import {
  bytes,
  duration,
  durationWords,
  fraction,
  kilohertz,
  localTime,
  plural,
  relativeTime,
} from '../lib/format'
import type { LibrarySession } from '../lib/types'
import { useStore } from '../state/store'

function SessionRow({ session }: { session: LibrarySession }) {
  const [open, setOpen] = useState(false)
  const suspect = session.tracks.some(
    (t) => (t.silenceFraction ?? 0) > 0.8 || t.discontinuities > 0,
  )

  return (
    <>
      <div className="row row--interactive">
        <button
          type="button"
          className="btn btn--ghost btn--sm btn--icon"
          aria-expanded={open}
          title={open ? 'Hide the tracks' : 'Show the tracks'}
          onClick={() => setOpen((v) => !v)}
        >
          {open ? <IconChevronDown size={12} /> : <IconChevronRight size={12} />}
        </button>

        <span className={`row__icon${session.complete ? '' : ' row__icon--accent'}`}>
          {session.complete ? <IconLibrary size={13} /> : <IconWarning size={13} />}
        </span>

        <div className="row__main">
          <div className="row__title">
            {/* An interrupted session has no manifest and therefore no start
                time, so it is named by its folder — which is a timestamp
                anyway, since that is how the recorder names them. */}
            <span>{session.startedUtc ? localTime(session.startedUtc) : session.name}</span>
            {!session.complete && (
              <span className="badge badge--warn" title={session.problem ?? undefined}>
                interrupted
              </span>
            )}
            {suspect && session.complete && (
              <span className="badge badge--quiet" title="One or more tracks had dropouts or were mostly silence">
                check
              </span>
            )}
            {(session.markers?.length ?? 0) > 0 && (
              <span className="badge badge--accent">
                <IconFlag size={10} /> {session.markers?.length}
              </span>
            )}
          </div>
          <div className="row__subtitle">
            {session.durationSecs != null
              ? durationWords(session.durationSecs)
              : 'length unknown'}{' '}
            · {plural(session.tracks.length, 'track')} · {bytes(session.sizeBytes)}
            {' · '}
            {relativeTime(session.startedUtc, session.modifiedUnix)}
          </div>
        </div>

        <div className="row__actions row__actions--always">
          <button
            type="button"
            className="btn btn--sm btn--icon"
            title={`Open ${session.dir}`}
            onClick={() => void api.revealPath(session.dir)}
          >
            <IconFolder size={13} />
          </button>
        </div>
      </div>

      {open && (
        <div className="row__expand">
          <div className="rowline" style={{ marginBottom: 6 }}>
            <span className="mono truncate selectable faint" title={session.dir}>
              {session.dir}
            </span>
          </div>

          {session.tracks.map((track) => (
            <div className="rowline" key={track.file} style={{ padding: '2px 0' }}>
              <span className="mono truncate" style={{ minWidth: 110, flex: 1 }}>
                {track.name}
              </span>
              <span className="badge badge--quiet badge--mono">{bytes(track.sizeBytes)}</span>
              {track.sampleRate > 0 && (
                <span className="badge badge--quiet badge--mono">
                  {kilohertz(track.sampleRate)} · {track.channels} ch
                </span>
              )}
              {track.durationSecs != null && (
                <span className="badge badge--quiet badge--mono">
                  {duration(track.durationSecs)}
                </span>
              )}
              {track.silenceFraction != null && (
                <span
                  className={`badge badge--mono ${
                    track.silenceFraction > 0.8 ? 'badge--warn' : 'badge--quiet'
                  }`}
                  title={`${fraction(track.silenceFraction)} of this track is manufactured silence`}
                >
                  {fraction(1 - track.silenceFraction)} audio
                </span>
              )}
              {track.discontinuities > 0 && (
                <span className="badge badge--danger">{track.discontinuities} drop</span>
              )}
            </div>
          ))}

          {session.markers && session.markers.length > 0 && (
            <div className="rowline" style={{ marginTop: 6, flexWrap: 'wrap' }}>
              {session.markers.map((marker, i) => (
                <span className="chip" key={i} title={marker.label}>
                  {duration(marker.at_secs)} {marker.label}
                </span>
              ))}
            </div>
          )}

          {session.problem && <Notice tone="warn">{session.problem}</Notice>}
          {(session.warnings ?? []).map((warning) => (
            <Notice tone="warn" key={warning}>
              {warning}
            </Notice>
          ))}
        </div>
      )}
    </>
  )
}

export function LibraryPanel() {
  const library = useStore((s) => s.library)
  const loading = useStore((s) => s.libraryLoading)
  const loadLibrary = useStore((s) => s.loadLibrary)
  const outputDir = useStore((s) => s.settings.outputDir)

  // Read the folder when the panel appears, and again if the folder changes.
  //
  // Keyed on the folder rather than on first mount: at mount the recordings
  // folder is often still being resolved from the backend, and an effect that
  // ran once would find no folder, do nothing, and leave the panel
  // permanently empty. Debounced because the folder is an editable field, and
  // without it every keystroke in the Session panel would re-read the disk.
  useEffect(() => {
    if (!outputDir) return
    const handle = window.setTimeout(() => void loadLibrary(), 350)
    return () => window.clearTimeout(handle)
  }, [outputDir, loadLibrary])

  const total = library.reduce((sum, s) => sum + s.sizeBytes, 0)
  const incomplete = library.filter((s) => !s.complete).length

  return (
    <div className="panel">
      <div className="panel__toolbar">
        <span className="badge">{plural(library.length, 'session')}</span>
        {incomplete > 0 && (
          <span className="badge badge--warn">{incomplete} interrupted</span>
        )}
        <span className="spacer" />
        <button
          type="button"
          className="btn btn--sm btn--icon"
          title="Re-read the recordings folder"
          onClick={() => void loadLibrary()}
        >
          <IconRefresh size={13} />
        </button>
        <button
          type="button"
          className="btn btn--sm btn--icon"
          title={`Open ${outputDir}`}
          onClick={() => void api.revealPath(outputDir)}
        >
          <IconFolder size={13} />
        </button>
      </div>

      <div className="panel__body">
        {loading && library.length === 0 ? (
          <Empty icon={IconLibrary} title="Reading the recordings folder" />
        ) : library.length === 0 ? (
          <Empty icon={IconLibrary} title="No recordings yet">
            Finished sessions appear here, newest first, each with what it
            captured and how much of it was real audio rather than manufactured
            silence.
          </Empty>
        ) : (
          <>
            <Section count={library.length}>Sessions · newest first</Section>
            {library.map((session) => (
              <SessionRow key={session.dir} session={session} />
            ))}
          </>
        )}
      </div>

      {library.length > 0 && (
        <div className="panel__footer">
          <span>{bytes(total)} in this folder</span>
          <span className="spacer" />
          <span className="mono truncate faint" title={outputDir}>
            {outputDir}
          </span>
        </div>
      )}
    </div>
  )
}
