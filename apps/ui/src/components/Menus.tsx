// The title bar's menus: arrangements, panels, and help.

import { useCallback, useEffect, useState } from 'react'
import type { DockviewApi } from 'dockview'
import { MenuItem, Modal, Popover } from './primitives'
import { IconCheck, IconLayout, IconSave, IconTrash } from './Icons'
import { PANELS } from '../panels/registry'
import {
  applyNamedLayout,
  applyPreset,
  clearLayout,
  deleteNamedLayout,
  readActivePreset,
  readSavedLayouts,
  saveNamedLayout,
  showPanel,
  type SavedLayout,
} from '../workspace/layout'
import { LAYOUT_PRESETS } from '../workspace/presets'
import { useStore } from '../state/store'

/** The drawn schematic of a preset. */
function Sketch({ rows, accent }: { rows: string[]; accent: string[] }) {
  const letters = [...new Set(rows.join(' ').split(/\s+/).filter(Boolean))]
  const columns = rows[0]?.trim().split(/\s+/).length ?? 1
  return (
    <div
      className="layout-card__sketch"
      style={{
        gridTemplateAreas: rows.map((row) => `"${row}"`).join(' '),
        gridTemplateColumns: `repeat(${columns}, 1fr)`,
        gridTemplateRows: `repeat(${rows.length}, 1fr)`,
      }}
      aria-hidden="true"
    >
      {letters.map((letter) => (
        <span
          key={letter}
          className={`layout-card__cell${accent.includes(letter) ? ' layout-card__cell--accent' : ''}`}
          style={{ gridArea: letter }}
        />
      ))}
    </div>
  )
}

interface LayoutMenuProps {
  api: DockviewApi | null
}

export function LayoutMenu({ api }: LayoutMenuProps) {
  const notify = useStore((s) => s.notify)
  const appendLog = useStore((s) => s.appendLog)
  const [saved, setSaved] = useState<SavedLayout[]>(() => readSavedLayouts())
  const [active, setActive] = useState<string>(() => readActivePreset())
  const [naming, setNaming] = useState(false)
  const [name, setName] = useState('')

  const choosePreset = useCallback(
    (id: string, label: string, close: () => void) => {
      if (!api) return
      applyPreset(api, id)
      setActive(id)
      appendLog('info', `Layout set to ${label}.`)
      close()
    },
    [api, appendLog],
  )

  const commitSave = () => {
    if (!api) return
    const trimmed = name.trim()
    if (trimmed === '') return
    setSaved(saveNamedLayout(api, trimmed))
    setActive(`saved:${trimmed}`)
    notify('ok', 'Layout saved', `Kept as “${trimmed}”.`)
    setNaming(false)
    setName('')
  }

  return (
    <>
      <Popover
        className="popover--wide"
        trigger={({ open, toggle }) => (
          <button
            type="button"
            className={`btn btn--ghost btn--sm${open ? ' btn--accent-quiet' : ''}`}
            aria-expanded={open}
            onClick={toggle}
            title="Workspace arrangements"
          >
            <IconLayout size={13} /> Layout
          </button>
        )}
      >
        {({ close }) => (
          <>
            <div className="popover__label">Arrangements</div>
            <div className="layout-grid">
              {LAYOUT_PRESETS.map((preset) => (
                <button
                  key={preset.id}
                  type="button"
                  className="layout-card"
                  aria-pressed={active === preset.id}
                  title={preset.description}
                  onClick={() => choosePreset(preset.id, preset.name, close)}
                >
                  <Sketch rows={preset.sketch} accent={preset.accent} />
                  <span className="layout-card__name">{preset.name}</span>
                  <span className="layout-card__desc">{preset.description}</span>
                </button>
              ))}
            </div>

            {saved.length > 0 && (
              <>
                <div className="popover__separator" />
                <div className="popover__label">Yours</div>
                {saved.map((entry) => (
                  <div className="rowline" key={entry.name} style={{ gap: 2 }}>
                    <MenuItem
                      current={active === `saved:${entry.name}`}
                      detail={new Date(entry.savedAt).toLocaleString()}
                      onClick={() => {
                        if (!api) return
                        if (applyNamedLayout(api, entry.name)) {
                          setActive(`saved:${entry.name}`)
                          appendLog('info', `Layout set to “${entry.name}”.`)
                        }
                        close()
                      }}
                    >
                      {entry.name}
                    </MenuItem>
                    <button
                      type="button"
                      className="btn btn--danger-ghost btn--sm btn--icon"
                      title={`Forget “${entry.name}”`}
                      onClick={() => setSaved(deleteNamedLayout(entry.name))}
                    >
                      <IconTrash size={12} />
                    </button>
                  </div>
                ))}
              </>
            )}

            <div className="popover__separator" />
            <MenuItem
              icon={IconSave}
              disabled={!api}
              onClick={() => {
                setNaming(true)
                close()
              }}
            >
              Save this arrangement…
            </MenuItem>
            <MenuItem
              icon={IconLayout}
              disabled={!api}
              onClick={() => {
                if (!api) return
                clearLayout()
                applyPreset(api, 'console')
                setActive('console')
                appendLog('info', 'Layout reset to the default arrangement.')
                close()
              }}
            >
              Reset to default
            </MenuItem>
          </>
        )}
      </Popover>

      {naming && (
        <Modal
          title="Save this arrangement"
          onClose={() => setNaming(false)}
          actions={
            <>
              <button type="button" className="btn" onClick={() => setNaming(false)}>
                Cancel
              </button>
              <button
                type="button"
                className="btn btn--primary"
                disabled={name.trim() === ''}
                onClick={commitSave}
              >
                Save
              </button>
            </>
          }
        >
          <label className="field">
            <span className="field__label">Name</span>
            <input
              className="input"
              autoFocus
              value={name}
              placeholder="Long call, second screen"
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') commitSave()
              }}
            />
            <span className="field__hint">
              The current arrangement, including which panels are floating and
              which are tabbed together, is kept under this name.
            </span>
          </label>
        </Modal>
      )}
    </>
  )
}

