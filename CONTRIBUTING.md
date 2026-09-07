# Contributing

## What CI will ask of you

Everything below runs on every push, and all of it can be run locally first.
From the repository root:

```bash
./scripts/check-versions.sh        # the five files carrying the version agree
cargo fmt --all                    # and in apps/desktop too
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm ci && npm run ui:typecheck && npm run ui:build
cargo deny --all-features check    # licences and advisories
```

`npm run verify` is the short version: the tests and the interface typecheck.

Two gates need explaining.

**Clippy is `-D warnings`.** The tree is clean, so a new warning is yours. If a
lint is wrong about your code, `#[allow]` it at the narrowest scope with a
comment saying why — not in a config file where nobody will read it.

**cargo-deny checks licences against an allow list** in [`deny.toml`](deny.toml).
A new dependency under a licence not on that list fails, deliberately: this is a
product distributed as an installer, and the licence question is easier to answer
before it ships than after. Adding to the list is fine when the licence really is
acceptable; say so in the pull request.

## Running it

The engine is Windows-only — every crate is `#![cfg(windows)]` — but it
cross-compiles from WSL, and the resulting Windows binaries run directly from
the WSL prompt, so `cargo test` and `cargo run` work with no wrapper:

```bash
./scripts/bootstrap-toolchain.sh   # once, no root needed
./scripts/build-cli.sh
cargo run -- discover --watch
```

The interface runs in an ordinary browser against a built-in simulator, which is
how it can be worked on without Windows at all:

```bash
./scripts/dev-ui.sh                # then open the printed address
```

Useful query parameters while doing so: `?demo=1` for a running session,
`?layout=monitor`, `?panel=mixer`, `?theme=light`, `?density=compact`.

## What the code is like

The existing code has a strong house style and it is worth matching.

**Comments say why, not what.** The interesting comments in this codebase
explain a decision or record something that was learned painfully — why the
silence fill stops short of the present, why process loopback deadlocks if you
collect the activation result yourself, why `PROPVARIANT` has to be wrapped in
`ManuallyDrop`. Comments that restate the code are noise; comments that would
have saved someone a day are the point.

**Tests are named as sentences.** `a_running_but_silent_catalogued_application_is_still_offered`
says what is guaranteed. `test_discover_2` does not.

**Panels own no shared state.** Every interface panel reads what it needs from
the one store, which is what makes them freely dockable. A panel that another
panel has to be arranged around breaks the whole arrangement.

**Prefer measuring to guessing.** The claims in the documentation are measured —
5 ms of alignment between tracks, 3 ms for a discovery refresh — and were
frequently wrong on the first attempt. If you change something that a number in
the docs describes, re-measure it.

## Pull requests

Keep them to one subject, describe what you observed rather than what you
intended, and say what you ran. If you changed the capture path or the timeline
placement, say how you convinced yourself it is still correct —
`scripts/verify-session.py` measures alignment and tone accuracy against a real
recording.

If you are changing the interface, a screenshot saves a great deal of
discussion. The interface can be screenshotted without Windows; see the
simulator above.

## Releasing

Releases are cut by tag. To publish `0.2.0`:

1. Update the version in all five places — `Cargo.toml`,
   `apps/desktop/Cargo.toml`, `apps/desktop/tauri.conf.json`, `package.json`,
   `apps/ui/package.json` — and run `./scripts/check-versions.sh` to confirm.
2. Add the section to [CHANGELOG.md](CHANGELOG.md). The release notes are taken
   from it.
3. Regenerate the notices if dependencies changed:
   `python3 scripts/third-party-notices.py > THIRD-PARTY-NOTICES.md`.
4. Tag and push: `git tag v0.2.0 && git push origin v0.2.0`.

The release workflow checks the tag against the source, builds the command line
recorder and the MSI and NSIS installers on a Windows runner, writes
`SHA256SUMS.txt`, and publishes the release. Versions below 1.0 are marked as
pre-releases automatically.

The installers only exist in CI. `scripts/build-desktop.sh` cross-compiles the
executable from WSL, which is enough to run, but bundling an installer needs the
Windows tooling.

## Conduct and licensing

By contributing you agree your contribution is licensed under the
[MIT License](LICENSE), like the rest of the project. Participation is governed
by the [Code of Conduct](CODE_OF_CONDUCT.md).

One thing specific to this project: **do not attach recordings of real people**
to an issue or a pull request. If you need to demonstrate an audio problem,
record a tone or your own voice. See [LEGAL.md](LEGAL.md).
