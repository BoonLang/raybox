# DOM Layout Extraction Guide

## Purpose

Extract precise layout data from the reference TodoMVC so we can compare our renderer's layout against ground truth.

---

## Quick Start (Manual)

### Step 1: Serve Reference TodoMVC

```bash
cd ~/repos/canvas_3d_6
miniserve reference --port 8080
```

### Step 2: Open in Chrome

Navigate to: `http://localhost:8080/todomvc_populated.html`

(Use Chrome with WebGPU flags - see CHROME_SETUP.md)

### Step 3: Run Extraction Script

1. Open Chrome DevTools (F12)
2. Go to **Console** tab
3. Copy-paste contents of `scripts/extract_dom_layout.js`
4. Press Enter
5. You'll see: `✅ extractDOMLayout() is ready!`
6. Run: `copy(extractDOMLayout())`
7. JSON is now in clipboard!

### Step 4: Save JSON

```bash
# Paste clipboard into file
cat > reference/todomvc_dom_layout.json
# Paste (Ctrl+Shift+V)
# Press Ctrl+D to save
```

Or use an editor:
```bash
# Open in editor, paste, save
code reference/todomvc_dom_layout.json
```

---

## What Gets Extracted

### Metadata
- URL, title, user agent
- Viewport size
- Device pixel ratio
- Timestamp

### Per Element
- **Identity**: tag, id, classes, index
- **Position**: x, y, left, top, right, bottom
- **Size**: width, height
- **Typography**: fontSize, fontFamily, fontWeight, lineHeight, etc.
- **Box Model**: padding, margin, border, borderRadius
- **Layout**: display, position, flex properties
- **Content**: textContent, value, placeholder
- **State**: checked, disabled
- **Colors**: color, backgroundColor (for V2)
- **Shadows**: boxShadow, textShadow (for V2)

### Summary
- Total element count
- Count by tag (div: 20, li: 4, etc.)
- Count by class (.todo-list: 1, .completed: 1, etc.)

---

## Automated Extraction (Future)

When `tools` crate is built:

```bash
cargo run -p tools -- extract-dom \
  --url http://localhost:8080/todomvc_populated.html \
  --output reference/todomvc_dom_layout.json
```

This will:
1. Launch headless Chrome
2. Navigate to URL
3. Inject extraction script
4. Save JSON automatically

---

## Inspecting the Output

```bash
# Pretty-print JSON
jq . reference/todomvc_dom_layout.json | less

# Count elements
jq '.summary.totalElements' reference/todomvc_dom_layout.json

# Find specific element
jq '.elements[] | select(.id == "new-todo")' reference/todomvc_dom_layout.json

# List all classes
jq '.summary.byClass | keys' reference/todomvc_dom_layout.json

# Get header element
jq '.elements[] | select(.classes[] == "header")' reference/todomvc_dom_layout.json
```

---

## Using the Data

### Find Element Positions

```bash
# Get "todos" header position
jq '.elements[] | select(.tag == "h1")' reference/todomvc_dom_layout.json

# Output:
{
  "tag": "h1",
  "textContent": "todos",
  "x": 136.5,
  "y": -10,  # Negative because absolute positioned
  "fontSize": "80px",
  "fontWeight": "200",
  "color": "rgb(184, 63, 69)"
}
```

### Compare Our Layout

```javascript
// In our layout engine tests
const reference = require('./reference/todomvc_dom_layout.json');
const ourLayout = computeLayout(todoMVCTree);

// Find corresponding elements
const refHeader = reference.elements.find(el =>
  el.tag === 'h1' && el.textContent === 'todos'
);

const ourHeader = ourLayout.find(node => node.id === 'header-title');

// Compare
const deltaX = Math.abs(ourHeader.x - refHeader.x);
const deltaY = Math.abs(ourHeader.y - refHeader.y);

console.log(`Position error: Δx=${deltaX}px, Δy=${deltaY}px`);

// Pass if within tolerance
assert(deltaX < 5, `X position off by ${deltaX}px`);
assert(deltaY < 5, `Y position off by ${deltaY}px`);
```

---

## Common Queries

### Get All Todo Items

