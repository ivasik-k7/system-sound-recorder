// Everything the session has said about itself, newest at the bottom.

import { useEffect, useMemo, useRef, useState } from 'react'
import { Empty, Segmented } from '../components/primitives'
import { IconList, IconSearch } from '../components/Icons'
import type { LogLevel } from '../lib/types'
import { useStore } from '../state/store'

type Level = LogLevel | 'all'

const LEVELS = [
  { value: 'all' as Level, label: 'All' },
  { value: 'info' as Level, label: 'Info' },
  { value: 'warn' as Level, label: 'Warnings' },
  { value: 'error' as Level, label: 'Errors' },
]

export function LogPanel() {
  const log = useStore((s) => s.log)
  const clearLog = useStore((s) => s.clearLog)
  const [level, setLevel] = useState<Level>('all')
  const [filter, setFilter] = useState('')
  const bottom = useRef<HTMLDivElement>(null)
  const body = useRef<HTMLDivElement>(null)
  const [pinned, setPinned] = useState(true)

  const needle = filter.trim().toLowerCase()
  const visible = useMemo(
    () =>
      log.filter(
        (entry) =>
          (level === 'all' || entry.level === level) &&
          (needle === '' || entry.message.toLowerCase().includes(needle)),
      ),
    [log, level, needle],
  )

  // Follow the tail, but stop following the moment someone scrolls up to read
  // something. A log that yanks itself back to the bottom mid-sentence is
  // worse than one that does not scroll at all.
  useEffect(() => {
    if (pinned) bottom.current?.scrollIntoView({ block: 'end' })
  }, [visible.length, pinned])

  const counts = useMemo(
    () => ({
      warn: log.filter((e) => e.level === 'warn').length,
      error: log.filter((e) => e.level === 'error').length,
    }),
    [log],
  )

  return (
    <div className="panel">
      <div className="panel__toolbar panel__toolbar--wrap">
        <Segmented value={level} options={LEVELS} onChange={setLevel} ariaLabel="Log level" />
        <div className="search">
          <span className="search__icon">
            <IconSearch size={13} />
          </span>
          <input
            className="input input--search"
            placeholder="Search the log"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
        </div>
        <button
          type="button"
          className="btn btn--sm"
          disabled={log.length === 0}
          onClick={clearLog}
        >
          Clear
        </button>
      </div>

      <div
        className="panel__body log"
        ref={body}
        onScroll={(e) => {
          const el = e.currentTarget
          setPinned(el.scrollHeight - el.scrollTop - el.clientHeight < 24)
        }}
      >
        {visible.length === 0 ? (
          <Empty icon={IconList} title={log.length === 0 ? 'Nothing logged yet' : 'Nothing matches'}>
            {log.length === 0
              ? 'What the recorder reports about itself appears here, including anything it had to work around.'
              : undefined}
          </Empty>
        ) : (
          visible.map((entry) => (
            <div className={`log__line log__line--${entry.level}`} key={entry.id}>
              <span className="log__time">{entry.at}</span>
              <span>{entry.message}</span>
            </div>
          ))
        )}
        <div ref={bottom} />
      </div>

      <div className="panel__footer">
        <span>{visible.length === log.length ? `${log.length} lines` : `${visible.length} of ${log.length}`}</span>
        {counts.warn > 0 && <span className="badge badge--warn">{counts.warn} warnings</span>}
        {counts.error > 0 && <span className="badge badge--danger">{counts.error} errors</span>}
        <span className="spacer" />
        {!pinned && (
          <button
            type="button"
            className="btn btn--ghost btn--sm"
            onClick={() => {
              setPinned(true)
              bottom.current?.scrollIntoView({ block: 'end' })
            }}
          >
            Jump to newest
          </button>
        )}
      </div>
    </div>
  )
}
