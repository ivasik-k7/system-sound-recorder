"""A speech engine that stays alive between utterances.

The command line whisper program is a one-shot: every invocation starts Python,
imports torch, reads the model from disk, transcribes a couple of seconds of
speech and exits. That is five to fifteen seconds of overhead per phrase no
matter how short the phrase, and no setting removes it because it is the shape
of the program rather than a choice it makes.

This does the expensive part once and then waits. Three further things matter
for it to feel like something happening quietly rather than something the
machine is visibly doing:

* It uses faster-whisper when that is installed, which is several times quicker
  on a processor than the reference implementation for the same model.
* It leaves cores free. Left alone these libraries take every core, which
  starves the audio capture and makes the whole interface stutter — the
  transcription gets faster and everything else gets worse.
* It decodes greedily, but it does fall back through a short ladder of rising
  temperatures when a decode comes back degenerate. This used to be switched off
  entirely, to stop a difficult passage taking many times as long as an easy
  one. That was a bad trade. The fallback is the mechanism that detects the
  model looping — emitting one phrase over and over until the window ends — and
  retries it, and without it a third of the words in a long recording came back
  as loop rather than speech. The ladder is kept short so the worst case stays
  bounded, and `sanity.rs` on the Rust side cleans up whatever still gets
  through.

Audio arrives as raw samples over the pipe rather than as a file, which avoids
writing a temporary wav per utterance and, more usefully, avoids ffmpeg: whisper
only shells out to ffmpeg when handed a path.

Protocol, one job at a time, on the standard streams:

    ->  a line of JSON: {"samples": N, "language": ..., "translate": ...}
    ->  N little-endian float32 samples, mono, 16 kHz
    <-  a line of JSON: {"ok": true, "language": ..., "segments": [...]}

Segments are returned individually rather than joined. Their timings are what
lets one utterance become several lines, and what lets a change of speaker
inside an utterance be noticed at all.

Anything the libraries print goes to stderr, so the pipe carrying the protocol
cannot be corrupted by a stray warning.
"""

import inspect
import json
import os
import sys


def reply(stream, payload):
    stream.write(json.dumps(payload) + "\n")
    stream.flush()


def fail(message):
    sys.stderr.write(message + "\n")
    sys.stderr.flush()
    raise SystemExit(1)


# The settings that stop a decode running away with itself.
#
# A model that has started looping produces text that compresses extremely well,
# which is exactly what `compression_ratio_threshold` measures; whisper responds
# by retrying the window at a higher temperature, which breaks the cycle. None
# of that happens with a single temperature, because there is nothing to retry
# at. The ladder is deliberately short — three attempts, not the stock six — so
# that a hard passage costs a bounded amount of extra time rather than ten times
# the usual.
#
# The two implementations spell some of these differently and older releases do
# not have all of them, so they are filtered against the actual signature rather
# than passed blind. A missing guard is a worse transcript; a TypeError is no
# transcript at all.
GUARDS = {
    "temperature": (0.0, 0.2, 0.4),
    "compression_ratio_threshold": 2.4,
    "logprob_threshold": -1.0,      # openai-whisper
    "log_prob_threshold": -1.0,     # faster-whisper
    "no_speech_threshold": 0.6,
}


def supported(fn, options):
    """Keep only the options `fn` will actually accept."""
    try:
        allowed = set(inspect.signature(fn).parameters)
    except (TypeError, ValueError):
        # Some builds expose C extensions with no introspectable signature.
        # Passing nothing extra is the safe reading.
        return {}
    return {k: v for k, v in options.items() if k in allowed}


