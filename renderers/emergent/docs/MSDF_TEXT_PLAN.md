# SDF Text Rendering Library Plan

## Overview

This document outlines the plan for implementing a proper SDF-based text rendering library for the emergent renderer. This replaces the current ad-hoc procedural SDF letter generation with high-quality, font-based text rendering.

## Final Decisions

| Decision | Choice | Reason |
|----------|--------|--------|
| **Atlas generation** | `msdf` Rust crate | Only Rust binding with MTSDF support |
| **Font** | Inter | Open source (SIL OFL), Helvetica-like appearance |
| **Crate structure** | Embedded in emergent | Simpler start, extract later if needed |
| **SDF type** | MTSDF (4-channel) | RGB for sharp corners, Alpha for effects |

**Build requirement:** `msdf` crate needs Clang: `sudo apt install clang libclang-dev`

## SDF Variant Comparison

| Type | Channels | Sharp Corners | Effects Support | Use Case |
|------|----------|---------------|-----------------|----------|
| **SDF** | 1 (gray) | No (rounded) | Basic | Simple text, maps |
| **MSDF** | 3 (RGB) | Yes | Limited | Crisp UI text |
| **MTSDF** | 4 (RGBA) | Yes | Full (outline, shadow, glow) | Styled UI text |

### Why MTSDF?

**MTSDF = Multi-channel True Signed Distance Field**

- **RGB channels:** Multi-channel data for sharp corners (same as MSDF)
- **Alpha channel:** True signed distance field for effects

The alpha channel enables:
- **Outlines/strokes** - Variable width borders around text
- **Drop shadows** - Soft shadows with controllable blur
- **Glow effects** - Bloom/neon effects
- **Material application** - Use true distance for shading

**Decision:** Use **MTSDF** for maximum flexibility in text styling.

## Current Problems with Ad-hoc SDF Text

- Each letter requires manual SDF construction from primitives
- Poor visual quality at various scales
- No proper typography (kerning, metrics, baseline alignment)
- Extremely time-consuming to add new characters
- Inconsistent stroke weights and proportions

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Build Time (tools crate)                     │
├─────────────────────────────────────────────────────────────────────┤
│  TTF/OTF Font ──► msdf-atlas-gen ──► MTSDF Atlas PNG + JSON         │
│                                                                     │
│  Command: msdf-atlas-gen -font assets/fonts/HelveticaNeue-Light.ttf │
│           -type mtsdf -format png -json helvetica.json              │
│           -charset charset.txt -o helvetica.png                     │
│                                                                     │
│  Or Rust: cargo run -p tools -- generate-mtsdf-atlas                │
│           --font assets/fonts/HelveticaNeue-Light.ttf               │
│           --output renderers/emergent/assets/fonts/                 │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     Runtime (WASM renderer)                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────┐   ┌─────────────────┐   ┌──────────────────┐ │
│  │   Font Atlas     │   │   Font Metrics  │   │   Text Layout    │ │
│  │ (RGBA texture)   │   │   (JSON data)   │   │   Engine         │ │
│  └────────┬─────────┘   └────────┬────────┘   └────────┬─────────┘ │
│           │                      │                      │          │
│           └──────────────────────┴──────────────────────┘          │
│                                  │                                  │
│                                  ▼                                  │
│                    ┌─────────────────────────┐                     │
│                    │   MTSDF Text Renderer   │                     │
│                    │   (mtsdf_text.wgsl)     │                     │
│                    └─────────────────────────┘                     │
│                                  │                                  │
│                                  ▼                                  │
│                    ┌─────────────────────────┐                     │
│                    │    Glyph Instance       │                     │
│                    │    GPU Buffer           │                     │
│                    └─────────────────────────┘                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Implementation Phases

### Phase 1: Atlas Generation (Pre-built)

**Recommended approach:** Use `msdf-atlas-gen` directly for initial implementation.

```bash
# Install msdf-atlas-gen (C++ tool)
# https://github.com/Chlumsky/msdf-atlas-gen

# Generate MTSDF atlas
msdf-atlas-gen \
  -font assets/fonts/HelveticaNeue-Light.ttf \
  -type mtsdf \
  -format png \
  -size 48 \
  -pxrange 6 \
  -charset charset.txt \
  -json helvetica-neue-light.json \
  -imageout helvetica-neue-light.png
```

