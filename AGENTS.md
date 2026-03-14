# Repository Guidelines

## Project Structure & Module Organization
`src/` contains the Rust library and runtime code. Core rendering and app plumbing live in `src/renderer.rs`, `src/sdf_renderer.rs`, `src/demo_core/`, and `src/control/`. Interactive showcase code lives in `src/demos/`; demo entry points and tools are in `src/bin/` (`demos`, `raybox-ctl`, `raybox-mcp`, `raybox-dev`). Shader sources live in `shaders/`. Reference images and fonts live under `assets/`. Design notes and implementation logs belong in `docs/`, for example `docs/plans/todomvc_classic/`.

## Build, Test, and Development Commands
Use `just` targets when possible:

- `just demos` runs the windowed demo switcher.
- `just demos-from 8` starts directly in a specific demo.
- `just demos-control` runs the demo app with the WebSocket control server enabled.
- `just ctl status` queries the running control server.
- `just build-web` builds the WASM target.
- `just web` builds, serves, and opens the web app in Chromium.
- `just open-browser` launches Chromium through Raybox's WebGPU/browser launcher.
- `just open-browser-hotreload` launches Chromium with `control=1&hotreload=1`.
- `just web-smoke` proves the served web app can launch, answer control, and capture a screenshot.
- `just dev` starts the hot-reload development binary.

For direct Cargo usage:

- `cargo build --bin demos --features windowed,control,mcp`
- `cargo test`
- `cargo run --bin raybox-ctl --features control -- status`
- `cargo run --bin raybox-ctl --features control -- web-open`
- `cargo run --bin raybox-ctl --features control -- web-open --control --hotreload --demo 8`
- `cargo run --bin raybox-ctl --features control -- web-smoke --control --output output/web_smoke.png`

## Graphics Architecture
Tracked shader source belongs only in `shaders/*.slang`. Do not add repo-tracked handwritten WGSL to runtime or example code. The only WGSL allowed outside `shaders/*.slang` is:

- generated shader code emitted into `$OUT_DIR`
- runtime hot-reload compiled output consumed by the shader loader

The generated shader binding layer from `build.rs` + `wgsl_bindgen` is the source of truth for the CPU/GPU ABI of every live shader surface. Do not keep handwritten Rust fallback mirrors for dead or removed shader ABI. In particular:

- prefer generated `*_std140_0` / `*_std430_0` types over manual `#[repr(C)]` buffer structs
- prefer generated bind group and pipeline layout helpers over handwritten layout duplication
- avoid `min_binding_size: None` on uniform bindings when a generated type size is available
- if a new utility shader is needed, add a new `.slang` file and extend `build.rs`; do not embed WGSL strings in repo-tracked runtime or example code
- do not reintroduce the removed per-glyph `GridCell` / `curveIndices` glyph-grid ABI; the active vector-text path uses generated bindings for the live curve/glyph/char-grid surfaces only

For web runs, Chromium is the default supported browser target. Launch it through the repo-managed commands (`just open-browser`, `just open-browser-hotreload`, or `raybox-ctl web-open`) instead of ad hoc manual flags so WebGPU and diagnostics stay consistent.

## Coding Style & Naming Conventions
Use standard Rust formatting with `cargo fmt`; no custom `rustfmt` config is checked in. Follow existing Rust naming: `snake_case` for functions/modules, `CamelCase` for types, `SCREAMING_SNAKE_CASE` for constants. Keep shader and demo names descriptive and aligned with filenames, for example `todomvc_3d.rs` and `sdf_todomvc_3d.slang`. Prefer small, focused helper functions over large monoliths.

## Testing Guidelines
There is no top-level `tests/` directory today; tests are mostly inline unit tests near the code, such as in `src/control/protocol.rs` and `src/text/`. Run `cargo test` before submitting changes. For rendering or UI work, also verify manually with `just demos` or `just demos-control`, and include screenshots when behavior is visual.

## Commit & Review Guidelines
This repository uses `jj`, not Git, for day-to-day history editing. Keep commits focused and use short imperative subjects with a scope prefix when useful, for example `todomvc3d: add classic2d default theme`. In reviews, include:

- what changed and why
- which demos or binaries were tested
- screenshots for visual changes
- any feature flags needed to reproduce the result

Avoid mixing unrelated renderer, control, and demo changes in one commit unless they are required for the same feature.
