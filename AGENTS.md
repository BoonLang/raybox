# AGENTS quick start

### Rendering & screenshots
- Serve classic renderer: `cargo run -p tools -- serve . --port 8000`
- Capture classic UI (700x700):
  `cargo run -p tools -- screenshot --url http://localhost:8000/web/index.html --output renderers/classic/screenshots/screenshot.png --width 700 --height 700`
- Capture reference (pre-populated fixture):
  `cargo run -p tools -- screenshot --url file://$(pwd)/reference/html/todomvc_populated.html --output reference/screenshots/screenshot.png --width 700 --height 700`
  (Use populated fixture to get todos in the shot; file:// is fine for the static reference.)
- Emergent (3D blocks) capture:
  1) `cargo run -p tools -- wasm-build`
  2) `cargo run -p tools -- serve . --port 8000` (if busy, bump the port, e.g. 8001)
  3) Emergent capture *must be headed* (WebGPU swapchains are blank in headless):
     `cargo run -p tools -- screenshot --url http://localhost:<port>/web/emergent_wasm.html --output renderers/emergent/screenshots/screenshot.png --width 700 --height 700`  
     The tool uses a real window and `page.screenshot`; if that ever fails it will fall back to `canvas.toDataURL`, which may be blank on some Chrome builds.  
  (serve repo root so `/reference/layouts/layout.json` resolves; stop server with Ctrl+C. Do NOT use headless for emergent—WebGPU won’t render.)
  Compare to reference: `cargo run -p tools -- pixel-diff --reference reference/screenshots/screenshot.png --current renderers/emergent/screenshots/screenshot.png --threshold 0.97`

### Layout & diff tools
- Compare layouts: `cargo run -p tools -- compare-layouts --reference reference/layouts/layout.json --actual <your_layout.json>`
- Pixel diff: `cargo run -p tools -- pixel-diff --reference reference/screenshots/todomvc_reference_700.png --current <img.png> --threshold 0.99`
- Visualize layout: `cargo run -p tools -- visualize-layout --input reference/layouts/layout.json --output /tmp/layout.html`

### Reference structure (no symlinks)
- HTML assets: `reference/html/`
- Layout JSONs: `reference/layouts/`
- Screenshots: `reference/screenshots/`
- Docs: `reference/docs/`
- Visuals: `reference/visuals/`

### Tips
- Don’t use file:// for classic screenshots; serve over HTTP so JS runs and todos render.
- Chrome CDP may log benign deserialize errors; screenshots still succeed.
- All tooling is Rust: `cargo run -p tools -- ...`.
- **Version control safety:** Do NOT run destructive git commands (reset --hard, clean, checkout --) or rewrite history. This repo prefers `jj` (Jujutsu); the same rule applies—no destructive `jj` operations—unless explicitly told to do so by the user.
