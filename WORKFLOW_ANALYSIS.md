# TodoMVC Rendering Workflow Analysis

## Critical Insight: You Need TWO Things to Succeed

1. **Working WebGPU in Chrome** (with correct flags)
2. **DOM Data Extraction** (to know what positions to match)

---

## Part 1: Getting WebGPU Working in Chrome

### Required Chrome Flags

Based on canvas_3d's working implementation, Chrome **requires** these flags on Linux:

```bash
google-chrome \
  --enable-unsafe-webgpu \
  --enable-webgpu-developer-features \
  --enable-features=Vulkan,VulkanFromANGLE,DefaultANGLEVulkan,UseSkiaRenderer \
  --enable-vulkan \
  --use-angle=vulkan \
  --disable-software-rasterizer \
  --ozone-platform=x11 \
  --remote-debugging-port=9222
```

**Critical**: Without `--enable-unsafe-webgpu`, WebGPU will be **disabled** and `navigator.gpu` will be `undefined`.

### WebGPU Verification (Fail-Fast Pattern)

From canvas_3d/demo/wasm-color-controls/main.js:

```javascript
// 1. Check navigator.gpu exists
if (!('gpu' in navigator)) {
  throw new Error('WebGPU not available');
}

// 2. Request high-performance adapter
let adapter = await navigator.gpu.requestAdapter({
  powerPreference: 'high-performance'
});

// 3. Fallback to any adapter
if (!adapter) {
  adapter = await navigator.gpu.requestAdapter();
}

// 4. Fail if still null
if (!adapter) {
  throw new Error('No WebGPU adapter available');
}

// 5. Now proceed with wgpu/WebGPU renderer
```

**No hidden fallbacks!** If WebGPU isn't available, fail immediately with clear error.

### Testing WebGPU

Before building anything, verify:

```bash
# 1. Launch Chrome with correct flags
google-chrome --enable-unsafe-webgpu \
  --enable-webgpu-developer-features \
  http://localhost:8080

# 2. Check chrome://gpu
# Look for: "WebGPU: Hardware accelerated"

# 3. Test with simple page
<script>
console.log('GPU available:', !!navigator.gpu);
navigator.gpu?.requestAdapter()
  .then(a => console.log('Adapter:', a))
  .catch(e => console.error('Failed:', e));
</script>
```

---

## Part 2: DOM Data Extraction Strategy

### The Problem

You have:
- ✅ Reference screenshot (pixels)
- ❌ Reference layout data (positions, sizes, fonts)

You need layout data to know:
- Where is the "todos" header? (x, y, font-size, color)
- Where is each todo item? (x, y, width, height)
- What are the exact padding/margin values?
- What font sizes, line heights, letter spacing?

### Solution: Extract DOM Data via Chrome DevTools Protocol

#### Approach 1: Manual Extraction (Quick Start)

```javascript
// Inject this into reference TodoMVC page
function extractLayout() {
  const elements = document.querySelectorAll('*');
  const data = [];

  elements.forEach(el => {
    const rect = el.getBoundingClientRect();
    const computed = window.getComputedStyle(el);

    data.push({
      tag: el.tagName,
      id: el.id,
      classes: Array.from(el.classList),
      text: el.textContent?.trim().substring(0, 50),

      // Position
      x: rect.x,
      y: rect.y,
      width: rect.width,
      height: rect.height,

      // Typography
      fontSize: computed.fontSize,
      fontFamily: computed.fontFamily,
      fontWeight: computed.fontWeight,
      lineHeight: computed.lineHeight,
      letterSpacing: computed.letterSpacing,
      textAlign: computed.textAlign,

      // Box model
      padding: computed.padding,
      margin: computed.margin,
      border: computed.border,

      // Colors (for V2)
      color: computed.color,
      backgroundColor: computed.backgroundColor,
    });
  });

  return JSON.stringify(data, null, 2);
}

// Copy result
copy(extractLayout());
```

**Run this in Chrome DevTools console** on reference TodoMVC page, then save JSON.

#### Approach 2: Automated via CDP (Rust)

Use `headless_chrome` or `chromiumoxide` to:
1. Navigate to reference TodoMVC
2. Inject extraction script
3. Save JSON automatically

