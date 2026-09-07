// Autodiscovery: what is making sound on this machine, right now.
//
// This panel is the answer to the question that comes before every recording,
// and the reason the engine asks the Windows audio engine directly rather than
// matching a list of executable names. Anything with an audio session appears
// here, catalogued or not, with a live level — so a browser tab nobody thought
// about is as findable as Citrix, and a Citrix client that is running but
// silent is visibly silent rather than being offered as though it were live.

import { useMemo, useState } from 'react'
import { MiniMeter } from '../components/Meter'
import { Empty, Notice, Section, Switch } from '../components/primitives'
import {
  categoryIcon,
  IconCheck,
  IconPlus,
  IconRadar,
  IconRefresh,
  IconSearch,
  IconChevronDown,
  IconChevronRight,
} from '../components/Icons'
import { decibels, pidSummary, plural } from '../lib/format'
import { emptyMeter } from '../lib/meters'
import type { AudioSessionInfo, DiscoveredSource, SourceCategory } from '../lib/types'
import { useStore } from '../state/store'

/** One shared empty list.
 *
 *  A selector that returns a fresh `[]` when the data is not there yet hands
 *  React a new snapshot on every store change, whether or not anything this
 *  component cares about moved. */
const NO_SESSIONS: AudioSessionInfo[] = []

/** Categories in the order they are worth looking at.
 *
 *  Remote sessions first, always. Their audio exists nowhere else on the
 *  machine: if one is playing, it is almost certainly what the user came for. */
const CATEGORY_ORDER: SourceCategory[] = [
  'remote',
  'conferencing',
  'browser',
  'chat',
  'media',
  'audio',
  'game',
  'other',
  'system',
]

const CATEGORY_NAMES: Record<SourceCategory, string> = {
  remote: 'Remote sessions',
  conferencing: 'Meetings and calls',
  browser: 'Browsers',
  chat: 'Chat',
  media: 'Media players',
  audio: 'Audio applications',
  game: 'Games',
  system: 'System',
  other: 'Other applications',
}

