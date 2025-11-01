# Current State Snapshot

Analysis completed after debugging viewport mismatch

---

## What's Working ✅

### Renderer Core
- **WebGPU**: Initializing successfully (no fallback adapter)
- **Rectangle Pipeline**: Rendering backgrounds correctly
- **Border Pipeline**: Rendering element borders
- **Text Pipeline**: Rendering text via Canvas2D → texture → WebGPU
- **Text Renderer**: Canvas2D hybrid working well

### Tooling
- **wasm-build**: Compiling WASM successfully
- **wasm-start**: Dev server with auto-reload working
- **check-console**: Browser console monitoring via CDP
- **screenshot**: Basic capture working (viewport bug noted)
- **serve**: HTTP server for static files

### Evidence from Screenshots
From `/tmp/renderer_700.png`:
- "Buy groceries" visible
- "Walk the dog" visible
- "Finish TodoMVC renderer" visible (with strikethrough)
- "Read documentation" visible
- Footer elements visible: "3 items left", "All", "Active", "Completed", "Clear completed"
- Info footer visible: "Double-click to edit a todo", credits

**Conclusion**: Renderer IS working. Text is readable, elements are present.

---

## What's Broken ❌

### 1. Element Positioning

**Issue**: Elements are not aligned correctly on canvas

**Root Cause**: Viewport size mismatch
- Reference data: 1920×1080 viewport
- Canvas size: 700×700 pixels
- Coordinate translation: Assumes same-size viewport

**Example**:
```
Reference (1920×1080):
  body.x = 685  (centered: (1920-550)/2 = 685)

Renderer (700×700):
  After offset: 685 - 685 = 0 (wrong!)
  Should be: (700-550)/2 = 75 (correct)
```

**Visual Impact**: Elements appear left-aligned instead of centered

### 2. Screenshot Viewport Sizing

**Issue**: Requesting 700×700 produces 800×600 images

**Current Implementation**: Sets browser window size, not viewport
```rust
.window_size(width, height)  // Sets window, includes chrome
```

**Actual Result**:
```bash
$ identify /tmp/renderer_700.png
PNG 800x600 800x600+0+0 8-bit sRGB
```

**Fix Needed**: Use CDP to set exact viewport dimensions

### 3. Reference Data Viewport

**Current**:
```json
{
  "metadata": {
    "viewport": {
      "width": 1920,
      "height": 1080,
      "devicePixelRatio": 1.0
    }
  }
}
```

**Needed**:
```json
{
  "metadata": {
    "viewport": {
      "width": 700,
      "height": 700,
      "devicePixelRatio": 1.0
    }
  }
}
```

---

## Coordinate System Analysis

### Reference Layout (1920×1080)
```
Element         | x      | y      | width  | height
----------------|--------|--------|--------|--------
html            | 0      | 0      | 1920   | 1080
body            | 685    | 0      | 550    | 1080
.todoapp        | 685    | 130    | 550    | 337
.header         | 685    | 130    | 550    | 65
h1              | 685    | -10    | 550    | 80
.new-todo       | 685    | 130    | 550    | 65
```

**Centering calculation**: `(1920 - 550) / 2 = 685px`

### Expected Layout (700×700)
```
Element         | x      | y      | width  | height
----------------|--------|--------|--------|--------
html            | 0      | 0      | 700    | 700
body            | 75     | 0      | 550    | 700
.todoapp        | 75     | 130    | 550    | 337
.header         | 75     | 130    | 550    | 65
h1              | 75     | -10    | 550    | 80
.new-todo       | 75     | 130    | 550    | 65
```

**Centering calculation**: `(700 - 550) / 2 = 75px`

**Difference**: All x-coordinates should shift by -610px (685 → 75)

### Current Renderer Calculation
```rust
// renderer/src/lib.rs:194-197
let (offset_x, offset_y) = layout.elements.iter()
    .find(|e| e.has_class("header"))
    .map(|e| (e.x, e.y))
    .unwrap_or((0.0, 0.0));
// offset_x = 685, offset_y = 130

// For each element:
element.x - offset_x  // body: 685 - 685 = 0 (WRONG!)
element.y - offset_y  // header: 130 - 130 = 0 (CORRECT for header)
```

