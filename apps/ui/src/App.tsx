// The shell.
//
// Two modes. The main window carries the title bar, the transport, the
// dockable workspace and the status bar. A satellite window — opened by
// tearing a panel out onto a second screen — carries that panel alone, reading
// the same live data through the same store.

import { useCallback, useEffect, useRef, useState } from 'react'
import type { DockviewApi } from 'dockview'
import { CommandBar } from './components/CommandBar'
import { CommandPalette } from './components/CommandPalette'
import { ModeSwitch } from './components/ModeSwitch'
import { SaveDialog } from './components/SaveDialog'
import { ShortcutsSheet } from './components/ShortcutsSheet'
import { StatusBar } from './components/StatusBar'
import { TitleBar } from './components/TitleBar'
import { Toasts } from './components/Toasts'
import { api, isDesktop } from './lib/api'
import { isTypingInto } from './lib/shortcuts'
import { PANELS_BY_ID } from './panels/registry'
import { applyPreferences, isRecording, useStore } from './state/store'
import { useConversationPolling } from './panels/ConversationPanel'
import { useConversation } from './state/conversation'
import { Workspace } from './workspace/Workspace'
import { applyPreset, clearLayout, openPanelIds } from './workspace/layout'
import { PRESETS_BY_ID } from './workspace/presets'

/** How fast discovery refreshes when nothing is watching it closely. */
const IDLE_DISCOVERY_MS = 1000

/** The panel id when this window is a torn-off satellite, else null. */
function satellitePanelId(): string | null {
  const id = new URLSearchParams(window.location.search).get('panel')
  return id && PANELS_BY_ID[id] ? id : null
}

/**
 * An arrangement named in the address, if there is one.
 *
 * `?layout=monitor` opens straight into that preset instead of whatever was
 * left behind last time. Useful for a shortcut that opens the monitoring
 * arrangement on the second screen, and it is how the interface is
 * screenshotted in each of its arrangements without clicking through them.
 */
function requestedLayout(): string | null {
  const id = new URLSearchParams(window.location.search).get('layout')
  return id && PRESETS_BY_ID[id] ? id : null
}

/** Keep the visual preferences on the root element. */
function usePreferences() {
  const prefs = useStore((s) => s.prefs)
  useEffect(() => applyPreferences(prefs), [prefs])
}

/** Keep the store's snapshot fresh while a session is running. */
function useSnapshotPolling() {
  const poll = useStore((s) => s.poll)
  const snapshot = useStore((s) => s.snapshot)
  const refreshMs = useStore((s) => s.prefs.refreshMs)
  const live = isRecording(snapshot)

  useEffect(() => {
    if (!live) return
    // A plain interval is enough: the work is one small message per tick, and
    // the meters are smoothed by the ballistics rather than by the poll rate.
    const handle = window.setInterval(() => void poll(), refreshMs)
    return () => window.clearInterval(handle)
  }, [live, poll, refreshMs])
}

/**
 * Poll autodiscovery, at a rate that depends on whether anyone is looking.
 *
 * The backend does the work on its own thread and this only collects the
 * answer, but there is no point collecting it ten times a second into a window
 * showing nothing but the mixer. So the shell tells the engine to slow down
 * when no discovery panel is open, and stops asking altogether — which is why
 * `watching` is a parameter rather than something this hook works out.
 */
function useDiscoveryPolling(watching: boolean) {
  const pollDiscovery = useStore((s) => s.pollDiscovery)
  const discoveryMs = useStore((s) => s.prefs.discoveryMs)

  useEffect(() => {
    const rate = watching ? discoveryMs : IDLE_DISCOVERY_MS
    void api.setDiscoveryRate(rate)
    // One pass immediately, so a panel that has just been opened is not blank
    // for the length of an interval.
    void pollDiscovery()
    const handle = window.setInterval(() => void pollDiscovery(), rate)
    return () => window.clearInterval(handle)
  }, [watching, discoveryMs, pollDiscovery])
}

/**
 * Seed the browser preview with a running session.
 *
 * Only reachable outside the desktop shell, where the simulator stands in for
 * the audio engine. It exists so the interface can be worked on, reviewed and
 * screenshotted without Windows in the loop.
 */
function useDemoSeed(enabled: boolean) {
  const addSource = useStore((s) => s.addSource)
  const start = useStore((s) => s.start)
  const mode = useConversation((s) => s.mode)
  const chooseTarget = useConversation((s) => s.chooseTarget)
  const startListening = useConversation((s) => s.start)
  const seeded = useRef(false)

  useEffect(() => {
    // Never in the desktop shell: there is a real engine there, and seeding it
    // would start a real recording nobody asked for.
    if (!enabled || seeded.current || isDesktop()) return
    seeded.current = true

    // The address is read directly rather than the store, because the effect
    // that applies it has not necessarily run yet and seeding the wrong mode
    // starts the wrong thing.
    if ((requestedMode() ?? mode) === 'listen') {
      // The transcription mode has its own thing to demonstrate: a conversation
      // arriving line by line, with the voices told apart.
      chooseTarget(
        { kind: 'app', target: 'wfica32.exe', exclude: false, label: null },
        'Citrix HDX session',
      )
      void startListening()
      return
    }

    addSource({ kind: 'system', device: 'default', label: null })
    addSource({ kind: 'app', target: 'wfica32.exe', exclude: false, label: null })
    addSource({ kind: 'app', target: 'ms-teams.exe', exclude: false, label: null })
    addSource({ kind: 'app', target: 'chrome.exe', exclude: false, label: null })
    addSource({ kind: 'input', device: 'default', label: null })
    void start()
  }, [enabled, mode, addSource, start, chooseTarget, startListening])
}

