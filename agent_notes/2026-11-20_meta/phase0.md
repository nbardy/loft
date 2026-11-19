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

## Scores (scored once; Apple M4, Metal, 1280×800)

One final `loft gate` on the milestone-2 code (`9c927d9`; the milestone-3 commit after it changes only E104's
wording) (`out/meta/final_gate.txt`, `out/meta/gate.json`): **pictures 12/12, twin 10/10, speed 8/8, load 18/18,
truth 10/10 Held.** Device creation 14 ms (outside load).

| Scene | Modules P/M | Pipelines P/M | Commands P/M | p50 ms P/M | p95 ms P/M | Cold s P/M | Warm s P/M |
|---|---|---|---|---|---|---|---|
| capsules ×1 (7 rows) | 4 / 4 | 5 / 5 | 5 / 5 | 0.30 / 0.246 | 0.40 / 0.684 | 0.12 / 0.233 | 0.03 / 0.009 |
| capsules ×20 (140 rows) | 4 / 4 | 5 / 5 | 5 / 5 | 0.40 / 0.293 | 0.55 / 0.739 | 0.27 / 0.239 | 0.18 / 0.051 |
| blobs ×1 (1 row) | 4 / 4 | 5 / 5 | 5 / 5 | 0.40 / 0.336 | 0.55 / 0.846 | 0.12 / 0.285 | 0.03 / 0.018 |
| blobs ×20 (20 rows) | 4 / 4 | 5 / 5 | 5 / 5 | 0.90 / 0.771 | 1.20 / 1.166 | 0.14 / 0.352 | 0.05 / 0.082 |

Also predicted and hit: derived L = 1.0 (capsules) and 1.36 (blobs) exactly; module text byte-identical between ×1
and ×20 for both scenes (checked by the load gate, not by count).

### Where the theory missed, and the corrected theory

- **p50 hit, p95 missed (×1.7–2.1 on three scenes).** Frame-time deciles are tight (capsules ×1: 0.225–0.33 ms over
  the 0–80% range) and every scene has a 5–10% tail of 0.7–1.7 ms frames whose size does not depend on the scene.
  The machine was shared with a running 3D game (Hearthstone, ~60% CPU) and WindowServer, so the tail is GPU
  time-slicing, not our work. Corrected theory: p95 = p50 + the contention tail on a shared machine; blobs ×20 hit
  its p95 only because its body is large next to the tail. The budgets (set before measuring at ~2× prediction)
  held in the final run, but capsules ×1 (0.8 ms budget) flapped to 0.816 once in five runs. Next: time frames
  pipelined (a ring of query sets) on a quiet GPU.
- **Cold load missed (×1.9–2.5).** Metal compiles each pipeline of a full kind module in ~70 ms serially (330–400 ms
  for 5); concurrent creation brought it to 180–270 ms. My ~6 ms/module came from 19 Nov's 5,538 tiny modules,
  which does not transfer to a module holding the row loader, the fns, and the vs/fs/shade/twin entries. Corrected
  theory: cold ≈ the slowest pipeline (~180–270 ms here) + derivation; it is O(kinds) as designed.
