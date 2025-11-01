# AI Agent Guide for Canvas 3D 6

**Last Updated:** 2025-11-01

This document contains essential context for AI agents (Claude Code, etc.) working on this project. Read this FIRST before making changes.

---

## ⚠️ CRITICAL RULES

### 1. Rust-Only Architecture
**NO PYTHON. NO NODE.JS. RUST ONLY.**

See: [`RUST_ONLY_ARCHITECTURE.md`](./RUST_ONLY_ARCHITECTURE.md)

- ❌ Do NOT create `*.py` files
- ❌ Do NOT create `package.json` or use npm/yarn
- ❌ Do NOT suggest Python/Node.js tools
- ✅ Use Rust for ALL tooling (tools crate)
- ✅ Use `cargo install` for external tools (miniserve, just, cargo-watch)
- ✅ All build/dev tools must be Rust

**If you created Python/JavaScript files, DELETE them and rewrite in Rust.**

### 2. WebGPU Requires Chrome Flags

WebGPU will NOT work in Chrome without flags. Always launch Chrome with:

```bash
google-chrome \
  --enable-unsafe-webgpu \
  --enable-webgpu-developer-features \
  --enable-features=Vulkan,VulkanFromANGLE \
  --enable-vulkan \
  --use-angle=vulkan \
  --disable-software-rasterizer \
  --ozone-platform=x11 \
  --remote-debugging-port=9222
```

See: [`docs/CHROME_SETUP.md`](./docs/CHROME_SETUP.md)

**Without these flags, WebGPU falls back to software rendering (SwiftShader) which melts the CPU.**

### 3. CPU Melting Prevention

Previous versions (canvas_3d, canvas_3d_3, canvas_3d_4) all failed due to CPU melting. Root causes:

1. **Software adapter fallback** - ALWAYS check `adapter.isFallbackAdapter === false`
2. **requestAnimationFrame flooding** - TodoMVC is STATIC UI, render on-demand NOT in a loop
3. **JS ↔ Wasm overhead** - Throttle calls across boundary
4. **High DPR without limits** - Cap at ≤1.5 during development
5. **Debug overlays updating every frame** - Disable in production

See: [`PROFILING_STRATEGY.md`](./PROFILING_STRATEGY.md)

**NEVER implement continuous rendering loop for TodoMVC. It's a static UI - render once per change.**

### 4. DOM Layout Reference is Ground Truth

The file `reference/todomvc_dom_layout.json` contains the EXACT positions of all TodoMVC elements.

See: [`reference/LAYOUT_ANALYSIS.md`](./reference/LAYOUT_ANALYSIS.md)

- **Success metric:** <5px error on all elements
- **Do NOT guess positions** - use the JSON data
- **Centering:** Body at x=685px for 1920px viewport: `(1920 - 550) / 2`
- **H1 position:** y=-10 (above viewport, CSS: `top: -140px` from y=130)

### 5. Testing is Required

Before claiming a feature is "done":

1. ✅ Write tests (unit + integration)
2. ✅ Run tests with `cargo test`
3. ✅ Verify output matches expected (use tools crate compare-layouts)
4. ✅ Test in Chrome with WebGPU enabled
5. ✅ Check CPU usage (must be <10% idle)

**No feature is complete without tests.**

**Standard Testing Sizes:**
- **Quick verification:** 700×700px (use this for rapid manual/automated testing)
- **Full reference:** 1920×1080px (matches `reference/todomvc_dom_layout.json`)

**Screenshot commands:**
```bash
# Quick 700x700 verification
cargo run -p tools -- screenshot --url http://localhost:8000 --output /tmp/test.png --width 700 --height 700

# Full 1920x1080 reference comparison
cargo run -p tools -- screenshot --url http://localhost:8000 --output /tmp/full.png --width 1920 --height 1080
```

**Note:** Both `screenshot` and `check-console` commands automatically use required WebGPU flags.

