# Lean Rust lane 2.3, solve part 2 (19 Nov 2026)

Plan: `docs/proposals/2026-11-19_lean_rust_loft.md` §1.7, §8.2 lane 2.3. Follows `2026-11-19_lean_rust_solve.md`
(feat/lr-solve, merged in here with its integrator notes: `pub mod solve;` and the sqrt-derivative floor in `prim.rs`
were already on that branch). Branch `feat/lr-solve2` from main `7f48109`. Owner: loft-core `solve.rs`, `check.rs`,
`cache.rs`, and the driver's Check/Solve support (`lib.rs`, new private `analyses.rs`). Numbers: M4, **heavily shared**
(load average 8 to 21 throughout: another lane's GPU tests and a Keel batch).

## 1. Proofs out of the build (decision already made)

- `solve::Solved.holds: Vec<Proof>`, `enum Proof { Unproved, Checked(Verdict) }`. `solve` returns curves with every
  hold `Unproved`; `solve::prove` (what `loft check` runs) replaces each with `check`'s proof on the returned f32
  curve. No printer can show `Unproved` as a pass: it carries no truth. Plan §1.7 and `docs/errors.md` say so.
- The TS solve already matched: it returns `Ranged` checks that `loft check` decides; nothing changed there.

## 2. The driver runs Check and Solve jobs (wire 2)

- `loft_core::run`: `Program::Solve` lowers each Solve decl (`analyses::solves`) and solves it, holds `Unproved`.
  `Program::Check` decides each Check decl (`analyses::checks`), then solves and proves each Solve decl.
  `Program::Evaluate` stays `Internal::Unbuilt`. `Report` gains `checks`, `solves`, and phases `solve` and `prove`;
  `src/core/native.ts` types them; the addon stamp is `abi 5` (the answer changed).
- Lowering is `kernel::Lower` (now `pub(crate)`): time is slot 0, unknown k's jet d (a binder on the wire) slot
  1 + 4k + d, then each param and input read, held at its default, then a check's ranged leaves. A multi-leaf range
  becomes nested one-leaf ranges. A hold's test and robustness margin are built as `holdsOf` builds them (bound:
  `m >= 0`, or `> 0` for gt/lt; equal: `max |m_c| <= within`, margin `within - max |m_c|`). Refused by name: a free
  variable outside a range; a table fetch, loop, raw function or gradient in a check or hold (the CPU tape cannot run
  them yet); a hold said as anything but equal/ge/le/gt/lt.
- The TS bridge `dump_solves.ts` now also writes each scene as the wire jobs `solve()` will send
  (`out/lr23/jobs/NAME.{solve,check}.job`), plus a three-check Check job. `tests/it/solve.rs` runs them through
  `loft_core::run`: the JSON graph bridge is no longer on the solve path.

