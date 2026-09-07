// Custom window chrome.
//
// The shell hides the native frame so the title bar can carry the workspace
// menus and read as one piece with the rest of the interface. That means the
// window controls have to be provided here.

import { useEffect, useState } from 'react'
import type { DockviewApi } from 'dockview'
import { isDesktop } from '../lib/api'
import { LayoutMenu, PanelsMenu } from './Menus'
import { AboutMenu, SettingsMenu } from './SettingsMenu'
import {
  AppMark,
  IconClose,
  IconMaximize,
  IconMinimize,
  IconRestore,
} from './Icons'
import { isRecording, useStore } from '../state/store'
import { VERSION } from '../lib/version'
import { plural } from '../lib/format'

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
      <div className="titlebar__brand" data-tauri-drag-region>
        <AppMark className="titlebar__mark" />
        <span className="titlebar__title">System Sound Recorder</span>
        <span className="titlebar__version" title={`Version ${VERSION}`}>
          {VERSION}
        </span>
      </div>

      <span className="titlebar__rule" />

      {/* What the window is currently for. The biggest switch in the program,
          so it gets a zone of its own rather than sitting among the menus. */}
      {extra && (
        <>
          <div className="titlebar__group">{extra}</div>
          <span className="titlebar__rule" />
        </>
      )}

      {/* What can be rearranged. */}
      <div className="titlebar__group">
        <LayoutMenu api={api} />
        <PanelsMenu api={api} />
      </div>

      <div className="titlebar__spacer" data-tauri-drag-region />

      <div className="titlebar__group">
        <span
          className={`titlebar__status${live ? ' titlebar__status--live' : ''}`}
          title={
            live
              ? `Recording ${plural(snapshot?.tracks.length ?? 0, 'track')}`
              : 'No session is running'
          }
        >
          <span className={`dot${live ? ' dot--live' : ' dot--ok'}`} />
          {live ? 'Recording' : 'Idle'}
        </span>
      </div>

      <span className="titlebar__rule" />

      {/* Utilities. Settings is a menu rather than a panel: it is somewhere
          you go for a few seconds, and everything in it applies immediately.
          The keyboard reference lives inside it. */}
      <div className="titlebar__group">
        <SettingsMenu onShowShortcuts={onShowShortcuts} />
        <AboutMenu />
      </div>

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
