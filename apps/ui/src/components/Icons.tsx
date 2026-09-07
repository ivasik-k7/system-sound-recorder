// Inline icons.
//
// Kept as source rather than a font or a sprite so the desktop shell has
// nothing to fetch, and so every stroke inherits the current text colour and
// therefore the theme. All drawn on the same 16-unit grid with the same stroke
// weight, which is what stops a toolbar of eight icons looking like eight icons
// from eight places.
//
// Sizes come from the `--icon-*` tokens rather than from numbers at the call
// site, so an icon in a toolbar matches every other icon in that toolbar
// without anyone having to remember which number was used last. Colour and the
// glow that goes with it come from `tone`, which is a meaning rather than a
// colour: `record` is red because recording is red everywhere in this program,
// not because red was wanted here.

import type { ReactNode } from 'react'

/** How large an icon should be, in terms of where it sits. */
export type IconSize = 'xs' | 'sm' | 'md' | 'lg' | 'xl'

/** What an icon means, which decides its colour and whether it glows. */
export type IconTone =
  | 'default'
  | 'muted'
  | 'accent'
  | 'record'
  | 'ok'
  | 'warn'
  | 'danger'

export interface IconProps {
  /** A token, or an explicit pixel size when something really needs one. */
  size?: IconSize | number
  tone?: IconTone
  /** Heavier stroke, for an icon that carries a button on its own. */
  strong?: boolean
  className?: string
  title?: string
}

/** Pixel size for each token. Kept in step with `--icon-*` in icons.css. */
const SIZES: Record<IconSize, number> = {
  xs: 13,
  sm: 15,
  md: 17,
  lg: 20,
  xl: 26,
}

function resolveSize(size: IconSize | number | undefined): number {
  if (typeof size === 'number') return size
  return SIZES[size ?? 'md']
}

function classes(tone: IconTone | undefined, className: string | undefined): string {
  return ['icon', tone && tone !== 'default' ? `icon--${tone}` : '', className]
    .filter(Boolean)
    .join(' ')
}

function stroked(path: ReactNode) {
  return function Icon({ size, tone, strong, className, title }: IconProps) {
    const px = resolveSize(size)
    return (
      <svg
        width={px}
        height={px}
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        // 1.7 rather than 1.4. At the sizes this interface uses, a 1.4 stroke
        // renders onto barely more than one physical pixel and reads as grey
        // rather than as a line.
        strokeWidth={strong ? 2 : 1.7}
        strokeLinecap="round"
        strokeLinejoin="round"
        shapeRendering="geometricPrecision"
        className={classes(tone, className)}
        role={title ? 'img' : undefined}
        aria-label={title}
        aria-hidden={title ? undefined : true}
        focusable="false"
      >
        {title ? <title>{title}</title> : null}
        {path}
      </svg>
    )
  }
}

function filled(path: ReactNode) {
  return function Icon({ size, tone, className, title }: IconProps) {
    const px = resolveSize(size)
    return (
      <svg
        width={px}
        height={px}
        viewBox="0 0 16 16"
        fill="currentColor"
        shapeRendering="geometricPrecision"
        className={classes(tone, className)}
        role={title ? 'img' : undefined}
        aria-label={title}
        aria-hidden={title ? undefined : true}
        focusable="false"
      >
        {title ? <title>{title}</title> : null}
        {path}
      </svg>
    )
  }
}

// --- transport ---------------------------------------------------------------

export const IconRecord = filled(<circle cx="8" cy="8" r="5" />)
export const IconStop = filled(<rect x="4" y="4" width="8" height="8" rx="1.5" />)

// --- sources -----------------------------------------------------------------

export const IconSpeaker = stroked(
  <>
    <path d="M2.5 6.3v3.4h2.3L8.3 12V4L4.8 6.3H2.5Z" />
    <path d="M10.8 6a2.7 2.7 0 0 1 0 4" />
    <path d="M12.7 4.3a5.2 5.2 0 0 1 0 7.4" />
  </>,
)

export const IconHeadphones = stroked(
  <>
    <path d="M3 9.5V8a5 5 0 0 1 10 0v1.5" />
    <rect x="1.8" y="9" width="2.6" height="4.2" rx="1.3" />
    <rect x="11.6" y="9" width="2.6" height="4.2" rx="1.3" />
  </>,
)

