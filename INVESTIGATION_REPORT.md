# Investigation Report: Reference Data and Renderer Alignment Issues

## Executive Summary

We discovered critical misalignment between our reference DOM layout data and the renderer canvas. The root cause is a **viewport size mismatch**: reference data uses 1920×1080 coordinates while the canvas is 700×700 pixels. This makes direct coordinate translation impossible without proper scaling.

**Status**: Renderer IS working (text visible, elements rendered), but element positions are incorrect due to coordinate system mismatch.

---

## Current State

### What's Working ✅
- WebGPU initialization and rendering
- Rectangle pipeline (backgrounds)
- Border pipeline (element borders)
- Text rendering via Canvas2D hybrid approach
- Text pipeline (textured quads)
- File watching and auto-reload
- Screenshot automation (with viewport size bug)
- Console monitoring

### What's Broken ❌
- **Element positioning** - coordinates don't align with reference
- **Viewport size mismatch** - reference is 1920×1080, canvas is 700×700
- **Screenshot tool** - captures 800×600 instead of requested 700×700
- **Coordinate translation** - current offset calculation is incorrect for scaling

---

## Problem 1: Viewport Size Mismatch

### Reference Data Viewport
```json
{
  "width": 1920,
  "height": 1080,
  "devicePixelRatio": 1.0
}
```

### Actual Canvas Size
```html
<canvas id="canvas" width="700" height="700"></canvas>
```

### Reference Element Positions (1920×1080 coordinates)
```json
{
  "tag": "body",
  "x": 685.0,      // Centered: (1920 - 550) / 2 = 685
  "y": 0.0,
  "width": 550.0,
  "height": 1080.0
}
{
  "tag": "header",
  "x": 685.0,
  "y": 130.0,
  "width": 550.0,
  "height": 65.0
}
```

### Current Renderer Coordinate Translation
```rust
// renderer/src/lib.rs:194-197
let (offset_x, offset_y) = layout.elements.iter()
    .find(|e| e.has_class("header"))
    .map(|e| (e.x, e.y))
    .unwrap_or((0.0, 0.0));

// Then for each element:
element.x - offset_x,  // 685 - 685 = 0
element.y - offset_y,  // 130 - 130 = 0 (for header)
```

**Problem**: This only translates coordinates to (0,0) origin. It doesn't scale from 1920×1080 to 700×700!

---

## Problem 2: Screenshot Tool Viewport Sizing

### Current Implementation
```rust
// tools/src/commands/screenshot.rs:25-31
let (browser, mut handler) = Browser::launch(
    BrowserConfig::builder()
        .with_head()
        .window_size(width, height)  // ← Sets WINDOW size, not VIEWPORT
        .args(webgpu_flags)
        .build()
```

### Result
- Requested: `--width 700 --height 700`
- Actual capture: 800×600 pixels
- Browser chrome (title bar, borders) reduces viewport size

### Fix Needed
Use Chrome DevTools Protocol to set exact viewport size:
```rust
page.set_viewport(ViewportParams {
    width: width as i64,
    height: height as i64,
    device_scale_factor: Some(1.0),
    ..Default::default()
}).await?;
```

---

## Problem 3: Reference Data Generation Strategy

We have **two different approaches** to generating reference data:

### Approach A: Rust Static Generator (Current)
**File**: `tools/src/commands/extract_dom.rs`
- Generates layout from **hardcoded CSS rules**
- Fast, no browser needed
- Currently set for 1920×1080
- **Easy to change viewport size** (just edit constants)

```rust
const VIEWPORT_WIDTH: u32 = 1920;  // ← Change to 700
const VIEWPORT_HEIGHT: u32 = 1080; // ← Change to 700
```

### Approach B: JavaScript Live Extraction
**File**: `scripts/extract_dom_layout.js`
- Extracts from **live DOM** using `getBoundingClientRect()`
- Requires browser, more accurate
- Viewport size determined by browser window
- **Harder to automate** at specific sizes

### Current Mismatch
- Reference JSON: Generated with Approach A at 1920×1080
- Canvas: 700×700
- Screenshot attempts: Trying to capture at 700×700 but getting 800×600

---

## Analysis: What Went Wrong

### Timeline of Issues

1. **Initial setup**: Created reference data at 1920×1080 (standard desktop size)
2. **Canvas created**: Set to 700×700 (standard TodoMVC demo size)
3. **Renderer implemented**: Used offset translation (works for same-size viewports)
4. **Tested**: Elements render but positions are wrong
5. **Screenshot tool used**: Discovered viewport sizing bug
6. **Investigation**: Discovered fundamental viewport mismatch

