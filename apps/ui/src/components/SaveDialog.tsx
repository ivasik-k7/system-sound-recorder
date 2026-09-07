// The save dialog.
//
// Written rather than borrowed from the operating system, because the choices
// here are not the ones a file picker offers. What matters is which formats to
// write and whether the speaker labels come with them; the folder is the least
// interesting part. The native picker is still one button away for choosing
// that folder.

import { useEffect, useMemo, useRef, useState } from 'react'
import { IconCheck, IconFolder, IconSave } from './Icons'
import { Switch } from './primitives'
import { FORMATS, type TranscriptFormat } from '../lib/transcribe'
import { isDesktop } from '../lib/api'
import { useConversation } from '../state/conversation'

/** Ask the shell for a folder, when there is a shell to ask. */
async function pickDirectory(current: string): Promise<string | null> {
  if (!isDesktop()) return null
  try {
    const dialog = await import('@tauri-apps/plugin-dialog')
    const chosen = await dialog.open({
      directory: true,
      multiple: false,
      defaultPath: current || undefined,
      title: 'Where should the transcript go?',
    })
    return typeof chosen === 'string' ? chosen : null
  } catch {
    return null
  }
}

export function SaveDialog() {
  const open = useConversation((s) => s.saving)
  const close = useConversation((s) => s.closeSave)
  const save = useConversation((s) => s.save)
  const status = useConversation((s) => s.status)
  const lines = useConversation((s) => s.lines)

  const [directory, setDirectory] = useState('')
  const [stem, setStem] = useState('')
  const [formats, setFormats] = useState<TranscriptFormat[]>(['md', 'txt'])
  const [includeSpeakers, setIncludeSpeakers] = useState(true)
  const [includeNotes, setIncludeNotes] = useState(true)
  const [result, setResult] = useState<string[] | null>(null)
  const [busy, setBusy] = useState(false)

  const firstField = useRef<HTMLInputElement>(null)

  // Suggest a name from the channel and the day, which is what someone would
  // have typed anyway.
  const suggested = useMemo(() => {
    const channel = status.channel || 'conversation'
    const now = new Date()
    const pad = (n: number) => String(n).padStart(2, '0')
    const stamp = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`
    return `${stamp} ${channel}`
  }, [status.channel])

  useEffect(() => {
    if (!open) return
    setResult(null)
    setStem((s) => s || suggested)
    setDirectory((d) => {
      if (d) return d
      // Default beside the running transcript, which is where someone would
      // look for it.
      const path = status.transcriptPath
      const cut = Math.max(path.lastIndexOf('\\'), path.lastIndexOf('/'))
      return cut > 0 ? path.slice(0, cut) : ''
    })
    // Focus lands on the name, the one field usually worth changing.
    const t = window.setTimeout(() => firstField.current?.select(), 30)
    return () => window.clearTimeout(t)
  }, [open, suggested, status.transcriptPath])

  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') close()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [open, close])

  if (!open) return null

  const toggle = (id: TranscriptFormat) =>
    setFormats((current) =>
      current.includes(id) ? current.filter((f) => f !== id) : [...current, id],
    )

  const canSave =
    (formats.length > 0 || includeNotes) &&
    stem.trim() !== '' &&
    directory.trim() !== '' &&
    !busy

  const onSave = async () => {
    setBusy(true)
    const written = await save(
      directory.trim(),
      stem.trim(),
      formats,
      includeSpeakers,
      includeNotes,
    )
    setBusy(false)
    if (written) setResult(written.files)
  }

  return (
    <div className="modal-scrim" onMouseDown={close} role="presentation">
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-label="Save the transcript"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <header className="modal__head">
          <IconSave size="lg" tone="accent" />
          <div>
            <h2 className="modal__title">Save the transcript</h2>
            <p className="modal__subtitle">
              {lines.length} line{lines.length === 1 ? '' : 's'} from {status.channel || 'this channel'}
            </p>
          </div>
        </header>

        {result ? (
          <div className="modal__body">
            <div className="notice notice--ok">
              <IconCheck tone="ok" size="lg" />
              <div>
                <strong>Written</strong>
                <ul className="modal__files">
                  {result.map((f) => (
                    <li key={f}>{f.split(/[\\/]/).pop()}</li>
                  ))}
                </ul>
              </div>
            </div>
          </div>
        ) : (
          <div className="modal__body">
            <label className="field">
              <span className="field__label">Name</span>
              <input
                ref={firstField}
                className="input"
                value={stem}
                onChange={(e) => setStem(e.target.value)}
                placeholder={suggested}
              />
            </label>

            <label className="field">
              <span className="field__label">Folder</span>
              <div className="field__row">
                <input
                  className="input input--mono"
                  value={directory}
                  onChange={(e) => setDirectory(e.target.value)}
                  placeholder="Choose where to write it"
                />
                <button
                  type="button"
                  className="btn"
                  onClick={async () => {
                    const chosen = await pickDirectory(directory)
                    if (chosen) setDirectory(chosen)
                  }}
                >
                  <IconFolder /> Browse
                </button>
              </div>
            </label>

            <div className="field">
              <span className="field__label">Formats</span>
              <div className="format-grid">
                {FORMATS.map((f) => {
                  const on = formats.includes(f.id)
                  return (
                    <button
                      key={f.id}
                      type="button"
                      className={`format-tile${on ? ' format-tile--on' : ''}`}
                      onClick={() => toggle(f.id)}
                      aria-pressed={on}
                    >
                      <span className="format-tile__check">{on ? <IconCheck size="sm" /> : null}</span>
                      <span className="format-tile__name">{f.label}</span>
                      <span className="format-tile__ext">.{f.id}</span>
                      <span className="format-tile__hint">{f.hint}</span>
                    </button>
                  )
                })}
              </div>
            </div>

            <Switch
              checked={includeSpeakers}
              onChange={setIncludeSpeakers}
              label="Put the speaker in front of each line"
            />

            <Switch
              checked={includeNotes}
              onChange={setIncludeNotes}
              label="Also write a meeting record"
              title="Who spoke, the lines that look like actions or decisions, and the whole transcript underneath. The picking out is done by matching turns of phrase, so it is shown as suggestions with the phrase that matched."
            />
          </div>
        )}

        <footer className="modal__foot">
          {result ? (
            <button type="button" className="btn btn--primary" onClick={close}>
              Done
            </button>
          ) : (
            <>
              <button type="button" className="btn" onClick={close}>
                Cancel
              </button>
              <button
                type="button"
                className="btn btn--primary"
                disabled={!canSave}
                onClick={() => void onSave()}
              >
                <IconSave />{' '}
                {busy
                  ? 'Writing…'
                  : `Write ${formats.length + (includeNotes ? 1 : 0)} file${
                      formats.length + (includeNotes ? 1 : 0) === 1 ? '' : 's'
                    }`}
              </button>
            </>
          )}
        </footer>
      </div>
    </div>
  )
}
