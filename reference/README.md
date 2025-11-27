# Reference (HTML Ground Truth)

Organized layout (no symlinks):
- `html/` – frozen TodoMVC HTML/CSS/JS assets (app.bundle.js/css, base.js, index.html, populated fixture).
- `layouts/` – canonical layout JSONs (700px + full, precise layouts).
- `screenshots/` – canonical reference capture (`screenshot.png`).
- `docs/` – LAYOUT_ANALYSIS, REFERENCE_METADATA.

Optional: regenerate a visualization when needed (not stored):
`cargo run -p tools -- visualize-layout --input reference/layouts/layout.json --output /tmp/layout.html`
