# motion-nfeet-2: four sites cost nothing, the crab's cost, the raised legs

Branch `motion-nfeet-2` from `motion-nfeet` (7cbbd27). Adapter Apple M4 (Metal), 1280x800, 300 frames,
`loft profile --json` under `tools/gpu_lock.sh`. Evidence in `~/git/loft/out/motion-nfeet-2/`
(`q_*.json` are the profiles below; `variants/` holds the experiment sources).

Timing contention: other lanes held the GPU lock and the load average ran from 6.5 (11:42, this batch)
up to 109 (earlier runs). Every comparison below is paired and interleaved (three rounds a/b/c,
variants back to back in each round). Read differences inside a round, not across rounds.

## 1. Zero cost at four sites

**Measured cause.** The motion-nfeet cost is real and larger than reported once paired (its "main"
was unpaired). In this batch, crossing round a/b/c: main 6.75 / 7.06 / 6.66 ms p50, motion-nfeet
8.13 / 8.51 / 8.88 ms. Vis rose 3.7 to 4.6-5.1 ms, Shadow 1.66 to 2.0-2.2 ms. The march inlines
`q_trace` five times (24-step loop plus four copies in the 42-step loop). Each copy carried the
runtime `i < sites` guards and the gated `4..8` block. Source experiments with the old binary
(`variants/`, `y_*`) put most of it on the gated block and the guard.

**Mechanism.** The spec already had the construct: `kind K[n: u in lo..hi]` is a discrete
parameter, part of kind identity (01_LANGUAGE §2.4). It parsed and checked, but no path lowered it
(`schedule.rs` returned E405). It is now lowered for one variant per kind:

- The parser also appends each parameter as a column. `emit` supplies it like any field, and CPU
  evaluation, tables, twins and live edits read it from the row unchanged.
