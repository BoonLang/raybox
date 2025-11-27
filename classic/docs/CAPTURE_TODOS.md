# Capture & Diff TODOs (fail-fast, exact geometry)

## Blocking tasks
- Make captures fail if the expected data source is missing (no fallbacks):
  - Renderer capture must consume `raybox_report_json` and error if absent/empty.
  - Reference capture must use the JS bounding-box collector; error if it returns zero nodes.
- Ensure renderer report nodes are actually emitted (count ~45+) and include h1, toggles, footer links/buttons.
- Make `diff-layouts` operate only on these node sets (no DOMSnapshot leftovers).

## Renderer alignment fixes (use measured reference)
- Title (h1): x=75, y=43.59375, w=550, h=19.59375; color rgb(184,63,69); font 80px, weight 200.
- Checkbox toggles: x=75; y=[205.390625, 265.1875, 324.984375, 384.78125]; size 40×40.
- Footer count: x=90, y=445.1875, w=72.53125, h=19.59375.
- Filters: All/Active/Completed at (x,w) = (250.78125, 32.671875) / (293.625, 56.859375) / (360.65625, 88.546875); y=442.1875, h=25.
- Clear completed: x=500.78125, y=445.1875, w=109.21875, h=19.0.
- Make text renderer use the element’s target height for h1 and small footer text (no extra padding inflation).

## Verification loop
1) `cargo run -p tools -- wasm-build --release`
2) `cargo run -p tools -- capture-reference --file reference/html/todomvc_populated.html`
3) `cargo run -p tools -- capture-renderer --url http://localhost:8000`
4) `cargo run -p tools -- diff-layouts --left reference/layouts/layout_precise_reference.json --right reference/layouts/layout_precise_renderer.json --threshold 0.05`
5) Check screenshot: `cargo run -p tools -- screenshot --url http://localhost:8000 --output classic/screenshots/screenshot.png --width 700 --height 700`

## Success criteria
- Diff reports zero element geometry differences.
- Visual check: title vertical spacing, checkbox padding, and footer row all match the reference PNG.
