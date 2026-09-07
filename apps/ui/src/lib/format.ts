// Formatting helpers, kept together so the panels read as layout rather than
// as arithmetic.

/** Seconds as `h:mm:ss.t`, dropping the hours when they are zero. */
export function duration(secs: number): string {
  const { text } = durationParts(secs)
  return text
}

/**
 * The same, split so the tenths can be styled down.
 *
 * A clock large enough to read across a desk has tenths that flicker
 * distractingly at full size, and shrinking them is the fix.
 */
export function durationParts(secs: number): { text: string; whole: string; frac: string } {
  let s = secs
  if (!Number.isFinite(s) || s < 0) s = 0
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const sec = Math.floor(s % 60)
  const tenths = Math.floor((s * 10) % 10)
  const mm = String(m).padStart(h > 0 ? 2 : 1, '0')
  const ss = String(sec).padStart(2, '0')
  const whole = h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`
  return { text: `${whole}.${tenths}`, whole, frac: `.${tenths}` }
}

/** Seconds as a rounded, human phrase: `2 h 14 m`, `38 s`. */
export function durationWords(secs: number | null | undefined): string {
  if (secs === null || secs === undefined || !Number.isFinite(secs)) return '—'
  if (secs < 60) return `${Math.round(secs)} s`
  const m = Math.round(secs / 60)
  if (m < 60) return `${m} min`
  const h = Math.floor(m / 60)
  const rest = m % 60
  return rest === 0 ? `${h} h` : `${h} h ${rest} m`
}

/** A decibel value with a fixed width and an explicit floor. */
export function decibels(db: number, digits = 1): string {
  if (!Number.isFinite(db) || db <= -119) return '−∞'
  return db.toFixed(digits)
}

/** A linear amplitude in 0..1 as dBFS, floored so it stays finite. */
export function amplitudeToDb(amplitude: number): number {
  if (!Number.isFinite(amplitude) || amplitude <= 0.000001) return -120
  return 20 * Math.log10(amplitude)
}

/** A byte count in the largest unit that keeps it under four digits. */
export function bytes(n: number): string {
  if (!Number.isFinite(n) || n < 0) return '—'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let v = n
  let i = 0
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i += 1
  }
  return `${v < 10 && i > 0 ? v.toFixed(1) : Math.round(v)} ${units[i]}`
}

/** Bytes per second of float audio at this format. */
export function byteRate(sampleRate: number, channels: number, tracks = 1): number {
  return sampleRate * channels * 4 * tracks
}

/** Estimated size on disk of a float32 WAV of this length. */
export function estimatedSize(
  secs: number,
  sampleRate: number,
  channels: number,
  tracks = 1,
): string {
  return bytes(secs * byteRate(sampleRate, channels, tracks))
}

/** Percentage with no decimals, guarding against a zero denominator. */
export function percent(part: number, whole: number, digits = 0): string {
  if (whole <= 0) return '0%'
  return `${((part / whole) * 100).toFixed(digits)}%`
}

/** A fraction in 0..1 as a percentage. */
export function fraction(value: number | null | undefined, digits = 0): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return '—'
  return `${(value * 100).toFixed(digits)}%`
}

/** Shorten a long filesystem path, keeping both ends readable. */
export function shortPath(path: string, max = 44): string {
  if (!path) return ''
  if (path.length <= max) return path
  const sep = path.includes('\\') ? '\\' : '/'
  const parts = path.split(sep)
  if (parts.length < 3) return `…${path.slice(-(max - 1))}`
  return `${parts[0]}${sep}…${sep}${parts[parts.length - 2]}${sep}${parts[parts.length - 1]}`
}

/** The last component of a path: the session folder's own name. */
export function baseName(path: string): string {
  if (!path) return ''
  const parts = path.split(/[\\/]/)
  return parts[parts.length - 1] ?? path
}

/** A sample rate as `48 kHz`, without a trailing `.0`. */
export function kilohertz(rate: number): string {
  if (!Number.isFinite(rate) || rate <= 0) return '—'
  const k = rate / 1000
  return `${Number.isInteger(k) ? k : k.toFixed(1)} kHz`
}

/**
 * A UTC timestamp from the manifest, in the reader's own time zone.
 *
 * The manifest stores UTC because being unambiguous matters there. A list of
 * recordings is read by a person, for whom being familiar matters more.
 */
export function localTime(utc: string | null | undefined): string {
  if (!utc) return '—'
  const at = new Date(utc)
  if (Number.isNaN(at.getTime())) return utc
  return at.toLocaleString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
}

/** `4 minutes ago`, for a list ordered by recency. */
export function relativeTime(utc: string | null | undefined, unixFallback = 0): string {
  const ms = utc ? new Date(utc).getTime() : unixFallback * 1000
  if (!Number.isFinite(ms) || ms <= 0) return ''
  const secs = (Date.now() - ms) / 1000
  if (secs < 90) return 'just now'
  const table: [number, string][] = [
    [60, 'minute'],
    [3600, 'hour'],
    [86400, 'day'],
    [604800, 'week'],
  ]
  let unit = 'minute'
  let size = 60
  for (const [seconds, name] of table) {
    if (secs >= seconds) {
      size = seconds
      unit = name
    }
  }
  const n = Math.round(secs / size)
  return `${n} ${unit}${n === 1 ? '' : 's'} ago`
}

/** `1, 2, 3 and 4 more`, for a process family with too many members to list. */
export function pidSummary(pids: number[], show = 3): string {
  if (pids.length === 0) return 'not running'
  if (pids.length === 1) return `pid ${pids[0]}`
  if (pids.length <= show + 1) return `pids ${pids.join(', ')}`
  return `${pids.length} processes · ${pids.slice(0, show).join(', ')} and ${
    pids.length - show
  } more`
}

/** `3 tracks`, with the singular handled. */
export function plural(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`
}
