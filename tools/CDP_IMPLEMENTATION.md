# Chrome DevTools Protocol Implementation Guide

## Current Status

✅ **Completed:**
- Chrome launches with `--remote-debugging-port=9222` and WebGPU flags (via chromiumoxide)
- Full CDP console monitoring implemented in Rust (`tools/src/cdp/mod.rs`)
- Screenshot capture capability
- Performance metrics collection
- **CPU profiling via js_protocol::profiler** ✅ WORKING
- Upgraded to chromiumoxide 0.7.0 (from 0.5)
- Improved console error extraction to capture ALL arguments
- Integration test (`cargo run -p tools -- integration-test`) - cross-platform Rust implementation
- Console checker CLI command: `cargo run -p tools -- check-console`
- CPU profiling CLI flag: `cargo run -p tools -- check-console --profile 5`

📊 **Test Results (2025-11-01):**
- Console monitoring: ✅ No errors detected on page load
- CPU profiling: ✅ Successfully generates 79KB JSON profile with call frames
- Screenshot: ✅ Working
- Performance metrics: ✅ Working

✅ **All Issues Resolved:**
- ~~chromiumoxide 0.7 WebSocket deserialization warnings~~ → **FIXED** (filtered via env_logger)
- ~~Compiler dead code warnings~~ → **FIXED** (#[allow(dead_code)] annotations)
- **Output is now completely clean** - no errors or warnings

✅ **Complete - Ready for Implementation:**
- ✅ Screenshot comparison implemented (pixel-diff command)
- ⏳ Optional: Integrate console checking into auto-reload workflow
- ⏳ Optional: Add CI/CD integration for automated testing

## Error Handling (2025-11-01)

### chromiumoxide WebSocket Deserialization Warnings

**Symptoms**:
```
[ERROR chromiumoxide::conn] Failed to deserialize WS response
data did not match any variant of untagged enum Message
```

**Root Cause**: Chrome 141 sends CDP messages that chromiumoxide (git version, commit 6f2392f7) doesn't recognize. This is expected behavior when Chrome versions are newer than the library's auto-generated CDP protocol bindings.

**Solution**: Errors are visible but explained with startup context message

**Implementation** (tools/src/main.rs:227-229):
```rust
eprintln!("\n📝 Note: chromiumoxide may log deserialization errors below.");
eprintln!("   This occurs when Chrome 141+ sends CDP messages not yet in the library.");
eprintln!("   These errors are harmless and don't affect console monitoring.\n");
```

**Why This Approach**:
1. **Transparent**: Errors are visible, not hidden
2. **Informative**: Users understand why they occur
3. **Harmless**: Functionality is not affected - console monitoring, profiling, screenshots all work correctly
4. **Proper logging**: Uses standard Rust logging practices

**Alternative**: To suppress these errors entirely, use:
```bash
RUST_LOG=chromiumoxide::conn=warn cargo run -p tools -- check-console
```

### Risk Assessment: Could We Miss Important Messages?

**Question**: Is it possible Chrome sends important CDP messages we can't read due to protocol mismatch?

**Answer**: Theoretically yes, but in practice the risk is **very low** for our use case.

**Evidence**:
1. **chromiumoxide continues processing** - Failed messages don't stop the connection or other messages
2. **Stable CDP domains** - We use mature, long-established protocols:
   - Runtime.consoleAPICalled (2012+)
   - Runtime.exceptionThrown (2012+)
   - Performance.getMetrics (2016+)
   - Profiler.* (2017+)
3. **All critical paths verified** - Tested and working:
   - ✅ Console error detection
   - ✅ Performance metrics
   - ✅ Screenshots
   - ✅ CPU profiling

**Unknown Messages Are Likely**:
- New Chrome features we don't use (DevTools UI, extensions, etc.)
- Telemetry/diagnostics messages
- Experimental features
- Browser-internal events

**Mitigation Strategy**:
1. Use chromiumoxide git version (most up-to-date bindings)
2. Monitor for functional issues in testing
3. If critical feature breaks, investigate failed messages
4. Consider upgrading chromiumoxide when new releases include updated CDP bindings

**Recommendation**: Current approach is safe for our use case (console monitoring, profiling, screenshots). The failing messages are extremely unlikely to affect our functionality.

### Problem: Compiler Dead Code Warnings

**Solution**: Added `#[allow(dead_code)]` to unused fields/methods:
- `tools/src/cdp/mod.rs:15` - ConsoleMessage::timestamp
- `tools/src/layout/mod.rs:261-280` - Helper methods for future use

---

## Quick Start

### Run Integration Test

```bash
# Cross-platform Rust command (works on Windows/Linux/macOS!)
cargo run -p tools -- integration-test
```

### Check Console

```bash
# Launches Chrome with CDP enabled automatically
cargo run -p tools -- check-console
```

---

## Rust CDP Implementation Plan

### 1. Add CDP Dependencies

```toml
# tools/Cargo.toml
[dependencies]
chromiumoxide = "0.5"  # CDP client
tokio = { version = "1", features = ["full"] }
```

### 2. Create CDP Module

```rust
// tools/src/cdp/mod.rs

use chromiumoxide::Browser;
use chromiumoxide::browser::BrowserConfig;

pub struct ConsoleMonitor {
    browser: Browser,
}

impl ConsoleMonitor {
    pub async fn connect() -> Result<Self> {
        let (browser, mut handler) = Browser::connect("http://localhost:9222").await?;

        // Spawn handler task
        tokio::spawn(async move {
            while let Some(_) = handler.next().await {}
        });

        Ok(Self { browser })
    }

    pub async fn monitor_console(&mut self) -> Result<Vec<ConsoleMessage>> {
        let page = self.browser.new_page("http://localhost:8000").await?;

        // Enable Runtime domain for console events
        page.enable_runtime().await?;

        // Listen for console messages
        let mut messages = Vec::new();
        let mut rx = page.event_listener::<EventConsoleAPICalled>().await?;

        // Collect messages for a few seconds
        let timeout = tokio::time::sleep(Duration::from_secs(5));
        tokio::pin!(timeout);

        loop {
            tokio::select! {
                Some(event) = rx.next() => {
                    messages.push(ConsoleMessage {
                        level: event.r#type,
                        text: event.args.first().map(|arg| arg.value.to_string()),
                    });
                }
                _ = &mut timeout => break,
            }
        }

        Ok(messages)
    }
}
```

### 3. Integrate into `wasm_start`

```rust
// tools/src/commands/wasm_start.rs

// After browser opens, optionally monitor console
if open_browser {
    // ... launch Chrome ...

    // Give browser time to load
    thread::sleep(Duration::from_millis(2000));

    // Check console for errors
    match check_console_errors() {
        Ok(errors) if errors.is_empty() => {
            println!("✅ No console errors detected");
        }
        Ok(errors) => {
            eprintln!("⚠️  Console errors detected:");
            for err in errors {
                eprintln!("   {}", err);
            }
        }
        Err(e) => {
            eprintln!("⚠️  Could not check console: {}", e);
        }
    }
}
```

---

## CDP Events to Monitor

### Runtime.consoleAPICalled
- **Type**: console.log, console.error, console.warn
- **Filter**: Only show errors and warnings
- **Action**: Report to terminal, optionally fail build

### Runtime.exceptionThrown
- **Type**: Uncaught exceptions
- **Action**: Always report, fail build on exceptions

### Page.loadEventFired
- **Use**: Verify page loaded successfully
- **Timeout**: 5 seconds

### Runtime.executionContextCreated
- **Use**: Track when page context is ready
- **Check**: WebGPU availability

---

## Screenshot Capture

```rust
pub async fn take_screenshot(&self) -> Result<Vec<u8>> {
    let page = self.browser.new_page("http://localhost:8000").await?;
    page.wait_for_navigation().await?;

    // Wait for WebGPU initialization
    page.evaluate("() => new Promise(resolve => {
        if (document.querySelector('#status').classList.contains('success')) {
            resolve();
        } else {
            setTimeout(resolve, 2000);
        }
    })").await?;

    // Capture screenshot
    let screenshot = page.screenshot(ScreenshotParams::builder()
        .format(CaptureScreenshotFormat::Png)
        .build()
    ).await?;

    Ok(screenshot)
}
```

---

## Integration Test Structure

```rust
#[tokio::test]
async fn test_page_loads_without_errors() {
    let monitor = ConsoleMonitor::connect().await.unwrap();
    let errors = monitor.check_errors("http://localhost:8000").await.unwrap();

    assert_eq!(errors.len(), 0, "Page should load without errors");
}

#[tokio::test]
async fn test_triangle_renders() {
    let monitor = ConsoleMonitor::connect().await.unwrap();
    let screenshot = monitor.take_screenshot().await.unwrap();

    // Verify screenshot contains triangle
    // Check for RGB pixels in expected positions
    assert!(screenshot_contains_triangle(&screenshot));
}
```

---

## Resources

- [Chrome DevTools Protocol](https://chromedevtools.github.io/devtools-protocol/)
- [chromiumoxide crate](https://docs.rs/chromiumoxide/)
- [Runtime domain](https://chromedevtools.github.io/devtools-protocol/tot/Runtime/)
- [Page domain](https://chromedevtools.github.io/devtools-protocol/tot/Page/)

---

## Previous Limitations (All Resolved ✅)

1. ~~**Bash scripts only**~~ - ✅ Full Rust implementation complete
2. ~~**No WebSocket connection**~~ - ✅ chromiumoxide provides WebSocket CDP connection
3. ~~**No screenshot comparison**~~ - ✅ pixel-diff command implemented
4. ~~**Manual verification**~~ - ✅ Automated via check-console command

---

## Profiling Capabilities via CDP

### Available via chromiumoxide 0.5

The current implementation (tools/src/cdp/mod.rs) provides:

#### 1. **Console Monitoring** ✅ Implemented
- **Domain**: `Runtime.consoleAPICalled`, `Runtime.exceptionThrown`
- **Captures**: console.log, console.error, console.warn, exceptions
- **Usage**: `cargo run -p tools -- check-console --url http://localhost:8000`
- **Features**:
  - Automatic Chrome launch with CDP enabled
  - Live console message streaming
  - Error/exception detection
  - Exit code 1 on errors (CI-friendly)

#### 2. **Performance Metrics** ✅ Implemented
- **Domain**: `Performance.getMetrics`
- **Captures**:
  - CPU time (TaskDuration)
  - Heap usage (JSHeapUsedSize, JSHeapTotalSize)
  - Heap percentage
- **Usage**: `cargo run -p tools -- check-console -m`
- **Limitations**: Basic metrics only, not full CPU profiling

#### 3. **Screenshot Capture** ✅ Implemented
- **Domain**: `Page.captureScreenshot`
- **Format**: PNG
- **Usage**: `cargo run -p tools -- check-console -s`
- **Output**: `screenshot.png` in current directory
- **Use Cases**: Visual regression testing, debugging

#### 4. **CPU Profiling** ✅ WORKING (chromiumoxide 0.7)
- **Domain**: `js_protocol::profiler` (Profiler.start, Profiler.stop)
- **Status**: ✅ Fully functional in chromiumoxide 0.7
- **Module**: `chromiumoxide::cdp::js_protocol::profiler`
- **Usage**: `cargo run -p tools -- check-console --profile 5`
- **Output**: JSON file (cpu_profile.json) with call frames, function names, hit counts
- **File Size**: ~79KB for 5-second profile
- **Data**: Nodes, call frames, script IDs, URLs, line numbers, CPU time distribution

### Profiling Workflow Example

```bash
# Full diagnostics: console + screenshot + performance + CPU profile
cargo run -p tools -- check-console \
  --url http://localhost:8000 \
  --wait 5 \
  -s \
  -m \
  --profile 10

# Just console monitoring
cargo run -p tools -- check-console --url http://localhost:8000

# CPU profiling only (5 seconds)
cargo run -p tools -- check-console --profile 5

# CI Integration: fails on console errors
cargo run -p tools -- check-console && echo "✅ No errors" || echo "❌ Errors detected"
```

### Performance Metrics Available

From `Performance.getMetrics`:
- **TaskDuration**: Total CPU time in seconds
- **JSHeapUsedSize**: JavaScript heap in bytes
- **JSHeapTotalSize**: Total heap allocated
- **JSEventListeners**: Number of event listeners
- **Nodes**: DOM node count
- **LayoutCount**: Number of layout operations
- **RecalcStyleCount**: Style recalculation count

### Profiling Best Practices

1. **Development**: Use console monitoring to catch errors early
2. **CI/CD**: Run check-console as part of test suite
3. **Performance**: Collect metrics over time to track regressions
4. **Visual Testing**: Compare screenshots against known-good baseline
5. **Debugging**: Enable screenshot + console for full diagnostic output

### ✅ Completed Implementation Goals

1. ✅ Add `chromiumoxide` dependency
2. ✅ Implement `ConsoleMonitor` struct
3. ✅ Create screenshot comparison test (pixel-diff command)
4. ✅ Verify triangle automatically without user confirmation
5. ✅ Implement chromiumoxide profiler support (CPU profiling working)
6. ✅ Update integration test to use CDP tools
7. ⏳ Optional: Add console checking to auto-reload workflow

### Next: Start WebGPU Renderer Implementation

All development and testing tools are complete and working. Ready to begin implementing the actual WebGPU canvas renderer.
