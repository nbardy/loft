# Render and motion integration (19 Nov 2026)

`feat/design-render` (31eaeab) and `feat/design-motion` (90f9b23) both start at 5d94531. Merged on
`integrate/design` in a separate worktree, render first and motion onto it, as both notes ask: render's ordered
passes are the larger change, and motion's tick work slots in ahead of them. Resolutions follow
`docs/proposals/2026-11-19_{render,motion}_architecture.md`.

## Seams, one thing each

| Seam | Before | Now |
|---|---|---|
| Wire 5 | Render: twelfth span plus decls 12/13 Kind/Open, `Job.wire` field never set. Motion: decl 11 RowStep found by scanning decls in `pack` | One decoder: 11 or 12 spans, tag 11 RowStep, 12/13 Kind/Open, all gated by one `typed` flag (wire 5). The recorder sets `wire: 5` when it writes row steps; `pack` writes 5 for that or for ordered declarations, 4 otherwise, so wire-4 job bytes are unchanged |
| Addon stamp | Render `abi 7 wire 5`, motion `abi 5 wire 5` | `abi 7 wire 5` on both sides |
| Frame identity | Render `previous` (digest of prior observations), motion `steps` (per-step checkpoint, prefix digest, tick, fraction) | `FrameKey` carries both. The previous-image digest hashes the frame key, which already holds the step keys, so history over a stepped scene covers its input prefix. `loft serve` returns both per frame; the batch header drops both |
| Order in a frame | Render: camera, frame kernels, old stepper loop, ordered passes. Motion: live steps, ticks, camera, kernels, legacy passes | Live steps, row steps (POSE presented), camera, frame kernels, then ordered or legacy passes. The step passes own the timestamp interval only when no camera or kernel follows (one writer per query slot) |
| Ordered scenes and steps | An ordered scene could not declare row steps; the shared-table fixture filled POSE with a graph map | Render `Scene.steps` takes `move(...).steps`; `buildOrdered` records them |
| POSE layout | `58`, `2*joint`, `50+i` written in move.ts and stage.ts | `move()` names it once: `frame(actor, joint)` and the observation rows |
| Scene classification | Render `subjectOf` gave `ordered`; motion sniffed `default.kind === "recorded"` in build, check and show | `subjectOf` classifies design, scene, ordered or recorded once; build, check and show switch on it. `moduleAt` is private again |
| Core vocabulary | Motion's observed-margin check was `Scope::Trace`/`check::trace`/`TraceSite`, colliding with render's `trace` ray query (render check 16 failed on the merge) | `Scope::Observed`, `check::observed`, `ObservedSite`, `Scene.observed`. The `loft-draw trace` subcommand keeps its name |
| GPU host counts | Two `built()` formulas | One: ordered adds its pipelines and the camera module; both add the two step pipelines per program |

**Not unified: grid draws.** Motion's stage draws read their instance tables and POSE through the graph's
`loft_packP[offset + min(i, last) * lanes + lane]`, so one crab and five crabs differ in two draw texts (offsets 549→340
and 693→1060, bounds 47→239, 37→189, POSE 57→289). Render's ordered tables already have a pack each and a
runtime count. Two fixes exist and both are larger than an integration: (a) port the stage draws to render's grid
cover with `Rows`/`Indirect` instance counts, which needs a rigid-frame placement in render's world (the origin law is
translation only); (b) read graph tables through a per-pack directory, which changes every corpus text and must be
timed. Check 17 stays a miss until one lands.

## Combined test

`motion_live::posed_observers`, run inside `recorded_take_score`: an ordered scene places a 0.5 m sphere at each of
the crab's eight distal frames, `origin = t_posed(frame(0, 3+3i)).xyz`, with the crab's live row step. Two bundles,
print (record) and inspect (independent ray), top orthographic view at 256×192. Ticks 0, 5, …, 240 (playback at
24 fps), then seek to 420, rewind to 150 and 60, forward to 240 and 480, back to 0: 55 frames per observer. After each
frame the same encoder copies POSE out, so the oracle is the snapshot the passes read.

Prediction before running: every snapshot equals the baked tick of `one.pose` (same law, input and script); zero
pixels off a ±1 mm edge band disagree with the snapshot oracle. Result: **0/110 snapshots differ from the baked tick, 0
wrong pixels, 554 edge-band pixels, 13 record/trace disagreements, all inside the band.**

## Focused checks on the combined revision (Apple M4, Metal)

All deterministic numbers equal the lane notes. Check 16 moved during the merge and was fixed above.

