# Security

## Reporting a vulnerability

Please report security issues privately, through GitHub's
[private vulnerability reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing-information-about-vulnerabilities/privately-reporting-a-security-vulnerability)
on this repository: **Security → Report a vulnerability**. That opens a
discussion visible only to the maintainers.

Please do not open a public issue for anything that would let someone record
audio they should not be able to, or read a recording they should not be able to.

Include what you need to make the problem reproducible: the version, the
Windows build, and the steps. A proof of concept helps; recorded audio of a real
third party does not, and please do not attach any.

**Expect a first response within a week.** If a report is confirmed, the fix and
an advisory go out together, and you will be credited unless you would rather
not be.

## What counts

The interesting boundaries in this program are narrow, which is deliberate.

**In scope:**

- Capturing audio without a session visibly running, or after one has stopped.
- Escaping the desktop application's content security policy — running remote
  code in the webview, or reaching the network from it.
- Reading or writing files outside the folder the user chose, through a track
  name, a session name, or a path in a manifest.
- Escalating from the interface to the operating system through a Tauri command:
  every command the interface can call is in
  `apps/desktop/src/lib.rs`, and the capability set it is allowed is in
  `apps/desktop/capabilities/default.json`.
- Memory-safety faults in the capture path. It contains real `unsafe`, because
  WASAPI is a COM API, and the interesting cases are documented at the site in
  `crates/ssr-core/src/process.rs`.
- A dependency vulnerability that is actually reachable from this program.

**Not in scope:**

- That the program records audio at all, including audio from remote sessions.
  That is the purpose, it requires no privilege beyond the user's own, and it is
  discussed in [LEGAL.md](LEGAL.md).
- That a recording is unencrypted on disk. It is an ordinary user file in a
  folder the user picked.
- That a user who is already an administrator can do administrator things.
- Anything requiring an attacker who can already run code as the user. Such an
  attacker can record audio with Windows' own APIs and does not need this.
- Unmaintained-but-not-vulnerable dependencies. CI reports these; they are not
  advisories.

## What the program is allowed to do

Worth stating, because it bounds the damage.

It runs entirely as the invoking user and asks for no elevation. It opens no
listening socket and makes no outbound connection. The desktop shell's webview
is confined by a content security policy to its own bundled assets, and its
Tauri capability set is deliberately minimal — driving its own windows and
opening the output folder in the file manager.

Recording someone else's process audio through WASAPI process loopback needs no
special privilege on Windows. That is the platform's design, not this program's
choice.

## Supported versions

Pre-1.0, only the latest release is supported. Fixes go to the next release
rather than to older tags.