- `lower_passes::fixed_parameters` collects the value every row of the kind was emitted with.
  Rows that disagree get E402 (a later lane can lift this to the spec's four used variants). A
  value outside `lo..hi` (upper bound excluded) gets E602.
- `Emitter.fixed` folds the parameter into a WGSL literal in `load_row` and at every member read,
  in the kind module and the motion `advance` module. A live edit (tier0) passes none and reads
  the row.
- A law cannot write a parameter (E501).
- The domain budget now follows the spec, cardinality ≤ 8 (it was 4), so `u in 1..9` is legal.
  Parameters are validated before `emit`s are checked, so a bad parameter is refused as a
  parameter and not as a missing column. The E402 fixtures move from 3 bools (8) to 4 bools (16).
- Grammar: `var_param: LIDENT ":" type_expr ("in" expr ".." expr)?` in `loft.lark` and the spec
  excerpt.

What the author writes: `pub kind Walker[sites: u in 1..9] @World (...)`, and the trace is one
loop, `for i in 0..sites`. Scenes write nothing new. `loft show` prints `loft_local_i < 4u` for
crossing and `< 8u` for crab_live. The gait's `i < w.sites` guards fold to `i < 4u` too.

Known gap: a host that `upsert_rows` a different parameter value is not refused. The GPU modules
keep the emitted value. The spec says so, and the runner has no per-column metadata to check it.

**Prediction.** The four-site trace becomes main's code (four unrolled legs, no guard). Cost equals
main within noise, and stills are byte-identical.

**Measured.**

| scene | main p50/p95 | motion-nfeet | motion-nfeet-2 |
| --- | --- | --- | --- |
| crossing a | 6.75 / 7.34 (Vis 3.68) | 8.13 / 9.28 (Vis 4.59) | 6.52 / 7.53 (Vis 3.54) |
| crossing b | 7.06 / 8.53 (Vis 3.88) | 8.51 / 10.05 (Vis 4.84) | 7.51 / 8.86 (Vis 4.15) |
| crossing c | 6.66 / 7.57 (Vis 3.72) | 8.88 / 9.78 (Vis 5.07) | 7.05 / 8.20 (Vis 3.88) |
| tidal a/b/c | 4.67 / 4.62 / 4.05 | 4.48 / 6.30 / 4.49 | 4.49 / 4.88 / 4.15 |
| tidal_fork a/b/c | 5.06 / 5.45 / 4.04 | 4.74 / 5.55 / 4.85 | 4.30 / 4.32 / 4.22 |

Crossing is back to main within the round noise (−0.2 to +0.45 ms, with round b the noisiest).
tidal and tidal_fork match main. Verdict: held.

- Stills: 21 of 21 byte-identical to main, by RGBA hash. Crossing, tidal and tidal_fork at
  t = 0, 0.5, 1, 1.5, 2, 2.5 and 3 s. Main was rendered with the motion-nfeet binary on an
  exported 34bd927 tree; that commit changed no Rust.
- Twin: `loft-verify trace-check --record ... --t 3` then `--replay` matches on crossing (f7c0a478),
  tidal (056113b7), tidal_fork (8d96dd40) and crab_live (4283d381).
- `tools/check.sh static`: 331/331 grammar files accepted, pins equal to Keel.
- Tests (`tools/dev.sh test`): loft-parse 5/5, loft-passes 77/77, loft-tier0 5/5, loft-cli 50/50
  (1 skipped). loft-check and loft-emit have no tests.

## 2. Crab Live cost

**Bound.** Stage 0 derives one AABB for the whole walker, about 1.44 × 0.87 × 1.75 m, at the
emitted pose, with L = 1. All eight legs are traced in one loose box: no per-leg bound, no per-leg
proxy. The march runs up to 24 + 4×42 steps, and each step evaluates all 8 legs, IK included. The
shadow map is 1024² fit to the walker alone, so the crab fills most of it, and it redraws every
frame because the rows move.

**Breakdown** (old binary, source variants, round a under load 100+, so compare within the row):
motion-nfeet-style unrolled 8 legs: Vis 4.16, Shadow 7.13. With IK removed (knee fixed, wrong
look): 2.67 / 5.08. With no legs: 0.52 / 1.20. The legs are about 85% of trace cost.

**Changes tried, with predictions:**

| change | predicted | measured (a / b / c p50; Vis, Shadow) | verdict |
| --- | --- | --- | --- |
| fold `sites` = 8 (this branch) | ±5% | 11.60 / 15.21 / 15.09 vs motion-nfeet 10.77 / 15.09 / 15.32 | no change |
| knees in `present` as columns, 8-site path only (`variants/c9_*`) | −35% legs | 12.02 / 13.99 / 14.24 (−8% in b, c; +12% in a) | rejected: about 5% for 24 columns and a second code path |
| shadow map 768² instead of 1024² | Shadow × 0.56 | Shadow 4.84 / 4.84 / 5.08 vs 5.85 / 7.44 / 7.49 | holds; it is a picture change, so left to the owner |

The IK result refutes the IK-dominates theory. Removing the IK in c6 also shortened the legs, so
the gain came from fewer march steps, not from fewer ALU instructions.

**Verdict: the target (under 5 ms p95) is not reachable in the existing engine without a look
change.** In the quiet round the crab is 10.8 ms. Even 768² shadows and a perfect IK hoist give
about 7 ms. The next lever is per-leg culling: skip a leg when
½(|p − hip| + |p − tip| − (upper + lower)) − r ≥ field. That bound is exact, because a skipped leg
can never win the `min`. Today the trace refuses it with E701 (a point-dependent branch). A
compiler change could accept a branch whose skipped side provably does not lower a `min`, or
derive an interval from the union of both branches. Per-leg proxies would also work: each
`LegChain` row could carry its own trace and bound. Both are engine work, beyond this lane.

## 3. The raised, vertical legs at 1.5 s

Probe checks (`variants/probe/crab_probe.loft`) give every toe at 1.5 s. Leg 3 is swinging
(toe_y 7 mm). All seven others have toe_y = 0, and `planted` holds. The swing order is
0, 1, 2, 3, 4, 5 at 0.6, 0.9, 1.2, 1.5, 1.9 and 2.3 s.

Cause: **swing order plus perspective, not reach, IK or touchdown.** Projecting the solved knees
onto the camera puts legs 0 and 1 at the two "vertical sticks" (screen x ≈ 690 and 770). Those legs
are on the z− side, planted behind their homes because the trunk has moved 0.37 m. Their upper
segments point back and away from the camera (knee − hip ≈ (−0.43, 0.17, −0.35)), so they
foreshorten into vertical lines. Their lower segments hide behind them. The side view
(`crab_side_1.5.webp`) shows all four near-side feet on the ground.

The underlying limit is the single swing. All unstepped pins tie on strain (shift 0, same capture),
and a tie keeps the lower ID, so the sweep runs strictly 0 → 7 at about 0.3 s per step. A
2.4 s cycle at 0.31 m/s lets the last legs trail about 0.75 m. A second inherited quirk: in
`choose_foot`, `clear` (no neighbour of the active foot) is always true. `choose_foot` runs only
when `foot < 0`, so the test is dead code (main has it too). No fix here; both are design, not bugs.

What the multi-swing lane (THEORY.md §3, `stride.loft`) needs for eight legs:

- a set of aloft feet: a bit mask, with per-site clock and span columns;
- leader/lag per side, sorted back to front, so the eight-leg wave alternates sides;
- a support test with several feet lifted, which becomes the capture test;
- a crab-specific pole: lateral, from the socket direction, instead of 0.28·Δ + up, so knees rise
  sideways rather than backward.

The tie-break order matters until then. A visit order computed per plan (identity at four sites)
would keep four-site byte identity, but THEORY replaces `choose_foot` outright.

## Outputs

`~/git/loft/out/motion-nfeet-2/`:

- `crab_live.mp4` (120 frames at 30 fps, SVT-AV1 preset 10, CRF 42)
- `crab_live_1.5.webp` and `crab_side_1.5.webp` (WebP q95)
- `stills_vs_main/` (the 21 lossless PNGs)
- `*.trace` and `q_*.json`
- `variants/` (the experiment sources and the round-a profiles)
