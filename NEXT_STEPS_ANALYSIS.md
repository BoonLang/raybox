# Next Steps Analysis

## Current State Assessment

### What We Have
1. ✅ Reference JSON at 700×700 (45 elements)
2. ✅ Reference screenshot at 700×700 (properly centered TodoMVC)
3. ✅ Screenshot tool producing exact 700×700 images
4. ✅ Renderer WASM rebuilt with coordinate fix
5. ✅ All Python removed, using Rust tools only

### Reference Layout Key Positions (700×700)
```
body:     x=75,  y=0    (centered: (700-550)/2 = 75)
.todoapp: x=75,  y=130  (white card with shadow)
.header:  x=75,  y=130  (input field section)
h1:       x=75,  y=-10  (red "todos" title, above viewport)
```

### Visual Comparison

**Reference Screenshot** (`reference/todomvc_reference_700.png`):
- ✅ Red "todos" title at top (visible)
- ✅ Gray background (body color: rgb(245,245,245))
- ✅ White card (.todoapp with shadow)
- ✅ Input field with placeholder "What needs to be done?"
- ✅ Checkboxes visible
- ✅ Todo items properly aligned
- ✅ Content centered horizontally
- ✅ Proper spacing and layout

**Renderer Screenshot** (`/tmp/renderer_final.png`):
- ❌ NO "todos" title visible
- ✅ Gray background (body color rendering)
- ❌ NO white card visible
- ❌ NO input field
- ❌ NO checkboxes
- ✅ Text items visible but plain
- ❌ Content appears left-aligned (not centered)
- ❌ Minimal spacing, no card structure

## Problem Diagnosis

### Issue 1: Element Positioning

**Current offset calculation:**
```rust
let offset_x = 0.0;
let offset_y = header.y;  // 130
```

**Effect on coordinates:**
```
.todoapp: (75, 130) → (75-0, 130-130) = (75, 0)   ✓ Correct x, correct y
body:     (75, 0)   → (75-0, 0-130)   = (75, -130) ✗ Body pushed above viewport!
h1:       (75, -10) → (75-0, -10-130) = (75, -140) ✗ Title WAY above viewport!
```

**Problem**: We're moving the .todoapp section to y=0, but this pushes the body ABOVE the viewport. Since body is a containing element, this might cause rendering issues.

### Issue 2: Missing Visual Elements

The renderer shows ONLY text, no backgrounds, no input fields, no checkboxes. This suggests:

1. **Background rectangles not rendering** - White .todoapp card missing
2. **Input elements filtered out** - No input field visible
3. **Checkbox elements filtered out** - No checkboxes visible
4. **Border rendering issues** - No card shadow visible

### Issue 3: Coordinate System Confusion

The reference screenshot shows the "todos" title at the TOP and visible, but JSON says h1.y = -10 (above viewport). This means:

**Theory**: The reference screenshot is NOT showing the full page from y=0. It's showing a CROPPED view starting from approximately y=-10 or so, to fit the visible content.

**Our canvas**: 700×700 pixels, trying to show the content

**Question**: What should we actually display on the canvas?
- Option A: Full page from y=0 (body top) to y=700
- Option B: Content area from y=-10 (h1 title) to y=467 (approx where content ends)
- Option C: Something else?

## Root Cause Analysis

### Why Text Appears Left-Aligned

Looking at the screenshots, text appears at the LEFT edge of the canvas. This means elements are rendering at x≈0, not x=75.

