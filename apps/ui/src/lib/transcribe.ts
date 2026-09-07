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
}

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

  return {
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
    async save(request: SaveRequest): Promise<SaveResult> {
      return {
        directory: request.directory,
        files: request.formats.map((f) => `${request.directory}\\${request.stem}.${f}`),
      }
    },
  }
})()
