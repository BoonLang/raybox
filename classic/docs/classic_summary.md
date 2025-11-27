# Classic Renderer Summary

- Purpose: current 2D renderer baseline while emergent version is developed.
- Key artifacts:
  - Screenshot (canonical): `classic/screenshots/screenshot.png` (captured via dev server).
  - Layout outputs: place renderer-generated layout/diff files in `classic/layouts/`.
- How to capture:
  1) `cargo run -p tools -- serve web --port 8000`
  2) `cargo run -p tools -- screenshot --url http://localhost:8000 --output classic/screenshots/screenshot.png --width 700 --height 700`
- How to compare layout: `cargo run -p tools -- compare-layouts --reference reference/layouts/layout.json --actual classic/layouts/<your_layout>.json`
