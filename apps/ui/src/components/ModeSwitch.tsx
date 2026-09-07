// Choosing what the window is for.
//
// Recording and transcribing are alternatives rather than layers: one produces
// audio files, the other produces words. Switching between them also rearranges
// the workspace, because the panels that matter are almost entirely different
// and leaving a mixer on screen while reading a conversation helps nobody. That
// part happens in the shell, which is what holds the workspace.

import { IconMessage, IconRecord } from './Icons'
import { useConversation, type Mode } from '../state/conversation'

const OPTIONS: { id: Mode; label: string; icon: typeof IconRecord; hint: string }[] = [
  {
    id: 'record',
    label: 'Record',
    icon: IconRecord,
    hint: 'Capture audio to files, one track per source',
  },
  {
    id: 'listen',
    label: 'Transcribe',
    icon: IconMessage,
    hint: 'Read a channel as it is spoken, and save the words',
  },
]

export function ModeSwitch() {
  const mode = useConversation((s) => s.mode)
  const setMode = useConversation((s) => s.setMode)

  // Only the choice is made here. Rearranging the workspace to suit it is the
  // shell's job, so that it happens however the mode was changed: from this
  // switch, from the keyboard, or from the address a window was opened with.
  const choose = (next: Mode) => {
    if (next !== mode) setMode(next)
  }

  return (
    <div className="mode-switch" role="tablist" aria-label="What this window is for">
      {OPTIONS.map((option) => {
        const Icon = option.icon
        const on = option.id === mode
        return (
          <button
            key={option.id}
            type="button"
            role="tab"
            aria-selected={on}
            title={option.hint}
            className={`mode-switch__option${on ? ' mode-switch__option--on' : ''}`}
            onClick={() => choose(option.id)}
          >
            <Icon size="sm" tone={on && option.id === 'listen' ? 'accent' : undefined} />
            {option.label}
          </button>
        )
      })}
    </div>
  )
}
