// Shapes shared with the Rust backend.
//
// These mirror the serde representations in ssr-core, ssr-session and
// ssr-timeline; changing one means changing the other. Two naming conventions
// meet here and the split is deliberate: everything crossing the IPC boundary
// is camelCase, and everything belonging to the manifest is snake_case,
// because the manifest is a file format that other tools read and its field
// names are not ours to restyle.

export type DeviceKind = 'render' | 'capture'

/** One channel of a stream, as the endpoint describes it. */
export interface ChannelInfo {
  /** Position in the interleaved frame, from zero. */
  index: number
  /** Short label, such as `LFE`. */
  label: string
  /** Full name, such as `Low frequency`. */
  name: string
  /** The speaker bit this channel carries, zero when the endpoint gave none. */
  maskBit: number
}

/**
 * The resolved stream format of an endpoint.
 *
 * Not a preference: a shared-mode capture has to use the endpoint's mix
 * format, so this is what the recorded file will contain.
 */
export interface EndpointFormat {
  sampleRate: number
  channels: number
  bits: number
  validBits: number
  /** `32-bit float`, `24-bit integer`, and so on. */
  sampleFormat: string
  channelMask: number
  /** A short name for the arrangement, such as `5.1`. */
  layout: string
  channelsDetail: ChannelInfo[]
}

export interface DeviceInfo {
  id: string
  name: string
  kind: DeviceKind
  isDefault: boolean
  /** Speakers, headphones, a headset, HDMI. */
  formFactor?: string | null
  /** The adapter's description, which names the hardware behind a generic
   *  endpoint name. */
  adapter?: string | null
  format?: EndpointFormat | null
}

export interface ProcessInfo {
  pid: number
  parent_pid: number
  name: string
}

/** A running application the backend recognises by name. */
export interface KnownApp {
  label: string
  exe: string
  pids: number[]
}

// --------------------------------------------------------------- discovery ---

export type SourceCategory =
  | 'remote'
  | 'conferencing'
  | 'browser'
  | 'chat'
  | 'media'
  | 'audio'
  | 'game'
  | 'system'
  | 'other'

/** Where an audio session is in its lifecycle, as the audio engine sees it. */
export type SessionState = 'active' | 'inactive' | 'expired'

/** One audio stream on the machine, discovered rather than configured. */
export interface AudioSessionInfo {
  /** Stable across refreshes, so a row keeps its place and its animation. */
  key: string
  pid: number
  exe: string
  label: string
  category: SourceCategory
  displayName?: string | null
  endpointId: string
  endpointName: string
  endpointKind: DeviceKind
  state: SessionState
  /** Loudest channel, linear amplitude 0..1. */
  peak: number
  /** Peak per channel, in the endpoint's channel order. */
  channelPeaks: number[]
  volume: number
  muted: boolean
  isSystemSounds: boolean
  known: boolean
  recommended: boolean
}

/** A candidate for recording, aggregated across every session a process owns. */
export interface DiscoveredSource {
  /** Executable name, which is also the selector an app source takes. */
  exe: string
  label: string
  category: SourceCategory
  pids: number[]
  sessions: number
  activeSessions: number
  peak: number
  /** Any session is actively moving audio to an output. */
  playing: boolean
  /** Any session is on a capture endpoint: it is listening. */
  capturing: boolean
  known: boolean
  recommended: boolean
  endpoints: string[]
}

export interface Discovery {
  devices: DeviceInfo[]
  sessions: AudioSessionInfo[]
  sources: DiscoveredSource[]
  /** How long the pass took. Shown in diagnostics. */
  elapsedMs: number
  /** Incremented once per completed pass; zero before the first one. */
  generation: number
  error?: string | null
}

// ------------------------------------------------------------ session setup ---

export type SourceRequest =
  | { kind: 'system'; device: string; label?: string | null }
  | { kind: 'app'; target: string; exclude: boolean; label?: string | null }
  | { kind: 'input'; device: string; label?: string | null }

export type OutputTarget =
  /** Write straight into this directory, replacing what is there. */
  | { kind: 'directory'; path: string }
  /** Treat this as a library and give each session a timestamped subfolder. */
  | { kind: 'newSessionIn'; root: string }
  | { kind: 'file'; path: string }

export interface SessionConfig {
  sources: SourceRequest[]
  output: OutputTarget
  bufferMs: number
  sampleRate: number
  channels: number
  durationSecs: number | null
}

// ----------------------------------------------------------------- live state ---

export type RecorderState =
  | 'starting'
  | 'recording'
  | 'stopping'
  | 'finished'
  | 'failed'

/** A cue point. Snake_case because it lives in the manifest. */
export interface Marker {
  at_secs: number
  label: string
}

export interface TrackSnapshot {
  name: string
  description: string
  file: string
  sampleRate: number
  channels: number
  pid?: number
  durationSecs: number
  peak: number
  peakDb: number
  rmsDb: number
  framesCaptured: number
  framesSilence: number
  discontinuities: number
}

export interface SessionSnapshot {
  state: RecorderState
  elapsedSecs: number
  outputDir: string
  tracks: TrackSnapshot[]
  warnings: string[]
  markers: Marker[]
}

// -------------------------------------------------------------------- manifest ---

export interface TrackStats {
  frames_captured: number
  frames_silence_filled: number
  frames_overlap_trimmed: number
  blocks_dropped: number
  discontinuities: number
  untimed_blocks: number
  lookbehind_growths: number
  longest_gap_ms: number
}

export interface TrackManifest {
  name: string
  file: string
  source: string
  pid?: number
  sample_rate: number
  channels: number
  duration_secs: number
  stats: TrackStats
  size_overflow?: boolean
}

export interface SessionManifest {
  tool: string
  version: string
  started_utc: string
  ended_utc: string
  duration_secs: number
  session_t0_qpc_100ns: number
  tracks: TrackManifest[]
  warnings?: string[]
  markers?: Marker[]
}

// --------------------------------------------------------------------- library ---

export interface LibraryTrack {
  name: string
  file: string
  sizeBytes: number
  durationSecs?: number | null
  sampleRate: number
  channels: number
  source?: string | null
  /** Fraction of the track the recorder had to manufacture, 0..1. */
  silenceFraction?: number | null
  discontinuities: number
}

export interface LibrarySession {
  dir: string
  name: string
  startedUtc?: string | null
  durationSecs?: number | null
  tracks: LibraryTrack[]
  sizeBytes: number
  /** False when there is no readable manifest: interrupted, or still being
   *  written. */
  complete: boolean
  problem?: string | null
  warnings?: string[]
  markers?: Marker[]
  modifiedUnix: number
}

export interface DiskSpace {
  freeBytes: number
  totalBytes: number
}

// ------------------------------------------------------------------------ log ---

export type LogLevel = 'info' | 'warn' | 'error'

export interface LogEntry {
  id: number
  at: string
  level: LogLevel
  message: string
}
