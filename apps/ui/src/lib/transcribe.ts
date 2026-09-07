// Transcription, from the interface's point of view.
//
// Kept apart from api.ts because it is a second mode rather than more of the
// first: recording produces audio files, this produces words. The split also
// keeps the two from tangling while both are being worked on.

import { isDesktop } from './api'
import type { SourceRequest } from './types'

type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>

let invokeImpl: InvokeFn | null = null

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!invokeImpl) {
    const mod = await import('@tauri-apps/api/core')
    invokeImpl = mod.invoke as InvokeFn
  }
  return invokeImpl<T>(cmd, args)
}

/** A speech engine found on the machine. */
export interface SpeechEngine {
  kind: 'whisperCpp' | 'whisperPython'
  label: string
  executable: string
  models: string[]
  /**
   * The interpreter that can hold this engine open, when one was found.
   *
   * Its absence is the difference between the model being loaded once and
   * being loaded again for every utterance, which costs five to fifteen
   * seconds each time.
   */
  interpreter?: string | null
}

/** A model, and whether it is already on the machine. */
export interface ModelInfo {
  id: string
  label: string
  hint: string
  /** Roughly what fetching it will cost. */
  approximateBytes: number
  /** True when it will load without downloading anything. */
  cached: boolean
  /** What it actually occupies, once it is here. */
  bytes?: number
}

/** How a model download is going. */
export interface ModelDownload {
  active: boolean
  model: string
  fraction: number
  bytes: number
  total: number
  done: boolean
  error?: string
}

/**
 * Languages worth putting at the top of the list.
 *
 * Naming the language is not only about accuracy: detection costs a pass over
 * the audio, so saying which language it is makes every utterance faster too.
 */
export const LANGUAGES: { id: string; label: string }[] = [
  { id: '', label: 'Detect automatically' },
  { id: 'uk', label: 'Ukrainian' },
  { id: 'en', label: 'English' },
  { id: 'ru', label: 'Russian' },
  { id: 'pl', label: 'Polish' },
  { id: 'de', label: 'German' },
  { id: 'fr', label: 'French' },
  { id: 'es', label: 'Spanish' },
]

export interface EngineReport {
  engines: SpeechEngine[]
  /** True when at least one engine could actually run. */
  ready: boolean
  /** What to do when none can. Shown as written; it contains commands. */
  advice: string
}

/** Who said something, as far as the recorder can tell. */
export interface Speaker {
  index: number
  label: string
  /** True when this came from grouping voices rather than from the channel,
   *  which is a guess and should be presented as one. */
  inferred: boolean
}

/** One line of the conversation. */
export interface Line {
  index: number
  channel: string
  speaker: Speaker
  start: number
  end: number
  text: string
}

export interface ListenStatus {
  active: boolean
  channel: string
  engine: string
  elapsedSecs: number
  /** Utterances heard but not yet recognised. Climbing means the machine is
   *  falling behind the conversation. */
  backlog: number
  transcriptPath: string
  totalLines: number
  speakers: number
  error?: string
}

export interface ListenConfig {
  source: SourceRequest
  label: string
  detectSpeakers: boolean
  language?: string | null
  model?: string | null
  translate?: boolean
  engine?: string | null
  transcriptPath?: string | null
  bufferMs?: number
}

export type TranscriptFormat = 'txt' | 'srt' | 'vtt' | 'json' | 'md'

export const FORMATS: { id: TranscriptFormat; label: string; hint: string }[] = [
  { id: 'txt', label: 'Plain text', hint: 'The words, with no timings' },
  { id: 'md', label: 'Markdown', hint: 'Timestamped, reads well in notes' },
  { id: 'srt', label: 'SubRip', hint: 'Subtitles most players read' },
  { id: 'vtt', label: 'WebVTT', hint: 'Subtitles for the web' },
  { id: 'json', label: 'JSON', hint: 'Structured, for processing further' },
]

export interface SaveRequest {
  lines: Line[]
  channel: string
  engine: string
  directory: string
  stem: string
  formats: TranscriptFormat[]
  includeSpeakers: boolean
  /** Also write a meeting record: who spoke, what looks like an action or a
   *  decision, and the whole transcript underneath it. */
  includeNotes: boolean
  startedUtc: string
}