export const IconMonitor = stroked(
  <>
    <rect x="1.8" y="3" width="12.4" height="8" rx="1.4" />
    <path d="M6 13.2h4M8 11v2.2" />
  </>,
)

export const IconMic = stroked(
  <>
    <rect x="6" y="1.8" width="4" height="7" rx="2" />
    <path d="M3.8 7.6a4.2 4.2 0 0 0 8.4 0" />
    <path d="M8 11.8v2.4" />
  </>,
)

export const IconApp = stroked(
  <>
    <rect x="2.2" y="2.8" width="11.6" height="10.4" rx="1.6" />
    <path d="M2.2 5.8h11.6" />
    <path d="M4.4 4.3h.01M6.2 4.3h.01" />
  </>,
)

export const IconGlobe = stroked(
  <>
    <circle cx="8" cy="8" r="5.6" />
    <path d="M2.4 8h11.2" />
    <path d="M8 2.4c1.7 1.7 1.7 9.5 0 11.2-1.7-1.7-1.7-9.5 0-11.2Z" />
  </>,
)

export const IconRemote = stroked(
  <>
    <rect x="1.6" y="3.2" width="8.4" height="6" rx="1.3" />
    <path d="M4.4 11.4h3.2" />
    <path d="M6 9.2v2.2" />
    <path d="M11.6 6.6h2.8v7.2h-2.8z" />
    <path d="M12.6 12.3h.8" />
  </>,
)

export const IconMessage = stroked(
  <>
    <path d="M2.4 4.4A1.4 1.4 0 0 1 3.8 3h8.4a1.4 1.4 0 0 1 1.4 1.4v4.6a1.4 1.4 0 0 1-1.4 1.4H6.4L3.4 13V10.4A1.4 1.4 0 0 1 2.4 9Z" />
  </>,
)

export const IconFilm = stroked(
  <>
    <rect x="1.8" y="3.4" width="12.4" height="9.2" rx="1.4" />
    <path d="M5 3.4v9.2M11 3.4v9.2M1.8 8h12.4" />
  </>,
)

export const IconWave = stroked(
  <path d="M1.6 8h1.3M4.2 5.4v5.2M6.4 2.8v10.4M8.6 4.6v6.8M10.8 6.4v3.2M13 7.4v1.2M14.4 8h.01" />,
)

export const IconGamepad = stroked(
  <>
    <path d="M5.6 5.4h4.8a3.6 3.6 0 0 1 3.6 3.6 2 2 0 0 1-3.6 1.2l-.5-.7H6.1l-.5.7A2 2 0 0 1 2 9a3.6 3.6 0 0 1 3.6-3.6Z" />
    <path d="M5 7.6v1.4M4.3 8.3h1.4M10.6 8.3h.01" />
  </>,
)

export const IconWindows = stroked(
  <>
    <rect x="2.4" y="2.4" width="5" height="5" rx="0.8" />
    <rect x="8.6" y="2.4" width="5" height="5" rx="0.8" />
    <rect x="2.4" y="8.6" width="5" height="5" rx="0.8" />
    <rect x="8.6" y="8.6" width="5" height="5" rx="0.8" />
  </>,
)

// --- actions -----------------------------------------------------------------

export const IconPlus = stroked(<path d="M8 3.2v9.6M3.2 8h9.6" />)
export const IconClose = stroked(<path d="M4 4l8 8M12 4l-8 8" />)
export const IconCheck = stroked(<path d="M3.2 8.4 6.4 11.6 12.8 4.6" />)

export const IconTrash = stroked(
  <>
    <path d="M3.2 4.4h9.6" />
    <path d="M6.4 4.4V2.9h3.2v1.5" />
    <path d="M4.6 4.4 5.3 13.2h5.4l.7-8.8" />
  </>,
)

export const IconRefresh = stroked(
  <>
    <path d="M13.2 8a5.2 5.2 0 1 1-1.7-3.85" />
    <path d="M13.4 2.4v2.9h-2.9" />
  </>,
)

export const IconSearch = stroked(
  <>
    <circle cx="7" cy="7" r="4.2" />
    <path d="M10.2 10.2 13.4 13.4" />
  </>,
)

export const IconFolder = stroked(
  <path d="M2 12.6V3.8h4.2L7.6 5.6h6.4v7z" />,
)

