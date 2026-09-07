// Application state.
//
// One store rather than several, because the panels are deliberately
// interchangeable: any of them can be moved, tabbed elsewhere, or torn off
// onto another screen, so none of them can own state another one needs. A
// panel reads what it wants and renders; that is the whole reason the docking
// works at all.

import { create } from 'zustand'
import { api, isDesktop } from '../lib/api'
import {
  advanceMeter,
  DEFAULT_BALLISTICS,
  emptyMeter,
  type Ballistics,
  type MeterState,
} from '../lib/meters'
import type {
  DeviceInfo,
  DiskSpace,
  Discovery,
  DiscoveredSource,
  LibrarySession,
  LogEntry,
  LogLevel,
  Marker,
  SessionConfig,
  SessionManifest,
  SessionSnapshot,
  SourceRequest,
} from '../lib/types'

/** Meter samples the timeline keeps per track: ninety seconds at ten a second. */
export const HISTORY_LENGTH = 900

/** How many log lines are kept. A long session must not grow this forever. */
const LOG_LIMIT = 800

export type ThemeName = 'dark' | 'midnight' | 'light' | 'contrast'
export type AccentName = 'blue' | 'violet' | 'teal' | 'amber' | 'rose'
export type DensityName = 'compact' | 'cosy' | 'comfortable'

/** Everything the user has chosen about how the program looks and behaves. */
export interface Preferences {
  theme: ThemeName
  accent: AccentName
  density: DensityName
  /** Poll interval for the live recording snapshot, in milliseconds. */
  refreshMs: number
  /** Discovery refresh interval while something is watching it. */
  discoveryMs: number
  /** Bottom of the meter scale, in dBFS. */
  meterFloor: number
  /** How long a meter holds its peak before it starts to fall. */
  holdMs: number
  /** Show the Windows notification-sounds session in discovery. */
  showSystemSessions: boolean
  /** Hide discovered applications that are not currently making a sound. */
  playingOnly: boolean
  /**
   * Add a remote session to the plan the moment discovery finds one.
   *
   * Off by default, because adding a track to someone's session without being
   * asked is a surprise. On, it is the difference between catching the start
   * of a Citrix call and missing it.
   */
  autoArmRemote: boolean
  /** Keep the window above others while recording. */
  alwaysOnTop: boolean
  /** Show the transport's secondary readouts. */
  showEstimates: boolean
}

export interface CaptureSettings {
  outputDir: string
  bufferMs: number
  sampleRate: number
  channels: number
  durationSecs: number | null
}

/** A source the user has added to the session, with a stable key for React. */
export interface PlannedSource {
  key: string
  request: SourceRequest
  /** Name shown in lists, derived when the request carries no label. */
  title: string
  /** Secondary line, describing what will be tapped. */
  subtitle: string
}

export interface Toast {
  id: number
  level: LogLevel | 'ok'
  title: string
  detail?: string
}

const PREFS_KEY = 'ssr.prefs.v1'
const CAPTURE_KEY = 'ssr.capture.v1'

export const DEFAULT_PREFERENCES: Preferences = {
  theme: 'dark',
  accent: 'blue',
  density: 'cosy',
  refreshMs: 100,
  discoveryMs: 200,
  meterFloor: -60,
  holdMs: DEFAULT_BALLISTICS.holdMs,
  showSystemSessions: false,
  playingOnly: false,
  autoArmRemote: false,
  alwaysOnTop: false,
  showEstimates: true,
}

const DEFAULT_CAPTURE: CaptureSettings = {
  outputDir: '',
  bufferMs: 200,
  sampleRate: 48000,
  channels: 2,
  durationSecs: null,
}

/** Read a persisted object, keeping only the keys the defaults declare.
 *
 *  A stored blob from an older version may be missing keys or carry ones that
 *  no longer mean anything; merging over the defaults rather than trusting the
 *  blob is what keeps an upgrade from producing an interface with holes in it. */
function load<T extends object>(key: string, defaults: T): T {
  try {
    const raw = localStorage.getItem(key)
    if (!raw) return defaults
    const parsed = JSON.parse(raw) as Partial<T>
    const merged = { ...defaults }
    for (const k of Object.keys(defaults) as (keyof T)[]) {
      const value = parsed[k]
      if (value !== undefined && typeof value === typeof defaults[k]) {
        merged[k] = value as T[keyof T]
      }
    }
    return merged
  } catch {
    return defaults
  }
}

