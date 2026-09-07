## What this changes

<!-- One or two sentences. What is different afterwards. -->

## Why

<!-- The problem. If it fixes an issue, "Fixes #123". -->

## How you know it works

<!--
What you actually ran, and what you observed. For the capture path or the
timeline, say how you convinced yourself it is still correct —
scripts/verify-session.py measures track alignment and tone accuracy against a
real recording.

For interface changes, a screenshot saves a lot of discussion. The interface can
be screenshotted without Windows; see CONTRIBUTING.md.
-->

## Checks

- [ ] `cargo fmt --all` in both workspaces
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is clean
- [ ] `cargo test --workspace` passes
- [ ] `npm run ui:typecheck && npm run ui:build` passes, if the interface changed
- [ ] `./scripts/check-versions.sh` passes, if a version changed
- [ ] `THIRD-PARTY-NOTICES.md` regenerated, if dependencies changed
- [ ] `CHANGELOG.md` updated under Unreleased
- [ ] No recordings of real people attached anywhere
