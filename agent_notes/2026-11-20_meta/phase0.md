# Meta engine Phase 0: the spine

Plan: `docs/proposals/2026-11-20_meta_engine_plan.md` §5 Phase 0. Language: `agent_notes/dump_manual_nov_19th_alpha_v1.md`.
Branch `meta/phase0` from main `ceb41d2`.

## What Phase 0 builds (as I understand the request)

A new workspace beside the old tree (`crates/`, `lib/`, `engines/`) with no dependency on the old TS or Rust engine:
a parser and checker for a WGSL superset subset (spaces, `kind` + `row` + `query visible|trace`, generative `map`
spaces with `.to_parent`, unit literals, `schedule`), staging errors E401/E402, space error E104, derived Lipschitz
with E211; lowering of each kind to one naga module, rows to storage tables, one cull dispatch plus one
`drawIndirect` per kind inside a RenderBundle, a Vis64 visibility buffer and a tile-classified shading pass; a wgpu
runner under 1,000 lines; two scenes (analytic capsules, a traced SDF) at 1 and 20 instances; the five gates live.

## Predictions (written before any code, scored once at the end)

The frame, for a scene with K kinds (both scenes have K = 1):

| Pass | GPU commands | Pipelines | Module |
|---|---|---|---|
| cull: test every row's derived AABB against the frustum, compact visible rows, write one `DrawIndirectArgs` per kind | 1 dispatch | 1 | `cull` (shared) |
| vis: RenderBundle, per kind `setPipeline` + one `drawIndirect` of a 36-vertex box proxy; the fragment intersects (analytic) or sphere-traces (SDF) and writes Vis64 = (kind:8 \| row:24, t:f32) and depth | K draws | K | one per kind |
| classify: 8×8 tiles, one bit per kind present, append tile to each kind's list, bump its indirect dispatch | 1 dispatch | 1 | `classify` (shared) |
| shade: one `dispatchWorkgroupsIndirect` per kind over its tiles: normal from the field, material, light | K dispatches | K | the kind's module |
| present: sky where Vis64 is empty, ACES, sRGB | 1 draw | 1 | `present` (shared) |

Instances: analytic scene = 7 capsule rows per critter (7 and 140 rows); SDF scene = 1 blob row per instance (1 and 20).

| Scene | Modules | Pipelines | GPU commands / frame | GPU p50 / p95 (ms) | Cold / warm load (s) |
|---|---|---|---|---|---|
| capsules ×1 | 4 | 5 | 5 | 0.30 / 0.40 | 0.12 / 0.03 |
| capsules ×20 | 4 | 5 | 5 | 0.40 / 0.55 | 0.27 / 0.18 |
| blobs ×1 | 4 | 5 | 5 | 0.40 / 0.55 | 0.12 / 0.03 |
| blobs ×20 | 4 | 5 | 5 | 0.90 / 1.20 | 0.14 / 0.05 |

Reasoning:
- Modules and pipelines are O(kinds) by construction: nothing in a kind's module depends on its row count, the
  kind's id, or its row base (those come from a per-kind uniform), so ×1 and ×20 emit byte-identical WGSL. The
  load gate checks this by hash, not by count alone.
- Frame time: four full-screen-ish passes at 1.02 Mpx (classify reads 8 B/px, present reads 24 B/px and writes 4)
  are about 0.05–0.1 ms each on the M4; pass boundaries with timestamps cost tens of µs. Capsules are closed-form
  (a few dozen flops per covered fragment), so 20 critters add little. Blobs sphere-trace a field with derived
  L ≈ 1.36 (unit sphere and capsule smin, plus a 0.02 m ripple at 9 rad/m: 0.02·(9+9) = 0.36) for ~30–60 steps
  over proxy boxes covering ~10% (×1) to ~35% (×20) of the screen, misses marching through to the box exit.
- Load = parse, check, comptime rows, derived bounds and L per row (interval branch-and-bound on the CPU), WGSL
  emit, naga validate, then Metal pipeline compile for 5 pipelines (the 19 Nov measurement, 32.9 s for 5,538
  modules, is ~6 ms per module cold). Cold uses a nonce in a live comparison so Metal's cache misses; warm is the
  same program loaded again in the process. Bound derivation is per row and on the CPU, so it is the one load cost
  that grows with instances: ~1 ms per row predicted, hence capsules ×20 (140 rows) is the slowest load.
- Device creation is excluded from load (the device may belong to a host); it is reported separately.

## Scores

(filled in once, at the end)
