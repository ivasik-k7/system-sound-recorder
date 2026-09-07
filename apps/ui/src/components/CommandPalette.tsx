// The command palette.
//
// Twelve panels, six arrangements and a transport is more than a menu bar can
// hold without becoming a maze, and it is exactly what a palette is for: type
// three letters of what you want and press Enter. It is also the only place
// every action in the program is listed together, which makes it the honest
// answer to "what can this thing do?".

import { useEffect, useMemo, useRef, useState, type ComponentType } from 'react'
import type { DockviewApi } from 'dockview'
import { Kbd } from './primitives'
import {
  IconFlag,
  IconFolder,
  IconLayout,
  IconRadar,
  IconRecord,
  IconRefresh,
  IconStop,
  type IconProps,
} from './Icons'
import { api as backend } from '../lib/api'
import { PANELS } from '../panels/registry'
import { LAYOUT_PRESETS } from '../workspace/presets'
import { applyPreset, showPanel } from '../workspace/layout'
import { isRecording, useStore } from '../state/store'

interface Command {
  id: string
  group: string
  title: string
  detail?: string
  icon: ComponentType<IconProps>
  keys?: string
  disabled?: boolean
  run: () => void
}

interface Props {
  api: DockviewApi | null
  onClose: () => void
}

export function CommandPalette({ api, onClose }: Props) {
  const [query, setQuery] = useState('')
  const [index, setIndex] = useState(0)
  const listRef = useRef<HTMLDivElement>(null)

  // Individual selectors, not the whole store: with the whole store this
  // rebuilt its command list on every meter tick while it was open.
  const snapshot = useStore((s) => s.snapshot)
  const planned = useStore((s) => s.planned)
  const sessionDir = useStore((s) => s.sessionDir)
  const outputDir = useStore((s) => s.settings.outputDir)
  const start = useStore((s) => s.start)
  const stop = useStore((s) => s.stop)
  const addMarker = useStore((s) => s.addMarker)
  const rescan = useStore((s) => s.rescan)
  const loadLibrary = useStore((s) => s.loadLibrary)
  const live = isRecording(snapshot)

  const commands = useMemo<Command[]>(() => {
    const list: Command[] = []

    list.push(
      live
        ? {
            id: 'stop',
            group: 'Transport',
            title: 'Stop recording',
            detail: 'Write the manifest and close every file',
            icon: IconStop,
            keys: 'Space',
            run: () => void stop(),
          }
        : {
            id: 'start',
            group: 'Transport',
            title: 'Start recording',
            detail:
              planned.length === 0
                ? 'Add a source first'
                : `${planned.length} track(s) armed`,
            icon: IconRecord,
            keys: 'Space',
            disabled: planned.length === 0,
            run: () => void start(),
          },
    )

    list.push({
      id: 'marker',
      group: 'Transport',
      title: 'Mark this instant',
      detail: 'Written into the session manifest',
      icon: IconFlag,
      keys: 'M',
      disabled: !live,
      run: () => void addMarker(''),
    })

    list.push({
      id: 'rescan',
      group: 'Discovery',
      title: 'Rescan for audio',
      detail: 'Rebuild the picture now rather than at the next pass',
      icon: IconRadar,
      keys: 'Ctrl+D',
      run: () => void rescan(),
    })

    list.push({
      id: 'reveal',
      group: 'Files',
      title: 'Open the recordings folder',
      detail: sessionDir ?? outputDir,
      icon: IconFolder,
      keys: 'Ctrl+O',
      run: () => void backend.revealPath(sessionDir ?? outputDir),
    })

    list.push({
      id: 'library',
      group: 'Files',
      title: 'Reload the library',
      detail: 'Re-read the recordings folder from disk',
      icon: IconRefresh,
      run: () => void loadLibrary(),
    })

    for (const panel of PANELS) {
      list.push({
        id: `panel:${panel.id}`,
        group: 'Panels',
        title: `Show ${panel.title}`,
        detail: panel.description,
        icon: panel.icon,
        disabled: !api,
        run: () => {
          if (api) showPanel(api, panel.id)
        },
      })
    }

    for (const preset of LAYOUT_PRESETS) {
      list.push({
        id: `layout:${preset.id}`,
        group: 'Layout',
        title: `Layout: ${preset.name}`,
        detail: preset.description,
        icon: IconLayout,
        disabled: !api,
        run: () => {
          if (api) applyPreset(api, preset.id)
        },
      })
    }

    return list
  }, [
    api,
    live,
    planned,
    sessionDir,
    outputDir,
    start,
    stop,
    addMarker,
    rescan,
    loadLibrary,
  ])

  const needle = query.trim().toLowerCase()
  const filtered = useMemo(() => {
    if (needle === '') return commands
    // Every word has to appear somewhere, in any order, so "lay mon" finds
    // the monitor layout. Substring rather than fuzzy: fuzzy matching on a
    // list this size mostly produces surprises.
    const words = needle.split(/\s+/)
    return commands.filter((command) => {
      const haystack = `${command.group} ${command.title} ${command.detail ?? ''}`.toLowerCase()
      return words.every((word) => haystack.includes(word))
    })
  }, [commands, needle])

  useEffect(() => setIndex(0), [needle])

  // Keep the highlighted row on screen while arrowing through a long list.
  useEffect(() => {
    const active = listRef.current?.querySelector('[data-active="true"]')
    active?.scrollIntoView({ block: 'nearest' })
  }, [index])

  const runAt = (i: number) => {
    const command = filtered[i]
    if (!command || command.disabled) return
    command.run()
    onClose()
  }

  let lastGroup = ''

  return (
    <div
      className="overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
    >
      <div className="palette" role="dialog" aria-modal="true" aria-label="Commands">
        <input
          className="palette__input"
          autoFocus
          placeholder="Type a command, a panel or a layout…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Escape') {
              onClose()
            } else if (e.key === 'ArrowDown') {
              e.preventDefault()
              setIndex((i) => Math.min(i + 1, filtered.length - 1))
            } else if (e.key === 'ArrowUp') {
              e.preventDefault()
              setIndex((i) => Math.max(i - 1, 0))
            } else if (e.key === 'Enter') {
              e.preventDefault()
              runAt(index)
            }
          }}
        />

        <div className="palette__list" ref={listRef}>
          {filtered.length === 0 && (
            <div className="palette__group">Nothing matches “{query}”</div>
          )}
          {filtered.map((command, i) => {
            const header = command.group !== lastGroup ? command.group : null
            lastGroup = command.group
            const Icon = command.icon
            return (
              <div key={command.id}>
                {header && <div className="palette__group">{header}</div>}
                <button
                  type="button"
                  className="palette__item"
                  data-active={i === index}
                  disabled={command.disabled}
                  onMouseMove={() => setIndex(i)}
                  onClick={() => runAt(i)}
                >
                  <span className="palette__icon">
                    <Icon size={14} />
                  </span>
                  <span className="palette__body">
                    <span className="palette__title">{command.title}</span>
                    {command.detail && <span className="palette__sub">{command.detail}</span>}
                  </span>
                  {command.keys && <Kbd keys={command.keys} />}
                </button>
              </div>
            )
          })}
        </div>

        <div className="palette__footer">
          <span>
            <Kbd keys="↑" /> <Kbd keys="↓" /> to move
          </span>
          <span>
            <Kbd keys="Enter" /> to run
          </span>
          <span>
            <Kbd keys="Esc" /> to close
          </span>
          <span className="spacer" />
          <span>{filtered.length} of {commands.length}</span>
        </div>
      </div>
    </div>
  )
}
