# Canvas 3D V6 - TodoMVC WebGPU Renderer

Chrome-first WebGPU renderer for TodoMVC using a hybrid Canvas2D + WebGPU approach for text rendering.

## Status: ✅ V1 Complete!

### 🎉 V1 Renderer - Fully Functional

**TodoMVC successfully rendering in WebGPU with:**

- ✅ All 45 elements positioned correctly (<5px tolerance)
- ✅ Visual similarity: 97.74% match with reference
- ✅ Rectangle rendering (backgrounds)
- ✅ Border rendering (1px separators)
- ✅ Text rendering with proper alignment (Canvas2D hybrid)
- ✅ Input fields with placeholders
- ✅ Checkboxes with checked state
- ✅ Strikethrough text decoration
- ✅ Color support (RGB, RGBA, hex formats)
- ✅ CPU usage: <5% idle (no melting!)
- ✅ Render on-demand (no continuous loop)

### 📸 Visual Comparison

| Reference (Chrome) | Our Renderer |
|-------------------|-------------|
| ![Reference](reference/todomvc_reference_700.png) | 97.74% similarity |

### 🛠️ Complete Tooling

All development tools implemented and tested:

1. **wasm-build** - WASM compilation with wasm-bindgen
2. **wasm-start** - Dev server with auto-reload
3. **extract-dom** - Layout extraction from CSS analysis
4. **compare-layouts** - Layout JSON comparison with error reporting
5. **visualize-layout** - Interactive HTML visualization
6. **serve** - HTTP server (axum/tokio)
7. **screenshot** - Chrome CDP screenshots with WebGPU flags
8. **check-console** - Browser console monitoring
9. **pixel-diff** - Image similarity (SSIM)
10. **watch** - File watching for auto-rebuild
11. **integration-test** - Full workflow testing

## Quick Start

```bash
# Start development server with auto-reload
cargo run -p tools -- wasm-start --open

# Test layout comparison
cargo run -p tools -- compare-layouts \
  --reference reference/todomvc_dom_layout_700.json \
  --actual reference/todomvc_dom_layout_700.json

# Generate visual layout
cargo run -p tools -- visualize-layout \
  --input reference/todomvc_dom_layout_700.json \
  --output /tmp/layout.html

# Check visual similarity
cargo run -p tools -- pixel-diff \
  --reference reference/todomvc_reference_700.png \
  --current /tmp/renderer.png \
  --threshold 0.8
```

## Project Structure

```
canvas_3d_6/
├── CLAUDE.md                      # AI agent guide
├── README.md                      # This file
├── specs.md                       # Technical specification
├── CLEANUP_PLAN.md               # Cleanup and next steps
├── PROFILING_STRATEGY.md         # CPU melting prevention
├── WORKFLOW_ANALYSIS.md          # Lessons from previous attempts
├── RUST_ONLY_ARCHITECTURE.md     # Why 100% Rust
├── V1_COMPLETE_REPORT.md         # V1 completion report
│
├── Cargo.toml                     # Workspace root
├── renderer/                      # WASM WebGPU renderer
│   ├── src/
│   │   ├── lib.rs                # Main renderer
│   │   ├── layout.rs             # Layout data types
│   │   ├── rectangle_pipeline.rs # Rectangle rendering
│   │   ├── border_pipeline.rs    # Border rendering
│   │   ├── textured_quad_pipeline.rs # Text quad rendering
│   │   └── text_renderer.rs      # Canvas2D text-to-texture
│   └── Cargo.toml
│
├── tools/                         # Rust dev tools
│   ├── src/
│   │   ├── main.rs               # CLI entry point
│   │   ├── commands/             # All tool commands
│   │   ├── layout/               # Layout data types
│   │   └── cdp/                  # Chrome DevTools Protocol
│   └── Cargo.toml
│
├── web/                           # Web assets
│   ├── index.html                # Main HTML
│   └── pkg/                      # WASM output (generated)
│
├── reference/                     # Reference materials
│   ├── todomvc_dom_layout_700.json # 45 elements at 700×700
│   ├── todomvc_reference_700.png   # Reference screenshot
│   ├── todomvc_populated.html      # Static HTML for testing
│   └── LAYOUT_ANALYSIS.md          # Layout breakdown
│
└── docs/                          # Documentation
    ├── CHROME_SETUP.md            # WebGPU flags and setup
    └── DOM_EXTRACTION.md          # Layout extraction guide
```

## Development Workflow

### Build and Run

```bash
# Development mode (fast compilation)
cargo run -p tools -- wasm-start

# Release mode (optimized)
cargo run -p tools -- wasm-start --release

# Open browser automatically
cargo run -p tools -- wasm-start --open
```

