# Reference screenshots

Canonical reference capture: `reference/screenshots/screenshot.png`

How to regenerate:
- Reference (populated fixture, 700x700):
  `cargo run -p tools -- screenshot --url file://$(pwd)/reference/html/todomvc_populated.html --output reference/screenshots/screenshot.png --width 700 --height 700`

- Classic (rendered via dev server, 700x700):
  1) `cargo run -p tools -- serve web --port 8000`
  2) `cargo run -p tools -- screenshot --url http://localhost:8000 --output classic/screenshots/screenshot.png --width 700 --height 700`

Notes:
- Use the HTTP server for classic so JS runs and todos render.
- File:// is fine for the static reference fixture; it contains the populated list.
