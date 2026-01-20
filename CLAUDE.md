# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Headless mode (default) - renders to PNG
cargo build
cargo run                    # outputs to output/screenshot.png

# Windowed mode - desktop window with live rendering
cargo build --features windowed
cargo run --features windowed

# Web/WASM mode
just build-web               # compile WASM + generate JS bindings
just web                     # build, serve, and open Chromium
just serve                   # start dev server on :8000

# Screenshots
just screenshot              # native headless render to output/screenshot.png
just screenshot-open         # same + open the image
just web-screenshot          # web render to output/web_screenshot.png
just web-screenshot-open     # same + open the image

# Setup (installs wasm-bindgen-cli, miniserve, wasm32 target)
just setup
```

## Architecture

### Execution Modes

Three mutually exclusive rendering modes controlled by features and target:

1. **Headless** (default): Renders to offscreen texture, exports PNG via `capture.rs`
2. **Windowed** (`--features windowed`): Desktop window via winit, continuous render loop
3. **Web** (`target_arch = "wasm32"`): WebGPU in browser via canvas element

### Module Structure

- `lib.rs` - Library root, exports `shader_bindings` module (auto-generated from shaders)
- `main.rs` - Binary entry, dispatches to windowed or headless mode
- `renderer.rs` - Headless wgpu renderer (no surface)
- `window_mode.rs` - Windowed renderer with winit event loop
- `web.rs` - WebGPU renderer for WASM target
- `capture.rs` - GPU texture readback and PNG export
- `constants.rs` - WIDTH (800), HEIGHT (600), TEXTURE_FORMAT (Rgba8UnormSrgb)

### Shader Pipeline

1. `build.rs` compiles `shaders/rectangle.slang` → WGSL using `slangc`
2. `wgsl_bindgen` generates Rust bindings in `$OUT_DIR/shader_bindings.rs`
3. Bindings included via `include!()` macro in `lib.rs`

The generated `shader_bindings::rectangle` module provides:
- `create_shader_module_embed_source()` - ShaderModule from embedded WGSL
- `create_pipeline_layout()` - PipelineLayout
- `vs_main_entry()` / `fs_main_entry()` - Entry point configurations
- `vertexInput_0` - Typed vertex struct with `new()` constructor

### Dependencies

- **slangc** (external): Required for shader compilation. Download from https://github.com/shader-slang/slang/releases
- **wgpu 25**: GPU abstraction layer
- **winit 0.30** (optional): Window management for windowed mode
- **web-sys 0.3** (wasm32): DOM/canvas bindings for web mode

## Web Mode Details

Chromium flags for WebGPU on Linux (automatically set by `just open-browser`):
```
--enable-unsafe-webgpu --enable-features=Vulkan,WebGPU,UseSkiaRenderer --use-angle=vulkan
```

Screenshot capture uses Chrome DevTools Protocol via `websocat`.