/** One discovered application. */
function SourceRow({ source }: { source: DiscoveredSource }) {
  const [open, setOpen] = useState(false)
  const addDiscovered = useStore((s) => s.addDiscovered)
  const planned = useStore((s) => s.planned)
  const sessionMeters = useStore((s) => s.sessionMeters)
  const floor = useStore((s) => s.prefs.meterFloor)
  const sessions = useStore((s) => s.discovery)?.sessions ?? NO_SESSIONS

  const mine = useMemo(
    () => sessions.filter((s) => s.exe.toLowerCase() === source.exe.toLowerCase()),
    [sessions, source.exe],
  )

  const alreadyPlanned = planned.some(
    (p) => p.request.kind === 'app' && p.request.target.toLowerCase() === source.exe.toLowerCase(),
  )

  // The loudest of this application's sessions carries the row's meter, since
  // the row stands for the process tree as a whole.
  const meter = useMemo(() => {
    let loudest = emptyMeter()
    for (const session of mine) {
      const m = sessionMeters[session.key]
      if (m && m.peakDb > loudest.peakDb) loudest = m
    }
    return loudest
  }, [mine, sessionMeters])

  const Icon = categoryIcon(source.category)
  const iconClass = source.playing
    ? 'row__icon row__icon--live'
    : source.recommended
      ? 'row__icon row__icon--accent'
      : 'row__icon'

  return (
    <>
      <div
        className={`row row--interactive${source.playing ? ' row--live' : ''}${
          !source.playing && source.sessions === 0 ? ' row--muted-source' : ''
        }`}
      >
        <button
          type="button"
          className="btn btn--ghost btn--sm btn--icon"
          aria-expanded={open}
          title={open ? 'Hide the streams' : `Show this application's ${plural(mine.length, 'stream')}`}
          disabled={mine.length === 0}
          onClick={() => setOpen((v) => !v)}
        >
          {open ? <IconChevronDown size={13} /> : <IconChevronRight size={13} />}
        </button>

        <span className={iconClass}>
          <Icon size={14} />
        </span>

        {/* Two lines, and the level lives on the second one. A meter and a
            decibel reading on the title line take sixty pixels from the name,
            which is the one thing in the row that cannot be abbreviated. */}
        <div className="row__main">
          <div className="row__title">
            <span>{source.label}</span>
            {source.recommended && (
              <span
                className="badge badge--accent"
                title="A remote session. Its audio passes through this machine and exists nowhere else, so this is the source the tool was built to separate."
              >
                remote
              </span>
            )}
            {source.capturing && (
              <span className="badge badge--warn" title="This application is also using a microphone.">
                mic
              </span>
            )}
          </div>
          <div className="rowline" style={{ gap: 7 }}>
            <span className="row__subtitle" style={{ flex: 1 }}>
              {source.exe} · {pidSummary(source.pids)}
              {source.endpoints.length > 0 ? ` · ${source.endpoints.join(', ')}` : ''}
            </span>
            <MiniMeter
              meter={meter}
              floor={floor}
              width={72}
              title={`${source.label}: ${decibels(meter.peakDb)} dB`}
            />
            <span
              className="mono faint text-xs"
              style={{ width: 32, textAlign: 'right', flex: 'none' }}
            >
              {decibels(meter.peakDb, 0)}
            </span>
          </div>
        </div>

        <div className="row__actions row__actions--always">
          {/* Whether a source is already in the session is the button's
              business, not a badge's: it is the same fact, and as a badge it
              competes for width with the name. */}
          <button
            type="button"
            className={`btn btn--sm${source.playing && !alreadyPlanned ? ' btn--accent-quiet' : ''}`}
            disabled={alreadyPlanned}
            title={
              alreadyPlanned
                ? 'Already in this session'
                : `Record ${source.label} on a track of its own`
            }
            onClick={() => addDiscovered(source)}
          >
            {alreadyPlanned ? <IconCheck size={12} /> : <IconPlus size={12} />}
            {alreadyPlanned ? 'Added' : 'Add'}
          </button>
        </div>
      </div>

      {open && mine.length > 0 && (
        <div className="row__expand">
          {mine.map((session) => (
            <SessionDetail key={session.key} session={session} />
          ))}
        </div>
      )}
    </>
  )
}

/** One stream of one application, with its channels.
 *
 *  A browser has a session per tab that makes noise, and a conferencing client
 *  has one playing and one listening. Rolling them into a single row is right
 *  for choosing what to record and wrong for understanding what is happening,
 *  so the detail is here. */
function SessionDetail({ session }: { session: AudioSessionInfo }) {
  const sessionMeters = useStore((s) => s.sessionMeters)
  const floor = useStore((s) => s.prefs.meterFloor)
  const devices = useStore((s) => s.devices)
  const meter = sessionMeters[session.key] ?? emptyMeter()

  const labels =
    devices
      .find((d) => d.id === session.endpointId)
      ?.format?.channelsDetail.map((c) => c.label) ?? []

  return (
    <div className="rowline" style={{ padding: '3px 0' }}>
      <span className="badge badge--quiet" style={{ minWidth: 62 }}>
        {session.endpointKind === 'render' ? 'playing to' : 'listening'}
      </span>
      <span className="truncate" style={{ flex: 1 }}>
        {session.displayName ?? session.endpointName}
      </span>
      {session.channelPeaks.length > 0 && (
        <span className="chip-list">
          {session.channelPeaks.map((peak, i) => (
            <span className="chip" key={i} title={labels[i] ?? `Channel ${i + 1}`}>
              {labels[i] ?? i + 1} {decibels(peak <= 0.000001 ? -120 : 20 * Math.log10(peak), 0)}
            </span>
          ))}
        </span>
      )}
      <MiniMeter meter={meter} floor={floor} width={44} />
      <span className="badge badge--quiet">{session.state}</span>
      {session.muted && <span className="badge badge--warn">muted</span>}
    </div>
  )
}

