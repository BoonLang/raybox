# Missing Elements Implementation Plan

## Current State

After viewport and header fixes, we have:
- ✅ Red "todos" title at top
- ✅ White .todoapp card with shadow
- ✅ Gray body background
- ✅ Todo item text (plain)
- ✅ Footer text
- ✅ All elements properly centered

## Missing Elements

### 1. Input Field (.new-todo)

**Reference Data:**
```json
{
  "classes": ["new-todo"],
  "x": 75.0,
  "y": 130.0,
  "width": 550.0,
  "height": 65.0,
  "placeholder": "What needs to be done?",
  "border": "none",
  "boxShadow": "inset 0 -2px 1px rgba(0,0,0,0.03)"
}
```

**Current Problem:** Input elements have no backgroundColor, so they're skipped by rectangle renderer

**Solution:**
1. Add special case for input elements in render_layout
2. Render input as white rectangle (default input background)
3. Render placeholder text in gray color
4. Add inset shadow using border pipeline

**Implementation:**
```rust
// In render_layout, after background rectangles:
if element.tag == "input" && element.has_class("new-todo") {
    // Render input background (white)
    rect_instances.push(RectangleInstance::new(
        element.x - offset_x,
        element.y - offset_y,
        element.width,
        element.height,
        [1.0, 1.0, 1.0, 1.0],  // White
    ));

    // Render placeholder text (gray)
    if let Some(placeholder) = &element.placeholder {
        // Create temp element for placeholder
        let mut placeholder_elem = element.clone();
        placeholder_elem.text = Some(placeholder.clone());
        placeholder_elem.color = Some("rgb(200, 200, 200)".to_string());

        // Render placeholder
        text_instances.push(...);
    }
}
```

### 2. Checkboxes (.toggle)

**Reference Data:**
```json
{
  "classes": ["toggle"],
  "x": 75.0,
  "y": 195.0,
  "width": 40.0,
  "height": 40.0,
  "checked": false
}
```

**Current Problem:** Checkboxes are input elements with no background, so they're skipped

**Solution:**
Need to render checkboxes as circles. Options:

**Option A: Use Rectangle Pipeline with Circle Approximation**
- Render as white circle (many small rectangles)
- Add gray border
- If checked, add checkmark using border pipeline

**Option B: Create Circle Pipeline (Recommended)**
- New shader pipeline specifically for circles
- Render circle using distance field in fragment shader
- Much cleaner and more performant

**Option C: Pre-render Checkbox Images**
- Use Canvas2D to draw circle + checkmark
- Upload as texture
- Render using textured quad pipeline

**Recommended: Option C** (simplest, reuses existing textured quad pipeline)

**Implementation:**
```rust
// In TextRenderer or new CheckboxRenderer:
pub fn render_checkbox(&mut self, checked: bool) -> RenderedText {
    // Create 40x40 canvas
    self.canvas.set_width(40);
    self.canvas.set_height(40);

    // Draw circle border
    self.context.begin_path();
    self.context.arc(20.0, 20.0, 18.0, 0.0, 2.0 * PI)?;
    self.context.set_stroke_style_str("#ddd");
    self.context.set_line_width(1.0);
    self.context.stroke();

    if checked {
        // Draw checkmark
        self.context.set_stroke_style_str("#5dc2af");
        self.context.set_line_width(2.0);
        self.context.begin_path();
        self.context.move_to(10.0, 20.0);
        self.context.line_to(18.0, 28.0);
        self.context.line_to(30.0, 12.0);
        self.context.stroke();
    }

    // Extract image data
    // ... same as text rendering
}
```

### 3. Strikethrough Text

**Reference Data:**
```json
{
  "textContent": "Finish TodoMVC renderer",
  "textDecoration": "line-through",
  "color": "rgb(148, 148, 148)"
}
```

**Current Problem:** TextRenderer doesn't support text decorations

**Solution:** Extend TextRenderer to draw line through text

**Implementation:**
```rust
// In TextRenderer::render_text, after fill_text:
if let Some(decoration) = &element.text_decoration {
    if decoration == "line-through" {
        // Calculate line position (middle of text)
        let line_y = baseline_y - (font_size / 3.0);  // Slightly above middle

        self.context.set_stroke_style_str(color);
        self.context.set_line_width(1.0);
        self.context.begin_path();
        self.context.move_to(padding as f64, line_y as f64);
        self.context.line_to((padding + text_width) as f64, line_y as f64);
        self.context.stroke();
    }
}
```

## Implementation Order

### Phase 1: Text Decorations (Easiest)
Add strikethrough support to TextRenderer
- Modify `render_text()` to check `text_decoration` field
- Draw line if "line-through"
- Test with "Finish TodoMVC renderer" item

### Phase 2: Input Field (Medium)
Add special case for .new-todo input
- Render white background rectangle
- Render placeholder text in gray
- Position text with padding (left: 60px from element.x)

### Phase 3: Checkboxes (Complex)
Add checkbox rendering via Canvas2D
- Create `render_checkbox()` method in TextRenderer
- Render circle border
- Add checkmark if checked
- Position at left of each todo item

## Data Structure Updates

### Element struct needs text_decoration field

Check if it's already there:
```rust
pub struct Element {
    // ...
    #[serde(rename = "textDecoration")]
    pub text_decoration: Option<String>,
    // ...
}
```

If not, add it.

### Element needs placeholder field

Check if it's already there:
```rust
pub struct Element {
    // ...
    pub placeholder: Option<String>,
    pub checked: Option<bool>,
    // ...
}
```

## Expected Results

After all three phases:

**Phase 1 Complete:**
- ✅ "Finish TodoMVC renderer" shows with strikethrough
- ✅ Gray color for completed items

**Phase 2 Complete:**
- ✅ Input field visible as white box
- ✅ "What needs to be done?" placeholder visible in gray
- ✅ Input positioned at top of white card

**Phase 3 Complete:**
- ✅ 4 checkboxes visible (circles)
- ✅ One checkbox checked (3rd item)
- ✅ Checkboxes positioned to left of text

## Testing Strategy

After each phase:
1. Rebuild WASM: `cargo run -p tools -- wasm-build`
2. Capture screenshot: `cargo run -p tools -- screenshot ...`
3. Visual comparison to reference
4. Check console for errors

## Potential Issues

### Issue: Element struct missing fields

**Check:** Does renderer/src/layout.rs have all required fields?
- text_decoration
- placeholder
- checked

**Solution:** Add fields if missing, rebuild

### Issue: Canvas2D drawing doesn't work in WASM

**Solution:** Canvas2D should work (already used for text), but test early

### Issue: Checkbox positioning

**Check:** Are checkboxes at the correct x position?

Reference shows checkboxes at x=75, but they should be inside the .todoapp card which starts at x=75. The visual reference shows checkboxes indented inside the card.

May need to adjust x position based on parent container.

## Success Criteria

Visual match to reference screenshot:
- [ ] Input field visible with placeholder
- [ ] All 4 checkboxes visible as circles
- [ ] 3rd checkbox shows checkmark
- [ ] "Finish TodoMVC renderer" shows strikethrough
- [ ] Colors match (gray for placeholder, gray for completed item)
- [ ] Positioning within 5px of reference

## Files to Modify

1. `renderer/src/layout.rs` - Add missing fields to Element struct
2. `renderer/src/text_renderer.rs` - Add strikethrough, checkbox rendering
3. `renderer/src/lib.rs` - Add input field and checkbox special cases
