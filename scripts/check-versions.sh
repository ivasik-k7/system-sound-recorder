#!/usr/bin/env bash
# Check that every file carrying the product version agrees.
#
# The version lives in five places, because three build systems each want it in
# their own file and two of them cannot read the others:
#
#   Cargo.toml                    the engine workspace
#   apps/desktop/Cargo.toml       the desktop shell, a separate workspace
#   apps/desktop/tauri.conf.json  what the installer and the window report
#   package.json                  the npm workspace root
#   apps/ui/package.json          the interface
#
# Nothing enforces that they match, and the failure is quiet: an installer that
# says 0.1.0 while the binary inside it says 0.2.0 is not obviously wrong until
# someone is trying to work out which build a bug report came from. So this is a
# release gate.
#
# With an argument, also checks that the version matches it. The release
# workflow passes the tag, so a tag that does not match the source fails before
# anything is published rather than after.
#
#   scripts/check-versions.sh          # just check they agree
#   scripts/check-versions.sh v0.2.0   # and that they are 0.2.0
set -euo pipefail
cd "$(dirname "$0")/.."

fail=0

# The first `version = "x.y.z"` in a TOML file, which for these files is the
# package's own rather than a dependency's.
toml_version() {
  grep -m1 -E '^version[[:space:]]*=' "$1" | sed -E 's/.*"([^"]+)".*/\1/'
}

json_version() {
  grep -m1 -E '^[[:space:]]*"version"[[:space:]]*:' "$1" | sed -E 's/.*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/'
}

declare -A found=(
  ["Cargo.toml"]="$(toml_version Cargo.toml)"
  ["apps/desktop/Cargo.toml"]="$(toml_version apps/desktop/Cargo.toml)"
  ["apps/desktop/tauri.conf.json"]="$(json_version apps/desktop/tauri.conf.json)"
  ["package.json"]="$(json_version package.json)"
  ["apps/ui/package.json"]="$(json_version apps/ui/package.json)"
)

reference="${found[Cargo.toml]}"
if [ -z "$reference" ]; then
  echo "error: no version found in Cargo.toml" >&2
  exit 1
fi

echo "Version according to each file:"
for file in "${!found[@]}"; do
  value="${found[$file]}"
  if [ "$value" = "$reference" ]; then
    printf '  %-32s %s\n' "$file" "$value"
  else
    printf '  %-32s %s  <-- expected %s\n' "$file" "${value:-<missing>}" "$reference"
    fail=1
  fi
done

# An expected version, if one was given. A leading v is accepted because that is
# how the tags are written.
if [ "$#" -ge 1 ]; then
  expected="${1#v}"
  echo
  if [ "$reference" != "$expected" ]; then
    echo "error: the source says $reference but $expected was expected" >&2
    fail=1
  else
    echo "Matches the expected version $expected."
  fi
fi

if [ "$fail" -ne 0 ]; then
  echo
  echo "error: the version is not consistent; fix the files above" >&2
  exit 1
fi

echo
echo "Consistent at $reference."