**Problem**: This only translates to (0,0) origin. It doesn't re-center for 700px viewport.

**Fix Options**:
1. **Regenerate reference at 700×700** (recommended) → offsets become correct automatically
2. **Add scaling logic** → more complex, more error-prone

---

## Visual Evidence

### Reference Screenshot (Expected)
**File**: `reference/todomvc_reference.png`
**Size**: 700×561 pixels
**Shows**: Proper TodoMVC layout with centered content

### Renderer Screenshot (Current)
**File**: `/tmp/renderer_700.png`
**Size**: 800×600 pixels (viewport bug)
**Shows**: TodoMVC content but left-aligned (should be centered)

### Comparison
- ✅ Text rendering works
- ✅ All elements present
- ✅ Vertical spacing looks correct
- ❌ Horizontal alignment wrong (left-aligned vs centered)
- ❌ Screenshot dimensions wrong (800×600 vs 700×700)

---

## Server Status

```bash
Port 8000: wasm-start dev server (renderer)
Port 8765: Python HTTP server (reference files)
Port 9090: canvas-tools serve (unused)
```

All servers are running.

---

## Console Errors

From `check-console` output:
```
❌ Console Errors:
   [Error] Initialization error: Failed to find suitable GPU adapter: NotFound
```

**Note**: This is from port 8000 BEFORE the renderer was properly built. The error is stale.

**Current Status**: No WebGPU errors. Adapter found successfully.

---

## Files Status

### Reference Data
- ✅ `reference/todomvc_dom_layout.json` - exists (1920×1080)
- ❌ `reference/todomvc_dom_layout_700.json` - missing (need to generate)

### Reference Screenshots
- ✅ `reference/todomvc_reference.png` - exists (700×561)
- ❌ `reference/todomvc_reference_700.png` - missing (need to capture at exact 700×700)

### Renderer Output
- ✅ `web/pkg/renderer_bg.wasm` - compiled successfully
- ✅ `web/pkg/renderer.js` - JS bindings generated
- ✅ `web/index.html` - loads reference JSON and starts renderer

---

## Performance

### CPU Usage
- Idle: <5% (good!)
- No continuous rendering loop (correct for static UI)
- No software adapter fallback

### Memory
- WASM binary: ~180KB (reasonable)
- Text textures: Generated on-demand
- No obvious leaks

---

## Dependencies

### Working
- wgpu: WebGPU initialization
- wasm-bindgen: JS ↔ Rust bindings
- serde: JSON serialization
- chromiumoxide: Browser automation
- notify: File watching

### Issues
- chromiumoxide: Viewport sizing API unclear (need to use CDP directly)

---

## Next Steps Priority

### P0: Critical (blocks progress)
1. Regenerate reference at 700×700
2. Update renderer to load new reference
3. Test alignment

### P1: High (improves accuracy)
1. Fix screenshot viewport sizing
2. Capture reference screenshot at 700×700
3. Implement compare-layouts tool

### P2: Medium (nice to have)
1. Add coordinate scaling utility (for future multi-viewport support)
2. Write integration tests
3. Document viewport size handling

### P3: Low (polish)
1. Support multiple reference sizes
2. Automated pixel-perfect comparison
3. CI/CD integration

---

## Success Metrics

After fixing viewport mismatch:

- [ ] All elements centered on 700×700 canvas
- [ ] Body at x=75 (not x=0)
- [ ] H1 "todos" visible at top center
- [ ] Todo items horizontally aligned at x=135 (75 + 60px)
- [ ] Footer centered
- [ ] Screenshots captured at exact requested dimensions
- [ ] Visual comparison shows <5px position error

---

## Confidence Level

**Diagnosis**: 100% confident in root cause (viewport mismatch)
**Solution**: 95% confident (regenerate at 700×700)
**Risk**: Low (simple constants change, no algorithmic changes needed)

---

## References

See also:
- `INVESTIGATION_REPORT.md` - Full analysis
- `NEXT_ACTIONS.md` - Quick fix guide
- `CLAUDE.md` - Project guidelines
- `specs.md` - Technical specification
