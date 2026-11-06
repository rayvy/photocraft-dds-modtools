# PhotoCraft DDS ModTools (unofficial)

Based on PhotoCraft and PR #9926. This local fork adds a file codec; it is **not** a filter plugin that can be installed in unchanged PhotoCraft. The upstream v1 WASM plugin API cannot register import/export formats.

## Supported textures

Single 2D DDS textures: legacy uncompressed RGB/luminance, BC1/2/3, BC4/5 UNORM and SNORM, BC6H unsigned/signed HDR, BC7, explicit linear/sRGB variants, DX10 RGBA8 and RGBA32F. BC4 is a gray/red data channel; BC5 is independent RGB(R,G,0), with opaque alpha. No automatic normal Z reconstruction, green inversion or swizzling. Signed and HDR samples use float documents.

Open options retain alpha as an editable Alpha 1 channel, import mip levels as hidden layers, and flip rows. For packed game data, use Alpha 1 to keep RGB visible beneath zero alpha. Save restores one document alpha channel to DDS alpha. CUSTOM fourth-channel data is accepted without premultiplication; select Alpha 1 when importing packed masks. BC1 reduces alpha to a binary mask; BC4/5 and BC6H cannot store alpha. Compression is lossy; keep a layered PSD/master and avoid repeated recompression.

Export generates a new mip chain (box for power-of-two dimensions, bilinear otherwise) when requested. Imported custom mip layers are editable references, not a promise to preserve their original bytes on export. Cubemaps, arrays, volumes, premultiplied/reserved alpha modes and unlisted DXGI formats are rejected explicitly. This is not support for every possible DDS container.

## Encoder backend

Reading is pure Rust (`bcdec_rs = 0.2.1`) on native and web. The original legacy writer remains built in. Advanced writing requires Microsoft's MIT-licensed DirectXTex **texconv.exe**, Windows x64, at `codecs/dds/texconv.exe` next to the executable. Advanced writing on other platforms returns an unsupported error. A developer can override its absolute path with `PHOTOCRAFT_DDS_BACKEND`.

The backend receives owned temporary DX10 DDS files (RGBA8 with matching transfer, or lossless RGBA32F for signed/HDR), explicit argument lists without a shell, separate alpha filtering, and a five-minute time limit. Signed BC6H uses the CPU codec because the tested DirectXTex GPU path clips negative input values. BC7 also uses the CPU when the top-level width or height is not a multiple of four: the tested GPU encoder introduced severe alpha error in partial edge blocks. Output dimensions, format, mip count and byte count are validated before the editor returns output. Failed exports do not replace the destination. The intermediate files are cleaned up automatically.

Explicit linear DDS inputs use a linear document profile; sRGB inputs use the normal sRGB interpretation. Legacy DXT choices retain their established sRGB interpretation; the separate `bc1-linear`/`bc2-linear`/`bc3-linear` choices write explicit DX10 linear metadata. Color output converts once to the chosen transfer; BC4/5 numeric output bypasses gamma conversion. Save's dialog defaults to the source format and mip setting. Float input exported to integer formats can clip values; choose BC6H or RGBA32F for HDR.

## Build and test

Use the Rust toolchain and Windows SDK described in `docs/development.md`. Build with `cargo build --release --locked -p photocraft -p photocraft-cli --features heif`. The dependency lock includes the pinned decoder. Ship the backend and its license separately; it is not downloaded or executed by the build.

Run `cargo test -p photocraft-codecs --test dds_modtools --lib` and `cargo test -p photocraft-io --test dds_modtools`; set the backend path to exercise advanced output. Standard codec/IO/CLI tests, clippy, layer and wasm checks remain applicable. Verification uses synthetic textures; private game/mod textures must not be committed.

Sources: [PhotoCraft](https://github.com/storytold/photocraft), [PR #9926](https://github.com/storytold/photocraft/pull/9926), [bcdec_rs](https://github.com/ScanMountGoat/image_dds), [DirectXTex](https://github.com/microsoft/DirectXTex), [texconv documentation](https://github.com/microsoft/DirectXTex/wiki/Texconv).
