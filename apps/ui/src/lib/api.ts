// The boundary between the interface and the recorder.
//
// Every call goes through here. When the page is not running inside the
// desktop shell the calls fall back to a simulator, so the interface can be
// developed, reviewed and demonstrated in an ordinary browser with no audio
// engine present. The simulator is not a stub: it models moving levels, a
// machine's worth of audio sessions coming and going, a library of past
// recordings and a filling disk, because a layout that only looks right
// against empty data is a layout that has not been checked.

import type {
  AudioSessionInfo,
  DeviceInfo,
  Discovery,
  DiskSpace,
  KnownApp,
  LibrarySession,
  Marker,
  ProcessInfo,
  SessionConfig,
  SessionManifest,
  SessionSnapshot,
} from './types'

/** True when running inside the Tauri desktop shell. */
export const isDesktop = (): boolean =>
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>

let invokeImpl: InvokeFn | null = null

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!invokeImpl) {
    const mod = await import('@tauri-apps/api/core')
    invokeImpl = mod.invoke as InvokeFn
  }
  return invokeImpl<T>(cmd, args)
}

export const api = {
  async listDevices(): Promise<DeviceInfo[]> {
    return isDesktop() ? invoke<DeviceInfo[]>('list_devices') : sim.listDevices()
  },

  async listApps(): Promise<KnownApp[]> {
    return isDesktop() ? invoke<KnownApp[]>('list_known_apps') : sim.listApps()
  },

  async listProcesses(): Promise<ProcessInfo[]> {
    return isDesktop() ? invoke<ProcessInfo[]>('list_processes') : sim.listProcesses()
  },

  /**
   * The latest autodiscovery snapshot.
   *
   * A memory copy on the backend, not an enumeration: the monitor thread does
   * the work on its own schedule, so this is safe to poll at frame rate.
   */
  async discover(): Promise<Discovery> {
    return isDesktop() ? invoke<Discovery>('discover') : sim.discover()
  },

  /** Ask discovery to refresh faster while something is watching it. */
  async setDiscoveryRate(intervalMs: number): Promise<void> {
    if (isDesktop()) await invoke('set_discovery_rate', { intervalMs })
    else sim.setDiscoveryRate(intervalMs)
  },

  /** Rebuild the discovery picture now rather than at the next rescan. */
  async rescan(): Promise<Discovery> {
    return isDesktop() ? invoke<Discovery>('rescan') : sim.rescan()
  },

  async startRecording(config: SessionConfig): Promise<SessionSnapshot> {
    return isDesktop()
      ? invoke<SessionSnapshot>('start_recording', { config })
      : sim.start(config)
  },

  async stopRecording(): Promise<SessionManifest> {
    return isDesktop() ? invoke<SessionManifest>('stop_recording') : sim.stop()
  },

  async snapshot(): Promise<SessionSnapshot> {
    return isDesktop() ? invoke<SessionSnapshot>('snapshot') : sim.snapshot()
  },

  /** Mark this instant in the running session. */
  async addMarker(label: string): Promise<Marker | null> {
    return isDesktop()
      ? invoke<Marker | null>('add_marker', { label })
      : sim.addMarker(label)
  },

  async defaultOutputDir(): Promise<string> {
    return isDesktop()
      ? invoke<string>('default_output_dir')
      : 'C:\\Users\\you\\Documents\\System Sound Recorder'
  },

  /** Past sessions in a library folder, newest first. */
  async listSessions(root: string, limit?: number): Promise<LibrarySession[]> {
    return isDesktop()
      ? invoke<LibrarySession[]>('list_sessions', { root, limit })
      : sim.listSessions()
  },

  async diskSpace(path: string): Promise<DiskSpace> {
    return isDesktop() ? invoke<DiskSpace>('disk_space', { path }) : sim.diskSpace()
  },

  /** Ask the shell to open a folder in the file manager. */
  async revealPath(path: string): Promise<void> {
    if (isDesktop()) await invoke('reveal_path', { path })
    else console.info('[simulated] reveal', path)
  },

  /** Open a panel in its own operating system window, for a second screen. */
  async openPanelWindow(panelId: string, title: string): Promise<void> {
    if (isDesktop()) await invoke('open_panel_window', { panelId, title })
    else
      window.open(
        `${location.pathname}?panel=${panelId}`,
        `ssr-${panelId}`,
        'width=560,height=760',
      )
  },

  async setAlwaysOnTop(enabled: boolean): Promise<void> {
    if (isDesktop()) await invoke('set_always_on_top', { enabled })
  },
}

