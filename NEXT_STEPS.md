# Next Steps - Start Here!

## ✅ What's Done

1. ✅ Chrome launched with WebGPU flags → You saw the red triangle!
2. ✅ Comprehensive documentation created:
   - **WORKFLOW_ANALYSIS.md** - Why previous attempts failed, how to succeed
   - **PROFILING_STRATEGY.md** - How to avoid CPU melting/stuttering
   - **docs/CHROME_SETUP.md** - WebGPU setup and verification
   - **docs/DOM_EXTRACTION.md** - How to extract reference layout data
   - **RUST_ONLY_ARCHITECTURE.md** - Why 100% Rust
   - **specs.md** - Full technical specification

3. ✅ Tools created:
   - `web/test_webgpu.html` - WebGPU verification page
   - `scripts/extract_dom_layout.js` - DOM extraction script

4. ✅ Reference materials ready:
   - TodoMVC HTML/CSS/JS in `reference/`
   - Chrome screenshot at `reference/todomvc_chrome_reference.png`

---

## 🚀 What To Do Next (In Order!)

### Priority 1: Extract DOM Layout Data (30 minutes)

**This is CRITICAL!** You need reference data to compare against.

```bash
# 1. Serve reference page
cd ~/repos/canvas_3d_6
miniserve reference --port 8080

# 2. Open in Chrome (with WebGPU flags - already running!)
# Navigate to: http://localhost:8080/todomvc_populated.html

# 3. Open DevTools Console (F12)
# 4. Copy-paste scripts/extract_dom_layout.js
# 5. Run: copy(extractDOMLayout())
# 6. Paste into: reference/todomvc_dom_layout.json
```

**Detailed instructions**: See `docs/DOM_EXTRACTION.md`

**Why critical?**: Without this data, you don't know where elements should be positioned!

### Priority 2: Verify Your Setup (10 minutes)

Test the WebGPU verification page:

```bash
# Serve web directory
miniserve web --port 8080

# Open in Chrome: http://localhost:8080/test_webgpu.html
# Should show: ✅ SUCCESS: WebGPU is ready!
```

**If it fails**: See troubleshooting in `docs/CHROME_SETUP.md`

### Priority 3: Read Key Documents (1 hour)

Read in this order:

1. **WORKFLOW_ANALYSIS.md** (20 min)
   - Understand the iterative workflow
   - Why DOM extraction is essential
   - How profiling works

2. **PROFILING_STRATEGY.md** (20 min)
   - Common CPU melting causes
   - How to avoid them
   - TodoMVC-specific tips (hint: render on demand, not in a loop!)

3. **specs.md** (20 min)
   - V1 scope review
   - Architecture overview
   - Implementation milestones

### Priority 4: Start Implementing (Next session)

Once you have DOM layout data:

```bash
# Create Cargo workspace
cd ~/repos/canvas_3d_6

# This will be Milestone 0 from specs.md
# - Setup Cargo.toml
# - Create renderer, layout, tools crates
# - Create Justfile
# - Hello WebGPU (clear canvas to red)
```

---

## 📊 Decision Matrix

### "Should I use browser MCP?"

**For V1**: No, manual DOM extraction is fine
- Takes 5 minutes with console copy/paste
- Works right now
- No extra setup

**For V2+**: Maybe, if you want automated re-extraction
- Build `tools` crate with CDP
- Automate the extraction
- Integrate with comparison tools

### "Should I start with tools crate or renderer?"

**Recommended**: Minimal tools first
- `serve` command (2 hours)
- Then start renderer
- Add more tools (screenshot, compare) as needed

**Alternative**: Renderer first
- Use `miniserve` directly
- Manual comparison
- Build tools when automation is needed

### "When do I build the comparison tools?"

**After**: You have a working layout engine
- Can't compare until you have output!
- Manual inspection works for first iterations
- Automate when you're iterating rapidly

---

## 🎯 Success Criteria

### By End of Day 1
- [x] WebGPU verified working
- [ ] DOM layout data extracted and saved
- [ ] Read WORKFLOW_ANALYSIS.md
- [ ] Read PROFILING_STRATEGY.md
- [ ] Cargo workspace created

### By End of Week 1
- [ ] Hello WebGPU (canvas clears to color)
- [ ] Basic layout engine (computes positions)
- [ ] Can compare our layout vs reference data
- [ ] Renders colored boxes where elements should be

### By End of Week 2
- [ ] Text rendering works (Canvas2D hybrid)
- [ ] All TodoMVC elements visible
- [ ] Positioning error <10px

### V1 Complete (2-3 weeks)
- [ ] TodoMVC renders with correct positions
- [ ] Text is readable
- [ ] Positioning error <5px
- [ ] No CPU melting (verified with profiling)

---

## 📁 Files You'll Create Soon