export function DiscoveryPanel() {
  const discovery = useStore((s) => s.discovery)
  const discovered = useStore((s) => s.discovered)
  const rescan = useStore((s) => s.rescan)
  const prefs = useStore((s) => s.prefs)
  const setPref = useStore((s) => s.setPref)
  const [filter, setFilter] = useState('')

  const needle = filter.trim().toLowerCase()

  const visible = useMemo(() => {
    const all = discovery?.sources ?? []
    return all.filter((source) => {
      if (prefs.playingOnly && !source.playing && !source.capturing) return false
      if (!prefs.showSystemSessions && source.category === 'system') return false
      if (needle === '') return true
      return (
        source.label.toLowerCase().includes(needle) ||
        source.exe.toLowerCase().includes(needle) ||
        source.pids.some((pid) => String(pid).includes(needle))
      )
    })
  }, [discovery, prefs.playingOnly, prefs.showSystemSessions, needle])

  const grouped = useMemo(() => {
    const map = new Map<SourceCategory, DiscoveredSource[]>()
    for (const source of visible) {
      const list = map.get(source.category) ?? []
      list.push(source)
      map.set(source.category, list)
    }
    return CATEGORY_ORDER.filter((c) => map.has(c)).map(
      (c) => [c, map.get(c) ?? []] as const,
    )
  }, [visible])

  const playing = (discovery?.sources ?? []).filter((s) => s.playing).length

  return (
    <div className="panel">
      <div className="panel__toolbar panel__toolbar--wrap">
        <div className="search">
          <span className="search__icon">
            <IconSearch size={13} />
          </span>
          <input
            className="input input--search"
            placeholder="Filter by name, executable or pid"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
        </div>
        <Switch
          checked={prefs.playingOnly}
          onChange={(v) => setPref('playingOnly', v)}
          label="Playing only"
          title="Hide applications that have audio open but are not making a sound"
        />
        <button
          type="button"
          className="btn btn--sm btn--icon"
          title="Rescan now, rather than waiting for the next automatic pass"
          onClick={() => void rescan()}
        >
          <IconRefresh size={13} />
        </button>
      </div>

      <div className="panel__body">
        {discovery?.error && <Notice tone="warn">{discovery.error}</Notice>}

        {!discovered ? (
          <Empty icon={IconRadar} title="Looking for audio">
            The recorder is asking the Windows audio engine which streams exist.
          </Empty>
        ) : visible.length === 0 ? (
          <Empty
            icon={IconRadar}
            title={prefs.playingOnly ? 'Nothing is playing' : 'Nothing found'}
            action={
              prefs.playingOnly ? (
                <button
                  type="button"
                  className="btn btn--sm"
                  onClick={() => setPref('playingOnly', false)}
                >
                  Show everything with audio open
                </button>
              ) : undefined
            }
          >
            {prefs.playingOnly
              ? 'No application is making a sound. Start something playing and it will appear here within a second.'
              : 'No application on this machine has an audio stream open. The system mix can still be recorded from the Sources panel.'}
          </Empty>
        ) : (
          grouped.map(([category, sources]) => (
            <div key={category}>
              <Section count={sources.length}>{CATEGORY_NAMES[category]}</Section>
              {sources.map((source) => (
                <SourceRow key={source.exe} source={source} />
              ))}
            </div>
          ))
        )}
      </div>

      <div className="panel__footer">
        <span>
          {playing > 0 ? `${playing} playing` : 'nothing playing'} ·{' '}
          {plural(discovery?.sources.length ?? 0, 'application')}
        </span>
        <span className="spacer" />
        <Switch
          checked={prefs.autoArmRemote}
          onChange={(v) => setPref('autoArmRemote', v)}
          label="Auto-arm remote"
          title="Add a remote session to the plan the moment one starts playing. The difference between catching the start of a call and missing it."
        />
      </div>
    </div>
  )
}