### 6. Self-Verification Before Asking User ⚠️ CRITICAL

**NEVER ask the user to confirm something you can verify yourself.**

Before asking "Can you confirm X works?":

1. ✅ **Take screenshots** yourself using Chrome DevTools Protocol
2. ✅ **Check browser console logs** for errors
3. ✅ **Test endpoints** with curl/fetch
4. ✅ **Verify file contents** match expected state
5. ✅ **Run automated tests** and check results

**Only ask the user if:**
- You genuinely cannot verify something programmatically
- You need user preferences/decisions (not factual verification)
- You hit a blocker that requires human intervention

**Example - WRONG**:
```
❌ "The triangle should now be visible! Can you confirm you see the RGB triangle?"
```

**Example - CORRECT**:
```
✅ Takes screenshot, analyzes pixels, detects triangle colors, reports: "Triangle verified: RGB gradient rendered correctly (screenshot saved)"
```

**Implementation:**
- Browser console checking (Chrome DevTools Protocol)
- Automated screenshot comparison
- Integration tests that verify browser state
- Log parsing and error detection

### 7. Git and Documentation Practices ⚠️ CRITICAL

**NEVER do these things:**

❌ **NO placeholder authors or bot identities**
```bash
# WRONG - placeholder emails
user.email = "canvas.bot@example.com"
user.email = "ai@example.com"

# WRONG - mentioning AI in author
Author: Claude <claude@anthropic.com>
Co-Authored-By: Claude Code <noreply@anthropic.com>

# CORRECT - use the user's real identity
user.name = "Martin Kavík"
user.email = "martin@kavik.cz"
```

❌ **NO time estimates ANYWHERE**
```markdown
WRONG: "Implement feature X (2 hours)"
WRONG: "Should take about 30 minutes"
WRONG: "Time estimate: 1-2 hours"
WRONG: "Quick fix (15 minutes)"
WRONG: "This will take approximately..."
CORRECT: "Implement feature X"
CORRECT: "Fix viewport sizing issue"
```

**Why NO time estimates?**
1. **They're always wrong** - Software estimation is fundamentally unreliable
2. **False expectations** - Users plan around estimates that won't be met
3. **Wasted effort** - Time spent estimating could be spent implementing
4. **Pressure and stress** - Creates artificial deadlines
5. **No accountability** - There's no consequence for wrong estimates

**Instead of estimates:**
- Break tasks into smaller pieces
- Report actual progress continuously
- Focus on completion, not prediction

❌ **NO dates or timestamps in documentation**
```markdown
WRONG: "Last updated: 2025-11-01"
WRONG: "Written on November 1, 2025"
WRONG: "Created at 23:45"
WRONG: "As of 2025-11-01..."
WRONG: "Date: 2025-11-01"
CORRECT: No date/time stamps in content (git history tracks this)
CORRECT: "Current state" (without date)
CORRECT: "Latest implementation" (without timestamp)
```

**Why NO dates?**
1. **Documentation rot** - Content appears "outdated" even when accurate
2. **Maintenance burden** - Dates must be updated manually
3. **Git exists** - `git log` provides accurate temporal information
4. **False staleness** - Good docs look old, misleading readers
5. **No value** - Dates don't indicate correctness or relevance

**Exception:** Timestamps ARE allowed ONLY in:
- CLAUDE.md header (AI context freshness indicator)
- CHANGELOG.md entries (changelog format requirement)
- Reference metadata (screenshot/extraction provenance)
- Test data fixtures (known timestamps for reproducibility)

**NOT allowed in:**
- Technical specifications
- Implementation docs
- Architecture documents
- User guides
- Task lists
- Reports
- Analysis documents

**When committing:**
- Use `jj config list | grep user` to verify your identity
- If you see a placeholder, run: `jj config set --user user.email "your@email.com"`
- Fix commit authors with: `jj metaedit --update-author`

---

## 📂 Project Structure

