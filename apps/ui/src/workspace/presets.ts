// Named workspace arrangements.
//
// A dockable interface with twelve panels can be arranged in more ways than
// anyone wants to arrange it, and the arrangement that suits setting a session
// up is not the one that suits watching it run for two hours. So the useful
// arrangements are named and one click away, and the docking is there for
// people who want something else.
//
// Each preset also carries a schematic. Layouts are recognised by shape rather
// than by name — a drawn rectangle divided three ways says more than the word
// "Console" does — so the menu shows the shape.

import type { AddPanelPositionOptions, DockviewApi, IDockviewPanel } from 'dockview'
import { PANELS_BY_ID } from '../panels/registry'

export interface LayoutPreset {
  id: string
  name: string
  description: string
  /**
   * A grid schematic, as CSS grid-template-areas rows. One letter per region.
   */
  sketch: string[]
  /** Regions drawn in the accent colour: the panel the layout is built around. */
  accent: string[]
  /** Panels this arrangement opens, for the palette and the menu. */
  panels: string[]
  build: (api: DockviewApi) => void
}

/** The subset of dockview's panel options these presets use. */
interface Placement {
  position?: AddPanelPositionOptions
  initialWidth?: number
  initialHeight?: number
  /** Add it without making it the active tab of its group. */
  inactive?: boolean
}

/** Add a panel, taking its title from the registry so the two cannot drift. */
function add(api: DockviewApi, id: string, options: Placement = {}): IDockviewPanel {
  const definition = PANELS_BY_ID[id]
  return api.addPanel({
    id,
    component: id,
    title: definition?.title ?? id,
    ...options,
  })
}

/**
 * Set the proportions once the grid has settled.
 *
 * `initialWidth` on `addPanel` is a hint the grid does not always honour: each
 * insertion re-divides the space, and the column added last can end up
 * collapsed to nothing. Assigning sizes after the layout pass is reliable, and
 * it has to happen one frame later for the same reason.
 */
function proportion(api: DockviewApi, plan: { id: string; width?: number; height?: number }[]) {
  const apply = () => {
    // On a narrow window, forcing widths would leave nothing for the middle,
    // so let the grid divide the space evenly instead.
    const width = api.width || window.innerWidth
    for (const item of plan) {
      const panel = api.getPanel(item.id)
      if (!panel) continue
      if (item.height !== undefined) {
        panel.group.api.setSize({
          height: Math.min(item.height, Math.round(api.height * 0.45)),
        })
      }
      if (item.width !== undefined && width >= 1000) {
        panel.group.api.setSize({ width: item.width })
      }
    }
  }
  if (typeof requestAnimationFrame === 'function') requestAnimationFrame(apply)
  else apply()
}

