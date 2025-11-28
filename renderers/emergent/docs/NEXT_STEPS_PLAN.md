# Emergent Renderer Next Steps Plan

## Overview

This plan outlines improvements to make the emergent renderer demo look more like the TodoMVC reference while leveraging the unique capabilities of SDF raymarching.

## Phase 1: Fix Visual Artifacts (Immediate)

### 1.1 Fix Triangular Shadow Artifacts

**Problem:** The main card shows triangular shadow artifacts at the edges due to raymarching rays hitting the background plane at oblique angles.

**Solution Options:**
- **Option A (Recommended):** Increase background plane size to 2x viewport dimensions
  - Change `[400.0, 400.0, 2.0]` to `[700.0, 700.0, 2.0]` in `create_demo_scene()`
- **Option B:** Add viewport clipping in the shader
  - Discard fragments outside the canvas bounds
- **Option C:** Move background further back and make it larger
  - Position at z=-50 with size [1000, 1000, 10]

**Files to modify:**
- `renderers/emergent/src/lib.rs` - `create_demo_scene()` function

### 1.2 Improve Hollow Checkbox Styling

**Problem:** Empty checkboxes are currently solid circles; should be hollow rings.

**Solution:** Add a new shape type `HollowCircle` or render as two concentric circles (outer filled, inner matching background).

**Approach:**
1. Add `Torus2D` or `Ring` shape to `scene.rs`
2. Implement ring SDF in shader: `length(p.xy) - outer_radius` with inner cutout
3. Use for unchecked checkboxes

**Files to modify:**
- `renderers/emergent/src/scene.rs` - Add new shape type
- `renderers/emergent/src/shaders/raymarch.wgsl` - Add ring SDF

## Phase 2: Text Rendering (High Impact)

### 2.1 Hybrid Canvas2D + WebGPU Approach

**Rationale:** Canvas2D provides excellent text rendering with system fonts. We can render text to textures and composite them over the raymarched scene.

**Implementation:**
1. Create `text_renderer.rs` module (similar to classic renderer)
2. Create offscreen Canvas2D for text rendering
3. Render text to RGBA bitmap
4. Create WebGPU textures from bitmaps
5. Add second render pass to composite text over SDF scene

**Text elements needed:**
- "todos" title (80px, #b83f45, centered)
- "What needs to be done?" placeholder (24px, #e6e6e6)
- Todo item labels (24px, #484848)
- "X items left" (14px, #777777)
- Filter buttons: "All", "Active", "Completed" (14px)

**Files to create:**
- `renderers/emergent/src/text_renderer.rs` - Text bitmap generation
- `renderers/emergent/src/text_pipeline.rs` - WebGPU texture rendering

**Files to modify:**
- `renderers/emergent/src/lib.rs` - Add text rendering pass
- `renderers/emergent/Cargo.toml` - Ensure web-sys features for Canvas2D

### 2.2 Alternative: SDF Font Rendering (Future)

For truly "emergent" text, could implement SDF fonts:
- Generate SDF glyph atlas at build time
- Sample glyph SDFs in shader
- Smooth text at any scale

This is more complex but aligns with the emergent philosophy.

## Phase 3: Precise Positioning (Medium Impact)

### 3.1 Use layout.json Reference Data

**Current state:** Element positions are hardcoded approximations.

**Improvement:** Parse layout.json and generate exact positions.

**Key positions from layout.json (700x700 viewport):**
```
Body: x=75, y=130, w=550
h1 "todos": x=75, y=43.6, w=550, font=80px
Input: x=75, y=130, w=550, h=65
Todo list: x=75, y=196
  Item 1: y=196, checkbox at (75, 205.4)
  Item 2: y=256, checkbox at (75, 265.2)
  Item 3: y=316, checkbox at (75, 325)
  Item 4: y=376, checkbox at (75, 384.8)
Footer: x=75, y=434, h=41
```

**Approach:**
1. Create `LayoutData` struct matching layout.json schema
2. Either embed layout data or fetch from JSON
3. Generate Scene elements from LayoutData

**Files to modify:**
- `renderers/emergent/src/lib.rs` - Replace hardcoded positions
- Optional: `renderers/emergent/src/layout.rs` - Layout data types

## Phase 4: Visual Polish (Lower Priority)

### 4.1 Box Shadow Effect

TodoMVC has subtle box-shadow: `0 2px 4px rgba(0,0,0,0.2), 0 25px 50px rgba(0,0,0,0.1)`

**Options:**
- **Emergent approach:** Let raymarching naturally create shadows (current)
- **Explicit shadow:** Add dark semi-transparent box behind main card
- **Post-process:** Add gaussian blur shadow layer

### 4.2 Strikethrough for Completed Items

When rendering text for completed items, draw a line through the text.
- Text color changes to #d9d9d9
- Line-through decoration

### 4.3 Input Field Inset Shadow

Input has `box-shadow: inset 0 -2px 1px rgba(0,0,0,0.03)`
- Subtle darkening at bottom edge of input field

## Implementation Order

1. **Fix shadow artifacts** (30 min) - Immediate visual improvement
2. **Hollow checkboxes** (1 hour) - Better checkbox appearance
3. **Basic text rendering** (2-3 hours) - Add "todos" title and labels
4. **Precise positioning** (1 hour) - Match layout.json exactly
5. **Visual polish** (ongoing) - Box shadows, strikethrough, etc.

## Success Criteria

- [ ] No triangular shadow artifacts at card edges
- [ ] Checkboxes appear as hollow circles (unchecked) or filled green (checked)
- [ ] "todos" title visible in correct position and color
- [ ] Todo item labels visible
- [ ] Footer text visible
- [ ] Element positions match layout.json within 5px tolerance

## Architecture Notes

### Render Pass Order
1. **Pass 1 (Raymarch):** Render SDF scene to framebuffer
2. **Pass 2 (Text Composite):** Render text textures with alpha blending

### File Structure After Changes
```
renderers/emergent/src/
├── lib.rs              # Entry point, scene setup
├── pipeline.rs         # SDF raymarch pipeline
├── scene.rs            # Element definitions
├── text_renderer.rs    # NEW: Canvas2D text rendering
├── text_pipeline.rs    # NEW: Text texture compositing
└── shaders/
    ├── raymarch.wgsl   # SDF raymarching shader
    └── text.wgsl       # NEW: Text compositing shader
```
