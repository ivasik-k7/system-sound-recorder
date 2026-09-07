# Toolchain

## What builds where

| Component | Builds from | Toolchain |
|---|---|---|
| `ssr-core`, `ssr-timeline`, `ssr-session`, `ssr-cli` | WSL or Windows | Rust, `x86_64-pc-windows-gnu`, mingw-w64 |
| `apps/ui` | anywhere | Node, Vite |
| `apps/desktop` | WSL or Windows | Rust, `x86_64-pc-windows-gnu`, mingw-w64 |

## Cross-compiling from WSL, without root

`scripts/bootstrap-toolchain.sh` does this. What it does, and why:

Ubuntu's mingw-w64 packages are downloaded with `apt-get download`, which needs
no privileges, and unpacked with `dpkg -x` into `~/.local/mingw-w64`. Ubuntu
ships the compiler drivers with a `-win32` suffix and expects
`update-alternatives` to provide the plain names, so the script creates those
symlinks itself.

`.cargo/config.toml` then points at absolute paths, so neither `PATH` nor a
login shell has to be set up first:

```toml
[build]
target = "x86_64-pc-windows-gnu"

[target.x86_64-pc-windows-gnu]
linker = ".../x86_64-w64-mingw32-gcc"
ar = ".../x86_64-w64-mingw32-ar"
rustflags = [
  "-Cdlltool=.../x86_64-w64-mingw32-dlltool",
  "-Clink-arg=-static-libgcc",
]
```

The `dlltool` flag matters: `windows-sys` generates import libraries at build
time and shells out to it, and without the flag it is looked up on `PATH` and
the build fails with a confusing error. `-static-libgcc` avoids a runtime
dependency on `libgcc_s_seh-1.dll`, so the result is a single file that can be
copied anywhere.

Relocate the toolchain by setting `SSR_MINGW_PREFIX` before running the script,
then editing the three paths in `.cargo/config.toml`.

## Running Windows binaries from WSL

WSL registers a binfmt handler for PE executables, so `./ssr.exe` just runs, and
so do `cargo run` and `cargo test`. Two things to know:

**Environment variables do not cross the boundary.** Setting `SSR_TRACE=1` in
bash has no effect on the Windows process unless the name is listed in `WSLENV`:

```bash
WSLENV=SSR_TRACE SSR_TRACE=1 cargo run -- rec --system -d 5
```

**Paths do.** Passing a `/mnt/c/...` path works, and so does a Windows path in
quotes. Writing to `\\wsl.localhost\...` works but is slower; prefer a Windows
path for long recordings.

## The desktop shell

Tauri does not officially support cross-compiling to Windows from Linux, but it
works with the same mingw toolchain. Three details make the difference, and each
of them fails in a way that does not name its cause:

**windres must be on PATH.** `tauri-winres` shells out to it, so naming it in
`.cargo/config.toml` is not enough. Without it the build script panics with
`NotAttempted("x86_64-w64-mingw32-windres")`.

**The custom-protocol feature is what embeds the interface.** Tauri decides
between development and production by that feature alone, not by the profile.
A release build without it opens a window that tries to load
`http://localhost:5173` and shows "cannot reach this page".

**Only `crate-type = ["rlib"]`.** The Tauri template also asks for `staticlib`
and `cdylib`, which exist for the mobile targets. On a desktop build they make
the linker produce a DLL as well, and mingw's `ld` gives up on its export table
with `export ordinal too large` in debug builds.

Then at runtime: **`WebView2Loader.dll` must sit beside the executable.** The
mingw build links it dynamically where the MSVC build links it statically. The
release directory already contains a copy, and `scripts/build-desktop.sh`
assembles both into `dist/desktop`. Ship the exe on its own and it starts,
finds nothing, and exits without a message.

**The interface is built as its own step, never through a Tauri hook.**
`tauri.conf.json` used to carry `beforeBuildCommand: npm --prefix ../ui run
build`. That path is relative to the config file, but the Tauri CLI does not run
the hook from the config file's directory, so in CI it resolved against the
wrong parent and went looking for the interface at the repository root. The
error arrives as a hook failure with a path nobody wrote, which is a poor way to
learn about it. Both hooks are gone; the scripts and the workflows run
`npm run ui:build` themselves, before anything that needs `apps/ui/dist` to
exist.

One consequence: `tauri dev` no longer starts the dev server on its own. Use
`scripts/dev-desktop.sh`, which starts it and the window together.

`scripts/build-desktop.sh` handles all four. `scripts/dev-desktop.sh` runs the
opposite arrangement: a debug build without `custom-protocol`, so the window
loads from the Vite dev server running in WSL, with hot reload intact.

If you would rather use the supported path, build on Windows with the MSVC
toolchain instead:

```powershell
winget install Rustlang.Rustup
winget install Microsoft.VisualStudio.2022.BuildTools   # Desktop development with C++
winget install OpenJS.NodeJS.LTS
rustup default stable-x86_64-pc-windows-msvc
cargo install tauri-cli --version "^2"

npm --prefix apps/ui install
cargo tauri build --config apps/desktop/tauri.conf.json
```

That route also produces msi and nsis installers, which the cross-build does
not.

`apps/desktop` is excluded from the root Cargo workspace so that its dependency
set and target directory stay separate from the engine's.

For interface work none of this is needed: `scripts/dev-ui.sh` runs the whole
front end in a browser against the simulator.