```
canvas_3d_6/
├── Cargo.toml                    # Workspace root (tools + renderer)
├── CLAUDE.md                     # THIS FILE - read first!
├── specs.md                      # Full technical specification
├── RUST_ONLY_ARCHITECTURE.md     # Why and how Rust-only
├── PROFILING_STRATEGY.md         # CPU melting prevention
├── WORKFLOW_ANALYSIS.md          # Critical insights from failures
├── NEXT_STEPS.md                 # Immediate action items
│
├── docs/
│   ├── CHROME_SETUP.md           # WebGPU flags and setup
│   └── DOM_EXTRACTION.md         # How to extract layout data
│
├── reference/
│   ├── todomvc_dom_layout.json   # GROUND TRUTH - all element positions
│   ├── LAYOUT_ANALYSIS.md        # Human-readable layout breakdown
│   ├── todomvc_chrome_reference.png  # Visual ground truth
│   ├── REFERENCE_METADATA.md     # Screenshot metadata
│   └── todomvc_populated.html    # Static HTML for testing
│
├── renderer/                     # WASM WebGPU renderer crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                # Entry point, WebGPU init, main render loop
│       ├── layout.rs             # Layout data types (shared with tools)
│       ├── pipeline.rs           # Triangle pipeline (demo/debug)
│       ├── rectangle_pipeline.rs # Rectangle rendering pipeline
│       ├── border_pipeline.rs    # Border rendering pipeline
│       ├── textured_quad_pipeline.rs # Textured quad for text rendering
│       └── text_renderer.rs      # Canvas2D text-to-texture renderer
│
├── web/                          # Web assets (served by wasm-start)
│   ├── index.html                # Main HTML with WASM bootstrap
│   └── pkg/                      # Auto-generated WASM + JS bindings
│       ├── renderer_bg.wasm      # Compiled WASM binary
│       ├── renderer.js           # JS bindings (wasm-bindgen output)
│       └── renderer.d.ts         # TypeScript definitions
│
├── tools/                        # Rust dev tools crate (canvas-tools CLI)
│   ├── Cargo.toml
│   ├── README.md                 # Full command documentation
│   └── src/
│       ├── main.rs               # CLI entry point
│       ├── commands/
│       │   ├── mod.rs            # Command registry
│       │   ├── extract_dom.rs    # ✅ Extracts layout from CSS analysis
│       │   ├── compare_layouts.rs # ✅ Compare reference vs actual
│       │   ├── visualize_layout.rs # ✅ Generate HTML visualization
│       │   ├── serve.rs          # ✅ HTTP server for static files
│       │   ├── screenshot.rs     # ✅ Chrome CDP screenshots (with WebGPU flags)
│       │   ├── integration_test.rs # ✅ Full workflow integration tests
│       │   ├── check_console.rs  # ✅ Console monitoring via CDP (in cdp module)
│       │   ├── wasm_build.rs     # ✅ WASM build orchestration
│       │   └── wasm_start.rs     # ✅ Dev server with auto-reload
│       ├── layout/
│       │   └── mod.rs            # Layout data types (LayoutData, Element, etc.)
│       ├── cdp/
│       │   └── mod.rs            # Chrome DevTools Protocol helpers
│       ├── wasm_bindgen.rs       # wasm-bindgen wrapper
│       └── wasm_opt.rs           # wasm-opt wrapper
│
└── target/                       # Build artifacts
    ├── wasm32-unknown-unknown/   # WASM build output
    │   ├── debug/
    │   └── release/
    └── debug/                    # Native (tools) build output
```

---

## 🎯 Current Status (2025-11-01)

### ✅ Completed

