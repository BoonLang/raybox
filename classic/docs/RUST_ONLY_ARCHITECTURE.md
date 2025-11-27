# Rust-Only Architecture

## Goal: 100% Rust Toolchain

Everything in this project uses Rust - no Python, Node.js, or other language dependencies (except Chrome for rendering).

---

## Dependencies

### External Tools (Installed via Cargo)
- ✅ `wasm-bindgen-cli` - Wasm ↔ JS bindings
- ✅ `wasm-opt` - Wasm optimization (from Binaryen project)
- ✅ `just` - Command runner (Rust alternative to Make)
- ✅ `raybox-tools` - Our custom dev tools (includes serve, watch, screenshot, etc.)

### Browser (External)
- ✅ `google-chrome` or `chromium` - For testing and screenshots

**Installation**:
```bash
cargo install wasm-bindgen-cli
cargo install just

# wasm-opt comes from Binaryen (install via package manager)
# Ubuntu/Debian:
sudo apt install binaryen

# Or download from: https://github.com/WebAssembly/binaryen/releases

# Build raybox-tools locally
cargo build --release -p tools
# Binary will be at: target/release/raybox-tools
```

---

## Workspace Crates

### 1. `renderer` - WebGPU Renderer (Wasm target)

**Purpose**: Core rendering logic that runs in the browser
**Target**: `wasm32-unknown-unknown`
**Output**: Wasm module with JS bindings

**Key Dependencies**:
```toml
wgpu = "27.0"
wasm-bindgen = "0.2.105"
js-sys = "0.3"
web-sys = { version = "0.3", features = [
    "Window", "Document", "HtmlCanvasElement",
    "CanvasRenderingContext2d", "TextMetrics",
    "ImageData", "GpuCanvasContext"
] }
```

### 2. `layout` - Layout Engine (no_std)

**Purpose**: Flexbox-like layout computation
**Target**: `wasm32` + native (no_std compatible)
**Output**: Library crate

**Key Dependencies**:
```toml
serde = { version = "1.0", default-features = false }
# Minimal deps, works in Wasm and native
```

### 3. `tools` - Dev Tools CLI (Native target)

**Purpose**: Rust-based development utilities
**Target**: Native (Linux/macOS/Windows)
**Output**: Binary crate

**Commands**:
- ✅ `extract-dom` - Extract DOM layout from HTML/CSS via Chrome
- ✅ `compare-layouts` - Compare two layout JSON files with tolerance
- ✅ `visualize-layout` - Generate interactive HTML visualization of layouts
- ✅ `serve` - HTTP server for local development
- ✅ `screenshot` - Capture screenshots via Chrome DevTools Protocol
- ✅ `watch` - Auto-rebuild on file changes

**Key Dependencies**:
```toml
headless_chrome = "1.0"  # Chrome DevTools Protocol
tokio = { version = "1", features = ["full"] }
image = "0.24"  # Screenshot processing
clap = { version = "4", features = ["derive"] }
anyhow = "1.0"
```

---

## Chrome Control (CDP)

### Chrome DevTools Protocol via Rust

Instead of shell scripts or Python, we use Rust to control Chrome programmatically.

**How it works**:
1. Launch Chrome with remote debugging enabled
2. Connect to Chrome via WebSocket (CDP)
3. Send commands (navigate, screenshot, evaluate JS, etc.)
4. Receive responses and events

**Example** (pseudo-code for `tools/src/chrome.rs`):
```rust
use headless_chrome::{Browser, LaunchOptions};

pub fn capture_screenshot(url: &str, output: &Path) -> Result<()> {
    let browser = Browser::new(LaunchOptions {
        headless: true,
        window_size: Some((1920, 1080)),
        ..Default::default()
    })?;

    let tab = browser.new_tab()?;
    tab.navigate_to(url)?;
    tab.wait_until_navigated()?;

    let screenshot = tab.capture_screenshot(
        headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png,
        None,
        None,
        true,
    )?;

    std::fs::write(output, &screenshot)?;
    Ok(())
}
```

