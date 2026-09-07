// The conversation, as it is being transcribed.
//
// Laid out as a chat rather than as a transcript, because that is what it is:
// lines arriving one at a time while people talk. Reading who said what down
// the left is the whole point of having recorded the channels separately.

import { useEffect, useMemo, useRef, useState } from 'react'
import { IconDownload, IconMessage, IconSave, IconStop, IconWarning } from '../components/Icons'
import { Switch } from '../components/primitives'
import { bytes } from '../lib/format'
import { api } from '../lib/api'
import { LANGUAGES, transcribeApi, type Line } from '../lib/transcribe'
import type { KnownApp, SourceRequest } from '../lib/types'
import { isListening, useConversation } from '../state/conversation'

/** The channels worth offering, in the order someone would look for them. */
function useChannels() {
  const [apps, setApps] = useState<KnownApp[]>([])
  useEffect(() => {
    let cancelled = false
    void api
      .listApps()
      .then((found) => {
        if (!cancelled) setApps(found)
      })
      .catch(() => {})
    return () => {
      cancelled = true
    }
  }, [])

  return useMemo(() => {
    const options: { key: string; label: string; request: SourceRequest }[] = [
      {
        key: 'system',
        label: 'Everything this machine plays',
        request: { kind: 'system', device: 'default', label: null },
      },
    ]
    for (const app of apps) {
      options.push({
        key: `app:${app.exe}`,
        label: app.label,
        request: { kind: 'app', target: app.exe, exclude: false, label: null },
      })
    }
    options.push({
      key: 'mic',
      label: 'Microphone',
      request: { kind: 'input', device: 'default', label: null },
    })
    return options
  }, [apps])
}

/** `M:SS`, or `H:MM:SS` once a meeting runs long. */
function clock(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds))
  const m = Math.floor(total / 60)
  const s = total % 60
  if (total >= 3600) {
    const h = Math.floor(total / 3600)
    return `${h}:${String(m % 60).padStart(2, '0')}:${String(s).padStart(2, '0')}`
  }
  return `${m}:${String(s).padStart(2, '0')}`
}

/** A stable colour per speaker, so the eye can follow one voice down the page. */
function speakerHue(label: string): number {
  let hash = 0
  for (let i = 0; i < label.length; i += 1) {
    hash = (hash * 31 + label.charCodeAt(i)) % 360
  }
  return hash
}

function Bubble({ line, continued }: { line: Line; continued: boolean }) {
  const hue = speakerHue(line.speaker.label)
  return (
    <div className={`chat-line${continued ? ' chat-line--continued' : ''}`}>
      {!continued && (
        <div className="chat-line__who">
          <span
            className="chat-line__dot"
            style={{ background: `hsl(${hue} 65% 58%)` }}
            aria-hidden="true"
          />
          <span className="chat-line__speaker">{line.speaker.label}</span>
          {line.speaker.inferred && (
            <span className="badge" title="Grouped by how the voice sounds, which is a guess">
              inferred
            </span>
          )}
          <span className="chat-line__time">{clock(line.start)}</span>
        </div>
      )}
      <p className="chat-line__text">{line.text}</p>
    </div>
  )
}