1. **Workspace setup** - Cargo.toml with tools crate
2. **Layout types** - Full `LayoutData`/`Element` structs with serde
3. **extract-dom command** - Generates reference layout JSON (723 lines, 45 elements)
4. **Documentation** - Comprehensive specs, architecture, profiling, workflow docs
5. **Reference data** - todomvc_dom_layout.json with all positions
6. **screenshot command** - Uses chromiumoxide with WebGPU flags (700×700 and 1920×1080)
7. **check-console command** - Browser console monitoring with WebGPU support
8. **wasm-build command** - Full WASM build pipeline with wasm-bindgen
9. **wasm-start command** - Dev server with file watching and auto-reload
10. **WebGPU automation** - All CDP tools apply required Chrome flags automatically

### 🚧 In Progress

1. **Tests** - Need to write tests for extract-dom, screenshot, check-console
2. **compare-layouts** - Stub exists, needs implementation
3. **visualize-layout** - Stub exists, needs implementation

### ⏳ TODO

1. **Implement remaining tools commands:**
   - compare-layouts (port from Python)
   - visualize-layout (port from Python)
   - serve (HTTP server for static files)
   - watch (file watcher for custom commands)

2. **Write comprehensive tests:**
   - tools/src/layout/mod.rs - test LayoutData methods
   - tools/src/commands/extract_dom.rs - test output correctness
   - tools/src/commands/compare_layouts.rs - test error calculations
   - Integration tests for full workflows

3. **Remove Python tools:**
   - Delete tools_python_old/ directory
   - Update all documentation references
   - Update RUST_ONLY_ARCHITECTURE.md

4. **Add cargo-watch to workflow:**
   - Make it required (not optional)
   - Add to Justfile
   - Document in RUST_ONLY_ARCHITECTURE.md

5. **Implement renderer (Milestones 0-4):**
   - M0: Hello WebGPU (clear screen)
   - M1: Layout engine
   - M2: Render boxes
   - M3: Text rendering (Canvas2D hybrid)
   - M4: Complete TodoMVC

---

## 🔧 Development Workflow

### Quick Start (Development Mode)

The fastest way to start developing with auto-reload:

```bash
# Start dev server with file watching and auto-reload
cargo run -p tools -- wasm-start

# Or with browser auto-open:
cargo run -p tools -- wasm-start --open
```

This command:
1. Builds the WASM renderer (debug mode, fast compilation)
2. Generates JS bindings with wasm-bindgen
3. Starts HTTP server on http://localhost:8000
4. Watches `renderer/src/` for file changes
5. Auto-rebuilds and triggers browser reload on changes

**The auto-reload workflow is CRITICAL for rapid iteration!**

### Build and Run

```bash
# Build tools crate
cargo build --release -p tools

# Build WASM renderer
cargo run -p tools -- wasm-build

# Run extract-dom command
cargo run -p tools -- extract-dom --output reference/todomvc_dom_layout.json

# Take screenshot for verification
cargo run -p tools -- screenshot --url http://localhost:8000 --output /tmp/screenshot.png --width 1920

# Check browser console for errors
cargo run -p tools -- check-console --url http://localhost:8000

# Run tests
cargo test --all
```

### Using `just` (recommended)

Create a `Justfile` in the project root:

```makefile
# Extract DOM layout
extract-dom:
    cargo run --release -p tools -- extract-dom --output reference/todomvc_dom_layout.json

# Compare layouts
compare:
    cargo run --release -p tools -- compare-layouts \
        --reference reference/todomvc_dom_layout.json \
        --actual output/renderer_layout.json

# Run tests
test:
    cargo test --all

# Build everything
build:
    cargo build --release --all
```

Then run: `just extract-dom`, `just test`, etc.

---

## 🧪 Testing Strategy

### What to Test

1. **Layout extraction accuracy:**
   - Element count (must be 45)
   - H1 position (x=685, y=-10)
   - Body centering (x=685)
   - Todo item spacing (58px apart)
   - All element positions match expected

2. **Layout comparison logic:**
   - Euclidean distance calculation
   - Error threshold detection (5px)
   - Missing/extra element detection
   - Summary statistics correctness