def spare_threads():
    """How many cores to use, leaving enough for everything else.

    Recognition is not the only thing happening: audio is being captured on a
    deadline and an interface is being drawn. Taking every core makes those
    stutter, and a transcript that arrives promptly is no use if the recording
    it came from has gaps in it.
    """
    total = os.cpu_count() or 4
    # Leave a quarter of the machine, and at least four cores, for the audio
    # capture and the interface. Recognition is the slow part and wants threads,
    # but a transcript that arrives promptly is no use if the recording it came
    # from has gaps in it.
    spare = max(4, total // 4)
    return max(1, min(16, total - spare))


def be_polite():
    """Ask the operating system to schedule this below everything else."""
    try:
        if os.name == "nt":
            import ctypes

            # BELOW_NORMAL_PRIORITY_CLASS
            ctypes.windll.kernel32.SetPriorityClass(
                ctypes.windll.kernel32.GetCurrentProcess(), 0x00004000
            )
        else:
            os.nice(5)
    except Exception:  # noqa: BLE001 - a hint, not a requirement
        pass


class FasterWhisper:
    """faster-whisper: the same models, several times quicker on a processor."""

    name = "faster-whisper"

    def __init__(self, model_name, threads):
        from faster_whisper import WhisperModel

        # int8 is the point of using this backend on a processor. The accuracy
        # cost is small and the speed difference is not.
        self.model = WhisperModel(
            model_name, device="cpu", compute_type="int8", cpu_threads=threads
        )

    def transcribe(self, audio, language, translate, prompt, job_words=False):
        segments, info = self.model.transcribe(
            audio,
            language=language or None,
            task="translate" if translate else "transcribe",
            beam_size=1,
            condition_on_previous_text=False,
            initial_prompt=prompt or None,
            **supported(self.model.transcribe, GUARDS),
            # Off by default. Word timings are a second alignment pass over the
            # audio, and while a conversation is being read live the delay costs
            # more than the precision buys. The segment boundaries are good
            # enough to split lines and to tell the voices apart.
            word_timestamps=bool(job_words),
            vad_filter=False,
        )
        found = [
            {"start": float(s.start), "end": float(s.end), "text": s.text}
            for s in segments
        ]
        return found, (info.language or "")


class OpenAIWhisper:
    """The reference implementation, used when it is the only one installed."""

    name = "openai-whisper"

    def __init__(self, model_name, threads):
        import torch
        import whisper

        torch.set_num_threads(threads)
        self.whisper = whisper
        self.model = whisper.load_model(model_name)

    def transcribe(self, audio, language, translate, prompt, job_words=False):
        result = self.model.transcribe(
            audio,
            fp16=False,
            language=language or None,
            task="translate" if translate else "transcribe",
            condition_on_previous_text=False,
            initial_prompt=prompt or None,
            **supported(self.model.transcribe, GUARDS),
            # See the note in the other backend: this is an extra pass over the
            # audio and it is the single largest avoidable cost per utterance.
            word_timestamps=bool(job_words),
        )
        found = [
            {
                "start": float(s.get("start", 0.0)),
                "end": float(s.get("end", 0.0)),
                "text": s.get("text", ""),
            }
            for s in result.get("segments", [])
        ]
        return found, (result.get("language") or "")


def load_backend(model_name, threads, prefer):
    """The quickest backend that is actually installed."""
    order = [FasterWhisper, OpenAIWhisper]
    if prefer == "openai-whisper":
        order.reverse()

    problems = []
    for backend in order:
        try:
            return backend(model_name, threads)
        except ImportError as error:
            problems.append(f"{backend.name}: not installed ({error})")
        except Exception as error:  # noqa: BLE001 - reported verbatim
            problems.append(f"{backend.name}: {error}")
    fail("no speech backend could be loaded. " + "; ".join(problems))


def download(model_name, protocol):
    """Fetch a model, reporting how far along it is.

    Doing this here rather than letting the library do it silently on first use
    is what makes the wait explainable. The alternative is a recorder that
    appears to hang for several minutes the first time anybody speaks.
    """
    try:
        import whisper
    except ImportError:
        # faster-whisper pulls its models through huggingface_hub, which does
        # not expose progress in a form worth reporting piecemeal.
        try:
            from faster_whisper import WhisperModel
        except ImportError:
            fail("neither whisper nor faster-whisper is installed")
        reply(protocol, {"stage": "downloading", "backend": "faster-whisper"})
        WhisperModel(model_name, device="cpu", compute_type="int8")
        reply(protocol, {"ok": True, "model": model_name})
        return

    import hashlib
    import urllib.request

    urls = getattr(whisper, "_MODELS", {})
    if model_name not in urls:
        fail(f"{model_name} is not a model this engine knows about")
    url = urls[model_name]
    # whisper embeds the expected digest in the path and checks it on load, so
    # the same check here means a truncated download is caught now rather than
    # looking like a corrupt install later.
    expected = url.split("/")[-2]
    root = os.path.join(os.path.expanduser("~"), ".cache", "whisper")
    os.makedirs(root, exist_ok=True)
    target = os.path.join(root, os.path.basename(url))

    if os.path.isfile(target):
        with open(target, "rb") as existing:
            if hashlib.sha256(existing.read()).hexdigest() == expected:
                reply(protocol, {"ok": True, "model": model_name, "cached": True})
                return

    partial = target + ".part"
    digest = hashlib.sha256()
    with urllib.request.urlopen(url) as source, open(partial, "wb") as sink:
        total = int(source.headers.get("Content-Length") or 0)
        done = 0
        last_reported = -1.0
        while True:
            chunk = source.read(1 << 20)
            if not chunk:
                break
            sink.write(chunk)
            digest.update(chunk)
            done += len(chunk)
            fraction = (done / total) if total else 0.0
            # Only on a visible change, or the pipe carries more progress than
            # payload.
            if fraction - last_reported >= 0.01:
                last_reported = fraction
                reply(
                    protocol,
                    {
                        "stage": "downloading",
                        "fraction": round(fraction, 4),
                        "bytes": done,
                        "total": total,
                    },
                )

    if digest.hexdigest() != expected:
        os.remove(partial)
        fail("the downloaded model did not match its checksum; nothing was kept")

    os.replace(partial, target)
    reply(protocol, {"ok": True, "model": model_name, "path": target})


def main():
    options = json.loads(sys.argv[1]) if len(sys.argv) > 1 else {}

    # The protocol lives on the real stdout. Everything else in the process is
    # pointed at stderr from here on, because one library printing a warning to
    # stdout would be read as a malformed reply.
    protocol = sys.stdout
    sys.stdout = sys.stderr

    if options.get("action") == "download":
        download(options.get("model") or "base", protocol)
        return

    model_name = options.get("model") or "base"
    threads = int(options.get("threads") or spare_threads())
    be_polite()

    try:
        import numpy as np
    except ImportError:
        fail("numpy is missing, so the speech install is incomplete")

    backend = load_backend(model_name, threads, options.get("prefer"))

    reply(
        protocol,
        {
            "ready": True,
            "model": model_name,
            "backend": backend.name,
            "threads": threads,
        },
    )

    source = sys.stdin.buffer
    while True:
        header = source.readline()
        if not header:
            break
        try:
            job = json.loads(header)
        except ValueError:
            continue

        wanted = int(job.get("samples", 0)) * 4
        raw = b""
        while len(raw) < wanted:
            chunk = source.read(wanted - len(raw))
            if not chunk:
                return  # the caller went away mid-job
            raw += chunk

        audio = np.frombuffer(raw, dtype="<f4").astype(np.float32)

        try:
            segments, language = backend.transcribe(
                audio,
                job.get("language"),
                bool(job.get("translate")),
                job.get("prompt"),
                bool(job.get("word_timestamps")),
            )
            reply(protocol, {"ok": True, "language": language, "segments": segments})
        except Exception as error:  # noqa: BLE001 - one bad utterance is not fatal
            reply(protocol, {"ok": False, "error": str(error)})


if __name__ == "__main__":
    main()
