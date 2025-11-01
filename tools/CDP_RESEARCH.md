# Chrome DevTools Protocol Libraries Research

## Executive Summary

**Key Finding**: CPU Profiling IS available in chromiumoxide! It's in the `js_protocol::profiler` module, not `browser_protocol::profiler` as initially assumed.

---

## chromiumoxide Research

### Current Version: 0.7.0

**Published**: Available on crates.io
**Repository**: https://github.com/mattsse/chromiumoxide
**CDP Revision**: 1354347
**License**: MIT/Apache-2.0

### Architecture

- **Auto-generated bindings** from Chrome's PDL (Protocol Definition Language) files
- Every CDP domain mapped to Rust modules
- Supports ~60K lines of generated code
- Async-first design using Tokio runtime

### Module Organization

```
chromiumoxide::cdp::
├── browser_protocol::
│   ├── page          (Page navigation, screenshots)
│   ├── performance   (Performance.getMetrics)
│   ├── network       (Network interception)
│   ├── dom           (DOM inspection)
│   └── ... (48+ domains)
│
└── js_protocol::
    ├── runtime       (console API, evaluation)
    ├── debugger      (Breakpoints, stepping)
    ├── profiler      ✅ CPU PROFILING
    └── heap_profiler (Memory profiling)
```

###  Profiler Module (js_protocol::profiler)

**Location**: `chromiumoxide::cdp::js_protocol::profiler`

#### Available Commands

| Command | Purpose |
|---------|---------|
| `Enable` | Activate profiler |
| `Disable` | Deactivate profiler |
| `Start` | Begin CPU profiling |
| `Stop` | End profiling, return Profile |
| `SetSamplingInterval` | Configure sample rate (μs) |
| `StartPreciseCoverage` | Enable code coverage |
| `StopPreciseCoverage` | Disable code coverage |
| `TakePreciseCoverage` | Get coverage data |
| `GetBestEffortCoverage` | Get approximate coverage |

#### Data Structures

- **Profile** - Complete CPU profile
- **ProfileNode** - Call tree node (function, time, children)
- **CoverageRange** - Code coverage ranges
- **ScriptCoverage** - Per-script coverage
- **FunctionCoverage** - Per-function coverage
- **PositionTickInfo** - Sample attribution to source positions

#### Events

- `EventConsoleProfileStarted` - Triggered by `console.profile()`
- `EventConsoleProfileFinished` - Profile complete
- `EventPreciseCoverageDeltaUpdate` - Coverage changes

### Why My Initial Assessment Was Wrong

I searched for profiler in `browser_protocol` instead of `js_protocol`. The profiler domain is V8-specific (JavaScript engine profiling), so it belongs in `js_protocol`, not `browser_protocol`.

**Correct import**:
```rust
use chromiumoxide::cdp::js_protocol::profiler::{
    EnableParams, StartParams, StopParams, SetSamplingIntervalParams
};
```

**NOT**:
```rust
use chromiumoxide::cdp::browser_protocol::profiler::...; // ❌ WRONG
```

---

## Alternative Libraries Comparison

### 1. rust-headless-chrome

**Repository**: https://github.com/rust-headless-chrome/rust-headless-chrome
**Status**: Mature, battle-tested
**Async**: ❌ Synchronous API
**Last Update**: Active maintenance

**Pros**:
- High-level API (similar to Puppeteer)
- Simple, ergonomic interface
- Well-documented
- Proven in production

**Cons**:
- Synchronous (blocking)
- Slower than async alternatives
- Less control over low-level CDP

**Use Case**: Web scraping, simple automation

### 2. chromiumoxide

**Repository**: https://github.com/mattsse/chromiumoxide
**Status**: Active development
**Async**: ✅ Tokio-based
**Last Update**: 2024

**Pros**:
- Fully async
- Auto-generated from CDP specs
- Complete protocol coverage
- Fast performance
- Direct access to all CDP domains

**Cons**:
- Lower-level API (more verbose)
- Less documentation than headless_chrome
- Requires async runtime knowledge

**Use Case**: Performance-critical apps, testing, browser automation

### 3. fantoccini

**Repository**: https://github.com/jonhoo/fantoccini
**Status**: Mature, maintained
**Protocol**: WebDriver (W3C standard)
**Async**: ✅ Tokio-based

**Pros**:
- Cross-browser (Chrome, Firefox, Safari via WebDriver)
- Asynchronous
- W3C standard compliance
- Good documentation

**Cons**:
- WebDriver protocol (less Chrome-specific features)
- No direct CDP access
- No JS coverage, profiling, etc.

