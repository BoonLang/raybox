# Raybox unified SDF rewrite plan

> Execution mode: one branch, one cutover, one renderer, one public API.
> Temporary duplication is acceptable while the rewrite is in flight, but the final tree must not preserve the old runtime split as a supported architecture.

This document is the one-shot implementation plan for replacing Raybox's current mixed set of renderer families with one public scene API, one scene compiler, and one portable `wgpu` runtime that works on native and browser.

It is intentionally grounded in the current Raybox repository. The target is not "invent a fresh engine from scratch"; the target is "reuse the repo's strongest ideas, delete the renderer-family split, and land one shipping runtime."

---

## 1. Non-negotiable end state

Raybox must end up with all of the following, simultaneously:

1. One public scene model for UI, text, shapes, images, effects, lights, and 3D objects.
2. One render entry point: app code builds a scene, sets a view, and calls render.
3. One internal execution model: authoring nodes compile into cached sparse distance data, then the renderer traverses that cache.
4. One portable fast path for native and browser WebGPU. The shipping path must not require native-only features.
5. One retained invalidation model shared across dense website-style UI and 3D scenes.
6. One shader architecture: tracked shader source in `shaders/*.slang`, generated Rust ABI from `build.rs`, no repo-tracked handwritten WGSL.
7. No live fullscreen scene-wide analytic raymarch path as the main renderer.
8. No final-tree split between `Ui2D`, `UiPhysical`, and `World3D` renderer families. Those become scene presets and input/view conventions, not different renderers.
9. No compatibility migration mode in the final design. Existing demos and examples must be fully ported to the new architecture.
10. No primary atlas text path. Text remains vector-to-distance at the source level and is realized into retained distance pages.

In one sentence: Raybox becomes an SDF scene compiler with a sparse distance-cache runtime.

---

## 2. What "one pipeline" means in this plan

"One pipeline" does not mean "one fullscreen shader that directly evaluates one giant analytic SDF tree for every pixel".

In this plan, "one pipeline" means:

- one scene graph
- one dirty-tracking model
- one scene compiler
- one cached runtime representation
- one renderer
- one compositor/present path
- one debug/profiling model
- one browser/native code path

The runtime representation is not the raw authoring graph. The authoring graph is the source of truth; the execution format is a sparse set of cached distance pages and distance bricks.

That is the only way to keep the API simple while making frame time depend mostly on:

- visible screen tiles
- visible pages/bricks
- dirty regions
- active lights/materials

instead of raw scene-node count.

---

## 3. Current repo facts this rewrite must respect

The rewrite should be shaped around the current Raybox tree instead of pretending the repo is empty.

### 3.1 Current structure that matters

Current top-level areas include:

- `src/demos/` with current demo families and runtimes
- `src/retained/` with retained semantic scene data and samples
- `src/text/` with vector font and `char_grid` acceleration logic
- `shaders/*.slang` compiled by `build.rs`
- `examples/*.rs`
- browser/native runner tooling in `src/web.rs`, `src/window_mode.rs`, `src/bin/*`, and `justfile`

### 3.2 Current demo/example inventory that must survive the rewrite

Demos (0-12):

- `Empty`
- `Objects`
- `Spheres`
- `Towers`
- `Text2D`
- `Clay`
- `TextShadow`
- `TodoMvc`
- `TodoMvc3D`
- `RetainedUi`
- `RetainedUiPhysical`
- `TextPhysical`
- `MixedUiWorld`

Examples:

- `examples/demo_clay.rs`
- `examples/demo_objects.rs`
- `examples/demo_spheres.rs`
- `examples/demo_text2d.rs`
- `examples/demo_text_shadow.rs`
- `examples/demo_towers.rs`

### 3.3 Current repo strengths that should be reused, not discarded

- shared retained scene semantics and invalidation
- the current `char_grid` idea for localized lookup and safe stepping
- strict Slang-only shader policy and generated ABI bindings
- browser/native hot reload and control tooling
- existing mixed-scene concepts already present in `frame_graph.rs`

The rewrite should reuse those strengths while deleting the renderer-family split.

### 3.4 Current anti-patterns that must be eliminated

The cutover should explicitly delete the patterns that make the current tree hard to unify:

- example-local `Renderer` types
- example-owned `wgpu::RenderPipeline` creation
- example-owned fullscreen `draw(0..3, 0..1)` scene render paths
- `#[path = "../src/..."]` imports from examples into core modules
- demo modules that own final render encoding
- per-demo scene shaders and per-demo runtime families

