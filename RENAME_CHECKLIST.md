# Raybox Rename - Quick Execution Checklist

**Use this checklist while executing the rename. Check off items as you complete them.**

---

## Pre-Rename Verification

- [ ] Current directory: `/home/martinkavik/repos/canvas_3d_6`
- [ ] `jj st` shows clean working copy (or commit current work)
- [ ] `cargo build --all` succeeds
- [ ] All tests pass: `cargo test --all`

---

## Phase 1: Documentation Updates

- [ ] Update `CLAUDE.md`
  - [ ] `/home/martinkavik/repos/canvas_3d_6` → `/home/martinkavik/repos/raybox`
  - [ ] Project structure paths
  - [ ] Example commands

- [ ] Update `README.md`
  - [ ] Project title
  - [ ] Descriptions

- [ ] Update `specs.md`
  - [ ] Project references

- [ ] Update `WORKFLOW_ANALYSIS.md`
  - [ ] Add raybox to evolution chain

- [ ] Update `RUST_ONLY_ARCHITECTURE.md`
  - [ ] Path references

- [ ] Update `docs/CHROME_SETUP.md`
  - [ ] Path examples

- [ ] Update `docs/DOM_EXTRACTION.md`
  - [ ] Path examples

- [ ] Update `reference/REFERENCE_METADATA.md`
  - [ ] Source paths

- [ ] Update `tools/README.md`
  - [ ] `canvas-tools` → `raybox-tools`
  - [ ] Binary examples

---

## Phase 2: Configuration Files

- [ ] Update `Cargo.toml` (root)
  ```toml
  authors = ["Raybox Team"]
  ```

- [ ] Update `tools/Cargo.toml`
  ```toml
  [[bin]]
  name = "raybox-tools"
  ```

- [ ] Update `Justfile` (if it has direct binary references)
  - [ ] `canvas-tools` → `raybox-tools`

---

## Phase 3: Source Code

- [ ] Update `tools/src/main.rs`
  - [ ] CLI command name: `#[command(name = "raybox-tools")]`

- [ ] Update `web/index.html`
  - [ ] `<title>TodoMVC - Raybox WebGPU Renderer</title>`

---

## Phase 4: Build & Verify (Before Directory Rename)

- [ ] `cargo clean`
- [ ] `cargo build --all` ✅ succeeds
- [ ] Check binary: `ls target/debug/raybox-tools` ✅ exists
- [ ] Check old binary gone: `ls target/debug/canvas-tools` ❌ doesn't exist
- [ ] `cargo run -p tools -- --help` shows "raybox-tools"
- [ ] `cargo test --all` ✅ passes

---

## Phase 5: Directory Rename

⚠️ **CRITICAL: Do this step ONLY after all above is complete!**

```bash
# Exit the directory
cd /home/martinkavik/repos

# Rename
mv canvas_3d_6 raybox

# Enter new location
cd raybox

# Verify jj still works
jj st
```

- [ ] Directory renamed
- [ ] `jj st` works
- [ ] Working copy clean (or shows expected changes)

---

## Phase 6: Post-Rename Verification

- [ ] `cargo clean`
- [ ] `cargo build --all` ✅ succeeds
- [ ] `cargo run -p tools -- --help` ✅ shows "raybox-tools"
- [ ] `cargo run -p tools -- wasm-build` ✅ works
- [ ] `cargo test --all` ✅ passes

---

## Phase 7: Verify No Old References

Run these searches (should find NOTHING):

```bash
grep -r "canvas_3d_6" . --exclude-dir=target --exclude-dir=.jj --exclude-dir=node_modules
grep -r "canvas-3d-6" . --exclude-dir=target --exclude-dir=.jj --exclude-dir=node_modules
grep -r "canvas-tools" . --exclude-dir=target --exclude-dir=.jj --exclude-dir=node_modules
```

- [ ] No `canvas_3d_6` references (except this file and RENAME_PLAN.md)
- [ ] No `canvas-tools` references (except this file and RENAME_PLAN.md)

Run this search (should find MANY):

```bash
grep -r "raybox" . --exclude-dir=target --exclude-dir=.jj --exclude-dir=node_modules
```

- [ ] Found "raybox" in multiple files ✅

---

## Phase 8: Final Workflow Test

- [ ] Start dev server: `cargo run -p tools -- wasm-start`
- [ ] Browser opens at `http://localhost:8000`
- [ ] TodoMVC renders correctly
- [ ] Page title shows "TodoMVC - Raybox WebGPU Renderer"
- [ ] Console shows no errors

---

## Phase 9: Commit

```bash
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

- [ ] Changes committed
- [ ] `jj log` shows commit

---

## Post-Commit Verification

- [ ] `jj st` shows clean working copy
- [ ] `cargo build --release --all` ✅ succeeds
- [ ] Binary exists: `ls target/release/raybox-tools` ✅
- [ ] WASM build: `cargo run -p tools -- wasm-build` ✅ works

---

## Clean Up

- [ ] Delete `RENAME_PLAN.md` (archive or commit first)
- [ ] Delete `RENAME_CHECKLIST.md` (archive or commit first)
- [ ] Update `CLAUDE.md` to reflect new project name
- [ ] Consider updating `README.md` with "formerly canvas_3d_6" note

---

## Success! 🎉

The project is now **Raybox**!

Next steps:
- [ ] Continue with V2 development (physically-based UI)
- [ ] Implement SDF normals + lighting
- [ ] Add shadow mapping
- [ ] Add Gaussian blur for glass effects
- [ ] Eventually add raymarching viewport for CAD

---

## If Something Goes Wrong

**Rollback procedure:**

```bash
# From /home/martinkavik/repos
mv raybox canvas_3d_6
cd canvas_3d_6

# Undo last commit
jj undo

# Verify
jj st
cargo build --all
```

Then investigate what went wrong, fix it, and restart from Phase 1.

---

**Remember:** Take your time, check each step, and verify before moving to the next phase!