| Check (`tools/dev.sh test loft-core --release -- solve driver`) | Result |
|---|---|
| heron_leap via the Solve job | solved, 4 rounds, 309 steps (TS 4, 309); curves within 3.3e-6 of scale 4.15; every hold `Unproved` |
| heron_leap via the Check job | same curves (bit-equal to the Solve job's); 26/26 verdicts as TS (24 Holds, 2 listed WithinPrecision) |
| heron_leap moved | solved, 4 rounds, 594 steps (TS 4, 402); 26/26 verdicts; curves within 4.7e-2 (see §4) |
| heron_leap_refused | Unsolved at heron_leap.ts:71, 3.734e-2 short (TS: the same hold) |
| mistake 35 | Infeasible, both holds (TS: the same two) |
| `driver_checks` (recorded Check decls) | `lifts` Holds, `early` Fails (witness t = 2), `fast` (eventually over a param) Holds |
| `point_fast_same` | the fast f64 atoms equal the interval rule at a point, bit for bit, on 1,728 operand triples per atom |

The driver gives the same rounds, steps and curves as the JSON-graph path did: the lowering is exact.

## 3. Speed

**Theory first (measured, heron base, instrumented, under load: 372 ms):** the old note blamed the interval-rule
evaluation (4.5 ns per tape op). Wrong: making the common atoms plain f64 loops saved about 20 ms. The time was:

| Where | ms | Why |
|---|---:|---|
| Rows built per call (outside the tape) | ~170 | every sample's B-spline placement (16 kernel calls per unknown) and a full leaf frame (every held slot), rebuilt at every evaluation |
| Derivative programs on every sample | ~95 | a bound's gradient and Hessian are read only where it is active; most span samples are not |
| Value rows run twice at one x | ~45 | an accepted step's line search, the next Newton step and the outer loop all evaluate the same x |
| Exchange | ~40 | golden-section searches one lane at a time, and the dense grid placed afresh every round |
| Cholesky (n = 75, 512 calls) | ~36 | dense, while the Hessian has an envelope |

Fixes, each keeping the arithmetic (the search takes the same 309 and 594 steps, curves unchanged to the bit):
per-line leaf slots (`narrow`) and cached placements; derivatives only on samples the Lagrangian reads; the last
rows reused at the same x (`memo`); lockstep golden sections; cached dense placements; envelope Cholesky; fast f64
atoms (add, sub, mul, div, neg, min, max, abs, sqrt, comparisons, select).

**Score** (`solve_ms`, least of 9, the same jobs and the same load, back to back):

| | before (52219e6's solve) | after | tape ops |
|---|---:|---:|---:|
| heron_leap | 754 ms | **117 ms** | 60.1M → 22.9M |
| heron_leap moved | 1,498 ms | **221 ms** | 104.3M → 35.4M |

6.5× on the same machine state. The feat/lr-solve note's 268 ms was taken under lighter load; scaled by the same
ratio it is about 41 ms. **The 90 ms target is met only by that scaling**: no unloaded run was possible in this
session (load average never fell below 8). Proofs (Check job only) are unchanged: 145k boxes base, 281k moved.

## 4. Conditioning: heron moved (not fixed; a named exception)

Diagnosis (DERIVATION plus measurement, state dumped at the returned x and analysed in numpy):

- Rust's own answer moves **0.25** under delta × (1 + 1e-9) (TS: 0.21). Rounds 5, steps 827 against 4, 594.
- The formulation is well-conditioned: on the tangent space of the active rows (rank 52 of 75, tangent dimension
  23) the reduced Hessian's least eigenvalue is **3.4e-4** of the largest diagonal. Not a flat valley.
- The answer is **not a KKT point**: the reduced gradient is 0.38 (|Qx| = 1.07); the linearised tangent Newton step
  is 0.85 long and lowers the jerk model from 1.289 to 1.159. The search stalls.
- Mechanism: rho climbs to 1e12. The augmented Lagrangian's quadratic model is good only within about rho^(-1/4)
  ≈ 1e-3 on a curved hold (its quartic penalty term), so Newton steps crawl along the active holds until the
  tolerances meet, at a place that turns on the last bits. Base heron stalls too (last row of the table below), but along a path whose
  bits happen to be stable (spread 3.3e-7).

Tried (two clean sidecars, then stopped per the stop rule):

| Change | Result |
|---|---|
| Cap rho at 1e6 / 1e8 | moved becomes determined (spread 1.8e-12 at 1e6) but takes 24 rounds and 6,101 steps; base heron fails (Unsolved, 1.2e-6 short) |
| Continue at a moderate rho (1e6) from the converged multipliers | base wanders 0.34 and plateaus at a 6e-6 violation: Unsolved at hold 10 |
| SQP polish: Newton on the KKT system of the active rows (null-space method), bands pinned | round 1 of base is a true KKT point (steps 3.5e-8, kept). Later rounds want a 2.2-long tangent step that breaks clearance rows by 0.13: the optimum lies across active-set changes |

**Why no formulation fixes it:** scaling, a different parameterisation or a damped step change neither the curved
active set nor the penalty's rho^(-1/4) reach, and the problem itself is already well-conditioned (3.4e-4). Reaching
the KKT point needs a different method: a globalised SQP with an inequality-constrained QP subproblem (active-set
changes, merit function), in Rust and TS alike, and it would move base heron's answer away from today's TS answer
too. That is a solver rewrite, not this lane.

Recorded as the named exception `UNDETERMINED = ["heron_leap moved"]` in `tests/it/solve.rs` (documented there): its
curves are gated on TS's own spread, its outcome and 26 verdicts on equality. The entry fails the test once TS's
spread falls inside `Tol::f32_rel(1e-4)`, so it expires by itself.

## Lines (scc code)

check.rs 1,043 + cache.rs 41 + solve.rs 955 + analyses.rs 282 = **2,321** (feat/lr-solve: 1,813; ceiling 1,400).
solve.rs grew 222 lines: the speed work (~150) and `Proof`/`prove`. analyses.rs is the driver's half of the cutover.

## Left

- The 90 ms target scored on an unloaded machine.
- The method (§4): globalised SQP, both languages, then drop `UNDETERMINED`.
- `cache::through` is not wired: `run` takes no cache directory (loft-node's arguments would change).
- Checks and holds that fetch tables (Const fills could run on the tape, as the kernel plan already does), loops and
  gradients are refused by the driver.
- A Scene job ignores its Solve decls (the TS compiler still bakes solved curves); `Evaluate` is unbuilt.

## Reproduce

`bun rust/crates/loft-core/dump_solves.ts > out/lr23/solves.json` (writes `out/lr23/jobs/*`), then
`LR23_BOXES=200000 tools/dev.sh test loft-core --release -- solve driver --nocapture`, and
`tools/dev.sh test loft-core --release --run-ignored only -- solve_ms --nocapture` for the timing.
