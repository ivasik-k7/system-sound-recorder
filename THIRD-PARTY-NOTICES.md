# Third-party notices

System Sound Recorder is distributed under the MIT License; see
[LICENSE](LICENSE). It also includes, or is built with, the third-party
software listed here, each under its own licence.

This file is generated. To refresh it after a dependency change:

```bash
python3 scripts/third-party-notices.py > THIRD-PARTY-NOTICES.md
```

Only what is built for Windows is listed: the Cargo graphs are resolved
for `x86_64-pc-windows-msvc` and `x86_64-pc-windows-gnu`, so crates that
exist in the lockfile for other platforms and are never compiled here are
left out.

## Worth reading before you redistribute

Nearly everything below is permissive — MIT, Apache-2.0, BSD, ISC, Zlib
or equivalent — and satisfied by shipping this file.

Two things are not quite that:

**MPL-2.0** covers a handful of crates reached through the desktop
shell's dependency on Tauri: the CSS parsing crates `cssparser`,
`cssparser-macros`, `selectors` and `dtoa-short`, and `option-ext` by way
of `dirs`. The Mozilla Public License is file-level copyleft: it obliges
whoever distributes it to make the source of *those files* available, and
it reaches no further into the program. They are used unmodified, so
pointing at the upstream crates satisfies it. If you patch one, publish
your patch.

**Unicode-3.0** covers the ICU crates and `unicode-ident`. It is
permissive and requires the notice be kept, which is what this file is
for.

A licence not on the allow list in [`deny.toml`](deny.toml) fails CI, so
this list cannot quietly acquire a licence nobody looked at.

## The engine (43 crates)

Used by the command line recorder and by the desktop shell.

| Package | Version | Licence |
|---|---|---|
| `anstream` | 1.0.0 | MIT OR Apache-2.0 |
| `anstyle` | 1.0.14 | MIT OR Apache-2.0 |
| `anstyle-parse` | 1.0.0 | MIT OR Apache-2.0 |
| `anstyle-query` | 1.1.5 | MIT OR Apache-2.0 |
| `anstyle-wincon` | 3.0.11 | MIT OR Apache-2.0 |
| `anyhow` | 1.0.104 | MIT OR Apache-2.0 |
| `clap` | 4.6.6 | MIT OR Apache-2.0 |
| `clap_builder` | 4.6.6 | MIT OR Apache-2.0 |
| `clap_derive` | 4.6.4 | MIT OR Apache-2.0 |
| `clap_lex` | 1.1.0 | MIT OR Apache-2.0 |
| `colorchoice` | 1.0.5 | MIT OR Apache-2.0 |
| `crossbeam-channel` | 0.5.17 | MIT OR Apache-2.0 |
| `crossbeam-utils` | 0.8.23 | MIT OR Apache-2.0 |
| `ctrlc` | 3.5.2 | MIT/Apache-2.0 |
| `heck` | 0.5.0 | MIT OR Apache-2.0 |
| `is_terminal_polyfill` | 1.70.2 | MIT OR Apache-2.0 |
| `itoa` | 1.0.18 | MIT OR Apache-2.0 |
| `memchr` | 2.8.3 | Unlicense OR MIT |
| `once_cell_polyfill` | 1.70.2 | MIT OR Apache-2.0 |
| `proc-macro2` | 1.0.107 | MIT OR Apache-2.0 |
| `quote` | 1.0.47 | MIT OR Apache-2.0 |
| `serde` | 1.0.229 | MIT OR Apache-2.0 |
| `serde_core` | 1.0.229 | MIT OR Apache-2.0 |
| `serde_derive` | 1.0.229 | MIT OR Apache-2.0 |
| `serde_json` | 1.0.151 | MIT OR Apache-2.0 |
| `strsim` | 0.11.1 | MIT |
| `syn` | 2.0.119 | MIT OR Apache-2.0 |
| `syn` | 3.0.5 | MIT OR Apache-2.0 |
| `unicode-ident` | 1.0.24 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| `utf8parse` | 0.2.2 | Apache-2.0 OR MIT |
| `windows` | 0.62.2 | MIT OR Apache-2.0 |
| `windows-collections` | 0.3.2 | MIT OR Apache-2.0 |
| `windows-core` | 0.62.2 | MIT OR Apache-2.0 |
| `windows-future` | 0.3.2 | MIT OR Apache-2.0 |
| `windows-implement` | 0.60.2 | MIT OR Apache-2.0 |
| `windows-interface` | 0.59.3 | MIT OR Apache-2.0 |
| `windows-link` | 0.2.1 | MIT OR Apache-2.0 |
| `windows-numerics` | 0.3.1 | MIT OR Apache-2.0 |
| `windows-result` | 0.4.1 | MIT OR Apache-2.0 |
| `windows-strings` | 0.5.1 | MIT OR Apache-2.0 |
| `windows-sys` | 0.61.2 | MIT OR Apache-2.0 |
| `windows-threading` | 0.2.1 | MIT OR Apache-2.0 |
| `zmij` | 1.0.23 | MIT |

