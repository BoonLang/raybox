# Emergent UI Notes (seed)

- Design intent: position semantic elements in 3D (depth + move_closer/move_further); let geometry + lighting create bevels/fillets/shadows automatically.
- Theme drives physics: lights, global geometry defaults (edge radius, bevel angle), material presets, depth/elevation scales, interaction physics (elasticity/weight), emissive states.
- Token reduction: no manual shadow/border/hover/focus tokens; keep semantic colors + material types. Text hierarchy can be Z-position based.
- Focus/loading/states: spotlight for focus, sweeping light for loading, emissive materials for error/success, ghost material for disabled.
- Migration plan: start by mirroring reference layout using emergent primitives; replace painted borders/shadows with physical depth; validate against new physical screenshots kept in `emergent/assets/`.
