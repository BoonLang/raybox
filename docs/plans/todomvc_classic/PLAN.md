# TodoMVC Classic2D Theme Plan

## Goal
Create a new `Classic2D` theme for demo 8 (`TodoMVC 3D`) that visually matches the existing 2D TodoMVC example as closely as possible from the default test view, and make it the default theme for easier verification.

## Constraints
- Do not change demo 7.
- Do not commit partial work.
- Keep the implementation geometric/analytic only: no textures, glyph atlases for runtime rendering, or static meshes.
- Preserve the existing non-Classic themes unless a small compatibility change is required for the new theme plumbing.
- For window resizing, prefer CSS-like responsive behavior over hard letterboxing:
  - keep canonical TodoMVC font sizes/layout metrics,
  - never non-uniformly stretch,
  - only scale down when the window is too small,
  - avoid scaling up beyond the reference/web size.

## Implementation Outline
1. Add `Classic2D` to the demo-8 theme enum, theme-cycle order, control parsing, and CLI/help text.
2. Keep `Classic2D` light-only and force it as the default startup theme.
3. Add any shared/demo-8-only TodoMVC layout helpers needed so Classic2D can reuse the canonical TodoMVC geometry and text anchors without touching demo 7.
4. Extend the demo-8 GPU setup with whatever extra buffers/uniforms the Classic2D branch needs, especially UI primitive data for borders, circles, separators, checkmarks, and card shadows.
5. Implement a dedicated Classic2D render branch in `shaders/sdf_todomvc_3d.slang` that prioritizes exact 2D fidelity and performance over physical 3D shading.
6. Make Classic2D responsive in a CSS-like way:
   - use window scale factor / logical pixels for sizing,
   - keep font sizes near the real web/reference size,
   - downscale uniformly on smaller windows,
   - remove the dark letterbox and fill the full page background.
7. Fix the CLI screenshot output-path behavior so repeated verification captures can be written outside the repo root.
8. Verify repeatedly with control-mode screenshots, comparing demo 8 Classic2D against the 2D TodoMVC look.

## Acceptance Criteria
- Demo 8 starts in `Classic2D`.
- `raybox-ctl theme classic2d` works.
- The default Classic2D view closely matches the 2D TodoMVC example in:
  - heading placement and color
  - card size, spacing, and stacked-paper effect
  - focused input border
  - separator placement
  - checkbox size and checked-state styling
  - completed-item strike-through
  - footer spacing and selected-filter outline
  - text colors and overall light-theme balance
- Classic2D uses responsive/downscale-only layout behavior:
  - no non-uniform stretching,
  - no forced upscaling past the reference/web size,
  - no dark letterbox around the page.
- Demo 7 remains unchanged.

## Verification Loop
- Build `demos` and `raybox-ctl` with `windowed,control`.
- Run demo 8 with control mode.
- Capture screenshots after each major visual iteration.
- Compare against demo 7 / the existing 2D TodoMVC look.
- Recheck both large-window and smaller-window framing to confirm downscale-only behavior.