function SatelliteWindow({ panelId, demo }: { panelId: string; demo: boolean }) {
  const definition = PANELS_BY_ID[panelId]
  const refresh = useStore((s) => s.refreshInventory)
  usePreferences()
  useSnapshotPolling()
  useConversationPolling()
  useDiscoveryPolling(definition?.needsDiscovery ?? false)
  // In the desktop shell a satellite reads the same engine as the main window,
  // so there is nothing to seed. In the browser preview each window has its
  // own simulator, and without this a torn-off panel would always look idle.
  useDemoSeed(demo)

  useEffect(() => {
    void refresh()
  }, [refresh])

  if (!definition) return null
  const Body = definition.component

  return (
    <div className="app-shell app-shell--satellite">
      <header className="titlebar" data-tauri-drag-region>
        <definition.icon size={14} />
        <span className="titlebar__title">{definition.title}</span>
        <span className="titlebar__subtitle">{definition.description}</span>
      </header>
      <div style={{ minHeight: 0 }}>
        <Body />
      </div>
      <Toasts />
    </div>
  )
}

function MainWindow({ demo }: { demo: boolean }) {
  const [dock, setDock] = useState<DockviewApi | null>(null)
  const [preset] = useState(requestedLayout)
  const [palette, setPalette] = useState(false)
  const [shortcuts, setShortcuts] = useState(false)
  /** Whether a panel that shows live discovery is currently open. */
  const [watchingDiscovery, setWatchingDiscovery] = useState(true)

  const refresh = useStore((s) => s.refreshInventory)
  const appendLog = useStore((s) => s.appendLog)
  const notify = useStore((s) => s.notify)
  const stop = useStore((s) => s.stop)
  const start = useStore((s) => s.start)
  const rescan = useStore((s) => s.rescan)
  const addMarker = useStore((s) => s.addMarker)
  const snapshot = useStore((s) => s.snapshot)
  const settings = useStore((s) => s.settings)
  const outputDir = settings.outputDir

  usePreferences()
  useSnapshotPolling()
  useConversationPolling()
  useDiscoveryPolling(watchingDiscovery)
  useDemoSeed(demo)

  const mode = useConversation((s) => s.mode)
  const setMode = useConversation((s) => s.setMode)
  const lastMode = useRef<string | null>(null)

  useEffect(() => {
    const wanted = requestedMode()
    if (wanted) setMode(wanted)
  }, [setMode])

  useEffect(() => {
    // Not on the first pass: the layout restored from last time is the user's
    // own and should not be replaced simply because the window opened.
    if (lastMode.current === null) {
      lastMode.current = mode
      return
    }
    if (lastMode.current === mode) return
    lastMode.current = mode
    if (dock) {
      applyPreset(dock, mode === 'listen' ? 'transcribe' : 'console')
    }
  }, [mode, dock])

  useEffect(() => {
    void refresh()
    appendLog('info', 'Ready. Discovery is listening for anything that makes a sound.')
  }, [refresh, appendLog])

  /** Track whether any open panel needs live discovery. */
  const onApi = useCallback(
    (next: DockviewApi) => {
      setDock(next)
      if (preset) applyPreset(next, preset)
      const recompute = () =>
        setWatchingDiscovery(
          openPanelIds(next).some((id) => PANELS_BY_ID[id]?.needsDiscovery),
        )
      recompute()
      next.onDidLayoutChange(recompute)
    },
    [preset],
  )

  const resetLayout = useCallback(() => {
    if (!dock) return
    clearLayout()
    applyPreset(dock, 'console')
    notify('ok', 'Layout reset')
  }, [dock, notify])

  // Transport and navigation from the keyboard, the way a recorder is
  // actually operated: the hands are elsewhere and the eyes are on a call.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = isTypingInto(e.target)

      // The palette and the shortcut sheet answer even while typing, because
      // their own field is the thing being typed into.
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault()
        setPalette((v) => !v)
        return
      }
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === 'p') {
        e.preventDefault()
        setPalette((v) => !v)
        return
      }
      if (typing) return

      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'd') {
        e.preventDefault()
        void rescan()
        return
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'l') {
        e.preventDefault()
        resetLayout()
        return
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'o') {
        e.preventDefault()
        void api.revealPath(outputDir)
        return
      }
      if (e.key === '?') {
        e.preventDefault()
        setShortcuts(true)
        return
      }
      if (e.key === ' ' || ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'r')) {
        e.preventDefault()
        if (isRecording(snapshot)) void stop()
        else void start()
        return
      }
      if (e.key.toLowerCase() === 'm' && !e.ctrlKey && !e.metaKey && !e.altKey) {
        e.preventDefault()
        void addMarker('')
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [snapshot, start, stop, rescan, addMarker, resetLayout, outputDir])

  return (
    <div className="app-shell">
      <TitleBar api={dock} onShowShortcuts={() => setShortcuts(true)} extra={<ModeSwitch />} />
      <CommandBar />
      <Workspace onApi={onApi} />
      <StatusBar />
      <Toasts />
      <SaveDialog />
      {palette && <CommandPalette api={dock} onClose={() => setPalette(false)} />}
      {shortcuts && <ShortcutsSheet onClose={() => setShortcuts(false)} />}
    </div>
  )
}

/** Which mode to open in, when the address says. Useful for a shortcut that
 *  goes straight to transcribing, and for reviewing either mode on its own. */
function requestedMode(): 'record' | 'listen' | null {
  const value = new URLSearchParams(window.location.search).get('mode')
  return value === 'listen' || value === 'record' ? value : null
}

export default function App() {
  const [panelId] = useState(satellitePanelId)
  const [demo] = useState(() => new URLSearchParams(window.location.search).has('demo'))
  return panelId ? (
    <SatelliteWindow panelId={panelId} demo={demo} />
  ) : (
    <MainWindow demo={demo} />
  )
}