```rust
// tools/src/extract_dom.rs (future)
use headless_chrome::{Browser, LaunchOptions};

pub fn extract_dom_data(url: &str, output: &Path) -> Result<()> {
    let browser = Browser::new(LaunchOptions::default())?;
    let tab = browser.new_tab()?;

    tab.navigate_to(url)?;
    tab.wait_until_navigated()?;

    // Inject extraction script
    let script = include_str!("extract_layout.js");
    let result = tab.evaluate(script, false)?;

    // Save JSON
    std::fs::write(output, result.value.unwrap().to_string())?;
    Ok(())
}
```

**Command**:
```bash
cargo run -p tools -- extract-dom \
  --url http://localhost:8080/reference/todomvc_populated.html \
  --output reference/todomvc_dom_data.json
```

#### Approach 3: Browser MCP (If Available)

Check if there's a browser MCP that can:
- Inject scripts
- Extract DOM data
- Return structured results

---

## Part 3: Iterative Refinement Workflow

### The Loop

```
1. Extract DOM data from reference TodoMVC
   ↓
2. Build layout tree in our renderer (hard-coded TodoMVC structure)
   ↓
3. Run layout solver → get computed positions
   ↓
4. Compare our positions vs reference DOM positions
   ↓
5. Identify largest discrepancies (sort by error magnitude)
   ↓
6. Fix layout engine (adjust flex, padding, font size, etc.)
   ↓
7. Re-render and compare
   ↓
8. Repeat until <5px error
```

### Tools We Need (in `tools` crate)

```rust
// tools/src/main.rs

#[derive(Parser)]
enum Command {
    /// Serve files locally
    Serve {
        path: PathBuf,
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },

    /// Extract DOM layout data from URL
    ExtractDom {
        url: String,
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Compare layout: our engine vs reference DOM
    CompareLayout {
        #[arg(long)]
        reference_dom: PathBuf,  // reference/todomvc_dom_data.json
        #[arg(long)]
        our_layout: PathBuf,     // dist/our_layout.json
        #[arg(long)]
        tolerance: f32,          // e.g., 5.0 for ±5px
    },

    /// Screenshot via CDP
    Screenshot {
        url: String,
        output: PathBuf,
        #[arg(long, default_value = "1920")]
        width: u32,
        #[arg(long, default_value = "1080")]
        height: u32,
    },

    /// Visual diff (pixel comparison)
    VisualDiff {
        reference: PathBuf,
        actual: PathBuf,
        #[arg(long)]
        diff_output: Option<PathBuf>,
        #[arg(long, default_value = "0.01")]
        tolerance: f32,
    },

    /// Watch: rebuild on file change + serve
    Watch {
        #[arg(long, default_value = "8080")]
        port: u16,
    },
}
```

### Comparison Output Example

```bash
$ cargo run -p tools -- compare-layout \
    --reference-dom reference/todomvc_dom_data.json \
    --our-layout dist/our_layout.json \
    --tolerance 5.0
```

**Output**:
```
Layout Comparison Report
========================

Total elements compared: 42
Within tolerance (<5px): 38 (90.5%)
Exceeding tolerance (≥5px): 4 (9.5%)

Top Discrepancies:
------------------
1. .todo-list li[0]
   Reference: x=60, y=265, width=490, height=58
   Ours:      x=60, y=270, width=490, height=55
   Error:     Δy=5px, Δheight=3px ❌

2. .footer
   Reference: x=0, y=585, width=550, height=40
   Ours:      x=0, y=590, width=550, height=40
   Error:     Δy=5px ❌

3. h1 "todos"
   Reference: fontSize=80px, y=-140
   Ours:      fontSize=80px, y=-135
   Error:     Δy=5px ❌

Recommendations:
----------------
- Check .todo-list li padding (currently 15px, may need adjustment)
- Verify line-height calculation (1.4em)
- Check footer margin-top spacing
```

---

## Part 4: Development Workflow (Integrated)

### Setup Phase (Once)

```bash
# 1. Create tools crate with CDP support
cd ~/repos/canvas_3d_6
cargo new --bin crates/tools

# 2. Add dependencies
# In crates/tools/Cargo.toml:
[dependencies]
headless_chrome = "1.0"
tokio = { version = "1", features = ["full"] }
image = "0.24"
clap = { version = "4", features = ["derive"] }
anyhow = "1.0"
serde_json = "1.0"

# 3. Extract reference DOM data
cargo run -p tools -- serve reference --port 8765 &
cargo run -p tools -- extract-dom \
  --url http://localhost:8765/todomvc_populated.html \
  --output reference/todomvc_dom_layout.json
```

### Daily Development Loop

