# Hot-Reload System Documentation

This document explains how the hot-reload system works for both native and web targets.

## Overview

The hot-reload system allows you to edit Rust source files or shaders and see changes automatically without manually restarting the application. State (camera position, current demo, overlay settings) is preserved across reloads.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      raybox-dev (Dev Server)                    │
│                                                                 │
│  File Watcher (notify)         WebSocket Server (:9300)         │
│    └─ Watch: src/**/*.rs         └─ Broadcasts events:          │
│    └─ Watch: shaders/*.slang        - BuildStarted              │
│                                      - BuildCompleted           │
│  On change:                          - ShaderReloaded           │
│    1. Rebuild (cargo/wasm-pack)      - WasmReload               │
│    2. Broadcast reload event                                    │
└─────────────────────────────────────────────────────────────────┘
                              │
                    WebSocket events
                              │
          ┌───────────────────┴───────────────────┐
          ▼                                       ▼
┌─────────────────────────┐           ┌─────────────────────────┐
│   Native Window         │           │   Browser (WASM)        │
│                         │           │                         │
│ On rebuild:             │           │ On WasmReload event:    │
│  - Save state to file   │           │  - Save state to JS var │
│  - Dev server restarts  │           │  - Reload WASM module   │
│    the process          │           │  - Restore state        │
│  - Restore state        │           │                         │
└─────────────────────────┘           └─────────────────────────┘
```

## Native Hot-Reload

### How It Works

1. **File Watcher**: The `raybox-dev` binary uses the `notify` crate to watch for changes in:
   - `src/**/*.rs` - Rust source files
   - `shaders/*.slang` - Shader files

2. **Rebuild**: When changes are detected:
   - Rust changes: Full `cargo build` is triggered
   - Shader changes: Only shaders are recompiled via `slangc`

3. **State Preservation**: Before the demo process is killed:
   - State is saved to `.raybox_state.json`
   - Includes: current demo, camera position/orientation, overlay mode, time offset

4. **Restart**: The demo process is restarted and loads the saved state

### Files Involved

- `src/bin/raybox_dev.rs` - Dev server with file watcher
- `src/hot_reload/watcher.rs` - File change detection
- `src/hot_reload/builder.rs` - Build invocation
- `src/hot_reload/state.rs` - `ReloadableState` struct
- `src/hot_reload/shader_loader.rs` - Runtime shader compilation
- `src/demos/runner.rs` - `save_state()` and `restore_state()` methods

### Usage

```bash
# Start native hot-reload development
just dev

# The demo window opens automatically
# Edit any .rs or .slang file and save
# The app rebuilds and restarts with state preserved
```

## Web Hot-Reload

### How It Works

1. **File Watcher**: Same as native - watches `.rs` and `.slang` files

2. **Rebuild**: When changes are detected:
   - Runs `cargo build --target wasm32-unknown-unknown`
   - Runs `wasm-bindgen` to generate JS bindings

3. **WebSocket Notification**: Dev server broadcasts `WasmReload` event to all connected browsers

4. **Browser-Side Reload**:
   - JavaScript receives the event
   - Calls `save_state_for_reload()` (wasm_bindgen export) to serialize state
   - Reloads the WASM module with cache-busting (`?t=timestamp`)
   - Calls `restore_state()` to apply saved state

### Files Involved

- `src/bin/raybox_dev.rs` - Dev server (broadcasts `WasmReload` event)
- `src/web.rs` - WASM exports: `save_state_for_reload()`, `restore_state()`, `cleanup_for_reload()`
- `src/web_control.rs` - `WebCommand::Reload` handling
- `src/control/protocol.rs` - `Event::WasmReload` definition
- `index.html` - JavaScript hot-reload logic

### State Preserved

The `WebReloadableState` struct preserves:
- `current_demo` - Which demo is active (0-6)
- `camera_distance`, `camera_azimuth`, `camera_elevation` - Orbital camera state
- `camera_target` - Camera look-at point
- `overlay_mode` - "off", "app", or "full"
- `show_keybindings` - Whether K overlay is visible
- `text2d_offset`, `text2d_scale`, `text2d_rotation` - 2D demo state
- `time_offset` - For animation continuity

### Usage

```bash
# Terminal 1: Start web hot-reload development
just dev-web

# Terminal 2: Open browser with hot-reload enabled
just open-browser-hotreload

# Or manually open: http://localhost:8000?hotreload=1&control=1
```

## URL Parameters (Web)

| Parameter | Description |
|-----------|-------------|
| `demo=N` | Start with demo N (0-6) |
| `control=1` | Enable control server connection |
| `hotreload=1` | Enable hot-reload WebSocket listener |
| `control_url=ws://...` | Custom WebSocket URL |

## Verification Steps

### Native Hot-Reload

1. Start dev server:
   ```bash
   just dev
   ```

2. Wait for initial build and demo window to appear

3. Edit a shader file (e.g., `shaders/sdf_spheres.slang`):
   - Change a color value
   - Save the file

4. Observe:
   - Console shows "Detected 1 file change(s)"
   - Console shows "Recompiling shaders..."
   - Demo updates with new shader (no restart needed for shader-only changes)

5. Edit a Rust file (e.g., `src/demos/spheres.rs`):
   - Add a comment or change something visible
   - Save the file

6. Observe:
   - Console shows "Rebuilding (Rust changes detected)..."
   - Demo restarts but camera position is preserved
   - Current demo selection is preserved

### Web Hot-Reload

1. Start dev server in web mode:
   ```bash
   just dev-web
   ```

2. Open browser with hot-reload:
   ```bash
   just open-browser-hotreload
   ```

3. In the browser:
   - Switch to a different demo (press 2 for Spheres)
   - Move the camera (W/S to zoom, A/D to rotate)
   - Open stats overlay (press F)

4. Edit a Rust file and save

5. Observe:
   - Browser shows "Reloading..." indicator
   - After reload:
     - Same demo is still selected
     - Camera position is preserved
     - Overlay state is preserved

6. Check browser console for:
   - "Received wasmReload event from dev server"
   - "Saved state: {...}"
   - "Hot-reload completed"

### Manual Hot-Reload Trigger

You can trigger a hot-reload manually from the browser console:

```javascript
// Trigger WASM hot-reload
window.rayboxHotReload()
```

## Troubleshooting

### Native

- **Build fails**: Check console for compiler errors. The previous version keeps running.
- **State not restored**: Check if `.raybox_state.json` exists and is valid JSON.

### Web

- **WebSocket not connecting**: Ensure dev server is running on port 9300.
- **WASM not reloading**: Check that `?hotreload=1` is in the URL.
- **State not restored**: Check browser console for JSON parse errors.

## Implementation Details

### State File Location

- Native: `.raybox_state.json` in project root
- Web: Stored in JavaScript variable (not persisted to disk)

### WebSocket Protocol

Events broadcast by dev server:
```json
{"event": {"type": "buildStarted"}}
{"event": {"type": "buildCompleted", "success": true, "error": null}}
{"event": {"type": "wasmReload"}}
{"event": {"type": "shaderReloaded", "shaderName": "sdf_spheres"}}
```

### WASM Exports for Hot-Reload

```rust
#[wasm_bindgen]
pub fn save_state_for_reload() -> String;  // Returns JSON

#[wasm_bindgen]
pub fn restore_state(json: &str);  // Apply saved state

#[wasm_bindgen]
pub fn cleanup_for_reload();  // Clean up before reload
```