`examples/demo_objects.rs` is the concrete warning sign: it currently mixes `#[path = "../src/..."]` imports, example-owned pipeline creation, and a fullscreen triangle draw path. The rewrite should remove that whole pattern rather than reproduce it under new names.

---

## 4. Final public API

The final public API must be centered on a single scene type.

```rust
let mut scene = DistanceScene::new();

scene.add(
    Node::panel(RoundedRect::new(...))
        .space(Space::Screen)
        .material(Material::glass())
);

scene.add(
    Node::text(TextRun::new("TodoMVC", font_id, 36.0))
        .space(Space::Screen)
        .extrusion(6.0)
        .material(Material::plastic())
);

scene.add(
    Node::mesh_asset("assets/chair.glb")
        .space(Space::World)
        .transform(chair_transform)
        .material(Material::painted_metal())
);

scene.add(Light::sun(...));
scene.add(Light::point(...));

renderer.render(&scene, &RenderView::window(&camera));
```

### 4.1 Public types

Required public types:

- `DistanceScene`
- `NodeId`
- `Node`
- `Space`
- `Material`
- `Light`
- `RenderView`
- `Renderer`
- `RenderStats`
- `HitResult`
- `SceneHandle` / `AssetHandle` for cached resources

### 4.2 Node categories

The scene must support these node categories directly:

- 2D shapes: rectangles, rounded rectangles, strokes, paths, borders, separators
- text: glyph outlines, text runs, paragraphs, extruded headings
- images/media surfaces: image/video textures attached to distance-defined panels
- 3D primitives: sphere, box, capsule, cylinder, torus, rounded box, plane, cones
- CSG groups: union, subtract, intersect, smooth variants where supported
- instances: repeated references to precompiled assets/subtrees
- mesh assets: imported meshes realized as sparse narrow-band fields
- portal/view nodes: UI-embedded world views and offscreen previews
- effect nodes: blur, distortion, frosted glass, shadow, glow, backdrop readback
- lights: sun, directional, point, spot, emissive surfaces

### 4.3 Space model

Use a single scene with explicit spaces instead of separate renderers.

Required spaces:

- `Space::Screen` for retained website/UI content in screen space
- `Space::World` for free-camera 3D content
- `Space::ViewAligned` for camera-facing cards, labels, and decals
- `Space::Portal(ViewId)` for content rendered for an embedded or offscreen view

This keeps one scene API while still supporting websites, overlays, and 3D scenes.

---

## 5. Final internal architecture

The final engine is made of five fixed layers.

### 5.1 Layer A - authoring scene graph

This is the source of truth.

Responsibilities:

- stable node ids
- transforms and hierarchical bounds
- retained semantic UI tree
- text/path/vector sources
- mesh asset references
- lights/materials/effect declarations
- dirty flags and generation counters

This layer is where editing happens.

### 5.2 Layer B - scene compiler

This converts the authoring graph into executable cached data.

Responsibilities:

- flatten visible/dirty subtrees into compiler jobs
- prune irrelevant operators and nodes per region
- generate page tasks for screen-space content
- generate brick tasks for world-space content
- map mesh assets to sparse field resources
- update dirty region lists and LOD selection hints
- emit GPU-ready descriptors and task buffers

### 5.3 Layer C - sparse runtime storage

This is the actual execution format.

Two concrete resource kinds are required:

- distance pages for dense 2D/UI/text regions
- distance bricks for 3D regions around visible or active surfaces

These must share one descriptor format philosophy so pages are just a specialized thin-brick case.

### 5.4 Layer D - unified renderer

This is the only renderer.

Responsibilities:

- upload compiler deltas
- cull pages/bricks per view
- build per-tile candidate lists
- resolve local surface intersections
- shade materials with lights
- apply screen-space effects
- composite portals/embedded views
- present final output

### 5.5 Layer E - debug / profiling / control

This is mandatory, not optional.

Responsibilities:

- page/brick/LOD overlays
- dirty-region overlays
- timing queries and CPU/GPU counters
- memory usage reporting
- scene inspector
- hit-test visualizer
- browser/native parity diagnostics

This layer is essential because this rewrite will be painful to maintain without introspection.

---

## 6. The single runtime representation

The runtime representation is the heart of the rewrite.

### 6.1 Distance pages

Use distance pages for screen-space content:

- grouped by scroll root / clip root / z layer / effect compatibility
- stored at device-pixel or retained zoom resolution
- updated by dirty rect
- internally represented as thin distance slabs with one compressed thickness axis
- can carry material ids, coverage, normal hints, opacity flags, and texture handles

This is what makes dense website content fast.

### 6.2 Distance bricks

