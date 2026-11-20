## Your lane: `draw.volume` — raymarch any density field (worktree `~/git/_wt/loft-volume`)

Today volumes are Gaussian blobs integrated in closed form (`src/soft/`, `draw.density`). Missing: an arbitrary
density the author writes (noise smoke, folded fractal fog, dust with fine structure) marched with scattering.
- A drawable taking a density expression of (position, time) and a bound (and colour/albedo/emission as expressions),
  marched front to back with a step count and early exit at opacity, single scattering from the sun and lamps with a
  shadow march (or a cheaper named approximation), Henyey–Greenstein phase (g as a parameter), ending at the opaque
  scene depth, composited with the soft medium and other transparents correctly or with a stated limitation.
- Jittered steps (blue noise or hashed) to hide banding; deterministic per frame.
- Styles must still apply (a toon or woodblock look of a volume: give the style the scattered light terms, as for
  surfaces, if it fits the existing style context).
- Example: `examples/volume_dust.ts` (a noise-driven dust cloud lit from behind, plus a folded fractal fog variant).
  Report ms/frame at 1280x720 for 64 and 128 steps with and without shadow march.