// ---------------------------------------------------------------------------
// Browser simulator
// ---------------------------------------------------------------------------

const sim = (() => {
  const startedWall = Date.now()
  /** Seconds since the page loaded, which drives every simulated level. */
  const clock = () => (Date.now() - startedWall) / 1000

  let running = false
  let recordingStartedAt = 0
  let config: SessionConfig | null = null
  let sessionDir = ''
  let markers: Marker[] = []
  let generation = 0
  let discoveryRate = 1000

  const stereo = (rate = 48000) => ({
    sampleRate: rate,
    channels: 2,
    bits: 32,
    validBits: 32,
    sampleFormat: '32-bit float',
    channelMask: 0x3,
    layout: 'Stereo',
    channelsDetail: [
      { index: 0, label: 'L', name: 'Front left', maskBit: 0x1 },
      { index: 1, label: 'R', name: 'Front right', maskBit: 0x2 },
    ],
  })

  /** A 5.1 endpoint, so the channel-layout parts of the interface get
   *  exercised rather than only ever seeing a stereo pair. */
  const surround = {
    sampleRate: 48000,
    channels: 6,
    bits: 32,
    validBits: 32,
    sampleFormat: '32-bit float',
    channelMask: 0x60f,
    layout: '5.1',
    channelsDetail: [
      { index: 0, label: 'L', name: 'Front left', maskBit: 0x1 },
      { index: 1, label: 'R', name: 'Front right', maskBit: 0x2 },
      { index: 2, label: 'C', name: 'Front centre', maskBit: 0x4 },
      { index: 3, label: 'LFE', name: 'Low frequency', maskBit: 0x8 },
      { index: 4, label: 'Lss', name: 'Side left', maskBit: 0x200 },
      { index: 5, label: 'Rss', name: 'Side right', maskBit: 0x400 },
    ],
  }

  const devices: DeviceInfo[] = [
    {
      id: '{0.0.0.00000000}.{sim-speakers}',
      name: 'Speakers (Senary Audio)',
      kind: 'render',
      isDefault: true,
      formFactor: 'Speakers',
      adapter: 'Senary High Definition Audio',
      format: stereo(),
    },
    {
      id: '{0.0.0.00000000}.{sim-hdmi}',
      name: 'DELL U2723QE (HDMI)',
      kind: 'render',
      isDefault: false,
      formFactor: 'HDMI or DisplayPort',
      adapter: 'Intel Display Audio',
      format: surround,
    },
    {
      id: '{0.0.0.00000000}.{sim-headset}',
      name: 'Headset Earphone (WH-1000XM5)',
      kind: 'render',
      isDefault: false,
      formFactor: 'Headset',
      adapter: 'Bluetooth Hands-free Audio',
      format: stereo(44100),
    },
    {
      id: '{0.0.1.00000000}.{sim-mic}',
      name: 'Microphone Array (Smart Sound)',
      kind: 'capture',
      isDefault: true,
      formFactor: 'Microphone',
      adapter: 'Intel Smart Sound Technology',
      format: stereo(),
    },
    {
      id: '{0.0.1.00000000}.{sim-headset-mic}',
      name: 'Headset Microphone (WH-1000XM5)',
      kind: 'capture',
      isDefault: false,
      formFactor: 'Headset',
      adapter: 'Bluetooth Hands-free Audio',
      format: { ...stereo(16000), channels: 1, layout: 'Mono', channelMask: 0x4,
        channelsDetail: [{ index: 0, label: 'C', name: 'Front centre', maskBit: 0x4 }] },
    },
  ]

  /** The processes the simulated machine is running, with how they behave. */
  const cast = [
    {
      exe: 'wfica32.exe',
      label: 'Citrix HDX session',
      category: 'remote' as const,
      pids: [31728],
      recommended: true,
      endpoint: 0,
      // Speech: mostly present, with pauses.
      voice: (t: number) => 0.55 + 0.3 * Math.sin(t * 1.7) * Math.sin(t * 0.31),
    },
    {
      exe: 'ms-teams.exe',
      label: 'Microsoft Teams',
      category: 'conferencing' as const,
      pids: [30980, 12004],
      recommended: true,
      endpoint: 0,
      voice: (t: number) => 0.45 + 0.35 * Math.sin(t * 2.3 + 1),
    },
    {
      exe: 'msrdc.exe',
      label: 'WSL / WSLg',
      category: 'remote' as const,
      pids: [28316],
      recommended: true,
      endpoint: 0,
      // Notification sounds: silent, then a short burst.
      voice: (t: number) => (t % 23 < 0.8 ? 0.7 : 0),
    },
    {
      exe: 'chrome.exe',
      label: 'Google Chrome',
      category: 'browser' as const,
      pids: [31556, 28272, 29668],
      recommended: false,
      endpoint: 1,
      // Music: loud and steady, with an occasional transient that reaches the
      // ceiling, so the clip indicator has something real to catch without
      // every meter in the window sitting on the stop the whole time.
      voice: (t: number) => 0.7 + 0.2 * Math.sin(t * 5.1) + (t % 31 < 0.4 ? 0.2 : 0),
    },
    {
      exe: 'Spotify.exe',
      label: 'Spotify',
      category: 'media' as const,
      pids: [21044],
      recommended: false,
      endpoint: 0,
      voice: () => 0,
    },
    {
      exe: 'slack.exe',
      label: 'Slack',
      category: 'chat' as const,
      pids: [9012],
      recommended: false,
      endpoint: 0,
      voice: () => 0,
    },
  ]

  const apps: KnownApp[] = cast.map((c) => ({
    label: c.label,
    exe: c.exe,
    pids: c.pids,
  }))

  function clamp01(v: number) {
    return Math.max(0, Math.min(1, v))
  }

  function discover(): Discovery {
    const t = clock()
    generation += 1

    const sessions: AudioSessionInfo[] = []
    for (const c of cast) {
      const device = devices[c.endpoint] ?? devices[0]!
      const level = clamp01(c.voice(t))
      const channels = device.format?.channels ?? 2
      const active = level > 0.001
      sessions.push({
        key: `sim-${c.exe}-render`,
        pid: c.pids[0]!,
        exe: c.exe,
        label: c.label,
        category: c.category,
        displayName: null,
        endpointId: device.id,
        endpointName: device.name,
        endpointKind: 'render',
        state: active ? 'active' : 'inactive',
        peak: level,
        // Slightly different per channel, the way real material is.
        channelPeaks: Array.from({ length: channels }, (_, i) =>
          clamp01(level * (1 - i * 0.06) * (i === 3 ? 0.35 : 1)),
        ),
        volume: 1,
        muted: false,
        isSystemSounds: false,
        known: true,
        recommended: c.recommended,
      })
    }

    // Teams is also holding the microphone, which is how the interface can
    // say which application is listening.
    const micLevel = clamp01(0.2 + 0.18 * Math.sin(t * 3.4))
    sessions.push({
      key: 'sim-teams-capture',
      pid: 30980,
      exe: 'ms-teams.exe',
      label: 'Microsoft Teams',
      category: 'conferencing',
      displayName: null,
      endpointId: devices[3]!.id,
      endpointName: devices[3]!.name,
      endpointKind: 'capture',
      state: 'active',
      peak: micLevel,
      channelPeaks: [micLevel, micLevel * 0.94],
      volume: 1,
      muted: false,
      isSystemSounds: false,
      known: true,
      recommended: true,
    })

    // The Windows notification sounds session, which exists on every machine
    // and is deliberately not offered as a recordable source.
    sessions.push({
      key: 'sim-system-sounds',
      pid: 0,
      exe: '',
      label: 'Windows system sounds',
      category: 'system',
      displayName: null,
      endpointId: devices[0]!.id,
      endpointName: devices[0]!.name,
      endpointKind: 'render',
      state: 'inactive',
      peak: 0,
      channelPeaks: [0, 0],
      volume: 1,
      muted: false,
      isSystemSounds: true,
      known: true,
      recommended: false,
    })

    const sources = cast.map((c) => {
      const mine = sessions.filter((s) => s.exe === c.exe)
      const peak = mine.reduce((m, s) => Math.max(m, s.peak), 0)
      return {
        exe: c.exe,
        label: c.label,
        category: c.category,
        pids: c.pids,
        sessions: mine.length,
        activeSessions: mine.filter((s) => s.state === 'active').length,
        peak,
        playing: mine.some((s) => s.state === 'active' && s.endpointKind === 'render'),
        capturing: mine.some((s) => s.state === 'active' && s.endpointKind === 'capture'),
        known: true,
        recommended: c.recommended,
        endpoints: [...new Set(mine.map((s) => s.endpointName))],
      }
    })

    sources.sort(
      (a, b) =>
        Number(b.recommended) - Number(a.recommended) ||
        Number(b.playing) - Number(a.playing) ||
        b.peak - a.peak ||
        a.label.localeCompare(b.label),
    )
    sessions.sort((a, b) => b.peak - a.peak || a.label.localeCompare(b.label))

    return {
      devices,
      sessions,
      sources,
      // Roughly what the real engine measures for a refresh pass.
      elapsedMs: 2.4 + Math.random() * 1.6,
      generation,
      error: null,
    }
  }

  function makeSessionDir(output: SessionConfig['output']): string {
    if (output.kind === 'file' || output.kind === 'directory') return output.path
    const d = new Date()
    const pad = (n: number) => String(n).padStart(2, '0')
    const stamp =
      `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}` +
      `_${pad(d.getHours())}-${pad(d.getMinutes())}-${pad(d.getSeconds())}`
    return `${output.root}\\${stamp}`
  }

  function describe(source: SessionConfig['sources'][number]): string {
    if (source.kind === 'system') return `loopback on ${source.device}`
    if (source.kind === 'input') return `input from ${source.device}`
    return source.exclude
      ? `everything except process tree ${source.target}`
      : `process tree ${source.target}`
  }

  function trackName(source: SessionConfig['sources'][number], i: number): string {
    if (source.label) return source.label
    if (source.kind === 'system') return 'system'
    if (source.kind === 'input') return 'mic'
    const stem = source.target.replace(/\.exe$/i, '')
    return source.exclude ? `except-${stem}` : stem || `track-${i + 1}`
  }

  /** The level a planned source would be producing, taken from the same model
   *  discovery uses, so the meters agree with the discovery panel. */
  function levelFor(source: SessionConfig['sources'][number], t: number): number {
    if (source.kind === 'input') return clamp01(0.2 + 0.18 * Math.sin(t * 3.4))
    if (source.kind === 'system') {
      // The mix is everything playing to the default endpoint at once.
      // Combined the way uncorrelated signals actually combine, which
      // saturates towards full scale rather than adding straight past it: a
      // linear sum has three quiet sources clipping the mix, which is not what
      // a real endpoint does and would make the demo meters useless.
      const remaining = cast
        .filter((c) => c.endpoint === 0)
        .reduce((product, c) => product * (1 - 0.85 * clamp01(c.voice(t))), 1)
      return clamp01(1 - remaining)
    }
    const found = cast.find((c) => c.exe.toLowerCase() === source.target.toLowerCase())
    return found ? clamp01(found.voice(t)) : clamp01(0.3 + 0.25 * Math.sin(t * 2))
  }

  function snapshot(): SessionSnapshot {
    if (!running || !config) {
      return {
        state: 'finished',
        elapsedSecs: 0,
        outputDir: '',
        tracks: [],
        warnings: [],
        markers: [],
      }
    }
    const elapsed = (Date.now() - recordingStartedAt) / 1000
    const t = clock()
    return {
      state: 'recording',
      elapsedSecs: elapsed,
      outputDir: sessionDir,
      warnings: [],
      markers,
      tracks: config.sources.map((s, i) => {
        const peak = levelFor(s, t)
        const db = peak <= 0.000001 ? -120 : 20 * Math.log10(peak)
        const rms = peak <= 0.000001 ? -120 : db - 7 - 3 * Math.abs(Math.sin(t * 0.9 + i))
        const silent = peak <= 0.000001
        return {
          name: trackName(s, i),
          description: describe(s),
          file: `${trackName(s, i)}.wav`,
          sampleRate: 48000,
          channels: 2,
          durationSecs: elapsed,
          peak,
          peakDb: db,
          rmsDb: rms,
          framesCaptured: Math.floor(elapsed * 48000 * (silent ? 0.1 : 0.94)),
          framesSilence: Math.floor(elapsed * 48000 * (silent ? 0.9 : 0.06)),
          discontinuities: i === 1 && elapsed > 25 ? 1 : 0,
        }
      }),
    }
  }

  /** Two finished sessions and one that was interrupted, so the library panel
   *  shows both states it has to distinguish between. */
  function librarySessions(): LibrarySession[] {
    return [
      {
        dir: 'C:\\Users\\you\\Documents\\System Sound Recorder\\2026-09-07_09-42-11',
        name: '2026-09-07_09-42-11',
        startedUtc: '2026-09-07T07:42:11Z',
        durationSecs: 2714.4,
        sizeBytes: 1_042_800_000,
        complete: true,
        warnings: ['the headset was disconnected once during the session'],
        markers: [
          { at_secs: 312.5, label: 'pricing question' },
          { at_secs: 1580.0, label: 'action items' },
        ],
        modifiedUnix: 1788053000,
        tracks: [
          {
            name: 'system',
            file: 'system.wav',
            sizeBytes: 521_400_000,
            durationSecs: 2714.4,
            sampleRate: 48000,
            channels: 2,
            source: 'loopback on default',
            silenceFraction: 0.31,
            discontinuities: 0,
          },
          {
            name: 'wfica32',
            file: 'wfica32.wav',
            sizeBytes: 521_400_000,
            durationSecs: 2714.4,
            sampleRate: 48000,
            channels: 2,
            source: 'process tree wfica32.exe',
            silenceFraction: 0.44,
            discontinuities: 2,
          },
        ],
      },
      {
        dir: 'C:\\Users\\you\\Documents\\System Sound Recorder\\2026-09-06_16-08-55',
        name: '2026-09-06_16-08-55',
        startedUtc: '2026-09-06T14:08:55Z',
        durationSecs: 611.2,
        sizeBytes: 234_700_000,
        complete: true,
        warnings: [],
        markers: [],
        modifiedUnix: 1787977000,
        tracks: [
          {
            name: 'system',
            file: 'system.wav',
            sizeBytes: 117_350_000,
            durationSecs: 611.2,
            sampleRate: 48000,
            channels: 2,
            source: 'loopback on default',
            silenceFraction: 0.06,
            discontinuities: 0,
          },
          {
            name: 'mic',
            file: 'mic.wav',
            sizeBytes: 117_350_000,
            durationSecs: 611.2,
            sampleRate: 48000,
            channels: 2,
            source: 'input from default',
            silenceFraction: 0.02,
            discontinuities: 0,
          },
        ],
      },
      {
        dir: 'C:\\Users\\you\\Documents\\System Sound Recorder\\2026-09-05_11-20-03',
        name: '2026-09-05_11-20-03',
        startedUtc: null,
        durationSecs: null,
        sizeBytes: 48_200_000,
        complete: false,
        problem:
          'no session.json: the recording was interrupted before it could be finished',
        warnings: [],
        markers: [],
        modifiedUnix: 1787836000,
        tracks: [
          {
            name: 'system',
            file: 'system.wav',
            sizeBytes: 48_200_000,
            durationSecs: null,
            sampleRate: 0,
            channels: 0,
            source: null,
            silenceFraction: null,
            discontinuities: 0,
          },
        ],
      },
    ]
  }

  return {
    async listDevices() {
      return devices
    },
    async listApps() {
      return apps
    },
    async listProcesses(): Promise<ProcessInfo[]> {
      return cast.flatMap((c) =>
        c.pids.map((pid) => ({ pid, parent_pid: 0, name: c.exe })),
      )
    },
    async discover() {
      return discover()
    },
    async rescan() {
      return discover()
    },
    setDiscoveryRate(ms: number) {
      discoveryRate = ms
      void discoveryRate
    },
    async start(c: SessionConfig) {
      config = c
      sessionDir = makeSessionDir(c.output)
      running = true
      recordingStartedAt = Date.now()
      markers = []
      return snapshot()
    },
    async stop(): Promise<SessionManifest> {
      const final = snapshot()
      running = false
      return {
        tool: 'system-sound-recorder',
        version: '0.1.0',
        started_utc: new Date(recordingStartedAt).toISOString().replace(/\.\d+Z$/, 'Z'),
        ended_utc: new Date().toISOString().replace(/\.\d+Z$/, 'Z'),
        duration_secs: final.elapsedSecs,
        session_t0_qpc_100ns: 0,
        markers,
        tracks: final.tracks.map((t) => ({
          name: t.name,
          file: t.file,
          source: t.description,
          sample_rate: t.sampleRate,
          channels: t.channels,
          duration_secs: t.durationSecs,
          stats: {
            frames_captured: t.framesCaptured,
            frames_silence_filled: t.framesSilence,
            frames_overlap_trimmed: 0,
            blocks_dropped: 0,
            discontinuities: t.discontinuities,
            untimed_blocks: 0,
            lookbehind_growths: 0,
            longest_gap_ms: 0,
          },
        })),
      }
    },
    async snapshot() {
      return snapshot()
    },
    async addMarker(label: string): Promise<Marker | null> {
      if (!running) return null
      const marker = { at_secs: (Date.now() - recordingStartedAt) / 1000, label }
      markers = [...markers, marker]
      return marker
    },
    async listSessions() {
      return librarySessions()
    },
    async diskSpace(): Promise<DiskSpace> {
      // A drive with enough room to be interesting but not enough to ignore,
      // draining slowly while a session runs.
      const used = running ? ((Date.now() - recordingStartedAt) / 1000) * 384_000 : 0
      return {
        freeBytes: Math.max(0, 41_600_000_000 - used),
        totalBytes: 510_000_000_000,
      }
    },
  }
})()
