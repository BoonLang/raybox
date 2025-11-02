# Comprehensive Plan: Rename canvas_3d_6 → raybox

**Goal:** Rename project from `canvas_3d_6` to `raybox` systematically without breaking anything.

**Strategy:** Update files from inside-out (content → metadata → directory) to maintain consistency.

---

## Phase 1: Update Documentation and Comments

### Files to Update

#### 1. CLAUDE.md
**Changes:**
- Line references to `/home/martinkavik/repos/canvas_3d_6` → `/home/martinkavik/repos/raybox`
- Project structure paths
- Example commands with paths
- Git status references

**Search patterns:**
- `canvas_3d_6`
- `canvas.3d.6` (if any)

#### 2. README.md
**Changes:**
- Project title
- Description references
- Installation paths
- Usage examples

#### 3. specs.md
**Changes:**
- Project name in title
- References to `canvas_3d_6` in examples
- Path references

#### 4. WORKFLOW_ANALYSIS.md
**Changes:**
- Project name references
- Historical context (canvas_3d → canvas_3d_3 → canvas_3d_4 → canvas_3d_6 → **raybox**)

#### 5. RUST_ONLY_ARCHITECTURE.md
**Changes:**
- Project name references
- Path examples

#### 6. docs/CHROME_SETUP.md
**Changes:**
- Path references in examples

#### 7. docs/DOM_EXTRACTION.md
**Changes:**
- Path references in examples

#### 8. reference/REFERENCE_METADATA.md
**Changes:**
- Source path references

#### 9. web/reference/REFERENCE_METADATA.md
**Changes:**
- Source path references (if different from above)

---

## Phase 2: Update Cargo.toml Files

### 1. tools/Cargo.toml

**Current:**
```toml
[[bin]]
name = "canvas-tools"
path = "src/main.rs"
```

**New:**
```toml
[[bin]]
name = "raybox-tools"
path = "src/main.rs"
```

### 2. Cargo.toml (workspace root)

**Current:**
```toml
[workspace.package]
version = "0.1.0"
edition = "2021"
authors = ["TodoMVC Renderer Team"]
license = "MIT OR Apache-2.0"
```

**New:**
```toml
[workspace.package]
version = "0.1.0"
edition = "2021"
authors = ["Raybox Team"]
license = "MIT OR Apache-2.0"
```

