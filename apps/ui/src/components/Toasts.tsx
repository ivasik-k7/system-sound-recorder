// Transient notifications.
//
// For things that happened rather than things to do: a recording started, a
// layout was saved, a remote session armed itself. Errors stay until they are
// dismissed, because an error nobody read is an error that will be reported
// again as a bug.

import { IconCheck, IconClose, IconError, IconInfo, IconWarning } from './Icons'
import { useStore } from '../state/store'

const ICONS = {
  ok: IconCheck,
  info: IconInfo,
  warn: IconWarning,
  error: IconError,
}

export function Toasts() {
  const toasts = useStore((s) => s.toasts)
  const dismiss = useStore((s) => s.dismissToast)

  if (toasts.length === 0) return null

  return (
    <div className="toasts" role="status" aria-live="polite">
      {toasts.map((toast) => {
        const Icon = ICONS[toast.level]
        return (
          <div className={`toast toast--${toast.level}`} key={toast.id}>
            <span className="toast__icon">
              <Icon size={14} />
            </span>
            <span className="toast__body">
              <span className="toast__title">{toast.title}</span>
              {toast.detail && (
                <>
                  <br />
                  <span className="muted">{toast.detail}</span>
                </>
              )}
            </span>
            <button
              type="button"
              className="btn btn--ghost btn--sm btn--icon"
              aria-label="Dismiss"
              onClick={() => dismiss(toast.id)}
            >
              <IconClose size={11} />
            </button>
          </div>
        )
      })}
    </div>
  )
}
