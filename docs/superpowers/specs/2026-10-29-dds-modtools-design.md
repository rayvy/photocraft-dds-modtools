# PhotoCraft DDS ModTools

## Intent and authorization
The user requests DDS editing for game modding, preferring a plugin on the vanilla editor, and explicitly authorizes a fork when that is not possible. Existing session instructions favor autonomous implementation without unnecessary questions. Implement locally and keep installed upstream and PR9926 builds intact. Do not publish private textures or credentials.

## Findings
Vanilla plugin ABI v1 exposes only `Kind::Filter` and operates on already decoded pixels; it cannot register file codecs. This is a host-interface limitation, not a defect in WebAssembly. A genuine format plugin for unchanged vanilla is unavailable. The chosen fallback is a focused fork with a separated DDS backend. Do not call this a vanilla-compatible plugin.

Base: PR9926 commit 6ac8a68dcb1033c4ffc3c5dff6eb9eb05511f420. Local branch: dds-modtools. Existing import controls (alpha as channel, mips as layers, vertical flip) stay.

A metadata-only scan of 1337 local mod DDS files found DXGI 98 (760), 99 (116), 95 (2), and 459 legacy textures, with no array/cube/volume containers. The concrete failed file is a 16x16 BC7_UNORM texture. Local files are verification inputs only and must not be committed.

## Approaches
1. Vanilla WASM plugin: cannot supply a codec with the current ABI; ruled out by source inspection.
2. Clipboard exporter or executable byte patch: does not provide correct import, HDR, metadata or channel fidelity; ruled out.
3. Focused fork: selected under the user's conditional fork authorization. Keep decoding in a dedicated Rust module and native encoding in an independent DirectXTex backend shipped beside the app. This retains a reusable backend without inventing a large general plugin ABI.

## Scope
- Direct open/edit/save for 2D BC1-BC7 DDS, including BC4/BC5 signed variants and BC6H HDR; preserve existing legacy/uncompressed support.
- BC6H and signed sample values must remain floating point; never silently quantize HDR to U8.
- Distinguish linear and sRGB DDS output formats. Preserve the source BC encoding as the default Save choice when supported.
- Preserve editable alpha through the existing PR9926 channel route. Do not reconstruct normal-map channels or change handedness implicitly.
- Decode all existing mip levels when requested; retain the existing explicit mip-generation export option. Do not claim an arbitrary layer stack is a preserved DDS mip chain.
- Expose added formats consistently in Save As, CLI and automation. Clear errors for a missing native encoder.
- Reject cube/array/volume input explicitly; do not flatten to a face or slice. These containers require a separate document UI and are not present in the user's current corpus.

## Implementation
Rust-only host changes with no unsafe code. Use the pinned pure Rust `bcdec_rs` block decoder behind the existing checked DDS parser and allocation limits. Advanced native encoding uses Microsoft DirectXTex texconv in `codecs/dds/texconv.exe` next to the application. It is an optional separate encoding backend, not the vanilla WASM plugin API. Invoke it through `std::process::Command`, never a shell. Use owned temporary files, a bounded execution time, bounded diagnostic reads, explicit argument lists and cleanup by RAII. The browser must continue compiling; unsupported native encoding there returns a clear error.

Keep legacy BC1-BC3 encoding paths for compatibility. Extend the existing compression enum with named DXGI choices instead of adding fields to every DdsOptions literal. HDR export fidelity planning must select F32; sRGB choices must set DDS DX10 format metadata and use the matching mip filtering behavior. No source file overwrite during tests.

Builder check decisions: BC4 decodes to Gray (the red data channel); BC5 to RGB(R,G,0), never GrayA. SNORM and BC6H decode to F32. Explicit linear DXGI inputs are tagged linear sRGB/gray in IO, sRGB inputs sRGB, legacy inputs keep their established untagged interpretation. BC4/5 export bypasses color transforms to preserve numeric data; other explicit linear/sRGB DDS targets convert once in IO. The intermediate sent to DirectXTex is uncompressed DX10 RGBA32F (format 2), or RGBA8 with the target transfer flag; only the backend generates mipmaps. DX10 premultiplied/reserved alpha modes are rejected. Empty arrays are malformed. The encoder validates output dimensions, DXGI format, mip count and payload size before returning. Its helper is Windows-only; other native platforms and wasm return an explicit unsupported error. Child output goes to owned files, diagnostics are bounded when read, and timed-out children are killed and reaped. Source DDS encoding and mip choice are retained in FileEncoding and used by Save defaults.

## Verification
Write failing codec tests before implementation. Cover BC7 header 98/99 and exact decoding vectors, BC4/BC5 channels and signed ranges, BC6H values above 1, truncated blocks, odd dimensions, mip chains and DX10 header invariants. Run regression tests for existing DDS. Cross-check generated synthetic fixtures and local real textures against DirectXTex/Pillow as applicable. Exercise CLI open, PSD channel preservation, save, re-open and reject malformed files. Render the DDS save/import controls offscreen if supported. All scope and platform limitations must be stated in the installed README.

## Delivery
Install a separate PhotoCraft DDS ModTools portable build with its own settings and shortcut; retain both previous installations. Ship codec backend license and source references. Keep the working source and reproducible build/test records. A local fork is authorized; creation of a hosted GitHub fork depends on an available authenticated fork-capable interface. Do not claim it exists until verified.