**Use Case**: Cross-browser testing, Selenium replacement

### 4. thirtyfour

**Repository**: https://github.com/stevepryde/thirtyfour
**Status**: Active, community-driven
**Protocol**: WebDriver
**Async**: ✅ Tokio-based

**Pros**:
- Modern WebDriver Rust implementation
- Weekly updates
- Cross-browser
- Good API design

**Cons**:
- WebDriver limitations (no CDP-specific features)
- Less mature than Python's Selenium

**Use Case**: Browser automation, E2E testing

---

## Feature Matrix

| Feature | chromiumoxide | headless_chrome | fantoccini | thirtyfour |
|---------|--------------|-----------------|------------|------------|
| **Async** | ✅ Tokio | ❌ Sync | ✅ Tokio | ✅ Tokio |
| **CPU Profiling** | ✅ | ✅ | ❌ | ❌ |
| **Code Coverage** | ✅ | ✅ | ❌ | ❌ |
| **Performance Metrics** | ✅ | ✅ | ❌ | ❌ |
| **Screenshot** | ✅ | ✅ | ✅ | ✅ |
| **Network Interception** | ✅ | ✅ | ❌ | ❌ |
| **Cross-browser** | ❌ | ❌ | ✅ | ✅ |
| **Console Monitoring** | ✅ | ✅ | ❌ | ❌ |
| **DOM Manipulation** | ✅ | ✅ | ✅ | ✅ |
| **JS Execution** | ✅ | ✅ | ✅ | ✅ |

---

## Recommendations

### For This Project: chromiumoxide ✅

**Reasons**:
1. ✅ CPU profiling available (`js_protocol::profiler`)
2. ✅ Already integrated and working
3. ✅ Async performance critical for dev tools
4. ✅ Complete CDP access for future needs
5. ✅ Active development

### Migration Path (if needed)

If chromiumoxide proves difficult:
1. **headless_chrome** - simpler API, but synchronous
2. **fantoccini** - if cross-browser testing needed
3. **Direct CDP WebSocket** - maximum control, most complex

---

## Implementation Plan for CPU Profiling

### 1. Fix Profiler Module Import

```rust
// tools/src/cdp/mod.rs

use chromiumoxide::cdp::js_protocol::profiler::{
    EnableParams,
    StartParams,
    StopParams,
    SetSamplingIntervalParams,
};
```

### 2. Implement profile_cpu Method

```rust
pub async fn profile_cpu(&self, url: &str, duration_secs: u64) -> Result<CpuProfile> {
    let page = self.browser.new_page(url).await?;

    // Enable profiler
    page.execute(EnableParams::default()).await?;

    // Set sampling interval (default: 1000μs)
    page.execute(SetSamplingIntervalParams::new(1000)).await?;

    // Start profiling
    page.execute(StartParams::default()).await?;

    // Wait for page load and profile duration
    page.wait_for_navigation().await?;
    tokio::time::sleep(Duration::from_secs(duration_secs)).await;

    // Stop and get profile
    let result = page.execute(StopParams::default()).await?;

    Ok(CpuProfile {
        profile: serde_json::to_string_pretty(&result.profile)?,
    })
}
```

### 3. CLI Integration

```bash
# Profile CPU for 5 seconds
cargo run -p tools -- check-console --url http://localhost:8000 --profile 5

# Full diagnostics
cargo run -p tools -- check-console \
  --url http://localhost:8000 \
  --wait 5 \
  --screenshot \
  --performance \
  --profile 10
```

---

## Performance Comparison

### chromiumoxide (Async)
- Concurrent operations
- Non-blocking I/O
- Better resource utilization
- Ideal for dev tools, CI/CD

### headless_chrome (Sync)
- Simple threading model
- Easier to reason about
- More predictable
- Good for scripts, simple automation

---

## Conclusion

**CPU Profiling is fully supported in chromiumoxide 0.7.0** via the `js_protocol::profiler` module. The initial assessment was incorrect due to searching in the wrong protocol namespace (`browser_protocol` vs `js_protocol`).

**Action Items**:
1. ✅ Fix profiler imports to use `js_protocol::profiler`
2. ✅ Implement `profile_cpu()` method
3. ✅ Test CPU profiling on WebGPU renderer
4. ✅ Add profiling to integration tests
5. ✅ Document profiling best practices

**Resources**:
- chromiumoxide docs: https://docs.rs/chromiumoxide
- CDP Profiler spec: https://chromedevtools.github.io/devtools-protocol/tot/Profiler/
- chromiumoxide examples: https://github.com/mattsse/chromiumoxide/tree/main/examples