function save(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value))
  } catch {
    // A full or disabled local store is not worth interrupting anyone over.
  }
}

interface AppState {
  ready: boolean
  prefs: Preferences
  settings: CaptureSettings

  devices: DeviceInfo[]
  discovery: Discovery | null
  /** True once a discovery pass has been seen, so the panel can tell
   *  "nothing is playing" from "nothing has been looked at yet". */
  discovered: boolean

  planned: PlannedSource[]
  snapshot: SessionSnapshot | null
  manifest: SessionManifest | null
  /** Folder the most recent session was written to, once it is known. */
  sessionDir: string | null

  /** Meter ballistics per track name, for the live session. */
  meters: Record<string, MeterState>
  /** The same for discovered audio sessions, keyed by session key. */
  sessionMeters: Record<string, MeterState>
  /** Rolling level history per track name, oldest first. */
  history: Record<string, { peak: number[]; rms: number[] }>

  library: LibrarySession[]
  libraryLoading: boolean
  disk: DiskSpace | null

  log: LogEntry[]
  toasts: Toast[]
  busy: boolean
  error: string | null

  setPref: <K extends keyof Preferences>(key: K, value: Preferences[K]) => void
  updateSettings: (patch: Partial<CaptureSettings>) => void

  refreshInventory: () => Promise<void>
  pollDiscovery: () => Promise<void>
  rescan: () => Promise<void>

  addSource: (request: SourceRequest, options?: { quiet?: boolean }) => boolean
  addDiscovered: (source: DiscoveredSource) => void
  removeSource: (key: string) => void
  moveSource: (key: string, delta: number) => void
  renameSource: (key: string, label: string) => void
  clearSources: () => void

  start: () => Promise<void>
  stop: () => Promise<void>
  poll: () => Promise<void>
  addMarker: (label: string) => Promise<void>
  clearClip: (track?: string) => void

  loadLibrary: () => Promise<void>
  refreshDisk: () => Promise<void>

  appendLog: (level: LogLevel, message: string) => void
  clearLog: () => void
  notify: (level: Toast['level'], title: string, detail?: string) => void
  dismissToast: (id: number) => void
}

let logCounter = 0
let sourceCounter = 0
let toastCounter = 0

/** Executables discovery has already offered to arm, so a source that comes
 *  and goes is not added again after the user has removed it. */
const autoArmed = new Set<string>()

function describe(request: SourceRequest): { title: string; subtitle: string } {
  switch (request.kind) {
    case 'system':
      return {
        title: request.label ?? 'System mix',
        subtitle:
          request.device === 'default'
            ? 'Everything played on the default output'
            : 'Everything played on this output',
      }
    case 'input':
      return {
        title: request.label ?? 'Microphone',
        subtitle: request.device === 'default' ? 'Default input device' : 'This input device',
      }
    case 'app':
      return {
        title: request.label ?? request.target.replace(/\.exe$/i, ''),
        subtitle: request.exclude
          ? `Everything except ${request.target}`
          : `Only ${request.target}, including child processes`,
      }
  }
}

/** Two requests are the same source if they would tap the same thing. */
function sameSource(a: SourceRequest, b: SourceRequest): boolean {
  if (a.kind !== b.kind) return false
  if (a.kind === 'app' && b.kind === 'app') {
    return a.target.toLowerCase() === b.target.toLowerCase() && a.exclude === b.exclude
  }
  if (a.kind === 'system' && b.kind === 'system') return a.device === b.device
  if (a.kind === 'input' && b.kind === 'input') return a.device === b.device
  return false
}

/** The stored preferences, readable before the store exists.
 *
 *  Used to set the theme on the document before the first render, so the
 *  window is never painted in the wrong one. */
export function readPreferences(): Preferences {
  return load(PREFS_KEY, DEFAULT_PREFERENCES)
}