Use sparse narrow-band bricks for 3D content:

- world-space AABB per brick
- voxel size chosen from a clipmap/LOD policy
- narrow-band storage near the zero set only
- material id and gradient metadata attached to cells
- only dirty or newly visible bricks rebuilt

This is what makes world scenes scale.

### 6.3 Shared descriptor model

Both pages and bricks must use the same descriptor vocabulary:

- bounds
- resolution / voxel size
- material range
- texture range
- data offsets
- LOD level
- dirty epoch
- visibility epoch
- space/view ownership

That unifies the renderer even though pages and bricks have different shapes.

### 6.4 Recommended GPU-side data blocks

Required descriptor and payload families:

- `GpuSceneGlobals`
- `GpuView`
- `GpuMaterial`
- `GpuLight`
- `GpuPageDesc`
- `GpuBrickDesc`
- `GpuTileBin`
- `GpuNodeProgramRange`
- `GpuFieldPoolHeader`
- `GpuTextureBindingTable`
- `GpuEffectDesc`

Use generated shader bindings as the ABI source of truth, following the existing Raybox policy.

---

## 7. Compiler design

The compiler is what turns "simple API" into "fast runtime".

### 7.1 Dirty tracking model

Each node carries:

- stable `NodeId`
- local bounds
- world bounds
- change generation
- semantic change kind
- target spaces/views
- material and effect dependencies

Change kinds must distinguish at minimum:

- transform-only
- material-only
- light-only
- text/layout change
- geometry change
- effect parameter change
- asset replacement

### 7.2 Compiler stages

The compiler must run these stages in order each frame:

1. collect dirty nodes
2. recompute bounds and affected ancestors
3. intersect dirty bounds with pages/bricks already alive
4. generate new page/brick tasks for newly exposed regions
5. prune node programs for each task
6. schedule GPU build jobs
7. upload descriptor deltas and task buffers

### 7.3 Node program flattening

Do not send a fully pointer-heavy scene tree to the GPU.

Flatten relevant subtrees into compact task-local programs.

Recommended instruction families:

- primitive eval instructions
- transform instructions
- repeat/mirror instructions
- boolean instructions
- smooth boolean instructions
- inflate/offset instructions
- glyph/path reference instructions
- mesh-field reference instructions
- material assignment instructions
- effect tag instructions

Each page/brick task receives only the minimal program range needed for that region.

### 7.4 Local pruning

Pruning is required.

For each page/brick task:

- discard nodes whose bounds cannot intersect the task bounds
- reduce boolean subtrees when one side is provably irrelevant in the local region
- collapse constant-sign regions
- reuse precompiled mesh-field references instead of traversing original triangles
- cluster repeated glyph/path references per task

### 7.5 Mesh assets

Mesh assets must be first-class citizens.

Required behavior:

- load mesh assets into an internal `DistanceAsset`
- bake them to sparse narrow-band fields
- persist baked results under `assets/compiled/` or equivalent cache location
- support hot reload / asset replacement by invalidating only affected resources
- allow multiple instances without rebaking geometry

Add a dedicated asset tool:

- `src/bin/raybox_asset.rs`

Required commands:

- `raybox-asset bake <mesh>`
- `raybox-asset inspect <asset>`
- `raybox-asset rebuild-all`

### 7.6 Text and vector shapes

Text and vector shapes remain source-level exact geometry, but runtime rendering must be page-based.

Required behavior:

- parse and cache glyph outlines via existing font/vector code
- shape paragraphs and runs on CPU
- compile visible text runs into distance pages by dirty rect
- compile vector paths into the same page builder
- remove per-frame exact Bezier tracing from the live renderer

### 7.7 Images and sampled media

Images are allowed.

Rule:

- the surface is distance-defined
- the content is a sampled texture/material payload

This keeps the unified geometry model without pretending JPEGs are SDFs.

---

## 8. Renderer design

The runtime is a single renderer with fixed stages.

### 8.1 Stage 1 - update and upload

Input:

- compiler-produced page/brick tasks
- descriptor deltas
- material/light/effect deltas

Output:

- updated page table
- updated brick table
- uploaded field payloads
- updated scene globals

### 8.2 Stage 2 - visibility and tiling

Use compute to:

- cull pages and bricks against the current view
- classify tiles touched by each visible page/brick
- build compact tile candidate lists
- build per-view portal candidate lists

This stage is essential for both dense UI and large 3D scenes.

### 8.3 Stage 3 - local surface resolve

For each screen tile/pixel candidate:

