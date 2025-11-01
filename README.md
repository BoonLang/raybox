# Canvas 3D V6 - TodoMVC WebGPU Renderer

Chrome-first WebGPU renderer for TodoMVC, proving our layout and text rendering approach before expanding to 3D features.

## Status: ✅ All Tools Complete - Ready for Renderer Implementation!

### What's Complete

- ✅ **Comprehensive specs.md** - Complete technical specification
- ✅ **100% Rust toolchain** - No Python/Node.js dependencies!
- ✅ **Complete tools crate** with 11 commands (all cross-platform!):
  - `wasm-build` - Build WASM renderer with wasm-opt
  - `wasm-start` - Dev server with auto-reload
  - `watch` - File watcher for auto-rebuild
  - `extract-dom` - Extract layout from HTML/CSS via Chrome
  - `compare-layouts` - Compare layout JSONs with tolerance
  - `visualize-layout` - Interactive HTML visualization
  - `serve` - HTTP server for development
  - `check-console` - CDP console monitoring + profiling + screenshots
  - `pixel-diff` - Screenshot comparison with similarity scoring
  - `screenshot` - Capture screenshots via Chrome
  - `integration-test` - Full integration test suite (replaces bash script!)

- ✅ **Reference data extracted**:
  - `reference/todomvc_dom_layout.json` - 45 elements with positions
  - Ground truth screenshot at 1920×1080, DPR=1
  - Populated HTML for testing

- ✅ **Chrome DevTools Protocol (CDP) integration**:
  - Console error monitoring
  - Performance metrics collection
  - CPU profiling (V8 profiler)
  - Screenshot capture automation
  - Visual regression testing (pixel diff)

- ✅ **Comprehensive tests** - 13+ tests passing
- ✅ **Integration test** - Full end-to-end validation
- ✅ **Justfile** with 18+ development commands
- ✅ **CLAUDE.md** for AI agent guidance

### Quick Start

```bash
# Build and start development server
cargo run -p tools -- wasm-start --open

# Run integration tests (100% Rust, cross-platform!)
cargo run -p tools -- integration-test

# Check for console errors
cargo run -p tools -- check-console

# With all diagnostics (console + screenshot + metrics + profiling)
cargo run -p tools -- check-console -s -m --profile 5

# Read the complete readiness checklist
cat READINESS_CHECKLIST.md
```

## V1 Goal

**Render TodoMVC with correct element positioning and readable text.**

Success = Visual comparison with reference shows <5px positioning errors.

❌ Colors, shadows (V2)
❌ Interactivity (V3+)

## Project Structure (Planned)

```
canvas_3d_6/
├── specs.md              ← Complete technical specification
├── README.md             ← This file
├── reference/            ← TodoMVC reference materials
│   ├── todomvc_chrome_reference.png   ← Ground truth screenshot
│   ├── REFERENCE_METADATA.md          ← Capture details
│   ├── todomvc_populated.html         ← Static state for testing
│   └── app.css, index.html, etc.      ← Original files
├── Cargo.toml            ← Workspace root (to be created)
├── Justfile              ← Build commands (to be created)
├── crates/               ← Rust crates (to be created)
│   ├── renderer/         ← WebGPU renderer (Wasm)
│   └── layout/           ← Layout engine
├── web/                  ← Web demo (to be created)
│   ├── index.html
│   └── demo.js
└── dist/                 ← Build output (gitignored)
```

## Development Approach

1. **Chrome DevTools first** - Immediate visual feedback
2. **Incremental milestones** - Layout → Rendering → Text → Polish
3. **Side-by-side comparison** - Our render vs reference screenshot
4. **Testable checkpoints** - Each milestone has clear success criteria

## Next Steps

### Milestone 0: Setup & Skeleton (Day 1)

```bash
# Create workspace
# Setup Cargo.toml, Justfile
# Create minimal web/index.html
# Verify "Hello WebGPU" - canvas clears to red
```

See `specs.md` for detailed implementation plan.

## Key Decisions

- **Text rendering**: Canvas2D hybrid (pragmatic, fast)
- **Layout**: Custom flexbox-like solver (TodoMVC subset)
- **Scope**: V1 = position + text only
- **Success metric**: <5px error acceptable

## 🎯 Your Next Action

**👉 READ**: `READINESS_CHECKLIST.md` ← **START HERE!**

All tooling is complete and verified. Ready to start WebGPU renderer implementation.

## Documentation

### Essential Reading
- **READINESS_CHECKLIST.md** - 👈 **READ THIS FIRST!** Comprehensive readiness assessment
- **specs.md** - Full technical specification
- **tools/CDP_IMPLEMENTATION.md** - Chrome DevTools Protocol integration details

### Reference
- **RUST_ONLY_ARCHITECTURE.md** - Why 100% Rust
- **reference/REFERENCE_METADATA.md** - Screenshot metadata
- **CLAUDE.md** - AI agent guidance

## ✅ Complete Readiness Confirmation

1. ✅ **All tools implemented in Rust** - 10 commands, no Python/Node.js!
2. ✅ **DOM data extracted** - `reference/todomvc_dom_layout.json`
3. ✅ **Tests written and passing** - 13+ tests green
4. ✅ **Integration test updated** - Uses Rust CDP tools
5. ✅ **CDP monitoring complete** - Console, profiling, screenshots, pixel diff
6. ✅ **Documentation updated** - All core docs current
7. ✅ **Error handling transparent** - chromiumoxide warnings explained
8. ✅ **Development workflow ready** - Auto-reload, file watching, testing

**Ready to implement the WebGPU renderer!** 🚀

---

**Next: Renderer Implementation** 🎨

Priority 1: Basic WebGPU setup (triangle rendering, canvas init, shader pipeline)
Priority 2: Layout integration (parse JSON, render rectangles, basic text)
Priority 3: TodoMVC features (full UI, interactivity, visual parity)

See `READINESS_CHECKLIST.md` for complete implementation roadmap.
