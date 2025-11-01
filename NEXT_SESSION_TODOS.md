# Next Session TODO List

**Date Created**: 2025-11-01
**Priority**: Start implementation of WebGPU renderer

---

## 🔴 CRITICAL: Test Auto-Reload First

### 1. Test Full Auto-Reload Workflow
```bash
cd ~/repos/canvas_3d_6
just start-wasm-open
```

**What to verify:**
- [ ] Server starts on http://localhost:8000
- [ ] Browser opens automatically
- [ ] Page loads (shows "Loading..." or similar)
- [ ] Edit `renderer/src/lib.rs` - change the log message
- [ ] Watch terminal for "File change detected, rebuilding..."
- [ ] Verify build completes (~2-3s)
- [ ] Verify browser auto-reloads (watch page refresh)
- [ ] Check browser console for new log message
- [ ] **IMPORTANT**: Test that shader changes also trigger reload!

**If anything fails**: Debug before proceeding with Milestone 0

---

## 🟡 IMPORTANT: Update Documentation

### 2. Update specs.md
- [ ] Remove/comment out Milestone 1 (Layout Engine) - we're using Chrome positions
- [ ] Update dependencies to wgpu 27.0, wasm-bindgen 0.2.105
- [ ] Add note about WASM build system (reference WASM_BUILD_SYSTEM_COMPLETE.md)
- [ ] Update timeline: ~20-30 hours instead of 10 days
- [ ] Add note about layer-by-layer approach
- [ ] Update "Getting Started" section with `just start-wasm`

---

## 🟢 OPTIONAL: Additional Checks

### 3. Verify Shader Auto-Reload Setup
- [ ] Check if wasm-start watches for shader file changes
- [ ] May need to add shader files to watch list
- [ ] Location TBD: `renderer/src/shaders/` or inline WGSL strings?
- [ ] **Decision needed**: Separate .wgsl files or Rust string constants?

**Recommendation**: Start with inline WGSL in Rust (simpler), move to separate files later if needed

---

## 🚀 START HERE: Milestone 0 - Hello WebGPU

### 4. Implement Colored Triangle
**Goal**: Render a single colored triangle to verify WebGPU pipeline works

**Tasks:**

#### A. Initialize WebGPU (renderer/src/lib.rs)
- [ ] Request GPU adapter
- [ ] Request GPU device
- [ ] Get canvas context
- [ ] Configure surface

**Code location**: `renderer/src/lib.rs` in `start_renderer()` function

#### B. Create Render Pipeline (NEW FILE: renderer/src/pipeline.rs)
- [ ] Create pipeline layout
- [ ] Define vertex buffer layout
- [ ] Set up render pass
- [ ] Configure depth/stencil (if needed)

#### C. Write Shaders (NEW FILE: renderer/src/shaders.rs OR inline in pipeline.rs)
- [ ] Vertex shader (WGSL):
  - Input: position (vec3)
  - Output: position (vec4), color (vec4)
  - Transform to clip space

- [ ] Fragment shader (WGSL):
  - Input: color (vec4)
  - Output: color (vec4)
  - Pass through color

**Example triangle vertices:**
```rust
// Top: red, Left: green, Right: blue
let vertices = [
    [0.0, 0.5, 0.0],   // top
    [-0.5, -0.5, 0.0], // bottom left
    [0.5, -0.5, 0.0],  // bottom right
];
```

#### D. Render Loop
- [ ] Create render pass
- [ ] Set pipeline
- [ ] Set vertex buffer
- [ ] Draw triangle (3 vertices)
- [ ] Submit command buffer

#### E. Test & Verify
- [ ] Run `just start-wasm-open`
- [ ] Should see colored triangle on white canvas
- [ ] Verify it renders correctly
- [ ] Try changing colors - verify auto-reload works

**Success Criteria:**
- ✅ Triangle appears on screen
- ✅ Colors are correct (red/green/blue gradient)
- ✅ No console errors
- ✅ Changing code triggers rebuild + reload

---

## 📝 Files to Create

```
renderer/src/
├── lib.rs          (MODIFY) - Add WebGPU initialization
├── pipeline.rs     (NEW)    - Render pipeline setup
├── shaders.rs      (NEW)    - WGSL shader code (OR inline in pipeline.rs)
└── layout.rs       (EXISTS) - Keep as-is for now
```

---

## 🎯 Milestone 0 Success Metrics

- [ ] Triangle renders correctly
- [ ] No WebGPU errors in console
- [ ] Changing Rust code triggers auto-reload
- [ ] Changing shader code triggers auto-reload (if separate files)
- [ ] Render at 60 FPS (check browser DevTools)

---

## ⏭️ After Milestone 0: Milestone 1

### Load Layout Data
- [ ] Fetch `reference/todomvc_dom_layout.json`
- [ ] Parse into `LayoutData` struct
- [ ] Log element count (should be 45)
- [ ] Display in console for verification

---

## 🛠️ Useful Commands

```bash
# Start dev server with auto-reload
just start-wasm
just start-wasm-open  # Opens browser

# Build only
just build-wasm
just build-wasm-release

# Run tests
just test

# See all commands
just --list
canvas-tools --help
```

---

## 📚 Reference Documentation

- `FINAL_READINESS_ANALYSIS.md` - Complete readiness check
- `WASM_BUILD_SYSTEM_COMPLETE.md` - Build system guide
- `SESSION_SUMMARY.md` - What was done this session
- `specs.md` - Original implementation plan (needs update)
- `reference/todomvc_dom_layout.json` - Layout data (45 elements)

---

## 🔍 Debugging Tips

### If Triangle Doesn't Appear
1. Check browser console for WebGPU errors
2. Verify `navigator.gpu` is defined
3. Check canvas size (should be 1920x1080)
4. Verify shaders compile (WGSL errors show in console)
5. Check render pipeline is valid

### If Auto-Reload Doesn't Work
1. Check server is running on port 8000
2. Verify `/_api/build_id` endpoint works
3. Check browser console for polling errors
4. Verify file watcher is running (see terminal)

### If Build Fails
1. Read error message carefully
2. Check Rust syntax
3. Verify WGSL shader syntax
4. Run `cargo check -p renderer` manually

---

## ⚠️ Important Notes

### WebGPU Shaders
- Use WGSL (not GLSL or SPIR-V)
- Modern syntax: `@vertex`, `@fragment`, `@location(0)`, etc.
- wgpu 27.0 uses latest WGSL spec

### Auto-Reload on Shader Changes
- **If inline Rust strings**: Already works (watches .rs files)
- **If separate .wgsl files**: Need to add to file watcher

**Current watcher watches:**
- `renderer/src/**/*.rs`
- `renderer/Cargo.toml`

**May need to add:**
- `renderer/src/shaders/**/*.wgsl` (if using separate files)

**Location to update**: `tools/src/commands/wasm_start.rs` in `run_watcher()` function

---

## 🎬 Session Start Checklist

When starting next session:
1. [ ] Read FINAL_READINESS_ANALYSIS.md
2. [ ] Read SESSION_SUMMARY.md
3. [ ] Read this file (NEXT_SESSION_TODOS.md)
4. [ ] Test auto-reload workflow
5. [ ] Update specs.md
6. [ ] Start Milestone 0: Hello WebGPU

---

**Everything is ready. Start with testing auto-reload, then begin Milestone 0!** 🚀
