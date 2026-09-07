// Preferences.
//
// Everything here is applied the moment it changes and remembered across
// restarts, so there is no apply button and no cancel. The two settings worth
// arguing about are the ones with explanations attached: the meter floor, which
// decides what "silence" means on every meter in the window, and automatic
// arming, which is the difference between catching the start of a call and
// missing it.

import { Kbd, Notice, Section, Segmented, Switch } from '../components/primitives'
import { IconKeyboard, IconPalette, IconRadar, IconSliders } from '../components/Icons'
import { SHORTCUTS } from '../lib/shortcuts'
import {
  DEFAULT_PREFERENCES,
  useStore,
  type AccentName,
  type DensityName,
  type ThemeName,
} from '../state/store'

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

export function SettingsPanel() {
  const prefs = useStore((s) => s.prefs)
  const setPref = useStore((s) => s.setPref)

  return (
    <div className="panel">
      <div className="panel__body panel__body--pad stack">
        <Section>
          <IconPalette size={12} /> Appearance
        </Section>

        <label className="field">
          <span className="field__label">Theme</span>
          <Segmented
            value={prefs.theme}
            options={THEMES}
            onChange={(v) => setPref('theme', v)}
            ariaLabel="Theme"
          />
        </label>

        <label className="field">
          <span className="field__label">Density</span>
          <Segmented
            value={prefs.density}
            options={DENSITIES}
            onChange={(v) => setPref('density', v)}
            ariaLabel="Density"
          />
          <span className="field__hint">
            Row heights, padding and type size throughout. Compact fits about a
            third more into a panel and is what a second screen wants.
          </span>
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
          <span className="field__hint">
            Used for selection and emphasis only. Record and clipping stay red
            whatever is chosen, because those colours carry meaning.
          </span>
        </div>

        <Section>
          <IconSliders size={12} /> Meters
        </Section>

        <label className="field">
          <span className="field__label">Scale floor · {prefs.meterFloor} dBFS</span>
          <input
            className="input"
            type="range"
            min={-90}
            max={-40}
            step={6}
            value={prefs.meterFloor}
            onChange={(e) => setPref('meterFloor', Number(e.target.value))}
          />
          <span className="field__hint">
            The bottom of every meter and every timeline plot. −60 is the usual
            choice. Lower it to see how quiet a nearly silent track really is;
            raise it to give the loud end of the scale more room.
          </span>
        </label>

        <label className="field">
          <span className="field__label">Peak hold · {(prefs.holdMs / 1000).toFixed(1)} s</span>
          <input
            className="input"
            type="range"
            min={0}
            max={4000}
            step={200}
            value={prefs.holdMs}
            onChange={(e) => setPref('holdMs', Number(e.target.value))}
          />
          <span className="field__hint">
            How long a peak mark stays where it was before it falls away. Without
            some hold, a transient that lasted one frame is invisible.
          </span>
        </label>

        <label className="field">
          <span className="field__label">Meter refresh · {prefs.refreshMs} ms</span>
          <input
            className="input"
            type="range"
            min={40}
            max={500}
            step={20}
            value={prefs.refreshMs}
            onChange={(e) => setPref('refreshMs', Number(e.target.value))}
          />
          <span className="field__hint">
            How often the interface asks the recorder for levels. The recorder is
            unaffected by this: it is already metering, and this only changes how
            often the answer is collected.
          </span>
        </label>

        <Section>
          <IconRadar size={12} /> Autodiscovery
        </Section>

        <label className="field">
          <span className="field__label">Discovery refresh · {prefs.discoveryMs} ms</span>
          <input
            className="input"
            type="range"
            min={100}
            max={2000}
            step={100}
            value={prefs.discoveryMs}
            onChange={(e) => setPref('discoveryMs', Number(e.target.value))}
          />
          <span className="field__hint">
            A refresh pass reads a state and a set of channel peaks per audio
            session, which costs a couple of milliseconds. A full rescan, which
            finds newly started streams, happens about once a second regardless.
          </span>
        </label>

        <Switch
          checked={prefs.playingOnly}
          onChange={(v) => setPref('playingOnly', v)}
          label="Show only applications that are making a sound"
        />
        <Switch
          checked={prefs.showSystemSessions}
          onChange={(v) => setPref('showSystemSessions', v)}
          label="Include the Windows system sounds session"
        />
        <Switch
          checked={prefs.autoArmRemote}
          onChange={(v) => setPref('autoArmRemote', v)}
          label="Arm remote sessions automatically"
        />
        <span className="field__hint">
          With this on, a Citrix or WSL session that starts playing is added to
          the plan by itself. Off by default, because adding a track to someone's
          session unasked is a surprise; on, it is the difference between
          catching the beginning of a call and missing it.
        </span>

        <Section>Window</Section>
        <Switch
          checked={prefs.alwaysOnTop}
          onChange={(v) => setPref('alwaysOnTop', v)}
          label="Keep this window above others"
        />
        <Switch
          checked={prefs.showEstimates}
          onChange={(v) => setPref('showEstimates', v)}
          label="Show size and capacity estimates in the transport"
        />

        <Section>
          <IconKeyboard size={12} /> Keyboard
        </Section>
        <div className="shortcuts">
          {SHORTCUTS.map((shortcut) => (
            <div className="shortcuts__row" key={shortcut.keys}>
              <span className="truncate">{shortcut.description}</span>
              <Kbd keys={shortcut.keys} />
            </div>
          ))}
        </div>

        <Notice>
          Preferences are stored in this window's local storage, alongside the
          panel layout. They are not part of a recording and never reach the
          engine, apart from the discovery refresh rate, which it is told about
          so an idle window costs nothing.
        </Notice>

        <div className="rowline">
          <span className="spacer" />
          <button
            type="button"
            className="btn btn--sm"
            onClick={() => {
              for (const key of Object.keys(DEFAULT_PREFERENCES) as (keyof typeof DEFAULT_PREFERENCES)[]) {
                setPref(key, DEFAULT_PREFERENCES[key] as never)
              }
            }}
          >
            Reset preferences
          </button>
        </div>
      </div>
    </div>
  )
}