**JSON Output Format:**
```json
{
  "atlas": {
    "type": "mtsdf",
    "distanceRange": 6,
    "size": 48,
    "width": 512,
    "height": 512
  },
  "metrics": {
    "emSize": 1,
    "lineHeight": 1.2,
    "ascender": 0.88,
    "descender": -0.12,
    "underlineY": -0.15,
    "underlineThickness": 0.05
  },
  "glyphs": [
    {
      "unicode": 65,
      "advance": 0.72,
      "planeBounds": { "left": 0.01, "bottom": -0.01, "right": 0.71, "top": 0.73 },
      "atlasBounds": { "left": 0, "bottom": 0, "right": 48, "top": 48 }
    }
  ],
  "kerning": [
    { "unicode1": 65, "unicode2": 86, "advance": -0.08 }
  ]
}
```

### Phase 2: Runtime Font Loading

**Location:** `renderers/emergent/src/font.rs`

**Data Structures:**
```rust
/// Loaded MTSDF font ready for rendering
pub struct MtsdfFont {
    /// GPU texture containing the atlas (RGBA)
    pub atlas_texture: wgpu::Texture,
    pub atlas_view: wgpu::TextureView,
    pub atlas_sampler: wgpu::Sampler,

    /// Font metrics
    pub metrics: FontMetrics,

    /// Glyph lookup (unicode -> glyph info)
    pub glyphs: HashMap<u32, GlyphInfo>,

    /// Kerning pairs lookup
    pub kerning: HashMap<(u32, u32), f32>,

    /// Distance range (for shader)
    pub distance_range: f32,
}

pub struct FontMetrics {
    pub em_size: f32,
    pub line_height: f32,
    pub ascender: f32,
    pub descender: f32,
}

pub struct GlyphInfo {
    pub advance: f32,
    pub plane_bounds: Rect,   // In em units
    pub atlas_bounds: Rect,   // In pixels
}
```

### Phase 3: Text Layout Engine

**Location:** `renderers/emergent/src/text_layout.rs`

```rust
pub struct TextLayout {
    font: Arc<MtsdfFont>,
}

impl TextLayout {
    pub fn layout(&self, text: &str, options: &LayoutOptions) -> Vec<PositionedGlyph> {
        // 1. Convert string to unicode codepoints
        // 2. For each codepoint, look up glyph
        // 3. Apply kerning between pairs
        // 4. Accumulate horizontal advance
        // 5. Handle line breaks if max_width specified
        // 6. Return positioned glyphs
    }
}

pub struct LayoutOptions {
    pub font_size: f32,
    pub line_height: Option<f32>,
    pub max_width: Option<f32>,
    pub alignment: TextAlign,
}

pub struct PositionedGlyph {
    pub unicode: u32,
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub atlas_uv: Rect,
}
```

### Phase 4: WGSL Shader (MTSDF)

**Location:** `renderers/emergent/src/shaders/mtsdf_text.wgsl`

