// The keyboard map, shown on demand.

import { Kbd, Modal } from './primitives'
import { SHORTCUTS } from '../lib/shortcuts'

export function ShortcutsSheet({ onClose }: { onClose: () => void }) {
  return (
    <Modal
      title="Keyboard"
      onClose={onClose}
      actions={
        <button type="button" className="btn btn--primary" onClick={onClose}>
          Close
        </button>
      }
    >
      <div className="shortcuts">
        {SHORTCUTS.map((shortcut) => (
          <div className="shortcuts__row" key={`${shortcut.keys}-${shortcut.description}`}>
            <span className="truncate">{shortcut.description}</span>
            <Kbd keys={shortcut.keys} />
          </div>
        ))}
      </div>
      <p className="field__hint">
        The single-key shortcuts do nothing while you are typing into a field, so
        naming a track cannot drop a marker.
      </p>
    </Modal>
  )
}
