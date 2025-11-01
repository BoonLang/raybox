# TodoMVC DOM Layout Analysis

## Overview

Extracted comprehensive layout data for TodoMVC reference implementation from HTML/CSS analysis.

**Generated:** 2025-11-01T14:30:00Z
**Source:** Python CSS/HTML analyzer (`tools/extract_layout.py`)
**Output:** `todomvc_dom_layout.json` (724 lines, 45 elements)

## Viewport & Centering

- **Viewport:** 1920×1080 pixels
- **Body width:** 550px (max-width from CSS)
- **Body X position:** 685px (centered: `(1920 - 550) / 2`)
- **Device Pixel Ratio:** 1.0

All content is horizontally centered in the viewport using CSS `margin: 0 auto`.

## Key Layout Measurements

### H1 "todos" Title
- **Position:** x=685, y=-10 (above viewport, positioned absolutely)
- **Size:** 550×80px
- **Typography:** 80px, weight 200, color rgb(184, 63, 69)
- **CSS reference:** `position: absolute; top: -140px` → y = 130 - 140 = -10

### Input Field (.new-todo)
- **Position:** x=685, y=130 (top of todoapp section)
- **Size:** 550×65px
- **Typography:** 24px
- **Placeholder:** "What needs to be done?"
- **Padding:** 16px 16px 16px 60px (left padding for checkmark icon)

### Todo Items (4 total)
Each todo item is 58px tall (padding + font + line-height).

1. **"Buy groceries"**
   - Position: x=745, y=195
   - Size: 490×58px
   - Color: rgb(72, 72, 72) (active)

2. **"Walk the dog"**
   - Position: x=745, y=253
   - Size: 490×58px
   - Color: rgb(72, 72, 72) (active)

3. **"Finish TodoMVC renderer"** ✓
   - Position: x=745, y=311
   - Size: 490×58px
   - Color: rgb(148, 148, 148) (completed - gray)
   - Text decoration: line-through

4. **"Read documentation"**
   - Position: x=745, y=369
   - Size: 490×58px
   - Color: rgb(72, 72, 72) (active)

**Note:** Todo labels are offset 60px from left edge (x=685+60=745) to make room for checkbox.

### Footer Section
- **Position:** x=685, y=427 (after 4 todos: 195 + 4×58 = 427)
- **Height:** ~40px
- **Contains:** Todo count, filters (All/Active/Completed), Clear completed button

## Vertical Layout Flow

```
y = -10    H1 "todos" (80px tall, above viewport)
y = 130    .todoapp section starts
           └─ Input field (65px)
y = 195    └─ Todo list starts
           ├─ Todo #1: "Buy groceries" (58px)
           ├─ Todo #2: "Walk the dog" (58px)
           ├─ Todo #3: "Finish TodoMVC renderer" ✓ (58px)
           ├─ Todo #4: "Read documentation" (58px)
y = 427    └─ Footer (40px)
y = 527    Info footer starts
```

**Total vertical range:** -10px to 1080px (1090px total height)

## Element Statistics

### By Tag
- `html`: 1
- `body`: 1
- `section`: 2
- `header`: 1
- `h1`: 1
- `input`: 5
- `div`: 2
- `label`: 5
- `main`: 1
- `ul`: 2
- `li`: 7
- `button`: 5
- `footer`: 2
- `span`: 1
- `a`: 3
- `p`: 3

**Total:** 45 elements

### By Class
- `todoapp`: 1 (main app container)
- `header`: 1
- `new-todo`: 1 (input field)
- `main`: 1
- `toggle-all-container`: 1
- `toggle-all`: 1 (checkbox)
- `toggle-all-label`: 1
- `todo-list`: 1 (ul)
- `view`: 4 (one per todo item)
- `toggle`: 4 (checkboxes)
- `destroy`: 4 (delete buttons)
- `completed`: 1 (todo #3)
- `footer`: 1
- `todo-count`: 1 ("3 items left")
- `filters`: 1 (ul)
- `selected`: 1 ("All" filter)
- `clear-completed`: 1 (button)
- `info`: 1 (bottom credits)

## Typography System

### Font Families
Primary: `Helvetica Neue, Helvetica, Arial, sans-serif`

### Font Sizes
- **80px:** H1 title
- **24px:** Input field, todo items
- **15px:** Footer text
- **11px:** Info section

### Font Weights
- **200:** H1 title (thin)
- **300:** Body text (light)
- **400:** Default (normal)

### Colors
- **Title:** rgb(184, 63, 69) - red/burgundy
- **Active todo text:** rgb(72, 72, 72) - dark gray
- **Completed todo text:** rgb(148, 148, 148) - light gray
- **Background (body):** rgb(245, 245, 245) - very light gray
- **Background (todoapp):** rgb(255, 255, 255) - white

## Critical Positioning Notes

1. **H1 absolute positioning:** The h1 uses `position: absolute; top: -140px` relative to the `.todoapp` section at y=130, placing it at y=-10 (above the viewport top).

2. **Horizontal centering:** All content horizontally centered via CSS `margin: 0 auto` on body with `max-width: 550px`, resulting in x=685 for 1920px viewport.

3. **Todo item offsets:** Todo labels start at x=745 (685 + 60px) to accommodate checkbox on left.

4. **Vertical spacing:** Todo items stack vertically with 58px height each (includes padding and line-height).

## Usage for Renderer

When implementing the WebGPU renderer:

1. **Read this JSON** at startup to get reference positions
2. **Compare rendered positions** to these values
3. **Success metric:** <5px error on all elements
4. **Focus on:** H1, input, 4 todo items, footer (most visible)

### Example Comparison

```rust
// Load reference data
let reference = load_json("reference/todomvc_dom_layout.json");

// After rendering, check positions
let h1_ref = reference.find_element(tag="h1");
assert_eq!(h1_ref.x, 685.0);
assert_eq!(h1_ref.y, -10.0);
assert_eq!(h1_ref.fontSize, "80px");

let input_ref = reference.find_element(class="new-todo");
assert_eq!(input_ref.y, 130.0);
assert_eq!(input_ref.height, 65);
```

## Next Steps

1. **Implement layout engine** that reads this JSON
2. **Render colored boxes** at these positions first
3. **Add text rendering** using Canvas2D hybrid
4. **Compare screenshots** to `todomvc_chrome_reference.png`
5. **Iterate** until positions match within 5px tolerance

## Files

- **Data:** `reference/todomvc_dom_layout.json`
- **Generator:** `tools/extract_layout.py`
- **Screenshot:** `reference/todomvc_chrome_reference.png`
- **This analysis:** `reference/LAYOUT_ANALYSIS.md`