```bash
jq '.elements[] | select(.classes[] == "todo-list") | .children' \
  reference/todomvc_dom_layout.json
```

### Get Input Box

```bash
jq '.elements[] | select(.classes[] == "new-todo")' \
  reference/todomvc_dom_layout.json
```

### Get Footer Elements

```bash
jq '.elements[] | select(.classes[] == "footer")' \
  reference/todomvc_dom_layout.json
```

### Get All Text Nodes

```bash
jq '.elements[] | select(.textContent != "")' \
  reference/todomvc_dom_layout.json
```

---

## Validation

After extraction, verify:

```bash
# Check file exists and is valid JSON
jq . reference/todomvc_dom_layout.json > /dev/null && echo "✅ Valid JSON"

# Check has elements
count=$(jq '.elements | length' reference/todomvc_dom_layout.json)
echo "Element count: $count"
[ "$count" -gt 10 ] && echo "✅ Has elements" || echo "❌ Too few elements"

# Check has key elements
jq -e '.elements[] | select(.tag == "h1" and .textContent == "todos")' \
  reference/todomvc_dom_layout.json > /dev/null \
  && echo "✅ Has header" || echo "❌ Missing header"

jq -e '.elements[] | select(.classes[] == "new-todo")' \
  reference/todomvc_dom_layout.json > /dev/null \
  && echo "✅ Has input" || echo "❌ Missing input"
```

---

## Troubleshooting

### "copy is not defined"

**Solution**: Some browsers don't have `copy()` in console.

Alternative:
```javascript
const json = extractDOMLayout();
console.log(json);
// Manually select all, copy
```

Or:
```javascript
// Create download link
const blob = new Blob([extractDOMLayout()], {type: 'application/json'});
const url = URL.createObjectURL(blob);
const a = document.createElement('a');
a.href = url;
a.download = 'todomvc_dom_layout.json';
a.click();
```

### "Elements array is empty"

**Causes**:
1. Script ran before page loaded
2. Wrong selector

**Solutions**:
1. Wait for page to load fully
2. Check `document.querySelectorAll('*')` returns elements

### "Positions all (0, 0)"

**Cause**: Elements not rendered yet

**Solution**:
```javascript
// Wait for layout
await new Promise(r => setTimeout(r, 1000));
copy(extractDOMLayout());
```

---

## Example Output Structure

```json
{
  "metadata": {
    "url": "http://localhost:8080/todomvc_populated.html",
    "title": "TodoMVC: JavaScript Es6",
    "viewport": {
      "width": 1920,
      "height": 1080,
      "devicePixelRatio": 1
    },
    "timestamp": "2025-11-01T14:30:00.000Z"
  },
  "elements": [
    {
      "index": 0,
      "tag": "html",
      "id": null,
      "classes": [],
      "x": 0,
      "y": 0,
      "width": 1920,
      "height": 1080,
      ...
    },
    {
      "tag": "h1",
      "id": null,
      "classes": [],
      "x": 685,
      "y": -10,
      "width": 550,
      "height": 80,
      "fontSize": "80px",
      "fontWeight": "200",
      "color": "rgb(184, 63, 69)",
      "textContent": "todos",
      ...
    },
    ...
  ],
  "summary": {
    "totalElements": 45,
    "byTag": {
      "html": 1,
      "body": 1,
      "h1": 1,
      "input": 2,
      "ul": 2,
      "li": 4,
      ...
    },
    "byClass": {
      "todoapp": 1,
      "header": 1,
      "new-todo": 1,
      "todo-list": 1,
      "completed": 1,
      ...
    }
  }
}
```

---

## Next Steps

1. Extract DOM data now (5 minutes)
2. Verify JSON is valid
3. Use in layout comparison tests
4. Build `tools` crate for automation (later)

---

## Summary

**Manual extraction** (do this now):
1. Serve reference page
2. Open in Chrome
3. Run script in console
4. Copy JSON to `reference/todomvc_dom_layout.json`

**Automated extraction** (future):
1. Build `tools` crate
2. Use CDP to inject script
3. Save JSON automatically

**Usage**:
- Compare our layout engine output
- Find positioning errors
- Iterate until <5px tolerance

---

**Ready to extract?** Follow Quick Start steps above!
