# Review of "Loft next" (reviewer: fable, 18 Nov 2026)

Reviewed: `docs/proposals/2026-11-18_next.md` and the code on main at `52c8d98`, plus the interpreter probe note on
`loft/interp-probe`. I did not read the other reviewers' files in this directory. Things I ran myself are marked
**ran**; numbers taken from the authors' notes are marked **theirs**; anything else is marked **unverified**.

## 1. Verdict

1. **Fund, with changes.** Keeping Loft and adding ranged checks, `solve`, PBR from parts and a warm server is the
   right direction, and the landed code is careful (my fuzz of the interval evaluator found no real unsoundness).
2. **The headline speed claim is wrong by 3 to 8 times.** Tier I removes shader compilation but not the per-scene
   graph build (0.6 to 1.4 CPU s), and no plan step touches that.
3. **The two flagship features do not compose yet.** A curve returned by `solve` fails the ranged check of its own
   hold (ran), and a ranged check on a real body costs 4.5 to 92 s (theirs).
4. **Reorder.** Linux measurement and the blind trial first (cheap, and either can redirect everything), then the
   solve → check → reward loop under a budget, then a whole-scene interpreter probe before building tier I.
5. **Stop three things:** background promotion to tier C in training, equality saturation, and correctly rounded
   maths on the GPU. Each has a cheaper substitute that buys the same capability.

## 2. Defects, most serious first

### Design

**D1. Tier I's sample cost leaves out the compile child.** Section 3 (lines 47–52) and step 5 promise "about 0.1 s
for a 16-frame sample" because "a new scene is a buffer upload". The bytecode comes from the same expression graph,
which a Bun child builds from `scene.ts`. That child is not shader work and stays. Creative's phase table (theirs,
after interning): compile 1.06 + lower 0.19 CPU s, plus Bun start 0.21, TypeScript load 0.63 and check 0.17 when not
warm. I timed `LOFT_NO_GPU=1 loft build` on two examples (ran, load 5): `materials/spheres.ts` 0.22 s wall and
0.57 CPU s; `grab.ts` 0.63 s wall and 1.3 CPU s. The serve note already says "the next lever is not the server, it is
the compile". Consequence: after tier I, 85% or more of a sample's CPU is single-threaded JS that the plan never
attacks, and the farm sizing (GPUs versus cores) is wrong. Fix: add a step "warm confined compile worker, compile
under 0.2 CPU s" beside step 5, and restate step 5's target as compile + interpret.

**D2. The probe that "passed" does not support "tier I goes ahead".** It interpreted one paint function. The same
note counts 78 modules and 108 pipelines for the Titan, 48 and 68 for the Wyrm, and only two modules shared between
scenes. Whole-scene compile is 5.4 and 3.3 s cold, about 50 ms per pipeline (derived). Tier I therefore needs a fixed
interpreter for every stage kind (pose, compute tables and scans, shadows, outline, water, march, traced), not
"full operator coverage". Using the probe's own numbers at the plan's 16 frames (derived): interpreting the 323-op
whirlpool paint costs 16 × 5.49 = 88 ms against a 51 ms cold compile of that pipeline, a loss of 37 ms; with a warm
driver cache it loses 84 ms. One paint stage already spends the whole "0.1 s per sample". The march estimate is also
off: at the measured 65.8 ps per pixel-op, a 128-step march costs 2.2 ms per field op per full-screen frame, so
"10–30 ms" allows a field of 5 to 14 ops. One fold plus one primitive is 15 to 40 ops: 33 to 88 ms a frame, 0.5 to
1.4 s for 16 frames, slower than compiling. The owner's named case (raymarched folding) is tier I's worst case. Fix:
make the next probe a gate with a number (section 5), and move per-tile pruning from "if step 2 fails" to a
requirement for `march`.

**D3. `solve` and ranged checks do not compose.** Section 7b says a hold is "any expression ≥ 0; the ranged machinery
works on it". I solved a one-unknown hop over a bump and ran `always([time, 0, 1], ge(hold.margin, 0))` on the
returned hold (ran): `holds: false`, margin −1.8e-13 (−9.2e-9 in a second case). The solver rests on active
constraints to within `TOLERANCE` (`solve.ts:161`) from either side; the checker's zero is exact. With
`ge(m, -1e-5)` it holds in 22 ms. There is also no adapter from `HoldLine` to a check, `equal` holds are margin 0 by
construction (the ranged note lists "margin exactly 0 over a stretch is undecided"), and the heron demo ships
`checks: []` (`heron_leap.ts:132`), so the loop was never closed on the one real example. Fix: `solve` aims
`atLeast` at +δ and reports δ; a check accepts a `HoldLine` directly (`checks: leap.holds`) with that δ as its `by`.

