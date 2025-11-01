# Profiling Strategy - Preventing CPU Melt & Stuttering

## Lessons from canvas_3d (3D Raymarching Version)

Your previous attempts had **CPU melting** issues. Here's what went wrong and how to avoid it.

---

## Root Causes of CPU Issues

### 1. **Software Adapter Fallback** (Most Common!)

**Symptom**: CPU usage across ALL cores, stuttering, slow rendering

**Cause**: Chrome using CPU-based WebGPU (SwiftShader/LLVMpipe) instead of GPU

**Check**:
```javascript
const adapter = await navigator.gpu.requestAdapter();
console.log('Adapter:', adapter);
console.log('Is fallback?', adapter.isFallbackAdapter);
```

**Fix**:
- Use correct Chrome flags (see CHROME_SETUP.md)
- Check `chrome://gpu` shows "WebGPU: Hardware accelerated"
- Never proceed if `adapter.isFallbackAdapter === true`

### 2. **rAF Flooding** (requestAnimationFrame)

**Symptom**: UI FPS ~1-6 while CPU is pegged

**Cause**: Calling `requestAnimationFrame` multiple times per frame

**Bad**:
```javascript
function render() {
  requestAnimationFrame(render);  // ❌
  renderer.render();
  requestAnimationFrame(render);  // ❌ DOUBLE REQUEST!
}
```

**Good**:
```javascript
function render() {
  renderer.render();
  requestAnimationFrame(render);  // ✅ ONE per frame
}
```

### 3. **JS ↔ Wasm Overhead**

**Symptom**: Long Tasks in Chrome DevTools showing frequent boundary crossings

**Cause**: Calling Wasm functions too often (every frame)

**Bad**:
```javascript
function render() {
  renderer.getTelemetry();  // ❌ Every frame
  renderer.getLayout();      // ❌ Every frame
  renderer.render();
  requestAnimationFrame(render);
}
```

**Good**:
```javascript
let frameCount = 0;
function render() {
  renderer.render();

  // Throttle expensive calls
  if (frameCount % 60 === 0) {  // Once per second at 60fps
    const telemetry = renderer.getTelemetry();
    updateDebugInfo(telemetry);
  }

  frameCount++;
  requestAnimationFrame(render);
}
```

### 4. **High DPR (Device Pixel Ratio)**

**Symptom**: CPU/GPU usage scales with window size

**Cause**: Rendering at 2x or 3x native resolution (Retina displays)

**Fix**: Limit DPR during development
```javascript
const dpr = Math.min(window.devicePixelRatio || 1, 1.5);  // Cap at 1.5
canvas.width = canvas.clientWidth * dpr;
canvas.height = canvas.clientHeight * dpr;
```

### 5. **Debug Overlay Overhead**

**Symptom**: FPS drops when showing debug info

**Cause**: Updating DOM too often

**Bad**:
```javascript
function render() {
  updateDebugOverlay();  // ❌ Every frame
  renderer.render();
  requestAnimationFrame(render);
}
```

**Good**:
```javascript
// Update overlay at 5-10 Hz max
setInterval(() => {
  updateDebugOverlay();
}, 200);  // 5 Hz

function render() {
  renderer.render();
  requestAnimationFrame(render);
}
```

---

## TodoMVC-Specific Considerations

For our TodoMVC renderer (simpler than 3D):

### Keep It Simple (V1)

- ❌ **No** continuous animation loop (TodoMVC is static!)
- ❌ **No** `requestAnimationFrame` loop (unless interacting)
- ✅ **Render on demand** (only when state changes)

```javascript
// Good for TodoMVC V1
function renderOnce() {
  renderer.render();
}

// Call only when needed
button.addEventListener('click', () => {
  updateState();
  renderOnce();
});
```

### If You Add Interactivity (V2+)

```javascript
let dirty = false;

function markDirty() {
  dirty = true;
  requestAnimationFrame(renderIfDirty);
}

function renderIfDirty() {
  if (!dirty) return;
  dirty = false;
  renderer.render();
}

// Mark dirty on user input
input.addEventListener('input', markDirty);
button.addEventListener('click', markDirty);
```

---

## Profiling Tools

### 1. Chrome DevTools Performance

**Use this to find hot spots!**

Steps:
1. Open Chrome DevTools (F12)
2. Go to **Performance** tab
3. Click **Record** ⏺️
4. Interact with your app for 5-10 seconds
5. Click **Stop**
6. Look for:
   - **Long Tasks** (>50ms) - blocks UI
   - **Scripting** time - is it JS or Wasm?
   - **Rendering** time - GPU work
   - **Idle** time - should be high!

**Red flags**:
- Many small function calls in a loop
- Frequent `JSON.parse` / `JSON.stringify`
- Wasm calls inside tight loops
- DOM updates every frame

### 2. Chrome Performance Monitor

Real-time metrics:

1. Cmd/Ctrl + Shift + P
2. Type "performance monitor"
3. Shows:
   - CPU usage %
   - JS heap size
   - DOM nodes
   - Layouts/sec
   - Style recalcs/sec

**TodoMVC targets** (V1, static):
- CPU: <5% idle
- Layouts: 0/sec (no layout thrashing)
- Style recalcs: 0/sec

### 3. System Monitoring

```bash
# Linux - Monitor Chrome CPU
pidstat -t 1 -p $(pgrep -f "google-chrome.*webgpu") | grep -v "^$"

# Or simpler
top -p $(pgrep -f "google-chrome.*webgpu")

# macOS
top -pid $(pgrep -f "Google Chrome")
```