export function ConversationPanel() {
  const lines = useConversation((s) => s.lines)
  const status = useConversation((s) => s.status)
  const engines = useConversation((s) => s.engines)
  const error = useConversation((s) => s.error)
  const busy = useConversation((s) => s.busy)
  const target = useConversation((s) => s.target)
  const targetLabel = useConversation((s) => s.targetLabel)
  const detectSpeakers = useConversation((s) => s.detectSpeakers)
  const setDetectSpeakers = useConversation((s) => s.setDetectSpeakers)
  const chooseTarget = useConversation((s) => s.chooseTarget)
  const channels = useChannels()
  const language = useConversation((s) => s.language)
  const setLanguage = useConversation((s) => s.setLanguage)
  const model = useConversation((s) => s.model)
  const setModel = useConversation((s) => s.setModel)
  const models = useConversation((s) => s.models)
  const download = useConversation((s) => s.download)
  const loadModels = useConversation((s) => s.loadModels)
  const fetchModel = useConversation((s) => s.fetchModel)
  const pollDownload = useConversation((s) => s.pollDownload)
  const start = useConversation((s) => s.start)
  const stop = useConversation((s) => s.stop)
  const openSave = useConversation((s) => s.openSave)
  const loadEngines = useConversation((s) => s.loadEngines)

  const bottom = useRef<HTMLDivElement>(null)
  const live = isListening(status)
  const selected = models.find((m) => m.id === model)

  useEffect(() => {
    if (!engines) void loadEngines()
  }, [engines, loadEngines])

  useEffect(() => {
    if (models.length === 0) void loadModels()
  }, [models.length, loadModels])

  useEffect(() => {
    if (!download?.active) return
    const handle = window.setInterval(() => void pollDownload(), 500)
    return () => window.clearInterval(handle)
  }, [download?.active, pollDownload])

  useEffect(() => {
    // Follow the conversation, the way a chat window does.
    bottom.current?.scrollIntoView({ block: 'end', behavior: 'smooth' })
  }, [lines.length])

  if (engines && !engines.ready) {
    return (
      <div className="panel">
        <div className="panel__body panel__body--pad">
          <div className="notice notice--warn">
            <IconWarning tone="warn" size="lg" />
            <div>
              <strong>No speech engine installed</strong>
              <p>
                Recognition runs on this machine, so nothing said in a meeting has
                to leave it. That means an engine has to be installed first.
              </p>
              <pre className="code">{engines.advice}</pre>
              <button type="button" className="btn btn--sm" onClick={() => void loadEngines()}>
                Look again
              </button>
            </div>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="panel">
      <div className="panel__toolbar">
        {live ? (
          <button
            type="button"
            className="btn"
            onClick={() => void stop()}
            disabled={busy}
            title="Stop reading this channel. The recording, if one is running, carries on."
          >
            <IconStop /> Stop listening
          </button>
        ) : (
          <button
            type="button"
            className="btn btn--primary"
            onClick={() => void start()}
            disabled={busy || !target}
            title={
              target
                ? `Read ${targetLabel} as it is spoken`
                : 'Choose a channel here, or press the transcribe button on a row in Discovery'
            }
          >
            <IconMessage /> Listen
          </button>
        )}

        <select
          className="select"
          style={{ maxWidth: 220 }}
          disabled={live}
          value={targetLabel ? channels.find((c) => c.label === targetLabel)?.key ?? '' : ''}
          onChange={(e) => {
            const chosen = channels.find((c) => c.key === e.target.value)
            if (chosen) chooseTarget(chosen.request, chosen.label)
          }}
        >
          <option value="" disabled>
            Choose a channel…
          </option>
          {channels.map((c) => (
            <option key={c.key} value={c.key}>
              {c.label}
            </option>
          ))}
        </select>

        <span className="chat-status">
          {live ? (
            <>
              <span className="dot dot--live" /> {status.channel} · {clock(status.elapsedSecs)}
              {status.backlog > 0 && (
                <span className="badge badge--warn" title="Utterances heard but not yet recognised">
                  {status.backlog} behind
                </span>
              )}
            </>
          ) : (
            <>{target ? `Ready to listen to ${targetLabel}` : 'No channel chosen'}</>
          )}
        </span>

        <div className="panel__spacer" />

        <Switch
          checked={detectSpeakers}
          onChange={setDetectSpeakers}
          disabled={live}
          label="Speakers"
          title="Try to tell voices apart within this channel. Between channels the answer is exact; within one it is a guess."
        />

        <button
          type="button"
          className="btn btn--sm"
          disabled={lines.length === 0}
          onClick={openSave}
        >
          <IconSave /> Save…
        </button>
      </div>

      <div className="chat-settings">
        <label className="chat-settings__field">
          <span>Language</span>
          <select
            className="select"
            value={language}
            disabled={live}
            onChange={(e) => setLanguage(e.target.value)}
            title="Naming the language is faster as well as more accurate: detection costs an extra pass over every utterance."
          >
            {LANGUAGES.map((l) => (
              <option key={l.id} value={l.id}>
                {l.label}
              </option>
            ))}
          </select>
        </label>

        <label className="chat-settings__field">
          <span>Model</span>
          <select
            className="select"
            value={model}
            disabled={live}
            onChange={(e) => setModel(e.target.value)}
            title="Bigger models are much better outside English, and slower."
          >
            {models.map((m) => (
              <option key={m.id} value={m.id}>
                {m.label} — {m.hint} {m.cached ? '· ready' : `· ${bytes(m.approximateBytes)} to fetch`}
              </option>
            ))}
          </select>
        </label>

        {selected && !selected.cached && !download?.active && (
          <button
            type="button"
            className="btn btn--sm"
            disabled={live}
            onClick={() => void fetchModel(selected.id)}
            title="Fetch it now, rather than waiting for it to happen on the first thing anybody says"
          >
            <IconDownload /> Get {selected.label} ({bytes(selected.approximateBytes)})
          </button>
        )}

        {download?.active && (
          <span className="model-progress" title={`Fetching ${download.model}`}>
            <span className="model-progress__track">
              <span
                className="model-progress__fill"
                style={{ width: `${Math.round(download.fraction * 100)}%` }}
              />
            </span>
            {Math.round(download.fraction * 100)}%
            {download.total > 0 && ` of ${bytes(download.total)}`}
          </span>
        )}

        <div className="panel__spacer" />

        {engines?.engines[0] && (
          <span
            className="chat-settings__engine"
            title={
              engines.engines[0].interpreter
                ? 'The engine stays loaded between utterances, so only the first one waits for the model.'
                : 'The engine is restarted for every utterance, which costs several seconds each time.'
            }
          >
            <span
              className={`dot ${engines.engines[0].interpreter ? 'dot--ok' : 'dot--warn'}`}
            />
            {engines.engines[0].label}
            {engines.engines[0].interpreter ? ' · held open' : ' · restarted each time'}
          </span>
        )}
      </div>

      {error && <div className="notice notice--error">{error}</div>}

      <div className="panel__body chat">
        {lines.length === 0 ? (
          <div className="panel__empty">
            {live
              ? 'Listening. Lines appear here as people speak.'
              : 'Choose a channel and press Listen, or use the transcribe button on any row in Discovery. Reading a channel is independent of recording: either, both, or neither.'}
          </div>
        ) : (
          lines.map((line, i) => (
            <Bubble
              key={`${line.index}-${i}`}
              line={line}
              continued={i > 0 && lines[i - 1]?.speaker.label === line.speaker.label}
            />
          ))
        )}
        <div ref={bottom} />
      </div>

      {status.transcriptPath && (
        <div className="chat-footer" title={status.transcriptPath}>
          Writing to {status.transcriptPath.split(/[\\/]/).pop()}
          {status.speakers > 1 && ` · ${status.speakers} voices`}
        </div>
      )}
    </div>
  )
}

/** Poll for new lines while a conversation is being transcribed. */
export function useConversationPolling(intervalMs = 400) {
  const active = useConversation((s) => s.status.active)
  const poll = useConversation((s) => s.poll)

  useEffect(() => {
    if (!active) return
    // Slower than the meter polling: lines arrive every few seconds at most, so
    // asking ten times a second would be pure waste.
    const handle = window.setInterval(() => void poll(), intervalMs)
    return () => window.clearInterval(handle)
  }, [active, poll, intervalMs])
}

/** Track names in a session, for the batch mode. Exposed here so the panel and
 *  the dialog share one way of asking. */
export const sessionTracks = transcribeApi.sessionTracks
