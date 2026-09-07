// State for the transcription mode.
//
// A store of its own rather than more fields on the recording store. The two
// modes are alternatives, not layers: one produces audio files and the other
// produces words, and nothing in either needs the other's state. Keeping them
// apart means a change to one cannot disturb the other.

import { create } from 'zustand'
import {
  idleStatus,
  transcribeApi,
  type EngineReport,
  type Line,
  type ListenConfig,
  type ListenStatus,
  type SaveRequest,
  type ModelDownload,
  type ModelInfo,
  type SaveResult,
  type TranscriptFormat,
} from '../lib/transcribe'
import type { SourceRequest } from '../lib/types'

/**
 * Which of the two things the application is doing.
 *
 * Set when a channel is sent to be transcribed rather than by a switch: the
 * choice that matters is which channel, and having chosen one there is nothing
 * left for a mode to decide.
 */
export type Mode = 'record' | 'listen'

interface ConversationState {
  mode: Mode
  engines: EngineReport | null
  status: ListenStatus
  lines: Line[]
  /** The channel chosen to listen to, before listening starts. */
  target: SourceRequest | null
  targetLabel: string
  detectSpeakers: boolean
  /** Empty means let the engine work it out. */
  language: string
  /** Which model to load. Bigger is better and slower. */
  model: string
  /** The models on offer, and which are already here. */
  models: ModelInfo[]
  /** A download in progress, when there is one. */
  download: ModelDownload | null
  busy: boolean
  error: string | null
  /** Whether the save dialog is open. */
  saving: boolean

  setMode: (mode: Mode) => void
  loadEngines: () => Promise<void>
  chooseTarget: (source: SourceRequest, label: string) => void
  setDetectSpeakers: (on: boolean) => void
  setLanguage: (code: string) => void
  setModel: (id: string) => void
  loadModels: () => Promise<void>
  fetchModel: (id: string) => Promise<void>
  pollDownload: () => Promise<void>
  start: () => Promise<void>
  stop: () => Promise<void>
  poll: () => Promise<void>
  clear: () => void
  openSave: () => void
  closeSave: () => void
  save: (
    directory: string,
    stem: string,
    formats: TranscriptFormat[],
    includeSpeakers: boolean,
  ) => Promise<SaveResult | null>
}

export const useConversation = create<ConversationState>((set, get) => ({
  mode: 'record',
  engines: null,
  status: idleStatus(),
  lines: [],
  target: null,
  targetLabel: '',
  detectSpeakers: true,
  language: '',
  model: 'base',
  models: [],
  download: null,
  busy: false,
  error: null,
  saving: false,

  setMode(mode) {
    set({ mode })
    // The engine list is only needed in one mode, so it is fetched on arrival
    // rather than at startup, where it would slow the first paint for someone
    // who only ever records.
    if (mode === 'listen' && !get().engines) void get().loadEngines()
  },

  async loadEngines() {
    try {
      set({ engines: await transcribeApi.engines(), error: null })
      void get().loadModels()
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) })
    }
  },

  chooseTarget(source, label) {
    set({ target: source, targetLabel: label })
  },

  setDetectSpeakers(on) {
    set({ detectSpeakers: on })
  },

  setLanguage(code) {
    set({ language: code })
  },

  setModel(id) {
    set({ model: id })
  },

  async loadModels() {
    try {
      set({ models: await transcribeApi.models() })
    } catch {
      // Not worth reporting: the models list is a convenience, and the engine
      // will still download on demand if it has to.
    }
  },

  async fetchModel(id) {
    try {
      const download = await transcribeApi.downloadModel(id)
      set({ download, error: null })
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) })
    }
  },

  async pollDownload() {
    const current = get().download
    if (!current || !current.active) return
    try {
      const download = await transcribeApi.downloadStatus()
      set({ download })
      if (download.done) {
        // The list carries the cached flag, so it has to be re-read before the
        // interface can stop offering to fetch something already here.
        await get().loadModels()
        if (download.error) set({ error: download.error })
      }
    } catch {
      // The next poll will either work or the job has gone.
    }
  },

  async start() {
    const { target, targetLabel, detectSpeakers, language, model } = get()
    if (!target) {
      set({ error: 'Choose a channel to listen to first.' })
      return
    }
    set({ busy: true, error: null, lines: [] })
    const config: ListenConfig = {
      source: target,
      label: targetLabel || 'Channel',
      detectSpeakers,
      language: language || null,
      model: model || null,
    }
    try {
      const status = await transcribeApi.startListening(config)
      set({ status, busy: false })
    } catch (e) {
      set({ busy: false, error: e instanceof Error ? e.message : String(e) })
    }
  },

  async stop() {
    set({ busy: true })
    try {
      const status = await transcribeApi.stopListening()
      // The final utterances are recognised during the stop, so one more poll
      // is needed or the last thing said never appears.
      const { lines } = await transcribeApi.poll(get().lines.length)
      set((s) => ({ status, lines: [...s.lines, ...lines], busy: false }))
    } catch (e) {
      set({ busy: false, error: e instanceof Error ? e.message : String(e) })
    }
  },

  async poll() {
    try {
      const { status, lines } = await transcribeApi.poll(get().lines.length)
      set((s) => (lines.length > 0 ? { status, lines: [...s.lines, ...lines] } : { status }))
    } catch {
      // A failed poll is not worth reporting; the next one either works or the
      // session has ended, which shows up in the status anyway.
    }
  },

  clear() {
    set({ lines: [], status: idleStatus() })
  },

  openSave() {
    set({ saving: true })
  },

  closeSave() {
    set({ saving: false })
  },

  async save(directory, stem, formats, includeSpeakers) {
    const { lines, status } = get()
    const request: SaveRequest = {
      lines,
      channel: status.channel || 'conversation',
      engine: status.engine || 'unknown',
      directory,
      stem,
      formats,
      includeSpeakers,
    }
    try {
      const result = await transcribeApi.save(request)
      set({ saving: false, error: null })
      return result
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) })
      return null
    }
  },
}))

/** True while a conversation is being transcribed. */
export function isListening(status: ListenStatus): boolean {
  return status.active
}
