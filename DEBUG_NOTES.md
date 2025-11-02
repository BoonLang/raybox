# V2 Rendering Issues - Debug Notes

## Critical Issues Reported (2025-11-02)

### 1. Filter Button Rounded Corners NOT Visible ✅ FIXED
**Expected:** "All" button should have 3px rounded corners
**Actual:** No rounded corners visible (sharp 90° corners)

**Root Cause:** Filter button had `border: "1px solid #ce4646"` but NO `backgroundColor`, so no rectangle was rendered. Border rendering used 4 separate edge rectangles which cannot create rounded corners.

**Solution Implemented:**
1. Added `border_width` field to `RectangleInstance` struct
2. Created `new_border_outline()` constructor for rendering rounded border rings
3. Updated WGSL shader with SDF ring rendering (outer box - inner box)
4. Modified border rendering logic to use rectangle pipeline for rounded borders
5. Borders with `border_radius > 0.5` now render as SDF rings instead of 4 edges

**Files Modified:**
- `renderer/src/rectangle_pipeline.rs:7-44` - Added border_width field and new_border_outline()
- `renderer/src/rectangle_pipeline.rs:181-222` - SDF ring rendering in fragment shader
- `renderer/src/lib.rs:326-394` - Border rendering logic checks border_radius

**Result:** ✅ "All" button now has visible 3px rounded corners on red border

### 2. "All" Text Label Mispositioned ✅ FIXED
**Expected:** Text centered in button (both horizontally and vertically)
**Actual:** Text in bottom-left corner of button

**Root Cause:**
1. **Vertical:** Text y-position was `element.y` (top edge), with no vertical centering
2. **Horizontal:** Filter buttons had `textAlign: null`, so default left-alignment was used

**Solution Implemented:**
1. Added vertical centering: `y_position = element.y + (element.height - canvas_height) / 2.0`
2. Added special case for filter buttons: Center text for `<a class="selected">` elements
3. Combined with existing `textAlign: "center"` detection

**Files Modified:**
- `renderer/src/text_renderer.rs:80-89` - Added horizontal centering for filter buttons
- `renderer/src/text_renderer.rs:144-145` - Added vertical centering calculation

**Result:** ✅ "All" text is now properly centered both horizontally and vertically

### 3. Filter Button Vertical Misalignment
**Expected:** Button vertically centered in footer
**Actual:** Button touching parent top edge

**Investigation:**
- Footer element: index 31, y=427.0, height=40.0
- "All" button: index 35, y=427.0, height=20.0
- Button y-position same as footer y-position (should be y=437.0 for centering)
- This is a CSS layout issue captured in reference JSON
- NOT a rendering bug - JSON positions are correct from browser

**Resolution:** This is expected behavior - JSON captured actual browser layout

### 4. No Background Rectangle for Filter Buttons?
**Possible Issue:**
- Filter buttons may not have background_color set
- Check if transparent backgrounds skip rectangle rendering
- Border shorthand exists but background might be missing

**Files to Check:**
- Reference JSON element index 35 - check backgroundColor field
- `renderer/src/lib.rs:282-311` - Rectangle creation logic skips if alpha == 0

## Quick Checks Needed

```bash
# Check filter button data
cat reference/todomvc_dom_layout_700.json | jq '.elements[35]'

# Look for backgroundColor on All button
cat reference/todomvc_dom_layout_700.json | jq '.elements[35].backgroundColor'

# Check all filter buttons
cat reference/todomvc_dom_layout_700.json | jq '.elements[] | select(.classes[] == "selected")'
```

## Likely Root Cause

**Border-radius not rendering because:**
1. Filter buttons might not have background rectangles (transparent background)
2. Border-radius only applies to background rectangles, not border edges
3. Need to render rounded rectangle for border area, then render content inside

**Text positioning wrong because:**
1. Text renderer not calculating vertical centering
2. Likely using element.y directly without font baseline offset
3. Need to calculate: `text_y = element.y + (element.height - font_height) / 2 + baseline_offset`

## Next Steps

1. Verify filter button has backgroundColor in JSON
2. If no background, create invisible rectangle with border_radius just for rounded border
3. Fix text vertical centering in `text_renderer.rs`
4. Test with screenshot comparison

## Code Locations

**Rectangle with border-radius:** `renderer/src/lib.rs:290-310`
**Text positioning:** `renderer/src/text_renderer.rs:render_text()`
**Border rendering:** `renderer/src/lib.rs:315-368`
**Reference JSON:** `reference/todomvc_dom_layout_700.json` element 35
