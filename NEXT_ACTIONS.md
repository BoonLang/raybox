# Next Actions: Fix Viewport Mismatch

## TL;DR

**Problem**: Reference data is 1920×1080, canvas is 700×700 → coordinates don't match

**Solution**: Regenerate reference at 700×700

---

## Quick Fix (Do This Now)

### Step 1: Update Reference Generator

Edit `tools/src/commands/extract_dom.rs`:

```rust
// Line 8-10
const VIEWPORT_WIDTH: u32 = 700;   // Change from 1920
const VIEWPORT_HEIGHT: u32 = 700;  // Change from 1080
```

### Step 2: Generate New Reference

```bash
cargo run -p tools -- extract-dom -o reference/todomvc_dom_layout_700.json
```

Verify:
```bash
jq '.metadata.viewport' reference/todomvc_dom_layout_700.json
# Should show: {"width": 700, "height": 700, "devicePixelRatio": 1.0}

jq '.elements[] | select(.tag == "body") | {x, width}' reference/todomvc_dom_layout_700.json
# Should show: {"x": 75.0, "width": 550.0}  (NOT x=685!)
```

### Step 3: Update Renderer

Edit `web/index.html` line 84:

```javascript
const response = await fetch('/reference/todomvc_dom_layout_700.json');
```

### Step 4: Test

```bash
# Rebuild
cargo run -p tools -- wasm-build

# Start server (if not already running)
cargo run -p tools -- wasm-start

# Take screenshot
cargo run -p tools -- screenshot \
  --url http://localhost:8000 \
  --output /tmp/renderer_test.png \
  --width 700 --height 700

# View
eog /tmp/renderer_test.png
```

**Expected result**: Elements should be horizontally centered on canvas (not pushed to left edge)

---

## Medium Priority: Fix Screenshot Tool

### Issue
Requesting `--width 700 --height 700` produces 800×600 images

### Fix
Add viewport override to `tools/src/commands/screenshot.rs`:

```rust
// After line 46, add:
use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;

page.set_device_metrics_override(SetDeviceMetricsOverrideParams {
    width: width as i64,
    height: height as i64,
    device_scale_factor: 1.0,
    mobile: false,
    ..Default::default()
}).await?;
```

Test:
```bash
cargo build -p tools
cargo run -p tools -- screenshot \
  --url http://localhost:8765/todomvc_populated.html \
  --output /tmp/test.png \
  --width 700 --height 700

identify /tmp/test.png
# Should show: PNG 700x700 (not 800x600!)
```

---

## Low Priority: Generate Reference Screenshot

Once screenshot tool is fixed:

```bash
# Make sure reference server is running
cargo run -p tools -- serve --dir reference --port 8765 &

# Capture reference
cargo run -p tools -- screenshot \
  --url http://localhost:8765/todomvc_populated.html \
  --output reference/todomvc_reference_700.png \
  --width 700 --height 700
```

---

## Verification Checklist

After completing Step 1-4:

- [ ] Reference JSON shows 700×700 viewport
- [ ] Body element at x=75 (centered: (700-550)/2)
- [ ] Renderer loads 700×700 reference
- [ ] Elements appear centered on canvas
- [ ] H1 "todos" visible at top center
- [ ] All 4 todo items visible and aligned
- [ ] Footer elements visible

---

## Files Changed

1. `tools/src/commands/extract_dom.rs` - viewport constants
2. `reference/todomvc_dom_layout_700.json` - new reference file (created)
3. `web/index.html` - reference JSON path
4. `tools/src/commands/screenshot.rs` - viewport override (optional but recommended)

---

## Debugging Tips

### Check Element Positions
```bash
# Body (should be centered)
jq '.elements[] | select(.tag == "body") | {x, y, width}' \
  reference/todomvc_dom_layout_700.json

# Header
jq '.elements[] | select(.classes[] == "header") | {x, y, width, height}' \
  reference/todomvc_dom_layout_700.json

# First todo item
jq '.elements[] | select(.tag == "label") | select(.textContent == "Buy groceries") | {x, y}' \
  reference/todomvc_dom_layout_700.json
```

### Visual Comparison
```bash
# Capture both
cargo run -p tools -- screenshot \
  --url http://localhost:8765/todomvc_populated.html \
  --output /tmp/reference.png --width 700 --height 700

cargo run -p tools -- screenshot \
  --url http://localhost:8000 \
  --output /tmp/renderer.png --width 700 --height 700

# Compare side-by-side
eog /tmp/reference.png /tmp/renderer.png &
```

---

## Expected Behavior After Fix

### Before (Current - WRONG)
- Body at x=0 (pushed to left edge)
- Elements appear left-aligned
- Not centered on canvas

### After (Fixed - CORRECT)
- Body at x=75 (centered with 75px margin on each side)
- Elements centered on 700px canvas
- Matches reference layout visually

---

## Questions?

See `INVESTIGATION_REPORT.md` for full analysis and alternative solutions.