**D4. The reward is too slow and punishes the wrong thing.** The serpent's head clearance took 4.5 s over time and
91.7 s with one param range (theirs): 12 to 24 ms a box on a 15k-node body. The judge's budget for the whole sample
was 4 cores × 3600 / 3142 = 4.6 core-s. One body check spends it. Second, `run()` maps `undecided` to
`pass: false` (`ranged.ts:152`), and refusals cover raw WGSL, GPU-only functions, cellular noise, CPU tables,
skins and every `recur` scene. A reward built on `pass` trains the model away from exactly the constructs position 2
protects. Third, contact (a planted foot, a landing) has margin 0 and never decides (`ranged.ts:359` keeps splitting
until the budget). Fix: budget in milliseconds, not boxes; the reward reads `bounds[0]` (proven, monotone in
budget); a refused or undecided ranged check falls back to the sampled check as a named variant of the result, never
to a fail; compile the interval evaluator to JS before any training use.

**D5. Background promotion breaks determinism.** Section 3 says same machine and driver give byte-identical frames,
and that `loft serve` "may compile a scene it sees often and swap it in". The probe measured compiled against
interpreted at up to 1.16e-4 apart, "within 1/255 before quantization": not byte-identical. A frame's sha256 then
depends on how often the server has seen the scene. Fix: tier is part of the request and of the frame's identity;
no promotion in the training path.

**D6. No work budget anywhere, and a slow scene is scored as a host fault.** `solve` loops up to 24 rounds × 60
outer × 100 inner steps with no limit (`solve.ts:692–723`); the note records 83 s for a far deck. `serve.ts:129–143`
gives the compile child no timeout, and a child killed by the confinement's CPU limit is answered as exit 1
"internal" (`serve.ts:139–141`). RESOURCES.md describes the same path scoring a program 0 for a host failure (a hazard it sized, not a case it observed). A 5 s
heron solve also re-runs on every load (check, render, each variant). Fix: one typed outcome, "budget", for
`solve`, ranged checks and compile, naming what ran out; cache solved numbers by the content of the spec.

**D7. The prize was measured on Metal; the farm is Vulkan.** "80% is shader compile" is 5.96 s of Dawn plus 10.5 s
of Apple's MTLCompilerService out of 19.9 (theirs; 83%, recomputed). Linux is step 6, after tier I is built. Nobody
has measured what an NVIDIA driver charges for the same pipelines, or whether its disk cache is reachable in the
sandbox. Fix: Linux first.

**D8. The trial comes last and its gate has little power.** Steps 3 and 4 grew the authoring surface before step 7
tests whether that surface earns its keep. "10 of 12 against each" passes a Loft that truly wins 75% of the time with
probability 0.39 per baseline, 0.15 against two, 0.02 against all four arms (ran, binomial). The fail branch (stop
growing the authoring layer) would most likely fire on a good Loft. Fix: 24 briefs, pass at 17 (false pass 3.2%,
power 0.77 at a true 75%), against three.js and raw shaders only; the other arms are informative, not gates.

**D9. Two items buy nothing new.** 6.5 (correctly rounded `sin`, `exp`, `pow` on GPU and twin) cannot make verdicts
agree while WGSL may fuse multiply-adds and gives division 2.5 ulp (unverified, from memory of the spec; the probe
did observe contraction differences). The capability wanted is "a verdict that holds for the GPU's value". The
affine form already carries a rounding radius: an f32 mode that pads each operator by its f32 error gives that in
tens of lines. Step 8's equality saturation either changes evaluation order (pixels move, against section 3) or does
not (then the interned structural hash that exists today is the cache key). Measure the dedup hit rate on creative's
970 stored programs with the structural hash first.

**D10. Three vocabularies for one idea, and a stale proposal.** A condition over time is written three ways: check
constructors (`staysClear`), holds (`atLeast(a, b, { from, to })`) and ranged (`always([time, a, b], ge(a, b))`).
Section 5 still shows `planted`/`lands`/`clear`/`prefer: smooth`, section 4 shows six `PbrParts` slots with other
signatures (seven landed), and section 6 shows `forAll` (does not exist) and says "in progress". The landed solve API
is the better one. Fix the document, and make a hold and a ranged condition the same value.

### Code that landed

**C1. Ranged checks are merged but not usable** (step 1 says "merged"). Nothing is exported from `src/index.ts`,
`src/check.ts` does not use it (ran, grep), `loft context` does not mention it, and no existing check became ranged.

**C2. Overflow is an internal error, not a typed one.** `fromI` (`interval.ts:293–297`) takes the middle of an
interval with an infinite end and produces NaN; `bounds` then throws `internal: interval: an empty enclosure
[NaN, NaN]`, which `ranged.ts:295–298` does not catch. Found by fuzz (ran) on a `length` of a vector holding
`exp(pow(time, time))` for t in [2.39, 4.39]. Only `exp` and `pow` check overflow; `mul`, `sqr` and `add` do not.
Fix: non-finite bounds throw `Unbounded("overflow")`.

