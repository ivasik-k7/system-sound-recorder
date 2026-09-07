// The small shared pieces.
//
// Deliberately thin: each one is a shape plus the behaviour that shape is
// expected to have, and no more. A switch that knows about preferences, or an
// empty state that knows which panel it is in, would have to be reasoned about
// every time it was reused.

import {
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type ComponentType,
  type ReactNode,
} from 'react'
import { IconInfo, type IconProps } from './Icons'

// ------------------------------------------------------------------ switch ---

interface SwitchProps {
  checked: boolean
  onChange: (value: boolean) => void
  label: ReactNode
  disabled?: boolean
  title?: string
}

export function Switch({ checked, onChange, label, disabled, title }: SwitchProps) {
  return (
    <label className={`switch${checked ? ' switch--on' : ''}`} title={title}>
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span className="switch__track">
        <span className="switch__thumb" />
      </span>
      <span className="truncate">{label}</span>
    </label>
  )
}

// -------------------------------------------------------------- segmented ---

export interface SegmentedOption<T extends string> {
  value: T
  label: string
  icon?: ComponentType<IconProps>
  title?: string
}

interface SegmentedProps<T extends string> {
  value: T
  options: SegmentedOption<T>[]
  onChange: (value: T) => void
  disabled?: boolean
  ariaLabel?: string
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  disabled,
  ariaLabel,
}: SegmentedProps<T>) {
  return (
    <div className="segmented" role="group" aria-label={ariaLabel}>
      {options.map((option) => {
        const Icon = option.icon
        return (
          <button
            key={option.value}
            type="button"
            className="segmented__option"
            aria-pressed={option.value === value}
            disabled={disabled}
            title={option.title ?? option.label}
            onClick={() => onChange(option.value)}
          >
            {Icon && <Icon size={13} />}
            {option.label}
          </button>
        )
      })}
    </div>
  )
}

// ---------------------------------------------------------------- popover ---

interface PopoverProps {
  /** The control that opens it. Rendered as given, with the open state
   *  handed back so it can show as pressed. */
  trigger: (props: { open: boolean; toggle: () => void }) => ReactNode
  children: (props: { close: () => void }) => ReactNode
  /** Which edge of the trigger the panel is aligned to. */
  align?: 'left' | 'right'
  className?: string
}

/**
 * A menu that closes when it should.
 *
 * Three ways out — clicking away, pressing Escape, and choosing something —
 * because a menu that only closes one of those ways is a menu that gets stuck
 * open, and this program has several of them open over a workspace people are
 * dragging panels around in.
 */
export function Popover({ trigger, children, align = 'left', className }: PopoverProps) {
  const [open, setOpen] = useState(false)
  const holder = useRef<HTMLDivElement>(null)
  const close = useCallback(() => setOpen(false), [])

  useEffect(() => {
    if (!open) return
    const onDown = (e: MouseEvent) => {
      if (!holder.current?.contains(e.target as Node)) setOpen(false)
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation()
        setOpen(false)
      }
    }
    // Capture, so a click inside a panel that stops propagation still closes
    // the menu floating above it.
    document.addEventListener('mousedown', onDown, true)
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('mousedown', onDown, true)
      document.removeEventListener('keydown', onKey)
    }
  }, [open])

  return (
    <div ref={holder} style={{ position: 'relative', display: 'inline-flex', flex: 'none' }}>
      {trigger({ open, toggle: () => setOpen((v) => !v) })}
      {open && (
        <div
          className={`popover${className ? ` ${className}` : ''}`}
          style={{ top: 'calc(100% + 5px)', [align]: 0 }}
          role="menu"
        >
          {children({ close })}
        </div>
      )}
    </div>
  )
}

// -------------------------------------------------------------- menu item ---

interface MenuItemProps {
  onClick?: () => void
  icon?: ComponentType<IconProps>
  children: ReactNode
  detail?: ReactNode
  hint?: ReactNode
  checked?: boolean
  current?: boolean
  disabled?: boolean
}

export function MenuItem({
  onClick,
  icon: Icon,
  children,
  detail,
  hint,
  checked,
  current,
  disabled,
}: MenuItemProps) {
  return (
    <button
      type="button"
      role="menuitem"
      className={`menuitem${current ? ' menuitem--current' : ''}`}
      disabled={disabled}
      onClick={onClick}
    >
      {checked !== undefined ? (
        <span className="menuitem__check">{checked ? '●' : ''}</span>
      ) : (
        Icon && <Icon />
      )}
      <span className="menuitem__body">
        <span className="menuitem__title">{children}</span>
        {detail && <span className="menuitem__sub">{detail}</span>}
      </span>
      {hint}
    </button>
  )
}

// ------------------------------------------------------------------- modal ---

interface ModalProps {
  title: string
  onClose: () => void
  children: ReactNode
  actions?: ReactNode
}

export function Modal({ title, onClose, children, actions }: ModalProps) {
  const labelId = useId()

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [onClose])

  return (
    <div
      className="overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
    >
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby={labelId}>
        <h2 className="dialog__title" id={labelId}>
          {title}
        </h2>
        <div className="dialog__body">{children}</div>
        {actions && <div className="dialog__actions">{actions}</div>}
      </div>
    </div>
  )
}

// ------------------------------------------------------------------ pieces ---

/** A keyboard shortcut, rendered as keys. */
export function Kbd({ keys }: { keys: string }) {
  return (
    <span className="kbd-group">
      {keys.split('+').map((key) => (
        <kbd className="kbd" key={key}>
          {key}
        </kbd>
      ))}
    </span>
  )
}

interface EmptyProps {
  icon?: ComponentType<IconProps>
  title: string
  children?: ReactNode
  action?: ReactNode
}

/**
 * What a panel says before it has anything to show.
 *
 * Worth writing properly: between them these are most of the documentation
 * this program has, and they are read at exactly the moment someone is
 * wondering what a panel is for.
 */
export function Empty({ icon: Icon = IconInfo, title, children, action }: EmptyProps) {
  return (
    <div className="empty">
      <span className="empty__icon">
        <Icon size={19} />
      </span>
      <span className="empty__title">{title}</span>
      {children && <p className="empty__text">{children}</p>}
      {action}
    </div>
  )
}

interface NoticeProps {
  tone?: 'neutral' | 'accent' | 'warn' | 'danger'
  icon?: ComponentType<IconProps>
  children: ReactNode
}

export function Notice({ tone = 'neutral', icon: Icon, children }: NoticeProps) {
  return (
    <div className={`notice${tone === 'neutral' ? '' : ` notice--${tone}`}`}>
      {Icon && (
        <span className="notice__icon">
          <Icon size={14} />
        </span>
      )}
      <span style={{ minWidth: 0 }}>{children}</span>
    </div>
  )
}

interface StatProps {
  label: string
  value: ReactNode
  tone?: 'neutral' | 'ok' | 'warn' | 'danger'
  title?: string
}

export function Stat({ label, value, tone = 'neutral', title }: StatProps) {
  return (
    <div className={`stat${tone === 'neutral' ? '' : ` stat--${tone}`}`} title={title}>
      <span className="stat__value truncate">{value}</span>
      <span className="stat__label truncate">{label}</span>
    </div>
  )
}

/** A section heading inside a scrolling panel. Sticky, with a count. */
export function Section({
  children,
  count,
  action,
}: {
  children: ReactNode
  count?: number | string
  action?: ReactNode
}) {
  return (
    <div className="section">
      <span>{children}</span>
      {count !== undefined && <span className="section__count">{count}</span>}
      <span className="section__line" />
      {action}
    </div>
  )
}
