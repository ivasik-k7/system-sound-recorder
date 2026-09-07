# Privacy

The short answer: this program has no server, sends nothing anywhere, and
collects nothing about you. What follows is the long answer, because "we don't
collect anything" is a claim worth being able to check.

## What leaves your machine

Nothing.

There is no telemetry, no crash reporting, no analytics, no update check, no
licence check and no account. The program has no network client of any kind. You
can verify this the way anyone should verify such a claim — by watching it with
a firewall or a network monitor, or by searching the source for an HTTP client
and not finding one.

The desktop application runs a webview to draw its interface, and that webview
is restricted by a content security policy to loading the interface itself from
disk: `default-src 'self'` with no permitted external origin. It cannot fetch a
font, an icon or a script from the internet, which is also why the interface
embeds its own.

## What is written to disk, and where

**Recordings.** Where you tell it to, and nowhere else. By default that is a
`System Sound Recorder` folder inside your Documents directory, with one dated
subfolder per session containing a WAV file per track and a `session.json`
manifest. Nothing is written to a temporary directory in passing.

**The session manifest** records what was captured: the track names, what each
one tapped — an endpoint name or an executable name such as `wfica32.exe` — the
sample format, timing statistics, and any cue points you marked. It contains no
audio and nothing about you beyond what you chose to record.

**Interface settings.** The theme, density, panel layout and meter preferences
are kept in the webview's local storage, which lives inside the application's own
data directory. They are preferences, not history: no record of what you
recorded or when is kept there.

**Transcripts**, if you use the transcription feature, are written next to the
audio they came from.

That is the complete list. The program creates no other files, no database, and
no log on disk.

## What it reads

To answer "what can I record?", the program asks Windows two questions on a
timer while it is running:

- the audio endpoints present, and their formats;
- the audio sessions currently open, which yields the process id, the
  executable name and the live level of every application playing or recording
  sound.

That means the program can see, while it is open, that you have a browser and a
conferencing client making noise. It uses this to populate the list you choose
from, holds it in memory, and neither writes it down nor sends it anywhere.

It also reads the process list to turn a process id into an executable name, and
the free space on the volume you are recording to.

It does not read the contents of other applications' memory, their windows,
their titles, or anything you type.

## What it records

Only the sources you add to a session, and only while a session is running.

The program cannot record without a visible session: there is no background
service, no hidden capture, and no way to arm it invisibly. The window shows a
clock and a level meter per track for as long as recording is happening, and
closing the main window stops the session rather than detaching it.

The one automatic behaviour worth naming is optional and off by default: *arm
remote sessions automatically* adds a remote session to the plan when one starts
playing. It adds a track to the list; it does not start recording.

## Microphones

A microphone is only captured if you add it as a source. Windows shows the
microphone-in-use indicator whenever it is, as it does for any application.

## Other people

A recording of a meeting is personal data about everyone audible in it, and the
obligations that come with that are yours rather than the program's — including
telling people, keeping the file somewhere sensible, and deleting it when it has
served its purpose. [LEGAL.md](LEGAL.md) covers what to consider.

## Transcription and third parties

Transcription uses a speech engine installed on your machine, chosen from
whatever is already there. Audio does not leave the machine to be transcribed.

If you point it at a cloud speech service instead, that is a transfer of the
audio to a third party, and their privacy terms then apply to it as well as
yours. The program does not do this for you.

## Changes

If this ever stops being true — if a version adds an update check, say — it will
be in the [changelog](CHANGELOG.md) as a change, not a footnote, and this file
will say so.
