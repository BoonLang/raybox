# CRITICAL FIXES NEEDED - Auto-Reload Server Issues

**Date**: 2025-11-01
**Status**: 🔴 URGENT - Server not working, auto-reload broken

---

## 🚨 CRITICAL RULES - READ FIRST

### Browser Rules (VERY IMPORTANT)
1. **NEVER kill Firefox** - It's the user's default browser
2. **NEVER use KillShell on browser-opening commands** - This kills the browser!
3. **ONLY kill background Rust build processes**, not browsers
4. **Use Chrome explicitly**: `google-chrome http://localhost:8000 2>/dev/null &`
5. **Check BROWSER env var** if using `open` crate - might default to Firefox

---

## 🐛 Current Problem

### Issue: Rust Server Starts But Doesn't Accept Connections

**Symptoms**:
- `wasm-start` command says "Server running on http://localhost:8000"
- File watcher shows "👀 Watching for file changes..."
- But `curl http://localhost:8000` fails (connection refused)
- Browser can't connect

**Root Cause** (in `tools/src/commands/wasm_start.rs`):

```rust
// Line 48 - BUG IS HERE
start_server(port, Arc::clone(&build_id))?;

// Watcher thread runs forever, but server thread exits!
watcher_thread.join().unwrap();
```

**The Problem**:
- `start_server()` spawns the watcher thread (line 26-30)
- `start_server()` then calls the blocking server (line 159-164)
- Server runs in `block_on()` which BLOCKS
- Function never returns
- When it does return (on error), watcher is already spawned
- Then line 51 tries to join watcher that's in a different context

**What's Happening**:
The server thread and watcher thread aren't coordinating properly. The server blocks but then the function structure is wrong.

---

## 🔧 THE FIX

### Option 1: Simple Fix - Spawn Server in Background Thread

**File**: `tools/src/commands/wasm_start.rs`

**Change around line 48**:

```rust
// BEFORE (BROKEN):
start_server(port, Arc::clone(&build_id))?;
watcher_thread.join().unwrap();

// AFTER (FIXED):
// Start server in background thread
let server_port = port;
let server_build_id = Arc::clone(&build_id);
let server_thread = thread::spawn(move || {
    if let Err(e) = start_server(server_port, server_build_id) {
        eprintln!("Server error: {}", e);
    }
});

// Give server time to start
thread::sleep(Duration::from_millis(500));

// Open browser AFTER server starts
if open_browser {
    let url = format!("http://localhost:{}", port);
    println!("Opening browser: {}", url);
    if let Err(e) = open::that(&url) {
        eprintln!("Failed to open browser: {}", e);
    }
}

// Wait for both threads
// Note: They both run forever, so this never returns (Ctrl+C kills)
let _ = server_thread.join();
watcher_thread.join().unwrap();
```

