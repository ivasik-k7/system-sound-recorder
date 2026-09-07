#!/usr/bin/env python3
"""Check that a recorded session is actually correct.

Reads a session directory and reports, per track, how long it is and how much
of it came from the device; then, across tracks, how well they line up.

Alignment is the measurement that matters. Two tracks recorded together should
end at the same length and cross-correlate at close to zero lag. Drift shows up
here long before anyone notices it by ear.

    scripts/verify-session.py recordings/meeting
    scripts/verify-session.py recordings/meeting --tone 440

The tone option checks a track against a reference frequency, which is how the
capture path is validated end to end: play a known sine, record it, and confirm
it comes back at the same pitch.

Only numpy is required. Python's own wave module cannot read the 32-bit float
files this recorder writes, so the parsing is done here.
"""

from __future__ import annotations

import argparse
import json
import struct
import sys
from pathlib import Path

try:
    import numpy as np
except ImportError:
    sys.exit("numpy is required: pip install numpy")


def read_wav_float32(path: Path) -> tuple[np.ndarray, int]:
    """Return interleaved samples as (frames, channels) and the sample rate."""
    data = path.read_bytes()
    if data[:4] != b"RIFF" or data[8:12] != b"WAVE":
        raise ValueError(f"{path.name} is not a RIFF WAVE file")

    chunks: dict[bytes, tuple[int, int]] = {}
    pos = 12
    while pos + 8 <= len(data):
        cid = data[pos : pos + 4]
        (size,) = struct.unpack("<I", data[pos + 4 : pos + 8])
        chunks[cid] = (pos + 8, size)
        pos += 8 + size + (size & 1)

    if b"fmt " not in chunks or b"data" not in chunks:
        raise ValueError(f"{path.name} is missing a fmt or data chunk")

    fo, _ = chunks[b"fmt "]
    _tag, channels, rate, _brate, _align, bits = struct.unpack("<HHIIHH", data[fo : fo + 16])
    if bits != 32:
        raise ValueError(f"{path.name} is {bits}-bit; this tool expects 32-bit float")

    do, ds = chunks[b"data"]
    usable = (ds // 4) * 4
    samples = np.frombuffer(data[do : do + usable], dtype="<f4")
    frames = len(samples) // channels
    return samples[: frames * channels].reshape(-1, channels), rate


def dominant_frequency(mono: np.ndarray, rate: int) -> float:
    """Peak frequency, interpolated between bins for sub-bin accuracy."""
    x = mono.astype(np.float64)
    x = x - x.mean()
    spectrum = np.abs(np.fft.rfft(x * np.hanning(len(x))))
    k = int(np.argmax(spectrum))
    if 0 < k < len(spectrum) - 1:
        y0, y1, y2 = spectrum[k - 1], spectrum[k], spectrum[k + 1]
        denom = y0 - 2 * y1 + y2
        delta = 0.5 * (y0 - y2) / denom if denom != 0 else 0.0
    else:
        delta = 0.0
    return (k + delta) * rate / len(x)


def bandpass(mono: np.ndarray, rate: int, low: float, high: float) -> np.ndarray:
    """Keep only one band of a signal.

    Needed whenever the system mix contains anything besides the reference
    sound. Correlating a track that carries a test tone against one that also
    carries music compares the music, and reports nonsense.
    """
    spectrum = np.fft.rfft(mono.astype(np.float64))
    freqs = np.fft.rfftfreq(len(mono), 1 / rate)
    spectrum[(freqs < low) | (freqs > high)] = 0
    return np.fft.irfft(spectrum, len(mono))


def envelope(mono: np.ndarray, rate: int, target_rate: int = 4000):
    """Rectified, smoothed and decimated amplitude, with its new sample rate.

    A running mean via cumulative sums keeps this linear in the length of the
    recording, which matters: a long session is tens of millions of samples.
    """
    x = np.abs(mono.astype(np.float64))
    width = max(1, int(rate * 0.005))
    cumulative = np.concatenate(([0.0], np.cumsum(x)))
    smoothed = (cumulative[width:] - cumulative[:-width]) / width
    step = max(1, rate // target_rate)
    return smoothed[::step], rate / step


def lag_between(a: np.ndarray, b: np.ndarray, rate: int, max_ms: float = 250.0):
    """Alignment between two tracks, in frames, or None when it cannot be told.

    Correlating the waveforms directly is a trap. A steady tone correlates just
    as well one whole period out as it does at zero, so the answer is ambiguous
    by a multiple of the period and can be confidently wrong by tens of
    milliseconds while looking convincing. Correlating the amplitude envelopes
    instead aligns on when sound started and stopped, which is unambiguous.

    That only works if there is a transient to align on. A track whose envelope
    is flat carries no timing information at all, and this says so rather than
    inventing a number.
    """
    ea, env_rate = envelope(a, rate)
    eb, _ = envelope(b, rate)
    n = min(len(ea), len(eb))
    if n < 16:
        return None, 0.0, "too short to compare"
    ea = ea[:n] - ea[:n].mean()
    eb = eb[:n] - eb[:n].mean()

    norm = float(np.linalg.norm(ea) * np.linalg.norm(eb))
    if norm == 0:
        return None, 0.0, "both tracks are effectively silent"
    # A flat envelope relative to the signal's own level means nothing starts
    # or stops, so there is no feature to line up.
    reference_level = float(np.abs(a).max()) or 1.0
    if float(np.abs(ea).max()) < 0.02 * reference_level:
        return None, 0.0, "no transients to align on"

    # Correlate by FFT; the direct form is quadratic and unusable at this length.
    size = 1 << int(np.ceil(np.log2(2 * n)))
    spectrum = np.fft.rfft(ea, size) * np.conj(np.fft.rfft(eb, size))
    correlation = np.fft.irfft(spectrum, size)

    max_lag = min(int(env_rate * max_ms / 1000), n - 1)
    # Positive lags sit at the start of the result, negative lags at the end.
    window = np.concatenate((correlation[-max_lag:], correlation[: max_lag + 1]))
    index = int(np.argmax(window))
    lag_env = index - max_lag
    quality = float(window[index] / norm)
    lag_frames = int(round(lag_env * rate / env_rate))
    return lag_frames, quality, ""


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("session", type=Path, help="a session directory containing session.json")
    ap.add_argument("--tone", type=float, help="expected frequency in Hz, to check the capture path")
    args = ap.parse_args()

    manifest_path = args.session / "session.json"
    if not manifest_path.exists():
        return fail(f"no session.json in {args.session}")
    manifest = json.loads(manifest_path.read_text())

    print(f"{manifest['tool']} {manifest['version']}")
    print(f"{manifest['started_utc']} to {manifest['ended_utc']}, {manifest['duration_secs']:.3f} s")
    for w in manifest.get("warnings", []):
        print(f"  warning: {w}")
    print()

    loaded: dict[str, tuple[np.ndarray, int]] = {}
    print(f"{'track':<18}{'seconds':>10}{'captured':>10}{'silence':>9}{'peak dBFS':>11}{'dropouts':>10}")
    for track in manifest["tracks"]:
        path = args.session / track["file"]
        if not path.exists():
            print(f"{track['name']:<18}  missing: {track['file']}")
            continue
        audio, rate = read_wav_float32(path)
        loaded[track["name"]] = (audio, rate)

        stats = track["stats"]
        total = max(stats["frames_captured"] + stats["frames_silence_filled"], 1)
        peak = float(np.abs(audio).max()) if audio.size else 0.0
        peak_db = 20 * np.log10(peak) if peak > 0 else float("-inf")
        print(
            f"{track['name']:<18}"
            f"{audio.shape[0] / rate:>10.3f}"
            f"{stats['frames_captured'] / total:>9.0%}"
            f"{stats['frames_silence_filled'] / total:>9.0%}"
            f"{peak_db:>11.1f}"
            f"{stats['discontinuities']:>10}"
        )

    problems = 0

    if len(loaded) > 1:
        print("\nAlignment between tracks")
        names = list(loaded)
        lengths = {n: loaded[n][0].shape[0] for n in names}
        spread = max(lengths.values()) - min(lengths.values())
        print(f"  length spread: {spread} frames")
        if spread > 2:
            print("    tracks should end within a frame or two of each other")
            problems += 1

        reference = names[0]
        ref_audio, rate = loaded[reference]
        for other in names[1:]:
            audio, other_rate = loaded[other]
            if other_rate != rate:
                print(f"  {reference} vs {other}: different rates, not comparable")
                continue
            left, right = ref_audio[:, 0], audio[:, 0]
            if args.tone:
                # Compare only the reference tone, so whatever else the machine
                # happened to be playing does not swamp the measurement.
                width = max(10.0, args.tone * 0.03)
                n = min(len(left), len(right))
                left = bandpass(left[:n], rate, args.tone - width, args.tone + width)
                right = bandpass(right[:n], rate, args.tone - width, args.tone + width)
            lag, quality, note = lag_between(left, right, rate)
            if lag is None:
                print(f"  {reference} vs {other}: {note}")
                continue
            ms = lag / rate * 1000
            print(
                f"  {reference} vs {other}: {lag:+d} frames ({ms:+.3f} ms), "
                f"envelope correlation {quality:.4f}"
            )
            if abs(ms) > 20:
                print("    more than 20 ms apart, which suggests the timeline is drifting")
                problems += 1

    if args.tone:
        print(f"\nTone check against {args.tone:.1f} Hz")
        print("  a track carrying other audio as well will not match; that is expected")
        for name, (audio, rate) in loaded.items():
            if audio.shape[0] < rate:
                continue
            energy = np.abs(audio[:, 0])
            window = rate
            centre = int(np.argmax(np.convolve(energy, np.ones(window) / window, "same")))
            lo = max(0, centre - window // 2)
            hi = min(audio.shape[0], lo + window)
            if hi - lo < window // 2:
                continue
            found = dominant_frequency(audio[lo:hi, 0], rate)
            ppm = abs(found - args.tone) / args.tone * 1e6
            print(f"  {name:<18}{found:>10.3f} Hz   {ppm:>8.1f} ppm")
            if ppm > 1000:
                print("    off by more than a tenth of a percent; check for resampling")
                problems += 1

    print()
    if problems:
        print(f"{problems} problem(s) found")
        return 1
    print("no problems found")
    return 0


def fail(message: str) -> int:
    print(message, file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