**C3. Two enclosures miss the twin by rounding.** `imix` clips to the hull of its ends unpadded
(`interval.ts:199–204`), but the twin's `a + (b − a) t` at t = 1 can exceed b by an ulp (three cases in 14,106 enclosures at zero slack, ran).
`imod`'s wide branch (`interval.ts:189–194`) returns the real range, while the twin's `a − b floor(a/b)` is off by
an ulp of a (−16384 against [−0.62, 0] at a ≈ 1e19). The header's claim (lines 15–18) is false to that extent.
Harmless until the value feeds `floor`, `step` or `sqrt`. Fix: pad both.

**C4. The soundness test is thin.** `tests/ranged.test.ts` checks 8 boxes × 5 points of two expressions with a slack
of 1e-4 (line 27), eleven orders looser than the evaluator achieves. My fuzz (random trees over about 30 operators, two
leaves, 104,000 expressions, 73,213 enclosed, 2.93 million points, slack 1e-9): no violation beyond C2 and C3. That
is a good result and belongs in the suite as the test that names C2 and C3.

**C5. `infeasible` asserts what it has not shown.** The search is local, but the message says the holds "cannot all
be met" and "without any one of them the others can be met" (`solve.ts:191–195`), and the deletion filter restarts
from the same stopped point (`solve.ts:775–793`). The note records its own demo as "wrong twice". Fix: two variants:
`infeasible` only when every hold in the conflict is linear in the unknowns, otherwise `notFound` with the "give a
start on the right side" fix.

**C6. PBR parts are not all slots.** The environment's specular weight is a fit to the default `ggx` and `smithGgx`
(`pbr.ts:44–50`) and is kept silently when either is swapped (line 42 admits it). A non-CPU sky throws a plain
`Error` (`pbr_parts.ts:156`), so it ends as exit 1 or 3 rather than the scene refusal (exit 4) it is; which of
the two is unverified. The sky is projected at time 0 and the
sun's default (`pbr_parts.ts:143–147`), so a moving sun lights bodies from a stale sky without a word. Fix: make
the lobe albedo a slot whose default integrates the given parts on the twin at build; type the error; refuse or
re-project when `sun` reads `time`.

**C7. `loft serve` cannot draw what the checks talk about.** Requests carry no `params` or `inputs`
(`serve.ts:208`), so a witness at (t, params) or a `forAll` variant cannot be rendered. `recur` scenes cannot be
built in the GPU-less child at all (`headless.ts:10`), so explicit state, which section 2 keeps, is outside both
training and ranged checks.

Not reproduced: the PBR note's "`--style toon` on a toon scene fails with a param declared twice". `build --style`
worked on `grab.ts` and `spheres.ts` (ran); I did not find the failing scene.

## 3. Against the five positions

1. **Motion is programmable.** Honoured in shape: curves are expressions and `solve` returns editable B-splines.
   Boxed in: `Family` has one variant (`solve.ts:39`). The model cannot ask the solver for the constants of its own
   equation (launch speed and angle of a ballistic arc), for a periodic curve (a walk cycle: y(0) = y(T) reads two
   times and is refused), or for a curve valid across a param range. A scalar unknown is a jet (v, 0, 0, 0) and
   costs little in this design.
2. **Arbitrary compute graph.** Tier C honours it. The fence is not the API but the reward: checks refuse or cannot
   decide the clever constructs (D4), and tier I makes them the slow samples (D2). "Never refused for being too
   clever" needs the same rule for checks: never scored down for being unreadable.
3. **Own styles, shared PBR parts.** Honoured. Exceptions: the hidden coupling in C6, and the fixed frame structure
   the plan admits.
4. **Faster, reliable, deterministic.** Not yet. Speed is mis-sized (D1, D2), promotion breaks determinism (D5),
   missing budgets turn slow scenes into host faults (D6), and Linux is unmeasured (D7).
5. **A new name buys a capability.** `solve` did this well (`equal` and `atLeast` replaced four proposed names).
   Against it: `forAll` beside `always`; holds beside ranged conditions (D10); `measure`, `why` and `try` as three
   commands when `Decision` already carries the witness (`ranged.ts:25–37`) and the evaluator the per-leaf sensitivity
   (`interval.ts:546`); equality
   saturation (D9).

## 4. Missing

- A compile-cost step (D1) and a per-phase CPU table after every speed change, on Linux.
- Typed budgets (D6) and a defined reward for undecided and refused checks (D4).
- An f32 mode of the enclosure in place of 6.5 (D9).
- `params` and `inputs` in serve requests; a build path for `recur` that keeps the compile untrusted.
- Scalar and periodic unknowns for `solve`; a cache of solved numbers.
- The operator fuzz in the suite (C4).
- Interpreter risks nobody has priced: the pose and compute stages (a body is ~15k nodes after tables are inlined),
  data-dependent loops inside a per-pixel dispatch loop, `fwidth`, and register pressure beyond the probe's 33 vec4.
