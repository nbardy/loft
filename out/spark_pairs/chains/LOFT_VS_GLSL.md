# Loft vs GLSL, from this session's samples

## What each one did here

- **Loft** (`chains/*/chain.loft`, all lint exit 0): the four iteration axes
  authored as scenes. One `param step: u = 0 in 0..10` drives the axis;
  nested improvement uses the seeded keep-threshold idiom
  (`u32(sample_key(..) * 100.0) < keep`, same as the lattice struts), so
  step t+1 strictly contains step t. Geometry axes (shape, expression) are
  natural in Loft: capsule endpoints and radii are plain arithmetic on
  `f = f32(step) / 10.0`. Color axes work per-channel on `rgb` literals
  (the `palette` idiom), plus take-over rolls for discrete parts.
- **GLSL** (`lantern-color/lantern_glow.glsl`, plus the earlier lava and
  frost shaders): the same lantern axis as a 2D glow field, one uniform
  (`uWarm = 1 - t/10`). Pixel-level, no scene graph, no sampling infra.

## The contrast, honestly

- **Iteration speed**: GLSL won whenever a numpy CPU mirror sufficed
  (lava mp4s, frost PNGs rendered in seconds, no build slot, no device).
  Loft iteration without a GPU stops at lint — which still catches real
  errors (unit mismatches, bad selects) but tells you nothing about the look.
- **What Loft expresses better**: shape and expression. A smile is two
  capsule endpoints moving with `f`; moss is 40 seeded keep-rolls. In GLSL
  those become SDF boilerplate you must hand-roll per scene, with no
  `sample_key` determinism story.
- **What GLSL expresses better**: surface and light. Frost veins and lava
  crust are a few lines of fBm; in Loft they'd be material/shader work, not
  build-stage geometry.
- **Voting data**: both feed the same `pairs.jsonl` shape
  (`{id, chain, axis, kind, step_a, step_b, a, b, criteria, created_by}`).
  The PNG steps are the ballot images today; the `.loft` files are the same
  steps re-renderable on the GPU later (`tools/gpu_lock.sh ... --frames N`).
  `kind` is `adjacent` (close neighbours) or `baseline` (step 0 vs later,
  including 0v10).

## Rule of thumb going forward

Author the axis in Loft when the improvement is geometric (shape, pose,
expression, layout, coverage); mirror it in GLSL/numpy when the improvement
is photometric (glow, veins, grain, flow) so you can see it today. Pair both
against the same step indices.
