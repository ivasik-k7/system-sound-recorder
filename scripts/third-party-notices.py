#!/usr/bin/env python3
"""Regenerate THIRD-PARTY-NOTICES.md from the dependency graphs.

Run from the repository root:

    python3 scripts/third-party-notices.py > THIRD-PARTY-NOTICES.md

A notices file written by hand goes stale on the first dependency bump, and a
stale one is worse than none: it makes a claim about what is being shipped that
is no longer true. So this reads the graphs instead.

Three graphs, because this project has three package managers. The two Cargo
workspaces are resolved for the Windows targets only — anything that is only
built on Linux or macOS is not shipped and its licence is not ours to carry.
The npm dependencies are split into what the interface bundle contains and what
merely builds it.
"""

import json
import subprocess
import sys
from pathlib import Path

WINDOWS_TARGETS = ["x86_64-pc-windows-msvc", "x86_64-pc-windows-gnu"]


def cargo_dependencies(manifest: str) -> list[tuple[str, str, str]]:
    """Third-party crates in a workspace, as (name, version, licence)."""
    # --filter-platform keeps the answer to what is actually built for Windows.
    # It takes one triple, so the union of both is collected.
    seen: dict[tuple[str, str], str] = {}
    for triple in WINDOWS_TARGETS:
        result = subprocess.run(
            [
                "cargo", "metadata", "--format-version", "1", "--all-features",
                "--filter-platform", triple, "--manifest-path", manifest,
            ],
            capture_output=True, text=True, check=True,
        )
        metadata = json.loads(result.stdout)
        local = {p["name"] for p in metadata["packages"] if p.get("source") is None}
        for package in metadata["packages"]:
            if package["name"] in local:
                continue
            licence = package.get("license") or "see the crate's repository"
            seen[(package["name"], package["version"])] = licence
    return sorted((name, version, licence) for (name, version), licence in seen.items())


def npm_dependencies() -> tuple[list[tuple[str, str, str]], list[tuple[str, str, str]]]:
    """Interface packages, split into shipped and build-only."""
    manifest = json.loads(Path("apps/ui/package.json").read_text())
    modules = Path("apps/ui/node_modules")

    def describe(names: list[str]) -> list[tuple[str, str, str]]:
        rows = []
        for name in sorted(names):
            path = modules / name / "package.json"
            if not path.exists():
                rows.append((name, "not installed", "unknown"))
                continue
            data = json.loads(path.read_text())
            licence = data.get("license") or "see the package's repository"
            if isinstance(licence, dict):  # the old object form
                licence = licence.get("type", "unknown")
            rows.append((name, data.get("version", "?"), licence))
        return rows

    return (
        describe(list(manifest.get("dependencies", {}))),
        describe(list(manifest.get("devDependencies", {}))),
    )


def table(rows: list[tuple[str, str, str]]) -> None:
    print()
    print("| Package | Version | Licence |")
    print("|---|---|---|")
    for name, version, licence in rows:
        print(f"| `{name}` | {version} | {licence} |")
    print()


def main() -> int:
    if not Path("Cargo.toml").exists():
        print("run this from the repository root", file=sys.stderr)
        return 2

    engine = cargo_dependencies("Cargo.toml")
    desktop = cargo_dependencies("apps/desktop/Cargo.toml")
    # A crate used by both is listed once, under the engine.
    engine_keys = {(n, v) for n, v, _ in engine}
    desktop_only = [row for row in desktop if (row[0], row[1]) not in engine_keys]
    shipped, build_only = npm_dependencies()

    print("# Third-party notices")
    print()
    print("System Sound Recorder is distributed under the MIT License; see")
    print("[LICENSE](LICENSE). It also includes, or is built with, the third-party")
    print("software listed here, each under its own licence.")
    print()
    print("This file is generated. To refresh it after a dependency change:")
    print()
    print("```bash")
    print("python3 scripts/third-party-notices.py > THIRD-PARTY-NOTICES.md")
    print("```")
    print()
    print("Only what is built for Windows is listed: the Cargo graphs are resolved")
    print("for `x86_64-pc-windows-msvc` and `x86_64-pc-windows-gnu`, so crates that")
    print("exist in the lockfile for other platforms and are never compiled here are")
    print("left out.")
    print()

    print("## Worth reading before you redistribute")
    print()
    print("Nearly everything below is permissive — MIT, Apache-2.0, BSD, ISC, Zlib")
    print("or equivalent — and satisfied by shipping this file.")
    print()
    print("Two things are not quite that:")
    print()
    print("**MPL-2.0** covers a handful of crates reached through the desktop")
    print("shell's dependency on Tauri: the CSS parsing crates `cssparser`,")
    print("`cssparser-macros`, `selectors` and `dtoa-short`, and `option-ext` by way")
    print("of `dirs`. The Mozilla Public License is file-level copyleft: it obliges")
    print("whoever distributes it to make the source of *those files* available, and")
    print("it reaches no further into the program. They are used unmodified, so")
    print("pointing at the upstream crates satisfies it. If you patch one, publish")
    print("your patch.")
    print()
    print("**Unicode-3.0** covers the ICU crates and `unicode-ident`. It is")
    print("permissive and requires the notice be kept, which is what this file is")
    print("for.")
    print()
    print("A licence not on the allow list in [`deny.toml`](deny.toml) fails CI, so")
    print("this list cannot quietly acquire a licence nobody looked at.")
    print()

    print(f"## The engine ({len(engine)} crates)")
    print()
    print("Used by the command line recorder and by the desktop shell.")
    table(engine)

    print(f"## The desktop shell ({len(desktop_only)} further crates)")
    print()
    print("Tauri and its dependencies, on top of the engine's.")
    table(desktop_only)

    print(f"## The interface, shipped ({len(shipped)} packages)")
    print()
    print("Bundled into the interface that the desktop shell embeds.")
    table(shipped)

    print(f"## The interface, build only ({len(build_only)} packages)")
    print()
    print("Used to build the interface. Not present in the shipped product.")
    table(build_only)
    return 0


if __name__ == "__main__":
    sys.exit(main())