```
canvas_3d_6/
├── reference/
│   └── todomvc_dom_layout.json    ← Priority 1: Create this!
│
├── Cargo.toml                     ← Milestone 0
├── Justfile                       ← Milestone 0
│
├── crates/
│   ├── renderer/                  ← Milestone 0-1
│   ├── layout/                    ← Milestone 1
│   └── tools/                     ← Optional, can defer
│
└── web/
    ├── index.html                 ← Milestone 0
    └── demo.js                    ← Milestone 0
```

---

## 🔄 The Development Loop

Once you start implementing:

```
1. Extract DOM data (one time)          ← DO THIS FIRST!
   ↓
2. Implement layout algorithm
   ↓
3. Compute positions for TodoMVC
   ↓
4. Compare: our positions vs reference
   ↓
5. Identify largest errors
   ↓
6. Fix layout algorithm
   ↓
7. Re-compute and compare
   ↓
8. Repeat until error <5px
   ↓
9. Add rendering (colored boxes)
   ↓
10. Add text (Canvas2D hybrid)
    ↓
11. Done! V1 complete.
```

---

## 🚨 Common Mistakes to Avoid

### ❌ Starting Without DOM Data

"I'll just eyeball the positions from the screenshot"

**Why bad**: You'll be off by 10-50px and won't know it

**Do instead**: Extract DOM first (5 minutes!)

### ❌ Continuous Rendering Loop for TodoMVC

```javascript
// ❌ BAD - TodoMVC is static!
function loop() {
  renderer.render();
  requestAnimationFrame(loop);
}
```

**Why bad**: Wastes CPU, causes stuttering

**Do instead**: Render on demand
```javascript
// ✅ GOOD
function updateUI() {
  renderer.render();  // Once
}

button.addEventListener('click', updateUI);
```

### ❌ Skipping Profiling Setup

"I'll optimize later"

**Why bad**: Hard to fix CPU issues after code is written

**Do instead**:
- Set up profiling from day 1
- Check CPU usage every session
- If >30% idle → investigate immediately

### ❌ Not Verifying WebGPU Adapter

```javascript
// ❌ BAD - No check
const adapter = await navigator.gpu.requestAdapter();
renderer.init(adapter);
```

**Why bad**: Might be using CPU renderer without knowing

**Do instead**:
```javascript
// ✅ GOOD - Fail fast
const adapter = await navigator.gpu.requestAdapter();
if (adapter.isFallbackAdapter) {
  throw new Error('CPU adapter! Fix GPU setup first.');
}
```

---

## 💡 Tips

### Use Chrome with WebGPU flags in a script

Save as `scripts/chrome.sh`:
```bash
#!/bin/bash
google-chrome \
  --enable-unsafe-webgpu \
  --enable-webgpu-developer-features \
  --enable-features=Vulkan \
  --enable-vulkan \
  --use-angle=vulkan \
  --remote-debugging-port=9222 \
  "$@"
```

```bash
chmod +x scripts/chrome.sh
./scripts/chrome.sh http://localhost:8080
```

### Keep profiling tab open

Always have Chrome DevTools Performance Monitor open
- Cmd/Ctrl+Shift+P → "performance monitor"
- Watch CPU % in real-time

### Log positions during development

```rust
// In layout engine
println!("Header: x={} y={} w={} h={}",
  node.x, node.y, node.width, node.height);
```

Compare against reference JSON manually at first

---

## 📚 Documentation Index

### Must Read (Before Coding)
1. WORKFLOW_ANALYSIS.md
2. PROFILING_STRATEGY.md
3. specs.md (skim, refer back as needed)

### Reference (As Needed)
- docs/CHROME_SETUP.md (when GPU issues)
- docs/DOM_EXTRACTION.md (when extracting data)
- RUST_ONLY_ARCHITECTURE.md (when building tools)
- reference/REFERENCE_METADATA.md (screenshot details)

### Specs (During Implementation)
- specs.md (architecture, milestones, API)

---

## ✅ Ready Checklist

Before starting to code:

- [ ] Chrome launches with WebGPU flags
- [ ] Saw red triangle at webgpu-samples
- [ ] Tested web/test_webgpu.html (shows success)
- [ ] Extracted DOM layout to reference/todomvc_dom_layout.json
- [ ] Read WORKFLOW_ANALYSIS.md
- [ ] Read PROFILING_STRATEGY.md
- [ ] Understand the iterative refinement loop
- [ ] Know how to check for CPU rendering fallback

**If all checked**: You're ready to start Milestone 0!

**If any unchecked**: Do those first!

---

## 🎉 You're Set Up For Success!

Previous attempts failed because:
- ❌ No reference data to compare against
- ❌ CPU rendering fallback (software adapter)
- ❌ No profiling strategy
- ❌ Tried to do everything at once

This time you have:
- ✅ Chrome with WebGPU verified working
- ✅ Clear workflow with reference data
- ✅ Profiling strategy to avoid CPU melt
- ✅ Incremental milestones
- ✅ Complete documentation

**Now extract that DOM data and start coding!**

---

**Next action**: Extract DOM layout data (see Priority 1 above)