```wgsl
struct VertexInput {
    @builtin(vertex_index) vertex_idx: u32,
    @builtin(instance_index) instance_idx: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct GlyphInstance {
    position: vec2<f32>,
    size: vec2<f32>,
    uv_min: vec2<f32>,
    uv_max: vec2<f32>,
    color: vec4<f32>,
    // Effect parameters
    outline_color: vec4<f32>,
    outline_width: f32,
    shadow_offset: vec2<f32>,
    shadow_softness: f32,
}

struct TextUniforms {
    screen_size: vec2<f32>,
    distance_range: f32,
    _padding: f32,
}

@group(0) @binding(0) var mtsdf_texture: texture_2d<f32>;
@group(0) @binding(1) var mtsdf_sampler: sampler;
@group(0) @binding(2) var<storage, read> glyphs: array<GlyphInstance>;
@group(0) @binding(3) var<uniform> uniforms: TextUniforms;

const QUAD_POS = array<vec2<f32>, 4>(
    vec2<f32>(0.0, 0.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 1.0),
);

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let glyph = glyphs[input.instance_idx];
    let local_pos = QUAD_POS[input.vertex_idx];

    let pixel_pos = glyph.position + local_pos * glyph.size;
    let ndc = (pixel_pos / uniforms.screen_size) * 2.0 - 1.0;

    var output: VertexOutput;
    output.position = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);
    output.tex_coord = mix(glyph.uv_min, glyph.uv_max, local_pos);
    output.color = glyph.color;
    return output;
}

// MSDF median function (for RGB channels)
fn median(r: f32, g: f32, b: f32) -> f32 {
    return max(min(r, g), min(max(r, g), b));
}

// Screen pixel range calculation for anti-aliasing
fn screen_px_range(tex_coord: vec2<f32>) -> f32 {
    let unit_range = vec2<f32>(uniforms.distance_range) / vec2<f32>(textureDimensions(mtsdf_texture));
    let screen_tex_size = vec2<f32>(1.0) / fwidth(tex_coord);
    return max(0.5 * dot(unit_range, screen_tex_size), 1.0);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let mtsdf = textureSample(mtsdf_texture, mtsdf_sampler, input.tex_coord);

    // RGB = MSDF for sharp corners
    let msdf_dist = median(mtsdf.r, mtsdf.g, mtsdf.b);

    // Alpha = True SDF for effects
    let true_sdf = mtsdf.a;

    let px_range = screen_px_range(input.tex_coord);

    // Main text shape (using MSDF for sharpness)
    let msdf_px_dist = px_range * (msdf_dist - 0.5);
    let text_alpha = clamp(msdf_px_dist + 0.5, 0.0, 1.0);

    // Optional: Outline using true SDF
    // let outline_dist = px_range * (true_sdf - 0.5 + outline_width);
    // let outline_alpha = clamp(outline_dist + 0.5, 0.0, 1.0) - text_alpha;

    if (text_alpha < 0.001) {
        discard;
    }

    return vec4<f32>(input.color.rgb, input.color.a * text_alpha);
}

// Extended fragment shader with effects
@fragment
fn fs_main_with_effects(input: VertexOutput) -> @location(0) vec4<f32> {
    let glyph = glyphs[0]; // Would need instance index passed
    let mtsdf = textureSample(mtsdf_texture, mtsdf_sampler, input.tex_coord);

    let msdf_dist = median(mtsdf.r, mtsdf.g, mtsdf.b);
    let true_sdf = mtsdf.a;
    let px_range = screen_px_range(input.tex_coord);

    // Text fill
    let text_px_dist = px_range * (msdf_dist - 0.5);
    let text_alpha = clamp(text_px_dist + 0.5, 0.0, 1.0);

    // Outline (using true SDF for smooth distance)
    let outline_outer = px_range * (true_sdf - 0.5 + glyph.outline_width);
    let outline_alpha = clamp(outline_outer + 0.5, 0.0, 1.0) - text_alpha;

    // Shadow (sample with offset, use true SDF)
    let shadow_uv = input.tex_coord + glyph.shadow_offset;
    let shadow_mtsdf = textureSample(mtsdf_texture, mtsdf_sampler, shadow_uv);
    let shadow_sdf = shadow_mtsdf.a;
    let shadow_dist = px_range * (shadow_sdf - 0.5);
    let shadow_alpha = smoothstep(-glyph.shadow_softness, glyph.shadow_softness, shadow_dist);

    // Composite: shadow -> outline -> fill
    var result = vec4<f32>(0.0, 0.0, 0.0, shadow_alpha * 0.5); // Shadow
    result = mix(result, glyph.outline_color, outline_alpha);   // Outline
    result = mix(result, input.color, text_alpha);              // Fill

    return result;
}
```

### Phase 5: Integration with Scene

**Location:** Update `renderers/emergent/src/scene.rs`

