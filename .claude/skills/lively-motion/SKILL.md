---
name: lively-motion
description: Make a Loft creature, ship or attack move with life (Disney's twelve principles of animation as formulas). Use when motion is correct but looks stiff, mechanical or dead, when authoring or reviewing an attack, idle or reaction, or when asked for exaggerated, lively or characterful movement.
---

# Lively motion

Read `docs/design/motion.md` and `docs/spec/03_SUB_ENGINES_AND_STDLIB.md` for the v1.0 motion model.
`docs/legacy/animation_principles.md` preserves pose and timing recipes to port from the earlier engine.

Rules of thumb:

- Every action has three beats, and the wind-up is several times longer than the action.
- Nothing holds perfectly still, and no two parts move in step.
- Loose parts arrive late and keep going after the body stops.
- Volume is kept when a form stretches or squashes.
- After it works, push every departure from rest by 1.3 to 2 and look at it.

Render a frames sheet across the action and look at it before and after. Staging and appeal are not formulas: use the
checklist at the end of the reference.

## Grabs

For a grasp, drive all the fingers from the same grip state. Carry the held object from that contact frame and
check that it stays anchored until release. The legacy `examples/grab.ts` records the worked scene to port.
