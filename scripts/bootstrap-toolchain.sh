#!/usr/bin/env bash
# Install everything needed to build the Windows binaries from WSL, without root.
#
# Ubuntu's mingw-w64 packages are fetched and unpacked into the home directory
# rather than installed system-wide, so no password is required. Re-running is
# safe and cheap.
set -euo pipefail

PREFIX="${SSR_MINGW_PREFIX:-$HOME/.local/mingw-w64}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

PACKAGES=(
  mingw-w64-common
  mingw-w64-x86-64-dev
  binutils-mingw-w64-x86-64
  gcc-mingw-w64-base
  gcc-mingw-w64-x86-64-win32-runtime
  gcc-mingw-w64-x86-64-win32
)

echo "==> Installing the Rust target"
rustup target add x86_64-pc-windows-gnu

if [ -x "$PREFIX/usr/bin/x86_64-w64-mingw32-gcc" ]; then
  echo "==> mingw-w64 already present at $PREFIX"
else
  echo "==> Downloading mingw-w64 packages"
  (cd "$WORK" && apt-get download "${PACKAGES[@]}")

  echo "==> Unpacking into $PREFIX"
  mkdir -p "$PREFIX"
  for deb in "$WORK"/*.deb; do
    dpkg -x "$deb" "$PREFIX"
  done

  # Ubuntu ships the compiler drivers with a -win32 suffix and expects
  # update-alternatives to provide the plain names. Do that ourselves.
  ( cd "$PREFIX/usr/bin"
    for f in *-win32; do
      [ -e "${f%-win32}" ] || ln -s "$f" "${f%-win32}"
    done )
fi

echo "==> Verifying"
printf 'int main(void){return 0;}\n' > "$WORK/probe.c"
"$PREFIX/usr/bin/x86_64-w64-mingw32-gcc" "$WORK/probe.c" -o "$WORK/probe.exe"
file "$WORK/probe.exe" | grep -q 'PE32+' && echo "    cross compiler works"

cat <<EOF

Done. The toolchain lives in $PREFIX and .cargo/config.toml points at it.

  cargo build --release      builds ssr.exe for Windows
  ./target/x86_64-pc-windows-gnu/release/ssr.exe devices

Windows executables run directly from WSL, so cargo run and cargo test work
without any extra step.
EOF