## The desktop shell (250 further crates)

Tauri and its dependencies, on top of the engine's.

| Package | Version | Licence |
|---|---|---|
| `adler2` | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| `aho-corasick` | 1.1.5 | Unlicense OR MIT |
| `alloc-no-stdlib` | 2.0.4 | BSD-3-Clause |
| `alloc-stdlib` | 0.2.4 | BSD-3-Clause |
| `autocfg` | 1.5.1 | Apache-2.0 OR MIT |
| `base64` | 0.22.1 | MIT OR Apache-2.0 |
| `base64` | 0.23.1 | MIT OR Apache-2.0 |
| `bit-set` | 0.8.0 | Apache-2.0 OR MIT |
| `bit-vec` | 0.8.0 | Apache-2.0 OR MIT |
| `bitflags` | 1.3.2 | MIT/Apache-2.0 |
| `bitflags` | 2.13.1 | MIT OR Apache-2.0 |
| `block-buffer` | 0.10.4 | MIT OR Apache-2.0 |
| `brotli` | 8.0.4 | BSD-3-Clause AND MIT |
| `brotli-decompressor` | 5.0.3 | BSD-3-Clause/MIT |
| `bs58` | 0.5.1 | MIT/Apache-2.0 |
| `byteorder` | 1.5.0 | Unlicense OR MIT |
| `bytes` | 1.12.1 | MIT |
| `camino` | 1.2.5 | MIT OR Apache-2.0 |
| `cargo-platform` | 0.1.9 | MIT OR Apache-2.0 |
| `cargo_metadata` | 0.19.2 | MIT |
| `cargo_toml` | 0.22.3 | Apache-2.0 OR MIT |
| `cc` | 1.4.5 | MIT OR Apache-2.0 |
| `cfb` | 0.7.3 | MIT |
| `cfg-if` | 1.0.4 | MIT OR Apache-2.0 |
| `chrono` | 0.4.45 | MIT OR Apache-2.0 |
| `cookie` | 0.18.2 | MIT OR Apache-2.0 |
| `cpufeatures` | 0.2.17 | MIT OR Apache-2.0 |
| `crc32fast` | 1.5.1 | MIT OR Apache-2.0 |
| `crypto-common` | 0.1.7 | MIT OR Apache-2.0 |
| `cssparser` | 0.36.0 | MPL-2.0 |
| `cssparser-macros` | 0.6.1 | MPL-2.0 |
| `ctor` | 0.8.0 | Apache-2.0 OR MIT |
| `ctor-proc-macro` | 0.0.7 | Apache-2.0 OR MIT |
| `darling` | 0.23.0 | MIT |
| `darling_core` | 0.23.0 | MIT |
| `darling_macro` | 0.23.0 | MIT |
| `defmt` | 1.1.1 | MIT OR Apache-2.0 |
| `defmt-macros` | 1.1.1 | MIT OR Apache-2.0 |
| `defmt-parser` | 1.0.0 | MIT OR Apache-2.0 |
| `deranged` | 0.5.8 | MIT OR Apache-2.0 |
| `derive_more` | 2.1.1 | MIT |
| `derive_more-impl` | 2.1.1 | MIT |
| `digest` | 0.10.7 | MIT OR Apache-2.0 |
| `dirs` | 6.0.0 | MIT OR Apache-2.0 |
| `dirs-sys` | 0.5.0 | MIT OR Apache-2.0 |
| `displaydoc` | 0.2.7 | MIT OR Apache-2.0 |
| `dom_query` | 0.27.0 | MIT |
| `dpi` | 0.1.2 | Apache-2.0 AND MIT |
| `dtoa` | 1.0.11 | MIT OR Apache-2.0 |
| `dtoa-short` | 0.3.5 | MPL-2.0 |
| `dtor` | 0.3.0 | Apache-2.0 OR MIT |
| `dtor-proc-macro` | 0.0.6 | Apache-2.0 OR MIT |
| `dunce` | 1.0.5 | CC0-1.0 OR MIT-0 OR Apache-2.0 |
| `dyn-clone` | 1.0.20 | MIT OR Apache-2.0 |
| `embed-resource` | 3.0.11 | MIT |
| `equivalent` | 1.0.2 | Apache-2.0 OR MIT |
| `erased-serde` | 0.4.10 | MIT OR Apache-2.0 |
| `fastrand` | 2.5.0 | Apache-2.0 OR MIT |
| `fdeflate` | 0.3.7 | MIT OR Apache-2.0 |
| `find-msvc-tools` | 0.1.12 | MIT OR Apache-2.0 |
| `flate2` | 1.1.10 | MIT OR Apache-2.0 |
| `fnv` | 1.0.7 | Apache-2.0 / MIT |
| `foldhash` | 0.2.0 | Zlib |
| `form_urlencoded` | 1.2.2 | MIT OR Apache-2.0 |
| `generic-array` | 0.14.7 | MIT |
| `getrandom` | 0.3.4 | MIT OR Apache-2.0 |
| `getrandom` | 0.4.3 | MIT OR Apache-2.0 |
| `glob` | 0.3.4 | MIT OR Apache-2.0 |
| `hashbrown` | 0.12.3 | MIT OR Apache-2.0 |
| `hashbrown` | 0.17.1 | MIT OR Apache-2.0 |
| `hex` | 0.4.3 | MIT OR Apache-2.0 |
| `html5ever` | 0.38.0 | MIT OR Apache-2.0 |
| `http` | 1.5.0 | MIT OR Apache-2.0 |
| `ico` | 0.5.0 | MIT |
| `icu_collections` | 2.3.0 | Unicode-3.0 |
| `icu_locale_core` | 2.3.0 | Unicode-3.0 |
| `icu_normalizer` | 2.3.0 | Unicode-3.0 |
| `icu_normalizer_data` | 2.3.0 | Unicode-3.0 |
| `icu_properties` | 2.3.0 | Unicode-3.0 |
| `icu_properties_data` | 2.3.0 | Unicode-3.0 |
| `icu_provider` | 2.3.1 | Unicode-3.0 |
| `ident_case` | 1.0.1 | MIT/Apache-2.0 |
| `idna` | 1.1.0 | MIT OR Apache-2.0 |
| `idna_adapter` | 1.2.2 | Apache-2.0 OR MIT |
| `indexmap` | 1.9.3 | Apache-2.0 OR MIT |
| `indexmap` | 2.14.2 | Apache-2.0 OR MIT |
| `infer` | 0.19.0 | MIT |
| `jiff` | 0.2.35 | Unlicense OR MIT |
| `jiff-core` | 0.1.0 | Unlicense OR MIT |
| `jiff-tzdb` | 0.1.8 | Unlicense OR MIT |
| `jiff-tzdb-platform` | 0.1.3 | Unlicense OR MIT |
| `json-patch` | 3.0.1 | MIT/Apache-2.0 |
| `jsonptr` | 0.6.3 | MIT OR Apache-2.0 |
| `keyboard-types` | 0.7.0 | MIT OR Apache-2.0 |
| `libc` | 0.2.189 | MIT OR Apache-2.0 |
| `litemap` | 0.8.3 | Unicode-3.0 |
| `lock_api` | 0.4.14 | MIT OR Apache-2.0 |
| `log` | 0.4.34 | MIT OR Apache-2.0 |
| `markup5ever` | 0.38.0 | MIT OR Apache-2.0 |
| `mime` | 0.3.17 | MIT OR Apache-2.0 |
| `miniz_oxide` | 0.8.9 | MIT OR Zlib OR Apache-2.0 |
| `miniz_oxide` | 0.9.1 | MIT OR Zlib OR Apache-2.0 |
| `mio` | 1.2.3 | MIT |
| `muda` | 0.19.3 | Apache-2.0 OR MIT |
| `new_debug_unreachable` | 1.0.6 | MIT |
| `num-conv` | 0.2.2 | MIT OR Apache-2.0 |
| `num-traits` | 0.2.19 | MIT OR Apache-2.0 |
| `once_cell` | 1.21.4 | MIT OR Apache-2.0 |
| `open` | 5.4.3 | MIT |
| `option-ext` | 0.2.0 | MPL-2.0 |
| `parking_lot` | 0.12.5 | MIT OR Apache-2.0 |
| `parking_lot_core` | 0.9.12 | MIT OR Apache-2.0 |
| `percent-encoding` | 2.3.2 | MIT OR Apache-2.0 |
| `phf` | 0.13.1 | MIT |
| `phf_codegen` | 0.13.1 | MIT |
| `phf_generator` | 0.13.1 | MIT |
| `phf_macros` | 0.13.1 | MIT |
| `phf_shared` | 0.13.1 | MIT |
| `pin-project-lite` | 0.2.17 | Apache-2.0 OR MIT |
| `plist` | 1.10.1 | MIT |
| `png` | 0.17.16 | MIT OR Apache-2.0 |
| `potential_utf` | 0.1.6 | Unicode-3.0 |
| `powerfmt` | 0.2.0 | MIT OR Apache-2.0 |
| `precomputed-hash` | 0.1.1 | MIT |
| `quick-xml` | 0.42.0 | MIT |
| `raw-window-handle` | 0.6.2 | MIT OR Apache-2.0 OR Zlib |
| `ref-cast` | 1.0.27 | MIT OR Apache-2.0 |
| `ref-cast-impl` | 1.0.27 | MIT OR Apache-2.0 |
| `regex` | 1.13.1 | MIT OR Apache-2.0 |
| `regex-automata` | 0.4.18 | MIT OR Apache-2.0 |
| `regex-syntax` | 0.8.11 | MIT OR Apache-2.0 |
| `rfd` | 0.16.0 | MIT |
| `rustc-hash` | 2.1.3 | Apache-2.0 OR MIT |
| `rustc_version` | 0.4.1 | MIT OR Apache-2.0 |
| `same-file` | 1.0.6 | Unlicense/MIT |
| `schemars` | 0.8.22 | MIT |
| `schemars` | 0.9.0 | MIT |
| `schemars` | 1.2.2 | MIT |
| `schemars_derive` | 0.8.22 | MIT |
| `scopeguard` | 1.2.0 | MIT OR Apache-2.0 |
| `selectors` | 0.36.1 | MPL-2.0 |
| `semver` | 1.0.28 | MIT OR Apache-2.0 |
| `serde-untagged` | 0.1.9 | MIT OR Apache-2.0 |
| `serde_derive_internals` | 0.29.1 | MIT OR Apache-2.0 |
| `serde_repr` | 0.1.21 | MIT OR Apache-2.0 |
| `serde_spanned` | 1.1.1 | MIT OR Apache-2.0 |
| `serde_with` | 3.22.0 | MIT OR Apache-2.0 |
| `serde_with_macros` | 3.22.0 | MIT OR Apache-2.0 |
| `serialize-to-javascript` | 0.1.2 | MIT OR Apache-2.0 |
| `serialize-to-javascript-impl` | 0.1.2 | MIT OR Apache-2.0 |
| `servo_arc` | 0.4.3 | MIT OR Apache-2.0 |
| `sha2` | 0.10.9 | MIT OR Apache-2.0 |
| `shlex` | 2.0.1 | MIT OR Apache-2.0 |
| `simd-adler32` | 0.3.10 | MIT |
| `siphasher` | 1.0.3 | MIT/Apache-2.0 |
| `smallvec` | 1.16.0 | MIT OR Apache-2.0 |
| `socket2` | 0.6.5 | MIT OR Apache-2.0 |
| `softbuffer` | 0.4.8 | MIT OR Apache-2.0 |
| `stable_deref_trait` | 1.2.1 | MIT OR Apache-2.0 |
| `string_cache` | 0.9.0 | MIT OR Apache-2.0 |
| `string_cache_codegen` | 0.6.1 | MIT OR Apache-2.0 |
| `synstructure` | 0.13.2 | MIT |
| `tao` | 0.35.3 | Apache-2.0 |
| `tauri` | 2.11.5 | Apache-2.0 OR MIT |
| `tauri-build` | 2.6.3 | Apache-2.0 OR MIT |
| `tauri-codegen` | 2.6.3 | Apache-2.0 OR MIT |
| `tauri-macros` | 2.6.3 | Apache-2.0 OR MIT |
| `tauri-plugin` | 2.6.3 | Apache-2.0 OR MIT |
| `tauri-plugin-dialog` | 2.7.3 | Apache-2.0 OR MIT |
| `tauri-plugin-fs` | 2.5.2 | Apache-2.0 OR MIT |
| `tauri-plugin-opener` | 2.5.5 | Apache-2.0 OR MIT |
| `tauri-runtime` | 2.11.3 | Apache-2.0 OR MIT |
| `tauri-runtime-wry` | 2.11.4 | Apache-2.0 OR MIT |
| `tauri-utils` | 2.9.3 | Apache-2.0 OR MIT |
| `tauri-winres` | 0.3.6 | MIT |
| `tendril` | 0.5.1 | MIT OR Apache-2.0 |
| `thiserror` | 1.0.69 | MIT OR Apache-2.0 |
| `thiserror` | 2.0.20 | MIT OR Apache-2.0 |
| `thiserror-impl` | 1.0.69 | MIT OR Apache-2.0 |
| `thiserror-impl` | 2.0.20 | MIT OR Apache-2.0 |
| `time` | 0.3.55 | MIT OR Apache-2.0 |
| `time-core` | 0.1.9 | MIT OR Apache-2.0 |
| `time-macros` | 0.2.32 | MIT OR Apache-2.0 |
| `tinystr` | 0.8.4 | Unicode-3.0 |
| `tinyvec` | 1.13.2 | Zlib OR Apache-2.0 OR MIT |
| `tinyvec_macros` | 0.1.1 | MIT OR Apache-2.0 OR Zlib |
| `tokio` | 1.53.1 | MIT |
| `toml` | 0.9.12+spec-1.1.0 | MIT OR Apache-2.0 |
| `toml` | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 |
| `toml_datetime` | 0.7.5+spec-1.1.0 | MIT OR Apache-2.0 |
| `toml_datetime` | 1.1.1+spec-1.1.0 | MIT OR Apache-2.0 |
| `toml_parser` | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 |
| `toml_writer` | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 |
| `tracing` | 0.1.44 | MIT |
| `tracing-attributes` | 0.1.31 | MIT |
| `tracing-core` | 0.1.36 | MIT |
| `tray-icon` | 0.24.2 | MIT OR Apache-2.0 |
| `typeid` | 1.0.3 | MIT OR Apache-2.0 |
| `typenum` | 1.20.1 | MIT OR Apache-2.0 |
| `unic-char-property` | 0.9.0 | MIT/Apache-2.0 |
| `unic-char-range` | 0.9.0 | MIT/Apache-2.0 |
| `unic-common` | 0.9.0 | MIT/Apache-2.0 |
| `unic-ucd-ident` | 0.9.0 | MIT/Apache-2.0 |
| `unic-ucd-version` | 0.9.0 | MIT/Apache-2.0 |
| `unicode-segmentation` | 1.13.3 | MIT OR Apache-2.0 |
| `url` | 2.5.8 | MIT OR Apache-2.0 |
| `urlpattern` | 0.3.0 | MIT |
| `utf8_iter` | 1.0.4 | Apache-2.0 OR MIT |
| `uuid` | 1.26.0 | Apache-2.0 OR MIT |
| `version_check` | 0.9.5 | MIT/Apache-2.0 |
| `vswhom` | 0.1.0 | MIT |
| `vswhom-sys` | 0.1.3 | MIT |
| `walkdir` | 2.5.0 | Unlicense/MIT |
| `web_atoms` | 0.2.6 | MIT OR Apache-2.0 |
| `webview2-com` | 0.38.2 | MIT |
| `webview2-com-macros` | 0.8.1 | MIT |
| `webview2-com-sys` | 0.38.2 | MIT |
| `winapi-util` | 0.1.11 | Unlicense OR MIT |
| `window-vibrancy` | 0.6.0 | Apache-2.0 OR MIT |
| `windows` | 0.61.3 | MIT OR Apache-2.0 |
| `windows-collections` | 0.2.0 | MIT OR Apache-2.0 |
| `windows-core` | 0.61.2 | MIT OR Apache-2.0 |
| `windows-future` | 0.2.1 | MIT OR Apache-2.0 |
| `windows-link` | 0.1.3 | MIT OR Apache-2.0 |
| `windows-numerics` | 0.2.0 | MIT OR Apache-2.0 |
| `windows-result` | 0.3.4 | MIT OR Apache-2.0 |
| `windows-strings` | 0.4.2 | MIT OR Apache-2.0 |
| `windows-sys` | 0.59.0 | MIT OR Apache-2.0 |
| `windows-sys` | 0.60.2 | MIT OR Apache-2.0 |
| `windows-targets` | 0.52.6 | MIT OR Apache-2.0 |
| `windows-targets` | 0.53.5 | MIT OR Apache-2.0 |
| `windows-threading` | 0.1.0 | MIT OR Apache-2.0 |
| `windows-version` | 0.1.7 | MIT OR Apache-2.0 |
| `windows_x86_64_gnu` | 0.52.6 | MIT OR Apache-2.0 |
| `windows_x86_64_gnu` | 0.53.1 | MIT OR Apache-2.0 |
| `windows_x86_64_msvc` | 0.52.6 | MIT OR Apache-2.0 |
| `windows_x86_64_msvc` | 0.53.1 | MIT OR Apache-2.0 |
| `winnow` | 0.7.15 | MIT |
| `winnow` | 1.0.4 | MIT |
| `winreg` | 0.55.0 | MIT |
| `writeable` | 0.6.4 | Unicode-3.0 |
| `wry` | 0.55.1 | Apache-2.0 OR MIT |
| `yoke` | 0.8.3 | Unicode-3.0 |
| `yoke-derive` | 0.8.2 | Unicode-3.0 |
| `zerofrom` | 0.1.8 | Unicode-3.0 |
| `zerofrom-derive` | 0.1.7 | Unicode-3.0 |
| `zerotrie` | 0.2.5 | Unicode-3.0 |
| `zerovec` | 0.11.8 | Unicode-3.0 |
| `zerovec-derive` | 0.11.6 | Unicode-3.0 |
| `zlib-rs` | 0.6.7 | Zlib |

## The interface, shipped (7 packages)

Bundled into the interface that the desktop shell embeds.

| Package | Version | Licence |
|---|---|---|
| `@tauri-apps/api` | 2.11.1 | Apache-2.0 OR MIT |
| `@tauri-apps/plugin-dialog` | 2.7.3 | MIT OR Apache-2.0 |
| `@tauri-apps/plugin-opener` | 2.5.5 | MIT OR Apache-2.0 |
| `dockview` | 4.13.1 | MIT |
| `react` | 19.2.8 | MIT |
| `react-dom` | 19.2.8 | MIT |
| `zustand` | 5.0.15 | MIT |

## The interface, build only (5 packages)

Used to build the interface. Not present in the shipped product.

| Package | Version | Licence |
|---|---|---|
| `@types/react` | 19.2.18 | MIT |
| `@types/react-dom` | 19.2.7 | MIT |
| `@vitejs/plugin-react` | 5.2.0 | MIT |
| `typescript` | 5.9.3 | Apache-2.0 |
| `vite` | 7.3.6 | MIT |

