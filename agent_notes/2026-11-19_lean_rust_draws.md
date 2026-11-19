# Lean Rust: the library records draws; the first strict parity run with costs (19 Nov 2026)

Plan: `docs/proposals/2026-11-19_lean_rust_loft.md` §2.2, lane 3.1 (TS half). Follows
`agent_notes/2026-11-19_lean_rust_draw.md` ("Left" 1) and `agent_notes/2026-11-19_lean_rust_wire3.md` ("For
feat/lr-draw"). Branch `feat/lr-draws`. M4. The machine crashed mid-lane; this note was finished after the reboot.

## Brief (before the edits)

- **Blocker.** 232 corpus bundles built and loaded, 0 drawn: the TS library recorded no Draw, Pass or Camera decls.
  Nine R0 builds were refused for things the wire or emit could not say (author compute 2, offscreen passes 3,
  `t_<name>` 1, `loft_cast` 2, `soft_box` 1).
- **Target.** Every library draw records its decls; lower those refusals; one strict full parity run against R0
  recording per scene built, loaded, drawn, parity pass, GPU frame p50/p95 at 960×540, load time, shader modules and
  pipelines. Not in scope: inlining or graph shape (the next lane).
- **Stop rule.** Two failed fixes in one subsystem. **Claim boundary:** counts and times on this machine.

## What

1. **Wire 4** (`9c9b9f1`): cameras, the sky Screen pass, one view pass in the TS phase order; lit surfaces as Grid
   covers (place, normal and cells roles, the style's shade as paint), the hull outline as the pushed grid with front
   faces culled, the sea's glazed cases, ground shadows as patch grids, a plane as the pass's traced ground. loft-gpu
   draws 4× multisampled with a nearest-sample depth resolve, as R0 was drawn, and gives grids their level of detail.
   A kind not written yet records a named `unported` answer that the build prints, never a silent gap.
2. **More draws** (`01f14a2`): field, traced, relief and telegraph draws; a style's post pass as a Screen draw in a
   pass of its own; `t_<name>` accessors; `loft-draw still --shot` draws an entry's frames from one load; `profile`
   reports modules and pipelines; the runner records the costs per entry.
3. **Post-crash fixes** (`3c99d1a`):
   - `postPass` refused *every* style post pass: POST_COMMON carries `sty_water`/`sty_submerged`, which read
     `sea_height`, so film and paper grain never drew. Helpers that read a style function and that the pass never
     reaches are now left out (road: 175,344 pixels over the picture rule → 40; crab seed train-9__2: 151,634 → 1,065).
   - The prelude's scene colour is named `loft_post_colour`, TS POST_WGSL's name (refocus's blur loads it directly).
   - A field's paint sees `fieldPixel` as `pixel_world` at the hit (coral, island_tide had refused a free variable;
     that refusal now names the variable).
   - emit's per-raw Naga parse declares the accessors of the tables the closure reads (spray_burst's veil).

Not done: author compute (2), offscreen passes and table cameras (3), `loft_cast` (2), `soft_box` (1). Each needs
textures readable by draws (group 1 has storage buffers only) and, for compute, a scheduled raw kernel per pass. These
are still refused by name.

## Counts (strict full run, `bun tools/parity/run.ts rust`, head `963c667`, clean tree, `out/parity/rust/rust.json`)

| of 295 | 963c667 (full run) | 3c99d1a (fixed entries rerun filtered) |
|---|---:|---:|
| built | 229 (283 outcomes as R0; 12 failed) | **233** (8 failed) |
| loaded (one load per entry draws its frames) | 229 | 233 |
| drawn at R0's frames | 229 | 233 |
| every frame passing §2.2a | **13** | not rerun |

Frames: 243 of 1,510 pass. Failed: 84 not built (44 after the fixes), 1,183 picture rule.

## Parity failure classes

Outcome failures (entries R0 built that the Rust path refuses), 8 after the fixes:

| Class | Entries |
|---|---:|
| offscreen pass / table camera not lowered (heron_lamp, pool, reeds_shadow) | 3 |
| author compute not lowered (compute_frame, compute_volume) | 2 |
| `loft_cast`, a TS draw module's WGSL (area_lamp, mirror) | 2 |
| `soft_box`, a TS draw module's WGSL (volume_dust) | 1 |

Picture-rule frames (1,183 at 963c667), by what the entry records (unported lists from a build of all 233 at 3c99d1a):

| Class | Frames |
|---|---:|
| Every drawable recorded, gross (≥ 10× the over-pixel limit). Measured causes: the grain post passes (fixed in 3c99d1a, not rerun in full); TS's inner-silhouette ink on ray-found solids is not drawn (bead); ink width differs at small features (owl) | 526 |
| Every drawable recorded, near miss (over ≤ limit; fails only on off-edge pixels or a cluster > 4): a few pixels 10–20 codes off in soft gradients (a claw's shadow on the sea) | 310 |
| Every drawable recorded, mid (< 10× the limit) | 48 |
| biolume's `reflection` post pass reads `sea_height` (25 entries), papercraft studio (1) | 138 |
| unported drawables (16 entries): quads (glow 4, rain 3, splashes 2, points 2, lightning 2), Gaussian media 4, copies' hit role per kind 4, glazed surfaces 5, impostors 2, march medium 1, moved mesh 1, draw.shader 1 | 161 |

## Costs (the full run's `loft-draw profile`, 960×540, R0's first camera, t = 1.37 s, 16 frames)

| | GPU p50 / p95 ms | load ms | modules | render / compute pipelines |
|---|---:|---:|---:|---:|
| Owl | 2.69 / 6.01 | 307 | 91 | 19 / 73 |
| Titan (reefback) | 6.79 / 7.20 | 35,054 | 990 | 116 / 889 |
| heron | 1.69 / 8.08 | 539 | 159 | 27 / 133 |
| five-crab lineup | 97.16 / 109.14 | 23,074 | 5,538 | 653 / 4,905 |

Across the 229 measured entries: median p95 6.1 ms, max 109 ms (the lineup); median load 1.27 s, max 73 s
(scenes/maelstrom, 952 modules). Load time and module count follow fill kernels (one module per kernel) and draws
(one pipeline each): CORE.md's "a handful of modules for five crabs" is the next lane's graph-shape work, not touched.

## Next

1. One full parity run at 3c99d1a or later, to score the grain fix across the corpus.
2. Textures readable by draws (group 1), then author compute and offscreen passes; `loft_cast` and `soft_box` with the
   cast and volume draws' hit roles.
3. Inner-silhouette ink on ray-found solids; quads; Gaussian media; biolume's reflection without `sea_height`.
4. Graph shape: shared fill modules per kind (lineup 5,538 modules, 23 s load).
