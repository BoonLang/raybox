# Remove Dead Glyph-Grid ABI

## Summary

Status: complete.

The obsolete per-glyph `GridCell` / `curveIndices` path was removed end to end.

The current vector-text shaders now match the live brute-force curve evaluation plus
character-grid design, and the generated bindings, host/runtime code, and docs were
updated to match that reality. The stale manual Rust mirror structs disappeared by
deleting the dead ABI surface itself rather than preserving fallback rules.

## Implementation Changes

### 1. Simplify the active vector-text shader ABI

Update all vector-text Slang shaders that still declare the dead per-glyph grid path:

- `sdf_text2d_vector`
- `sdf_todomvc`
- `sdf_clay_vector`
- `sdf_text_shadow_vector`
- `sdf_todomvc_3d`

For each shader:

- remove `struct GridCell`
- remove `StructuredBuffer<GridCell> gridCells`
- remove `StructuredBuffer<uint> curveIndices`
- remove `GlyphData.gridInfo`
- remove unused `gridInfo` parameters from `sdGlyph*` helpers and all call sites
- keep `curves`, `glyphData`, `charInstances`, `charGridCells`, `charGridIndices`, and
  `charGridDistField` / `uiPrimitives` where they are actually live

Renumber bindings compactly after the removal:

- `sdf_text2d_vector` / `sdf_clay_vector`
  - `1=curves`, `2=glyphData`, `3=charInstances`, `4=charGridCells`, `5=charGridIndices`
- `sdf_todomvc`
  - `1=curves`, `2=glyphData`, `3=charInstances`, `4=charGridCells`,
    `5=charGridIndices`, `6=uiPrimitives`
- `sdf_text_shadow_vector`
  - `1=curves`, `2=glyphData`, `3=charInstances`, `4=charGridCells`,
    `5=charGridIndices`, `6=charGridDistField`
- `sdf_todomvc_3d`
  - keep `0=uniforms`, `1=theme`, then
    `2=curves`, `3=glyphData`, `4=charInstances`, `5=charGridCells`,
    `6=charGridIndices`, `7=uiPrimitives`

After the shader edit, regenerate bindings via the normal `build.rs` path and update all
generated-type constructors/usages to the new `GlyphData` layout.

### 2. Remove dead host/runtime plumbing

Delete the dead glyph-grid upload/binding path from shared runtime code, native demos,
web, and examples:

- remove `GpuGridCell` / `AtlasGridCell`
- remove `grid_cells` and `curve_indices` from the shared GPU font upload model
- remove `grid_cells_buffer` and `curve_indices_buffer` from shared storage buffer
  structs and bind-group entry builders
- update world/native/web storage-binding helpers to the new compact binding numbers
- keep `charGridCells`, `charGridIndices`, and `charGridDistField` intact

Apply that cleanup consistently in:

- shared runtime helpers
- native 2D/3D text passes
- web text passes
- clay/text-shadow-specific setup
- `examples/` text demos

Do not preserve empty fallback buffers for the removed bindings. The goal is that no
repo-tracked runtime/example code still knows those bindings ever existed.

### 3. Simplify the CPU atlas model to only live data

Refactor `src/text/glyph_atlas.rs` so it matches the live shader ABI:

- remove the `GridCell` type
- remove `VectorFontAtlas.grid_cells`, `VectorFontAtlas.curve_indices`, and
  `VectorFontAtlas.grid_resolution`
- remove glyph-grid construction logic and related helpers/tests/comments
- keep only the packed curve list plus per-glyph metadata needed by the live shaders
  and text layout

Change `GlyphAtlasEntry` to retain only live data:

- `bounds`
- `advance`
- `curve_offset`
- `curve_count`

Remove dead per-glyph grid metadata:

- `grid_size`
- `grid_offset`

Change the atlas constructor API from `VectorFontAtlas::from_font(&font, grid_resolution)`
to `VectorFontAtlas::from_font(&font)` and update all call sites in `src/` and
`examples/`. This is an intentional API cleanup; do not keep `grid_resolution` as an
ignored compatibility parameter.

### 4. Tighten docs and guardrails to the new reality

Update `docs/plans/slang_everywhere.md` so it states the final rule plainly:

- generated bindings are the source of truth for all live vector-text ABI types
- the obsolete per-glyph grid ABI was removed rather than grandfathered as a fallback

Extend the repo guardrails so the dead path cannot quietly return:

- fail on handwritten `GpuGridCell` / `AtlasGridCell` mirrors in runtime/example code
- fail on reintroduced dead `gridCells` / `curveIndices` runtime plumbing in
  repo-tracked code
- update the retained ABI parity assertions so they validate the simplified
  `GlyphData` layout between `sdf_todomvc` and `sdf_todomvc_3d`

## Public APIs / Types

- `VectorFontAtlas::from_font(&font, grid_resolution)` becomes `VectorFontAtlas::from_font(&font)`
- `GlyphAtlasEntry` drops `grid_size` and `grid_offset`
- generated `GlyphData_std430_0` changes from `bounds + gridInfo + curveInfo`
  to `bounds + curveInfo`
- vector-text storage binding numbers shift down by two everywhere the dead bindings existed

These are internal repo interfaces; update all in-repo callers in the same change.

## Verification

The following checks passed after the cleanup:

Run:

- `./scripts/check_shader_architecture.sh`
- `cargo test`
- `cargo build --bin demos --features windowed,control,mcp`
- `cargo build --examples --features windowed`
- `cargo check --lib --target wasm32-unknown-unknown --features web`

Manual verification:

- captured native screenshots for demos `5`, `6`, `7`, `8`, `10`, and `11` into
  `output/manual_check/native_*.png`
- captured controlled web screenshots for demos `5`, `6`, `7`, `8`, `10`, and `11`
  into `output/manual_check/web_ctrl_*.png`
- confirmed the affected text-heavy native and web paths still render through the
  cleaned-up storage bindings and simplified glyph ABI

## Assumptions

- the brute-force glyph SDF path is now the intended rendering design
- `charGridCells`, `charGridIndices`, and `charGridDistField` remain live and are not
  part of this removal
- binding numbers should be compacted after removing the dead slots; preserving holes
  is not worth the complexity
- no benchmark-first phase is added in this cleanup; if performance becomes a real
  issue later, acceleration should come back as a new deliberate feature, not as dead
  ABI baggage