- traverse only the relevant page/brick candidates
- do local stepping or DDA through occupied samples
- find the zero crossing inside the local cached field
- refine with a short root solve
- fetch normal/gradient/material information

Important: this is not fullscreen scene-wide analytic sphere tracing. It is local traversal against cached fields.

### 8.4 Stage 4 - shading and effects

Use one lighting/material model for both physical UI and world objects.

Material families required:

- unlit UI
- standard lit surface
- metallic/roughness
- translucent glass
- frosted glass / backdrop blur
- emissive
- shadow receiver / contact shadow
- signed edge glow / outline

Effects must be driven by tagged effect descriptors, not separate renderer families.

### 8.5 Stage 5 - composition and present

Use one composition path for:

- world + screen-space UI
- UI-only scenes
- world-only scenes
- UI with embedded world preview
- world with transparent UI overlay

The internal frame graph can remain, but demos must never construct manual packet families anymore.

---

## 9. Browser and native policy

The shipping path must target modern browser WebGPU and native `wgpu`, not a native-only feature set.

### 9.1 Feature policy

Allowed as portable core:

- standard WebGPU pipeline features
- compute pipelines
- storage buffers/textures within WebGPU-safe limits
- timestamp queries when available
- `f16` shader paths when available

Forbidden in the portable shipping path:

- mesh shaders
- ray query
- acceleration-structure-dependent renderer design
- anything that only works on native-only experimental `wgpu` features

### 9.2 Limits policy

Use conservative requested limits that stay inside modern WebGPU-safe envelopes.

Rules:

- do not request "best possible" adapter limits
- request only what the unified runtime truly needs
- keep bind-group counts, storage sizes, workgroup sizes, and texture dimensions compatible with the browser target

### 9.3 Platform policy

- the browser target is WebGPU, not WebGL2 fallback
- native and browser must share the same scene compiler and renderer logic
- any platform-specific code must be restricted to surface creation, input, timing, hot reload, and browser interop

### 9.4 Shader policy

Keep the current Slang pipeline, but extend it to the new runtime.

Required changes:

- add compute entry point support to `build.rs`
- generate ABI bindings for compute stages too
- keep all tracked shader source in `shaders/*.slang`
- forbid repo-tracked handwritten WGSL in runtime and examples

### 9.5 Repository guardrails

These rules from the cutover plan are worth keeping verbatim in spirit because they prevent architectural backslide:

- no example file may own a `wgpu::RenderPipeline`
- no example file may use `#[path = "../src/..."]` to reach core modules
- no example file may directly read assets from disk with `std::fs::read`
- no demo module may own final render encoding
- no demo module may compile scene-specific GPU pipelines
- no shader file may exist purely for one example or one demo family

---

## 10. Required final module layout

The final tree should look conceptually like this.

```text
src/
  scene/
    mod.rs
    node.rs
    material.rs
    light.rs
    effect.rs
    text.rs
    shape.rs
    mesh.rs
    image.rs
    space.rs
    view.rs
  compiler/
    mod.rs
    dirty.rs
    bounds.rs
    prune.rs
    page_tasks.rs
    brick_tasks.rs
    node_program.rs
    asset_bake.rs
    scene_compiler.rs
  runtime/
    mod.rs
    page_pool.rs
    brick_pool.rs
    descriptor_tables.rs
    tile_bins.rs
    visibility.rs
    stats.rs
    hit_test.rs
  render/
    mod.rs
    update.rs
    visibility.rs
    surface.rs
    lighting.rs
    effects.rs
    composite.rs
    present.rs
  platform/
    mod.rs
    native.rs
    web.rs
  demos/
    mod.rs
    runner.rs
    scene_demo.rs
    ...scene builder demos only...
  retained/
    mod.rs
    ...semantic retained front-end...
  text/
    mod.rs
    vector_font.rs
    text_compile.rs
  spatial/
    mod.rs
    grid2d.rs
    grid3d.rs
```

### 10.1 Files/directories that must disappear from the final architecture

These are not allowed as supported runtime families in the final tree:

- `src/renderer.rs`
- `src/sdf_renderer.rs`
- `src/demos/ui2d_runtime.rs`
- `src/demos/ui_physical_runtime.rs`
- `src/demos/world3d_runtime.rs`

These may be deleted outright or reduced to stubs that re-export the unified runtime, but the old renderer split must not remain architecturally real.

### 10.2 Files that should be rewritten instead of deleted

