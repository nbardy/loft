# Lean Rust lane 2.3, solve part (19 Nov 2026)

Plan: `docs/proposals/2026-11-19_lean_rust_loft.md` §1.7, §3.4, §8.2 lane 2.3 (the `solve.rs` that
`2026-11-19_lean_rust_lr23.md` left). Branch `feat/lr-solve` from main `d9d208a`. Source material: `src/geom/solve.ts`
(solve2, `cc2ca61`) and `feat/rust-core`'s `rust/loft-core/src/solve.rs` (numeric kernel only).

## What

- `rust/crates/loft-core/src/solve.rs`: `solve(&Solve) -> Result<Solved, Error>`. It takes the holds as an `Ssa` over
  leaf slots, with unknown k's d-th time derivative at slot `jets + 4k + d`, plus the span, families, preference and
  delta. Derivatives come from `grad`, applied twice in Rust. A small `compact` pass (dead code, CSE, x*1, x+0) runs
  between the passes. Heron's largest hold has 184 value instructions and 5,288 for the Hessian, down from 17,063 raw.
  It returns the f32 controls and one `check::Verdict` per hold, the same type `check` returns.
  `curve(ssa, t, over, controls)` is `src/curve.ts` `bspline`: the per-span f32 polynomial the shader draws.
- **Never false.** Each verdict is a `check` proof (`Strategy::Prove`) over the hold's window, run on the returned
  curve: the holds' program with every jet leaf replaced by `grad`ⁿ of `curve` over the f32 controls, at f32 times,
  with f32 frame defaults. That is solve2's fix (verify the returned f32 coefficients, not the f64 search state), now
  true by construction. If `check` cannot decide a hold, the verdict is `Undecided(Budget | WithinPrecision |
  Codomain)`. A proved break is `Error::Scene { Unsolved }`, naming the hold, the margin and the time.
- Errors: certified conflict → `SceneFault::Infeasible { a, b }`; a stopped search → `Unsolved { fix }` at the
  worst hold's site; a non-finite margin, gradient or Hessian → `Unreadable`; a budget → `Error::Budget { TapeOps }`.
  The budget counts work (instructions × lanes), not wall time, so it is deterministic.
- **New, DERIVATION:** the search evaluates holds in f64 through each atom's interval rule at a point box. The rule
  encloses the real operation, so at a point it is that operation, and no second op table is needed. A rule with no
  point answer gives NaN (a divisor at 0, `rem`, `atan2` on its cut). That NaN is an error only where a root reads it.
- Sums and products follow `solve.ts`'s order: the env sums then divides by h^d; `powf`; eq rows before ge rows; the
  gradient seeded with -b; rank-one terms, then curvature; Cholesky by sequential subtraction. With sums reordered,
  heron moved took 6 rounds and 684 steps. In TS order it takes 4 rounds and 594 steps, with the same samples as TS.
- Bridge: `rust/crates/loft-core/dump_solves.ts` (out: `out/lr23/solves.json`) and `tests/it/solve.rs`. The graph walk
  moved out of `dump_checks.ts` into `dump_graph.ts`, now shared. `dump_checks.ts` output is unchanged except TS
  timings. `solve`'s jet `JET(time, v_d, ...)` walks as its leaf v_d.

## Pass / fail (`tools/dev.sh test loft-core --release -- solve_same analyses --nocapture`, M4, shared load)

Corpus scenes with holds: heron_leap (base, moved), heron_leap_refused, tests/fixtures/mistakes/35_solve_conflict.

| Scene | TS | Rust | Match |
|---|---|---|---|
| heron_leap | solved, 4 rounds, 309 steps | solved, 4 rounds, 309 steps; curves within 3.3e-6 of scale 4.15 | yes |
| heron_leap moved | solved, 4 rounds, 402 steps | solved, 4 rounds, 594 steps, same sample sets; curves differ by up to 4.7e-2 | **outcome only** (see below) |
| heron_leap_refused | unsolved (search), hold 1 (heron_leap.ts:71), off 0.0373 | Unsolved at heron_leap.ts:71, 3.734e-2 short | yes |
| mistake 35 | infeasible, holds 1 and 2 | Infeasible, both holds | yes |