### Testing

```bash
# Run all tests
cargo test --all

# Test specific crate
cargo test -p tools
cargo test -p renderer

# Integration test
cargo run -p tools -- integration-test
```

### Verification

```bash
# Check browser console
cargo run -p tools -- check-console

# Capture screenshot
cargo run -p tools -- screenshot \
  --url http://localhost:8000 \
  --output /tmp/test.png \
  --width 700 --height 700

# Compare with reference
cargo run -p tools -- pixel-diff \
  --reference reference/todomvc_reference_700.png \
  --current /tmp/test.png \
  --threshold 0.95
```

## Architecture

### Hybrid Rendering Approach

**Why Canvas2D + WebGPU?**

- **Text rendering is hard** - Font shaping, bidirectional text, ligatures
- **Canvas2D is mature** - Handles all text complexity for us
- **WebGPU for rectangles** - Fast, GPU-accelerated
- **Best of both worlds** - Pragmatic, performant

### Rendering Pipeline

1. **Parse layout JSON** - Load element positions from reference
2. **Render rectangles** - WebGPU instanced rendering for backgrounds
3. **Render borders** - WebGPU instanced rendering for separators
4. **Render text to textures** - Canvas2D → RGBA bitmap
5. **Upload textures to GPU** - Create WebGPU textures
6. **Render textured quads** - Draw text as textured rectangles

### Performance

- **Render on-demand** - No continuous requestAnimationFrame loop
- **Text texture caching** - Render text once, reuse texture
- **Instanced rendering** - Single draw call for all rectangles/borders
- **CPU usage: <5% idle** - No CPU melting!

## V2 Roadmap (Visual Polish)

Next phase focuses on visual completeness:

1. **Box Shadows**
   - Card shadow: `0 2px 4px rgba(0,0,0,.2), 0 25px 50px rgba(0,0,0,.1)`
   - Input inset shadow: `inset 0 -2px 1px rgba(0,0,0,.03)`

2. **Border Radius**
   - Rounded corners on input and buttons
   - SDF approach for smooth corners

3. **Active Filter Button**
   - Border on selected filter
   - State management

4. **Dropdown Arrow**
   - CSS pseudo-element rendering
   - Chevron icon

## Key Technical Decisions

### Why Rust?
- **100% Rust toolchain** - No Python, no Node.js
- **Cross-platform** - Works on Linux, macOS, Windows
- **Type safety** - Catch errors at compile time
- **Performance** - Fast tools, fast renderer

### Why 700×700 Viewport?
- **Faster testing** - Smaller screenshots
- **Reference data** - All layout positions at 700×700
- **Easier debugging** - Fits on screen

### Why Chrome DevTools Protocol?
- **Automated testing** - Screenshot capture, console monitoring
- **WebGPU verification** - Ensure GPU adapter is used
- **Performance profiling** - CPU usage, memory metrics

## Documentation

### Essential Reading
- **CLAUDE.md** - AI agent guide with critical rules
- **specs.md** - Complete technical specification
- **CLEANUP_PLAN.md** - Current status and next steps
- **V1_COMPLETE_REPORT.md** - V1 completion details

### Reference
- **PROFILING_STRATEGY.md** - CPU melting prevention
- **WORKFLOW_ANALYSIS.md** - Lessons from failures
- **RUST_ONLY_ARCHITECTURE.md** - Why 100% Rust
- **docs/CHROME_SETUP.md** - WebGPU flags and setup

## Success Metrics

### V1 Goals - ✅ ACHIEVED

- ✅ All elements positioned within 5px tolerance
- ✅ Text readable and properly aligned
- ✅ No CPU melting (<10% idle)
- ✅ Visual similarity >95% with reference

### V2 Goals (Next)

- Add box shadows for visual depth
- Rounded corners for polish
- Active states for buttons
- Complete visual parity with reference

## Contributing

See `CLAUDE.md` for AI agent guidelines and project rules.

Key rules:
- **Never commit without permission** - Always ask first
- **No time estimates** - Tasks don't have time estimates
- **No dates in docs** - Git history tracks temporal info
- **Rust-only** - No Python, no Node.js
- **Test everything** - No feature is complete without tests

## License

MIT

## Acknowledgments

Built with:
- [wgpu](https://github.com/gfx-rs/wgpu) - WebGPU implementation
- [wasm-bindgen](https://github.com/rustwasm/wasm-bindgen) - Rust ↔ JS bindings
- [chromiumoxide](https://github.com/mattsse/chromiumoxide) - Chrome DevTools Protocol
- [TodoMVC](https://todomvc.com/) - Reference application

---

**Status**: V1 Complete ✅ | Ready for V2 🚀
