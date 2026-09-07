// The endpoints: what plays, what listens, and what each one's stream will
// actually contain.
//
// Discovery handles applications. This panel handles the hardware, and its job
// beyond offering a device is to say what recording it would produce. A
// playback endpoint dictates its own format — a shared-mode loopback capture
// has to use the endpoint's mix format — so the sample rate, the sample type
// and the channel layout shown here are not settings but facts about the file
// that will come out.

import { useMemo, useState } from 'react'
import { Empty, Notice, Section } from '../components/primitives'
import {
  endpointIcon,
  IconApp,
  IconChevronDown,
  IconChevronRight,
  IconInfo,
  IconMic,
  IconPlus,
  IconRefresh,
  IconSearch,
  IconSpeaker,
} from '../components/Icons'
import { kilohertz } from '../lib/format'
import type { AudioSessionInfo, DeviceInfo } from '../lib/types'

/** One shared empty list, so a missing snapshot does not look like a change. */
const NO_SESSIONS: AudioSessionInfo[] = []
import { useStore } from '../state/store'

/** One endpoint, with its channel layout available underneath. */
function DeviceRow({ device }: { device: DeviceInfo }) {
  const [open, setOpen] = useState(false)
  const addSource = useStore((s) => s.addSource)
  const planned = useStore((s) => s.planned)
  const sessions = useStore((s) => s.discovery)?.sessions ?? NO_SESSIONS

  const isRender = device.kind === 'render'
  // 'default' rather than the endpoint id when this is the default, so the
  // recording follows the user's default if they change it before starting.
  const selector = device.isDefault ? 'default' : device.id
  const already = planned.some(
    (p) =>
      p.request.kind === (isRender ? 'system' : 'input') &&
      'device' in p.request &&
      p.request.device === selector,
  )

  const active = sessions.filter(
    (s) => s.endpointId === device.id && s.state === 'active',
  ).length

  const Icon = endpointIcon(device.formFactor)
  const format = device.format

  return (
    <>
      <div className="row row--interactive">
        <button
          type="button"
          className="btn btn--ghost btn--sm btn--icon"
          aria-expanded={open}
          title={open ? 'Hide the detail' : 'Show the channel layout and identity'}
          onClick={() => setOpen((v) => !v)}
        >
          {open ? <IconChevronDown size={12} /> : <IconChevronRight size={12} />}
        </button>

        <span className={`row__icon${device.isDefault ? ' row__icon--accent' : ''}`}>
          <Icon size={14} />
        </span>

        <div className="row__main">
          <div className="row__title">
            <span>{device.name}</span>
            {device.isDefault && (
              <span className="badge badge--accent" title="The current Windows default">
                default
              </span>
            )}
            {active > 0 && (
              <span className="badge badge--ok" title={`${active} stream(s) active here now`}>
                {active} live
              </span>
            )}
          </div>
          <div className="row__subtitle">
            {format
              ? `${kilohertz(format.sampleRate)} · ${format.layout} · ${format.sampleFormat}`
              : 'format unavailable'}
            {device.formFactor ? ` · ${device.formFactor}` : ''}
          </div>
        </div>

        <div className="row__actions row__actions--always">
          <button
            type="button"
            className="btn btn--sm"
            disabled={already}
            title={
              already
                ? 'Already in this session'
                : isRender
                  ? 'Record everything played here, mixed onto one track'
                  : 'Record this input on a track of its own'
            }
            onClick={() =>
              addSource(
                isRender
                  ? { kind: 'system', device: selector, label: null }
                  : { kind: 'input', device: selector, label: null },
              )
            }
          >
            <IconPlus size={12} /> Add
          </button>
        </div>
      </div>

      {open && (
        <div className="row__expand">
          {format ? (
            <>
              <div className="rowline" style={{ marginBottom: 5 }}>
                <span style={{ minWidth: 96 }}>Channels</span>
                <span className="chip-list">
                  {format.channelsDetail.map((channel) => (
                    <span className="chip" key={channel.index} title={channel.name}>
                      {channel.index + 1} {channel.label}
                    </span>
                  ))}
                </span>
              </div>
              <div className="rowline">
                <span style={{ minWidth: 96 }}>Speaker mask</span>
                <span className="mono">
                  0x{format.channelMask.toString(16)} · {format.layout}
                </span>
              </div>
              <div className="rowline">
                <span style={{ minWidth: 96 }}>Samples</span>
                <span className="mono">
                  {format.sampleFormat}
                  {format.validBits !== format.bits ? ` (${format.validBits} valid)` : ''}
                </span>
              </div>
            </>
          ) : (
            <div style={{ marginBottom: 5 }}>
              The endpoint is present but its audio client would not open, so its
              format could not be read. A half-connected Bluetooth device sits in
              exactly this state.
            </div>
          )}
          {device.adapter && (
            <div className="rowline">
              <span style={{ minWidth: 96 }}>Adapter</span>
              <span className="truncate">{device.adapter}</span>
            </div>
          )}
          <div className="rowline">
            <span style={{ minWidth: 96 }}>Endpoint id</span>
            <span className="mono truncate selectable" title={device.id}>
              {device.id}
            </span>
          </div>
        </div>
      )}
    </>
  )
}