export const useStore = create<AppState>((set, get) => ({
  ready: false,
  prefs: readPreferences(),
  settings: load(CAPTURE_KEY, DEFAULT_CAPTURE),

  devices: [],
  discovery: null,
  discovered: false,

  planned: [],
  snapshot: null,
  manifest: null,
  sessionDir: null,

  meters: {},
  sessionMeters: {},
  history: {},

  library: [],
  libraryLoading: false,
  disk: null,

  log: [],
  toasts: [],
  busy: false,
  error: null,

  // --- preferences -------------------------------------------------------

  setPref(key, value) {
    set((s) => {
      const prefs = { ...s.prefs, [key]: value }
      save(PREFS_KEY, prefs)
      return { prefs }
    })
    if (key === 'alwaysOnTop') void api.setAlwaysOnTop(value as boolean)
    if (key === 'discoveryMs') void api.setDiscoveryRate(value as number)
  },

  updateSettings(patch) {
    set((s) => {
      const settings = { ...s.settings, ...patch }
      save(CAPTURE_KEY, settings)
      return { settings }
    })
    if (patch.outputDir) void get().refreshDisk()
  },

  // --- inventory and discovery -------------------------------------------

  async refreshInventory() {
    try {
      const devices = await api.listDevices()
      set({ devices, ready: true, error: null })
      if (!get().settings.outputDir) {
        const dir = await api.defaultOutputDir()
        get().updateSettings({ outputDir: dir })
      }
      await get().refreshDisk()
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e)
      set({ error: message, ready: true })
      get().appendLog('error', `Could not read the audio inventory: ${message}`)
    }
  },

  async pollDiscovery() {
    try {
      const discovery = await api.discover()
      const now = Date.now()
      const ballistics: Ballistics = {
        ...DEFAULT_BALLISTICS,
        holdMs: get().prefs.holdMs,
      }

      set((s) => {
        // Peak hold on the discovery rows too, so a notification sound that
        // lasted a third of a second is still visible a moment later.
        const sessionMeters: Record<string, MeterState> = {}
        for (const session of discovery.sessions) {
          const db = session.peak <= 0.000001 ? -120 : 20 * Math.log10(session.peak)
          sessionMeters[session.key] = advanceMeter(
            s.sessionMeters[session.key],
            { peakDb: db, rmsDb: db },
            now,
            ballistics,
          )
        }
        return {
          discovery,
          discovered: true,
          sessionMeters,
          // The endpoint list comes with every pass, so the devices panel
          // stays current without a call of its own.
          devices: discovery.devices.length > 0 ? discovery.devices : s.devices,
        }
      })

      if (get().prefs.autoArmRemote) {
        for (const source of discovery.sources) {
          if (!source.recommended || !source.playing) continue
          const exe = source.exe.toLowerCase()
          if (autoArmed.has(exe)) continue
          autoArmed.add(exe)
          const added = get().addSource(
            { kind: 'app', target: source.exe, exclude: false, label: null },
            { quiet: true },
          )
          if (added) {
            get().notify(
              'ok',
              `Armed ${source.label}`,
              'A remote session started playing and was added to the plan.',
            )
            get().appendLog('info', `Armed ${source.label} (${source.exe}) automatically.`)
          }
        }
      }
    } catch {
      // A failed pass is not worth reporting: the next one either works, or
      // the transport itself is gone, which shows up elsewhere.
    }
  },

  async rescan() {
    try {
      const discovery = await api.rescan()
      set({ discovery, discovered: true })
      get().appendLog(
        'info',
        `Rescanned: ${discovery.sources.length} application(s), ${discovery.devices.length} endpoint(s).`,
      )
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e)
      get().notify('error', 'Rescan failed', message)
    }
  },

  // --- the plan ----------------------------------------------------------

  addSource(request, options) {
    const existing = get().planned
    if (existing.some((p) => sameSource(p.request, request))) {
      if (!options?.quiet) {
        get().notify('info', 'Already in the session', 'That source is on the list.')
      }
      return false
    }
    sourceCounter += 1
    const { title, subtitle } = describe(request)
    set({
      planned: [...existing, { key: `src-${sourceCounter}`, request, title, subtitle }],
    })
    return true
  },

  addDiscovered(source) {
    const added = get().addSource({
      kind: 'app',
      target: source.exe,
      exclude: false,
      label: null,
    })
    if (added) {
      get().appendLog('info', `Added ${source.label} (${source.exe}) to the session.`)
    }
  },

  removeSource(key) {
    set((s) => ({ planned: s.planned.filter((p) => p.key !== key) }))
  },

  /** Move a planned source up or down. Track order is file order, which is
   *  the order they appear in an editor, so it is worth being able to set. */
  moveSource(key, delta) {
    set((s) => {
      const index = s.planned.findIndex((p) => p.key === key)
      if (index < 0) return {}
      const target = index + delta
      if (target < 0 || target >= s.planned.length) return {}
      const planned = s.planned.slice()
      const [moved] = planned.splice(index, 1)
      if (!moved) return {}
      planned.splice(target, 0, moved)
      return { planned }
    })
  },

  /** Give a track a name of its own, which becomes its file name. */
  renameSource(key, label) {
    const trimmed = label.trim()
    set((s) => ({
      planned: s.planned.map((p) => {
        if (p.key !== key) return p
        const request = { ...p.request, label: trimmed === '' ? null : trimmed }
        const described = describe(request)
        return { ...p, request, title: described.title, subtitle: described.subtitle }
      }),
    }))
  },

  clearSources() {
    set({ planned: [] })
    autoArmed.clear()
  },

  // --- transport ---------------------------------------------------------

  async start() {
    const { planned, settings } = get()
    if (planned.length === 0) {
      get().notify(
        'warn',
        'Nothing to record',
        'Add a source first: an output endpoint for everything at once, or an application to keep it separate.',
      )
      return
    }
    set({ busy: true, error: null, history: {}, meters: {}, manifest: null })
    const config: SessionConfig = {
      sources: planned.map((p) => p.request),
      // A library folder, not a destination: the recorder makes a timestamped
      // subfolder for each session so recordings accumulate rather than
      // overwriting one another.
      output: { kind: 'newSessionIn', root: settings.outputDir },
      bufferMs: settings.bufferMs,
      sampleRate: settings.sampleRate,
      channels: settings.channels,
      durationSecs: settings.durationSecs,
    }
    try {
      const snapshot = await api.startRecording(config)
      set({ snapshot, busy: false, sessionDir: snapshot.outputDir })
      get().appendLog(
        'info',
        `Recording ${planned.length} track(s) to ${snapshot.outputDir}`,
      )
      get().notify('ok', 'Recording', `${planned.length} track(s) to ${snapshot.outputDir}`)
      for (const w of snapshot.warnings) get().appendLog('warn', w)
      void get().refreshDisk()
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e)
      set({ busy: false, error: message })
      get().appendLog('error', `Could not start recording: ${message}`)
      get().notify('error', 'Could not start recording', message)
    }
  },

  async stop() {
    set({ busy: true })
    try {
      const manifest = await api.stopRecording()
      set({ manifest, busy: false })
      const summary = `${manifest.duration_secs.toFixed(1)} s across ${
        manifest.tracks.length
      } track(s)`
      get().appendLog('info', `Finished: ${summary}`)
      get().notify('ok', 'Recording finished', summary)
      for (const w of manifest.warnings ?? []) get().appendLog('warn', w)
      const snapshot = await api.snapshot()
      set({ snapshot })
      void get().loadLibrary()
      void get().refreshDisk()
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e)
      set({ busy: false, error: message })
      get().appendLog('error', `Could not stop cleanly: ${message}`)
      get().notify('error', 'Could not stop cleanly', message)
    }
  },

  async poll() {
    try {
      const snapshot = await api.snapshot()
      const now = Date.now()
      const ballistics: Ballistics = { ...DEFAULT_BALLISTICS, holdMs: get().prefs.holdMs }

      set((s) => {
        const meters: Record<string, MeterState> = {}
        const history = { ...s.history }
        for (const track of snapshot.tracks) {
          meters[track.name] = advanceMeter(
            s.meters[track.name],
            { peakDb: track.peakDb, rmsDb: track.rmsDb },
            now,
            ballistics,
          )
          const prev = history[track.name] ?? { peak: [], rms: [] }
          const peak = prev.peak.length >= HISTORY_LENGTH ? prev.peak.slice(1) : prev.peak.slice()
          const rms = prev.rms.length >= HISTORY_LENGTH ? prev.rms.slice(1) : prev.rms.slice()
          peak.push(track.peakDb)
          rms.push(track.rmsDb)
          history[track.name] = { peak, rms }
        }
        return { snapshot, meters, history }
      })
    } catch {
      // As with discovery: one failed poll is not news.
    }
  },

  async addMarker(label) {
    try {
      const marker: Marker | null = await api.addMarker(label)
      if (!marker) {
        get().notify('warn', 'Nothing to mark', 'Markers can only be dropped while recording.')
        return
      }
      get().appendLog(
        'info',
        `Marker at ${marker.at_secs.toFixed(1)} s${marker.label ? `: ${marker.label}` : ''}`,
      )
      // Take the marker straight into the snapshot rather than waiting for the
      // next poll, so the timeline shows it the instant the key is pressed.
      set((s) =>
        s.snapshot
          ? { snapshot: { ...s.snapshot, markers: [...s.snapshot.markers, marker] } }
          : {},
      )
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e)
      get().notify('error', 'Could not add the marker', message)
    }
  },

  /** Clear a latched clip indicator, or all of them. */
  clearClip(track) {
    set((s) => {
      const meters = { ...s.meters }
      for (const [name, meter] of Object.entries(meters)) {
        if (track && name !== track) continue
        meters[name] = { ...meter, clipped: false }
      }
      return { meters }
    })
  },

  // --- library and capacity ----------------------------------------------

  async loadLibrary() {
    const root = get().settings.outputDir
    if (!root) return
    set({ libraryLoading: true })
    try {
      const library = await api.listSessions(root)
      set({ library, libraryLoading: false })
    } catch (e) {
      set({ libraryLoading: false })
      const message = e instanceof Error ? e.message : String(e)
      get().appendLog('warn', `Could not read the recordings folder: ${message}`)
    }
  },

  async refreshDisk() {
    const root = get().settings.outputDir
    if (!root) return
    try {
      set({ disk: await api.diskSpace(root) })
    } catch {
      // A path that cannot be measured is reported by the panel that shows it
      // as simply unknown, which is the truth.
      set({ disk: null })
    }
  },

  // --- log and notifications ---------------------------------------------

  appendLog(level, message) {
    logCounter += 1
    const entry: LogEntry = {
      id: logCounter,
      at: new Date().toLocaleTimeString(),
      level,
      message,
    }
    set((s) => ({ log: [...s.log, entry].slice(-LOG_LIMIT) }))
  },

  clearLog() {
    set({ log: [] })
  },

  notify(level, title, detail) {
    toastCounter += 1
    const toast: Toast = { id: toastCounter, level, title, detail }
    set((s) => ({ toasts: [...s.toasts, toast].slice(-4) }))
    // Errors stay until dismissed. Everything else is a confirmation of
    // something the user just did, and does not need to be read twice.
    if (level !== 'error') {
      window.setTimeout(() => get().dismissToast(toast.id), level === 'warn' ? 7000 : 4200)
    }
  },

  dismissToast(id) {
    set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }))
  },
}))

