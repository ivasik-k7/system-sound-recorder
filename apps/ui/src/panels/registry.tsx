// The catalogue of panels.
//
// Every panel is self-contained and reads what it needs from the store, which
// is what makes them freely dockable: any one can be moved, tabbed, floated or
// torn off onto another screen without arranging anything else around it.
// Adding a panel means adding an entry here and nothing else.

import type { ComponentType } from 'react'
import {
  IconFlag,
  IconLayers,
  IconLibrary,
  IconList,
  IconMessage,
  IconMixer,
  IconPulse,
  IconRadar,
  IconSpeaker,
  IconWave,
  type IconProps,
} from '../components/Icons'
import { ConversationPanel } from './ConversationPanel'
import { DiscoveryPanel } from './DiscoveryPanel'
import { HealthPanel } from './HealthPanel'
import { LibraryPanel } from './LibraryPanel'
import { LogPanel } from './LogPanel'
import { ManifestPanel } from './ManifestPanel'
import { MarkersPanel } from './MarkersPanel'
import { MetersPanel } from './MetersPanel'
import { MixerPanel } from './MixerPanel'
import { SessionPanel } from './SessionPanel'
import { SourcesPanel } from './SourcesPanel'
import { TimelinePanel } from './TimelinePanel'

export interface PanelDefinition {
  id: string
  title: string
  component: ComponentType
  icon: ComponentType<IconProps>
  /** Shown in the panel menu, the palette, and as the satellite window's
   *  subtitle. One sentence, saying what the panel is for rather than what it
   *  is called. */
  description: string
  /**
   * True when this panel shows live discovery data.
   *
   * The shell uses this to decide how fast the backend should be discovering:
   * with one of these on screen it asks for the user's chosen rate, and
   * without any of them it lets discovery idle, so a window showing only the
   * mixer costs nothing to keep open.
   */
  needsDiscovery?: boolean
}

export const PANELS: PanelDefinition[] = [
  {
    id: 'discovery',
    title: 'Discovery',
    component: DiscoveryPanel,
    icon: IconRadar,
    description: 'What is making sound right now, with live levels',
    needsDiscovery: true,
  },
  {
    id: 'sources',
    title: 'Sources',
    component: SourcesPanel,
    icon: IconSpeaker,
    description: 'Endpoints, their channel layouts, and what each track will contain',
    needsDiscovery: true,
  },
  {
    id: 'session',
    title: 'Session',
    component: SessionPanel,
    icon: IconLayers,
    description: 'The tracks to record, in file order, and where they go',
  },
  {
    id: 'mixer',
    title: 'Mixer',
    component: MixerPanel,
    icon: IconMixer,
    description: 'One vertical strip per track, for comparing them at a glance',
  },
  {
    id: 'meters',
    title: 'Levels',
    component: MetersPanel,
    icon: IconWave,
    description: 'One track at a time, with a full scale and a decibel reading',
  },
  {
    id: 'timeline',
    title: 'Timeline',
    component: TimelinePanel,
    icon: IconPulse,
    description: 'Level history and markers across the last minute or few',
  },
  {
    id: 'markers',
    title: 'Markers',
    component: MarkersPanel,
    icon: IconFlag,
    description: 'Cue points, written into the session manifest',
  },
  {
    id: 'conversation',
    title: 'Conversation',
    component: ConversationPanel,
    icon: IconMessage,
    description: 'A channel transcribed as it is spoken, saved as it goes',
  },
  {
    id: 'library',
    title: 'Library',
    component: LibraryPanel,
    icon: IconLibrary,
    description: 'Recordings already made, and whether each one finished',
  },
  {
    id: 'health',
    title: 'Diagnostics',
    component: HealthPanel,
    icon: IconPulse,
    description: 'Dropouts, manufactured silence, capacity and cost',
  },
  {
    id: 'log',
    title: 'Activity',
    component: LogPanel,
    icon: IconList,
    description: 'What the recorder has reported about itself',
  },
  {
    id: 'manifest',
    title: 'Manifest',
    component: ManifestPanel,
    icon: IconLayers,
    description: 'The record of the session that just finished',
  },
]

export const PANELS_BY_ID: Record<string, PanelDefinition> = Object.fromEntries(
  PANELS.map((p) => [p.id, p]),
)