### Root Cause

**The coordinate translation logic assumes the canvas size matches the reference viewport size.** It only translates coordinates to (0,0) origin but doesn't scale them.

For a 1920×1080 reference rendered on a 700×700 canvas:
- Body at x=685 should be at x=685 × (700/1920) = **249.7**
- But current code puts it at x=685 - 685 = **0** (wrong!)

---

## Solutions: Three Approaches

### Option 1: Regenerate Reference at 700×700 (RECOMMENDED) ⭐

**Pros:**
- Simple, no coordinate scaling needed
- Reference matches target exactly
- Easier to debug positioning issues
- Fast to implement

**Cons:**
- Loses 1920×1080 reference (but we can keep both)
- Need to verify centering calculation changes

**Implementation:**
```rust
// tools/src/commands/extract_dom.rs:8-10
const VIEWPORT_WIDTH: u32 = 700;   // Changed from 1920
const VIEWPORT_HEIGHT: u32 = 700;  // Changed from 1080
const BODY_MAX_WIDTH: f32 = 550.0; // Keep same

// Recalculate centering
fn calculate_body_x() -> f32 {
    (700.0 - 550.0) / 2.0  // = 75px (was 685px)
}
```

**Steps:**
1. Edit `extract_dom.rs` constants
2. Run: `cargo run -p tools -- extract-dom -o reference/todomvc_dom_layout_700.json`
3. Update `web/index.html` to load new JSON
4. Test renderer
5. Keep old 1920 version as `todomvc_dom_layout_1920.json`

### Option 2: Implement Coordinate Scaling

**Pros:**
- Can use any reference size with any canvas size
- More flexible for future changes

**Cons:**
- More complex logic
- Potential for rounding errors
- Harder to debug

**Implementation:**
```rust
// renderer/src/lib.rs
fn scale_coordinates(
    layout: &LayoutData,
    target_width: f32,
    target_height: f32,
) -> Vec<ScaledElement> {
    let ref_width = layout.metadata.viewport.width as f32;
    let ref_height = layout.metadata.viewport.height as f32;

    let scale_x = target_width / ref_width;
    let scale_y = target_height / ref_height;

    layout.elements.iter().map(|elem| {
        ScaledElement {
            x: elem.x * scale_x,
            y: elem.y * scale_y,
            width: elem.width * scale_x,
            height: elem.height * scale_y,
            // ... other fields
        }
    }).collect()
}
```

### Option 3: Resize Canvas to 1920×1080

**Pros:**
- No changes needed to reference or renderer logic

**Cons:**
- Huge canvas, performance issues
- Doesn't fit on screen
- Not practical for demo

**Verdict:** ❌ Don't do this

---

## Recommended Action Plan

### Phase 1: Fix Reference Data

1. **Update extract_dom.rs for 700×700:**
   ```bash
   # Edit constants
   vim tools/src/commands/extract_dom.rs
   # Change VIEWPORT_WIDTH/HEIGHT to 700
   ```

2. **Generate new reference:**
   ```bash
   cargo run -p tools -- extract-dom -o reference/todomvc_dom_layout_700.json
   ```

3. **Verify centering:**
   ```bash
   # Should show x=75 (not 685)
   jq '.elements[] | select(.tag == "body") | {x, y, width}' \
     reference/todomvc_dom_layout_700.json
   ```

4. **Update renderer to load new file:**
   ```javascript
   // web/index.html:84
   const response = await fetch('/reference/todomvc_dom_layout_700.json');
   ```

### Phase 2: Fix Screenshot Tool

1. **Add viewport setting to screenshot.rs:**
   ```rust
   use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;

   page.set_device_metrics_override(SetDeviceMetricsOverrideParams {
       width: width as i64,
       height: height as i64,
       device_scale_factor: 1.0,
       mobile: false,
       ..Default::default()
   }).await?;
   ```

2. **Test:**
   ```bash
   cargo run -p tools -- screenshot \
     --url http://localhost:8765/todomvc_populated.html \
     --output /tmp/test_700.png \
     --width 700 --height 700

   # Verify size
   identify /tmp/test_700.png
   # Should show: PNG 700x700
   ```

### Phase 3: Regenerate Reference Screenshot

1. **Start reference server:**
   ```bash
   cargo run -p tools -- serve --dir reference --port 8765
   ```

2. **Capture reference at 700×700:**
   ```bash
   cargo run -p tools -- screenshot \
     --url http://localhost:8765/todomvc_populated.html \
     --output reference/todomvc_reference_700.png \
     --width 700 --height 700
   ```