**Benefits**:
- ✅ Cross-platform (Linux, macOS, Windows)
- ✅ Type-safe
- ✅ Testable
- ✅ Fast (native Rust)
- ✅ No external script dependencies

---

## Build Workflow (All Rust)

### Using `just` (Rust command runner)

```makefile
# Justfile

# Build Wasm (dev)
build-wasm-dev:
    cargo build --target wasm32-unknown-unknown -p renderer
    wasm-bindgen target/wasm32-unknown-unknown/debug/renderer.wasm \
        --out-dir dist --target web --no-typescript

# Serve via miniserve
serve:
    miniserve dist --port 8080 --index index.html

# Serve via our tools crate
serve-tools:
    cargo run -p tools -- serve dist --port 8080

# Screenshot via tools crate
screenshot:
    cargo run -p tools -- screenshot \
        --url http://localhost:8080 \
        --output classic/screenshots/screenshot.png

# Compare screenshots
compare:
    cargo run -p tools -- compare \
        --reference reference/screenshots/todomvc_chrome_reference.png \
        --actual classic/screenshots/screenshot.png
```

### Development Loop

**Option 1: Manual**
```bash
# Terminal 1
just build-wasm-dev && just serve

# Edit code, then re-run build
```

**Option 2: Auto-rebuild (with raybox-tools watch)**
```bash
# Terminal 1: Auto-rebuild
cargo run -p tools -- watch --command "just build" .

# Terminal 2: Serve
just serve
```

**Option 3: Using Justfile recipes**
```bash
# Single command for common workflows
just dev    # Watch and rebuild
just test   # Run tests
just verify # Run all verification
```

---

## Why Rust-Only?

### Advantages

1. **Portability**
   - Works on Linux, macOS, Windows
   - No Python/Node.js version issues
   - Single `cargo install` for all tools

2. **Performance**
   - Native Rust speed for build tools
   - Fast screenshot processing
   - Efficient file watching

3. **Type Safety**
   - Compile-time checks
   - Fewer runtime errors in tooling

4. **Consistency**
   - Same language for app and tools
   - Shared code between crates
   - Unified build system

5. **Simplicity**
   - One toolchain to install
   - No `package.json` + `requirements.txt` + `Cargo.toml`
   - Fewer moving parts

### Trade-offs

1. **Initial setup**
   - Need to install Rust toolchain
   - Binaryen for wasm-opt (not pure Rust)

2. **Ecosystem maturity**
   - Chrome CDP libraries less mature than Puppeteer (Node.js)
   - But `headless_chrome` and `chromiumoxide` are solid

3. **Learning curve**
   - Team needs Rust knowledge
   - But simpler than multi-language stack

---

## File Locations

### No Python
- ❌ No `*.py` files
- ❌ No `requirements.txt`
- ❌ No `venv/` or `__pycache__/`

### No Node.js
- ❌ No `package.json`
- ❌ No `node_modules/`
- ❌ No `npm` or `yarn`

### Only Rust
- ✅ `Cargo.toml` (workspace + crates)
- ✅ `Justfile` (commands)
- ✅ `src/` dirs in each crate
- ✅ `target/` (build output)

---

## Summary

**Before** (multi-language):
```
Project uses: Rust + Python + Node.js + Shell scripts
Tools: cargo, pip, npm, bash
Dependencies: Cargo.toml + requirements.txt + package.json
```

**After** (Rust-only):
```
Project uses: Rust only
Tools: cargo, just, miniserve (all via cargo install)
Dependencies: Cargo.toml only
```

**Result**: Simpler, faster, more portable development environment.

---

## Status

✅ **Complete** - All tooling implemented in Rust:
1. ✅ `tools` crate created with 6 commands
2. ✅ All commands tested and working
3. ✅ Comprehensive test suite (10/10 passing)
4. ✅ End-to-end Rust workflow verified
5. ✅ Documentation updated

Ready for renderer implementation!