```rust
pub struct TextElement {
    pub text: String,
    pub position: [f32; 3],
    pub font_size: f32,
    pub color: [f32; 4],
    pub alignment: TextAlign,
    // Effects (using true SDF from alpha channel)
    pub outline: Option<TextOutline>,
    pub shadow: Option<TextShadow>,
}

pub struct TextOutline {
    pub color: [f32; 4],
    pub width: f32,  // In pixels
}

pub struct TextShadow {
    pub color: [f32; 4],
    pub offset: [f32; 2],
    pub softness: f32,
}

impl Scene {
    pub fn add_text(&mut self, element: TextElement) {
        self.text_elements.push(element);
    }
}
```

---

## Exotic SDF Variants for Future Consideration

### Gradient-SDF (for 3D elements)

**When to use:** For 3D SDF shapes in `raymarch.wgsl` that need proper lighting.

**What it does:** Stores both distance AND gradient (normal) per sample point.

**Current approach (finite differences):**
```wgsl
fn calculate_normal(p: vec3<f32>) -> vec3<f32> {
    let e = vec2<f32>(0.001, 0.0);
    return normalize(vec3<f32>(
        scene_sdf(p + e.xyy) - scene_sdf(p - e.xyy),
        scene_sdf(p + e.yxy) - scene_sdf(p - e.yxy),
        scene_sdf(p + e.yyx) - scene_sdf(p - e.yyx)
    ));
}
```

**Gradient-SDF approach:**
```wgsl
struct GradientSdfSample {
    distance: f32,
    gradient: vec3<f32>,  // Pre-computed normal
}

// No finite differences needed - gradient is stored
fn get_normal(sample: GradientSdfSample) -> vec3<f32> {
    return normalize(sample.gradient);
}
```

**Benefits:**
- No banding artifacts in normals
- Faster (no 6 extra SDF evaluations)
- Better specular highlights
- Smoother shading on curved surfaces

**Implementation note:** For procedural SDFs (our current approach), gradients can be computed analytically. For stored SDFs (voxel grids, textures), pre-compute and store gradients.

