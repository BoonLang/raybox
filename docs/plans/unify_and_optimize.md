# Unified Mixed-Scene Frame Renderer

## Summary

Raybox will move to one engine-level frame renderer that can compose retained UI,
physical UI, world-space raymarched scenes, and post/composite effects in the
same frame. This is a unification of frame orchestration, resource scheduling,
and presentation. It is not a forced unification of scene semantics or shader
technique.

The architecture target is:

- one runner-owned frame graph
- one packet extraction seam from demos/content into the renderer
- shared composition rules for world, UI, effects, overlay, and present
- separate internal prepared packet contracts for UI and world content
- no public backend choice and no atlas-based primary text path

This document is the implementation contract. Code changes must follow it.

## Goals

- Support mixed scenes as first-class cases:
  - a 3D world with UI overlays and menus
  - a UI scene with animated 3D background
  - UI panels containing embedded 3D previews
- Keep current `Ui2D`, `UiPhysical`, and `World3D` demos visually stable during
  migration.
- Make the runner the architectural seam instead of direct demo-owned final
  pass encoding.
- Preserve semantic retained UI authoring.
- Preserve runtime font swapping and exact-vector fallback paths.
- Enable later performance work on proxy 3D text, proxy UI shells, and hybrid
  effect passes without leaking those choices through the API.

## Non-Goals

- Do not force `World3D` onto the retained UI scene model.
- Do not force `Ui2D`, `UiPhysical`, and `World3D` to share one universal scene
  blob.
- Do not adopt atlas-based text as the primary path.
- Do not replace all rendering with one universal fullscreen shader.
- Do not block current demos on a public API redesign before the internal graph
  exists.

## Public API Direction

The migration keeps current public demo and retained APIs working through
compatibility adapters first. After the internal graph is stable, expose one new
public top-level composition API:

- `ComposedScene`
- `WorldView`
- `UiLayer`
- `EffectLayer`

Current in-repo status:

- `ComposedScene` exists publicly in `src/demos/frame_graph.rs`
- `WorldView`, `UiLayer`, and `EffectLayer` now also exist as public wrapper
  types over the packet layer
- it already provides builder helpers for common mixed-scene cases:
  fullscreen world, fullscreen UI overlay, and embedded world preview
- current demos still own the actual world/UI renderers behind those packets
- the wrapper types are still thin; they do not yet replace the existing demo
  and retained authoring APIs across the engine

`UiLayer` supports `Flat` and `Physical` styles as content styles, not as
separate renderer families. `WorldView` supports full-frame rendering and
offscreen rendering for later embedding into UI regions.

No public API may expose:

- fake-vs-real glass/refraction implementation details
- proxy-vs-exact text implementation choice
- atlas-specific controls
- backend family selection

## Internal Contracts

### 1. `FramePacketSource`

Add an internal extraction seam that produces renderable frame packets without
encoding final passes directly.

Required shape:

- `prepare_frame(&mut self, queue: &wgpu::Queue)`
- `build_frame_packet(&self, time: f32) -> FramePacket`
- existing update/resize hooks remain in place during migration

During the first migration step, current demos can continue using the existing
`Demo` trait with a default `build_frame_packet()` implementation.

### 2. `FramePacket`

`FramePacket` is the ordered, extracted description of what the renderer should
compose in the current frame.

The packet contains ordered `FramePacketItem`s from this fixed set:

- `WorldView(WorldViewPacket)`
- `UiLayer(UiLayerPacket)`
- `Effect(EffectPacket)`
- `Overlay(OverlayPacket)`
- `Present(PresentPacket)`

Packet items are explicit and ordered. The first implementation does not need
full graph culling, but it must already compile this packet into executable pass
descriptors instead of letting the runner hardcode scene/overlay/present.

### 3. Packet Schemas

`WorldViewPacket` fields:

- `label`
- `target`
- `composite_mode`
- `clear_color`

`UiLayerPacket` fields:

- `label`
- `style: Flat | Physical`
- `target`
- `composite_mode`
- `clear_color`

`EffectPacket` fields:

- `label`
- `source`
- `target`
- `composite_mode`
- `clear_color`
- `source_rect`
- `target_rect`

`OverlayPacket` fields:

- `label`
- `target`

`PresentPacket` fields:

- `target`

Shared enums:

- `FrameTarget`
  - `SceneColor`
  - `Offscreen(name)`
- `CompositeMode`
  - `Replace`
  - `Over`
- `UiLayerStyle`
  - `Flat`
  - `Physical`
- `NormalizedRect`
  - normalized origin and size used for sub-rect composition

## Render Graph Behavior

The runner must compile a `FramePacket` into a graph-owned pass list. The first
implementation can be linear, but it must already separate:

- packet extraction
- graph compilation
- pass execution

Required execution behavior:

- `Replace` means the pass clears its target before drawing.
- `Over` means the pass loads the target before drawing.
- `Overlay` always composites over the current scene target.
- `Present` is always the last executed packet item.
- `Offscreen(name)` is valid in the packet schema immediately, but phase-1
  execution may reject or skip it until offscreen composition is implemented.
- `EffectPacket` rects allow one offscreen result to be composited full-frame or
  into a normalized destination sub-rect. This is the mechanism for embedded
  world previews without a separate runner rewrite.

## Migration Order

### Phase 0: Document First

- Add this document before code changes.
- Keep it aligned with implementation.

### Phase 1: Runner-Owned Graph

- Replace the runner’s hardcoded `scene -> overlay -> present` sequence with:
  - `build_frame_packet()`
  - `compile_frame_graph()`
  - `execute_frame_graph()`
- Preserve identical visuals.

### Phase 2: Demo Family Adapters

