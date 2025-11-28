# Emergent Renderer Master Plan

## Project Vision

**Raybox** is a physically-based 3D UI renderer for the **Boon** programming language. The goal is to create UI where **geometry emerges from spatial relationships** rather than explicit declarations.

### The Revolutionary Insight

```
TRADITIONAL:  "I declare this button has a drop shadow with blur=4px, spread=2px..."
EMERGENT:     "This button floats 50px above its parent. Shadows emerge from lighting."
```

**Result:** 15x code reduction for shadows/bevels, instant theme switching, physics-based intuition.

---

## Table of Contents

1. [Core Philosophy](#1-core-philosophy)
2. [Architecture Overview](#2-architecture-overview)
3. [SDF Techniques](#3-sdf-techniques)
4. [Boon Runtime Integration](#4-boon-runtime-integration)
5. [Element System](#5-element-system)
6. [Material System](#6-material-system)
7. [Lighting System](#7-lighting-system)
8. [Theme System](#8-theme-system)
9. [Implementation Phases](#9-implementation-phases)
10. [File Structure](#10-file-structure)
11. [Integration Points](#11-integration-points)
12. [Success Metrics](#12-success-metrics)
13. [Open Questions](#13-open-questions)
14. [References](#14-references)

---

## 1. Core Philosophy

### Emergent Geometry

**Principle:** Geometry emerges from spatial relationships, not explicit declarations.

| User Specifies | Renderer Generates |
|----------------|-------------------|
| `depth: 8` | 3D box, 8px thick |
| `move: [closer: 50]` | Position 50px toward camera |
| Contact with parent | Outward bevel at contact zone |
| `move: [further: 4]` | Recessed into parent → inward fillet |
| Scene lighting | Real shadows (no configuration) |

### Physical Metaphor

- **Buttons** = objects sitting ON a surface → raised, beveled, cast shadows
- **Inputs** = wells carved INTO a surface → recessed, filleted, inset shadows
- **Focus** = spotlight illuminating element → glow effect
- **Disabled** = pushed back into shadow → faded appearance

### Code Comparison

**Traditional (explicit):**
```css
box-shadow:
  0 2px 4px rgba(0,0,0,0.2),
  0 25px 50px rgba(0,0,0,0.1);
border-radius: 4px;
background: linear-gradient(...);
```

**Emergent (physical):**
```boon
depth: 8
move: [closer: 50]
material: Surface
```

---

## 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         BOON RUNTIME                            │
│  RUN.bn → Parser → Evaluator → Actor Streams                    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      RAYBOX BRIDGE                              │
│  Scene streams → Element tree → Geometry commands               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    SCENE GRAPH BUILDER                          │
│  - Element hierarchy with world positions                       │
│  - Contact zone detection (parent-child relationships)          │
│  - Automatic bevel/fillet generation                            │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     SDF SCENE COMPILER                          │
│  - Convert elements to SDF primitives                           │
│  - Boolean operations (union, subtract, smooth blend)           │
│  - Gradient-SDF for instant normals                             │
│  - Pack into GPU-friendly buffers                               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      LIGHTING SYSTEM                            │
│  - Directional (sun) → parallel shadows                         │
│  - Ambient (fill) → no shadows                                  │
│  - Point (spotlight) → focus effects                            │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    RAYMARCHING RENDERER                         │
│  - Full-screen quad + ray-march shader                          │
│  - SDF scene evaluation per pixel                               │
│  - Shadow rays for real shadows                                 │
│  - PBR material shading                                         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      TEXT RENDERING                             │
│  - MSDF atlas for crisp text                                    │
│  - Text as 3D quads in scene                                    │
│  - Future: FDF for gradient typography                          │
└─────────────────────────────────────────────────────────────────┘
```

---

## 3. SDF Techniques

### Phase 1: Analytic SDF (Foundation)

**Primitives:**
```wgsl
fn sd_rounded_box(p: vec3f, size: vec3f, radius: f32) -> f32 {
    let q = abs(p) - size + radius;
    return length(max(q, vec3f(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - radius;
}
```

**Boolean Operations:**
```wgsl
fn op_union(d1: f32, d2: f32) -> f32 { return min(d1, d2); }
fn op_subtract(d1: f32, d2: f32) -> f32 { return max(d1, -d2); }
fn op_smooth_union(d1: f32, d2: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (d2 - d1) / k, 0.0, 1.0);
    return mix(d2, d1, h) - k * h * (1.0 - h);
}
```

### Phase 2: Gradient-SDF (Better Lighting)

**Store distance + gradient:**
- Instant normals without finite differences
- Better ray-marching step sizing
- Cleaner specular highlights

### Phase 3: MSDF Text (Crisp Typography)

**Multi-channel SDF:**
- 3 distance fields in RGB channels
- Sharp corners at any zoom level
- Tiny atlas, infinite scalability

### Phase 4: Lᵖ-SDF (Style Control)

**Non-Euclidean distance for themes:**
- p = 2.0 (Euclidean) → rounded/soft
- p = ∞ (Chebyshev) → boxy/sharp
- p = 1.0 (Manhattan) → diamond shapes

### Phase 5: ADF (Performance)

**Adaptive Distance Fields:**
- Octree storage, dense only near surfaces
- Automatic LOD
- For complex scenes

### Future: FDF (Gradient Typography)

**Foliated Distance Fields:**
- Gradients across glyphs without artifacts
- For branded/themed text

---

## 4. Boon Runtime Integration

### Boon Execution Model

```
Boon Source (.bn)
    ↓ Lexer
    ↓ Parser (LALR)
    ↓ Reference Resolver
    ↓ Evaluator → Rust Actor Streams
    ↓ Bridge → Renderer Commands
```

**Key:** Boon is interpreted, NOT compiled to WASM. All values are reactive streams.

### Current State

| Component | Status |
|-----------|--------|
| Parser | ✅ Complete |
| Evaluator | ✅ Complete |
| Engine (actors) | ✅ Complete |
| 2D Bridge (Zoon) | ✅ Working |
| 3D Bridge (Raybox) | ⏳ Not implemented |
| Scene/new() API | ⏳ Not implemented |

### Required API Functions (in api.rs)

```rust
// Scene creation
pub fn function_scene_new(...) -> impl Stream<Item = Value>

// Lighting
pub fn function_light_directional(...) -> impl Stream<Item = Value>
pub fn function_light_ambient(...) -> impl Stream<Item = Value>
pub fn function_light_point(...) -> impl Stream<Item = Value>
```

### Bridge Implementation (raybox_bridge.rs)

```rust
pub fn scene_to_raybox_commands(
    scene: Arc<Object>,
    context: ConstructContext,
) -> impl Signal<Item = RayboxRenderCommand> {
    // 1. Subscribe to scene.root_element stream
    // 2. Convert element tree to geometry
    // 3. Apply materials and lighting
    // 4. Emit render commands
}
```

### Data Flow: Boon → Raybox

```
RUN.bn: Scene/new(root: ..., lights: ..., geometry: ...)
    ↓
Evaluator: Creates Scene object with reactive streams
    ↓
Bridge: Subscribes to scene streams
    ↓
On change: Convert to geometry + materials
    ↓
Raybox: Render via WebGPU ray-marching
```

---

## 5. Element System

### Built-in Elements

| Element | 3D Behavior | Geometry |
|---------|-------------|----------|
| `Element/stripe` | Layout container | Transparent (children only) |
| `Element/block` | Container | Rounded box with depth |
| `Element/button` | Raised above parent | Beveled edges, drop shadow |
| `Element/text_input` | Recessed into parent | Cavity with filleted edges |
| `Element/checkbox` | Interactive toggle | Icon swap, spring animation |
| `Element/text` | On surface | MSDF quad in 3D space |
| `Element/link` | Interactive text | Material glow on hover |

### Style Properties

**Layout:**
- `width`, `height` - Dimensions (Fill, fixed px, etc.)
- `padding` - [top, right, bottom, left] or [all: N]
- `gap` - Spacing between children
- `align` - [row: Left/Center/Right, column: Top/Center/Bottom]

**3D Positioning:**
- `depth` - Element thickness (px)
- `move` - [closer: N] or [further: N] - Z-offset from parent

**Geometry:**
- `rounded_corners` - Corner radius, Fully (pill), None (sharp)
- `borders` - [width, color]

**Material:**
- `material` - [color, gloss, metal, glow]
- See Material System section

### Contact Zone Detection

```rust
fn detect_contact(parent: &Element, child: &Element) -> ContactType {
    let child_back = child.z - child.depth / 2.0;
    let parent_front = parent.z + parent.depth / 2.0;

    if child_back > parent_front {
        ContactType::Floating    // Gap → no bevel
    } else if child_back == parent_front {
        ContactType::Touching    // Contact → outward bevel
    } else {
        ContactType::Embedded    // Overlap → inward fillet
    }
}
```

### Automatic Geometry Generation

**Button (move_closer):**
```wgsl
// Child raised above parent → smooth union at contact
let d = op_smooth_union(parent_sdf, child_sdf, bevel_radius);
```

**Input (move_further):**
```wgsl
// Child recessed into parent → smooth subtraction
let cavity = sd_rounded_box(p - child_pos, child_size, corner_radius);
let d = op_smooth_subtract(parent_sdf, cavity, fillet_radius);
```

---

## 6. Material System

### Material Properties

```rust
struct Material {
    color: Oklch,           // Color in perceptual space
    gloss: f32,             // 0.0 = matte, 1.0 = mirror
    metal: f32,             // 0.0 = plastic, 1.0 = metal
    glow: Option<Glow>,     // Emissive light
}

struct Glow {
    color: Oklch,
    intensity: f32,
}
```

### Material Presets (from Theme)

| Semantic Tag | Use Case |
|--------------|----------|
| `Background` | Page background |
| `Surface` | Card/panel surface |
| `SurfaceVariant` | Secondary surface |
| `SurfaceElevated` | Raised surface |
| `Interactive[hovered]` | Clickable, hover state |
| `InteractiveRecessed[focus]` | Input well, focus state |
| `Primary` | Primary action |
| `PrimarySubtle` | Selected filter |
| `Danger` | Destructive action |
| `Ghost` | Disabled state |

### Material in Boon

```boon
material: Theme/material(of: Interactive[hovered: element.hovered])

-- Or composed:
material: [
    ...Theme/material(of: SurfaceElevated)
    glow: hovered |> WHEN {
        True => [color: Theme/material(of: Danger).color, intensity: 0.08]
        False => None
    }
]
```

### Oklch to PBR Conversion

```rust
fn oklch_to_pbr(oklch: Oklch, gloss: f32, metal: f32) -> PBRMaterial {
    PBRMaterial {
        base_color: oklch.to_srgb(),
        roughness: 1.0 - gloss,  // Invert gloss to roughness
        metallic: metal,
        emissive: Vec3::ZERO,    // Set separately from glow
    }
}
```

---

## 7. Lighting System

### Light Types

```rust
enum Light {
    Directional {
        azimuth: f32,      // Horizontal angle (degrees)
        altitude: f32,     // Elevation angle (degrees)
        spread: f32,       // Shadow softness
        intensity: f32,
        color: Oklch,
    },
    Ambient {
        intensity: f32,
        color: Oklch,
    },
    Point {
        position: Vec3,
        intensity: f32,
        color: Oklch,
        falloff: f32,
    },
}
```

### Shadow Casting

**Only Directional and Point lights cast shadows.**

```wgsl
fn soft_shadow(ro: vec3f, rd: vec3f, mint: f32, maxt: f32, k: f32) -> f32 {
    var res = 1.0;
    var t = mint;
    for (var i = 0; i < 64; i++) {
        let h = scene_sdf(ro + rd * t);
        res = min(res, k * h / t);
        t += clamp(h, 0.02, 0.1);
        if h < 0.001 || t > maxt { break; }
    }
    return clamp(res, 0.0, 1.0);
}
```

### Theme = Light Configuration

| Theme | Light Direction | Shadow Softness | Ambient |
|-------|-----------------|-----------------|---------|
| Professional | 30° azimuth, 45° altitude | High (soft) | 0.4 |
| Neobrutalism | 0° azimuth, 30° altitude | Low (sharp) | 0.2 |
| Neumorphism | 315° azimuth, 45° altitude | Very high | 0.6 |

### Lighting in Boon

```boon
lights: LIST {
    Light/directional(
        azimuth: 30
        altitude: 45
        spread: 1
        intensity: 1.2
        color: Oklch[lightness: 1, chroma: 0]
    )
    Light/ambient(
        intensity: 0.4
        color: Oklch[lightness: 1, chroma: 0]
    )
}
```

---

## 8. Theme System

### Two-Layer Architecture

**Layer 1: Theme.bn (Router)**
```boon
FUNCTION material(of) { get(from: Material, of: of) }
FUNCTION font(of) { get(from: Font, of: of) }
-- ... 9 more wrappers

FUNCTION get(from, of) {
    PASSED.theme_options.name |> WHEN {
        Professional => Professional/get(from: from, of: of)
        Neobrutalism => Neobrutalism/get(from: from, of: of)
        -- etc.
    }
}
```

**Layer 2: Theme Implementations**
- `Professional.bn` - Soft, neutral
- `Neobrutalism.bn` - Sharp, bold
- `Glassmorphism.bn` - Translucent
- `Neumorphism.bn` - Subtle, monochromatic

### Theme Categories

| Category | Examples |
|----------|----------|
| `material(of:)` | Surface, Interactive, Primary, Danger |
| `font(of:)` | Hero, Body, Input, ButtonIcon |
| `depth(of:)` | Container, Element, Detail |
| `elevation(of:)` | Card, Inset, Popup |
| `corners(of:)` | Touch, Pill, Soft |
| `lights()` | Scene lighting setup |
| `geometry()` | Edge radius, bevel angles |

### Instant Theme Switching

**One variable change:**
```boon
theme_options: [name: Professional]  -- or Neobrutalism, etc.
```

**Everything updates:**
- Materials re-evaluate
- Lights change direction/intensity
- Geometry settings update
- Shadows/bevels automatically adjust

---

## 9. Implementation Phases

### Phase 1: SDF Raymarching Foundation ⏳
- [ ] Full-screen quad WebGPU pipeline
- [ ] Basic ray-march loop in WGSL
- [ ] Analytic SDF primitives (box, rounded_box, sphere)
- [ ] Boolean operations (union, subtract, smooth blend)
- [ ] Single directional light with hard shadows
- [ ] Basic diffuse shading

**Milestone:** Render a single rounded box with shadow

### Phase 2: Emergent Geometry ⏳
- [ ] Scene graph with element hierarchy
- [ ] Z-positioning (move_closer/move_further)
- [ ] Contact zone detection
- [ ] Smooth union for outward bevels
- [ ] Smooth subtract for inward fillets
- [ ] Global geometry settings

**Milestone:** Button raised above surface with automatic bevel

### Phase 3: Materials & Lighting ⏳
- [ ] PBR material system (roughness, metallic)
- [ ] Oklch color conversion
- [ ] Multiple light types (directional, ambient, point)
- [ ] Soft shadows via ray-marching
- [ ] Emissive materials (glow)

**Milestone:** Professional theme renders correctly

### Phase 4: Boon Integration ⏳
- [ ] Implement Scene/new() in api.rs
- [ ] Implement Light/* functions in api.rs
- [ ] Create raybox_bridge.rs
- [ ] Subscribe to Boon actor streams
- [ ] Reactive rendering (update on stream changes)

**Milestone:** todomvc_physical runs with Raybox

### Phase 5: Text Rendering ⏳
- [ ] MSDF atlas generation (msdfgen)
- [ ] MSDF text shader
- [ ] Text as 3D quads in scene
- [ ] Text depth/move properties
- [ ] Relief effects (carved, raised)

**Milestone:** All TodoMVC text renders crisply

### Phase 6: Polish & Performance ⏳
- [ ] Gradient-SDF for better normals
- [ ] ADF for complex scenes (if needed)
- [ ] Lᵖ-SDF for theme style control
- [ ] Anti-aliasing (temporal or MSAA)
- [ ] 60fps on integrated GPU

**Milestone:** Production-ready renderer

---

## 10. File Structure

```
raybox/
├── Cargo.toml                    # Workspace root
├── docs/
│   └── EMERGENT_RENDERER_MASTERPLAN.md  # THIS FILE
│
├── renderers/
│   ├── classic/                  # Traditional 2D renderer (complete)
│   │   └── src/
│   │       ├── lib.rs
│   │       └── ...
│   │
│   └── emergent/                 # 3D SDF renderer (in progress)
│       ├── Cargo.toml
│       ├── docs/
│       │   └── ARCHITECTURE.md   # Technical architecture
│       └── src/
│           ├── lib.rs            # Entry point, WebGPU init
│           ├── scene.rs          # Scene graph, element hierarchy
│           ├── sdf/
│           │   ├── mod.rs
│           │   ├── primitives.rs # SDF shapes
│           │   ├── operations.rs # Boolean ops
│           │   └── gradient.rs   # Gradient-SDF
│           ├── lighting/
│           │   ├── mod.rs
│           │   ├── lights.rs     # Light types
│           │   └── shadows.rs    # Shadow calculation
│           ├── materials/
│           │   ├── mod.rs
│           │   └── presets.rs    # Standard materials
│           ├── text/
│           │   ├── mod.rs
│           │   └── msdf.rs       # MSDF atlas
│           ├── shaders/
│           │   ├── raymarch.wgsl # Main shader
│           │   ├── sdf_lib.wgsl  # SDF library
│           │   └── lighting.wgsl # Lighting
│           └── pipeline.rs       # WebGPU setup
│
├── tools/                        # Build tools (complete)
│   └── src/
│       └── ...
│
└── web/                          # Web assets
    ├── index.html
    └── pkg/                      # Generated WASM
```

---

## 11. Integration Points

### Boon api.rs Additions

```rust
// Scene/new(root, lights, geometry)
pub fn function_scene_new(
    arguments: Arc<Vec<Arc<ValueActor>>>,
    function_call_id: ConstructId,
    function_call_persistence_id: PersistenceId,
    construct_context: ConstructContext,
    actor_context: ActorContext,
) -> impl Stream<Item = Value> {
    let root = arguments[0].clone();
    let lights = arguments[1].clone();
    let geometry = arguments[2].clone();

    TaggedObject::new_reactive(
        function_call_id,
        "Scene",
        [
            Variable::new_arc(..., "root_element", root),
            Variable::new_arc(..., "lights", lights),
            Variable::new_arc(..., "geometry", geometry),
        ]
    )
}

// Light/directional(azimuth, altitude, spread, intensity, color)
pub fn function_light_directional(...) -> impl Stream<Item = Value> {
    TaggedObject::new_reactive(
        ...,
        "LightDirectional",
        [...]
    )
}
```

### Boon bridge.rs (or raybox_bridge.rs)

```rust
pub fn object_with_scene_to_raybox(
    root_object: Arc<Object>,
    construct_context: ConstructContext,
) -> impl Signal<Item = Option<RayboxCommand>> {
    root_object
        .expect_variable("scene")
        .subscribe()
        .map(move |scene_value| {
            let scene = scene_value.expect_tagged_object();

            let root = scene.expect_variable("root_element");
            let lights = scene.expect_variable("lights");
            let geometry = scene.expect_variable("geometry");

            // Convert to Raybox commands
            Some(RayboxCommand {
                elements: element_tree_to_geometry(root),
                lights: lights_to_shader(lights),
                geometry_settings: geometry_to_settings(geometry),
            })
        })
}
```

### Raybox Render Command

```rust
struct RayboxCommand {
    elements: Vec<ElementGeometry>,
    lights: LightSetup,
    geometry_settings: GeometrySettings,
}

struct ElementGeometry {
    sdf_type: SDFType,
    position: Vec3,
    size: Vec3,
    corner_radius: f32,
    material: PBRMaterial,
    children: Vec<ElementGeometry>,
}

struct LightSetup {
    directional: Vec<DirectionalLight>,
    ambient: Vec<AmbientLight>,
    point: Vec<PointLight>,
}

struct GeometrySettings {
    edge_radius: f32,
    bevel_angle: f32,
}
```

---

## 12. Success Metrics

### Visual Quality
- [ ] Shadows emerge naturally from lighting
- [ ] Bevels emerge from spatial relationships
- [ ] Materials look physically plausible
- [ ] Text is razor-sharp at any zoom

### Code Simplicity
- [ ] 15x reduction in shadow/bevel configuration
- [ ] Theme switching is one line change
- [ ] No explicit geometry operations in user code

### Performance
- [ ] 60fps on integrated GPU
- [ ] <100ms initial render
- [ ] Incremental updates (not full re-render)

### Compatibility
- [ ] todomvc_physical renders identically to design
- [ ] All 4 themes work (Professional, Neobrutalism, etc.)
- [ ] Hot-reload preserves state

---

## 13. Open Questions

### Technical
1. **Camera model:** Orthographic vs perspective for UI?
2. **Coordinate system:** Screen-space vs world-space positioning?
3. **Text rendering:** MSDF in ray-march vs separate pass?
4. **Transparency:** How to handle glassmorphism?

### Design
1. **Bevel sharpness:** Fixed or per-element control?
2. **Animation:** Spring physics in shader vs CPU?
3. **Accessibility:** How to ensure contrast ratios?

### Integration
1. **Bridge location:** In Boon repo or Raybox repo?
2. **API stability:** Lock Scene/Light API before implementing?
3. **Testing:** How to regression test visual output?

---

## 14. References

### Boon Repository
- `/home/martinkavik/repos/boon/`
- `playground/frontend/src/examples/todo_mvc_physical/RUN.bn` - Main example
- `crates/boon/src/platform/browser/engine.rs` - Reactor core
- `crates/boon/src/platform/browser/api.rs` - Element APIs

### SDF Resources
- [Inigo Quilez SDF Functions](https://iquilezles.org/articles/distfunctions/)
- [MSDF Generator](https://github.com/Chlumsky/msdfgen)
- [Gradient-SDF Paper](https://openaccess.thecvf.com/content/CVPR2022/html/Sitzmann_Light_Field_Networks_CVPR_2022_paper.html)

### Raybox Documentation
- `renderers/emergent/docs/ARCHITECTURE.md` - Technical details
- `renderers/classic/` - Reference 2D implementation

---

## Quick Start for Next Session

1. **Read this document** to understand the vision
2. **Check current phase** in Implementation Phases section
3. **Start with Phase 1** tasks if not complete
4. **Reference ARCHITECTURE.md** for technical details
5. **Test with todomvc_physical** when Boon integration is ready

---

## Changelog

- **2024-11-28:** Initial master plan created
  - Comprehensive Boon research completed
  - SDF techniques integrated
  - Implementation phases defined
  - File structure planned
