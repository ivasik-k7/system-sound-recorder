# Changelog

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project uses [semantic versioning](https://semver.org/spec/v2.0.0.html).
While the version is below 1.0, the interfaces — including the manifest format —
may change between minor versions.

The release workflow reads the section for the version being tagged and uses it
as the release notes, so a section that says nothing produces a release that
says nothing.

## [Unreleased]

### Added

- Continuous integration: formatting, clippy at `-D warnings`, the tests on
  Windows, the interface typecheck and build, a cross-compile from Linux
  matching the documented WSL workflow, the declared minimum Rust versions, and
  a licence and advisory check.
- A release workflow. Tagging `vX.Y.Z` builds the command line recorder and the
  MSI and NSIS installers on a Windows runner, writes `SHA256SUMS.txt`, and
  publishes a GitHub Release. The installers exist only here: the local
  cross-compile from WSL cannot bundle them.
- `scripts/check-versions.sh`, because the version lives in five files and
  nothing was checking that they agreed.
- `scripts/third-party-notices.py`, which generates `THIRD-PARTY-NOTICES.md`
  from the dependency graphs rather than from memory.
- Legal and policy documents: `LICENSE`, `LEGAL.md` on recording other people,
  `PRIVACY.md` on what the program does and does not do with data,
  `SECURITY.md`, `CONTRIBUTING.md` and `CODE_OF_CONDUCT.md`.
- `deny.toml`, a dependency policy whose licence allow list is the set the tree
  actually uses.

### Changed

- The declared minimum Rust version was 1.82 and was not true. The engine needs
  1.85, because clap requires edition 2024; the desktop shell needs 1.88,
  because of the bundler's dependencies. Both are now declared correctly and
  checked on every push.
- The whole tree is `rustfmt`-clean, which it had never been.
- The crates are marked `publish = false`, which they are: they depend on each
  other by path, and crates.io does not allow that.
- A root `package-lock.json` now exists, so the interface can be installed
  reproducibly. The root declared npm workspaces but carried no lockfile, which
  meant `npm ci` could not work anywhere.

### Fixed

- Five clippy findings, all mechanical.
