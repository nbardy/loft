# `solve`: motion from stated constraints (18 Nov 2026)

Spec: `docs/proposals/2026-11-18_next.md` section 5 (and 7b, the "any expression >= 0" contract). Branch `loft/solve`.
Code: `src/geom/solve.ts` (exported from `src/motion.ts`, unproven marker), `bspline` in `src/curve.ts`. Demos:
`examples/second/heron_leap.ts` (design: `base`, `moved`), `examples/second/heron_leap_refused.ts`. Tests:
`tests/solve.test.ts`, fixture `tests/fixtures/mistakes/35_solve_conflict.ts`.

## What a user writes

```ts
const leap = solve({
  over: [0, 1.8],
  vary: { x: spline(15, roughX), y: spline(15, roughY), pitch: spline(15) },   // n numbers each; `from` = start
  hold: (c) => [
    equal(vec2(c.fx, c.fy), [0, 0], { from: 0, to: 0.6 }),        // planted
    equal(accel, [0, -9.81], { from: 0.6, to: 1.2 }),             // flies (accel = derivative(derivative(c.y, time), time))
    equal(vec2(c.fx, c.fy), [3.2, 0.7], { from: 1.2, to: 1.8 }),  // lands on the deck at 1.2
    atLeast(0.97 * SPAN, reach),                                  // within reach (whole span when `when` is left out)
    atLeast(apart(rail, knee, foot), 0.1),                        // clear of the rail
  ],
  prefer: { jerk: 1 },                                             // or acceleration, or near: { to: { y: old }, weight }
});
leap.curves.y      // bspline(time, [0, 1.8], [15 numbers]): an ordinary expression of time
leap.holds         // the same holds over the solved curves: margins of `time` alone, for checks
leap.report        // ms, steps, rounds, samples, and each hold's worst miss on the returned curves
```

Names added: `solve`, `spline`, `equal`, `atLeast`, `bspline`, `SolveError` (and types). Why these and not the
proposal's `planted`/`lands`/`clear`/`reaches`/`during`/`always`: those four are one thing (a margin at some times);
`equal` and `atLeast` with `{ at }`, `{ from, to }` or nothing say all four, plus any hold the model invents.
`during` is already an export (`expression.ts`), and `always` is the ranged-checks branch's combinator, so time is a
plain object here. The check vocabulary (`staysClear`, `landsInside`, `stopsAt`) takes attacks and skeletons, not
expressions, so reusing a name would have meant two signatures for one name. A `HoldLine` is `{ kind, margin, times,
site }`: the margin is the expression >= 0 (or = 0) that the ranged checks need, `times` is where it applies.

## The method, and why

Inherited: B-splines, augmented Lagrangian (Powell-Hestenes-Rockafellar; Nocedal and Wright 17.3), Levenberg-
Marquardt damping, the exchange method for semi-infinite constraints, the deletion filter for an irreducible
infeasible subset (Chinneck). Mine: the jets (below), and taking second derivatives symbolically through them.

- **Family: uniform cubic B-spline, n numbers per unknown.** Linear in its numbers, so jerk and acceleration
  preferences are exact quadratics (Gauss-Legendre, 4 points per knot gap, exact); C2, so a jerk preference is defined
  and a flight's acceleration is continuous; local (4 numbers reach any time); and its value lies in the hull of those 4
  numbers, which is the bound an interval evaluator wants. Returned as `bspline(time, over, numbers)`: a model reads 15
  numbers per curve and an edit moves the curve near one time by at most 2/3 of the edit. Not interpolating: the
  numbers are near the curve, not on it (`keys`/`through` are for curves through values).
- **Holds are any expression**, compiled to the JS twin with their gradient and Hessian. The trick that makes this
  affordable: while solving, each unknown is a library call `JET(time, v0, v1, v2, v3)` whose value is v0 and whose
  time derivative (a registered derivative rule) is `JET(time, v1, v2, v3, 0)`. The four jet variables are set, at
  each sample time, to the B-spline's value and three derivatives there; a hold's derivatives in the jets go to the
  numbers through the kernel weights. So the symbolic work is per jet a hold reads (5 positions for a clearance), not
  per number (75): the heron's holds with derivatives in all 75 numbers did not finish in 2 minutes; through jets (Gauss-Newton
  steps then) it took 9.8 s, and with second derivatives in the jets the inner steps are Newton steps.
  `derivative(c.y, time)` in a hold works through the rule. A hold that reads an unknown at another time (a delay) is
  caught at build (jets and real B-splines evaluated at made-up numbers must agree) and refused.