- `src/demos/frame_graph.rs` - keep only as internal task scheduling / composition support
- `src/demos/mod.rs` - new demo trait and scene-builder registration
- `src/demo_core/mod.rs` - unify native/web demo trait around scene emission
- `src/retained/*` - keep semantics, change backend emission target
- `src/text/char_grid.rs` - generalize into shared spatial indexing logic
- `build.rs` - extend to compute shaders and new shader set
- `Cargo.toml` - add new modules/bin/features as needed
- `justfile` - update commands and verification targets
- `src/web.rs`, `src/window_mode.rs`, `src/bin/demos.rs`, `src/bin/raybox_dev.rs`, `src/bin/raybox_ctl.rs`

### 10.3 Text subsystem changes

Keep:

- `vector_font.rs`

Replace/rework:

- `char_grid.rs` -> generalized spatial binning support
- `glyph_atlas.rs` -> remove from the live primary path or delete if no longer needed

Final rule:

- text is vector source -> retained distance pages
- no exact per-pixel curve tracing in the live main renderer
- no atlas-based primary text renderer

---

## 11. New shader set

Replace the current render-family shader inventory with a unified runtime shader set.

Recommended tracked shaders:

- `distance_update.slang` - compute
- `distance_page_build.slang` - compute
- `distance_brick_build.slang` - compute
- `distance_visibility.slang` - compute
- `distance_surface.slang` - fragment or compute-based surface resolve
- `distance_lighting.slang` - shared lighting helpers
- `distance_effects.slang` - blur/distortion/backdrop/effect processing
- `distance_composite.slang` - portal/effect composition
- `present.slang` - final present
- `overlay.slang` - optional debug overlay only

Rules:

- no demo-specific renderer family shaders
- legacy scene-specific shaders may remain only as scene-building helper logic if absolutely necessary, but not as the architecture
- generated bindings must cover all shader ABI surfaces

---

## 12. Big-bang execution order for Codex CLI

This is the exact order Codex CLI should follow. The branch is not done until all steps complete and dead code is removed.

### Step 1 - create the new architecture skeleton

- add `src/scene/`, `src/compiler/`, `src/runtime/`, `src/render/`, `src/spatial/`, `src/platform/`
- wire them from `src/lib.rs`
- define `DistanceScene`, `Node`, `Space`, `Material`, `Light`, `RenderView`, `Renderer`
- define one new demo trait based on scene building, not direct render-pass encoding

### Step 2 - replace demo trait semantics

- remove render-family methods from the live demo abstraction (`render_world_view`, `render_ui_layer`, etc.)
- demos must become scene builders + update/input handlers
- runner owns the unified renderer
- demos never encode their own final passes

### Step 3 - build dirty tracking and retained scene emission

- keep stable node ids
- teach retained UI code to emit `DistanceScene` nodes
- add explicit dirty rect / dirty volume reporting
- ensure scroll roots, clip roots, and text runs map to compiler-relevant bounds

### Step 4 - generalize spatial indexing

- turn `char_grid` ideas into shared `Grid2D` / `Grid3D`
- use `Grid2D` for UI/text page bucketing
- use `Grid3D` for brick occupancy and neighborhood lookup

### Step 5 - implement node-program flattening and pruning

- add compact per-task node programs
- add local subtree pruning by bounds and operator semantics
- add reusable mesh-field references
- emit compact compiler task buffers

### Step 6 - implement distance page compilation

- group screen-space retained content into pages
- support dirty-rect rebuilds
- compile text, paths, borders, rounded panels, separators, and icons into page fields
- attach material/effect metadata to page outputs

### Step 7 - implement distance brick compilation

- create sparse brick pool
- create clipmap/LOD selection for world-space content
- compile analytic primitives, CSG groups, and mesh assets into narrow-band brick data
- support partial rebuild by dirty volume

### Step 8 - add mesh asset bake tool

- add `raybox-asset` binary
- implement bake/read/inspect pipeline
- store compiled sparse field assets in a deterministic on-disk cache
- integrate hot reload for modified mesh assets

### Step 9 - extend shader toolchain

- update `build.rs` so Slang compute entry points are compiled and reflected too
- generate ABI bindings for update/page-build/brick-build/visibility/surface/effects shaders
- add architecture guardrails that reject reintroduction of handwritten WGSL or dead renderer-family ABI

### Step 10 - implement unified renderer core

- update/upload stage
- visibility/tile-bin stage
- local surface-resolve stage
- shared lighting/effects stage
- composition/present stage
- `RenderStats` and debug views

### Step 11 - implement hit testing and scene queries

- screen hit testing for buttons, lists, text carets, and links
- world picking for objects and surfaces
- one query path based on page/brick data, not separate UI/world picking systems

### Step 12 - port retained UI demos

Rewrite these to emit unified scene nodes only:

