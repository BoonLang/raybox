# Discrepancies Between Captured HTML Layout and Rendered Output

## Root Causes We Observed
- **Font metrics mismatch**: Canvas2D/WebGPU text rendering uses different ascent/descent/AA than the DOM/CSS box metrics. This shifted titles, labels, footer text, and forced manual Y/height overrides.
- **Baseline vs. top-left placement**: We positioned text using element top/height; the DOM lays out by baseline + line-height. This inflated text boxes and miscentered labels/footers.
- **Box-model parsing gaps**: The loader doesn’t fully honor `box-sizing`, padding, borders. Content-box vs. border-box confusion caused width/height offsets; we compensated with overrides (body/section/footer).
- **Checkbox reimplementation**: Custom circles/checkmarks (manual radii/paths) didn’t match the native/CSS visuals—sizes, stroke widths, and inset padding drifted, requiring per-pixel tweaks.
- **DPR/viewport rounding**: Even at DPR=1, `SetDeviceMetricsOverride` + canvas sizing can introduce 1–2px offsets from rounding; body/section y/heights were off until forced.
- **Shadow/centering effects**: Shadows and auto-centering of the white panel weren’t accounted for identically, shifting the body/section/flex centering relative to the reference CSS.
- **Style capture vs. render mismatch**: Reference capture used DOM/CSS; renderer reused parsed JSON but reinterpreted styles (e.g., defaults for line-height, font-weight) differently.

## Mitigations (going forward)
- **Capture true font metrics**: Use a JS probe in Chrome stable to read ascent/descent/lineHeight per element (`FontMetrics` or measureText with actualBoundingBoxAscent/Descent) and store them in layout JSON. Render text using these metrics (baseline placement), not element height.
- **Align box-model semantics**: Parse `box-sizing`, padding, borders from the captured styles; compute content and border boxes exactly as the DOM; remove `apply_reference_overrides` once parity is achieved.
- **Parameterize primitives**: Drive checkbox circle radius, stroke, and path coordinates from a normalized 40×40 spec derived from the reference capture (percent-of-size), so scale/inset changes propagate consistently.
- **DPR/viewport lock**: After `SetDeviceMetricsOverride`, set canvas width/height to the same CSS pixel size; add a 1px grid sanity probe to catch rounding drift.
- **Baseline-aware placement**: Place text at `y = element.baseline_y - ascent`, using captured ascent/descent; center vertically only when the DOM does (e.g., for controls with `line-height` equal to box height).
- **Shadow handling**: Either include shadows in captured boxes or subtract them consistently; avoid mixing visual shadows with layout boxes.
- **CI guards**: Add pixel-mask checks for key anchors (title bbox, checkbox stroke bbox, footer text bbox) vs. reference with ±1px tolerance to catch regressions early.

## Browser Choice
- **Chrome stable as truth** for geometry/metrics. Use Chrome Canary only if a specific WebGPU/CDP feature is required for capture robustness, but keep captures/metrics pinned to stable to avoid font/AA drift.

## What to remove once fixed
- Hardcoded `apply_reference_overrides` for body/section/footer/title/toggles/filters.
- Manual text height/Y overrides for h1, labels, footer texts.
- Ad‑hoc checkbox insets once parameterized from spec.
