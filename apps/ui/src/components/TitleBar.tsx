// Custom window chrome.
//
// The shell hides the native frame so the title bar can carry the workspace
// menus and read as one piece with the rest of the interface. That means the
// window controls have to be provided here.

import { useEffect, useState } from 'react'
import type { DockviewApi } from 'dockview'
import { isDesktop } from '../lib/api'
import { LayoutMenu, PanelsMenu } from './Menus'
import {
  AppMark,
  IconClose,
  IconKeyboard,
  IconMaximize,
  IconMinimize,
  IconRestore,
} from './Icons'
import { isRecording, useStore } from '../state/store'

interface Props {
  api: DockviewApi | null
  onShowShortcuts: () => void
  /** Placed in the title bar beside the menus. The mode switch lives here
   *  because it changes what the whole window is for, which is a bigger thing
   *  than anything on the transport below it. */
  extra?: React.ReactNode
}

type Win = {
  minimize(): Promise<void>
  toggleMaximize(): Promise<void>
  close(): Promise<void>
  isMaximized(): Promise<boolean>
}

async function currentWindow(): Promise<Win | null> {
  if (!isDesktop()) return null
  const mod = await import('@tauri-apps/api/window')
  return mod.getCurrentWindow() as unknown as Win
}

export function TitleBar({ api, onShowShortcuts, extra }: Props) {
  const [maximized, setMaximized] = useState(false)
  const snapshot = useStore((s) => s.snapshot)
  const live = isRecording(snapshot)

  useEffect(() => {
    let cancelled = false
    void currentWindow().then(async (w) => {
      if (!w || cancelled) return
      setMaximized(await w.isMaximized())
    })
    return () => {
      cancelled = true
    }
  }, [])

  const act = async (fn: (w: Win) => Promise<void>) => {
    const w = await currentWindow()
    if (!w) return
    await fn(w)
    setMaximized(await w.isMaximized())
  }

  return (
    <header className="titlebar" data-tauri-drag-region>
      <AppMark className="titlebar__mark" />
      <span className="titlebar__title">System Sound Recorder</span>
      {live ? (
        <span className="badge badge--record">
          <span className="dot dot--live" /> recording
        </span>
      ) : (
        <span className="titlebar__subtitle">
          Citrix, WSL and every other application on this machine
        </span>
      )}

      <div className="titlebar__menus">
        <LayoutMenu api={api} />
        <PanelsMenu api={api} />
        <button
          type="button"
          className="btn btn--ghost btn--sm btn--icon"
          title="Keyboard shortcuts"
          onClick={onShowShortcuts}
        >
          <IconKeyboard size={13} />
        </button>
      </div>

      {extra}

      <div className="titlebar__spacer" data-tauri-drag-region />

      {isDesktop() && (
        <div className="titlebar__controls">
          <button
            type="button"
            className="titlebar__button"
            aria-label="Minimize"
            onClick={() => void act((w) => w.minimize())}
          >
            <IconMinimize size={14} />
          </button>
          <button
            type="button"
            className="titlebar__button"
            aria-label={maximized ? 'Restore' : 'Maximize'}
            onClick={() => void act((w) => w.toggleMaximize())}
          >
            {maximized ? <IconRestore size={14} /> : <IconMaximize size={14} />}
          </button>
          <button
            type="button"
            className="titlebar__button titlebar__button--close"
            aria-label="Close"
            onClick={() => void act((w) => w.close())}
          >
            <IconClose size={14} />
          </button>
        </div>
      )}
    </header>
  )
}
