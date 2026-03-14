# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Version Control

**Use `jj` (Jujutsu), NOT `git`.** Commands: `jj status`, `jj log`, `jj describe`, `jj new`, etc.

## Build Commands

```bash
# Headless mode (default) - renders to PNG
cargo run                    # outputs to output/screenshot.png

# Windowed mode - desktop window with the full demo switcher
just demos                   # press 0-9/-/= to switch demos, F=stats, K=keybindings
just demos-from 3            # start from specific demo

# Windowed with control server (WebSocket on port 9300)
just demos-control

# Development mode with hot-reload (watches files, auto-rebuilds, restarts)
just dev                     # native
just dev-web                 # web/WASM

# FPS benchmark across all demos (requires running demo with control)
just bench

# Control CLI
just ctl ping                # test connection
just ctl status              # get current demo/FPS/camera
just ctl switch 3            # switch to demo 3
just ctl screenshot          # capture PNG
cargo run --bin raybox-ctl --features control -- web-open
cargo run --bin raybox-ctl --features control -- web-open --control --hotreload --demo 8
cargo run --bin raybox-ctl --features control -- web-smoke --control --output output/web_smoke.png

# Web/WASM mode
just build-web               # compile WASM + generate JS bindings
just web                     # build, serve, and open Chromium
just open-browser            # launch Chromium through Raybox's WebGPU launcher
just open-browser-hotreload  # launch Chromium with control + hot reload URL params
just web-smoke               # prove browser launch + control + screenshot work

# Screenshots
just screenshot              # native headless render
just web-screenshot          # web render

# Setup (installs wasm-bindgen-cli, miniserve, wasm32 target)
just setup
```

## Architecture

### Execution Modes

Four build modes controlled by features:

1. **Headless** (default): Renders to offscreen texture, exports PNG via `capture.rs`
2. **Windowed** (`--features windowed`): Desktop window via winit with overlay (cosmic-text)
3. **Hot-reload** (`--features hot-reload`): Windowed + file watcher + auto-rebuild + control server
4. **Web** (`target_arch = "wasm32"`): WebGPU in browser via canvas element

### Feature Flags

- `windowed` — winit window, sysinfo, overlay (cosmic-text SimpleOverlay)
- `overlay` — cosmic-text CPU-rasterized text overlay (included by `windowed`)
- `control` — WebSocket control server (tokio-tungstenite)
- `hot-reload` — file watcher + builder (includes `windowed` + `control`)
- `mcp` — MCP server (includes `control`)

### Module Structure

- `lib.rs` — library root, exports shader_bindings
- `main.rs` — binary entry, dispatches to windowed or headless
- `demos/` — unified demo system, retained scene hosts, runner, switching
  - `runner.rs` — DemoRunner: window, input, overlay, demo lifecycle
  - `empty.rs`, `objects.rs`, `spheres.rs`, `towers.rs` — simple/fullscreen demos
  - `clay.rs`, `text_shadow.rs`, `todomvc.rs`, `todomvc_3d.rs` — vector-text / retained demo entry points
- `demo_core/` — platform-agnostic demo trait, DemoId, CameraConfig
- `text/` — vector font parsing, packed glyph atlas metadata, and live character-grid acceleration
  - `vector_font.rs` — TTF parsing, Bézier curve extraction
  - `glyph_atlas.rs` — packed curves plus per-glyph metadata for the active brute-force glyph path
  - `char_grid.rs` — character-grid acceleration used by the active vector-text shaders
- `simple_overlay.rs` — cosmic-text CPU-rasterized text overlay
- `input.rs` — input handling, camera controls, stats formatting
- `camera.rs` — FlyCamera with yaw/pitch/roll
- `control/` — WebSocket control protocol
  - `protocol.rs` — Command/Response enums, Request/ResponseMessage
  - `ws_server.rs` — WebSocket server (tokio-tungstenite)
  - `ws_client.rs` — blocking WebSocket client for CLI
  - `state.rs` — SharedControlState (Arc<RwLock>)
- `hot_reload/` — file watcher + cargo builder
- `capture.rs` — GPU texture readback and PNG export
- `constants.rs` — WIDTH (800), HEIGHT (600), TEXTURE_FORMAT (Rgba8UnormSrgb)
- `web.rs`, `web_input.rs`, `web_control.rs` — WASM-specific modules

### Binaries

- `demos` — unified demo app (main binary for windowed mode)
- `raybox-ctl` — CLI control tool
- `raybox-dev` — dev server with hot-reload
- `raybox-mcp` — MCP server

### Shader Pipeline

1. `build.rs` compiles `shaders/*.slang` → WGSL using `slangc`
2. `wgsl_bindgen` generates Rust bindings in `$OUT_DIR/shader_bindings.rs`
3. Bindings included via `include!()` macro in `lib.rs`

Tracked shader source must live only in `shaders/*.slang`. Do not add repo-tracked handwritten WGSL to runtime or example code. The only WGSL allowed outside `shaders/*.slang` is generated output in `$OUT_DIR` or runtime hot-reload compiler output.

Generated shader binding types are the source of truth for the GPU ABI:
- use generated `*_std140_0` / `*_std430_0` Rust types instead of manual `#[repr(C)]` mirrors for every live shader ABI surface
- prefer generated bind group and pipeline layout helpers over duplicating layouts by hand
- avoid `min_binding_size: None` on uniform bindings when the generated type size is known
- if a new utility shader is needed, add a new `.slang` file and extend `build.rs`
- do not reintroduce the removed per-glyph `GridCell` / `curveIndices` glyph-grid ABI; the active vector-text shaders only consume the live curve, glyph, char-instance, and char-grid bindings

Shaders include utility passes and demo shaders such as `empty`, `overlay`, `present`, `rectangle`, `sdf_raymarch`, `sdf_spheres`, `sdf_towers`, `sdf_text2d_vector`, `sdf_clay_vector`, and `sdf_text_shadow_vector`

### Control Protocol

WebSocket on port 9300. JSON messages with `{id, version, command}` / `{id, response}`.

Commands: `switchDemo`, `setCamera`, `screenshot`, `getStatus`, `toggleOverlay`, `pressKey`, `ping`, `reloadShaders`

### Dependencies

- **slangc** (external): Required for shader compilation
- **wgpu 25**: GPU abstraction layer
- **winit 0.30** (optional): Window management
- **cosmic-text** (optional): CPU text rasterization for overlay
- **tokio-tungstenite** (optional): WebSocket for control protocol
- **sysinfo** (optional): System/GPU stats in overlay

## Web Mode Details

Chromium is the default supported browser target for Raybox web runs. Use `just open-browser`, `just open-browser-hotreload`, or `raybox-ctl web-open` instead of ad hoc manual launches so the repo-managed WebGPU flags and diagnostics are applied consistently.

Chromium flags for WebGPU on Linux (automatically set by the repo launcher):
```
--enable-unsafe-webgpu
--enable-webgpu-developer-features
--enable-features=UnsafeWebGPU,SharedArrayBufferOnDesktop,Vulkan,VulkanFromANGLE,DefaultANGLEVulkan,UseSkiaRenderer
--enable-vulkan
--use-angle=vulkan
--ignore-gpu-blocklist
--disable-extensions
--disable-component-extensions-with-background-pages
--disable-background-networking
--disable-sync
--disable-default-apps
--disable-component-update
--metrics-recording-only
--no-service-autorun
--force-color-profile=srgb
```