**Reference:** [Gradient-SDF CVPR 2022](https://github.com/c-sommer/gradient-sdf)

---

### Adaptively Sampled Distance Fields (ADF)

**When to use:** Large/complex 2D or 3D scenes with varying detail levels.

**What it does:** Stores SDF in quadtree (2D) or octree (3D), sampling densely only near features.

**Benefits:**
- 10-20x memory reduction for complex shapes
- Automatic LOD
- Exact boolean operations

**When NOT needed:** Our current TodoMVC UI is simple enough that uniform sampling is fine.

**Reference:** [Frisken et al. SIGGRAPH 2000](https://graphics.stanford.edu/courses/cs468-03-fall/Papers/frisken00adaptively.pdf)

---

### Foliated Distance Fields (FDF)

**When to use:** Typography with gradient fills that follow glyph shapes.

**What it does:** Organizes SDF into "leaves" (layers) that follow geometry, enabling gradients to flow naturally along letter shapes.

**Status:** Research/experimental. Presented at BSC 2025, no public tooling yet.

**Future potential:** For branded UI with gradient-filled text, FDF would be the ideal solution.

---

### Lp-Distance Fields

**When to use:** Stylized visual effects.

**What it does:** Replaces Euclidean distance (L2) with other metrics:
- L1 (Manhattan): Diamond-shaped falloff
- L-infinity (Chebyshev): Boxy/square falloff

**Implementation (shader trick):**
```wgsl
// Standard Euclidean (L2)
fn sdf_circle_l2(p: vec2<f32>, r: f32) -> f32 {
    return length(p) - r;
}

// Manhattan (L1) - diamond shape
fn sdf_diamond_l1(p: vec2<f32>, r: f32) -> f32 {
    return (abs(p.x) + abs(p.y)) - r;
}

// Chebyshev (L-inf) - square shape
fn sdf_square_linf(p: vec2<f32>, r: f32) -> f32 {
    return max(abs(p.x), abs(p.y)) - r;
}
```

**Use case:** Art direction for stylized UI effects (boxy halos, diamond buttons).

---

## File Structure

```
renderers/emergent/
├── assets/
│   └── fonts/
│       ├── helvetica-neue-light.png      # MTSDF atlas (RGBA)
│       └── helvetica-neue-light.json     # Glyph metrics
├── src/
│   ├── lib.rs
│   ├── scene.rs
│   ├── pipeline.rs
│   ├── font.rs                           # Font loading
│   ├── text_layout.rs                    # Text layout
│   ├── mtsdf_renderer.rs                 # MTSDF GPU rendering
│   └── shaders/
│       ├── raymarch.wgsl                 # 3D SDF (consider Gradient-SDF)
│       └── mtsdf_text.wgsl               # MTSDF text shader
└── docs/
    └── MSDF_TEXT_PLAN.md                 # This document

tools/
├── src/
│   └── commands/
│       └── generate_mtsdf_atlas.rs       # Optional: Rust atlas generation
```

## Dependencies

### Build-time (if implementing Rust atlas generation)
```toml
[dependencies]
msdfgen = { version = "0.2", features = ["ttf-parse", "png"] }
ttf-parser = "0.21"
rectangle-pack = "0.4"
image = "0.25"
```

### Runtime (renderer crate)
```toml
[dependencies]
# No new dependencies - uses existing wgpu, serde_json
```

## Migration Path

1. **Phase 1:** Generate MTSDF atlas using `msdf-atlas-gen` tool
2. **Phase 2:** Implement font loading and basic text rendering
3. **Phase 3:** Add effect support (outline, shadow) using alpha channel
4. **Phase 4:** Replace ad-hoc SDF letters in "todos" title
5. **Phase 5:** Replace all text elements (todo items, footer, etc.)
6. **Phase 6:** Remove old procedural letter SDFs from raymarch.wgsl
7. **Phase 7 (optional):** Add Gradient-SDF to 3D raymarch for better normals

## Performance Considerations

- **Instancing:** Single draw call for all glyphs via GPU instancing
- **Batching:** Group text by font to minimize texture switches
- **Atlas size:** 512x512 or 1024x1024 sufficient for ASCII + common symbols
- **Distance range:** 6px provides good anti-aliasing and room for effects
- **RGBA format:** 4 bytes per pixel (vs 3 for MSDF) - negligible overhead

## Testing Strategy

1. **Visual comparison:** Screenshot test against reference TodoMVC
2. **Glyph coverage:** Ensure all required characters render correctly
3. **Scale testing:** Verify sharpness at 0.5x, 1x, 2x, 4x scale
4. **Effects testing:** Verify outline, shadow render correctly
5. **Performance:** Measure frame time with 1000+ glyphs

## References

### Core MTSDF
- [Chlumsky/msdfgen](https://github.com/Chlumsky/msdfgen) - Original MSDF/MTSDF implementation
- [msdf-atlas-gen](https://github.com/Chlumsky/msdf-atlas-gen) - Atlas generator with MTSDF support
- [WebGPU MSDF Sample](https://webgpu.github.io/webgpu-samples/?sample=textRenderingMsdf)
- [awesome-msdf](https://github.com/Blatko1/awesome-msdf) - Shader collection

### Gradient-SDF
- [Gradient-SDF CVPR 2022](https://openaccess.thecvf.com/content/CVPR2022/html/Sommer_Gradient-SDF_A_Semi-Implicit_Surface_Representation_for_3D_Reconstruction_CVPR_2022_paper.html)
- [GitHub: c-sommer/gradient-sdf](https://github.com/c-sommer/gradient-sdf)

### ADF
- [Frisken et al. SIGGRAPH 2000](https://graphics.stanford.edu/courses/cs468-03-fall/Papers/frisken00adaptively.pdf)

### Alternative: Direct Bezier Rendering
- [Slug Library](https://sluglibrary.com/) - Commercial, GPU Bezier rendering
- [AMD Mesh Shaders](https://gpuopen.com/learn/mesh_shaders/mesh_shaders-font_and_vector_art_rendering_with_mesh_shaders/)
- [Will Dobbie](https://wdobbie.com/post/gpu-text-rendering-with-vector-textures/)

### Rust Crates
- [msdfgen](https://docs.rs/msdfgen) - High-level Rust bindings
- [msdf](https://docs.rs/msdf) - Alternative bindings
- [msdfont](https://github.com/Blatko1/msdfont) - Pure Rust (WIP)