```bash
# Terminal 1: Watch mode (auto-rebuild)
cargo watch -s "just build-wasm-dev"

# Terminal 2: Serve
just serve

# Terminal 3: Compare & iterate
# After each change:
cargo run -p tools -- compare-layout \
  --reference-dom reference/todomvc_dom_layout.json \
  --our-layout dist/layout_debug.json \
  --tolerance 5.0

# Visual check
cargo run -p tools -- screenshot \
  --url http://localhost:8080 \
  --output dist/latest.png

cargo run -p tools -- visual-diff \
  reference/todomvc_chrome_reference.png \
  dist/latest.png \
  --diff-output dist/diff.png
```

---

## Part 5: Priority Order

### Milestone 0.5: Verify WebGPU Works

**Before writing any renderer code:**

1. Create minimal HTML with WebGPU detection
2. Launch Chrome with correct flags
3. Verify `navigator.gpu` is available
4. Request adapter successfully
5. Document exact flags needed

**Files to create**:
- `web/test_webgpu.html` - Minimal WebGPU test
- `docs/CHROME_SETUP.md` - Document flags and verification

### Milestone 0.6: Extract Reference DOM

1. Serve reference TodoMVC locally
2. Write JS extraction script
3. Run in Chrome console OR
4. Implement `tools extract-dom` command
5. Save `reference/todomvc_dom_layout.json`

### Milestone 0.7: Build Tools Crate

1. Create `crates/tools`
2. Implement `serve` command (wrap miniserve or embedded)
3. Implement `extract-dom` command (CDP + JS injection)
4. Implement `screenshot` command
5. Implement `compare-layout` command
6. Implement `visual-diff` command

### Then: Continue with Original Plan

- Milestone 1: Layout Engine
- Milestone 2: Basic Rendering
- Milestone 3: Text Rendering
- etc.

---

## Key Files We Need

```
canvas_3d_6/
├── reference/
│   ├── todomvc_dom_layout.json     # 🆕 Extracted DOM positions
│   ├── todomvc_chrome_reference.png # ✅ Already have
│   └── ...
│
├── crates/tools/                    # 🆕 Dev tools crate
│   ├── src/
│   │   ├── main.rs
│   │   ├── serve.rs
│   │   ├── chrome.rs                # CDP client
│   │   ├── extract_dom.rs           # DOM → JSON
│   │   ├── screenshot.rs
│   │   ├── compare_layout.rs        # Layout diff
│   │   └── visual_diff.rs           # Pixel diff
│   └── extract_layout.js            # Injected JS script
│
├── web/
│   └── test_webgpu.html             # 🆕 WebGPU verification page
│
└── docs/
    └── CHROME_SETUP.md              # 🆕 Chrome flags docs
```

---

## Critical Questions Before Starting

1. **Can you launch Chrome with WebGPU flags?**
   - Try: `google-chrome --enable-unsafe-webgpu chrome://gpu`
   - Does it say "WebGPU: Hardware accelerated"?

2. **Do you have a working WebGPU adapter?**
   - Console: `await navigator.gpu.requestAdapter()`
   - Should return an adapter object, not null

3. **Can we inject JavaScript via CDP?**
   - This is how we'll extract DOM data
   - `headless_chrome` crate supports this

4. **Do you prefer:**
   - Manual DOM extraction (copy/paste from console)
   - Automated via tools crate (cleaner, repeatable)

---

## Recommendations

### Start Here

1. **Verify WebGPU** (30 minutes)
   - Create `web/test_webgpu.html`
   - Test with correct Chrome flags
   - Document what works

2. **Extract Reference DOM** (1 hour)
   - Manually via console first (quick)
   - Then automate with tools crate (optional)
   - Save as `reference/todomvc_dom_layout.json`

3. **Build Minimal Tools Crate** (2-3 hours)
   - `serve` command
   - `extract-dom` command
   - `compare-layout` command

4. **Then**: Proceed with layout engine knowing you have ground truth data

---

## Summary

**The workflow is:**

```
Extract Reference DOM → Build Our Layout → Compare → Fix → Repeat
         ↑                                    ↓
    (One time)                         (Iterative)
```

**You can't skip** extracting DOM data - without it, you're flying blind!

**You can't skip** verifying WebGPU works - otherwise renderer won't run!

---

**Ready to start?** I recommend:

1. First: Create `web/test_webgpu.html` and verify Chrome flags work
2. Second: Extract DOM data (manual console copy-paste is fine for V1)
3. Third: Build basic tools crate with CDP
4. Fourth: Original implementation plan

Want me to create the WebGPU test page and extraction script?
