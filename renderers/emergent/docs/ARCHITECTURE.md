# Emergent Renderer Architecture

## Core Philosophy

**Emergent rendering = geometry emerges from spatial relationships, not explicit declarations.**

Instead of declaring shadows, bevels, and borders explicitly, users describe:
- **Depth** (thickness of element)
- **Position** (Z-axis: `move_closer` / `move_further`)
- **Material** (surface properties)

The renderer then:
1. Generates 3D geometry using SDF boolean operations
2. Applies real lighting → shadows emerge naturally
3. Creates bevels/fillets at parent-child contact zones

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                      USER INPUT LAYER                           │
│  Element { depth, move, material, padding, children }           │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    SCENE GRAPH BUILDER                          │
│  - Build hierarchy of elements                                  │
│  - Calculate world positions (X, Y, Z)                          │
│  - Determine parent-child spatial relationships                 │
│  - Generate bevel/fillet zones at contact points                │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     SDF SCENE COMPILER                          │
│  - Convert elements to SDF primitives                           │
│  - Apply boolean operations (union, subtract, smooth_union)     │
│  - Generate Gradient-SDF data (distance + normals)              │
│  - Pack into GPU-friendly format                                │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      LIGHTING SYSTEM                            │
│  - Directional lights (sun-like, parallel shadows)              │
│  - Ambient lights (fill, no shadows)                            │
│  - Point lights (spotlights for focus effects)                  │
│  - Light configuration = theme aesthetic                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    RAYMARCHING RENDERER                         │
│  - Full-screen quad with ray-march shader                       │
│  - SDF scene evaluation per ray                                 │
│  - Shadow rays for real shadows                                 │
│  - Material shading (diffuse, specular, emissive)               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      TEXT RENDERING                             │
│  - MSDF atlas for crisp text at any scale                       │
│  - Future: FDF for gradient typography                          │
│  - Rendered as textured SDF quads in 3D space                   │
└─────────────────────────────────────────────────────────────────┘
```

---

## SDF Techniques Integration

### Phase 1: Basic Analytic SDF (Foundation)

**Primitives in WGSL:**
```wgsl
// Rounded box - the workhorse for UI elements
fn sd_rounded_box(p: vec3f, size: vec3f, radius: f32) -> f32 {
    let q = abs(p) - size + radius;
    return length(max(q, vec3f(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - radius;
}

// Boolean operations
fn op_union(d1: f32, d2: f32) -> f32 { return min(d1, d2); }
fn op_subtract(d1: f32, d2: f32) -> f32 { return max(d1, -d2); }
fn op_smooth_union(d1: f32, d2: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (d2 - d1) / k, 0.0, 1.0);
    return mix(d2, d1, h) - k * h * (1.0 - h);
}
```

**Use for:** Basic shapes, CSG operations, bevels via smooth unions.

### Phase 2: Gradient-SDF (Better Lighting)

**Store distance + gradient together:**
```rust
struct GradientSDF {
    distance: f32,
    gradient: Vec3,  // Pre-computed normal direction
}
```

**Benefits:**
- Instant normals without finite-difference calculation
- Better ray-marching step sizing (use gradient magnitude)
- Cleaner specular highlights
- Reduced banding artifacts

**Implementation:** Compute gradients analytically for primitives, propagate through boolean ops.

### Phase 3: MSDF Text (Crisp Typography)

**Approach:**
- Generate MSDF atlas at build time (using msdfgen or Rust port)
- Store RGB channels = 3 distance fields tracking different edges
- Render as textured quads positioned in 3D space
- Text inherits Z-position from parent element

**Benefits:**
- Razor-sharp text at any zoom level
- Tiny atlas textures (efficient memory)
- Text participates in 3D lighting (carved/embossed effects)

### Phase 4: Adaptive SDF (Performance)

**When needed:** If scenes grow complex (many nested elements), switch to ADF.

**Approach:**
- Octree storage of SDF values
- Dense sampling near surfaces, sparse elsewhere
- Dynamic LOD based on camera distance

### Phase 5: Lᵖ-SDF (Style Control)

**Theme lever for aesthetic control:**
```wgsl
// p = 2.0 (Euclidean - default rounded)
// p = ∞ (Chebyshev - boxy/industrial)
// p = 1.0 (Manhattan - diamond shapes)
fn sd_lp_box(pos: vec3f, size: vec3f, p: f32) -> f32 {
    let d = abs(pos) - size;
    // Lp norm calculation...
}
```

**Use for:**
- Neobrutalist theme: high p value → sharp/boxy
- Soft theme: p = 2.0 → rounded
- Art deco: mixed p values per element

---

## Emergent Geometry System

### The `move_closer` / `move_further` Model

**User writes:**
```
Element {
    depth: 8,
    move: [closer: 50],
    material: Surface,
}
```

**Renderer interprets:**
1. Element is 8px thick (Z-dimension)
2. Element floats 50px closer to camera than parent
3. At contact zone (where element meets parent's surface):
   - Outward bevel emerges via `smooth_union`
   - Bevel angle controlled by global geometry settings

**Contact Zone Detection:**
```rust
fn detect_contact_zone(parent: &Element, child: &Element) -> ContactType {
    let child_back = child.z - child.depth / 2.0;
    let parent_front = parent.z + parent.depth / 2.0;

    if child_back > parent_front {
        ContactType::Floating  // Gap between - no bevel
    } else if child_back == parent_front {
        ContactType::Touching  // Perfect contact - outward bevel
    } else {
        ContactType::Embedded  // Child sinks into parent - inward fillet
    }
}
```

### Bevel/Fillet Generation

**Outward bevel (button raised above surface):**
```wgsl
// Smooth union at contact creates natural bevel
let d = op_smooth_union(parent_sdf, child_sdf, bevel_radius);
```

**Inward fillet (input recessed into surface):**
```wgsl
// Subtract cavity from parent, smooth the edge
let cavity = sd_rounded_box(p - child_pos, child_size, corner_radius);
let d = op_smooth_subtract(parent_sdf, cavity, fillet_radius);
```

---

## Lighting System

### Light Types

```rust
enum Light {
    Directional {
        direction: Vec3,
        color: Vec3,
        intensity: f32,
    },
    Ambient {
        color: Vec3,
        intensity: f32,
    },
    Point {
        position: Vec3,
        color: Vec3,
        intensity: f32,
        falloff: f32,
    },
}
```

### Shadow Casting

**Only directional and point lights cast shadows.**

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

| Theme Style | Light Direction | Shadow Softness | Ambient Level |
|-------------|-----------------|-----------------|---------------|
| Professional | (0.5, 1.0, 0.5) | High (soft) | Medium |
| Dramatic | (1.0, 0.2, 0.0) | Low (sharp) | Low |
| Flat | N/A | N/A | High (shadows disabled) |

---

## Material System

```rust
struct Material {
    base_color: Vec3,
    roughness: f32,      // 0.0 = mirror, 1.0 = matte
    metallic: f32,       // 0.0 = dielectric, 1.0 = metal
    emissive: Vec3,      // Self-illumination (for focus glow)
    opacity: f32,        // For ghost/disabled states
}
```

### Material Presets

```rust
impl Material {
    fn surface() -> Self { /* matte white */ }
    fn interactive() -> Self { /* slight gloss */ }
    fn interactive_recessed() -> Self { /* glossy interior */ }
    fn ghost() -> Self { /* semi-transparent, pushed back */ }
    fn focus_glow() -> Self { /* emissive highlight */ }
}
```

---

## Implementation Phases

### Phase 1: SDF Raymarching Foundation
- [ ] Full-screen quad pipeline
- [ ] Basic ray-march loop in WGSL
- [ ] Analytic SDF primitives (box, rounded_box, sphere)
- [ ] Boolean operations (union, subtract, intersect)
- [ ] Basic diffuse lighting (single directional light)

### Phase 2: Emergent Geometry
- [ ] Scene graph with Z-positioning
- [ ] Contact zone detection
- [ ] Smooth union for outward bevels
- [ ] Smooth subtract for inward fillets
- [ ] Global geometry settings (edge_radius, bevel_angle)

### Phase 3: Advanced Lighting
- [ ] Multiple light types (directional, ambient, point)
- [ ] Soft shadow ray-marching
- [ ] Ambient occlusion (optional)
- [ ] Specular highlights (Blinn-Phong or PBR)

### Phase 4: Materials & Effects
- [ ] Material system with presets
- [ ] Emissive materials for focus states
- [ ] Ghost materials for disabled states
- [ ] Gradient-SDF for better normals

### Phase 5: Text Integration
- [ ] MSDF atlas generation pipeline
- [ ] MSDF text rendering shader
- [ ] Text as 3D quads in scene
- [ ] Text lighting (carved/embossed effects)

### Phase 6: Performance & Polish
- [ ] ADF for complex scenes (if needed)
- [ ] Lᵖ-SDF for style control
- [ ] FDF for gradient typography (future)
- [ ] Anti-aliasing (temporal or MSAA)

---

## File Structure

```
renderers/emergent/
├── Cargo.toml
├── docs/
│   └── ARCHITECTURE.md      # This file
└── src/
    ├── lib.rs               # Entry point, WebGPU init
    ├── scene.rs             # Scene graph, element hierarchy
    ├── sdf/
    │   ├── mod.rs           # SDF module
    │   ├── primitives.rs    # Basic shapes
    │   ├── operations.rs    # Boolean ops
    │   └── gradient.rs      # Gradient-SDF support
    ├── lighting/
    │   ├── mod.rs           # Lighting system
    │   ├── lights.rs        # Light types
    │   └── shadows.rs       # Shadow calculation
    ├── materials/
    │   ├── mod.rs           # Material system
    │   └── presets.rs       # Standard materials
    ├── text/
    │   ├── mod.rs           # Text rendering
    │   └── msdf.rs          # MSDF atlas handling
    ├── shaders/
    │   ├── raymarch.wgsl    # Main ray-march shader
    │   ├── sdf_lib.wgsl     # SDF primitives library
    │   └── lighting.wgsl    # Lighting calculations
    └── pipeline.rs          # WebGPU pipeline setup
```

---

## Success Metrics

1. **Visual Quality:** Shadows emerge naturally from lighting
2. **Code Simplicity:** 15x reduction in shadow/bevel configuration
3. **Theme Flexibility:** Change light direction → entire aesthetic changes
4. **Performance:** 60fps on integrated GPU for typical UI
5. **Text Quality:** Razor-sharp at any zoom (MSDF)
