# PhotoCraft DDS ModTools

An **unofficial PhotoCraft fork for game texture modding**, based on PhotoCraft 0.17.0 and PR [#9926](https://github.com/storytold/photocraft/pull/9926), with extended DDS import/export. It is maintained independently of the PhotoCraft team.

**[Download the Windows x64 portable build](https://github.com/rayvy/photocraft-dds-modtools/releases)** · [Русская инструкция](packaging/dds-modtools/START-HERE.ru.md) · [DDS implementation details](DDS-MODTOOLS.md)

## Run

Download the portable ZIP from **Releases**, extract the whole folder to a writable location, and run `photocraft.exe`. Keep `codecs/dds/texconv.exe` and the DLLs beside the application in their supplied locations. No Rust toolchain, administrator access, or separate codec installation is needed to use this Windows package.

The included binaries are the tested 2026-10-29 DDS build, not a build of current upstream `main`. The GUI and CLI report `0.17.0 (dds-modto, 2026-10-29)`. This community build is unsigned. SHA-256 checksums are included. Settings are created in its own `PhotoCraftData` folder at first launch; the downloadable package starts with a fresh profile.

## DDS support for modders

| Texture storage | Read | Write |
|---|---|---|
| Legacy uncompressed RGB/RGBA/luminance and BC1/2/3 (DXT1/3/5) | Yes | Yes |
| BC4/BC5 UNORM and SNORM | Yes | Yes, bundled Windows backend |
| BC6H unsigned and signed HDR | Yes, float | Yes, bundled Windows backend |
| BC7, including sRGB | Yes | Yes, bundled Windows backend |
| DX10 RGBA8 linear/sRGB and RGBA32F | Yes | Yes, bundled Windows backend |

- On import, **Alpha 1** keeps a packed fourth data channel separate from layer transparency, including RGB beneath zero alpha.
- The save dialog starts with the source DDS storage and mip setting. Keep the game's original linear/sRGB interpretation.
- BC5 stores independent R/G data as RGB(R,G,0). There is no automatic normal Z reconstruction, green-channel inversion, or channel swizzle.
- Signed and HDR values remain floating point. Signed BC6H uses the CPU encoder to preserve negative samples; BC7 with partial edge blocks uses the CPU encoder to reduce alpha error.
- Mipmaps are generated as a new chain on export. Imported mip layers do not preserve an authored mip chain automatically.
- BC compression is lossy. Keep a layered PSD or other lossless master for repeated editing.

Support is for **single 2D textures**. Cubemaps, arrays, volumes, premultiplied/reserved alpha modes and unlisted DXGI formats are explicitly rejected. Extended encoding currently requires Windows x64; reading is implemented in safe Rust and also compiles for WebAssembly. See [DDS-MODTOOLS.md](DDS-MODTOOLS.md) for the full contract.

## Sources and build

The `dds-modtools` branch contains the implementation and tests. The portable program binaries were built from local commit `7094b5d6dbdf914cf09267dee7ce1f1bae36f762`. The published source has the same Rust application code; later documentation and packaging changes do not alter the binaries. The release source tag is the public reference for rebuilding this version.

Use a Windows MSVC Rust toolchain, Visual Studio C++ Build Tools and the Windows SDK, as described in [development.md](docs/development.md). From an x64 developer PowerShell:

```powershell
$env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS = '-C target-feature=+crt-static'
$env:PHOTOCRAFT_BUILD_SHA = 'dds-modtools'
$env:PHOTOCRAFT_BUILD_DATE = '2026-10-29'
cargo build --release --locked --target x86_64-pc-windows-msvc -p photocraft -p photocraft-cli --features 'photocraft/heif,photocraft-cli/heif'
```

For extended export, install the official [DirectXTex July 2025 texconv.exe](https://github.com/microsoft/DirectXTex/releases/tag/jul2025) at `codecs/dds/texconv.exe` beside the built executable, with its MIT license. The portable release includes it already. UI CJK fonts are an optional external build input; no font files are committed here.

To package an existing complete native build, use [package-portable.ps1](packaging/dds-modtools/package-portable.ps1). It copies an explicit file list and creates a ZIP with checksums, without copying an existing profile or personal documents.

## Verification

The tested implementation passed 1,943 codec/IO tests and 4,208 UI-library tests, plus engine, CLI, integration, malformed-input, layer, clippy and WebAssembly checks. A local read-only corpus contained 1,337 DDS files across legacy, BC6H, BC7 and sRGB storage; those private files are not distributed. Tests use synthetic textures. Portable-package checks exercise BC7 and signed HDR output with automatic adjacent backend discovery.

```powershell
$env:PHOTOCRAFT_DDS_BACKEND = 'C:\path\to\texconv.exe'
cargo test -p photocraft-codecs --lib --test dds_modtools
cargo test -p photocraft-io --test dds_modtools
```

## Credits and license

Based on [PhotoCraft by the ArtCraft team and contributors](https://github.com/storytold/photocraft), the DDS import work in [PR #9926](https://github.com/storytold/photocraft/pull/9926), [bcdec_rs](https://github.com/ScanMountGoat/image_dds) / [bcdec](https://github.com/iOrange/bcdec), and Microsoft's [DirectXTex](https://github.com/microsoft/DirectXTex).

PhotoCraft code is MIT OR Apache-2.0; retain [LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE), [NOTICE](NOTICE) and [ATTRIBUTION.md](ATTRIBUTION.md). The DDS decoder and external encoder retain their supplied licenses. The ArtCraft logo assets have been removed from this modified distribution under their [brand terms](docs/brand/LICENSE-brand.txt). The PhotoCraft application icon retains its existing open-source attribution. This fork is not an official ArtCraft release.
