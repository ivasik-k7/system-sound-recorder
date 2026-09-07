// Settings, and About.
//
// Settings live in a menu rather than in a dockable panel. Everything here is
// applied the moment it changes and remembered across restarts, so it is a
// place you visit for a few seconds and leave — which is what a menu is for,
// and a panel occupying a quarter of the window is not.
//
// The explanations stay, though. Two of these settings decide what the meters
// mean, and a slider labelled only "floor" is a slider nobody touches.

import { useState } from 'react'
import { api } from '../lib/api'
import { Modal, Popover, Segmented, Switch } from './primitives'
import {
  AppMark,
  IconInfo,
  IconKeyboard,
  IconPalette,
  IconPopOut,
  IconRadar,
  IconRefresh,
  IconSliders,
} from './Icons'
import {
  DEFAULT_PREFERENCES,
  useStore,
  type AccentName,
  type DensityName,
  type ThemeName,
} from '../state/store'

const REPOSITORY = 'https://github.com/ivasik-k7/system-sound-recorder'
const BLOB = `${REPOSITORY}/blob/main`

const THEMES: { value: ThemeName; label: string }[] = [
  { value: 'dark', label: 'Dark' },
  { value: 'midnight', label: 'Midnight' },
  { value: 'light', label: 'Light' },
  { value: 'contrast', label: 'Contrast' },
]

const DENSITIES: { value: DensityName; label: string }[] = [
  { value: 'compact', label: 'Compact' },
  { value: 'cosy', label: 'Cosy' },
  { value: 'comfortable', label: 'Roomy' },
]

/** The accent swatches. Deliberately not applied to anything load-bearing:
 *  record stays red and clipping stays red whatever is chosen here, because
 *  those two colours mean something. */
const ACCENTS: { value: AccentName; colour: string }[] = [
  { value: 'blue', colour: '#5b8cff' },
  { value: 'violet', colour: '#9d7bff' },
  { value: 'teal', colour: '#35c2c9' },
  { value: 'amber', colour: '#e5a13c' },
  { value: 'rose', colour: '#ec6a9c' },
]

/** A labelled slider with its value in the label, which is the only way a
 *  slider in a menu says anything useful. */
function Slider({
  label,
  value,
  display,
  min,
  max,
  step,
  hint,
  onChange,
}: {
  label: string
  value: number
  display: string
  min: number
  max: number
  step: number
  hint?: string
  onChange: (value: number) => void
}) {
  return (
    <label className="field">
      <span className="field__label">
        {label} · <span className="mono">{display}</span>
      </span>
      <input
        className="input"
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
      />
      {hint && <span className="field__hint">{hint}</span>}
    </label>
  )
}

interface SettingsMenuProps {
  onShowShortcuts: () => void
}

export function SettingsMenu({ onShowShortcuts }: SettingsMenuProps) {
  const prefs = useStore((s) => s.prefs)
  const setPref = useStore((s) => s.setPref)
  const notify = useStore((s) => s.notify)

  return (
    <Popover
      align="right"
      className="popover--settings"
      trigger={({ open, toggle }) => (
        <button
          type="button"
          className={`btn btn--ghost btn--sm btn--icon${open ? ' btn--accent-quiet' : ''}`}
          aria-expanded={open}
          onClick={toggle}
          title="Settings"
          aria-label="Settings"
        >
          <IconSliders size={14} />
        </button>
      )}
    >
      {({ close }) => (
        <div className="settings">
          <div className="popover__header">
            <span className="popover__headerTitle">Settings</span>
            <span className="popover__headerHint">applied as you change them</span>
          </div>

          <div className="popover__group">
            <span className="popover__groupTitle">
              <IconPalette size={11} /> Appearance
            </span>

            <label className="field">
              <span className="field__label">Theme</span>
              <Segmented
                value={prefs.theme}
                options={THEMES}
                onChange={(v) => setPref('theme', v)}
                ariaLabel="Theme"
              />
            </label>

            <div className="field">
              <span className="field__label">Accent</span>
              <div className="swatches">
                {ACCENTS.map((accent) => (
                  <button
                    key={accent.value}
                    type="button"
                    className="swatch"
                    style={{ background: accent.colour }}
                    aria-pressed={prefs.accent === accent.value}
                    aria-label={accent.value}
                    title={accent.value}
                    onClick={() => setPref('accent', accent.value)}
                  />
                ))}
              </div>
            </div>

            <label className="field">
              <span className="field__label">Density</span>
              <Segmented
                value={prefs.density}
                options={DENSITIES}
                onChange={(v) => setPref('density', v)}
                ariaLabel="Density"
              />
            </label>
          </div>

          <div className="popover__group">
            <span className="popover__groupTitle">
              <IconSliders size={11} /> Meters
            </span>
            <Slider
              label="Scale floor"
              value={prefs.meterFloor}
              display={`${prefs.meterFloor} dBFS`}
              min={-90}
              max={-40}
              step={6}
              hint="The bottom of every meter and every timeline plot. −60 is the usual choice; lower it to see how quiet a nearly silent track really is."
              onChange={(v) => setPref('meterFloor', v)}
            />
            <Slider
              label="Peak hold"
              value={prefs.holdMs}
              display={`${(prefs.holdMs / 1000).toFixed(1)} s`}
              min={0}
              max={4000}
              step={200}
              hint="How long a peak mark stays before it falls away. Without some hold, a transient that lasted one frame is invisible."
              onChange={(v) => setPref('holdMs', v)}
            />
            <Slider
              label="Meter refresh"
              value={prefs.refreshMs}
              display={`${prefs.refreshMs} ms`}
              min={40}
              max={500}
              step={20}
              onChange={(v) => setPref('refreshMs', v)}
            />
          </div>

          <div className="popover__group">
            <span className="popover__groupTitle">
              <IconRadar size={11} /> Autodiscovery
            </span>
            <Slider
              label="Refresh"
              value={prefs.discoveryMs}
              display={`${prefs.discoveryMs} ms`}
              min={100}
              max={2000}
              step={100}
              hint="A refresh reads a state and a set of channel peaks per session, a couple of milliseconds. The rescan that finds newly started streams happens about once a second regardless."
              onChange={(v) => setPref('discoveryMs', v)}
            />
            <Switch
              checked={prefs.playingOnly}
              onChange={(v) => setPref('playingOnly', v)}
              label="Only applications making a sound"
            />
            <Switch
              checked={prefs.showSystemSessions}
              onChange={(v) => setPref('showSystemSessions', v)}
              label="Include Windows system sounds"
            />
            <Switch
              checked={prefs.autoArmRemote}
              onChange={(v) => setPref('autoArmRemote', v)}
              label="Arm remote sessions automatically"
            />
            <span className="field__hint">
              With this on, a Citrix or WSL session that starts playing adds
              itself to the plan. It arms a track; it does not start recording.
            </span>
          </div>

          <div className="popover__group">
            <span className="popover__groupTitle">Window</span>
            <Switch
              checked={prefs.alwaysOnTop}
              onChange={(v) => setPref('alwaysOnTop', v)}
              label="Keep above other windows"
            />
            <Switch
              checked={prefs.showEstimates}
              onChange={(v) => setPref('showEstimates', v)}
              label="Size and capacity in the transport"
            />
          </div>

          <div className="popover__footer">
            <button
              type="button"
              className="btn btn--sm"
              onClick={() => {
                onShowShortcuts()
                close()
              }}
            >
              <IconKeyboard size={12} /> Shortcuts
            </button>
            <span className="spacer" />
            <button
              type="button"
              className="btn btn--sm btn--ghost"
              title="Put every setting back to its default"
              onClick={() => {
                for (const key of Object.keys(DEFAULT_PREFERENCES) as (keyof typeof DEFAULT_PREFERENCES)[]) {
                  setPref(key, DEFAULT_PREFERENCES[key] as never)
                }
                notify('ok', 'Settings reset')
                close()
              }}
            >
              <IconRefresh size={12} /> Reset
            </button>
          </div>
        </div>
      )}
    </Popover>
  )
}