- `src/demos/todomvc.rs`
- `src/demos/todomvc_3d.rs`
- `src/demos/retained_ui.rs`
- `src/demos/retained_ui_physical.rs`
- `src/demos/text2d.rs`
- `src/demos/text_physical.rs`
- `src/demos/text_shadow.rs`
- `src/demos/clay.rs`

### Step 13 - port world demos

Rewrite these to emit unified scene nodes only:

- `src/demos/objects.rs`
- `src/demos/spheres.rs`
- `src/demos/towers.rs`
- `src/demos/empty.rs`

### Step 14 - port mixed-scene demo

Rewrite:

- `src/demos/mixed_ui_world.rs`

Required final behavior:

- one `DistanceScene`
- one unified renderer
- portal or embedded world preview supported through the same runtime
- transparent UI overlay and UI-in-world both possible

### Step 15 - port example binaries

Every file in `examples/` must become a thin scene-building app that uses the unified renderer:

- `demo_clay.rs`
- `demo_objects.rs`
- `demo_spheres.rs`
- `demo_text2d.rs`
- `demo_text_shadow.rs`
- `demo_towers.rs`

No example may keep a parallel rendering architecture.

### Step 16 - port native runner and control tooling

Update:

- `src/demos/runner.rs`
- `src/window_mode.rs`
- `src/bin/demos.rs`
- `src/bin/raybox_ctl.rs`
- `src/bin/raybox_dev.rs`

Required behavior:

- unified renderer bootstrapping
- same debug counters on native and web
- hot reload preserves scene/view/debug state
- control server reports unified stats: pages, bricks, dirty counts, GPU memory, timings

### Step 17 - port web runtime

Update:

- `src/web.rs`
- `src/web_input.rs`
- `src/web_control.rs`
- `index.html`

Required behavior:

- browser target boots the same renderer
- same demos 0-12 exist
- same scene/view state save/restore
- same control protocol
- same screenshot/smoke pipeline

### Step 18 - delete dead architecture

Delete or fully neutralize:

- old renderer-family runtime files
- old demo-owned render-pass logic
- old fullscreen analytic live render path
- dead text fallback plumbing
- dead shader inventory
- obsolete docs describing the old runtime split

### Step 19 - rewrite docs and guardrails

Replace or rewrite:

- `docs/plans/unify_and_optimize.md`
- `docs/plans/retained.md`
- `README.md`
- `HOW_HOT_RELOAD_WORKS__VERIFY.md`
- `AGENTS.md`
- `CLAUDE.md`

The docs must describe the new architecture only.

### Step 20 - verify everything, then stop

The work is not done until the verification matrix in section 17 passes and the acceptance criteria in section 18 are true.

### Condensed cutover checklist

Use this as the short work queue when the longer step list is too wide:

- freeze the new public API and `src/lib.rs` exports around it
- build the portable asset layer for native and web
- build the one scene compiler with invalidation, page keys, page allocators, and brick allocators
- build the one runtime renderer with generic GPU layout, visibility, lighting, effects, and present
- rewrite all scenes, demos, and examples into thin scene builders
- replace per-demo shader/runtime ownership with generic runtime shaders and bindings
- rewrite native/web entrypoints as thin platform adapters
- update tests, scripts, commands, and docs together
- remove dead legacy modules, legacy shaders, and legacy dependencies before calling the rewrite done

---

## 13. How each existing demo and example changes

This section is the explicit porting contract.

### 13.1 Demo conversion matrix

#### `Empty`

New role:

- boots the unified renderer with an empty scene
- proves present/debug plumbing and zero-content correctness

Must verify:

- no crashes
- correct clear/present
- page/brick counts remain zero

#### `Objects`

New role:

- world scene using imported or procedural distance assets

Must verify:

- mesh asset bake/load path
- lights/materials in world space
- camera movement updates visibility only, not geometry rebuilds

#### `Spheres`

New role:

- analytic primitive/CSG scene compiled to bricks

Must verify:

- primitive node programs
- boolean pruning
- narrow-band brick generation

#### `Towers`

New role:

- repeated 3D primitives at larger world scale

Must verify:

- instance handling
- clipmap/LOD
- repeated-content pruning and brick reuse

#### `Text2D`

New role:

- screen-space text and vector UI compiled to pages

Must verify:

- retained page updates
- zoom/pan/rotate behavior
- crisp text from page rebuilds

#### `Clay`

New role:

- text/shape relief demo using distance-defined surface detail

Must verify:

- text-to-page compilation
- material response on shallow relief
- shadowing and surface normals

