// Layout persistence.
//
// The arrangement of panels is the user's, not the program's, so it survives
// restarts — and so do any arrangements they have chosen to name and keep. A
// stored layout from an older version may reference panels that no longer
// exist, which is why loading is guarded rather than trusted: falling back to
// a preset is far better than a workspace with holes in it.

import type { DockviewApi, SerializedDockview } from 'dockview'
import { PANELS_BY_ID } from '../panels/registry'
import { DEFAULT_PRESET, PRESETS_BY_ID } from './presets'

// Bump this whenever the set of panels or the default arrangement changes in a
// way that should reach people who have already used the program. A stored
// layout otherwise wins forever, and the improvement is only ever seen by new
// installations.
const ACTIVE_KEY = 'ssr.layout.v3'
const SAVED_KEY = 'ssr.layouts.v1'
const PRESET_KEY = 'ssr.layout.preset.v1'

/** An arrangement the user has named and kept. */
export interface SavedLayout {
  name: string
  /** When it was saved, as an ISO timestamp. */
  savedAt: string
  layout: SerializedDockview
}

// --------------------------------------------------------------- the active ---

export function saveLayout(api: DockviewApi): void {
  try {
    localStorage.setItem(ACTIVE_KEY, JSON.stringify(api.toJSON()))
  } catch {
    // A full or disabled local store is not worth interrupting anyone over.
  }
}

export function clearLayout(): void {
  try {
    localStorage.removeItem(ACTIVE_KEY)
  } catch {
    /* nothing to do */
  }
}

/** Read a stored layout, or null when there is nothing usable. */
export function readLayout(): SerializedDockview | null {
  return readFrom(ACTIVE_KEY)
}

function readFrom(key: string): SerializedDockview | null {
  let raw: string | null = null
  try {
    raw = localStorage.getItem(key)
  } catch {
    return null
  }
  if (!raw) return null
  try {
    return validate(JSON.parse(raw) as SerializedDockview)
  } catch {
    return null
  }
}

/**
 * Accept a layout only if every panel in it still exists.
 *
 * A build that has renamed or removed a panel would otherwise restore a
 * workspace with an empty group in it, which cannot be recovered from without
 * knowing to reset the layout.
 */
function validate(parsed: SerializedDockview): SerializedDockview | null {
  const ids = Object.keys(parsed.panels ?? {})
  if (ids.length === 0) return null
  if (ids.some((id) => !PANELS_BY_ID[id])) return null
  return parsed
}

// ------------------------------------------------------------------ presets ---

/** Which preset is in use, for showing the current one as selected. */
export function readActivePreset(): string {
  try {
    return localStorage.getItem(PRESET_KEY) ?? DEFAULT_PRESET
  } catch {
    return DEFAULT_PRESET
  }
}

function rememberPreset(id: string): void {
  try {
    localStorage.setItem(PRESET_KEY, id)
  } catch {
    /* nothing to do */
  }
}

/** Replace the workspace with a named preset. */
export function applyPreset(api: DockviewApi, id: string): boolean {
  const preset = PRESETS_BY_ID[id]
  if (!preset) return false
  api.clear()
  preset.build(api)
  rememberPreset(id)
  saveLayout(api)
  return true
}

/** Build the arrangement to open with: the stored one, or the default preset. */
export function restoreLayout(api: DockviewApi): void {
  const stored = readLayout()
  if (stored) {
    try {
      api.fromJSON(stored)
      return
    } catch {
      // A layout that will not deserialise is not worth diagnosing; the
      // default arrangement is one call away.
      api.clear()
    }
  }
  applyPreset(api, DEFAULT_PRESET)
}

// ------------------------------------------------------------ saved layouts ---

export function readSavedLayouts(): SavedLayout[] {
  try {
    const raw = localStorage.getItem(SAVED_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as SavedLayout[]
    if (!Array.isArray(parsed)) return []
    // Drop any that reference panels this build no longer has, rather than
    // offering an arrangement that cannot be restored.
    return parsed.filter((entry) => entry?.name && validate(entry.layout) !== null)
  } catch {
    return []
  }
}

function writeSavedLayouts(layouts: SavedLayout[]): void {
  try {
    localStorage.setItem(SAVED_KEY, JSON.stringify(layouts))
  } catch {
    /* nothing to do */
  }
}

/** Keep the current arrangement under a name, replacing one of the same name. */
export function saveNamedLayout(api: DockviewApi, name: string): SavedLayout[] {
  const trimmed = name.trim()
  if (trimmed === '') return readSavedLayouts()
  const entry: SavedLayout = {
    name: trimmed,
    savedAt: new Date().toISOString(),
    layout: api.toJSON(),
  }
  const rest = readSavedLayouts().filter(
    (l) => l.name.toLowerCase() !== trimmed.toLowerCase(),
  )
  const next = [entry, ...rest].slice(0, 12)
  writeSavedLayouts(next)
  return next
}

export function applyNamedLayout(api: DockviewApi, name: string): boolean {
  const entry = readSavedLayouts().find((l) => l.name === name)
  if (!entry) return false
  try {
    api.clear()
    api.fromJSON(entry.layout)
    rememberPreset(`saved:${name}`)
    saveLayout(api)
    return true
  } catch {
    applyPreset(api, DEFAULT_PRESET)
    return false
  }
}

export function deleteNamedLayout(name: string): SavedLayout[] {
  const next = readSavedLayouts().filter((l) => l.name !== name)
  writeSavedLayouts(next)
  return next
}

// -------------------------------------------------------------- panel access ---

/**
 * Bring a panel into the workspace, or focus it if it is already there.
 *
 * The way a closed panel comes back. Panels are closeable, so without this the
 * only way to see one again would be resetting the whole layout.
 */
export function showPanel(api: DockviewApi, id: string): void {
  const existing = api.getPanel(id)
  if (existing) {
    existing.api.setActive()
    return
  }
  const definition = PANELS_BY_ID[id]
  if (!definition) return
  api.addPanel({ id, component: id, title: definition.title })
}

/** Which panels are currently open. */
export function openPanelIds(api: DockviewApi): string[] {
  return api.panels.map((p) => p.id)
}
