# Emergent / Physical TodoMVC

Staging area for the upcoming physically-based (emergent) UI version.

Use this space for:
- `docs/` – design notes, migration guides, interaction rules.
- `design/` – sketches, theme presets, scene/light configs.
- `assets/` – captures of physical prototypes or shader previews.

Goals:
1) Translate TodoMVC into the emergent 3D model (depth + spatial relationships, no painted borders/shadows).
2) Keep reference HTML untouched in `reference/` and the classic renderer in `renderer/`.
3) Store new screenshots, layout exports, and theme definitions here to avoid mixing classic artifacts.
4) Align with Boon TodoMVC Physical example (see `/home/martinkavik/repos/boon/playground/frontend/src/examples/todo_mvc_physical`) and work toward rendering parity with `reference/screenshots/screenshot.png` using `reference/layouts/layout.json` as positional truth.

### Regenerating the emergent screenshot
1. Build the wasm bundle: `cargo run -p tools -- wasm-build`
2. Serve the repo root (so `reference/layouts/layout.json` is reachable): `cargo run -p tools -- serve . --port 8000` (if 8000 is busy, pick another port, e.g. 8001).
3. Capture (headed only — WebGPU swapchains are blank in headless):  
   `cargo run -p tools -- screenshot --url http://localhost:<port>/web/emergent_wasm.html --output emergent/screenshot.png --width 700 --height 700`
4. Stop the server (Ctrl+C in the serve tab if running foreground).

The page reads `reference/layouts/layout.json` for element positions and uses the emergent (3D block) renderer compiled to wasm in `web/pkg/`.
