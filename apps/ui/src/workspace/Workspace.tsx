// The dockable workspace.
//
// dockview provides the docking; this wires the panels into it and keeps the
// arrangement saved. Panels can be dragged between groups, tabbed together,
// floated over the window, maximised, or torn off into a window of their own
// on a second display — which is the arrangement this program was built for,
// with the mixer on one screen and the work on the other.

import { useCallback, useEffect, useMemo, useRef } from 'react'
import {
  DockviewReact,
  type DockviewApi,
  type DockviewReadyEvent,
  type IDockviewHeaderActionsProps,
  type IDockviewPanelProps,
} from 'dockview'
import { api as backend, isDesktop } from '../lib/api'
import { PANELS, PANELS_BY_ID } from '../panels/registry'
import { Empty } from '../components/primitives'
import { IconFloat, IconLayout, IconPopOut } from '../components/Icons'
import { restoreLayout, saveLayout } from './layout'

interface Props {
  /** Called once the workspace is live, so the shell can drive it. */
  onApi: (api: DockviewApi) => void
}

/** Open one panel in its own operating system window. */
async function popOut(panelId: string): Promise<void> {
  const definition = PANELS_BY_ID[panelId]
  if (!definition) return
  if (isDesktop()) {
    await backend.openPanelWindow(panelId, definition.title)
    return
  }
  window.open(`${location.pathname}?panel=${panelId}`, `ssr-${panelId}`, 'width=560,height=760')
}

function HeaderActions({ activePanel, containerApi }: IDockviewHeaderActionsProps) {
  if (!activePanel) return null
  return (
    <div className="dv-tab-actions">
      <button
        type="button"
        className="btn btn--ghost btn--sm btn--icon"
        title="Float this panel over the window"
        onClick={() => containerApi.addFloatingGroup(activePanel)}
      >
        <IconFloat size={12} />
      </button>
      <button
        type="button"
        className="btn btn--ghost btn--sm btn--icon"
        title="Move this panel to a window of its own, for a second screen"
        onClick={() => void popOut(activePanel.id)}
      >
        <IconPopOut size={12} />
      </button>
    </div>
  )
}

/** What is shown when every panel has been closed. */
function Watermark() {
  return (
    <div className="panel">
      <Empty icon={IconLayout} title="No panels open">
        Choose an arrangement from the Layout menu in the title bar, or add a
        single panel from the Panels menu next to it.
      </Empty>
    </div>
  )
}

export function Workspace({ onApi }: Props) {
  const apiRef = useRef<DockviewApi | null>(null)
  const disposeRef = useRef<{ dispose(): void } | null>(null)

  const components = useMemo(() => {
    const map: Record<string, React.FunctionComponent<IDockviewPanelProps>> = {}
    for (const panel of PANELS) {
      const Body = panel.component
      map[panel.id] = () => <Body />
    }
    return map
  }, [])

  const onReady = useCallback(
    (event: DockviewReadyEvent) => {
      apiRef.current = event.api
      restoreLayout(event.api)

      // Subscribe here rather than in an effect. An effect runs before this
      // callback has assigned the api, so it would silently subscribe to
      // nothing and no layout would ever be saved.
      disposeRef.current = event.api.onDidLayoutChange(() => saveLayout(event.api))

      onApi(event.api)
    },
    [onApi],
  )

  useEffect(() => () => disposeRef.current?.dispose(), [])

  return (
    <DockviewReact
      className="app-workspace dockview-theme-abyss dockview-theme-ssr"
      components={components}
      rightHeaderActionsComponent={HeaderActions}
      watermarkComponent={Watermark}
      onReady={onReady}
    />
  )
}
