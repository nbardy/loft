# Auncel fixes: theory, predictions, results

Lane `lane/fix-auncel`, from local main `6ea8a122` merged with `origin/main` (`6244f9da`). Findings are those of
[NOTES.md](NOTES.md) §4. This file was written in two passes: the theory and the predictions (this section and
each finding's "Theory" and "Prediction") before any code was built; the "Result" lines after the run.

Reproductions are in [repro/](repro/). "Before" is the authors' pinned binary `loft-c3660843-auncel` run on this
worktree's stdlib (`repro/before_pinned.txt`, `repro/before_scenes_pinned.txt`); "after" is this lane's
`target/lab/loft` (`repro/after.txt`).

## What is inherited and what is new

Inherited and kept: the evaluator's design (one tree walker, generic in its scalar; an interval and a Lipschitz
bound carried beside every value; a branch the domain cannot decide is an error, never a guess), the centred
form in `derive`, and the rule list of spec §2.3. New here:

1. **A condition has three answers over a region, not two.** `Val::B(Option<bool>)` could say "decided" or
   "undecided". Undecided conflated two different things: a condition that differs between points of the region
   (a break in space, no finite L), and one that is the same at every point but unknown at compile time because it
   reads `time` (each frame takes one arm everywhere). The second is not a discontinuity in space. `Truth` names
   the three: `Known(b)`, `PerFrame`, `PerPoint`. A comparison is `PerFrame` exactly when it is undecided and
   both operands have L = 0 in the traced point. That one bit is the whole of F2.
2. **`atan2` gets an honest rule, and the honest rule says no to pleats round an axis.** See F1.
3. **Squares are squares.** `x * x` of one read, `dot(v, v)` and an integer `pow` no longer let interval
   arithmetic treat the two factors as independent. See F11 (the lead's diagnosis, that `min` loses the lower
   bound, was wrong).

## F1. Inverse trigonometry in a trace

**Before.** `atan2` (and `acos`, `fma`, `log`) pass the checker and reach `Eval::builtin`, whose catch-all arm
is `unreachable!("checked builtin {}")`: every derive thread panics. `asin`, `atan` and `tan` are not reachable at
all: the catalog marks them `Poly(Unchecked)`, so the checker answers E011 "unknown function" (so F1's "also
asin, atan" and F9c are one defect: three WGSL builtins the language never admitted). The f32 domain (stage 0 and
the CPU twin) shares the same arm, so `build { emit K(a: atan2(1.0, 2.0)) }` also panics.

**Theory.** Each rule gives a range and a bound on |f'| over the argument's interval; the chain rule multiplies by
the argument's L.

| f | range over [lo, hi] | sup \|f'\| | where it is unbounded |
|---|---|---|---|
| `atan` | [atan lo, atan hi] (increasing) | 1 / (1 + mig²) | never |
| `asin`, `acos` | monotone images of [lo, hi] | 1 / sqrt(1 − mag²) | at ±1; outside [−1, 1] the value is NaN |
| `tan` | [tan lo, tan hi] inside one branch (kπ − π/2, kπ + π/2) | 1 + max(tan² lo, tan² hi) | across a pole |
| `log` | [ln lo, ln hi] for lo > 0 | 1 / lo | at 0; NaN below |
| `fma(a, b, c)` | a·b + c through the existing rules | | |

`mig` and `mag` are the smallest and largest |x| over the interval. Unbounded means range (−∞, ∞) and L = ∞ when
the argument moves with the point (L = 0 when it does not: a per-frame value); E211 then names the function.

`atan2(y, x)`: the gradient is (x, −y)/r², of norm 1/r. With x, y of Lipschitz bounds lx, ly,
|x dy − y dx| ≤ r · sqrt(dx² + dy²) (Cauchy–Schwarz) ≤ r · hypot(lx, ly) |dp|, so
**L(atan2) ≤ hypot(lx, ly) / r_min**, where r_min = hypot(mig x, mig y) is the distance from the (x, y) box to
the origin. The range: a box that avoids the origin and the cut {y = 0, x < 0} lies within one continuous branch
and subtends less than π, so its angle extremes are at its corners. A box that contains the origin or meets the
cut gets [−π, π] and, if x or y moves with the point, L = ∞: across the cut the value jumps by 2π, so no finite
bound exists for `atan2` itself. The rule is exact about where it fails; it is loose by √2 inside (hypot(1, 1)
for `atan2(p.z, p.x)` against the true 1), because the two coordinates are bounded separately (inherited
slack; the provenance forms in `lip.rs` could remove it later).

