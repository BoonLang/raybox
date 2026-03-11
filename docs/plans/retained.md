# Retained Rendering Plan

## Status

The native retained plan in this document is complete as of 2026-03-12.

RayBox now has:

- invalidation-driven retained redraw scheduling in the native runner
- shared retained scene state with stable `NodeId`, dirty tracking, clip/scroll
  visibility helpers, and scroll-local resource invalidation
- retained `Ui2D` and `UiPhysical` runtime families that consume the same
  semantic scene data and apply incremental text/UI updates
- dynamic retained slot assignment for the generic wrapped-text and sample
  scenes instead of hard-coded demo slot inventories
- wasm retained demo coverage for `RetainedUi`, `TodoMvc`, `TodoMvc3D`,
  `RetainedUiPhysical`, and `TextPhysical`

The remaining work in this area is no longer "finish retained rendering". It is
future renderer evolution on top of a working retained native base.

Testing note:

- `cargo test --lib` passes on 2026-03-12
- `cargo check --lib --target wasm32-unknown-unknown --features web` passes on
  2026-03-12

## Completed Native Milestones

### 1. Retained Redraw Contract

Completed.

What now exists:

- retained demos expose redraw demand directly to `src/demos/runner.rs`
- retained hosts report redraw demand from scene dirtiness, rebuild state, and
  theme dirtiness without falling back to continuous redraw
- screenshot preparation and normal frame preparation share the same retained
  frame-prep path

Result:

- retained demos stay idle when nothing changes
- local retained mutations can request a frame from retained state alone

### 2. Dynamic Retained Realization

Completed for the generic retained scene path.

What now exists:

- generic wrapped-text scenes assign `text_slot` dynamically from retained node
  order
- generic sample scenes assign both `text_slot` and `ui_slot` dynamically from
  the retained scene graph
- retained text layout ownership moved to an internal owned layout structure
  instead of demo-local static arrays
- fixed-layout helpers remain as compatibility scaffolding, not as the primary
  way generic retained scenes are authored

Result:

- retained demos in the generic path no longer require hard-coded slot
  inventories
- partial text/UI patching still works against the dynamically assigned scene

### 3. Scroll-Root Chunk Reuse

Completed in the form that fits the current retained architecture.

What changed:

- scroll invalidation moved into shared retained scene dirty classification
- `RetainedScene::set_scroll_state()` now records the union of old/new visible
  descendants for each scroll root
- generic `Ui2D` and generic `UiPhysical` now patch only those scroll-local
  text/UI resources instead of invalidating every descendant of the scroll root

Why this is the right completion point:

- current wrapped-text scenes already realize one retained text slot per line
- current retained feed/sample scenes already realize one retained text slot per
  row fragment
- that means old/new visible resource reuse already gives chunk-local behavior
  without introducing another cache layer that would duplicate the slot model

Result:

- large retained scroll roots no longer force full descendant resource updates
  on each scroll
- `Ui2D` and `UiPhysical` both benefit from the same shared scroll-local update
  path

### 4. `UiPhysical` Backend Boundary

Completed for the current fullscreen backend family.

What already exists in code:

- `UiPhysicalSceneState` separates retained semantic scene/resource state from
  the runtime host
- `UiPhysicalRuntimeScene` separates the themed fullscreen renderer from the
  active retained scene provider
- `StateBackedUiPhysicalHost`, `FixedUiPhysicalSceneState`, and
  `ThemedUiPhysicalHost` already communicate through those internal interfaces
  rather than through one monolithic shader-specific scene type

Result:

- retained `UiPhysical` semantics are not tied to one concrete demo scene type
- later backend experiments can reuse the same retained scene input and runtime
  contracts

## What This Plan Deliberately Does Not Include

These are still valid future goals, but they are not unfinished parts of this
plan:

- runtime shaping, outline extraction, and tessellation as the primary text path
- public `Document/new` and `Scene/new`
- retained web runtime polish beyond the current wasm retained demo coverage,
  especially any remaining browser/platform polish beyond demo and control parity
- backend capability tiers beyond the current native retained path
- any retained-core unification with `World3D`

## Acceptance Criteria

The native retained plan is considered complete because all of the following are
true in the codebase:

- static retained scenes stay idle when nothing changes
- retained demos can request redraw from retained work itself
- generic retained demos are no longer authored around fixed slot inventories
- large scrollable retained scenes patch only old/new visible resources on
  scroll
- `Ui2D` and `UiPhysical` continue to share one semantic retained scene model
- `World3D` remains separate from the retained UI roadmap

## Related Internal Sources

- `src/demos/runner.rs`
- `src/retained/mod.rs`
- `src/retained/text.rs`
- `src/retained/text_scene.rs`
- `src/retained/samples.rs`
- `src/demos/ui2d_runtime.rs`
- `src/demos/ui_physical_runtime.rs`