3. **Integration tests:**
   - Extract → Compare → Verify workflow
   - File I/O correctness
   - JSON serialization/deserialization
   - Error handling (missing files, invalid JSON)

### Test Data Fixtures

Create `tools/tests/fixtures/`:
- `test_layout_valid.json` - Valid layout data
- `test_layout_shifted.json` - Same layout but shifted +10px
- `test_layout_missing.json` - Missing elements
- `test_layout_extra.json` - Extra elements

### Running Tests

```bash
# All tests
cargo test --all

# Specific test
cargo test --package tools --test integration_tests

# With output
cargo test -- --nocapture

# Watch tests (with cargo-watch)
cargo watch -x test
```

---

## 🐛 Common Pitfalls

### 1. Forgetting Rust-Only Rule

**Symptom:** You create a `script.py` or `package.json`

**Fix:** Stop immediately. Delete the file. Implement in Rust using the tools crate.

### 2. Missing Chrome WebGPU Flags

**Symptom:** "WebGPU not supported" or CPU at 100%

**Fix:** Launch Chrome with ALL required flags (see CHROME_SETUP.md)

### 3. Continuous Rendering Loop

**Symptom:** CPU at 100% even when page is idle

**Fix:** Remove `requestAnimationFrame` loop. TodoMVC is static - render on-demand only.

### 4. Not Checking Reference JSON

**Symptom:** Layout positions are "close but not exact"

**Fix:** Use `reference/todomvc_dom_layout.json` as ground truth. Don't guess positions.

### 5. No Tests

**Symptom:** "Feature works on my machine" but breaks later

**Fix:** Write tests BEFORE claiming feature is done. Use `cargo test`.

### 6. Software Adapter Fallback

**Symptom:** WebGPU works but PC is melting

**Fix:** Check `adapter.isFallbackAdapter === false` in JavaScript. If true, abort and show error.

---

## 📖 Key Documents to Read

### Must Read (in order):

1. **THIS FILE** (`AGENTS.md`) - Overview and rules
2. **`specs.md`** - Full technical specification
3. **`RUST_ONLY_ARCHITECTURE.md`** - Rust-only rationale
4. **`WORKFLOW_ANALYSIS.md`** - Why previous attempts failed
5. **`PROFILING_STRATEGY.md`** - CPU melting prevention
6. **`reference/LAYOUT_ANALYSIS.md`** - Ground truth layout data

### Reference When Needed:

- `docs/CHROME_SETUP.md` - When setting up Chrome
- `docs/DOM_EXTRACTION.md` - When extracting layout data
- `NEXT_STEPS.md` - Current priorities
- `reference/REFERENCE_METADATA.md` - Screenshot details

---

## 🤖 Working Autonomously

As an AI agent, you should be able to develop, test, and verify WITHOUT human intervention. Here's how:

### 1. Development Loop

```bash
# Terminal 1: Watch and auto-rebuild
cargo watch -x "build --release"

# Terminal 2: Auto-test on changes
cargo watch -x test

# Terminal 3: Run commands as needed
./target/release/canvas-tools extract-dom --output /tmp/test.json
```

### 2. Verification Workflow

After implementing a feature:

```bash
# 1. Build
cargo build --release

# 2. Run command
./target/release/canvas-tools extract-dom --output /tmp/actual.json

# 3. Compare with reference (once compare-layouts is implemented)
./target/release/canvas-tools compare-layouts \
  --reference reference/todomvc_dom_layout.json \
  --actual /tmp/actual.json

# 4. Run tests
cargo test --all

# 5. Check for success (exit code 0)
echo $?  # Must be 0
```

### 3. Self-Validation Checklist

Before marking a task as "done", verify:

- [ ] Code compiles without warnings
- [ ] All tests pass (`cargo test`)
- [ ] Documentation updated (if API changed)
- [ ] No Python/Node.js files created
- [ ] Output verified against reference data
- [ ] CPU usage acceptable (<10% idle)
- [ ] Changes committed (if using git)

