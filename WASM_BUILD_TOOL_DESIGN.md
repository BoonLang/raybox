# Custom WASM Build Tool Design

**Date**: 2025-11-01
**Inspired by**: MoonZoon's `mzoon` CLI

---

## Why Not wasm-pack?

**Problems with wasm-pack**:
- Opinionated defaults
- Limited flexibility
- Extra layer of abstraction
- Historical issues (as noted in MoonZoon)

**Our approach** (like MoonZoon):
- Use `wasm-bindgen` directly
- Use `wasm-opt` for optimization
- Auto-download tools as needed
- Full control over build pipeline

---

## Architecture Overview

### Core Principles
1. **Zero config** - Auto-detect project structure
2. **Auto-installing** - Download wasm-bindgen & wasm-opt on first use
3. **Fast iteration** - File watching + auto-rebuild + live reload
4. **Production-ready** - Optimization + compression

### Commands

```bash
# Dev mode: build + serve + watch + reload
canvas-tools wasm-start [--release] [--open] [--port 8000]

# Build only (no server)
canvas-tools wasm-build [--release]

# Serve existing build
canvas-tools serve web [--port 8000]  # Already exists!
```

---

## Build Pipeline

### 1. Check/Install Tools

**wasm-bindgen 0.2.105**:
- Check: `web/bin/wasm-bindgen -V`
- Download from: `https://github.com/rustwasm/wasm-bindgen/releases/download/0.2.105/wasm-bindgen-0.2.105-{platform}.tar.gz`
- Extract to: `web/bin/wasm-bindgen`
- Platforms: x86_64-unknown-linux-musl, x86_64-apple-darwin, aarch64-apple-darwin, x86_64-pc-windows-msvc

**wasm-opt (binaryen 123)**:
- Check: `web/bin/binaryen/bin/wasm-opt --version`
- Download from: `https://github.com/WebAssembly/binaryen/releases/download/version_123/binaryen-version_123-{platform}.tar.gz`
- Extract to: `web/bin/binaryen/`
- Platforms: x86_64-linux, x86_64-macos, arm64-macos, x86_64-windows

### 2. Compile to WASM

```bash
RUSTFLAGS=--cfg=web_sys_unstable_apis \
cargo build \
  --target wasm32-unknown-unknown \
  --package renderer \
  [--release]
```

**Output**: `target/wasm32-unknown-unknown/{debug|release}/renderer.wasm`

### 3. Generate JS Bindings

```bash
web/bin/wasm-bindgen \
  --target web \
  --no-typescript \
  --weak-refs \
  --out-dir web/pkg \
  [--debug]  # only in dev mode \
  target/wasm32-unknown-unknown/{profile}/renderer.wasm
```

**Output**:
- `web/pkg/renderer.js`
- `web/pkg/renderer_bg.wasm`
- `web/pkg/renderer_bg.wasm.d.ts` (optional)

### 4. Optimize (Release Only)

```bash
web/bin/binaryen/bin/wasm-opt \
  web/pkg/renderer_bg.wasm \
  --output web/pkg/renderer_bg.wasm \
  --enable-reference-types \
  -Oz  # Maximum size optimization
```

### 5. Compress (Release Only)

Generate compressed versions:
- `web/pkg/renderer_bg.wasm.br` (Brotli)
- `web/pkg/renderer_bg.wasm.gz` (Gzip)

---

## File Watching & Live Reload

### Watched Files
- `renderer/src/**/*.rs`
- `renderer/Cargo.toml`

### Debouncing
- 300ms delay after last file change
- Prevents multiple rebuilds on batch changes

