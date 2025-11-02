# Canvas Tools

Rust-based development tools for the Canvas 3D 6 TodoMVC renderer project.

## Overview

This crate provides the `raybox-tools` CLI with commands for building, testing, and verifying the WebGPU-based TodoMVC renderer.

## Commands

### Screenshot

Capture screenshots of web pages with WebGPU support.

**Automatically applies required WebGPU Chrome flags.**

```bash
# Quick 700×700 verification (recommended)
cargo run -p tools -- screenshot \
  --url http://localhost:8000 \
  --output /tmp/test.png \
  --width 700 \
  --height 700

# Full 1920×1080 reference comparison
cargo run -p tools -- screenshot \
  --url http://localhost:8000 \
  --output /tmp/full.png \
  --width 1920 \
  --height 1080
```

**Standard Testing Sizes:**
- **700×700px** - Quick verification, recommended for rapid testing
- **1920×1080px** - Full reference size (matches `reference/todomvc_dom_layout.json`)

**Implementation:**
- Uses `chromiumoxide` browser automation
- Automatically launches Chrome with WebGPU flags:
  - `--enable-unsafe-webgpu`
  - `--enable-webgpu-developer-features`
  - `--enable-features=Vulkan,VulkanFromANGLE`
  - `--enable-vulkan`
  - `--use-angle=vulkan`
  - `--disable-software-rasterizer`
  - `--ozone-platform=x11`
- Non-headless mode (WebGPU requires visible window)
- 2-second wait for WebGPU initialization

### Check Console

Monitor browser console for errors and warnings via Chrome DevTools Protocol.

**Automatically applies required WebGPU Chrome flags.**

```bash
# Check for console errors
cargo run -p tools -- check-console \
  --url http://localhost:8000 \
  --wait 5

# Check console with screenshot and performance metrics
cargo run -p tools -- check-console \
  --url http://localhost:8000 \
  --wait 5 \
  --screenshot \
  --performance

# Check console with CPU profiling
cargo run -p tools -- check-console \
  --url http://localhost:8000 \
  --wait 5 \
  --profile 10
```

**Options:**
- `--url <URL>` - URL to check (default: `http://localhost:8000`)
- `--wait <SECONDS>` - Wait time for page load (default: 3)
- `--screenshot` - Also capture a screenshot
- `--performance` - Also collect performance metrics
- `--profile <SECONDS>` - Run CPU profiling for N seconds

**Exit codes:**
- `0` - No errors detected
- `1` - Errors or exceptions found in console

**Expected output (success):**
```
📊 Browser Console Report
   URL: http://localhost:8000
   Messages: 0 total
   Errors: 0
   Exceptions: 0
   ✅ No errors detected!
```

**Expected output (error):**
```
📊 Browser Console Report
   URL: http://localhost:8000
   Messages: 1 total
   Errors: 1
   Exceptions: 0

❌ Console Errors:
   [Error] Initialization error: Failed to find suitable GPU adapter...
```

### WASM Build

Build the WebGPU renderer to WebAssembly.

```bash
# Debug build (fast compilation)
cargo run -p tools -- wasm-build

# Release build (optimized)
cargo run -p tools -- wasm-build --release
```

**What it does:**
1. Compiles `renderer` crate to `wasm32-unknown-unknown`
2. Runs `wasm-bindgen` to generate JS bindings
3. Optionally runs `wasm-opt` for size optimization (release only)
4. Outputs to `web/pkg/`

**Generated files:**
- `web/pkg/renderer_bg.wasm` - Compiled WASM binary
- `web/pkg/renderer.js` - JS bindings
- `web/pkg/renderer.d.ts` - TypeScript definitions

### WASM Start

Start development server with auto-reload.

```bash
# Start dev server
cargo run -p tools -- wasm-start

# Start with browser auto-open
cargo run -p tools -- wasm-start --open

# Start with release build
cargo run -p tools -- wasm-start --release --open

# Custom port
cargo run -p tools -- wasm-start --port 3000
```

**What it does:**
1. Builds WASM renderer (debug or release mode)
2. Starts HTTP server on `http://localhost:8000` (or custom port)
3. Watches `renderer/src/` for file changes
4. Auto-rebuilds and triggers browser reload on changes
5. Optionally opens browser automatically

**File watching:**
- Watches: `renderer/src/**/*.rs`, `renderer/src/**/*.wgsl`
- Debounced: 500ms (prevents rebuild spam)
- Rebuild triggers: File save/modify events

**Recommended workflow:**
```bash
# Terminal 1: Dev server with auto-reload
cargo run -p tools -- wasm-start --open

# Edit files in renderer/src/
# Browser auto-reloads on save
```

### Extract DOM

Extract DOM layout data from CSS analysis (generates reference JSON).

```bash
cargo run -p tools -- extract-dom \
  --output reference/todomvc_dom_layout.json
```

**Generates:** JSON file with element positions, sizes, and styles.

