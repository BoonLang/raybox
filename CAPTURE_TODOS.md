# Capture & Diff TODOs (fail-fast, exact geometry)

## Blocking tasks
- Make captures fail if the expected data source is missing (no fallbacks):
  - Renderer capture must consume `raybox_report_json` and error if absent/empty.
  - Reference capture must use the JS bounding-box collector; error if it returns zero nodes.
- Ensure renderer report nodes are actually emitted (count ~45+) and include h1, toggles, footer links/buttons.
- Make `diff-layouts` operate only on these node sets (no DOMSnapshot leftovers).

## Renderer alignment fixes (use measured reference)
- Title (h1): y=43.59375, h≈19.59375, x=75, w=550; color rgb(184,63,69); font 80px, weight 200.
- Checkbox toggles: x=75; y=[205.390625, 265.1875, 324.984375, 384.78125]; size 40×40.
- Footer count: x=75, y=427, w=100, h=20.
- Filters: All/Active/Completed at x=225/295/365, y=427, w=40, h=20.
- Clear completed: x=475, y=427, w=130, h=20.
- Make text renderer use the element’s target height for h1 and small footer text (no extra padding inflation).

## Verification loop
1) `cargo run -p tools -- wasm-build --release`
2) `cargo run -p tools -- capture-reference --file reference/todomvc_populated.html`
3) `cargo run -p tools -- capture-renderer --url http://localhost:8000`
4) `cargo run -p tools -- diff-layouts --left reference/layout_precise_reference.json --right reference/layout_precise_renderer.json --threshold 0.05`
5) Check screenshot: `cargo run -p tools -- screenshot --url http://localhost:8000 --output screenshot.png --width 700 --height 700`

## Success criteria
- Diff reports zero element geometry differences.
- Visual check: title vertical spacing, checkbox padding, and footer row all match the reference PNG.