**Possible causes:**
1. Old WASM cached by browser (auto-reload didn't trigger)
2. Wrong reference JSON loaded (still using 1920×1080)
3. Coordinate offset still wrong
4. Different bug in rendering code

### Why No Backgrounds

The .todoapp section has `backgroundColor: "rgb(255, 255, 255)"` (white) but it's not visible. This means:

**Possible causes:**
1. Element is filtered out by `is_visible()` check
2. Color parsing fails for this element
3. Element rendered but alpha=0
4. Z-ordering issue (text in front of background)

### Why No Input Fields/Checkboxes

Input elements and checkboxes are not visible at all. This means:

**Possible causes:**
1. These elements have no `backgroundColor` so they're skipped
2. They're filtered out as "invisible"
3. They render but are positioned off-screen
4. They're beneath other elements (z-order)

## Recommended Next Steps

### Step 1: Verify Current State

**Check which reference is actually being used:**
```bash
# Check browser console logs
cargo run -p tools -- check-console --url http://localhost:8000

# Look for log message like:
# "Loaded layout with X elements"
# "Content area offset: (X, Y)"
```

**Expected:**
- 45 elements loaded
- offset_x = 0, offset_y = 130

### Step 2: Debug Coordinate Offset

The offset calculation might be fundamentally wrong. We need to reconsider what we're trying to achieve.

**Current approach:** Offset to bring .header to canvas origin (0,0)

**Problem:** This pushes other elements (body, h1) above viewport

**Alternative approach:** Don't offset at all, use reference coordinates as-is
```rust
let offset_x = 0.0;
let offset_y = 0.0;
```

Then body is at (75, 0), .todoapp is at (75, 130), h1 is at (75, -10).

This shows the page starting from y=0 (body top), with h1 slightly above viewport (clipped), which matches the reference screenshot!

### Step 3: Debug Missing Backgrounds

**Check why .todoapp background isn't rendering:**
1. Verify element isn't filtered by `is_visible()`
2. Verify color parsing succeeds
3. Check if rectangle instance is created
4. Check rendering order

**Add debug logging:**
```rust
log::info!("Element {}: visible={}, bg_color={:?}",
    element.tag, element.is_visible(), element.background_color);
```

### Step 4: Verify Auto-Reload

The browser might be showing old WASM. Force reload:

```bash
# Stop wasm-start if running
pkill -f "wasm-start"

# Rebuild
cargo run -p tools -- wasm-build

# Manual test in fresh browser window
cargo run -p tools -- screenshot --url http://localhost:8000 ...
```

### Step 5: Compare Element Counts

**Reference has:** 45 elements
**Renderer should render:** All visible elements with backgrounds or text

**Check actual render count:**
```rust
log::info!("Rendering {} rectangles, {} borders, {} text elements",
    rect_instances.len(), border_instances.len(), text_instances.len());
```

Expected from reference:
- Backgrounds: body (gray), .todoapp (white), maybe others = 2-5 rectangles
- Borders: input field, todo items = 5-10 border edges
- Text: h1, placeholder, 4 todo items, footer elements = 8-12 text elements

## Proposed Fix Strategy

### Option A: No Offset (Recommended for Testing)

Set both offsets to 0 and see what renders:

```rust
let offset_x = 0.0;
let offset_y = 0.0;
```

This will show the page as-is from the reference JSON. If this produces a centered result, we know the reference JSON is correct and we don't need offsets.

### Option B: Y-Only Offset (Original Approach)

Keep current approach but investigate why backgrounds aren't rendering:

```rust
let offset_x = 0.0;
let offset_y = 130.0;  // Bring .todoapp to top
```

But fix background rendering issues.

### Option C: Smart Viewport Cropping

Calculate a viewport that shows just the visible content:

```rust
// Find content bounds
let min_y = -10.0;  // h1 top
let max_y = 467.0;  // footer bottom (approximate)
let content_height = max_y - min_y;  // 477px

// Offset to show content from min_y
let offset_y = min_y;  // -10

// Then h1 at y=-10 becomes y=0 (top of canvas)
// .todoapp at y=130 becomes y=140 (below title)
```

## Questions to Answer

1. **What viewport should the canvas show?**
   - Full page (y=0 to y=700)?
   - Content area (y=-10 to y=467)?
   - Just .todoapp section (y=130 to y=467)?

2. **Why are backgrounds not rendering?**
   - Color parsing issue?
   - Visibility filtering?
   - Z-order problem?

3. **Is the new WASM actually loaded?**
   - Browser cache?
   - Auto-reload working?

4. **What does the browser console actually say?**
   - Offset values logged?
   - Element counts?
   - Any errors?

## Immediate Action Plan

**BEFORE making any code changes:**

1. Check browser console logs for actual offset values
2. Check element render counts (rectangles, borders, text)
3. Verify which reference JSON is loaded (45 elements?)
4. Compare visual output to expected

**THEN decide:**
- If offset values are wrong → fix offset calculation
- If backgrounds aren't rendering → debug background rendering
- If wrong JSON loaded → fix caching/reload issue
- If coordinates are right but visually wrong → investigate rendering pipeline

## Success Criteria

When this is working correctly:
- [ ] White .todoapp card visible and centered at x=75
- [ ] Gray body background visible
- [ ] Red "todos" title visible at top
- [ ] Input field visible with placeholder
- [ ] All 4 todo items visible with proper spacing
- [ ] Checkboxes visible (even if not functional)
- [ ] Footer elements visible
- [ ] All elements positioned within 5px of reference
- [ ] Visual match to reference screenshot