### 4. Tools Required for Autonomy

Install these via `cargo install`:

```bash
cargo install cargo-watch    # Auto-rebuild/test
cargo install just            # Command runner
cargo install miniserve       # HTTP server
cargo install wasm-bindgen-cli # Wasm bindings
```

Also install (system package manager):
```bash
sudo apt install binaryen      # wasm-opt
sudo apt install google-chrome # For WebGPU testing
```

---

## 🎓 Learning From Failures

### canvas_3d (Original Rust)
- ❌ Melted CPU due to software adapter fallback
- ❌ Continuous rAF loop for static UI
- ❌ Too much JS ↔ Wasm communication
- ✅ WebGPU worked when configured correctly

### canvas_3d_3 (Rust SDF)
- ❌ High CPU usage
- ❌ Difficult to test/profile in Chrome
- ✅ SDF approach was interesting but overcomplicated

### canvas_3d_4 (Zig + Vulkan)
- ❌ Stopped working with TodoMVC
- ✅ Was very fast when it worked
- ❌ Harder to iterate than Rust

### canvas_3d_6 (Current - Rust WebGPU)
- ✅ Rust-only toolchain for portability
- ✅ Comprehensive specs and docs
- ✅ DOM layout reference data
- ✅ CPU melting prevention strategies
- ✅ Clear success metrics (<5px error)
- 🚧 Still in development

**Key Lesson:** Don't just write code - measure, test, compare, and iterate.

---

## 💡 Development Philosophy

### 1. Measure, Don't Guess

- Use `reference/todomvc_dom_layout.json` for positions
- Use `compare-layouts` tool to measure accuracy
- Use Chrome DevTools Performance tab to profile CPU
- Use `cargo test` to verify correctness

### 2. Test-Driven Development

1. Write test first (RED)
2. Implement feature (GREEN)
3. Refactor if needed (REFACTOR)
4. Verify with compare-layouts tool

### 3. Iterative Refinement

TodoMVC V1 goal: **Layout + Text only**

- ❌ Don't implement colors yet
- ❌ Don't implement shadows yet
- ❌ Don't implement animations yet
- ✅ Focus on <5px positioning accuracy
- ✅ Focus on readable text rendering
- ✅ Focus on preventing CPU melt

**Ship V1, then iterate.**

### 4. Documentation as Code

- Update docs when changing behavior
- Write clear commit messages
- Add comments for complex logic
- Keep AGENTS.md up-to-date

### 5. Integration Tests Before Implementation ⚠️ CRITICAL

**ALWAYS follow this order:**

1. **Prepare Tooling FIRST**
   - Build automation (file watchers, auto-reload, etc.)
   - Prepare testing infrastructure
   - Create integration test scenarios
   - Document expected behavior

2. **Test Tooling End-to-End**
   - Run COMPLETE workflow tests
   - Verify automation actually works
   - Test edge cases (build errors, file changes, etc.)
   - Document any issues found

3. **Fix Issues Before Proceeding**
   - Don't skip broken tooling
   - Don't work around automation failures
   - Fix properly, test again, verify

4. **ONLY THEN Implement Features**
   - With working tooling, development is fast
   - With broken tooling, every change is painful
   - Working automation = confidence in changes

**Example from This Session:**
```
❌ WRONG APPROACH (what happened):
- Implemented WebGPU triangle rendering ✓
- Auto-reload "seems" to work ✓
- Declare Milestone 0 complete ✗
- Discover server doesn't actually run ✗
- Discover browser selection broken ✗
- Can't properly test the feature ✗

✅ CORRECT APPROACH (what should have happened):
1. Build wasm-start server + file watcher
2. Test complete workflow:
   - Edit .rs file → build → Chrome reloads
   - Edit .wgsl file → build → Chrome reloads
   - Edit Cargo.toml → build → Chrome reloads
   - Build error → no reload (stays on old version)
3. Fix all issues found (server binding, browser selection)
4. Re-test until ALL scenarios work
5. Document working workflow
6. THEN implement WebGPU triangle
7. Verify changes trigger auto-reload
8. Declare Milestone 0 complete ✓
```