export interface SaveResult {
  directory: string
  files: string[]
}

export const idleStatus = (): ListenStatus => ({
  active: false,
  channel: '',
  engine: '',
  elapsedSecs: 0,
  backlog: 0,
  transcriptPath: '',
  totalLines: 0,
  speakers: 0,
})

export const transcribeApi = {
  async engines(): Promise<EngineReport> {
    return isDesktop() ? invoke<EngineReport>('transcription_engines') : sim.engines()
  },

  async startListening(config: ListenConfig): Promise<ListenStatus> {
    return isDesktop()
      ? invoke<ListenStatus>('start_listening', { config })
      : sim.start(config)
  },

  /** Status, plus only the lines the caller has not seen. Sending the whole
   *  conversation on every poll would grow without bound over a long meeting. */
  async poll(after: number): Promise<{ status: ListenStatus; lines: Line[] }> {
    return isDesktop()
      ? invoke<{ status: ListenStatus; lines: Line[] }>('listening_poll', { after })
      : sim.poll(after)
  },

  async stopListening(): Promise<ListenStatus> {
    return isDesktop() ? invoke<ListenStatus>('stop_listening') : sim.stop()
  },

  async save(request: SaveRequest): Promise<SaveResult> {
    return isDesktop() ? invoke<SaveResult>('save_conversation', { request }) : sim.save(request)
  },

  async models(): Promise<ModelInfo[]> {
    return isDesktop() ? invoke<ModelInfo[]>('list_speech_models') : sim.models()
  },

  async downloadModel(model: string): Promise<ModelDownload> {
    return isDesktop()
      ? invoke<ModelDownload>('download_speech_model', { model })
      : sim.download(model)
  },

  async downloadStatus(): Promise<ModelDownload> {
    return isDesktop() ? invoke<ModelDownload>('model_download_status') : sim.downloadStatus()
  },

  /** Name one of the voices, backwards through the conversation and forwards. */
  async nameSpeaker(index: number, name: string): Promise<Line[]> {
    return isDesktop() ? invoke<Line[]>('name_speaker', { index, name }) : sim.name(index, name)
  },

  /** The voices heard so far: number, automatic label, and any name given. */
  async voices(): Promise<[number, string, string][]> {
    return isDesktop()
      ? invoke<[number, string, string][]>('conversation_voices')
      : sim.voices()
  },

  async sessionTracks(path: string): Promise<string[]> {
    return isDesktop() ? invoke<string[]>('session_tracks', { path }) : ['system', 'mic']
  },
}

// ---------------------------------------------------------------------------
// Browser simulator
//
// Produces a conversation between two speakers so the chat, the speaker
// labelling and the save dialog can be worked on without Windows or a model.
// ---------------------------------------------------------------------------

