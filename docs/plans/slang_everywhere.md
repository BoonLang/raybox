# Slang Everywhere

## Summary

Status: complete.

Raybox now uses one graphics authoring pipeline everywhere:

1. tracked shader source lives only in `shaders/*.slang`
2. `build.rs` compiles Slang to WGSL
3. `wgsl_bindgen` generates Rust bindings in `$OUT_DIR/shader_bindings.rs`
4. runtime and examples consume generated shader modules, generated layout helpers, and generated ABI types

This plan removed the remaining repo-tracked handwritten WGSL utility paths, moved utility shaders into the tracked Slang pipeline, replaced the main hand-mirrored world-demo ABI structs with generated types, tightened bind group sizing, migrated examples to the same rule, and added repo guardrails so the architecture cannot quietly regress.

## Requirements

The end-state rule is strict:

- repo-tracked shader source is `.slang` only
- repo-tracked runtime/example WGSL source is not allowed
- host-side GPU layout structs should come from generated bindings, not hand-written `#[repr(C)]` mirrors, when a generated binding type exists
- bind group layout entries should carry explicit `min_binding_size` based on generated types, not `None`
- native, web, and `examples/` must all follow the same shader/layout architecture

Allowed exceptions:

- generated code in `$OUT_DIR`
- hot-reload/runtime shader compiler plumbing that consumes generated WGSL output

## Current State

The required architecture is in place across runtime and examples:

- tracked utility shaders now live in:
  - `shaders/empty.slang`
  - `shaders/overlay.slang`
  - `shaders/present.slang`
- `build.rs` includes those utility shaders in the tracked Slang build
- runtime and web now use generated utility shader bindings for empty, overlay, and present passes
- the native/world/web demo families use generated uniform types for the main world demos and text-heavy demos where bindings exist
- the retained `Ui2D` and `UiPhysical` runtime/web passes now use generated `sdf_todomvc` / `sdf_todomvc_3d` uniform and theme ABI types instead of local mirrors
- runtime and examples no longer leave `min_binding_size` implicit in repo-tracked code
- `AGENTS.md` and `CLAUDE.md` now codify the same shader-pipeline rule
- `scripts/check_shader_architecture.sh` plus `src/architecture_guard.rs` enforce the rule in CI/test runs

## In-Scope Work

### 1. Utility Shaders On The Generated Path

Move shared utility shaders into `shaders/` and through `build.rs`:

- `empty.slang`
- `overlay.slang`
- `present.slang`

Then replace handwritten WGSL in runtime code with generated shader modules and generated layout helpers.

### 2. Generated Types As The GPU ABI Source Of Truth

Replace hand-written Rust GPU interface structs with generated binding types such as:

- `*_std140_0`
- `*_std430_0`

Rust-side helper code may still exist, but only to populate generated types from app state. The helper struct itself should not be the ABI contract.

### 3. Explicit Bind Layout Sizes

Where bind groups are still built manually, `min_binding_size` must come from generated type sizes rather than being left implicit.

This is required to prevent runtime-only layout bugs like "host buffer is smaller than shader-required uniform size".

### 4. Native, Web, And Examples Must Match

The same architecture rule applies everywhere:

- `src/`
- `src/web.rs`
- `examples/`

Examples are not allowed to keep a parallel hand-written graphics stack just because they are examples.

### 5. Guardrails

Add repo checks and documentation so the architecture stays pure:

- fail on new repo-tracked runtime/example WGSL outside the allowlist
- fail on new hand-written GPU ABI structs where generated bindings already exist
- document the rule in `AGENTS.md`
- sync any overlapping guidance in `CLAUDE.md`

## Completed Work

### Phase 1. Utility Shader Migration

Done:

1. added `empty.slang`, `overlay.slang`, and `present.slang`
2. extended `build.rs` to compile them and generate Rust bindings
3. migrated the runtime `empty`, `overlay`, and `present` paths to generated bindings
4. removed the old repo-tracked WGSL utility shader strings

### Phase 2. Runtime ABI Cleanup

Done:

1. replaced the hand-mirrored world-demo uniforms with generated types for the main world demos
2. migrated the clay/text-shadow demo uniforms and key storage-side instance types to generated bindings
3. migrated the retained `Ui2D` and native `UiPhysical` runtime uniforms/theme ABI to generated bindings
4. tightened runtime bind-group sizing so uniform/storage bindings no longer rely on implicit minimum sizes

### Phase 3. Web ABI Cleanup

Done:

1. removed the web-side handwritten empty WGSL path
2. migrated the web simple world demos to generated uniform types
3. migrated the web clay/text-shadow uniform and instance ABI to generated bindings
4. migrated the web retained `Ui2D` / `UiPhysical` uniform ABI to the generated bindings
5. kept browser-specific logic focused on platform behavior rather than shader/layout authoring

### Phase 4. Examples Cleanup

Done:

1. examples use the generated shader path
2. examples no longer leave `min_binding_size` implicit
3. examples now use generated GPU ABI types where bindings exist for the SDF/text examples

### Phase 5. Guardrails And Docs

Done:

1. updated `AGENTS.md` with the strict Slang/generated-bindings architecture rule and canonical commands
2. synced `CLAUDE.md` to the same rule
3. added:
   - `scripts/check_shader_architecture.sh`
   - `src/architecture_guard.rs`

## Acceptance Criteria

This plan is complete when all of the following are true:

- all repo-tracked shader source lives in `shaders/*.slang`
- no runtime/example handwritten WGSL remains outside the explicit allowlist
- generated binding types are the authoritative GPU ABI source wherever bindings exist
- manual bind group layouts use explicit `min_binding_size`
- native demos, web demos, and examples all still build and run
- the old class of host/shader uniform size mismatch is prevented by construction

## Canonical Commands

Native:

- `just demos`
- `just demos-from 8`
- `just demos-control`
- `just ctl status`
- `just ctl switch 8`

Web:

- `just build-web`
- `just web`
- `just open-browser`
- `just open-browser-hotreload`
- `just web-smoke`

Direct Cargo / CLI:

- `cargo build --bin demos --features windowed,control,mcp`
- `cargo test`
- `cargo run --bin raybox-ctl --features control -- status`
- `cargo run --bin raybox-ctl --features control -- web-open --control --hotreload --demo 8`

Browser policy:

- Chromium is the default supported browser target for Raybox web runs
- browser launch should go through the repo launcher path, not ad hoc manual flags

## Verification

Automated:

- `./scripts/check_shader_architecture.sh`
- `cargo test`
- `cargo build --bin demos --features windowed,control,mcp`
- `cargo build --examples --features windowed`
- `cargo check --lib --target wasm32-unknown-unknown --features web`

Manual:

- run native demos with `just demos`
- run web demos with `just web`
- confirm utility paths like present/overlay/empty still render correctly after migration
- confirm hot-reload still works for `.slang` edits

## Notes

This plan intentionally kept the existing Slang -> WGSL -> generated Rust pipeline.

The goal was not to replace the toolchain. The goal was to enforce it everywhere in the repo and add checks so the repo cannot drift back into handwritten WGSL or implicit GPU ABI layouts.
