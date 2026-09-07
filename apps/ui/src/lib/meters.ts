// Meter ballistics.
//
// A meter driven straight from the samples is useless: at ten frames a second
// a transient that clipped the recording occupies one frame, and the eye does
// not catch it. Real meters therefore have memory, and this is that memory.
//
// Three behaviours, each answering a different question.
//
// The **bar** follows the average level, which says how loud the material is.
// The **peak mark** follows the instantaneous peak, which says how close it is
// to the ceiling. The **hold** keeps the loudest peak of the last couple of
// seconds and then falls away at a fixed rate, which is what makes a single
// sharp click visible at all.
//
// And the **clip indicator latches**. It stays lit until it is cleared,
// because a clipped recording cannot be repaired afterwards, and a warning
// that fades on its own is a warning that can be missed while you are looking
// at something else.

/** The state a meter carries between frames. */
export interface MeterState {
  /** Instantaneous peak, dBFS. */
  peakDb: number
  /** Average level, dBFS, smoothed. */
  rmsDb: number
  /** Loudest recent peak, dBFS, decaying. */
  holdDb: number
  /** When the current hold was set, as a millisecond timestamp. */
  holdAt: number
  /** True once the signal has reached full scale, until cleared. */
  clipped: boolean
  /** When the most recent clip happened. */
  clippedAt: number
}

export interface Ballistics {
  /** How long a peak is held before it starts to fall, in milliseconds. */
  holdMs: number
  /** How fast the hold falls once it starts, in decibels per second. */
  decayDbPerSec: number
  /**
   * Level at which the clip indicator latches, in dBFS.
   *
   * Not zero. The captured stream is 32-bit float, which can exceed full
   * scale without wrapping, but anything mixed to it at nought decibels was
   * already at the ceiling somewhere upstream. A hair below is the honest
   * threshold and it catches the case that matters.
   */
  clipDb: number
  /**
   * How quickly the bar follows the average, 0..1 per frame.
   *
   * The bar rises almost instantly and falls slowly, which is what a needle
   * does and what makes a level readable rather than a flicker.
   */
  attack: number
  release: number
}

export const DEFAULT_BALLISTICS: Ballistics = {
  holdMs: 1600,
  decayDbPerSec: 20,
  clipDb: -0.1,
  attack: 0.85,
  release: 0.28,
}

/** The floor of the meter scale, in dBFS. Everything below this reads as
 *  silence. */
export const METER_FLOOR = -60

export function emptyMeter(now = Date.now()): MeterState {
  return {
    peakDb: -120,
    rmsDb: -120,
    holdDb: -120,
    holdAt: now,
    clipped: false,
    clippedAt: 0,
  }
}

/**
 * Advance one meter by one frame.
 *
 * Pure, so the store can hold the state and the components can stay dumb.
 */
export function advanceMeter(
  previous: MeterState | undefined,
  sample: { peakDb: number; rmsDb: number },
  now: number,
  ballistics: Ballistics = DEFAULT_BALLISTICS,
): MeterState {
  const prev = previous ?? emptyMeter(now)
  const peakDb = Number.isFinite(sample.peakDb) ? sample.peakDb : -120
  const rawRms = Number.isFinite(sample.rmsDb) ? sample.rmsDb : -120

  // Rise fast, fall slow.
  const coefficient = rawRms > prev.rmsDb ? ballistics.attack : ballistics.release
  const rmsDb = prev.rmsDb + (rawRms - prev.rmsDb) * coefficient

  // The hold either takes a new maximum, waits, or falls.
  let holdDb = prev.holdDb
  let holdAt = prev.holdAt
  if (peakDb >= prev.holdDb) {
    holdDb = peakDb
    holdAt = now
  } else {
    const waited = now - prev.holdAt
    if (waited > ballistics.holdMs) {
      const falling = ((waited - ballistics.holdMs) / 1000) * ballistics.decayDbPerSec
      holdDb = Math.max(peakDb, prev.holdDb - falling)
      // Re-base the timer so the fall is continuous rather than accelerating
      // with every frame that passes.
      holdAt = now - ballistics.holdMs
    }
  }

  const clipping = peakDb >= ballistics.clipDb
  return {
    peakDb,
    rmsDb,
    holdDb,
    holdAt,
    clipped: prev.clipped || clipping,
    clippedAt: clipping ? now : prev.clippedAt,
  }
}

/**
 * Map a decibel value onto 0..1 for meter geometry.
 *
 * Linear in decibels, which is what makes the same colour mean the same level
 * on every meter in the window. A linear-in-amplitude scale would compress
 * everything below a third of full scale into the first few pixels, and that
 * is where nearly all real programme material sits.
 */
export function meterFraction(db: number, floor = METER_FLOOR): number {
  if (!Number.isFinite(db)) return 0
  const span = -floor
  return Math.max(0, Math.min(1, (db - floor) / span))
}

/** The scale marks a meter is labelled with, in dBFS. */
export const METER_TICKS = [-60, -40, -20, -12, -6, 0]

/** Which band a level falls in, for colouring something that is not a bar. */
export function levelBand(db: number): 'silent' | 'low' | 'mid' | 'high' | 'clip' {
  if (db <= METER_FLOOR) return 'silent'
  if (db >= -1) return 'clip'
  if (db >= -6) return 'high'
  if (db >= -18) return 'mid'
  return 'low'
}

/** The CSS variable holding the colour for a level. */
export function bandColour(db: number): string {
  switch (levelBand(db)) {
    case 'clip':
      return 'var(--meter-clip)'
    case 'high':
      return 'var(--meter-high)'
    case 'mid':
      return 'var(--meter-mid)'
    case 'low':
      return 'var(--meter-low)'
    default:
      return 'var(--text-3)'
  }
}