- **Solver: augmented Lagrangian**, Newton inner steps with exact symbolic Hessians and LM damping (raised until the
  damped Hessian is positive definite and the step descends). Chosen over SQP with a QP: interval holds sampled more
  finely than the family's freedom give redundant, dependent rows, which an active-set QP needs removed and the
  Lagrangian does not mind. Gauss-Newton (no constraint curvature) converged linearly where a curved hold carried a
  large multiplier: 5878 steps in the first round; Newton: 195.
- **Instants are exact; intervals are honest.** An instant is held at its time: worst miss on the returned (f32)
  curves 1e-13 to 1e-15 in the demos. An interval is held at one sample per knot gap, then checked on a grid 64 times
  finer, each local worst refined between grid points by golden section; a broken stretch is sampled every h/16 round
  its worst time and the solve runs again from where it was (up to 24 rounds). Nothing between grid points is proven;
  a ranged check can prove it from `leap.holds`. Before the golden-section refinement, a test found a dip 9.5e-5 under
  `atLeast(y, 0)` between grid points (f'' spacing^2 / 8). `TOLERANCE` is 1e-5 in the hold's units, measured on the
  returned expressions, or the solve is `unsolved`.
- **Failure is typed** (`SolveError extends SceneError`, exit 4). `infeasible`: a least-squares search for any curve
  meeting the holds, from where the solve stopped, fails; then a deletion filter over hold lines leaves a minimal set,
  each named by file and line (`siteOf` moved to `src/errors.ts` so constructors can record their caller).
  `unsolved`: a curve meeting them exists, but the solve did not settle (or interval misses remain after 24 rounds).
  `unreadable`: a hold the CPU cannot evaluate or differentiate, or a delay. Searches are local: "infeasible" means
  none was found near where the search stopped, exact only for holds linear in the unknowns.
- **Deterministic**: no randomness; the probe uses fixed made-up numbers.

## Rigid parts between keys

Loft has no keyed-pose interpolator that blends a rigid part's points: rigid parts are placed by frames (`rigid`,
`poseFrame`, `turnedAbout`, `blendFrames`) and chains (`chain`, segment lengths exact in reach), all of which keep
shape. `blendFrames` re-orthonormalises its mixed axes, so a part keeps its shape (its turn is not at an even rate,
as documented). What does deform is mixing points (`mix`, `keys` on positions, `through`, an attack's spine mix, which
is a flexible body by design): a 2 m rod whose end is mixed between two keys 30 degrees apart is 2 cos 15 = 1.932 at the
midpoint. Test (`tests/solve.test.ts`, first test): the rod blended by `blendFrames` and the rod turned by a solved
hinge angle each measure 2 within 1e-4 at the midpoint (measured: exact to f64 rounding). So: solve angles and targets,
not the points of a rigid part; the heron's body is `poseFrame` of solved (x, y, pitch), its legs `chain` to solved feet.

Found on the way and fixed in `src/curve.ts`: the B-spline kernel written with `abs` had a second derivative of 0 at a
knot (`grad.ts` differentiates abs as sign, and sign(0) = 0), so a hold on acceleration read exactly at a knot was wrong.
It is now `select(x < 0, -x, x)`, whose derivative at the tie is the right one-sided value; the solver's probe caught it.

## Demos (predictions were written in the scene headers before the first run)

Heron of `examples/second/heron.ts` (refactored into `examples/second/lib/heron.ts`; heron.ts renders the identical
picture, rgba be04cd8c741721b6 before and after). Unknowns: body x, y, pitch and both feet (fx, fy), spline(15) over
[0, 1.8] (a number per 0.15 s, so takeoff 0.6 and landing 1.2 are knots). 26 holds (15 lines). Solve on an M-series
CPU (shared machine), wall time inside `solve`:

| | base (deck at 3.2, top 0.7) | moved (deck at 4.0, top 1.0) |
|---|---|---|
| solve time | 5.0 s (816 steps, 16 rounds, 426 samples) | 5.6 s (968 steps, 19 rounds, 423 samples) |
| worst hold miss on returned curves | 8.1e-7 | 5.3e-7 |
| crouch (predicted 0.4 to 0.6 m below 1.9, near 0.3 s) | 0.68 m, at 0.375 s | 0.80 m, at 0.38 s |
| takeoff vx (predicted 2.3 to 3) | 2.66 m/s | 3.39 m/s (+0.73; predicted +1.3) |
| takeoff vy (predicted 3.4 to 3.8) | 3.88 m/s (missed, high) | 4.13 m/s (+0.25; predicted +0.5) |
| apex hip (predicted 2.5 to 2.8 near 0.95 s) | 2.61 m at 1.00 s | 2.61 m at 1.02 s |
| landing dip (predicted, qualitative) | to 2.07 at 1.41 s (0.53 below standing on the deck) | to 2.19 at 1.39 s |

Scored: the crouch nobody keyed appears, as predicted, but deeper (0.68 m, not 0.4 to 0.6); vertical takeoff speed
missed high; vx and apex in range; the feet tuck behind and pass over the rail (at 0.9 s the feet are at (1.42, 1.12),
just above the 1.0 m rail), as predicted. The moved deck's changes were half the predicted size: the solver moved the
takeoff point forward (body x at 0.6: 0.53 -> 0.64) and landed with the body farther back instead of only flying faster.

Files: `leap_base.mp4`, `leap_moved.mp4` (side view, 2.4 s, SVT-AV1 crf 42), `leap_base.webp`, `leap_moved.webp`
(frames at 0, 0.375, 0.6, 0.9, 1.2, 1.8 s, WebP q95), `leap_base_game.webp` (three-quarter view).

**Infeasible** (`heron_leap_refused.ts`, the rail raised): `refused.txt` is the whole output, exit 4 in 1.3 s:

```
loft: scene: solve: given holds that cannot all be met: equal at t = 0 (examples/second/heron_leap.ts:71); atLeast over
the whole span (examples/second/heron_leap.ts:83) (the nearest found misses by 0.0373); ... (at examples/second/heron_leap.ts:95)
```

Scored: wrong twice. Predicted for a 3.6 m rail: refused, naming the flight, both plantings, the reach and a clearance.
At 3.6 m it solved: the bird passed under the rail ("clear" says nothing of the side). At 3.2 m the minimal conflict is
two lines, standing at t = 0 and the bill's clearance: the standing heron's bill tip is at 3.18 m, inside the rail's
pad. Nothing about the flight was needed. The guide's example was refused twice while being written, both correctly
(a C2 curve cannot switch from free fall to standing still at one instant; and too few knots to push between "still
until 0.4" and "flying from 0.6"), each naming the right lines.

No still of the refused request: a refused scene does not draw.

## Not done, and limits measured

- Speed. 5 s for the heron is mostly exchange rounds (each restarts the Lagrangian on more samples) and evaluating
  compiled Hessians of the IK chain at ~400 samples. A deck 9 m away (feasible: nothing limits push acceleration) took
  83 s. No work budget yet: a slow solve is slow, not refused. Next levers: warm-start rounds with fewer outer
  iterations; a budget that ends `unsolved` naming it.
- Local search. A hold has no side: "clear" was met under the rail at 3.6 m. `spline(n, from)` picks the side; the
  heron needs a rough arc over the rail as its start (without it the feet go under the rail and the legs through it,
  reported as infeasible from that start).
- Holds read the unknowns at their own time only; params and inputs take their defaults (no `forAll` over a param).
- The deletion filter costs one least-squares solve per hold line; fine at 15 lines.
- `spline` is the only family; `bspline` is not `keys` (not a table row: an edit recompiles).
- `loft context` shows `solve` through the generated API map (marked `~`, one user so far). DESIGN.md not updated.