**Note:** `renderer` package name stays the same (it's a library crate).

---

## Phase 3: Update Source Code References

### 1. tools/README.md

**Search and replace:**
- `canvas-tools` → `raybox-tools`
- References to project name in descriptions

**Example:**
```bash
# OLD
cargo run -p tools -- wasm-build

# STAYS THE SAME (package name is "tools", not "canvas-tools")
cargo run -p tools -- wasm-build

# But binary name changes:
# OLD: ./target/release/canvas-tools
# NEW: ./target/release/raybox-tools
```

### 2. tools/src/main.rs

**Check for:**
- Hard-coded references to "canvas-tools" in help text
- Program name in CLI definitions

**Current (likely):**
```rust
#[derive(Parser)]
#[command(name = "canvas-tools")]
// ...
```

**New:**
```rust
#[derive(Parser)]
#[command(name = "raybox-tools")]
// ...
```

### 3. web/index.html

**Current:**
```html
<title>TodoMVC - Canvas WebGPU Renderer</title>
```

**New:**
```html
<title>TodoMVC - Raybox WebGPU Renderer</title>
```

### 4. Justfile

**Search and replace:**
- `canvas-tools` → `raybox-tools` in all commands

**Example:**
```makefile
# OLD
extract-dom:
    cargo run --release -p tools -- extract-dom --output reference/todomvc_dom_layout.json

# NEW (stays the same, package name is still "tools")
extract-dom:
    cargo run --release -p tools -- extract-dom --output reference/todomvc_dom_layout.json
```

**But if there are direct binary calls:**
```makefile
# OLD
./target/release/canvas-tools screenshot ...

# NEW
./target/release/raybox-tools screenshot ...
```

---

## Phase 4: Build and Verify

### Commands to run:

```bash
# Clean old artifacts
cargo clean

# Build all packages
cargo build --all

# Verify tools binary name
ls -la target/debug/
# Should show: raybox-tools (not canvas-tools)

# Test tools help
cargo run -p tools -- --help
# Should show: "raybox-tools" in output

# Build release
cargo build --release -p tools

# Test a command
cargo run -p tools -- wasm-build
```

**Expected:**
- ✅ Binary created as `raybox-tools`
- ✅ All commands work
- ✅ Help text shows "raybox-tools"

---

## Phase 5: Rename Directory (LAST STEP!)

**⚠️ CRITICAL: Do this ONLY after all file contents are updated!**

### Commands:

```bash
# From OUTSIDE the directory
cd /home/martinkavik/repos

# Rename
mv canvas_3d_6 raybox

# Enter new directory
cd raybox

# Verify jj still works
jj st
# Should show: Working copy changes (if any)

# Verify git reference (jj uses git backend)
jj log --limit 5
```

**What happens to .jj directory:**
- ✅ Moves with the directory (no changes needed)
- ✅ All commit history preserved
- ✅ Working copy state preserved

---

## Phase 6: Post-Rename Verification

### Checklist:

```bash
# 1. Verify jj repository
jj st
jj log --limit 3

# 2. Verify build works
cargo clean
cargo build --all

# 3. Test tools
cargo run -p tools -- --help

# 4. Test WASM workflow
cargo run -p tools -- wasm-build
cargo run -p tools -- wasm-start --open

# 5. Verify all references updated
grep -r "canvas_3d_6" . --exclude-dir=target --exclude-dir=.jj
# Should return: NO MATCHES (except maybe in git history)

grep -r "canvas-3d-6" . --exclude-dir=target --exclude-dir=.jj
# Should return: NO MATCHES

# 6. Check binary name
ls -la target/debug/ | grep raybox
# Should show: raybox-tools

ls -la target/debug/ | grep canvas
# Should show: NOTHING
```

---

## Phase 7: Commit Changes

```bash
# Stage all changes (jj doesn't have staging, so just commit)
jj commit -m "Rename project from canvas_3d_6 to raybox

- Update all documentation (CLAUDE.md, README.md, specs.md, etc.)
- Rename tools binary: canvas-tools → raybox-tools
- Update Cargo.toml workspace metadata
- Update web/index.html title
- Update all path references in documentation
- Rename project directory: canvas_3d_6 → raybox

Rationale: Aligns with future direction as SDF-based CAD tool with
raymarching 3D viewport. See RENDER_RESEARCH.md for full context."
```

---

## File Changes Summary

### Files to Modify (Content Only)

**Documentation:**
1. CLAUDE.md - ~50 path references
2. README.md - Title, descriptions, examples
3. specs.md - Project references
4. WORKFLOW_ANALYSIS.md - Historical context
5. RUST_ONLY_ARCHITECTURE.md - Examples
6. docs/CHROME_SETUP.md - Path examples
7. docs/DOM_EXTRACTION.md - Path examples
8. reference/REFERENCE_METADATA.md - Source paths
9. tools/README.md - Binary name, examples

**Configuration:**
10. Cargo.toml (root) - workspace.package.authors
11. tools/Cargo.toml - [[bin]].name
12. Justfile - Binary references (if any)

**Source Code:**
13. tools/src/main.rs - CLI command name
14. web/index.html - Page title

### Files to Rename

**None inside project** (package names stay the same)

### Directory to Rename

**One:**
- `/home/martinkavik/repos/canvas_3d_6` → `/home/martinkavik/repos/raybox`

---

## Rollback Plan (If Something Breaks)

If rename causes issues:

```bash
# 1. Go back to parent directory
cd /home/martinkavik/repos

# 2. Rename back
mv raybox canvas_3d_6

# 3. Use jj to undo file changes
cd canvas_3d_6
jj undo  # Undo last commit

# 4. Verify
jj st
cargo build --all
```

---

## Package Names vs Binary Names vs Directory Names

**Important distinctions:**

1. **Package names** (in Cargo.toml `[package].name`):
   - `tools` (stays the same)
   - `renderer` (stays the same)
   - Used in: `cargo build -p tools`

2. **Binary names** (in Cargo.toml `[[bin]].name`):
   - `canvas-tools` → `raybox-tools` ✏️ CHANGES
   - Used in: `target/debug/raybox-tools`

3. **Directory name**:
   - `canvas_3d_6` → `raybox` ✏️ CHANGES
   - Used in: file paths, documentation

4. **Crate name in code** (for libraries):
   - `renderer` (stays the same)
   - Used in: `use renderer::...`

---

## Estimated Time

- **Phase 1 (Documentation):** 30 minutes
- **Phase 2 (Cargo.toml):** 5 minutes
- **Phase 3 (Source code):** 10 minutes
- **Phase 4 (Build/Verify):** 10 minutes
- **Phase 5 (Rename dir):** 2 minutes
- **Phase 6 (Post-verify):** 10 minutes
- **Phase 7 (Commit):** 5 minutes

**Total:** ~1 hour

---

## Search Patterns for Verification

After renaming, search for these patterns to ensure completeness:

```bash
# Should find NOTHING (except in .jj history):
grep -r "canvas_3d_6" . --exclude-dir=target --exclude-dir=.jj --exclude-dir=node_modules
grep -r "canvas-3d-6" . --exclude-dir=target --exclude-dir=.jj --exclude-dir=node_modules
grep -r "canvas.3d.6" . --exclude-dir=target --exclude-dir=.jj --exclude-dir=node_modules

# Should find NOTHING:
grep -r "canvas-tools" . --exclude-dir=target --exclude-dir=.jj --exclude-dir=node_modules
# (Except maybe in this RENAME_PLAN.md documentation)

# Should find SOMETHING:
grep -r "raybox" . --exclude-dir=target --exclude-dir=.jj --exclude-dir=node_modules
grep -r "raybox-tools" . --exclude-dir=target --exclude-dir=.jj --exclude-dir=node_modules
```

---

## Additional Considerations

### 1. GitHub Repository (if exists)

If project is on GitHub:
- Repository name can be changed in Settings → Rename
- GitHub automatically creates redirect from old name
- Local git remotes will continue to work

### 2. External Documentation

Check for references in:
- Blog posts
- README links
- Wiki pages
- Issue trackers

### 3. Build Artifacts

After rename:
```bash
# Clean all old artifacts
cargo clean
rm -rf web/pkg/  # Old WASM bindings
rm -rf target/   # All build artifacts

# Rebuild from scratch
cargo build --all
cargo run -p tools -- wasm-build
```

### 4. IDE/Editor Configuration

If using:
- **VS Code**: `.vscode/` settings might have project paths → update
- **RustRover/IntelliJ**: `.idea/` settings → reimport project
- **vim/neovim**: Local config might have paths → update

---

## Success Criteria

Rename is complete when:

- [ ] All documentation references updated
- [ ] Binary name is `raybox-tools` (not `canvas-tools`)
- [ ] Directory is `/home/martinkavik/repos/raybox`
- [ ] `cargo build --all` succeeds
- [ ] `cargo run -p tools -- --help` shows "raybox-tools"
- [ ] `jj st` works (repository intact)
- [ ] `grep -r "canvas_3d_6"` finds nothing (except git history)
- [ ] `grep -r "canvas-tools"` finds nothing
- [ ] All tests pass: `cargo test --all`
- [ ] WASM workflow works: `wasm-build` + `wasm-start`
- [ ] Changes committed to jj repository

---

## Notes

- **Package names** (`tools`, `renderer`) do NOT change
  - This means `cargo run -p tools` still works
  - Only the binary output name changes

- **Import statements** in Rust do NOT change
  - `use renderer::...` stays the same

- **WASM output** name does NOT change
  - Still `renderer.js`, `renderer_bg.wasm`
  - Because package name is still `renderer`

- **JJ repository** is safe
  - Lives in `.jj/` directory
  - Moves with directory rename
  - No configuration changes needed

---

**END OF RENAME PLAN**