**What authors want, and why no rule for `atan2` can give it.** Authors want cos(n·θ), which is continuous across
the cut. But a pleat of constant depth a, `a · cos(n θ)`, has gradient a·n·|sin(nθ)|/r: **unbounded at the axis
whatever the primitive.** A cone round the y axis reaches r = 0 at its tip, so its field with constant-depth
pleats is not Lipschitz, and no derivation can prove a march step there. The amplitude must vanish at the axis
(a ∝ r gives |∇| ≤ k·sqrt(1 + n²)) or the pleats belong in shading. Even with a ∝ r, the product rule bounds
r·cos(nθ) by r_max · n / r_min = ∞, so a rule that only made `atan2` periodic would still fail. The primitive that
can work is a **polar fold**: `repeat_polar(n)`, a map stage that rotates the point into one sector, after which
the pleat is written in sector coordinates. It is an isometry inside each sector; across a sector wall it is
continuous exactly when the folded field is mirror symmetric about the wall (spec §2.3: "automatic cell-wall
clamp unless wall symmetry is proven"). That is a map stage across the checker, the evaluator, the emitter and
AD: more than a small addition, so it is **pending** (specified below), not half built here.

**What authors write today for angular patterns.**
- Pleats finer than a few pixels (the Auncel's 471): in `material`, where `atan2` costs nothing and needs no bound.
  All four authors ended there for this reason.
- A few lobes on a body that surrounds the axis: write cos(kθ), sin(kθ) from the components, which are
  polynomials in u = x/r, v = z/r (cos 2θ = u² − v², sin 2θ = 2uv, and so on), multiplied by an amplitude that
  vanishes at the axis. No `atan2`, no cut.
- A shape away from the axis, or an angle of something that is not the point (a heading from `time`, a pose):
  `atan2` now works and derives a finite bound.
- The polynomial `atan2` stand-ins the authors wrote (`ang2`, `angle_of`) fold quadrants with `select` on the
  point, so in a trace they derive L = ∞ just as the real `atan2` across the cut does. They are not a workaround.

**Change.** `Un` gains `Acos`, `Asin`, `Atan`, `Tan`, `Log`; `Scalar` gains `atan2`; `fma` is a·b + c through the
existing arithmetic. The catalog admits `asin`, `atan`, `tan` as componentwise builtins (motion stays
`Unavailable`, so a law that uses them still gets E504; the kernel unit rules learn them). The three calls the
checker accepts but no evaluation can run on the CPU (`held`, `missed`, `unknown`, `sample_prior`) now give E405
instead of a panic; `exp2`, `log2`, `round` stay unchecked (E011), so their arm stays unreachable by construction.

**Predictions** (derived L printed by `check`):
- `f1_atan2` (the lead's form, `0.0 * atan2(p.z, p.x)`): passes, **L = 1.0000** (0 · unbounded is 0 by `ml`).
- `f1_acos` (acos and asin of a clamp to ±0.5, atan of p.x, each × 0.01): **L ≈ 1.0331** = 1 + 2·0.01/sqrt(0.75) + 0.01.
- `f1_heading` (atan2 of sin and cos of time, through `rot_y`): passes; the angle is [−π, π] with L = 0, the
  rotation stays an isometry, and the field's own bound (an anisotropic division) is **L = 2.0000**.
- `f1_pleat` (`0.01 * cos(12 * atan2(q.z, q.x))` on a unit sphere): **E211**, with a note at `atan2` naming the
  cut and the axis.
- `f1_quadrant` (`0.05 * sin(6 * atan2(...))` on a sphere centred 3 m out on x): passes, **L ≈ 1.22**
  (1 + 0.05·6·√2/1.95). The true slope is about 1.01.
- `f9c_tan` (`0.01 * tan(clamp(p.y, -1, 1))`): passes, **L ≈ 1.0343** (1 + 0.01·(1 + tan² 1)).

## F2. A branch on time inside a trace

**Before.** `select(a, b, time > 3.0)` derives L = ∞ (E211), an `if` statement on time is E701 "depends on the
point", and an `if` expression with statements in its arms is E211. `time` is (−∞, ∞) in the derivation, so the
comparison is undecided, and undecided was treated as varying with the point.

**Theory.** If a condition c does not depend on the point (L(c) = 0: it reads only time, rows and params), then at
any frame the whole region takes one arm, so the field over the region is f_a or f_b and
L ≤ max(L(f_a), L(f_b)); the range is within the join of the arms' ranges. This is the rule the spec already gives
for `min`/`max`, for a different reason. `Truth::PerFrame` carries it:
- `select`, `if` expressions: join both arms (ranges joined, L the larger).
- `if` statements: run both arms from the same environment; where both fall through, every variable becomes the
  join of its two values; where both return, the result is the join; where one returns and the other falls
  through, the returned value is held as a pending result of the enclosing call (or expression arm) and the rest
  of the body continues from the fall-through environment; the call's result is the join of all of them. This
  is a sound enumeration of the paths a frame can take, at twice the cost of one arm per branch, never
  exponential (environments are joined, not forked).
- Integers, transforms and rows cannot be joined (the evaluator has no interval for them): a per-frame branch
  that gives them different values is a typed E701 that says so.
- `PerPoint` keeps its old handling: `select` gives L = ∞ (E211), `if` statements give E701.

**One more finding while reproducing: a point-dependent `if` expression is accepted with a finite L.**
`let d = if p.x > 0.0 { length(coords(p)) - 1.0 } else { 5.0 }` derives **L = 1.0000** on the pinned binary
(`repro/f2_point_letif.loft`): the arm join takes the larger L with no account of the jump at x = 0, where the
field steps from 5 to −1. A sphere tracer on the x < 0 side steps 5 m. This is unsound. With `Truth` the case is
`PerPoint`, and it now gets L = ∞ like `select` (E211). Prediction: boss, heron and crab do not use it in a
trace (the inline `if` expressions in them and in stdlib are all in laws and checks), so their L does not move.

In the `unproved` proxy search the point is evaluated with L = 0 (natural intervals only), so every undecided
comparison there is `PerFrame` and both arms are joined. That is sound there because that search uses only
ranges, and a select's range is within the union of its arms' ranges wherever the condition is. Worker D's "an
`if` on time is undecidable over an interval, E701" inside `unproved` therefore goes away too.

**Predictions.** `f2_select`, `f2_if`, `f2_ifexpr`: pass with **L = 1.0000**. `f2_point` (select on p.x): E211 as
before. `f2_point_ifexpr` (an `if` statement on p.x): E701 as before. `f2_point_letif`: **E211** (was L = 1).

## F4. A slow scene gives no warning

**Theory.** The bound derivation is the cost an author pays once per geometry edit (it is cached on disk by the
kind's source and the row's shape bytes). `check` reports evaluations, but one evaluation of a long trace costs
a thousand times one of a sphere, so evaluations do not predict seconds. Wall time per kind does, and the
derivation already runs once per kind, so it is one `Instant` around it. Threshold: **10 s per kind**. The
shipped scenes derive each kind in well under that cold (the whole of `check` on boss, heron, crab takes 3.2 to
4.5 s with the pinned binary, including a GPU check), and every slow case the four authors measured was above
it: 2 to 9 min before restructuring, 7 to 125 s after (only the best of those, 7 s, falls under). 10 s is also
about the longest an author waits on an inner-loop `check` before switching to something else. The warning names
the causes the authors found, so it buys something: the reader learns what to change, not only that it is slow.
No new flag: `render --max-build-ms` already exists for a hard budget.

**Prediction.** `check` lines gain `derive N.NN s`; boss, heron and crab print no warning, cold or cached.

## F9. Three reports from worker A

(a) **`--set` and a sibling module's `param` — reproduced.** The linker renames a non-entry module's declarations
to `loft_m_<hex path>_<name>`; `--set NAME` compares against declaration names after renaming, so it finds only
an entry-file (or stock) declaration. Before: `--set size=2` on a param declared only in the sibling is E011 "no
scene parameter"; with a second `param size` in the entry, it sets the entry's copy and silently leaves the
sibling's (`repro/f9a/dup.loft`: L 2.0 where both copies set would give 4.0). **Fix:** resolve each `--set NAME`
through the modules' link tables (authored name to declaration key) before compiling. One parameter: set it. Two
or more distinct parameters with that authored name: E011 naming each declaration, with the fix "declare it once
and import it". `--live NAME` in `render` and `clip` had the same lookup by key and goes through the same
resolver. Predictions: `f9a/main.loft --set size=2` gives **L = 2.0000**; `f9a/dup.loft --set size=2` gives
**E011** naming `shot.loft` and `dup.loft`.

(b) **`const N = 36` as a loop bound — reproduced** (`repro/f9b_const_loop.loft`, WGSL validation error at
`loft_local_i < loft_const_N`). The checker types `0..N` as u32 (two abstract integers default to u32) and records
u32 on the use of `N`, but the emitter declares an untyped integer const as `const loft_const_N: i32` and emits the
use as the bare name. **Fix:** where a const's use was typed differently from its declaration (both numeric
scalars), emit the conversion, `u32(loft_const_N)`. Only uses that were invalid WGSL change text. Prediction:
passes, **L = 1.1500** (1 + 0.01·(0 + 1 + … + 5)).

(c) **`tan` is not a builtin — reproduced** (E011). Fixed with F1.

## F11. E211 on `1 / (1 + 8·min(x, 0.35))`

**Reproduced, with a different cause.** `repro/f11_min.loft` tries four non-negative x: `length(q)`, `q.x * q.x`,
`dot(q, q)` and a row field in 0..1. The first E211 is at `q.x * q.x`. `min` is not at fault: its interval is
[min(lo_x, 0.35), min(hi_x, 0.35)], which keeps a non-negative lower bound. The fault is the product: interval
multiplication treats the two factors as independent, so x ∈ [−1.05, 1.05] gives x·x ∈ [−1.10, 1.10], and then
1 + 8·min(x·x, 0.35) ∈ [−7.8, 3.8] contains 0. Same for `dot(v, v)` and `pow(x, 2)`. **Fix:** `Scalar::sqr`
(range [mig², mag²], L = 2·mag·L(x), the product rule's own L), used for `a * b` when a and b are the same read
(a name or a field or swizzle of one; pure, so equal values), for `dot(v, v)` on the same read, and in `pow` with
an integer exponent (even: [mig^k, mag^k]; odd: [lo^k, hi^k]). Worker B's `x / (1.0 + 0.28 * x * x)` parses as
`(0.28 * x) * x`, which is not recognised; written `0.28 * (x * x)` it is. Prediction: `f11_min` passes,
**L ≈ 1.53** (1 + 0.01·(8 + 16.6 + 28.8)).

## Soundness instrument

`crates/loft-eval/tests/interval_rules.rs`: for each new rule, random argument intervals (including ones that
touch the singular points), and for each a few thousand points inside: the value must lie in the derived range and
|f'(x)|·L(x) (for `atan2`, (|y|·lx + |x|·ly)/r², the worst alignment of the two gradients) must not exceed the
derived L. `crates/loft-passes/tests/lipschitz_inverse_trig.rs`: compiled traces, sampled secants of the CPU twin
inside each derived AABB against the derived L (the method of `lipschitz_rotations.rs`). Prediction: no violation;
the secant ratios printed below.

## Pending (not this lane)

| # | What | The fix, in one line |
|---|---|---|
| F1+ | `repeat_polar(n)` | A map stage: θ' = θ − (2π/n)·round(θ·n/2π), an isometry per sector, L(f) when the folded field is mirror symmetric about the sector wall (else clamp at the wall, spec §2.3); checker `MAP_OPS`, an evaluator `Stage`, WGSL helper, AD through the fold, and `.cell` for the sector index. |
| F2+ | Point-dependent branches in a trace | Trace culling: decide the branch per cell in the derivation and per tile on the GPU (the 23 Nov session notes' item). |
| F3 | Loose bounds for hand-written rotations | Recognise a 3×3 constant orthonormal product in the provenance forms of `lip.rs`, or let `place` take a full rotation so the tilt goes into the pose. |
| F5 | `clip` rebuilds per law-written frame and publishes at the end | Upload law state as rows instead of recompiling; streaming frames changes the contract tested in `clip.rs` (it never publishes partial output), so it needs an owner decision. |
| F6 | Law writes of points and poses | Allow vector and transform fields in `writes`, with their row ABI. |
| F7 | Culling bounds from build-time rows | Bound law-moved rows from their declared field ranges instead of the build-time values. |
| F8 | Contact and history | Capability R (`probe-rows`) and the §5.3 last-frame read; a ring of frames for two seconds. |
| F10 | `world_trace_ray` returns −2 on thin geometry | Not in this lane's list; unexamined. |
| F12 | Authoring guide | `render`, compile cost, and now `asin`/`atan`/`tan` and the angular-pattern advice above. The guide's builtin list changes with this lane (it prints the catalog), which changes the trainer's prompt text. |
| F13 | Library candidates | Unchanged. |
| — | √2 slack in `atan2` | Bound the (x, y) pair jointly through the provenance forms. |
| — | `(k * x) * x` | Recognise a square through a constant factor. |

## Merge onto the integrated tree, 23 Nov evening (unbuilt)

`lane/integrate-meta` merged in by the meta-engine thread after the reset. Two conflicts, resolved by hand:

- `loft-eval/src/eval.rs`: both sides' evaluator fields are kept (`forked` from this lane; `views` and `snapshots`
  from probe-rows). `sample_prior` went with `cut-unused`, so the E405 arm covers `held`, `missed` and `unknown`,
  and its fix text no longer names a motion plan.
- `loft-passes/src/pipeline/stage0.rs`: the integrated tree derives every kind's shapes in one worker pool, so a
  kind no longer has a wall time of its own. **F4's figure changes meaning:** each job is timed, and `derive_s` is
  the kind's share of the wait, its jobs' seconds over the worker count. The 10 s threshold is unchanged. On a scene
  with one slow kind the two definitions agree; on a scene with several the old one would have counted the other
  kinds' work in the pool.

No "Result" line above has been filled: this lane has never finished a build.

## Results, 23 Nov 23:50 (first build of this lane)

Scored by the meta-engine thread on `01e896ca` (this lane on the integrated tree plus main's `cea5fe7e`), lab
profile. Repro output: [repro/after.txt](repro/after.txt), one `loft check` each.

| Prediction | Measured | Verdict |
|---|---|---|
| `f1_atan2` L = 1.0000 | 1.0000 | held |
| `f1_acos` L ≈ 1.0331 | 1.0331 | held |
| `f1_heading` passes, L = 2.0000 | **E212**, "the surface reaches the search limit ±1024 m" | **missed** |
| `f1_pleat` E211 | E211 | held |
| `f1_quadrant` L ≈ 1.22 | 1.2191 | held |
| `f9c_tan` L ≈ 1.0343 | 1.0343 | held |
| `f2_select`, `f2_if`, `f2_ifexpr` L = 1.0000 | 1.0000 each | held |
| `f2_point` E211, `f2_point_ifexpr` E701, `f2_point_letif` E211 | E211, E701, E211 | held |
| `f9b_const_loop` L = 1.1500 | 1.1500 | held |
| `f11_min` L ≈ 1.53 | 1.5177 | held |
| `f9a/main --set size=2` L = 2.0000; `f9a/dup` E011 naming both files | 2.0000; E011 naming `shot.loft` and `dup.loft` | held |
| F4: `check` lines gain `derive N.NN s` | present on every kind line | held |
| No secant above the derived L | six compiled traces, worst ratio 0.9999 (`lipschitz_inverse_trig`) | held |

**The miss.** `f1_heading` no longer panics, but it still does not compile, and `atan2` is not the reason. Any
rotation by a time-dependent angle inside a trace fails the same way, with or without this lane:
`map m = p |> rot_y(0.3 * time)` on a unit sphere gives E212 on the integration binary too. The angle's interval is
all of [−π, π], each rotated coordinate's interval then contains 0 at any distance, and the bound search cannot
exclude any region. The prediction counted the rotation as an isometry for L and forgot that the box search needs a
range. This is finding F3's family (let a rotation go into the pose); it stays pending. So the advice above, "an
angle of something that is not the point: `atan2` now works", is true of the function and not yet of the scene the
Auncel authors wanted.

**Two fixtures were wrong** and are corrected in `01e896ca`: a one-line `if { return … } return …` (a parse error,
E001), and `log(1.5 + p.x)`, which is NaN over half the search region and which E212 rightly refuses.

**The gate on this stack** is in `agent_notes/2026-11-23_meta_engine/INTEGRATE.md` §6.
