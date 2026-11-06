# PhotoCraft DDS ModTools Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Open, edit and save the user's BC7 and HDR DDS textures directly in PhotoCraft.
**Architecture:** A focused fork retains PR9926's import UI. A checked Rust BCn decoder and a separate native DirectXTex encoding backend extend the existing DDS codec; vanilla's filter plugin ABI remains unchanged.
**Tech Stack:** Rust 1.99.1, bcdec_rs pinned, DirectXTex texconv, existing egui UI/CLI.
**Spec:** docs/superpowers/specs/2026-10-29-dds-modtools-design.md

## Global Constraints
- No unsafe host code, no shell invocation, no private textures committed.
- Keep the official and PR9926 installed binaries and profiles intact.
- BC6H and signed sample values remain floating point.
- Reject unsupported container shapes rather than silently dropping data.
- Web compilation remains supported; missing native encoding returns an error.

## Review Focus
- Zero-alpha RGB data must remain editable through Alpha 1.
- HDR values above one and signed channels must not be clipped to U8.
- DXGI sRGB flags and encoded data must agree without double gamma transforms.
- Hostile DDS dimensions and truncated blocks must fail before allocation/decode.
- Backend failure, timeout or absence must not overwrite the requested destination.

### Task 1: Extended DDS decoding
**Files:** crates/codecs/src/codecs/dds.rs; new crates/codecs/src/codecs/dds_advanced.rs; crates/codecs/Cargo.toml; crates/codecs/tests/it/dds.rs.
**Interfaces:** existing dds_info/decode/decode_dds_levels; produce checked BC4-BC7 decoding with U8/F32 images.
- [x] Add synthetic BC7, BC4/5, HDR and malformed-input tests; watch the BC7 test fail for unsupported format.
- [x] Pin bcdec_rs and implement checked block dispatch, signed/HDR sample handling and mip layout.
- [x] Run the new tests and existing DDS decoder suite; commit the passing decoding increment.

### Task 2: Native encoding and format choices
**Files:** crates/codecs/src/options.rs; crates/codecs/src/fidelity.rs; new crates/codecs/src/codecs/dds_native.rs; crates/codecs/src/codecs/dds.rs; crates/codecs/tests/it/dds_write.rs; crates/io/src/flat.rs and DDS tests; crates/doc/src/lib.rs FileEncoding; crates/ui-egui/src/dialogs/dds_options_ui.rs; apps/photocraft-cli/src/args.rs and tests where required.
**Interfaces:** DdsCompression name/from_name/ALL; native encoder returns Result<Vec<u8>, CodecError> and consumes the selected Plan without losing floating point data.
- [x] Add tests for enum parsing, BC7 output header, HDR fidelity, absent backend and failure-safe export; watch failures.
- [x] Add extended DXGI compression choices and a bounded temporary-file backend invocation with explicit arguments.
- [x] Wire GUI labels, command parsing and source-format defaulting; verify write/reopen against DirectXTex.
- [x] Run DDS regression tests and commit the passing encoding increment.

### Task 3: Verification and delivery
**Files:** docs/file-format-parity.md; crates/codecs/README.md; docs/plugins.md; install README; build/test records outside tracked sources.
**Interfaces:** produced native binaries and backend use a stable adjacent `codecs/dds` layout.
- [x] Run relevant codec/io/CLI tests, clippy and layer/wasm checks; inspect exact counts and failures.
- [x] Verify all distinct real local DDS formats without committing the files; check export/reopen and alpha/HDR invariants.
- [x] Render UI offscreen and review it; perform an independent whole-change review and resolve actionable findings.
- [x] Build release, install side by side, verify installed version/shortcut/profile/backend and report exact capabilities and remaining limits.
- [x] Publish to a user-owned GitHub fork only through an authenticated available interface; otherwise leave a complete local fork and identify the missing authentication/capability explicitly.

Verification completion: code commit7094b5d; codecs/IO1943 PASS, UI library4208 PASS (4 threads), UI integration134 PASS, engine2357 PASS plus integrations, adversarial registry PASS, local1337DDS PASS, corpus15 PASS/9579 mutations no panics with32MiB test worker stack; all-target clippy/layers/wasm/fmt PASS. Updated GUI/CLI installed independently, both shortcuts/profile/backend checked. Installed CLI retained exact decoded pixels for real legacy/BC7/sRGB examples and matched BC6H float oracle; signed HDR and partial-edge alpha regressions pass. Raw test logs and full Russian delivery report are preserved in the task outputs; the unrelated parallel font-menu harness flake remains documented. Hosted fork unavailable; complete local fork retained.