### Rebuild Flow
1. File change detected
2. Abort current build (if running)
3. Wait for debounce
4. Start new build
5. On success: trigger browser reload
6. On failure: show error (don't reload)

### Live Reload Mechanism

**Option A: Simple polling** (easier)
- Inject `<script>` in index.html
- Poll `/_api/build_id` every 1 second
- Reload if build_id changes

**Option B: WebSocket** (better UX)
- WebSocket connection to dev server
- Server sends "reload" message on build complete
- Browser reloads immediately

**Implementation: Option A for V1** (simpler, good enough)

---

## Project Structure

```
canvas_3d_6/
├── renderer/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       └── layout.rs
├── web/
│   ├── index.html           # Entry point
│   ├── bin/                 # Downloaded tools (gitignored)
│   │   ├── wasm-bindgen     # Auto-downloaded
│   │   └── binaryen/
│   │       ├── bin/wasm-opt
│   │       └── lib/         # Required on macOS
│   ├── pkg/                 # Build output (gitignored)
│   │   ├── renderer.js
│   │   ├── renderer_bg.wasm
│   │   ├── renderer_bg.wasm.br   # Release only
│   │   └── renderer_bg.wasm.gz   # Release only
│   └── _api/
│       └── build_id.txt     # For reload detection
└── tools/
    └── src/commands/
        ├── wasm_build.rs    # NEW
        ├── wasm_start.rs    # NEW
        └── wasm_serve.rs    # Optional (reuse serve.rs)
```

---

## Implementation Plan

### Phase 1: Tool Installation (1 hour)
**Files**: `tools/src/wasm_bindgen.rs`, `tools/src/wasm_opt.rs`

- Create download helper (reuse from serve.rs or add reqwest)
- Implement `check_or_install_wasm_bindgen()`
  - Check version: `web/bin/wasm-bindgen -V`
  - Download from GitHub releases
  - Extract tar.gz
  - Make executable
- Implement `check_or_install_wasm_opt()`
  - Check version: `web/bin/binaryen/bin/wasm-opt --version`
  - Download from GitHub releases
  - Extract tar.gz
  - Make executable

### Phase 2: Build Command (1.5 hours)
**File**: `tools/src/commands/wasm_build.rs`

- Add `WasmBuild` command to main.rs
- Implement build pipeline:
  1. Check/install tools
  2. Run `cargo build --target wasm32-unknown-unknown -p renderer`
  3. Run `wasm-bindgen` on output
  4. If `--release`: run `wasm-opt`
  5. If `--release`: compress with brotli/gzip
- Handle errors gracefully
- Progress indicators

### Phase 3: Dev Server & Reload (1.5 hours)
**File**: `tools/src/commands/wasm_start.rs`

- Add `WasmStart` command
- Reuse existing `serve` command for HTTP server
- Add live reload injection:
  - Inject `<script>` into index.html responses
  - Script polls `/_api/build_id`
- Create `/_api/build_id` endpoint
  - Returns current build timestamp
- Generate unique build ID on each build
- Write to `web/_api/build_id.txt`

### Phase 4: File Watcher (1 hour)
**File**: `tools/src/watcher.rs`

- Add `notify` dependency (already have it!)
- Watch `renderer/src/**/*.rs` and `renderer/Cargo.toml`
- Debounce changes (300ms)
- On change:
  - Print "Rebuilding..."
  - Abort current build
  - Run build pipeline
  - Update build_id
  - Print "✓ Build complete"

### Phase 5: Polish (30 mins)
- Add `--open` flag (auto-open browser using `open` crate)
- Better error messages
- Build time reporting
- File size reporting

**Total estimate: 5-6 hours**

---

## Dependencies to Add

```toml
# tools/Cargo.toml
[dependencies]
# ... existing deps ...

# File watching (already have notify)
# notify = { workspace = true }  # Already present

# HTTP client for downloading tools
reqwest = { version = "0.11", features = ["blocking"] }

# Archive extraction
tar = "0.4"
flate2 = { version = "1.0", features = ["rust_backend"] }

# Compression
brotli = "3.4"

# Browser opening
open = "5.0"
```

---

## Usage Examples

### Development Workflow

```bash
# First time: auto-downloads wasm-bindgen & wasm-opt
canvas-tools wasm-start

# Output:
# Checking wasm-bindgen... not found
# Downloading wasm-bindgen 0.2.105...
# ✓ wasm-bindgen installed
# Checking wasm-opt... not found
# Downloading wasm-opt 123...
# ✓ wasm-opt installed
# Building renderer...
# ✓ Build complete (1.2s)
# Starting server on http://localhost:8000
# Watching renderer/ for changes...
```

### Production Build

```bash
canvas-tools wasm-build --release

# Output:
# Building renderer (release)...
# ✓ Compiled (12.3s)
# ✓ wasm-bindgen (0.4s)
# ✓ wasm-opt -Oz (5.1s)
# ✓ Compressed:
#   - renderer_bg.wasm: 245 KB
#   - renderer_bg.wasm.br: 89 KB (-64%)
#   - renderer_bg.wasm.gz: 112 KB (-54%)
# ✓ Build complete: web/pkg/
```

### Just Serve

```bash
# Serve existing build (no compilation)
canvas-tools serve web --port 8000
```

---

## Key Differences from MoonZoon

| Aspect | MoonZoon | Our Tool |
|--------|----------|----------|
| Scope | Frontend + Backend + Workers | Frontend only |
| Integration | Standalone binary | Part of tools crate |
| Commands | `mzoon start/build` | `canvas-tools wasm-*` |
| wasm-bindgen | 0.2.100 | 0.2.105 (latest) |
| Config | MoonZoon.toml | None (auto-detect) |
| Live Reload | WebSocket | Polling (simpler) |

---

## Testing Plan

1. **Test tool installation**:
   - Delete `web/bin/`
   - Run `canvas-tools wasm-build`
   - Verify tools downloaded correctly

2. **Test dev build**:
   - Run `canvas-tools wasm-build`
   - Check `web/pkg/renderer.js` exists
   - Check WASM file is unoptimized (larger)

3. **Test release build**:
   - Run `canvas-tools wasm-build --release`
   - Check WASM file is optimized (smaller)
   - Check `.br` and `.gz` files exist

4. **Test file watching**:
   - Run `canvas-tools wasm-start`
   - Edit `renderer/src/lib.rs`
   - Verify auto-rebuild
   - Verify browser reload

5. **Test error handling**:
   - Introduce syntax error in Rust code
   - Verify error displayed
   - Verify no reload triggered
   - Fix error
   - Verify rebuild succeeds

---

## Next Steps

1. Create design document ✅ (this file)
2. Get user approval on approach
3. Implement Phase 1 (tool installation)
4. Implement Phase 2 (build command)
5. Test basic build works
6. Implement Phase 3 (dev server)
7. Implement Phase 4 (file watcher)
8. Implement Phase 5 (polish)
9. Update Justfile with new commands
10. Update README with new workflow

---

## Open Questions

1. **Live reload**: Polling vs WebSocket? → Start with polling, upgrade later
2. **Command naming**: `wasm-build` vs `build-wasm`? → `wasm-build` (consistent with existing)
3. **Separate binary**: Keep in `tools/` or create `canvas-dev/`? → Keep in `tools/` for now
4. **Browser auto-open**: Include `--open` flag? → Yes, useful for quick iteration

---

## Success Criteria

✅ Can run `canvas-tools wasm-build` without manual tool installation
✅ Can run `canvas-tools wasm-start` and see live WASM app
✅ File changes trigger automatic rebuild
✅ Browser automatically reloads after rebuild
✅ Release builds are optimized and compressed
✅ Clear error messages on build failure
✅ Build time < 3s for incremental dev builds
✅ Build time < 20s for release builds

---

**Ready to implement!** 🚀
