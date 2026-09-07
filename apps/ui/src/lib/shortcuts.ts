// The keyboard map, in one place.
//
// Declared as data rather than scattered through handlers so that the settings
// panel, the command palette and the actual key handling cannot disagree about
// what a key does — which is the usual way a shortcut ends up documented but
// not working, or working but not documented.

export interface Shortcut {
  /** Displayed form, joined with `+`. */
  keys: string
  description: string
}

export const SHORTCUTS: Shortcut[] = [
  { keys: 'Space', description: 'Start or stop recording' },
  { keys: 'Ctrl+R', description: 'Start or stop recording' },
  { keys: 'M', description: 'Mark this instant' },
  { keys: 'Ctrl+K', description: 'Command palette' },
  { keys: 'Ctrl+Shift+P', description: 'Command palette' },
  { keys: '?', description: 'Keyboard shortcuts' },
  { keys: 'Ctrl+D', description: 'Rescan for audio' },
  { keys: 'Ctrl+L', description: 'Reset the panel layout' },
  { keys: 'Ctrl+O', description: 'Open the recordings folder' },
  { keys: 'Esc', description: 'Close whatever is open' },
]

/**
 * Whether a keystroke should be ignored because the user is typing.
 *
 * Without this, the single-key shortcuts fire while someone is naming a track
 * or filtering a list — `M` in a search box would drop a marker rather than
 * type an m, which is the sort of bug that makes people stop trusting an
 * interface.
 */
export function isTypingInto(target: EventTarget | null): boolean {
  const element = target as HTMLElement | null
  if (!element) return false
  if (element.isContentEditable) return true
  return /^(INPUT|TEXTAREA|SELECT)$/.test(element.tagName)
}
