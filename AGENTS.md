# AGENTS quick start

### Rendering & screenshots
- Serve classic renderer: `cargo run -p tools -- serve web --port 8000`
- Capture classic UI (700x700):
  `cargo run -p tools -- screenshot --url http://localhost:8000 --output classic/screenshots/screenshot.png --width 700 --height 700`
- Capture reference (pre-populated fixture):
  `cargo run -p tools -- screenshot --url file://$(pwd)/reference/html/todomvc_populated.html --output reference/screenshots/screenshot.png --width 700 --height 700`
  (Use populated fixture to get todos in the shot; file:// is fine for the static reference.)

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
