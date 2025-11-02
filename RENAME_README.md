# Raybox Rename - Quick Start

This directory contains everything you need to rename `canvas_3d_6` to `raybox`.

---

## 📋 Three Ways to Execute the Rename

### Option 1: Automated (Fastest) ⚡

Use the helper script to update all files automatically:

```bash
# Run the script
./rename_helper.sh

# Verify changes
jj diff

# Build and test
cargo clean
cargo build --all
cargo run -p tools -- --help  # Should show "raybox-tools"

# If all looks good, rename directory
cd /home/martinkavik/repos
mv canvas_3d_6 raybox
cd raybox

# Verify and commit
jj st
cargo build --all
jj commit -m "Rename project from canvas_3d_6 to raybox"
```

**Time:** ~10 minutes

---

### Option 2: Manual with Checklist (Recommended) ✅

Follow the step-by-step checklist for maximum control:

```bash
# Open the checklist
cat RENAME_CHECKLIST.md

# Or in your editor
code RENAME_CHECKLIST.md  # VS Code
vim RENAME_CHECKLIST.md   # Vim
```

Work through each checkbox systematically.

**Time:** ~30-60 minutes

---

### Option 3: Guided Manual (Most Educational) 📚

Read the comprehensive plan with full rationale:

```bash
# Read the detailed plan
cat RENAME_PLAN.md

# Or in your editor
code RENAME_PLAN.md  # VS Code
```

Understand WHY each change is needed and execute manually.

**Time:** ~1-2 hours

---

## 📁 Files in This Directory

| File | Purpose |
|------|---------|
| `RENAME_README.md` | This file - quick start guide |
| `RENAME_PLAN.md` | Comprehensive plan with full rationale |
| `RENAME_CHECKLIST.md` | Step-by-step execution checklist |
| `rename_helper.sh` | Automated script for file updates |

---

## 🎯 What Gets Renamed?

### Changed:
- ✅ **Binary name:** `canvas-tools` → `raybox-tools`
- ✅ **Directory name:** `canvas_3d_6` → `raybox`
- ✅ **Documentation:** All path references and project names
- ✅ **Workspace metadata:** Authors field
- ✅ **Web page title:** "Canvas" → "Raybox"

### Unchanged:
- ❌ **Package names:** `tools`, `renderer` (stay the same)
- ❌ **Crate names in code:** `use renderer::...` (stays the same)
- ❌ **WASM output:** `renderer.js` (stays the same)
- ❌ **JJ repository:** Just moves with directory (no changes)

---

## ⚠️ Important Notes

1. **Order matters!** Update file contents BEFORE renaming directory
2. **Test thoroughly** before committing
3. **Verify searches** to ensure no old references remain
4. **JJ is safe** - repository moves with directory rename

---

## 🔍 Quick Verification

After renaming, run these to verify success:

```bash
# Should find NOTHING (except in these RENAME_*.md files):
grep -r "canvas_3d_6" . --exclude-dir=target --exclude-dir=.jj
grep -r "canvas-tools" . --exclude-dir=target --exclude-dir=.jj

# Should find MANY:
grep -r "raybox" . --exclude-dir=target --exclude-dir=.jj

# Build should work:
cargo build --all

# Binary should exist:
ls target/debug/raybox-tools  # Should exist
ls target/debug/canvas-tools  # Should NOT exist
```

---

## 🚨 If Something Goes Wrong

**Rollback procedure:**

```bash
# From /home/martinkavik/repos
mv raybox canvas_3d_6
cd canvas_3d_6

# Undo file changes
jj undo

# Verify
jj st
cargo build --all
```

---

## 🎓 Why "Raybox"?

From `RENDER_RESEARCH.md`:

> **Raybox** aligns with the project's future direction:
> - **Ray**marching for 3D CAD viewport
> - SDF-based (distance fields, "boxes")
> - Physically-based UI rendering
> - Professional name for a CAD tool

Evolution: `canvas_3d` → `canvas_3d_3` → `canvas_3d_4` → `canvas_3d_6` → **`raybox`**

---

## 📊 Files Updated Summary

**14 files to modify:**
1. CLAUDE.md
2. README.md
3. specs.md
4. WORKFLOW_ANALYSIS.md
5. RUST_ONLY_ARCHITECTURE.md
6. docs/CHROME_SETUP.md
7. docs/DOM_EXTRACTION.md
8. reference/REFERENCE_METADATA.md
9. tools/README.md
10. Cargo.toml (root)
11. tools/Cargo.toml
12. tools/src/main.rs
13. web/index.html
14. Justfile (if exists)

**1 directory to rename:**
- `/home/martinkavik/repos/canvas_3d_6` → `/home/martinkavik/repos/raybox`

---

## ✅ Success Criteria

Rename is successful when:

- [ ] Binary is `raybox-tools` (not `canvas-tools`)
- [ ] Directory is `/home/martinkavik/repos/raybox`
- [ ] `cargo build --all` succeeds
- [ ] `cargo run -p tools -- --help` shows "raybox-tools"
- [ ] No `canvas_3d_6` or `canvas-tools` references in code
- [ ] JJ repository still works (`jj st`, `jj log`)
- [ ] All tests pass (`cargo test --all`)
- [ ] WASM workflow works (`wasm-build`, `wasm-start`)

---

## 🚀 After Rename

Next development steps (V2 - Physically-Based UI):

1. Add Z-depth to elements
2. Implement SDF normal extraction
3. Add simple lighting (bevels!)
4. Implement shadow mapping
5. Add Gaussian blur for glass effects

See `RENDER_RESEARCH.md` Part 6 for full details.

---

**Ready to begin? Pick your preferred method above and start!**