#### `TextShadow`

New role:

- text plus unified effect nodes

Must verify:

- soft shadow effect path
- text page compilation
- no separate text renderer family remains

#### `TodoMvc`

New role:

- content-heavy retained UI demo compiled to screen pages

Must verify:

- scroll roots and dirty rects
- list item edits only rebuild affected pages
- hit testing and text updates remain correct

#### `TodoMvc3D`

New role:

- same semantic retained UI as `TodoMvc`, but realized with depth/extrusion/materials in the same unified runtime

Must verify:

- same retained source model
- same controls/data model
- different scene styling only, not different renderer family

#### `RetainedUi`

New role:

- retained UI showcase on the unified page compiler

Must verify:

- scroll/clip/layout correctness
- mixed text, shapes, images, and effects

#### `RetainedUiPhysical`

New role:

- same retained showcase with physical styling and lighting

Must verify:

- shared retained semantics
- page/bricks used appropriately under one runtime

#### `TextPhysical`

New role:

- extruded headings / labels in the unified pipeline

Must verify:

- text source remains vector based
- runtime is page/brick compiled
- lighting on extruded text works

#### `MixedUiWorld`

New role:

- proof that one scene/runtime can do UI and world content together

Must verify:

- screen UI over world
- embedded world view inside UI region
- one renderer path for both

### 13.2 Example conversion matrix

#### `examples/demo_objects.rs`

- use `DistanceScene`
- build world objects and lights
- no custom renderer bootstrap beyond standard example scaffolding

#### `examples/demo_spheres.rs`

- build primitive world scene using analytic source nodes
- prove CSG-to-brick compile path

#### `examples/demo_towers.rs`

- build repeated structures / instances
- prove LOD and culling behavior

#### `examples/demo_text2d.rs`

- build screen-space text scene
- prove page compilation and text interaction path

#### `examples/demo_clay.rs`

- build relief/text material demo
- prove text/shape compilation and material shading

#### `examples/demo_text_shadow.rs`

- build text + effect demo
- prove effect tags and shared runtime

---

## 14. Required debug and observability features

This plan will be hard to finish without good introspection. Make these features mandatory.

### 14.1 On-screen overlays

Required overlays:

- page bounds
- brick bounds
- dirty rects / dirty volumes
- LOD level heatmap
- tile candidate counts
- material id view
- normal view
- shadow/effect mask view
- portal/view boundaries

### 14.2 Control/CLI inspection

Update `raybox-ctl status` to report at least:

- frame time
- GPU time by stage
- visible pages
- visible bricks
- dirty pages rebuilt this frame
- dirty bricks rebuilt this frame
- uploaded bytes this frame
- page pool usage
- brick pool usage
- texture/effect resource counts

### 14.3 Deterministic capture

Keep and extend screenshot tooling.

Required outputs:

- native capture for demos 0-12
- web capture for demos 0-12
- native capture for all examples
- web capture for all examples that are browser-capable

---

## 15. Browser/native integration checklist

This is the platform parity contract.

### 15.1 Native

Must support:

- windowed demos
- examples
- screenshot capture
- hot reload
- control server
- profiling overlays

### 15.2 Browser

Must support:

- same demo ids and names
- same unified renderer
- same scene/view persistence across hot reload
- same control protocol where applicable
- smoke screenshot capture
- same debug overlays where possible

### 15.3 Shared behavior

The following must behave the same on native and browser:

- demo switching
- retained UI editing
- camera movement
- portal rendering
- page/brick stats semantics
- scene serialization for hot reload

---

## 16. Commands that must work at the end

At minimum, the final rewrite must preserve and/or update these workflows so they work on the unified architecture.

### 16.1 Build and run

```bash
cargo test
cargo build --bin demos --features windowed,control,mcp
cargo build --examples --features windowed
cargo check --lib --target wasm32-unknown-unknown --features web
just demos
just build-web
just web
just web-smoke
```

### 16.2 Control and inspection

```bash
just demos-control
just ctl status
just ctl switch 12
```

### 16.3 Hot reload

```bash
just dev
just dev-web
just open-browser-hotreload
```

### 16.4 Asset pipeline

```bash
cargo run --bin raybox-asset -- bake assets/example.glb
cargo run --bin raybox-asset -- inspect assets/compiled/example.rbxfield
```

If new commands are introduced, update `justfile`, docs, and verification scripts together.

---

## 17. Verification matrix

This is the pass/fail checklist.

### 17.1 Automated

All must pass:

