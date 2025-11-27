# Classic TodoMVC Renderer

This folder groups everything related to the current (classic) 2D WebGPU renderer:

- `docs/` – minimal summary (`classic_summary.md`).
- `screenshots/` – canonical classic screenshot (`screenshot.png`, captured via dev server).
- `layouts/` – renderer-produced layout JSONs or diffs (keep reference comparisons here).
- Code lives at `renderer/` (still at repo root); tools live in `tools/`.

Suggested workflow for new captures/layouts:
1. Serve the app: `cargo run -p tools -- serve web --port 8000`.
2. Capture a frame: `cargo run -p tools -- screenshot --url http://localhost:8000 --output classic/screenshots/screenshot.png --width 700 --height 700`.
3. Dump layout JSON: `cargo run -p tools -- extract-dom --output classic/layouts/renderer_dom_layout.json`.
4. Compare to reference: `cargo run -p tools -- compare-layouts --reference reference/layouts/layout.json --actual classic/layouts/renderer_dom_layout.json`.

Keep new artifacts in this folder so we can track the classic baseline as we move to the emergent/physical version.