export const IconClipboard = stroked(
  <>
    <rect x="3.8" y="3.2" width="8.4" height="10.2" rx="1.3" />
    <path d="M6.4 3.2V2.6a.8.8 0 0 1 .8-.8h1.6a.8.8 0 0 1 .8.8v.6" />
    <path d="M6.4 7h3.2M6.4 9.6h3.2" />
  </>,
)

export const IconPopOut = stroked(
  <>
    <path d="M9.2 2.8h4v4" />
    <path d="M13.2 2.8 8 8" />
    <path d="M12.2 9.4v3.2a.7.7 0 0 1-.7.7H3.5a.7.7 0 0 1-.7-.7V4.5a.7.7 0 0 1 .7-.7h3.2" />
  </>,
)

export const IconFloat = stroked(
  <>
    <rect x="1.8" y="2.6" width="9" height="7" rx="1.2" />
    <rect x="5.6" y="6.4" width="8.6" height="7" rx="1.2" />
  </>,
)

export const IconSave = stroked(
  <>
    <path d="M2.8 4.1a1.3 1.3 0 0 1 1.3-1.3h6.3l2.8 2.8v6.3a1.3 1.3 0 0 1-1.3 1.3H4.1a1.3 1.3 0 0 1-1.3-1.3z" />
    <path d="M5.4 2.8v3.4h4.4V2.8M5.4 13.2v-3.4h5.2v3.4" />
  </>,
)

export const IconDownload = stroked(
  <>
    <path d="M8 2.6v7.2" />
    <path d="M5.1 7.1 8 9.9l2.9-2.8" />
    <path d="M3 11.4v1.1a.9.9 0 0 0 .9.9h8.2a.9.9 0 0 0 .9-.9v-1.1" />
  </>,
)

export const IconFlag = stroked(
  <>
    <path d="M4 2.6v10.8" />
    <path d="M4 3.2h7.6l-1.3 2.6 1.3 2.6H4" />
  </>,
)

export const IconChevronDown = stroked(<path d="M3.8 6 8 10.2 12.2 6" />)
export const IconChevronRight = stroked(<path d="M6 3.8 10.2 8 6 12.2" />)
export const IconArrowUp = stroked(<path d="M8 12.8V3.2M4.4 6.8 8 3.2l3.6 3.6" />)
export const IconArrowDown = stroked(<path d="M8 3.2v9.6M4.4 9.2 8 12.8l3.6-3.6" />)

// --- panels ------------------------------------------------------------------

/** Discovery: concentric arcs sweeping outward, with a source found. */
export const IconRadar = stroked(
  <>
    <path d="M8 13.4A5.4 5.4 0 1 1 13.4 8" />
    <path d="M8 10.6A2.6 2.6 0 0 1 10.6 8" />
    <path d="M8 8l4.6-4.6" />
    <circle cx="8" cy="8" r="1" fill="currentColor" stroke="none" />
  </>,
)

export const IconMixer = stroked(
  <>
    <path d="M4 2.8v3.4M4 9.6v3.6M8 2.8v6.2M8 12.4v.8M12 2.8v1.4M12 7.6v5.6" />
    <path d="M2.6 7.9h2.8M6.6 10.7h2.8M10.6 5.9h2.8" />
  </>,
)

export const IconLayers = stroked(
  <>
    <path d="M8 2.4 2.4 5.2 8 8l5.6-2.8z" />
    <path d="M2.4 8.4 8 11.2l5.6-2.8" />
    <path d="M2.4 11.2 8 14l5.6-2.8" />
  </>,
)

export const IconList = stroked(
  <path d="M5.4 4.2h8M5.4 8h8M5.4 11.8h8M2.6 4.2h.01M2.6 8h.01M2.6 11.8h.01" />,
)

export const IconPulse = stroked(
  <path d="M1.6 8.4h2.8L6 4.8l2.2 6.6 1.6-3.4h3" />,
)

export const IconLibrary = stroked(
  <>
    <rect x="2" y="2.8" width="3" height="10.4" rx="1" />
    <rect x="6.2" y="2.8" width="3" height="10.4" rx="1" />
    <path d="M11.2 3.4l2.6.7-2 9.4-2.2-.6z" />
  </>,
)