- **Warm load hit except blobs ×20 (0.082 vs 0.05 s).** Warm pipelines take 0.4 ms (Metal's cache); what is left is
  the CPU bound derivation, the one load cost that grows with rows: ~0.35 ms/row for capsules and ~3.8 ms/row for
  blobs (interval + Lipschitz evaluation, branch-and-bound to 1/16 m, all cores). Next: cache by (kind hash, row
  bytes), and move the evaluator off `Vec` values (5.7 µs per blob evaluation is allocation, not maths).

### Gate detail

- Pictures: 3 frames per scene (t = 0, 4, 9 s of the orbit), every frame bit-identical to its approved reference
  across runs (max channel diff 0). The tolerance is 0.1% of pixels over 2/255. References are per device (the
  plan's bit-identity contract): files in `out/meta/refs`, hashes in `engines/gates.json`; a missing file or a hash
  mismatch is `Unknown`, which fails.
- Twin (GPU `twin` entry vs the CPU evaluator, 1e-5 abs or 8 ULP): worst errors trace 9.5e-7, visible 2.4e-6,
  material 6.8e-6 over 1,536 samples.
- Truth: sampled slopes stay under the derived L (capsules 0.9999 ≤ 1.0, blobs 1.050 ≤ 1.36); the field is ≥ 0.047
  just outside every derived AABB; the closed-form `visible` lands on the `trace` surface (|f| ≤ 1e-3) on 816 hits.
- Load: 4 modules and 5 pipelines for every scene; ×1 and ×20 modules byte-identical.
- Speed: 5 GPU commands per frame for every scene (cull 1, drawIndirect 1, classify 1, shade 1, present 1).

## What was built

| Layer | Lines (total / code) | Budget | Contents |
|---|---|---|---|
| 0 runner `crates/loft-runner` | 643 / 580 | ≤ 1,000 | device, explicit layouts, concurrent pipelines, nonce patched into naga IR, RenderBundle + indirect draws and dispatches, timestamps, readback, twin dispatch |
| 1 IR + gates `crates/loft-ir`, `loft-cli/src/gates.rs` | 585 + 406 = 991 | ≤ 1,500 | spans, `Diag` with notes and fix, `Verdict`, AST and types, `Program`; the five gates |
| 2 passes `crates/loft-passes` | 3,980 | ~10k | parse 657, check 943, eval 905 (generic over the scalar), lip 342, emit 658, lib (schedule, stage 0, derive, pack, naga) 475 |
| CLI `loft-cli/src/main.rs` | 110 | | check, show, still, gate |
| 3 library `lib/` | 111 | | sdf axioms, analytic intersectors, light and sky, layout |
| engines `engines/` | 104 | | capsules, blobs, `gates.json` |
| tests | 131 | | E104 ×2, E211 ×2, E401, E402, E403, E404, baseline |

The draft's gaze bug, as the checker reports it (from the blobs scene with `to_parent` removed):

```text
error[E104]: space mismatch in transform application: expected `Pt[Blob]`, found `Pt['side]`
  --> out/meta/dbg/gaze.loft:24:27
 24 |     let eye = self.pose * Pt['side](0.0, 0.0, 0.0);
   |                           ^^^^^^^^^^^^^^^^^^^^^^^^
  --> out/meta/dbg/gaze.loft:22:9
 22 |     map side = mirror_x(q) |> translate(Vec[Blob](-0.2 * s, -0.78 * s, -0.45 * s));
   |         ---- space `'side` introduced here
   = fix: lift it out of `'side` first: `self.pose * (side.to_parent * value)` inverts the map's offset and mirror
```

Language as built: `space`, `unit` (f32 aliases; suffixes m, rad, deg, s, kg; `deg` converts), `const`/`comptime
const` (overridable: `--set COUNT=1`), `fn`/`comptime fn`, `@lipschitz(p)` / `@lipschitz(max(a, b))` axioms,
`kind K[comptime ...]` with `row` and `query visible|trace|material`, `map m = p |> mirror_x() |> translate(v) |>
rot_y(a)` with `m.cell`, `m.to_parent`, `m.from_parent`, `comptime for`, `emit`, `schedule` with passes rows (a
comptime fn), `orbit`, `build_indirect`, `render_bundle(Vis64) { draw_indirect(K); }`, `tile_bin_kinds`,
`dispatch_material_tiles(light: fn)`, `tonemap_aces(sky: fn)`.

Inherited vs ours: interval arithmetic, the mean-value (centred) form and sphere tracing with a Lipschitz step are
textbook; the Vis64 + tile classification + per-kind indirect shading shape is the draft's. Ours: L and the AABB are
derived per row and stored as hidden row columns (so the module never changes with data); one evaluator generic in
its scalar serves stage 0, the CPU twin and the derivation, so the twin compares two independent lowerings (AST
interpreter vs WGSL); the cold-load nonce is patched into the naga IR so cold and warm compile the same path.

## Left for later phases

- A window/surface: the runner draws offscreen (gates and `still`); no WASM build yet.
- Bound derivation is CPU and O(rows) (above); the evaluator allocates per vector.
- Units do not check dimensions; no smoothness lattice, `diff`, E307; no `scan`, `solver`, `law`, `constraint`;
  kind variants are checked (E401/E402) but refused at lowering (E405); maps have one mirror and three stage kinds.
- Light is one library fn per pass with self-occlusion only; no shadows across kinds, no Bsdf algebra yet.
- `tools/check.sh` still covers only the legacy `rust/` workspace; `tools/dev.sh` builds the new one into `target/`.
- References must be approved per device (`loft gate --approve` after looking at the stills here).