**TodoMVC V1 targets**:
- Total CPU: <10% when idle
- Total CPU: <30% during single render

If you see >50% CPU when idle, something is wrong!

### 4. WebGPU Adapter Check

```javascript
async function checkWebGPU() {
  if (!navigator.gpu) {
    console.error('❌ WebGPU not available');
    return false;
  }

  const adapter = await navigator.gpu.requestAdapter({
    powerPreference: 'high-performance'
  });

  if (!adapter) {
    console.error('❌ No WebGPU adapter');
    return false;
  }

  console.log('✅ Adapter:', adapter);
  console.log('✅ Fallback?', adapter.isFallbackAdapter);

  if (adapter.isFallbackAdapter) {
    console.warn('⚠️ Using fallback adapter (CPU)');
    alert('WebGPU is using CPU renderer. Performance will be poor!');
    return false;
  }

  return true;
}
```

Put this in your demo and call it on startup!

---

## Development Profiling Workflow

### Phase 1: Setup (One Time)

1. **Verify WebGPU**
   ```bash
   # Launch Chrome with flags
   google-chrome --enable-unsafe-webgpu chrome://gpu
   # Check "WebGPU: Hardware accelerated"
   ```

2. **Create Profiling Page**
   ```html
   <!-- web/profile.html -->
   <!DOCTYPE html>
   <html>
   <body>
     <div id="stats">
       FPS: <span id="fps">0</span><br>
       Render: <span id="render-ms">0</span> ms<br>
       CPU: Check DevTools
     </div>
     <canvas id="canvas"></canvas>
     <script type="module" src="profile.js"></script>
   </body>
   </html>
   ```

### Phase 2: Each Development Session

1. **Before making changes**:
   - Record baseline performance (DevTools Performance)
   - Note FPS, render time

2. **After making changes**:
   - Record again
   - Compare: did performance get worse?
   - If yes, revert or optimize

3. **Look for**:
   - Increased Long Tasks
   - More frequent GC
   - Higher CPU %
   - Lower FPS

### Phase 3: Optimization

**If you see CPU issues:**

1. Check adapter is GPU (not fallback)
2. Verify ONE `requestAnimationFrame` per frame
3. Throttle telemetry/debug calls
4. Reduce canvas size or DPR
5. Profile with Chrome DevTools
6. Find hotspot, fix, re-profile

---

## Profiling Tools Crate (Future)

When we build `crates/tools`, add profiling commands:

```bash
# Launch Chrome with profiling
cargo run -p tools -- profile \
  --url http://localhost:8080 \
  --duration 10 \
  --output profile.json

# Analyze performance
cargo run -p tools -- analyze \
  --profile profile.json \
  --report report.html
```

**Metrics to capture**:
- Frame times (min/avg/max/p95/p99)
- CPU usage over time
- Memory usage
- Long Tasks count
- WebGPU calls per frame

---

## Performance Targets (TodoMVC)

### V1 (Static Rendering)

- **First render**: <100ms
- **CPU idle**: <5%
- **Memory**: <50MB
- **No continuous rendering**: 0 FPS when idle is GOOD!

### V2 (With Interactions)

- **Render on input**: <16ms (60 FPS)
- **CPU during typing**: <20%
- **No stuttering**: Every keypress renders smoothly

### V3+ (Animations)

- **60 FPS**: 16.7ms budget
- **Frame time p95**: <20ms
- **CPU average**: <30%
- **No Long Tasks**: >50ms

---

## When Things Go Wrong

### Symptom: CPU at 100% across all cores

**Cause**: Software adapter (CPU rendering)

**Fix**:
1. Check `chrome://gpu`
2. Verify WebGPU flags
3. Test with `adapter.isFallbackAdapter`
4. May need GPU drivers update

### Symptom: UI freezes for seconds

**Cause**: Long Task (>1000ms)

**Fix**:
1. Record with DevTools Performance
2. Find the function causing freeze
3. Break into smaller chunks
4. Use `requestIdleCallback` for non-critical work

### Symptom: Gradual slowdown over time

**Cause**: Memory leak or GC pressure

**Fix**:
1. Check Memory tab in DevTools
2. Take heap snapshot
3. Look for growing arrays/objects
4. Check if textures are released

### Symptom: Low FPS (5-10 instead of 60)

**Causes**:
1. Too many draw calls (batch them)
2. Large textures (reduce size)
3. Complex shaders (simplify)
4. JS ↔ Wasm overhead (throttle calls)

---

## Quick Checklist Before Blaming Your Code

- [ ] Chrome launched with `--enable-unsafe-webgpu`?
- [ ] `chrome://gpu` shows hardware WebGPU?
- [ ] `adapter.isFallbackAdapter === false`?
- [ ] Only ONE `requestAnimationFrame` per frame?
- [ ] Not calling Wasm every frame unnecessarily?
- [ ] DPR capped at reasonable value (≤2)?
- [ ] Debug overlay updating at ≤10 Hz?
- [ ] Canvas size reasonable (<1080p while developing)?

If all ✅, then profile with Chrome DevTools to find actual hotspot.

---

## Summary

**For TodoMVC V1**:
- Render **on demand**, not in a loop
- Check adapter is GPU, not CPU
- Profile early, profile often
- If CPU >50% when idle → something is wrong

**Remember**: TodoMVC is mostly static UI. You should NOT need continuous rendering!

---

## References

- Chrome DevTools Performance: https://developer.chrome.com/docs/devtools/performance/
- WebGPU Best Practices: https://toji.dev/webgpu-best-practices/
- canvas_3d cpu_melting.md (for complex 3D scenarios)