- `cargo test`
- shader architecture checks
- build of demo binary with `windowed,control,mcp` features
- build of all examples
- wasm check/build for the web target
- real demo verification via `scripts/test_all_demos.sh`
- any new asset-bake tests
- any new scene compiler unit tests
- any new page/brick serialization tests

### 17.2 Manual native verification

Verify:

- demos 0-12 launch and switch correctly
- every example launches
- retained UI editing works
- mixed UI/world demo works
- page/brick overlays are meaningful
- screenshots can be captured

### 17.3 Manual web verification

Verify:

- same demo inventory exists
- same demo switching works
- retained UI editing works
- mixed UI/world demo works
- web smoke screenshot succeeds
- hot reload preserves demo/view state

### 17.4 Structural verification

The final codebase must satisfy these structural truths:

- no live renderer-family split remains
- no demo owns final render-pass encoding
- no live fullscreen analytic scene raymarch is the main path
- all demos/examples use the same scene/renderer entry point
- tracked shaders are Slang only
- generated bindings are still the ABI source of truth
- browser/native share the same renderer logic

---

## 18. Acceptance criteria

This rewrite is done only when all of the following are true.

1. A developer can build one `DistanceScene` containing UI, text, lights, and 3D objects and render it with one renderer.
2. The shipping runtime uses cached distance pages/bricks, not a giant fullscreen analytic scene march.
3. `Ui2D`, `UiPhysical`, and `World3D` no longer exist as separate renderer families.
4. All current demos and examples are ported.
5. Native and browser run the same renderer architecture.
6. Old renderer-family files and dead fallback code are removed or fully reduced to thin aliases over the new runtime.
7. Documentation and guardrails describe the new architecture only.
8. The repo passes the verification matrix.

If any one of these is false, the rewrite is not complete.

---

## 19. Recommended implementation decisions

These are the decisions I would hard-code unless the code itself forces a better answer.

### 19.1 Keep retained UI as the semantic front-end

Do not throw away the retained UI work. Use it as the authoring/input/layout front-end for screen-space content.

### 19.2 Keep Slang everywhere

Do not replace the current shader toolchain during the rewrite. Extend it to compute stages and the new runtime.

### 19.3 Make pages the default for website content

Dense UI/text should land in pages, not direct live analytic evaluation.

### 19.4 Make bricks the default for world content

3D objects and physical UI should land in sparse bricks, with clipmap-style LOD.

### 19.5 Keep analytic geometry only at source/compile time

Analytic/vector/SDF definitions remain the authoring truth. The renderer consumes compiled cached fields.

### 19.6 Keep debugging first-class

If the rewrite is hard to introspect, it will become painful to maintain.

---

## 20. Research basis and current-source references

These sources justify the design direction and the current platform constraints.

### Raybox current repo and architecture

- Raybox repository root and current tree: `https://github.com/BoonLang/raybox`
- current `docs/plans/unify_and_optimize.md`
- current `docs/plans/retained.md`
- current `docs/plans/slang_everywhere.md`
- current `Cargo.toml`
- current `justfile`
- current demo and example inventory under `src/demos/`, `src/retained/`, `src/text/`, and `examples/`

### Current platform/runtime constraints

- `wgpu` crate docs: cross-platform, native + wasm/WebGPU/WebGL runtime surface
- `wgpu` `FeaturesWebGPU`: `TIMESTAMP_QUERY` and `SHADER_F16` are web+native features
- `wgpu` `FeaturesWGPU`: `EXPERIMENTAL_MESH_SHADER` and `EXPERIMENTAL_RAY_QUERY` are native-only features
- `wgpu` `Limits`: request conservative limits instead of over-requesting adapter capabilities
- browser WebGPU support references

### Rendering and data-structure references

- Vello docs for large-scene 2D `wgpu` rendering
- WebRender blob/tile/cache design notes
- Frisken et al., *Adaptively Sampled Distance Fields: A General Representation of Shape for Computer Graphics*
- Museth, *NanoVDB: A GPU-Friendly and Portable VDB Data Structure for Volumes*
- Hansson Soderlund et al., *Ray Tracing of Signed Distance Function Grids*
- Barbier et al., *Lipschitz Pruning: Hierarchical Simplification of Primitive-Based SDFs*
- Epic Nanite docs for the "virtualized geometry" reference point

---

## 21. Final instruction to Codex CLI

Do not stop at "the architecture exists".

The task is finished only when:

- the old runtime split is gone
- the new unified SDF scene compiler/runtime is the real shipping path
- all demos/examples are ported
- native and browser both work
- docs/tests/guardrails are updated
- the repo is left in a clean final state rather than a migration half-state

That is the standard for completion.