export function SourcesPanel() {
  const devices = useStore((s) => s.devices)
  const refresh = useStore((s) => s.refreshInventory)
  const rescan = useStore((s) => s.rescan)
  const addSource = useStore((s) => s.addSource)
  const [filter, setFilter] = useState('')
  const [manual, setManual] = useState('')

  const needle = filter.trim().toLowerCase()
  const match = (d: DeviceInfo) =>
    needle === '' ||
    d.name.toLowerCase().includes(needle) ||
    (d.adapter ?? '').toLowerCase().includes(needle)

  const render = useMemo(
    () => devices.filter((d) => d.kind === 'render' && match(d)),
    [devices, needle],
  )
  const capture = useMemo(
    () => devices.filter((d) => d.kind === 'capture' && match(d)),
    [devices, needle],
  )

  const addManual = () => {
    const target = manual.trim()
    if (target === '') return
    addSource({ kind: 'app', target, exclude: false, label: null })
    setManual('')
  }

  return (
    <div className="panel">
      <div className="panel__toolbar">
        <div className="search">
          <span className="search__icon">
            <IconSearch size={13} />
          </span>
          <input
            className="input input--search"
            placeholder="Filter endpoints"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
        </div>
        <button
          type="button"
          className="btn btn--sm btn--icon"
          title="Re-enumerate the endpoints and rescan for audio"
          onClick={() => {
            void refresh()
            void rescan()
          }}
        >
          <IconRefresh size={13} />
        </button>
      </div>

      <div className="panel__body">
        {devices.length === 0 && (
          <Empty icon={IconSpeaker} title="No endpoints found">
            Windows reported no active audio endpoints. That is unusual enough to
            be worth checking in the sound control panel.
          </Empty>
        )}

        {render.length > 0 && (
          <>
            <Section count={render.length}>Playback · everything, mixed</Section>
            <Notice tone="accent" icon={IconInfo}>
              A loopback tap on a playback endpoint catches every sound that
              reaches it — Citrix, WSL, browsers and native applications alike.
              It is the safety net: add one even when the interesting sources are
              also on tracks of their own.
            </Notice>
            {render.map((device) => (
              <DeviceRow key={device.id} device={device} />
            ))}
          </>
        )}

        {capture.length > 0 && (
          <>
            <Section count={capture.length}>Inputs · your own voice</Section>
            {capture.map((device) => (
              <DeviceRow key={device.id} device={device} />
            ))}
          </>
        )}

        <Section>By name</Section>
        <div className="row">
          <span className="row__icon">
            <IconApp size={14} />
          </span>
          <input
            className="input"
            style={{ flex: 1 }}
            placeholder="Any process name or pid"
            value={manual}
            onChange={(e) => setManual(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') addManual()
            }}
          />
          <button
            type="button"
            className="btn btn--sm"
            disabled={manual.trim() === ''}
            onClick={addManual}
          >
            <IconPlus size={12} /> Add
          </button>
        </div>
        <Notice icon={IconMic}>
          Anything can be recorded by name, whether it is playing yet or not.
          Naming a process that has not started is how you arm a track before a
          call begins: the recorder resolves the name when the session starts.
        </Notice>
      </div>
    </div>
  )
}
