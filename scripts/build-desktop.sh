#!/usr/bin/env bash
# Build the desktop application for Windows, from WSL.
#
# Tauri does not officially support cross-compiling to Windows from Linux, but
# it does work with the mingw toolchain that scripts/bootstrap-toolchain.sh
# installs. Two details make the difference:
#
#   * tauri-winres shells out to windres, so the mingw bin directory has to be
#     on PATH rather than only named in .cargo/config.toml.
#   * The frontend is only embedded when the custom-protocol feature is on.
#     Without it Tauri stays in development mode and the window tries to load
#     http://localhost:5173, which is a blank "cannot reach this page".
#
# What this does not produce is an installer. Bundling msi or nsis packages
# needs the Windows tooling; this builds the executable itself, which is enough
# to run and to copy somewhere.
set -euo pipefail
cd "$(dirname "$0")/.."

PREFIX="${SSR_MINGW_PREFIX:-$HOME/.local/mingw-w64}"
if [ ! -x "$PREFIX/usr/bin/x86_64-w64-mingw32-windres" ]; then
  echo "mingw-w64 not found at $PREFIX; run scripts/bootstrap-toolchain.sh first" >&2
  exit 1
fi

echo "==> Building the interface"
npm --prefix apps/ui install --no-audit --no-fund
npm --prefix apps/ui run build

echo "==> Building the desktop shell"
FEATURES="custom-protocol"
if [ "${SSR_CONSOLE:-0}" = "1" ]; then
  # Keeps a console attached so a startup failure is visible instead of the
  # window simply never appearing.
  FEATURES="$FEATURES,console"
fi

cd apps/desktop
PATH="$PREFIX/usr/bin:$PATH" cargo build --release \
  --features "$FEATURES" \
  --target x86_64-pc-windows-gnu

BUILD="target/x86_64-pc-windows-gnu/release"
cd ../..
DIST="dist/desktop"
rm -rf "$DIST"
mkdir -p "$DIST"

cp "apps/desktop/$BUILD/ssr-desktop.exe" "$DIST/System Sound Recorder.exe"
# WebView2Loader.dll is not optional. The mingw build links it dynamically
# rather than statically, so the executable will not start unless the library
# sits beside it. Copying only the exe produces a program that silently does nothing.
cp "apps/desktop/$BUILD/WebView2Loader.dll" "$DIST/"
# The command line recorder is small and useful to have alongside.
if [ -f target/x86_64-pc-windows-gnu/release/ssr.exe ]; then
  cp target/x86_64-pc-windows-gnu/release/ssr.exe "$DIST/"
fi

ls -lh "$DIST"
echo
echo "Copy $DIST anywhere on the Windows side and run the executable."
echo "Keep WebView2Loader.dll next to it."