export const IconSliders = stroked(
  <>
    <path d="M2.6 4.6h10.8M2.6 8h10.8M2.6 11.4h10.8" />
    <circle cx="5.6" cy="4.6" r="1.5" fill="var(--bg-panel)" />
    <circle cx="10.4" cy="8" r="1.5" fill="var(--bg-panel)" />
    <circle cx="6.6" cy="11.4" r="1.5" fill="var(--bg-panel)" />
  </>,
)

export const IconLayout = stroked(
  <>
    <rect x="2.2" y="2.8" width="11.6" height="10.4" rx="1.6" />
    <path d="M6.4 2.8v10.4M6.4 8h7.4" />
  </>,
)

export const IconKeyboard = stroked(
  <>
    <rect x="1.6" y="4" width="12.8" height="8" rx="1.4" />
    <path d="M4 6.6h.01M6.4 6.6h.01M8.8 6.6h.01M11.2 6.6h.01M4.8 9.4h6.4" />
  </>,
)

export const IconPalette = stroked(
  <>
    <path d="M8 13.6a5.6 5.6 0 1 1 5.6-5.6c0 1.6-1.4 2.2-2.6 2.2h-1c-.8 0-1.4.6-1.4 1.4 0 .9-.3 2-.6 2Z" />
    <path d="M5.4 6.2h.01M8 4.8h.01M10.6 6.2h.01" />
  </>,
)

export const IconInfo = stroked(
  <>
    <circle cx="8" cy="8" r="5.8" />
    <path d="M8 7.2v3.8M8 5.2h.01" />
  </>,
)

export const IconWarning = stroked(
  <>
    <path d="M8 2.4 14.4 13.2H1.6z" />
    <path d="M8 6.4v3.2M8 11.4h.01" />
  </>,
)

export const IconError = stroked(
  <>
    <circle cx="8" cy="8" r="5.8" />
    <path d="M5.8 5.8l4.4 4.4M10.2 5.8l-4.4 4.4" />
  </>,
)

export const IconMinimize = stroked(<path d="M3 8h10" />)
export const IconMaximize = stroked(<rect x="3.4" y="3.4" width="9.2" height="9.2" rx="1.2" />)
export const IconRestore = stroked(
  <>
    <rect x="3" y="5.6" width="7.4" height="7.4" rx="1.2" />
    <path d="M5.6 5.6V4a1 1 0 0 1 1-1h5.4a1 1 0 0 1 1 1v5.4a1 1 0 0 1-1 1H10.4" />
  </>,
)

/** The application mark: three bars, loudest in the middle. */
export const AppMark = ({ size = 'lg', className }: IconProps) => (
  <svg
    width={resolveSize(size)}
    height={resolveSize(size)}
    viewBox="0 0 16 16"
    className={['icon', 'app-mark', className].filter(Boolean).join(' ')}
    aria-hidden="true"
    focusable="false"
  >
    <rect x="1.8" y="6" width="2.8" height="4" rx="1.4" fill="var(--text-3)" />
    <rect x="6.6" y="2.2" width="2.8" height="11.6" rx="1.4" fill="var(--record)" />
    <rect x="11.4" y="4.8" width="2.8" height="6.4" rx="1.4" fill="var(--text-3)" />
  </svg>
)

// --- category mapping --------------------------------------------------------

import type { SourceCategory } from '../lib/types'

/** The icon for a discovered application's category.
 *
 *  A list of a dozen discovered processes is scanned by shape before it is
 *  read, so the categories have to be distinguishable at a glance rather than
 *  merely correct. */
export function categoryIcon(category: SourceCategory) {
  switch (category) {
    case 'remote':
      return IconRemote
    case 'conferencing':
      return IconMonitor
    case 'browser':
      return IconGlobe
    case 'chat':
      return IconMessage
    case 'media':
      return IconFilm
    case 'audio':
      return IconWave
    case 'game':
      return IconGamepad
    case 'system':
      return IconWindows
    default:
      return IconApp
  }
}

/** The icon for an output endpoint, from its form factor. */
export function endpointIcon(formFactor?: string | null) {
  const f = (formFactor ?? '').toLowerCase()
  if (f.includes('headphone') || f.includes('headset')) return IconHeadphones
  if (f.includes('hdmi') || f.includes('display')) return IconMonitor
  if (f.includes('microphone')) return IconMic
  return IconSpeaker
}
