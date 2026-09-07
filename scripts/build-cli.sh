#!/usr/bin/env bash
# Build the command line recorder for Windows.
set -euo pipefail
cd "$(dirname "$0")/.."

PROFILE="${1:-release}"
case "$PROFILE" in
  release) cargo build --release ;;
  debug)   cargo build ;;
  *) echo "usage: $0 [release|debug]" >&2; exit 2 ;;
esac

OUT="target/x86_64-pc-windows-gnu/$PROFILE/ssr.exe"
ls -lh "$OUT"
echo
echo "Try it:"
echo "  $OUT devices"
echo "  $OUT apps"
echo "  $OUT rec --system --app wfica32.exe -o recordings/meeting"