**Integration Test Checklist:**

Before declaring infrastructure "ready":
- [ ] Test complete workflow end-to-end
- [ ] Test with actual file changes (not just theory)
- [ ] Test error conditions (build failures, missing files)
- [ ] Test browser automation (correct browser opens)
- [ ] Test auto-reload triggers correctly
- [ ] Test debouncing (rapid changes don't spam rebuilds)
- [ ] Document all test scenarios
- [ ] Fix any issues found
- [ ] Re-test after fixes
- [ ] Get user confirmation it works

**Never say "it should work" - TEST IT AND VERIFY!**

---

## 🔗 External Resources

### WebGPU
- [WebGPU Spec](https://www.w3.org/TR/webgpu/)
- [wgpu Rust crate](https://docs.rs/wgpu/)
- [WebGPU Samples](https://webgpu.github.io/webgpu-samples/)

### Layout/Flexbox
- [Flexbox Spec](https://www.w3.org/TR/css-flexbox-1/)
- [Taffy Layout Engine](https://github.com/DioxusLabs/taffy) (for reference)

### Testing
- [Rust Book - Testing](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [cargo test docs](https://doc.rust-lang.org/cargo/commands/cargo-test.html)

### Chrome DevTools Protocol
- [CDP Overview](https://chromedevtools.github.io/devtools-protocol/)
- [headless_chrome crate](https://docs.rs/headless_chrome/)

---

## 📝 Quick Reference

### File Paths
- Layout reference: `reference/todomvc_dom_layout.json`
- Screenshot reference: `reference/todomvc_chrome_reference.png`
- Tools binary: `target/release/canvas-tools`
- Main spec: `specs.md`

### Key Numbers
- Viewport: 1920×1080
- Body width: 550px
- Body x-offset: 685px (centered)
- H1 y-position: -10px
- Todo item height: 58px
- Success threshold: <5px error

### Commands
```bash
# Extract layout
cargo run -p tools -- extract-dom -o reference/todomvc_dom_layout.json

# Compare (once implemented)
cargo run -p tools -- compare-layouts -r reference/todomvc_dom_layout.json -a output/actual.json

# Test
cargo test --all

# Build
cargo build --release --all
```

---

## ✅ Next Actions for AI Agents

When you start working on this project:

1. **Read this file completely** ✓ You're doing it!
2. **Read `specs.md`** - Understand the full scope
3. **Read `RUST_ONLY_ARCHITECTURE.md`** - Understand constraints
4. **Run `cargo build --release`** - Verify everything compiles
5. **Run `./target/release/canvas-tools extract-dom -o /tmp/test.json`** - Verify tools work
6. **Pick a task from TODO section** - Start implementing
7. **Write tests first** - TDD approach
8. **Verify with tools** - compare-layouts, cargo test
9. **Update AGENTS.md** - Keep this doc current

---

## 🆘 When You're Stuck

1. **Check existing docs** - Answer might be in specs.md or other docs
2. **Read error messages carefully** - Rust errors are helpful
3. **Run tests** - `cargo test` might reveal the issue
4. **Check reference data** - Are you using todomvc_dom_layout.json?
5. **Verify Chrome flags** - WebGPU issues? Check CHROME_SETUP.md
6. **Ask the user** - If truly stuck, ask for clarification

---

## 📅 Version History

- **2025-11-01:** Initial version
  - Created Rust tools crate
  - Implemented extract-dom command
  - Documented Rust-only architecture
  - Established testing requirements

---

**Remember:** This project is Rust-only. No Python. No Node.js. Test everything. Prevent CPU melting. Use reference data. Ship V1.