- `Ui2D`, `UiPhysical`, and `World3D` all produce packets through the same
  runner seam.
- Current demos still render through existing hosts internally.
- The graph is the unification layer, not the host internals.

### Phase 3: Shared Retained UI Extraction

- Unify `Ui2D` and `UiPhysical` around one retained extraction/resource update
  layer.
- Keep two realizers:
  - `UiFlatPass`
  - `UiPhysicalPass`
- Move `UiPhysicalRuntimeScene` toward a prepared-packet provider instead of a
  final renderer.

Current implementation state:

- shared retained extraction/update seam: implemented
- shared retained GPU/runtime resource state and buffer update helpers:
  implemented
- `Ui2D` and `UiPhysical` now both route retained text/UI storage creation and
  runtime patch application through the same shared runtime module
- actual current buffer capacities, especially grid-cell capacity, now drive fit
  checks before sync-vs-rebuild decisions
- full renderer-host unification is still pending

### Phase 4: Mixed-Scene Composition

Add built-in support for these composition patterns:

- `WorldThenUi`
- `WorldWithFullscreenUiOverlay`
- `UiWithEmbeddedWorldView`
- `UiThenOverlayEffects`

The first two must be supported directly once packet/graph routing is stable.
`UiWithEmbeddedWorldView` requires offscreen target execution.
The first concrete mixed-scene implementation should be a world background with
transparent `Ui2D` overlay so the frame graph can prove world-under-UI
composition before embedded previews are added.

Current implementation state:

- world background with transparent UI overlay: implemented
- transparent `UiPhysical` overlay: implemented
- embedded world preview via effect target rects: implemented
- mixed demo exercises both flat and physical UI overlay styles through the
  public composition wrappers: implemented
- themed TodoMVC `UiPhysical` host construction is now shared between the
  standalone 3D demo and mixed-scene composition helpers: implemented

### Phase 5: Performance Realization Changes

- Premium 3D text in physical UI moves off the exact curve hot path and onto
  cached runtime-generated proxy 3D geometry.
- Card shells, bevels, separators, and similar fake-3D UI structure move to
  analytic/proxy geometry.
- Glass, acrylic, contact shadow, and distortion become effect passes or hybrid
  screen-space passes.
- Exact vector text remains a reference/debug/fallback path.

Current implementation state:

- conservative coarse-to-detail marching for `UiPhysical`: implemented
- conservative CPU-provided text-bounds hints for `UiPhysical`: implemented
- conservative early-outs for card-top detail evaluation and geometry-only
  normal distance queries: implemented
- repeated physical UI details now narrow to the nearest item/separator bands
  instead of always evaluating every stacked-card row/line candidate:
  implemented
- exact text tracing can now seed the next text sample from the last valid glyph
  hit before falling back to the full exact scan: implemented
- repeated exact text samples can now early-return the cached glyph when the
  point is safely inside that same glyph volume: implemented
- exact text neighborhood scans now prioritize the center/current cell before
  adjacent and diagonal cells so stable hits converge earlier: implemented
- exact text-normal taps reuse the hit glyph first, and interior flat top/bottom
  text faces can now take a conservative analytic normal fast path: implemented
- broad flat physical UI surfaces now take conservative analytic top-face
  normals for stacked cards, item panels, and separator/border strips before
  falling back to geometry sampling: implemented
- those same broad flat physical UI surfaces now take the reduced shadow/AO
  lighting path before falling back to the full expensive lighting path:
  implemented
- broad flat glass UI surfaces now use a single backdrop gradient sample before
  falling back to the full multi-tap glass backdrop path: implemented
- broad flat interior glass UI surfaces now skip the card/item/input border
  glow calculations and keep that extra work only near visible edges:
  implemented
- non-interior glass item-panel border shading now narrows directly to the
  nearest repeated item band instead of rescanning all item rows: implemented
- non-interior glass border effects now also short-circuit once the sample is
  outside each glow/rim distance band instead of evaluating dead formulas:
  implemented
- repeated checkbox-ring evaluation in the physical UI scene now narrows to the
  nearest item row instead of evaluating both nearest and neighbor row
  candidates: implemented
- the flat-surface classifier for separator/input/footer strips now resolves the
  nearest repeated separator analytically instead of scanning all strip centers:
  implemented
- reduced shadow/AO shading path for glass 3D heading text: implemented
- proxy 3D text, proxy UI shells, and hybrid glass/effect passes: still pending

### Phase 6: Public Composer

- Expand the already-public `ComposedScene` from packet helpers into the main
  user-facing mixed-scene composition API.
- `Ui2D` and `UiPhysical` become scene styles/presets under the same renderer
  family instead of top-level renderer choices.

## Caching and Invalidation

- Retained UI partial updates remain patch-based.
- Runtime font changes invalidate only affected text-derived resources.
- Graph compilation is per-frame.
- Resource caches remain per-runtime host until the retained extraction layer is
  unified.
- Offscreen world view resources are cached by target id, size, and format once
  introduced.

## Acceptance Criteria

- The runner no longer hardcodes final scene/overlay/present sequencing.
- One extracted `FramePacket` drives frame execution.
- Existing `Ui2D`, `UiPhysical`, and `World3D` demos still render correctly.
- One demo can later emit both world and UI packet items in the same frame
  without custom runner logic.
- The architecture leaves room for world-under-UI and UI-with-embedded-world
  composition without another runner rewrite.
- No atlas-based primary text path is introduced.

## Verification Plan

- `cargo test`
- manual verification of current retained UI demos
- manual verification of current `World3D` demos
- future mixed-scene demos:
  - full-screen world with UI overlay
  - retained UI with world background
  - retained UI with embedded world preview
