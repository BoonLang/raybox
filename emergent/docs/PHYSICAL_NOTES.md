# Emergent UI Notes

## Target
- Goal: render TodoMVC with the emergent physical technique so it visually matches our 700×700 baseline (`reference/screenshots/screenshot.png`) while using the ground-truth layout (`reference/layouts/layout.json`).
- Source reference implementation/docs: `/home/martinkavik/repos/boon/playground/frontend/src/examples/todo_mvc_physical` (see `docs/PHYSICALLY_BASED_RENDERING.md`, `EMERGENT_GEOMETRY_CONCEPT.md`, `EMERGENT_THEME_TOKENS.md`, `PATTERNS_STATUS.md`, `RUN.bn`, `Theme/`).

## Core principles (from Boon docs)
- Position in 3D (depth + move_closer/move_further); geometry/lighting produce bevels/fillets/shadows automatically.
- Theme drives physics: lights, global geometry defaults (edge radius, bevel angle), material presets, depth/elevation scales, interaction physics (elasticity/weight), emissive states.
- Token reduction: no manual shadows/borders/hover/focus; keep semantic colors/material types; text hierarchy via Z where possible.
- States: spotlight for focus, sweeping light for loading, emissive for error/success, ghost for disabled.

## Plan (M1: emergent render)
1) Extract requirements from Boon example:
   - Map TodoMVC elements in `RUN.bn` to emergent properties (depth, move_closer/further, rim, material).
   - Capture theme defaults from `Theme/` (lights, geometry, materials, corners, interaction).
2) Build emergent renderer path (in this project):
   - Load `reference/layouts/layout.json` as positional truth.
   - Apply emergent theme/tokens to produce physical styling (even approximate geometry is OK for M1).
3) Validate:
   - Screenshot to `emergent/assets/screenshot.png`.
   - Visual compare against `reference/screenshots/screenshot.png`.
4) Document mapping:
   - Element → depth/transform/material decisions.
   - Theme preset used (start with Professional equivalent).
5) Iterate toward parity; after M1, integrate Boon runtime when ready.

## Handy commands
- Reference screenshot: `cargo run -p tools -- screenshot --url file://$(pwd)/reference/html/todomvc_populated.html --output reference/screenshots/screenshot.png --width 700 --height 700`
- Classic screenshot: `cargo run -p tools -- serve web --port 8000` then `cargo run -p tools -- screenshot --url http://localhost:8000 --output classic/screenshots/screenshot.png --width 700 --height 700`
- Layout regen: `cargo run -p tools -- extract-dom --output reference/layouts/layout.json`