/** Whether a session is currently capturing. */
export function isRecording(snapshot: SessionSnapshot | null): boolean {
  return snapshot?.state === 'recording' || snapshot?.state === 'starting'
}

/** The meter state for a track, or an empty one, so components never branch. */
export function meterFor(meters: Record<string, MeterState>, name: string): MeterState {
  return meters[name] ?? emptyMeter()
}

/**
 * Apply the visual preferences to the document.
 *
 * Done as attributes on the root element rather than by re-rendering, so a
 * theme change costs one style recalculation and nothing else.
 *
 * The address can override any of the three, without changing what is stored.
 * That is what lets a torn-off window differ from the one it came from — a
 * monitor window on a bright second screen wants the light theme even when the
 * main window is dark — and it is how the interface is inspected in each of
 * its appearances.
 */
export function applyPreferences(prefs: Preferences): void {
  const root = document.documentElement
  const search = new URLSearchParams(window.location.search)
  const override = <T extends string>(name: string, allowed: readonly T[], fallback: T): T => {
    const value = search.get(name) as T | null
    return value && allowed.includes(value) ? value : fallback
  }

  root.dataset.theme = override(
    'theme',
    ['dark', 'midnight', 'light', 'contrast'] as const,
    prefs.theme,
  )
  root.dataset.accent = override(
    'accent',
    ['blue', 'violet', 'teal', 'amber', 'rose'] as const,
    prefs.accent,
  )
  root.dataset.density = override(
    'density',
    ['compact', 'cosy', 'comfortable'] as const,
    prefs.density,
  )
}

/** True when the interface is running without an engine behind it. */
export const isPreview = (): boolean => !isDesktop()
