# Canvas TodoMVC - Chrome-First WebGPU Renderer

## Project Vision

Build a WebGPU-based renderer that can perfectly render the TodoMVC UI in Chrome, proving the viability of our SDF rendering approach before expanding to headless/native/3D features.

**Core Philosophy**: Chrome DevTools first, visual feedback first, then expand.

---

## V1 Success Criteria

**"Working" means**: TodoMVC renders in Chrome with:
- ✅ **Correct element positioning** (using Chrome's pre-computed layout positions)
- ✅ **Text content visible and readable** (all labels, placeholders, footer text)
- ✅ **Proper text alignment and sizing** (font sizes, line heights match)
- ✅ **Colors** (implemented layer-by-layer: rectangles → text → colors)
- ❌ Shadows, gradients (V2)
- ❌ Interactive states, hover effects (V3)
- ❌ Full interactivity (add/edit/delete todos) (V4)
- ❌ Responsive layout (V5)

**Why this scope?**
- Using Chrome's layout data (`reference/todomvc_dom_layout.json`) - no layout engine needed
- Layer-by-layer implementation for easier debugging
- Text rendering proves our hybrid approach works
- Visual validation in Chrome gives immediate feedback
- Everything else builds on this foundation

---

## Reference Materials

### TodoMVC Source
- **HTML/CSS/JS**: `./reference/` (copied from tastejs/todomvc javascript-es6 example)
- **Live reference**: https://todomvc.com/examples/vanilla-es6/ (for comparison)
- **Reference screenshot**: `./reference/todomvc_chrome_reference.png`
  - Captured from Chrome 141.0.7390.122
  - Resolution: 1920×1080, DPR=1
  - Populated state: 4 todos (3 active, 1 completed)
  - See `./reference/REFERENCE_METADATA.md` for full details

### Key Reference Values (Extracted from CSS)

#### Layout
```css
body {
  max-width: 550px;
  margin: 0 auto;
  background: #f5f5f5;
}

.todoapp {
  background: #fff;
  margin: 130px 0 40px 0;
  box-shadow: 0 2px 4px 0 rgba(0,0,0,.2), 0 25px 50px 0 rgba(0,0,0,.1);
}

.todoapp h1 {
  font-size: 80px;
  font-weight: 200;
  color: #b83f45;
  position: absolute;
  top: -140px;
  width: 100%;
  text-align: center;
}

.new-todo {
  padding: 16px 16px 16px 60px;
  height: 65px;
  font-size: 24px;
}

.todo-list li {
  font-size: 24px;
  padding: 15px 15px 15px 60px;
}

.footer {
  font-size: 15px;
  padding: 10px 15px;
  height: 20px;
}
```

#### Typography
```css
font-family: Helvetica Neue, Helvetica, Arial, sans-serif;
font-weight: 300;
line-height: 1.4em;
```

#### Colors (V2, for reference)
- Background: `#f5f5f5`
- TodoApp background: `#fff`
- Header "todos": `#b83f45`
- Text: `#484848`
- Placeholder: `rgba(0,0,0,.4)`
- Borders: `#e6e6e6`, `#ededed`
- Completed text: `#949494`

---

## Architecture

### High-Level Stack
```
┌─────────────────────────────────────┐
│   Chrome Browser (DevTools ready)   │
├─────────────────────────────────────┤
│   index.html + demo.js              │
│   (Host, handles setup & events)    │
├─────────────────────────────────────┤
│   Renderer (Wasm + WebGPU)          │
│   - Canvas element owner            │
│   - WebGPU device/pipelines         │
│   - Layout engine                   │
│   - Text renderer (Canvas2D hybrid) │
└─────────────────────────────────────┘
```

### Crate Structure
```
raybox/
├── crates/
│   ├── renderer/          # Core WebGPU renderer (wasm32 target)
│   │   ├── src/
│   │   │   ├── lib.rs     # Wasm bindings, public API
│   │   │   ├── gpu.rs     # WebGPU device, surface, pipelines
│   │   │   ├── layout.rs  # Flexbox-like layout engine
│   │   │   ├── text.rs    # Text rendering via Canvas2D hybrid
│   │   │   └── primitives.rs  # Basic shapes (quad, rounded rect)
│   │   └── Cargo.toml
│   │
│   ├── layout/            # Shared layout logic (no_std compatible)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── node.rs    # Layout tree nodes
│   │   │   ├── style.rs   # Style properties (flex, padding, etc.)
│   │   │   └── solver.rs  # Layout computation
│   │   └── Cargo.toml
│   │
│   └── tools/             # Dev tools (Rust-only, native target)
│       ├── src/
│       │   ├── main.rs    # CLI entry point
│       │   ├── serve.rs   # HTTP server (miniserve wrapper or embedded)
│       │   ├── chrome.rs  # Chrome DevTools Protocol client
│       │   ├── screenshot.rs  # Screenshot capture via CDP
│       │   └── compare.rs # Visual comparison utilities
│       └── Cargo.toml
│
├── web/                   # Web demo
│   ├── index.html
│   ├── demo.js
│   └── styles.css
│
├── reference/             # TodoMVC reference materials
├── dist/                  # Build output (gitignored)
├── Cargo.toml            # Workspace root
├── Justfile              # Build commands
└── specs.md              # This file
```

### Wasm Public API (renderer crate)
```rust
#[wasm_bindgen]
pub struct TodoRenderer {
    // internal state
}

#[wasm_bindgen]
impl TodoRenderer {
    /// Create new renderer attached to canvas element
    #[wasm_bindgen(constructor)]
    pub async fn new(canvas_id: &str) -> Result<TodoRenderer, JsValue>;

    /// Set the TodoMVC state to render (static for V1)
    pub fn set_state(&mut self, state_json: &str) -> Result<(), JsValue>;

    /// Render a single frame
    pub fn render(&mut self) -> Result<(), JsValue>;

    /// Resize canvas (call from JS on window resize)
    pub fn resize(&mut self, width: u32, height: u32);
}
```

---

## Text Rendering Strategy (V1)

**Approach**: Canvas2D Hybrid
- Use offscreen `<canvas>` with 2D context for text rasterization
- Measure text bounds using `measureText()`
- Render text to canvas, upload to WebGPU texture
- Draw textured quads in WebGPU

**Why this approach?**
1. ✅ Pragmatic - leverages browser's font rendering
2. ✅ Fast to implement - no MSDF pipeline needed
3. ✅ Proven technique (many engines use this)
4. ✅ Supports system fonts automatically
5. ❌ Limited quality at small sizes (acceptable for V1)

**Implementation steps**:
1. Create offscreen canvas in JS, pass to Wasm
2. For each text element:
   - Measure bounds: `ctx.measureText(text)`
   - Draw: `ctx.fillText(text, x, y)`
   - Copy pixel data to Wasm memory
   - Upload to WebGPU texture
   - Draw textured quad at computed layout position

**Alternative considered**: MSDF atlas (V2+)
- Better quality, especially for UI
- Requires build-time atlas generation
- More complex pipeline
- Defer until layout + basic text works

---

## Layout Engine Design

### Goals
- Flexbox-like behavior matching TodoMVC CSS
- No external dependencies (pure Rust)
- Deterministic, testable
- Outputs absolute positions for rendering

### Layout Tree Nodes
```rust
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub style: Style,
    pub children: Vec<NodeId>,
    pub computed: ComputedLayout,  // Output after layout pass
}

pub enum NodeKind {
    Container,      // Generic flex container
    Text(String),   // Text content
    Input { placeholder: String },
    // ... more types as needed
}

pub struct Style {
    // Flexbox
    pub display: Display,           // Flex | Block
    pub flex_direction: FlexDirection,  // Row | Column
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub gap: f32,

    // Box model
    pub padding: Padding,
    pub margin: Margin,
    pub width: Dimension,   // Auto | Px(f32) | Percent(f32)
    pub height: Dimension,
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,

    // Typography
    pub font_size: f32,
    pub font_weight: FontWeight,
    pub line_height: f32,
    pub text_align: TextAlign,
}

pub struct ComputedLayout {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
```

### Layout Algorithm
1. **Measure pass** (bottom-up)
   - Measure text intrinsic sizes
   - Compute content sizes
2. **Layout pass** (top-down)
   - Resolve flex layouts
   - Compute absolute positions
3. **Output**: `Vec<RenderPrimitive>` ready for GPU

---

## WebGPU Rendering Pipeline (V1)

### Render Primitives
```rust
pub enum RenderPrimitive {
    Rect {
        x: f32, y: f32,
        width: f32, height: f32,
        color: [f32; 4],  // V2
    },
    TexturedQuad {
        x: f32, y: f32,
        width: f32, height: f32,
        texture_id: TextureId,
        uv_rect: [f32; 4],  // (u0, v0, u1, v1)
    },
    // V2: RoundedRect, borders, shadows
}
```

### GPU Pipeline
- **Single render pass** (V1)
- **Vertex shader**: Transform 2D positions to clip space
- **Fragment shader**:
  - Solid color for rects
  - Texture sampling for text
- **Coordinate system**:
  - Input: CSS pixels (0,0 = top-left)
  - Output: NDC (-1,-1 = bottom-left, 1,1 = top-right)

### Shaders
```wgsl
// Vertex shader
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var<uniform> screen_size: vec2<f32>;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // CSS pixels to NDC
    let ndc_x = (in.position.x / screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (in.position.y / screen_size.y) * 2.0;
    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.uv = in.uv;
    return out;
}

// Fragment shader (textured)
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var tex_sampler: sampler;

@fragment
fn fs_text(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(tex, tex_sampler, in.uv);
}

// Fragment shader (solid color)
@group(1) @binding(0) var<uniform> color: vec4<f32>;

@fragment
fn fs_color(in: VertexOutput) -> @location(0) vec4<f32> {
    return color;
}
```

---

## Development Workflow

### WASM Build System

We use a custom build tool (inspired by MoonZoon's `mzoon` CLI) that provides:
- **Auto-installing build tools**: Downloads wasm-bindgen and wasm-opt on first use
- **File watching**: Monitors `.rs`, `.wgsl`, and `Cargo.toml` files
- **Live reload**: Browser auto-reloads on successful builds
- **Fast incremental builds**: ~2-3 seconds after first build

**Full documentation**: See `WASM_BUILD_SYSTEM_COMPLETE.md`

### Build Commands (Justfile)

```bash
# Development (RECOMMENDED - auto-reload enabled)
just start-wasm          # Start dev server with auto-reload
just start-wasm-open     # Start dev server + open browser

# Build only
just build-wasm          # Dev build (fast, unoptimized)
just build-wasm-release  # Release build (optimized + compressed)

# Manual serve (no auto-reload)
just serve-web           # Serve existing build

# Tools
just test                # Run all tests
just check               # Check Rust code
just fmt                 # Format code

# Utilities (raybox-tools commands)
raybox-tools wasm-build [--release]
raybox-tools wasm-start [--open] [--port 8000]
raybox-tools screenshot -u URL -o output.png
raybox-tools pixel-diff -r ref.png -c current.png
raybox-tools extract-dom -o layout.json
raybox-tools compare-layouts -r ref.json -a actual.json
```

### Development Loop

**Recommended workflow** (with auto-reload):
```bash
# Terminal 1: Start dev server (keeps running)
just start-wasm-open

# Edit code in your editor
# - Changes to renderer/src/*.rs → auto-rebuild + reload
# - Changes to shaders (*.wgsl) → auto-rebuild + reload
# - Browser reloads automatically on successful build
# - Build errors don't reload browser
```

**Manual workflow** (if needed):
1. Edit Rust code
2. Run `just build-wasm`
3. Refresh Chrome manually
4. Inspect in DevTools

---

## Implementation Plan

### Milestone 0: Hello WebGPU
- [x] Create workspace structure
- [x] Write specs.md
- [x] Setup Cargo workspace (renderer, tools crates)
- [x] Create Justfile with build commands
- [x] Create web/index.html with WASM loading
- [x] Create tools crate with wasm-build, wasm-start commands
- [x] Verify toolchain (wasm32 target, auto-installing tools)
- [x] Build system with auto-reload working
- [ ] Initialize WebGPU in renderer/src/lib.rs
- [ ] Create render pipeline
- [ ] Write WGSL shaders (vertex + fragment)
- [ ] Render colored triangle (verify pipeline works)

**Success**: Chrome shows colored triangle on canvas, no console errors

### ~~Milestone 1: Layout Engine~~ (SKIPPED - Using Chrome's positions)
<!--
We're using pre-computed layout positions from `reference/todomvc_dom_layout.json` instead of building a layout engine.
This saves significant development time and guarantees pixel-perfect accuracy.
For future projects, a separate layout library may be developed.

Original plan (no longer needed):
- [ ] Implement layout tree (Node, Style, ComputedLayout)
- [ ] Implement flexbox layout solver
  - [ ] Column/row layout
  - [ ] Padding, margin
  - [ ] Justify content, align items
  - [ ] Gap
- [ ] Write unit tests for layout cases
- [ ] Create TodoMVC layout tree hard-coded in Rust
- [ ] Compute layout, log positions to console
-->

### Milestone 1: Load Layout Data
- [ ] Load `reference/todomvc_dom_layout.json` via fetch
- [ ] Parse into LayoutData struct (already implemented in renderer/src/layout.rs)
- [ ] Log element count to console (should be 45 elements)
- [ ] Verify all positions, colors, fonts loaded correctly

**Success**: Console shows "Loaded layout with 45 elements"

### Milestone 2: Basic Rendering - Rectangles
- [ ] Create WebGPU render pipeline (vertex + fragment shaders)
- [ ] Implement coordinate transform (CSS pixels → NDC)
- [ ] Render colored rectangles at layout positions
- [ ] Draw all 45 elements as colored boxes using layout data

**Success**: Chrome shows 45 colored boxes at correct positions

### Milestone 3: Text Rendering
- [ ] Create offscreen Canvas2D in JS, pass to Wasm
- [ ] Implement text measurement in JS
- [ ] Render text to canvas, copy pixels to Wasm
- [ ] Upload text texture to WebGPU
- [ ] Draw textured quads for all text elements
- [ ] Handle all 16 text elements from layout data (one texture per element for V1)

**Success**: TodoMVC renders with all text visible and positioned correctly

### Milestone 4: Polish & Verification
- [ ] Fine-tune positioning if needed
- [ ] Use raybox-tools screenshot to capture render
- [ ] Use raybox-tools pixel-diff to compare with reference
- [ ] Achieve <5px tolerance on pixel-diff
- [ ] Verify all 45 elements render correctly
- [ ] Document any known issues

**Success**: pixel-diff shows <5px deviation from reference screenshot

---

## Known Challenges & Solutions

### Challenge 1: Text Measurement Accuracy
**Problem**: Canvas2D measureText() might not match browser layout exactly
**Solution**:
- Use same font stack as reference
- Measure in same pixel scale as layout
- Add small padding buffer if needed
- Iterate and compare with DevTools inspector

### Challenge 2: Layout Algorithm Complexity
**Problem**: Full flexbox spec is huge
**Solution**:
- Implement only subset needed for TodoMVC
- Reference: Yoga, Taffy for algorithm inspiration
- Start with simpler cases, add features as needed
- Write tests for each TodoMVC layout pattern

### Challenge 3: Text Texture Management
**Problem**: Need textures for every text element
**Solution** (V1 - simple):
- One texture per text element
- Recreate on change
- No caching initially

**Solution** (V2 - optimized):
- Texture atlas
- Cache common text
- Reuse glyphs

### Challenge 4: WebGPU Device Initialization
**Problem**: Async in JS, needs proper error handling
**Solution**:
- Show loading state
- Graceful fallback if WebGPU unavailable
- Clear error messages in UI

---

## Testing Strategy

### V1 Testing
1. **Visual testing** (primary)
   - Side-by-side with reference in browser
   - Screenshot comparison (manual)
   - Pixel ruler in DevTools

2. **Layout testing**
   - Unit tests for layout computations
   - Golden positions from reference DOM
   - Assert computed positions match

3. **Rendering testing**
   - Verify WebGPU pipeline compiles
   - Check console for errors
   - Validate texture uploads

### V2+ Testing (future)
- Automated screenshot diff
- Pixel-perfect regression tests
- Interaction testing
- Performance benchmarks

---

## Constraints & Requirements

### Browser Support
- **V1**: Chrome 113+ (stable WebGPU)
- **V2**: Firefox (when WebGPU ships)
- No Safari (WebGPU implementation differs)
- No mobile (V1)

### Performance Targets
- **V1**: 60 FPS for static render (trivial)
- **V2**: 60 FPS with interactions
- **V3**: <100ms startup time

### Build Targets
- **V1**: Wasm32 only
- **V2**: Add native support (for headless testing)

---

## What's NOT in V1

Explicitly out of scope to stay focused:

- ❌ Colors, shadows, gradients (V2)
- ❌ Rounded corners (V2)
- ❌ Interactive states (hover, focus) (V3)
- ❌ Editing, adding, removing todos (V4)
- ❌ Filters (All/Active/Completed) (V4)
- ❌ Local storage persistence (V4)
- ❌ Animations, transitions (V5)
- ❌ Responsive layout (V5)
- ❌ Keyboard navigation (V5)
- ❌ Accessibility (V6)
- ❌ Headless rendering (V2)
- ❌ Native binary (V2)
- ❌ SDF rendering (V3+, once proven)
- ❌ 3D features (V10+)

---

## Dependencies

### Rust Crates
```toml
# Workspace Cargo.toml
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.dependencies]
wgpu = "27.0"
wasm-bindgen = "0.2.105"
js-sys = "0.3"
web-sys = { version = "0.3", features = [
    "Window", "Document", "HtmlCanvasElement",
    "WebGl2RenderingContext", "OffscreenCanvas",
    "CanvasRenderingContext2d", "TextMetrics",
    "ImageData", "GpuCanvasContext",
] }
wasm-bindgen-futures = "0.4"
console_error_panic_hook = "0.1"
log = "0.4"
console_log = "1.0"

# For layout (no_std compatible)
[workspace.dependencies.layout-deps]
serde = { version = "1.0", features = ["derive"], default-features = false }
```

### External Tools (All Rust-based!)
- ✅ `wasm-bindgen-cli` 0.2.104 (installed)
- ✅ `wasm-opt` version 121 (installed)
- ✅ `miniserve` 0.32.0 (Rust HTTP server, installed)
- ✅ `google-chrome` 141.0.7390.122 (for testing)
- ✅ `just` 1.42.4 (installed)
- ⚠️ `cargo-watch` (optional, recommended for dev loop)

### Dev Tools Crate (To Be Created)
- `tools/` crate will provide Rust-only dev utilities:
  - HTTP server (wrapper around miniserve or embedded)
  - Chrome screenshot capture (via Chrome DevTools Protocol)
  - Build orchestration
  - Visual comparison utilities

---

## Chrome Control Strategy

### How Chrome Is Controlled

**Current approach** (for reference screenshot):
- Used Chrome headless mode via CLI: `google-chrome --headless --screenshot=...`
- Simple but limited (can't interact, no real DevTools access)

**V1+ approach** (Rust-only via `tools` crate):
- Use **Chrome DevTools Protocol (CDP)** via Rust
- Recommended crate: `headless_chrome` or `chromiumoxide`
- Allows programmatic control from Rust:
  - Launch Chrome with remote debugging
  - Connect via WebSocket to CDP
  - Navigate, screenshot, evaluate JS, etc.

### Tools Crate Commands

```bash
# Serve demo locally
cargo run -p tools -- serve dist --port 8080

# Capture screenshot via CDP
cargo run -p tools -- screenshot \
  --url http://localhost:8080 \
  --output screenshot.png \
  --width 1920 --height 1080

# Compare screenshots
cargo run -p tools -- compare \
  --reference reference/todomvc_chrome_reference.png \
  --actual screenshot.png \
  --diff-output diff.png \
  --tolerance 5

# Watch mode (rebuild + serve)
cargo run -p tools -- watch
```

### CDP Benefits (Rust-only)
- ✅ Portable - no Python/Node.js dependencies
- ✅ Programmatic - can automate complex workflows
- ✅ Fast - native Rust performance
- ✅ Testable - can verify renders in CI
- ✅ Integrated - same codebase for dev and test tools

### Dependencies for `tools` crate
```toml
[dependencies]
headless_chrome = "1.0"  # or chromiumoxide
tokio = { version = "1", features = ["full"] }
image = "0.24"  # For screenshot comparison
clap = { version = "4", features = ["derive"] }  # CLI parsing
anyhow = "1.0"
```

---

## Decisions Made ✅

1. **Text rendering**: ✅ Canvas2D hybrid for V1
2. **Success tolerance**: ✅ <5px positioning error acceptable
3. **HTTP server**: ✅ Python3 http.server (built-in)
4. **Reference screenshot**: ✅ Fresh Chrome 141 capture at 1920×1080, DPR=1
   - See `./reference/todomvc_chrome_reference.png`
   - Full metadata in `./reference/REFERENCE_METADATA.md`

---

## Implementation Status

**Completed** (as of 2025-11-01):
- ✅ Workspace structure created
- ✅ Cargo.toml and Justfile configured
- ✅ web/index.html with WASM loading and live reload
- ✅ Custom WASM build system (auto-install, file watching, live reload)
- ✅ Tools crate with 9 commands (wasm-build, wasm-start, screenshot, pixel-diff, etc.)
- ✅ Build system tested and working
- ✅ All infrastructure ready for renderer implementation

**Current**: Starting Milestone 0 (Hello WebGPU - render colored triangle)

**Next**: Proceed with milestones 0-4 to complete V1 TodoMVC renderer