| Check | Lane | Combined |
|---|---|---|
| Render 1 | 6 hashes, 6 pipelines, 701 lines | Same (628 + 73) |
| Render 2, 4–10, 14, 15 | Pass | Same numbers (9/9 bead frames, 9,664/12,080/12,892, furnace 1.5/1.75/1.999956–1.999998, history 0.932%) |
| Render 11 | −2.069/−1.990/−1.830%, control −0.840/−0.841/−0.834% | Same; still a miss |
| Render 16 | 0 domain identifiers | **3 after merge** (`Trace`, `TraceSite`, `trace` from motion), **0 after the rename** |
| Render 17 | 205/193/152+33/706/185 | Same |
| Render indirect, POSE fixtures | 512/256, 256/256, 0/0; 156 px, centres 32/40 | Same |
| Motion 1–11, 13, 18, Phase 2 | As recorded | Same to the printed digit, including 9.33536 mm (10) and 1.633205/0.869567 m, 457/481 (12) |
| Motion 17 hashes | One vs five differ in the two draw modules | Same two modules differ (above) |
| Motion trace exit | 5 | 5 |

Timing moved, but the machine was not quiet: a game client and a Keel batch held the GPU and four to nine cores
(load 4–9). Render 3 cold loads 360/360/364 ms (lane 482/376/365), warm spread 10.06% (lane 7.0%); still a miss.
Render 12 shade 2.31/4.47 ms, slope 0.108 (lane 2.27/4.44, 0.108). Render 13 p50/p95 19.8/20.9 ms first run,
19.8/22.4 ms second (lane 17.8/18.2); still under 50 ms. Motion solo/256/1000 p95 0.062/0.073/0.097 ms and bake
5.56 ms on the first run (lane 0.070/0.101/0.086, 5.31); a second run under heavier load read 2–4× slower everywhere.
None of these is a measurement of the merge; rerun 3, 12–17 and motion 14–17 on a quiet machine.

## Reproduce

```sh
tools/dev.sh build loft-node --fast && tools/dev.sh build loft-draw --fast
bun tools/motion_fixtures.ts out/motion-score
env LOFT_TAKE_DIR="$PWD/out/motion-score" tools/dev.sh test loft-gpu --fast --retries 0 \
  --run-ignored ignored-only -E 'test(recorded_take_score)' --no-capture
uv run --no-project python tools/motion_score.py out/motion-score
bun tools/calib.ts; bun tools/slice_profile.ts; bun tools/slice_oracle.ts; bun tools/slice_light.ts
tools/parity/score_bead.sh; uv run --no-project --with tiktoken tools/slice_audit.py
bun tools/slice_indirect.ts; bun tools/slice_pose.ts
```

## Full gate

`tools/check.sh full` at 21623a8: static 12/12, clippy clean, nextest 40 passed and 5 skipped, doctests clean, Bun
312/314. The two Bun failures were load, not code (machine load 12–14): `tests/text.test.ts` "parses and lowers in under
20 ms" read 76 ms, and `tests/spacetime.test.ts` hit its 120 s timeout. Neither file nor the code under them changed
in this merge; rerun alone, both files passed 11/11 in 30 s. The merge also fixed clippy and rustfmt findings both lanes
had left under the gate flags (wildcard matches, `chunks_exact`, a complex return type, an `expect` after `is_some`).

Before the gate, the loft-core fixtures in `out/lr13`, `out/lr21`, `out/lr23` were regenerated with
`rust/crates/loft-core/dump_{math,checks,kernels,solves}.ts`. They do not exist in a fresh worktree.

## What blocks the corpus switch to Rust

Unchanged by this merge, and now in one place:

1. **Compact graph.** The TS engine still inlines every call (Titan 308k nodes); the switch needs it first.
2. **Corpus parity.** 233/295 build and draw, 13 pass picture parity in every frame (963c667, before the grain fix);
   the grain-fixed draw baseline and the eight wire-4 refusals are not rerun.
3. **Grid draws as data.** Above: a wire-4 draw's text changes with its table sizes, so scenes that differ only in
   counts do not share modules (motion check 17). The render Phase 2 corpus kinds (chart and triangle reconstruction, rigid placement, relief roots, fused fills)
   are the route; compaction at corpus scale is unmeasured.
4. **Open Phase 1 gates.** Render 3 (cold load), 11 (one-bounce bias with default primaries), 12 (time model), 17
   (reference and inspector tokens); motion 10 (vertical travel), 12 (second-opening magnitude), 14–17 (tails,
   bake, full-scene reuse); motion 21 is the owner's verdict.