interface PanelsMenuProps {
  api: DockviewApi | null
}

/**
 * The list of panels, with the open ones marked.
 *
 * Panels are closeable, so this is how one comes back. Without it the only
 * route to a panel someone had closed would be resetting the whole layout,
 * which would cost them the rest of their arrangement.
 */
export function PanelsMenu({ api }: PanelsMenuProps) {
  const [open, setOpen] = useState<string[]>([])

  // Re-read on every open rather than tracking the layout: the list is short,
  // it is read at the moment the menu appears, and subscribing to layout
  // changes to keep a menu that is closed up to date is work for nothing.
  const refresh = useCallback(() => {
    setOpen(api ? api.panels.map((p) => p.id) : [])
  }, [api])

  useEffect(refresh, [refresh])

  return (
    <Popover
      trigger={({ open: isOpen, toggle }) => (
        <button
          type="button"
          className={`btn btn--ghost btn--sm${isOpen ? ' btn--accent-quiet' : ''}`}
          aria-expanded={isOpen}
          onClick={() => {
            refresh()
            toggle()
          }}
          title="Show a panel"
        >
          Panels
        </button>
      )}
    >
      {({ close }) => (
        <>
          <div className="popover__label">Panels</div>
          {PANELS.map((panel) => (
            <MenuItem
              key={panel.id}
              icon={panel.icon}
              detail={panel.description}
              current={open.includes(panel.id)}
              hint={open.includes(panel.id) ? <IconCheck size={12} /> : undefined}
              disabled={!api}
              onClick={() => {
                if (api) showPanel(api, panel.id)
                close()
              }}
            >
              {panel.title}
            </MenuItem>
          ))}
        </>
      )}
    </Popover>
  )
}