### Compare Layouts

Compare two layout JSON files and report differences.

```bash
cargo run -p tools -- compare-layouts \
  --reference reference/todomvc_dom_layout.json \
  --actual output/renderer_layout.json \
  --diff-output /tmp/diff.json
```

**Reports:**
- Position differences (Euclidean distance)
- Size differences
- Missing/extra elements
- Summary statistics

### Visualize Layout

Generate HTML visualization of layout data.

```bash
cargo run -p tools -- visualize-layout \
  --input reference/todomvc_dom_layout.json \
  --output /tmp/layout_viz.html
```

**Opens in browser:** Visual representation of element positions.

### Pixel Diff

Compare two images pixel-by-pixel using SSIM.

```bash
cargo run -p tools -- pixel-diff \
  --reference reference/todomvc_chrome_reference.png \
  --current /tmp/screenshot.png \
  --threshold 0.95 \
  --output /tmp/diff.png
```

**Metrics:**
- SSIM (Structural Similarity Index)
- PSNR (Peak Signal-to-Noise Ratio)
- Pixel-by-pixel difference

### Integration Test

Run full integration test suite.

```bash
cargo run -p tools -- integration-test \
  --url http://localhost:8000
```

**Tests:**
- WebGPU initialization
- Rendering correctness
- Layout accuracy
- Performance benchmarks

## WebGPU Flags

**All browser automation commands (`screenshot`, `check-console`) automatically apply WebGPU flags.**

You **do not** need to manually launch Chrome with flags when using these tools.

**Flags applied:**
```bash
--enable-unsafe-webgpu
--enable-webgpu-developer-features
--enable-features=Vulkan,VulkanFromANGLE
--enable-vulkan
--use-angle=vulkan
--disable-software-rasterizer
--ozone-platform=x11
```

**Why these flags?**
- WebGPU is experimental on Linux and requires explicit enabling
- Without flags, WebGPU falls back to software rendering (CPU melts)
- See `docs/CHROME_SETUP.md` for detailed explanation

## Standard Testing Workflow

### 1. Start Dev Server

```bash
cargo run -p tools -- wasm-start --open
```

### 2. Check Console for Errors

```bash
cargo run -p tools -- check-console
```

**Expected:** `✅ No errors detected!`

### 3. Take Screenshot

```bash
cargo run -p tools -- screenshot \
  --url http://localhost:8000 \
  --output /tmp/test.png \
  --width 700 \
  --height 700
```

### 4. Verify Rendering

```bash
# Open screenshot
xdg-open /tmp/test.png

# Or compare with reference
cargo run -p tools -- pixel-diff \
  --reference reference/todomvc_chrome_reference.png \
  --current /tmp/test.png \
  --threshold 0.95
```

### 5. Run Tests

```bash
cargo test --all
```

## Troubleshooting

### "Failed to find suitable GPU adapter"

**Cause:** WebGPU not working (software rendering fallback)

**Fix:**
1. Check Vulkan: `vulkaninfo | grep deviceName`
2. Update GPU drivers
3. See `docs/CHROME_SETUP.md` for detailed setup

### "Browser process exited"

**Cause:** Multiple Chrome instances or profile lock

**Fix:**
```bash
# Clean up Chrome processes
pkill -f chromiumoxide-runner
rm -rf /tmp/chromiumoxide-runner
```

### Screenshot is blank

**Cause:** WebGPU not initialized or flags not applied

**Fix:**
1. Run `check-console` to see errors
2. Make sure `wasm-start` server is running
3. Increase wait time: modify `screenshot.rs` sleep duration

### Auto-reload not working

**Cause:** File watcher not detecting changes

**Fix:**
1. Check `renderer/src/` path is correct
2. Verify files have `.rs` or `.wgsl` extensions
3. Check console output for rebuild messages

## Dependencies

**Browser automation:**
- `chromiumoxide` - Chrome DevTools Protocol
- `futures` - Async runtime utilities
- `tokio` - Async runtime

**Image processing:**
- `image` - Image loading/saving
- `image-compare` - SSIM/PSNR comparison

**File watching:**
- `notify` - Cross-platform file watcher

**WASM tooling:**
- `wasm-bindgen-cli` - Generate JS bindings
- `binaryen` (wasm-opt) - WASM optimization

**HTTP server:**
- `axum` - Web framework
- `tower-http` - Static file serving

## Building

```bash
# Build tools
cargo build --release -p tools

# Run commands
./target/release/raybox-tools screenshot --url http://localhost:8000 --output /tmp/test.png --width 700 --height 700
./target/release/raybox-tools check-console --url http://localhost:8000
```

## See Also

- `../CLAUDE.md` - AI agent guide
- `../docs/CHROME_SETUP.md` - WebGPU Chrome setup
- `../specs.md` - Full technical specification
- `../RUST_ONLY_ARCHITECTURE.md` - Why Rust-only
