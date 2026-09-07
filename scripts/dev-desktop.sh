#!/usr/bin/env bash
# Run the desktop application against a live interface.
#
# Tauri's own `tauri dev` cannot help here: from WSL it would try to build a
# Linux application, and the tauri CLI is not required for any of this. The loop
# below does the same job.
#
# Built without the custom-protocol feature, the shell stays in development mode
# and loads its interface from http://localhost:5173 instead of embedding it.
# Windows can reach a port listening inside WSL, so the Vite dev server runs
# here and the window runs there, with hot reload intact.
set -euo pipefail
cd "$(dirname "$0")/.."

PREFIX="${SSR_MINGW_PREFIX:-$HOME/.local/mingw-w64}"
if [ ! -x "$PREFIX/usr/bin/x86_64-w64-mingw32-windres" ]; then
  echo "mingw-w64 not found at $PREFIX; run scripts/bootstrap-toolchain.sh first" >&2
  exit 1
fi

EXE="apps/desktop/target/x86_64-pc-windows-gnu/debug/ssr-desktop.exe"

echo "==> Building the desktop shell in development mode"
( cd apps/desktop
  PATH="$PREFIX/usr/bin:$PATH" cargo build --target x86_64-pc-windows-gnu )

echo "==> Starting the interface on http://localhost:5173"
[ -d apps/ui/node_modules ] || npm --prefix apps/ui install --no-audit --no-fund
npm --prefix apps/ui run dev &
VITE_PID=$!
trap 'kill "$VITE_PID" 2>/dev/null || true' EXIT

# Wait for the dev server rather than guessing how long it takes.
for _ in $(seq 1 60); do
  if curl -sf -o /dev/null http://localhost:5173/; then break; fi
  sleep 0.5
done

echo "==> Launching the window"
echo "    Edit anything under apps/ui/src and the window reloads."
echo "    Ctrl-C here stops both."
"./$EXE"