3. **Verify visually:**
   ```bash
   eog reference/todomvc_reference_700.png
   ```

### Phase 4: Test Renderer Alignment

1. **Rebuild renderer:**
   ```bash
   cargo run -p tools -- wasm-build
   ```

2. **Start dev server:**
   ```bash
   cargo run -p tools -- wasm-start
   ```

3. **Capture renderer output:**
   ```bash
   cargo run -p tools -- screenshot \
     --url http://localhost:8000 \
     --output /tmp/renderer_700.png \
     --width 700 --height 700
   ```

4. **Compare screenshots:**
   ```bash
   # Visual comparison
   eog reference/todomvc_reference_700.png /tmp/renderer_700.png &

   # Or use ImageMagick compare
   compare reference/todomvc_reference_700.png \
           /tmp/renderer_700.png \
           /tmp/diff.png
   ```

5. **Check element positions:**
   - H1 "todos" should be at x=75, y=-10
   - Input should be at x=75, y=130
   - Todo items should be at x=135 (75 + 60px offset)

### Phase 5: Implement Layout Comparison

1. **Extract live DOM from renderer:**
   ```javascript
   // Add to web/index.html for debugging
   function extractRenderedPositions() {
       // Query all rendered elements
       // Return JSON with actual positions
   }
   ```

2. **Use compare-layouts tool:**
   ```bash
   cargo run -p tools -- compare-layouts \
     --reference reference/todomvc_dom_layout_700.json \
     --actual /tmp/renderer_positions.json \
     --threshold 5.0
   ```

3. **Iterate until all elements within 5px tolerance**

---

## Testing Checklist

Before declaring this fixed:

- [ ] Reference JSON generated at 700×700 viewport
- [ ] Body centered at x=75 (= (700-550)/2)
- [ ] Screenshot tool captures exact 700×700 images
- [ ] Reference screenshot taken at 700×700
- [ ] Renderer loads 700×700 reference JSON
- [ ] Renderer canvas is 700×700
- [ ] All elements render (backgrounds, borders, text)
- [ ] Element positions match reference within 5px
- [ ] H1 "todos" visible and positioned correctly
- [ ] All 4 todo items visible and aligned
- [ ] Footer elements visible

---

## Key Files to Modify

### 1. tools/src/commands/extract_dom.rs
```rust
// Lines 8-10
const VIEWPORT_WIDTH: u32 = 700;   // WAS: 1920
const VIEWPORT_HEIGHT: u32 = 700;  // WAS: 1080
```

### 2. web/index.html
```javascript
// Line 84
const response = await fetch('/reference/todomvc_dom_layout_700.json');
```

### 3. tools/src/commands/screenshot.rs
```rust
// After line 46, before taking screenshot
page.set_device_metrics_override(SetDeviceMetricsOverrideParams {
    width: width as i64,
    height: height as i64,
    device_scale_factor: 1.0,
    mobile: false,
    ..Default::default()
}).await?;
```

### 4. renderer/src/lib.rs (optional - verify offset calculation)
```rust
// Lines 194-197
// Current offset calculation should work once reference is 700×700
// But verify centering is correct for new viewport
```

---

## Future Improvements

### Support Multiple Viewport Sizes
```rust
// Layout scaling utility
pub fn scale_layout(
    layout: &LayoutData,
    target_width: u32,
    target_height: u32,
) -> LayoutData {
    // Scale all coordinates proportionally
}
```

### Automated Testing
```bash
# integration_test.rs
#[test]
fn test_renderer_accuracy() {
    // 1. Generate reference at 700×700
    // 2. Start renderer
    // 3. Take screenshot
    // 4. Compare pixel-by-pixel
    // 5. Assert <5px error
}
```

### Reference Data Versioning
```
reference/
  ├── todomvc_dom_layout_700.json    # 700×700 viewport
  ├── todomvc_dom_layout_1920.json   # 1920×1080 viewport
  ├── todomvc_reference_700.png      # Reference screenshot 700×700
  └── todomvc_reference_1920.png     # Reference screenshot 1920×1080
```

---

## Conclusion

**The renderer is working correctly** - it renders backgrounds, borders, and text as expected. The **positioning errors are entirely due to viewport size mismatch** between the reference data (1920×1080) and the canvas (700×700).

**Recommended fix**: Regenerate reference data at 700×700 (Option 1). This is the simplest, fastest, and most debuggable solution.

**Success metric**: All TodoMVC elements positioned within 5px of reference positions, verified via automated screenshot comparison.