**Why This Works**:
- Server runs in its own thread (doesn't block main)
- Watcher runs in its own thread
- Main thread waits for both
- Both threads run forever until Ctrl+C

### Option 2: Move Browser Opening BEFORE Server Start

The current code opens browser (line 39-45) BEFORE server starts (line 48), which is why Firefox can't connect!

**Better Structure**:
1. Spawn watcher thread
2. Spawn server thread
3. Wait for server to be ready
4. THEN open browser
5. Join threads

---

## 🧪 TESTING CHECKLIST

After fixing the server, test this EXACT sequence:

### Test 1: Server Starts and Responds
```bash
cd ~/repos/canvas_3d_6
just start-wasm-open

# In another terminal:
curl http://localhost:8000/
# Should return HTML, not "connection refused"
```

### Test 2: Chrome Opens (Not Firefox)
```bash
# After running just start-wasm-open:
# 1. Check which browser opens
# 2. Should be Chrome, NOT Firefox
# 3. URL should be http://localhost:8000
# 4. Should show triangle (red/green/blue)
```

### Test 3: Rust File Auto-Reload
```bash
# With server running and Chrome open:
# 1. Edit renderer/src/pipeline.rs
# 2. Change line 22: out.color = vec3<f32>(1.0, 1.0, 0.0); // Yellow
# 3. Save file
# 4. Watch terminal for "File change detected, rebuilding..."
# 5. Watch Chrome - should auto-reload in ~2-3 seconds
# 6. Triangle top vertex should now be YELLOW instead of red
```

### Test 4: Shader File Auto-Reload
```bash
# Create a test shader:
echo "// test" > renderer/src/test.wgsl

# Terminal should show:
# "📝 File change detected, rebuilding..."

# Delete test file:
rm renderer/src/test.wgsl
```

### Test 5: Cargo.toml Auto-Reload
```bash
# Edit renderer/Cargo.toml
# Add a comment at the end: # test
# Save file
# Terminal should show rebuild
```

### Test 6: Build Errors Don't Reload
```bash
# Edit renderer/src/lib.rs
# Add syntax error: let x = ;
# Save
# Terminal shows "❌ Build failed"
# Chrome should NOT reload
# Fix error, save again
# Chrome should reload
```

---

## 📝 Key Files to Check

### `tools/src/commands/wasm_start.rs`
- Line 11-53: Main function structure
- Line 26-30: Watcher thread spawn
- Line 39-45: Browser opening (uses `open` crate)
- Line 48: Server start call
- Line 51: Thread join

### Fix the `open` crate browser selection:
```rust
// Line 39-45: Current code uses open::that()
// This respects BROWSER env var, which might be Firefox!

// Better approach:
#[cfg(target_os = "linux")]
std::process::Command::new("google-chrome")
    .arg(&url)
    .spawn()?;

#[cfg(not(target_os = "linux"))]
open::that(&url)?;
```

---

## ✅ Success Criteria

After fixes, you should see:

1. **Server starts** ✓
   - Terminal: "URL: http://localhost:8000"
   - `curl http://localhost:8000` returns HTML

2. **Chrome opens** (NOT Firefox) ✓
   - New Chrome window/tab
   - Shows triangle with RGB gradient

3. **Auto-reload works** ✓
   - Edit any `.rs` file → rebuild → Chrome reloads
   - Edit any `.wgsl` file → rebuild → Chrome reloads
   - Edit `Cargo.toml` → rebuild → Chrome reloads
   - Build error → no reload (stays on old version)

4. **File watcher works** ✓
   - Terminal shows "📝 File change detected, rebuilding..."
   - Shows "✅ Build complete! Browser will reload..."
   - Shows "❌ Build failed" on errors

---

## 🔍 Debugging Tips

### Server won't start:
```bash
# Check if port 8000 is in use:
lsof -i :8000

# Kill any process on port 8000:
lsof -ti:8000 | xargs -r kill -9
```

### Browser issues:
```bash
# Check default browser:
echo $BROWSER

# Explicitly use Chrome:
google-chrome http://localhost:8000 &
```

### Build not triggering:
```bash
# Check what's being watched:
# In wasm_start.rs line 64-69
# Should watch: .rs, .wgsl, Cargo.toml
```

---

## 📚 Related Files

- `tools/src/commands/wasm_start.rs` - Main server + watcher (NEEDS FIX)
- `tools/src/commands/wasm_build.rs` - Build pipeline (working)
- `web/index.html` - Live reload polling (working)
- `renderer/src/pipeline.rs` - Triangle rendering (working)
- `renderer/src/lib.rs` - WebGPU init (working)

---

## ⚠️ What's Working vs Broken

### ✅ Working:
- WASM compilation
- WebGPU triangle rendering
- File watcher detecting changes
- Build pipeline (wasm-bindgen, etc)
- Browser live reload polling script

### 🔴 Broken:
- Rust HTTP server not accepting connections
- Auto-reload workflow incomplete
- Browser selection (opens Firefox instead of Chrome)

---

## 🎯 Next Steps

1. Fix `wasm_start.rs` server thread structure
2. Fix browser opening to use Chrome explicitly
3. Test all 6 auto-reload scenarios
4. Document working auto-reload workflow
5. Continue with Milestone 1 (load layout data)

---

**Once these fixes are done, the development workflow will be fully operational and we can proceed with implementing the TodoMVC renderer!**