const sim = (() => {
  let running = false
  let startedAt = 0
  let channel = 'Citrix'
  let produced = 0

  const script: [string, string][] = [
    ['1', 'Good morning everyone, can you hear me?'],
    ['2', 'Yes, coming through clearly.'],
    ['1', 'Good. Let me share the numbers from last week.'],
    ['2', 'Before that, did the deployment finish?'],
    ['1', 'It did, about an hour ago. No rollbacks.'],
    ['2', 'That is a relief. Go ahead with the numbers.'],
    ['1', 'Right. Throughput is up about eleven percent.'],
    ['2', 'Is that the caching change or the new hardware?'],
    ['1', 'Mostly caching. The hardware lands next month.'],
  ]

  const lines: Line[] = []

  function generate() {
    if (!running) return
    const elapsed = (Date.now() - startedAt) / 1000
    // Roughly one line every four seconds, as a conversation goes.
    const want = Math.min(script.length, Math.floor(elapsed / 4))
    while (produced < want) {
      const entry = script[produced]
      if (!entry) break
      const [who, text] = entry
      const index = produced + 1
      lines.push({
        index,
        channel,
        speaker: {
          index: Number(who),
          label: `${channel}, speaker ${who}`,
          inferred: true,
        },
        start: produced * 4 + 1,
        end: produced * 4 + 3.6,
        text,
      })
      produced += 1
    }
  }

  function status(): ListenStatus {
    generate()
    return {
      active: running,
      channel,
      engine: 'whisper.cpp (simulated)',
      elapsedSecs: running ? (Date.now() - startedAt) / 1000 : 0,
      backlog: 0,
      transcriptPath: `C:\\Users\\you\\Documents\\transcripts\\${channel}.md`,
      totalLines: lines.length,
      speakers: new Set(lines.map((l) => l.speaker.label)).size,
    }
  }

  const catalogue: ModelInfo[] = [
    { id: 'tiny', label: 'Tiny', hint: 'Fastest, roughest. English only.', approximateBytes: 75e6, cached: true, bytes: 75e6 },
    { id: 'base', label: 'Base', hint: 'Fine for clear English.', approximateBytes: 145e6, cached: true, bytes: 145e6 },
    { id: 'small', label: 'Small', hint: 'The first size worth using for Ukrainian.', approximateBytes: 480e6, cached: false },
    { id: 'medium', label: 'Medium', hint: 'Better again, noticeably slower.', approximateBytes: 1.5e9, cached: false },
    { id: 'large-v3', label: 'Large', hint: 'Best available. Slow on a processor.', approximateBytes: 3.1e9, cached: false },
  ]
  let downloading: ModelDownload = { active: false, model: '', fraction: 0, bytes: 0, total: 0, done: false }
  let downloadStarted = 0

  return {
    async models(): Promise<ModelInfo[]> {
      return catalogue
    },
    async download(model: string): Promise<ModelDownload> {
      downloadStarted = Date.now()
      const entry = catalogue.find((m) => m.id === model)
      downloading = { active: true, model, fraction: 0, bytes: 0, total: entry?.approximateBytes ?? 0, done: false }
      return downloading
    },
    async downloadStatus(): Promise<ModelDownload> {
      if (downloading.active) {
        // Twenty seconds to finish, so the progress bar can be looked at.
        const fraction = Math.min(1, (Date.now() - downloadStarted) / 20000)
        downloading = {
          ...downloading,
          fraction,
          bytes: Math.round(fraction * downloading.total),
          active: fraction < 1,
          done: fraction >= 1,
        }
        if (downloading.done) {
          const entry = catalogue.find((m) => m.id === downloading.model)
          if (entry) {
            entry.cached = true
            entry.bytes = entry.approximateBytes
          }
        }
      }
      return downloading
    },
    async engines(): Promise<EngineReport> {
      return {
        ready: true,
        advice: '',
        engines: [
          {
            kind: 'whisperCpp',
            label: 'whisper.cpp (simulated)',
            executable: 'whisper-cli',
            models: ['ggml-base.bin', 'ggml-medium.bin'],
          },
        ],
      }
    },
    async start(config: ListenConfig): Promise<ListenStatus> {
      channel = config.label || 'Citrix'
      lines.length = 0
      produced = 0
      running = true
      startedAt = Date.now()
      return status()
    },
    async poll(after: number) {
      const s = status()
      return { status: s, lines: lines.slice(after) }
    },
    async stop(): Promise<ListenStatus> {
      const s = status()
      running = false
      return { ...s, active: false }
    },
    async name(index: number, name: string): Promise<Line[]> {
      for (const line of lines) {
        if (line.speaker.index === index) {
          line.speaker.label = name.trim() || `${channel}, speaker ${index}`
          line.speaker.inferred = name.trim() === ''
        }
      }
      return [...lines]
    },
    async voices(): Promise<[number, string, string][]> {
      const seen = new Map<number, string>()
      for (const line of lines) {
        if (!seen.has(line.speaker.index)) seen.set(line.speaker.index, line.speaker.label)
      }
      return [...seen].map(([i, label]) => [i, label, ''] as [number, string, string])
    },
    async save(request: SaveRequest): Promise<SaveResult> {
      const files = request.formats.map((f) => `${request.directory}\\${request.stem}.${f}`)
      if (request.includeNotes) files.unshift(`${request.directory}\\${request.stem} - notes.md`)
      return { directory: request.directory, files }
    },
  }
})()