export const LAYOUT_PRESETS: LayoutPreset[] = [
  {
    id: 'transcribe',
    name: 'Transcribe',
    description:
      'A conversation being read as it is spoken, with the channels beside it and the activity below.',
    sketch: ['d c c', 'd c c', 'l l l'],
    accent: ['c'],
    panels: ['conversation', 'discovery', 'log'],
    build(api) {
      const conversation = add(api, 'conversation')

      // Discovery on the left, because choosing which channel to listen to is
      // the one decision that has to be made before anything happens.
      add(api, 'discovery', {
        position: { referencePanel: conversation, direction: 'left' },
        initialWidth: 380,
      })

      add(api, 'log', {
        position: { direction: 'below' },
        initialHeight: 180,
      })

      conversation.api.setActive()
      proportion(api, [
        { id: 'log', height: 180 },
        { id: 'discovery', width: 380 },
      ])
    },
  },

  {
    id: 'console',
    name: 'Console',
    description:
      'Everything at once: discovery, the plan, levels, and a tray of reference material below.',
    sketch: ['d s m', 'd s m', 't t t'],
    accent: ['d'],
    panels: ['discovery', 'sources', 'session', 'mixer', 'meters', 'timeline', 'log', 'manifest'],
    build(api) {
      const session = add(api, 'session')

      const discovery = add(api, 'discovery', {
        position: { referencePanel: session, direction: 'left' },
        initialWidth: 420,
      })
      add(api, 'sources', {
        position: { referencePanel: discovery, direction: 'within' },
        inactive: true,
      })

      const mixer = add(api, 'mixer', {
        position: { referencePanel: session, direction: 'right' },
        initialWidth: 360,
      })
      add(api, 'meters', {
        position: { referencePanel: mixer, direction: 'within' },
        inactive: true,
      })

      // No reference panel, so this docks against the whole grid and the tray
      // runs the full width of the window rather than sitting under one column.
      const timeline = add(api, 'timeline', {
        position: { direction: 'below' },
        initialHeight: 250,
      })
      add(api, 'log', {
        position: { referencePanel: timeline, direction: 'within' },
        inactive: true,
      })
      add(api, 'manifest', {
        position: { referencePanel: timeline, direction: 'within' },
        inactive: true,
      })

      discovery.api.setActive()
      proportion(api, [
        { id: 'timeline', height: 250 },
        { id: 'mixer', width: 360 },
        { id: 'discovery', width: 420 },
      ])
    },
  },

  {
    id: 'setup',
    name: 'Setup',
    description:
      'For deciding what to record. Discovery given the room to be read properly.',
    sketch: ['d d o', 'd d e'],
    accent: ['d'],
    panels: ['discovery', 'sources', 'session'],
    build(api) {
      const discovery = add(api, 'discovery')
      add(api, 'sources', {
        position: { referencePanel: discovery, direction: 'right' },
        initialWidth: 420,
      })
      add(api, 'session', {
        position: { referencePanel: 'sources', direction: 'below' },
      })
      discovery.api.setActive()
      proportion(api, [{ id: 'sources', width: 420 }])
    },
  },

  {
    id: 'monitor',
    name: 'Monitor',
    description:
      'For a running session. Mixer and level history, large. The one to leave on a second screen.',
    sketch: ['x t', 'x t', 'x h'],
    accent: ['x'],
    panels: ['mixer', 'timeline', 'health', 'markers'],
    build(api) {
      const mixer = add(api, 'mixer')
      const timeline = add(api, 'timeline', {
        position: { referencePanel: mixer, direction: 'right' },
      })
      add(api, 'health', {
        position: { referencePanel: timeline, direction: 'below' },
        initialHeight: 220,
      })
      add(api, 'markers', {
        position: { referencePanel: 'health', direction: 'within' },
        inactive: true,
      })
      mixer.api.setActive()
      proportion(api, [
        { id: 'mixer', width: 420 },
        { id: 'health', height: 220 },
      ])
    },
  },

  {
    id: 'diagnostics',
    name: 'Diagnostics',
    description:
      'For working out why a recording is not what it should be.',
    sketch: ['h a', 'h a', 'n n'],
    accent: ['h'],
    panels: ['health', 'log', 'manifest', 'timeline'],
    build(api) {
      const health = add(api, 'health')
      add(api, 'log', {
        position: { referencePanel: health, direction: 'right' },
      })
      const manifest = add(api, 'manifest', {
        position: { direction: 'below' },
        initialHeight: 260,
      })
      add(api, 'timeline', {
        position: { referencePanel: manifest, direction: 'within' },
        inactive: true,
      })
      health.api.setActive()
      proportion(api, [{ id: 'manifest', height: 260 }])
    },
  },

  {
    id: 'library',
    name: 'Library',
    description:
      'For finding a recording made earlier, with its manifest beside it.',
    sketch: ['l l n', 'l l e'],
    accent: ['l'],
    panels: ['library', 'manifest', 'session'],
    build(api) {
      const library = add(api, 'library')
      add(api, 'manifest', {
        position: { referencePanel: library, direction: 'right' },
        initialWidth: 400,
      })
      add(api, 'session', {
        position: { referencePanel: 'manifest', direction: 'below' },
      })
      library.api.setActive()
      proportion(api, [{ id: 'manifest', width: 400 }])
    },
  },

  {
    id: 'minimal',
    name: 'Minimal',
    description:
      'The plan and the levels, nothing else. For a small window in a corner.',
    sketch: ['e x'],
    accent: ['x'],
    panels: ['session', 'mixer'],
    build(api) {
      const session = add(api, 'session')
      add(api, 'mixer', {
        position: { referencePanel: session, direction: 'right' },
        initialWidth: 300,
      })
      session.api.setActive()
      proportion(api, [{ id: 'mixer', width: 300 }])
    },
  },
]

export const PRESETS_BY_ID: Record<string, LayoutPreset> = Object.fromEntries(
  LAYOUT_PRESETS.map((p) => [p.id, p]),
)

/** The arrangement the program opens with the first time it is run. */
export const DEFAULT_PRESET = 'console'
