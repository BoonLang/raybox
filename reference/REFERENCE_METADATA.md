# TodoMVC Reference Screenshot Metadata

## Screenshot Details

**Filename**: `todomvc_chrome_reference.png`
**Created**: 2025-11-01
**Source**: Chrome headless capture from `todomvc_populated.html`

## Browser Information

- **Browser**: Google Chrome
- **Version**: 141.0.7390.122
- **Mode**: Headless
- **Platform**: Linux

## Viewport Configuration

- **Window Size**: 1920×1080 pixels
- **Device Scale Factor (DPR)**: 1.0
- **Rendering**: Hardware acceleration disabled (headless)

## TodoMVC State Captured

The screenshot shows TodoMVC with the following populated state:

### Todos List
1. ☐ "Buy groceries" (active)
2. ☐ "Walk the dog" (active)
3. ☑ "Finish TodoMVC renderer" (completed, strikethrough)
4. ☐ "Read documentation" (active)

### UI Elements Visible
- **Header**: "todos" title in red (#b83f45)
- **Input**: "What needs to be done?" placeholder
- **Toggle all**: Checkbox with "Mark all as complete" label
- **Todo items**: 4 items with checkboxes and labels
- **Footer**:
  - Counter: "3 items left"
  - Filters: All (selected) | Active | Completed
  - Button: "Clear completed"
- **Info footer**:
  - "Double-click to edit a todo"
  - "Created by the TodoMVC Team"
  - "Part of TodoMVC" (with link)

## Source Files

- **HTML**: Based on tastejs/todomvc javascript-es6 example
- **CSS**: `app.css` (7.3KB, minified)
- **Repository**: https://github.com/tastejs/todomvc
- **Commit**: Latest from master branch as of 2025-11-01

## Layout Key Measurements

From CSS:

```
Body max-width: 550px
TodoApp margin-top: 130px
Header "todos" font-size: 80px
Header position: absolute, top: -140px
Input height: 65px, padding: 16px 16px 16px 60px
Todo item font-size: 24px, padding: 15px 15px 15px 60px
Footer font-size: 15px, height: 20px, padding: 10px 15px
```

## Font Stack

```
font-family: Helvetica Neue, Helvetica, Arial, sans-serif
font-weight: 300 (light, default)
line-height: 1.4em
```

## Color Palette (for V2)

```css
Background: #f5f5f5
TodoApp bg: #fff
Header "todos": #b83f45
Text: #484848
Completed text: #949494 (with strikethrough)
Placeholder: rgba(0,0,0,0.4)
Borders: #e6e6e6, #ededed
Shadows: rgba(0,0,0,0.2), rgba(0,0,0,0.1)
```

## Usage

This screenshot serves as the **ground truth** for V1 layout validation.

**Success criteria**: Our WebGPU renderer should match element positions within ±5px.

For V1, we focus on:
- ✅ Layout (positions, sizes, spacing)
- ✅ Text content and readability
- ❌ Colors, shadows (defer to V2)
- ❌ Hover states, focus (defer to V3)

## Regenerating This Screenshot

**Current method** (used for initial capture):
```bash
# Serve reference files
cd ~/repos/raybox/reference
python3 -m http.server 8765

# Capture screenshot
google-chrome --headless --disable-gpu \
  --screenshot=/tmp/todomvc_chrome_reference.png \
  --window-size=1920,1080 \
  --force-device-scale-factor=1 \
  http://127.0.0.1:8765/todomvc_populated.html
```

**Future method** (Rust-only via `tools` crate):
```bash
# Serve and capture in one command
cargo run -p tools -- screenshot \
  --file reference/todomvc_populated.html \
  --output reference/todomvc_chrome_reference.png \
  --width 1920 --height 1080

# Or separate steps
just serve &
cargo run -p tools -- screenshot \
  --url http://localhost:8080/todomvc_populated.html \
  --output reference/todomvc_chrome_reference.png
```

## Notes

- The screenshot is captured in headless mode, which may have slight rendering differences from normal Chrome
- Font rendering might vary slightly between headless and interactive modes
- DPR=1 ensures predictable pixel measurements
- 1920×1080 provides ample space for the centered TodoMVC layout (max-width 550px)
