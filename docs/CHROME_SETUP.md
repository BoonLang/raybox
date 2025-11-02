# Chrome WebGPU Setup & Verification

## Quick Start

```bash
# Launch Chrome with WebGPU enabled
google-chrome \
  --enable-unsafe-webgpu \
  --enable-webgpu-developer-features \
  --enable-features=Vulkan,VulkanFromANGLE,DefaultANGLEVulkan,UseSkiaRenderer \
  --enable-vulkan \
  --use-angle=vulkan \
  --disable-software-rasterizer \
  --ozone-platform=x11 \
  --remote-debugging-port=9222 \
  --user-data-dir=/tmp/chrome-canvas-webgpu
```

**Note**: You'll see a warning banner "You are using an unsupported command-line flag". This is expected on Linux until WebGPU graduates from experimental.

---

## Required Flags Explained

### Core WebGPU Flags (REQUIRED)

- `--enable-unsafe-webgpu` - **REQUIRED** Enables WebGPU on Linux (it's behind a flag)
- `--enable-webgpu-developer-features` - Additional debug/profiling features
- `--enable-features=Vulkan,VulkanFromANGLE` - Use Vulkan backend (faster than OpenGL)
- `--enable-vulkan` - Enable Vulkan support
- `--use-angle=vulkan` - ANGLE uses Vulkan (not OpenGL)
- `--disable-software-rasterizer` - Prevent fallback to CPU renderer

**Minimal flags** (used by `raybox-tools` commands):
```bash
--enable-unsafe-webgpu
--enable-webgpu-developer-features
--enable-features=Vulkan,VulkanFromANGLE
--enable-vulkan
--use-angle=vulkan
--disable-software-rasterizer
--ozone-platform=x11
```

### Linux-Specific

- `--ozone-platform=x11` - Use X11 instead of Wayland (more stable for WebGPU)
  - Remove this if you're on Wayland and it works
  - **Note:** `raybox-tools` always includes this flag

### Optional Flags (Manual Testing Only)

- `--remote-debugging-port=9222` - For CDP automation
  - **Note:** `raybox-tools` manages this automatically
- `--user-data-dir=/tmp/chrome-canvas-webgpu` - Isolate profile

### Optional (but recommended)

- `--no-first-run` - Skip first-run dialogs
- `--no-default-browser-check` - Don't ask to be default browser

---

## Verification Steps

### Step 1: Check chrome://gpu

1. Open Chrome with the flags above
2. Navigate to `chrome://gpu`
3. Look for section "Graphics Feature Status"
4. Find "WebGPU" row
5. Should say: **"Hardware accelerated"**

**If it says**:
- ❌ "Disabled" → Flags not working, check command line
- ❌ "Software only" → Using CPU renderer (bad!)
- ❌ "Unavailable" → GPU drivers issue

### Step 2: Test navigator.gpu

1. Open DevTools console (F12)
2. Run:
```javascript
console.log('GPU available:', !!navigator.gpu);
```
3. Should print: `GPU available: true`

**If false**: WebGPU is disabled, check flags

### Step 3: Request Adapter

```javascript
const adapter = await navigator.gpu.requestAdapter({
  powerPreference: 'high-performance'
});

console.log('Adapter:', adapter);
console.log('Is fallback?', adapter.isFallbackAdapter);
```

**Expected**:
- `Adapter:` should show an object (not null)
- `Is fallback?: false`

**If null**: No compatible adapter found

**If `isFallbackAdapter: true`**: Using CPU renderer (SwiftShader/LLVMpipe)
  - Performance will be TERRIBLE
  - Fix GPU setup before proceeding

### Step 4: Simple Triangle Test

Visit: https://webgpu.github.io/webgpu-samples/samples/helloTriangle

You should see a **red triangle** with no errors in console.

**If you see**:
- ✅ Red triangle → WebGPU works!
- ❌ Black screen → Adapter issue
- ❌ Error message → Check console for details

---

## Prerequisites (Linux)

### GPU Drivers

**NVIDIA**:
```bash
# Check driver
nvidia-smi

# Should show your GPU and driver version (550+ recommended)
```

**AMD**:
```bash
# Check Vulkan support
vulkaninfo | grep deviceName
```

**Intel**:
```bash
# Usually works out of the box
```

### Vulkan

**Install packages**:
```bash
# Ubuntu/Debian
sudo apt install vulkan-utils libvulkan1

# Arch
sudo pacman -S vulkan-tools vulkan-icd-loader

# Fedora
sudo dnf install vulkan-tools vulkan-loader
```

**Verify**:
```bash
vulkaninfo | grep "GPU"
# Should list your GPU
```

---

## Troubleshooting

### "WebGPU: Disabled"

**Cause**: Flags not applied

**Fix**:
1. Make sure you're using the command line flags
2. Check `chrome://version` shows the flags in "Command Line"
3. Try closing ALL Chrome windows first, then launch with flags

### "WebGPU: Software only"

**Cause**: Using CPU renderer (SwiftShader)

**Fixes**:
1. Check `chrome://gpu` → "Driver Information"
2. Should show your real GPU (NVIDIA/AMD/Intel), not "SwiftShader"
3. If SwiftShader:
   - Check Vulkan drivers: `vulkaninfo`
   - Try removing `--ozone-platform=x11` (or add it if missing)
   - Update GPU drivers

### "requestAdapter() returns null"

**Causes**:
1. WebGPU disabled (check flags)
2. Blocklist hit your GPU
3. Vulkan not working

**Debug**:
```bash
# Check Vulkan
vulkaninfo | grep deviceName

# Check Chrome sees your GPU
# In chrome://gpu look for "GL_RENDERER"
```

### Performance is terrible

**Check**:
```javascript
const adapter = await navigator.gpu.requestAdapter();
console.log('Fallback?', adapter.isFallbackAdapter);
```

**If `true`**: You're on CPU renderer!
- See "Software only" fixes above

**If `false`** but still slow:
- Check DPR: `window.devicePixelRatio`
- Check canvas size: `canvas.width * canvas.height`
- Profile with Chrome DevTools

---

## Testing Your Setup

### Quick Test Page

Create `web/test_webgpu.html`:
```html
<!DOCTYPE html>
<html>
<head>
  <title>WebGPU Test</title>
</head>
<body>
  <h1>WebGPU Test</h1>
  <div id="status">Testing...</div>
  <pre id="results"></pre>

  <script>
  async function test() {
    const status = document.getElementById('status');
    const results = document.getElementById('results');
    let log = '';

    function append(msg) {
      log += msg + '\n';
      results.textContent = log;
    }

    try {
      // Check navigator.gpu
      if (!navigator.gpu) {
        status.textContent = '❌ FAIL: navigator.gpu not available';
        append('navigator.gpu: undefined');
        append('\nWebGPU is not enabled!');
        append('Check Chrome flags: --enable-unsafe-webgpu');
        return;
      }

      append('✅ navigator.gpu: available');

      // Request adapter
      const adapter = await navigator.gpu.requestAdapter({
        powerPreference: 'high-performance'
      });

      if (!adapter) {
        status.textContent = '❌ FAIL: No adapter';
        append('❌ requestAdapter(): null');
        append('\nNo compatible WebGPU adapter found!');
        return;
      }

      append('✅ Adapter: ' + (adapter.constructor?.name || 'GPUAdapter'));
      append('   Fallback: ' + adapter.isFallbackAdapter);

      if (adapter.isFallbackAdapter) {
        status.textContent = '⚠️ WARNING: Using CPU renderer';
        append('\n⚠️ WARNING: Fallback adapter (CPU rendering)');
        append('Performance will be poor!');
        append('Check your GPU drivers and Vulkan support.');
        return;
      }

      // Get device
      const device = await adapter.requestDevice();
      append('✅ Device: ' + (device.constructor?.name || 'GPUDevice'));

      status.textContent = '✅ SUCCESS: WebGPU ready!';
      append('\n🎉 WebGPU is working correctly!');
      append('GPU rendering enabled.');

    } catch (error) {
      status.textContent = '❌ ERROR';
      append('❌ Error: ' + error.message);
      console.error(error);
    }
  }

  test();
  </script>
</body>
</html>
```

**Test it**:
```bash
# Serve
cd ~/repos/raybox
miniserve web --port 8080

# Open in Chrome (with flags)
# http://localhost:8080/test_webgpu.html
```

**Expected output**:
```
✅ navigator.gpu: available
✅ Adapter: GPUAdapter
   Fallback: false
✅ Device: GPUDevice

🎉 WebGPU is working correctly!
GPU rendering enabled.
```

---

## Automation (CDP)

### Rust Tools (Recommended)

The `raybox-tools` CLI automatically applies WebGPU flags when launching Chrome:

```bash
# Take screenshot (auto-applies WebGPU flags)
cargo run -p tools -- screenshot \
  --url http://localhost:8000 \
  --output /tmp/test.png \
  --width 700 \
  --height 700

# Check console for errors (auto-applies WebGPU flags)
cargo run -p tools -- check-console \
  --url http://localhost:8000 \
  --wait 5
```

**Standard Testing Sizes**:
- **Quick verification:** 700×700px (recommended for rapid testing)
- **Full reference:** 1920×1080px (matches reference layout data)

**Implementation Details**:

Both `screenshot` and `check-console` commands use `chromiumoxide` with the following configuration:

```rust
let webgpu_flags = vec![
    "--enable-unsafe-webgpu",
    "--enable-webgpu-developer-features",
    "--enable-features=Vulkan,VulkanFromANGLE",
    "--enable-vulkan",
    "--use-angle=vulkan",
    "--disable-software-rasterizer",
    "--ozone-platform=x11",
];

Browser::launch(
    BrowserConfig::builder()
        .with_head()  // WebGPU requires visible window
        .args(webgpu_flags)
        .build()?
)
```

**Why Non-Headless?**
WebGPU hardware acceleration typically requires a visible window (`.with_head()`). Headless mode often falls back to software rendering.

### Manual CDP Testing

For manual automation via Chrome DevTools Protocol:

```bash
# Launch with debugging port
google-chrome \
  --enable-unsafe-webgpu \
  --enable-webgpu-developer-features \
  --remote-debugging-port=9222 \
  --headless=new \
  --disable-gpu-sandbox \
  http://localhost:8080/test_webgpu.html
```

**Note**: Headless WebGPU is experimental. May not work on all systems. Use Rust tools instead.

---

## Recommended Launch Script

Save as `scripts/launch_chrome.sh`:

```bash
#!/bin/bash

google-chrome \
  --enable-unsafe-webgpu \
  --enable-webgpu-developer-features \
  --enable-features=Vulkan,VulkanFromANGLE,DefaultANGLEVulkan,UseSkiaRenderer \
  --enable-vulkan \
  --use-angle=vulkan \
  --disable-software-rasterizer \
  --ozone-platform=x11 \
  --no-first-run \
  --no-default-browser-check \
  --remote-debugging-port=9222 \
  --user-data-dir=/tmp/chrome-canvas-webgpu \
  "$@"
```

```bash
chmod +x scripts/launch_chrome.sh

# Use it
./scripts/launch_chrome.sh http://localhost:8080
```

---

## Summary

**Minimum viable test**:
1. Launch Chrome with `--enable-unsafe-webgpu`
2. Open `chrome://gpu`
3. Check "WebGPU: Hardware accelerated"
4. Test `navigator.gpu` is defined
5. Test `requestAdapter()` returns non-null, non-fallback adapter

**If all pass**: You're ready to develop!

**If any fail**: Debug with troubleshooting section above.

---

## Verified Configuration

**Tested on**:
- OS: Linux (Ubuntu 22.04+)
- Chrome: 141.0.7390.122
- GPU: NVIDIA with proprietary drivers 550+
- Test: https://webgpu.github.io/webgpu-samples/samples/helloTriangle
- Result: ✅ Red triangle visible

**Your configuration**: (fill in after testing)
- OS: ________________
- Chrome version: ________________
- GPU: ________________
- Vulkan working: [ ] Yes [ ] No
- WebGPU test: [ ] Pass [ ] Fail
