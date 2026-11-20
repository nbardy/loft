## Your lane: PBR ambient occlusion and a moving sun (worktree `~/git/_wt/loft-pbrsky`)

Read `src/style/pbr.ts`, `src/style/pbr_parts.ts`, `agent_notes/2026-11-18_pbr_style/NOTE.md`,
`agent_notes/2026-11-18_fix_pbr.md`.
- Ambient occlusion: PBR has none, so creases and contact look flat. Add it as a swappable part (an `occlusion` slot)
  with a good default: for distance-field bodies, a few-tap field AO along the normal; for others (meshes, copies) a
  screen-space AO post pass or a named fallback. It multiplies environment light only, not direct sun. Measure cost.
- Moving sun: a sun that is an input (or reads time) is currently projected at its default by the sky projection.
  Make the environment follow it (per-frame projection into a small table, or an analytic update), or refuse by name
  where it cannot. Measure the per-frame cost.
- Example scenes and GPU tests of measured properties (AO darkens a crease by a stated amount; environment colour
  follows a moving sun).