// ---------------------------------------------------------------------------

/** One line of the About box's fact table. */
function Fact({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="about__fact">
      <span className="about__factLabel">{label}</span>
      <span className="about__factValue">{children}</span>
    </div>
  )
}

function AboutDialog({ onClose }: { onClose: () => void }) {
  const link = (label: string, url: string) => (
    <button
      key={url}
      type="button"
      className="btn btn--sm"
      onClick={() => void api.openUrl(url)}
      title={url}
    >
      {label} <IconPopOut size={11} />
    </button>
  )

  return (
    <Modal
      title="About"
      onClose={onClose}
      actions={
        <button type="button" className="btn btn--primary" onClick={onClose}>
          Close
        </button>
      }
    >
      <div className="about__head">
        <AppMark size={34} />
        <div style={{ minWidth: 0 }}>
          <div className="about__name">System Sound Recorder</div>
          <div className="rowline" style={{ gap: 6 }}>
            <span className="badge badge--accent badge--mono">{__APP_VERSION__}</span>
            <span className="badge badge--quiet badge--mono">{__BUILD_REF__}</span>
            <span className="badge badge--warn">pre-release</span>
          </div>
        </div>
      </div>

      <p className="about__blurb">
        Records every audio stream reaching this machine — Citrix HDX sessions,
        WSL, browsers and native applications — as one mixed file or as separate
        tracks that line up with each other to the millisecond.
      </p>

      <div className="about__facts">
        <Fact label="Version">
          <span className="mono">{__APP_VERSION__}</span>
        </Fact>
        <Fact label="Build">
          <span className="mono">{__BUILD_REF__}</span>
        </Fact>
        <Fact label="Licence">MIT</Fact>
        <Fact label="Capture">WASAPI loopback and process loopback</Fact>
      </div>

      <div className="chip-list" style={{ gap: 6 }}>
        {link('Repository', REPOSITORY)}
        {link('Changelog', `${BLOB}/CHANGELOG.md`)}
        {link('Licence', `${BLOB}/LICENSE`)}
        {link('Legal', `${BLOB}/LEGAL.md`)}
        {link('Privacy', `${BLOB}/PRIVACY.md`)}
        {link('Notices', `${BLOB}/THIRD-PARTY-NOTICES.md`)}
      </div>

      <p className="about__legal">
        Copyright © 2026 Ivan Kovtun. Provided as is, without warranty of any
        kind. <strong>The author accepts no responsibility or liability for how
        this program is used</strong>, for anything recorded with it, or for any
        consequence of that recording — deciding what may be recorded, telling
        the people involved, and looking after the files afterwards are entirely
        yours. Recording other people is regulated in much of the world; see{' '}
        <button
          type="button"
          className="about__inlineLink"
          onClick={() => void api.openUrl(`${BLOB}/LEGAL.md`)}
        >
          LEGAL.md
        </button>
        .
      </p>
    </Modal>
  )
}

/** The About button, and the dialog it opens. */
export function AboutMenu() {
  const [open, setOpen] = useState(false)
  return (
    <>
      <button
        type="button"
        className={`btn btn--ghost btn--sm btn--icon${open ? ' btn--accent-quiet' : ''}`}
        onClick={() => setOpen(true)}
        title={`About System Sound Recorder ${__APP_VERSION__}`}
        aria-label="About"
      >
        <IconInfo size={14} />
      </button>
      {open && <AboutDialog onClose={() => setOpen(false)} />}
    </>
  )
}