- In the trial: first-draft validity and tokens spent, not only preference; and one arm that gives the writer the
  check's witness, since exact checks are the claimed advantage.
- A steady-state cache measurement: 100 distinct programs through one server with the driver cache kept, to learn
  what share of pipelines is cold in a real run. Tier I's gain is about 8 times if all are cold and about 1.3 times
  if all are warm (section 6).

## 5. Order of work

Changes to steps 5–9: split 5 into a gated whole-scene probe and the build, and drop promotion from training; move
6 (Linux) to first; start 7 now with the 24-brief gate; in 8 drop equality saturation and fold `measure`/`try` into
`loft check` flags; in 9 replace 6.5 with f32-padded enclosures and hold 6.3 until checks have budgets.

The next three things to build:

1. **Linux baseline (two days).** Creative's 10 bench programs plus 100 distinct training programs through
   `loft serve` on the farm's Vulkan GPU, with per-phase CPU. Pass: every frame byte-identical across two runs and
   a restart, and driver + Dawn compile is at least 60% of per-program CPU in steady state. Fail on the second
   clause: build the warm compile worker instead of tier I.
2. **Blind trial (the judge priced it at 10 engineer-hours).** 24 briefs, one shot, `loft context` only. Pass: 17 of
   24 against three.js and against raw shaders, and at least 20 valid first drafts. Fail: the plan's own fail
   branch applies.
3. **Close the loop on the heron.** `checks: leap.holds`, the interval evaluator compiled to JS, budgets typed.
   Pass: all 26 holds return `holds: true` at the solver's reported δ, none undecided, total check CPU at most 5 s
   and `solve` at most 2 s (its own prediction). Fail: ranged checks stay for closed forms only, and 6.3 and 6.4
   are cut.

Fourth, the tier-I gate: one whole Titan frame with zero per-scene pipelines at 20 ms or less interpreted at 512²,
and the whirlpool march within 3 times its compiled frame time after pruning. Only then step 5.

## 6. Numbers

| Claim | Recomputed | Basis |
|---|---|---|
| Shader compile is ~80% of CPU | (5.96 + 10.5) / (9.4 + 10.5) = 83%, on Metal, before shared shaders | theirs |
| Tier I: "about 0.1 s per 16-frame sample" | compile child alone 0.22–0.65 s wall, 0.57–1.4 CPU s | ran |
| Interpreter price | 2.032 ms / (512² × 63) = 123 ps per pixel-op; 5.571 ms / (512² × 323) = 65.8 ps | derived from theirs |
| Paint interpreted over 16 frames | 32 ms (63 ops), 88 ms (323 ops); net against cold compile +2 ms and −37 ms; against warm −31 and −84 ms | derived |
| "10–30 ms for a 128-step folded march" | 2.2 ms × field ops per full-screen frame; 10–30 ms means 5–14 ops; 30 ops is 66 ms | derived |
| Cold pipeline cost | 5442 / 108 = 50 ms; 3274 / 68 = 48 ms | derived from theirs |
| Gain from tier I | all shaders new: (5.68 + 6.78) / ~1.6 ≈ 8×; driver cache warm: 2.13 / ~1.6 ≈ 1.3× (CPU s, serve rounds 1 and 2) | derived from theirs |
| Throughput after tier I | 3600 / 1.25 = 2,880 samples per core-hour; with one 4.5 s body check 626; with a 5 s solve as well 335 | derived |
| Ranged check cost | 93 ms and 123 ms closed forms; 4.5 s and 91.7 s on the serpent | theirs, not rerun |
| Ranged check on a solved spline hold | 12–30 ms, 156–244 boxes | ran |
| `solve` cost | heron 5.0 and 5.6 s, far deck 83 s (theirs); one-unknown hop 17–23 ms (ran) | mixed |
| Trial gate, 10 of 12 | false pass 1.9%; power 0.39 at a true 75%, 0.89 at 90% | ran |
| Trial gate, 17 of 24 | false pass 3.2%; power 0.77 at 75%, 0.998 at 90% | ran |
| Interval soundness | 2.93M points, 73,213 enclosures: 1 internal throw, 1 `mod` miss at 1e19, ulp-level `mix` misses | ran |
| `tests/ranged.test.ts`, `tests/solve.test.ts` | 7 pass, 0 fail, 0.9 s | ran |

My two scripts (the fuzz, and solve-then-check) were run from a scratch directory and are not in the repository; the
fuzz is described in C4 closely enough to rewrite.