- **Verdicts:** 52/52 on both heron variants: 49 Holds, plus 3 `Undecided(WithinPrecision)`. The 3 are plan §2.2(b)'s
  listed exception, where the TS margin is within tolerance:
  - base hold 8 (reach), TS margin 5.9e-6;
  - base hold 25 (leg-rail clearance), TS margin 4.1e-6;
  - moved hold 26 (foot-rail clearance), TS margin 3.9e-6.

  They are equalities pinned at half-band and contacts at delta, and f32-padded enclosures over 900-instruction rig
  programs cannot resolve 5e-6. **No hold is passed unproved.** Proved margins agree with TS's to about 1e-6
  (0.861257 vs 0.861259, 1.1849511 vs 1.184952).
- **Moved is not determined by the method.** `dump_solves.ts` re-solves each TS scene with delta × (1 + 1e-9).
  Heron base moves 3.3e-7; heron moved moves **0.21 m** in TS itself. The Rust–TS difference (4.7e-2) is inside that
  spread. At rho = 1e12 the Newton path turns on the last bits, and the stall point along the flat jerk directions
  differs. The test gates such a scene on the reference's own spread, which is wider than `f32_rel(1e-4)`, and prints
  it. **Not met:** curves within 1e-4 on moved. This needs a better-conditioned method (for example a final solve of
  the KKT system on the active set), not a closer port.
- Proof budget: TS proves heron at 20,000 boxes per hold (affine forms). Rust `check` needs up to about 32,000 on the
  rig clearances, so the test uses 200,000. At 20,000, base gives 22/26, with four `Undecided(Budget)`.
- `analyses` 25/25 still passes after the `prim.rs` change; claw -0.3499924 both. `dev.sh lint loft-core` and
  `cargo fmt --check` are clean. `tsc --noEmit` is clean. `bun test tests/solve.test.ts` was **not run**: the machine's
  loft bun-test lock was held by another suite.

## Cost

- Solve phase, without proofs: heron 268 ms (TS 1.0–1.5 s cold), moved 770 ms (TS 1.2–1.8 s). Plan target ≤ 90 ms
  **not met**. 60M tape ops at about 4.5 ns each through the interval rules. A direct f64 match per atom would cost
  about 45 lines.
- Proofs: 145k boxes, about 4 s (base); 281k, about 9 s (moved). That is `check`'s cost on deep rig programs, not
  solve's.
- **Lines (scc code): check.rs 1,039 + cache.rs 41 + solve.rs 733 = 1,813. Ceiling 1,400: does not fit.** The
  numeric kernel alone was 774 lines on feat/rust-core. The port adds hold compilation, derivatives, preference,
  start, curve and verification, and needs about 700 lines. Fitting in 320 would mean dropping the conflict
  certificate, `near`, golden-section peaks or the exchange neighbourhood. Each of those changes which scenes match.

## Outside solve.rs (for the integrator)

- `rust/crates/loft-core/src/lib.rs`: one line, `pub mod solve;`.
- `rust/crates/loft-core/src/prim.rs`, sqrt's derivative: it is now `da / (2 max(sqrt a, 1e-30))`, floored as
  `src/grad.ts` floors it. Unfloored, `chain`'s fully stretched leg (a clamped cosine) gave 0.5/0 × 0 = NaN, and heron
  failed as Unreadable at hold 24.
  For lane 1.3's owner, on the centred form: the floor keeps D finite where sqrt's argument touches 0. That is sound
  for squared-distance arguments, whose true slope is bounded. A bare `sqrt(t)` is unsound only below about 1e-60.
  `asin`/`acos` are not floored yet; TS floors them.
- `tests/it/main.rs`: `mod solve;`. `tests/it/analyses.rs`: `graph` is `pub(super)`.
- `src/geom/solve.ts`: `JET` and `holdsOf` are exported for the bridge. `examples/second/heron_leap.ts`: `leapSpec(n)`
  is split out of `leap` with no behaviour change; the bridge and the TS solve give the same 4 rounds and 309 steps.
- Fixture: `bun rust/crates/loft-core/dump_solves.ts > out/lr23/solves.json`.

## Left

- Moved determinacy (above); solve phase ≤ 90 ms; the line ceiling.
- `check` budgets on rig programs (lane 2.3 check.rs). Affine-form correlation or a tighter centred form on select
  trees would bring heron back to TS's 20,000.
- The wire has no Solve decl layout (§3.2). The driver lowers a recorded Solve to `Solve` and wraps it in
  `cache::through`.
