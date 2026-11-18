# Review of `docs/proposals/2026-11-18_next.md`, reviewer: opus (18 Nov 2026)

I read the proposal, the landed code at `52c8d98` (`src/ranged.ts` in full, `src/interval.ts` in the parts that matter
for soundness, `src/style/pbr.ts` and `src/style/pbr_parts.ts` in full, `src/geom/solve.ts` lines 520–760,
`src/cli/serve.ts` in full, `tests/ranged.test.ts`), the four agent notes, the interpreter-probe note on
`loft/interp-probe` (be17739), and creative's `RESOURCES.md`. I ran nothing. Any number marked **derived** is my own
arithmetic on numbers the notes measured.

## 1. Verdict

1. The direction is right. One expression graph with a CPU twin, rewards proved over ranges instead of sampled, and
   PBR built from parts all serve the owner's positions, and the three merged steps are real work with honest notes.
2. The plan as ordered is wrong. Step 5 (tier I) is the largest build, and it is aimed at a cost split measured only on
   macOS with a cold Metal cache. It also cannot reach its stated "~0.1 s per sample", because the measured floor from
   the compile child alone is 0.45–0.85 s wall.
3. The plan also leaves out the cost of the reward. A ranged check on a real creature took 4.5–92 s and `solve` takes
   5–83 s on each build, with no time budget and no cache. Either one is larger than the shader compile the plan is
   trying to remove.
4. `recur` is the only route for a model's own simulation, and it is shut out of both `loft serve` and ranged checks.
   That conflicts with positions 1 and 2, and the plan does not mention it.
5. **Fund, with changes.** Move Linux measurement and the blind trial (step 7) ahead of tier I. Add a budget and a
   cache to `solve` and the ranged checks. Make the tier part of a frame's identity. Run tier I only as a whole-scene
   probe until Linux numbers justify it.

## 2. Defects, most serious first

### Design defects

**D1. The step-5 speed target is unreachable as designed, and its premise is unmeasured on the farm's platform.**
Where: proposal §3 "Why two tiers" (lines 47–52) and step 5 ("samples at ~0.1 s", line 203).
- The "~80% of CPU is shader compile" figure comes from creative `RESOURCES.md`: ~20 CPU s per program, made up of
  MTLCompilerService 10.5 s and Dawn/Tint 6 s. That was before the compile speedup, with a cold Metal cache.
- `loft serve`'s own round 2, with a warm Metal cache, measured 1.86 CPU s per program. About 1.1 s of that is the
  compile child, which has no GPU and does no shader work, and MTL was 0.00. So shader work is at most
  0.76 / 1.86 ≈ **41%** warm. Cold (round 1) it is (4.97 + 6.03 − ~1.1) / 11.0 ≈ **90%** (derived).
- Tier I removes only the shader part. The compile child (Bun start, scene import, graph build, `lower`) stays, and it
  was measured at 0.45–0.85 s wall and 1–1.5 CPU s per program. Even with a free GPU stage, a sample costs at least
  that, which is 4–8× the target.
- The farm runs Linux on Vulkan (§3 "Linux"). There the driver's shader cache, Dawn's pipeline cache and the
  Tint→SPIR-V cost are all unmeasured. Step 6 is scheduled *after* step 5, so the plan builds the remedy before
  measuring the disease on the platform that will run it.

Consequence: weeks spent on a tier whose payoff could range from ~0.4 CPU s per program (a warm cache with shared
library WGSL) to ~10 CPU s (cold), and whose headline target is out of reach either way. Fix: run step 6 first, with a
per-phase CPU split on Vulkan, cold and warm. Fund tier I only if Tint plus the driver are above half of the
per-sample CPU at the cache-hit rate a real training stream gives. Restate step 5's target as "shader setup → upload";
the per-sample floor is the compile child until that is made warm too (D6).

**D2. Background promotion breaks determinism.** Where: §3, "may compile a scene it sees often in the background and
swap it in" (lines 51–52), against §3 "Determinism" ("byte-identical", line 58).
- The probe measured interpreted and compiled pixels differing by up to 1.16e−4 before quantisation (the whirlpool's
  worst pixel, traced to multiply-add contraction and f32 stores).
- A frame quantised near a 1/255 boundary therefore changes with the tier. If promotion happens in the background, the
  tier depends on timing, so the same request can hash differently between runs.
- The determinism tuple (scene, camera, time, size, commit, adapter) does not include the tier.

Fix: put the tier into the tuple and into every answer (`tier: "I" | "C"`). In training, never promote within a run:
the tier is chosen once per scene and recorded. Promotion is for the game only.

**D3. `recur` is excluded from both the training path and the checks.** Where: `src/host/headless.ts:10` ("a scene
with recur bakes on the GPU: `loft build` it outside serve"); the ranged-checks note under Limits ("tables filled on
the CPU ... baked recurrences: `recur`" are refused).
- The proposal (§2) lists explicit state through `recur` as a strength.
- In practice, any motion the model integrates itself (a spring, a cloth, a flock, an IK relaxation, a particle
  system) can neither be trained through `loft serve` nor checked. Both refuse it.
- That is exactly position 1's "any equation" and position 2's "any algorithm".

Fix: give `recur` a CPU twin bake (deterministic, f64 or f32-emulated) for checks, and a bake inside the serve
process for training. The bake is GPU work on data, not scene code, so it does not breach the no-scene-code rule:
the child emits the step graph, and the server runs it. State in the plan how ranged checks treat a baked state:
enclose the stepped trajectory (Taylor models), or refuse it by name.

**D4. The cost of rewards and of `solve` is missing from the plan, and both are open-ended.**
- Ranged checks, as measured: claw 93 ms, stone 123 ms, serpent head 4.5 s, and serpent over a `neckRise` range
  91.7 s (3,882 boxes at ~24 ms each).
- The default budget is 20,000 boxes. At 24 ms a box the worst case is **~480 s for one check** (derived).
- `solve` takes 5.0–5.6 s per heron and 83 s for a far deck. The note says "no work budget yet: a slow solve is slow".
- I read `solve()` (`src/geom/solve.ts:558–744`) and found no memo: it runs whenever the scene module is evaluated.
  That means every `loft build`, every serve compile child and every `loft check`.

Consequence: a model that learns to write `solve` or a ranged check makes its own sample 5–100× more expensive. The
plan lists none of this under position 4. Fix (a sketch of the API follows this list):
- A wall-clock or node budget on both, ending in the existing `undecided` / `unsolved` variants and naming the budget.
- A content-addressed cache for `solve` results, keyed by the canonical graph of the holds plus the family and
  preference. This reuses step 8's canonical form.
- Compile the interval evaluator to JS (the note's own next lever) before ranged checks become a training reward.

```ts
// what a task author writes; the budget is part of the reward spec, not of the scene
always([time, 0, 4], ge(clearance, 0.1), { boxes: 2000 })   // exists
solve({ ..., budget: { ms: 1500 } })                          // proposed: `unsolved: budget 1500 ms`
```

**D5. Two vocabularies for one idea: a margin over a time window.** Where: `solve`'s `equal` / `atLeast` / `When`
(`src/geom/solve.ts:57–110`) against ranged's `always([time, a, b], ge(x, y))` (`src/ranged.ts:77`).
- `atLeast(a, b, { from, to })` means exactly `always([time, from, to], ge(a, b))`.
- `leap.holds` hands back `HoldLine`s that someone then has to rewrite as ranged formulas in order to prove them.
- Under position 5, three of the names (`equal`, `atLeast`, `When`) buy nothing that `always` / `ge` / `le` with a
  tolerance does not already give.

Fix: `solve` takes ranged formulas as holds: `always` over a time window gives an interval hold, and a range of zero
width gives an instant. Its result's `holds` are then the very `Ranged` checks that `loft check` runs. An equality is
written `and(ge(a, b − tol), le(a, b + tol))`, as `marginOf`'s own error message already tells authors to do
(`ranged.ts:115`). That is one vocabulary, and solve and check share a proof path.

**D6. The cheaper speed lever comes after the expensive one.** The serve note's conclusion is explicit: the next
throughput lever "is not the server. It is the compile". That means a warm confined compile worker, since
`RESOURCES.md` showed compile does not get faster within one process but Bun start and import do. This lever is
~0.44 CPU s per program and needs no new tier. Fix: make it step 5a, before any tier-I work.

**D7. Tier I and swappable PBR parts contradict each other.** Where: §4 "In tier I the BRDF and term computation are
fixed code; the style's own graph is interpreted" (line 80), against §4 "any part can be swapped and every part runs
in ... the interpreter" (line 83). If the BRDF is fixed code, then `pbr({ fresnel: thinFilm(380) })` either silently
runs the default Fresnel or must fall back to tier C. Fix: decide. The honest rule is that parts are graph like
anything else and are interpreted. "Fixed code" can only be an optimisation for the default parts, keyed by the
parts' canonical hash.

**D8. The space-folding estimate is optimistic by about 4–20×.** Where: §3, "10–30 ms for a 128-step folded march".
- Measured interpreter price: 65–123 ps per pixel-op (whirlpool, crab).
- A 128-step march over a 512² image is 3.36e7 step-pixels. Then 10–30 ms allows **2.4–14 ops per step** (derived:
  10e−3 / (3.36e7 × 123e−12) = 2.4, and 30e−3 / (3.36e7 × 65e−12) = 13.7).
- A folded field with a few fold iterations and a primitive is 40–100 ops. That gives **90–410 ms per frame**, and
  1.4–6.6 s for 16 frames, which is worse than compiling.
- Early ray termination divides by perhaps 2–4. That is unmeasured.

Position 2 names raymarched folding specifically. Fix: route `march` to tier C by default with a named reason, and
measure one folded scene in the whole-scene probe before promising otherwise.

**D9. The plan's text is stale against the code it describes.**
- §5 shows `planted` / `lands` / `clear` / `during` / `vary: { root: spline(6) }`. The code has `over`, `equal`,
  `atLeast`, and `hold` as a function.
- §6 shows `forAll(...)`, which does not exist; `always([param, lo, hi], …)` does that job.
- §6 says that `staysClear` and the rest "gain ranged semantics without new names". `src/check.ts:264` `staysClear`
  is still sampled at `rate`, and the solve note explains why reuse was refused: those checks take attacks and
  skeletons, not expressions.
- §6 says "in progress on branch" while the table says merged.

Because `loft context` is generated from source and the plan is what agents read, the stale text will be copied.
Fix: rewrite §5–§6 to match the code, and state that ranged semantics for the existing checks are not done.

### Code defects in what landed

**C1. Ranged checks have almost no soundness tests for a reward engine** (`tests/ranged.test.ts:34–55`, verified).
- Containment is tested on two expressions (the claw tip and the serpent head), five points per box, with a relative
  slack of 1e−4.
- `interval.ts` has rules for every library function, hashes and gradient noise cell by cell, case-splitting of
  `select`, and fetches.
- One unsound rule turns `holds: true` into an exploitable reward: a model optimising margin will find it.
- The 1e−4 slack also hides any unsoundness smaller than that, which is the size of the f32/f64 gap the note
  mentions.

Fix: a per-operator randomised containment test. For each rule, draw random boxes (including boxes that straddle
zeros, poles and integers) and random points inside, and compare against the f64 twin with zero slack. The f32 check
is a separate one. Name the bug it catches: an outward-rounding or Taylor-remainder slip in a single rule.

**C2. `and` / `or` margins mix units, and a model can exploit that** (`src/ranged.ts:112–113`). `min(metres, seconds)`
is acknowledged in the note, but as a dense reward it lets a model raise the margin by inflating the term in the
larger unit, or fail one leg and still read a large `or`. Fix: require normalised atoms when checks feed a reward (each
`ge` divided by a declared scale), or report per-atom margins and grade on the worst normalised one.

**C3. `pbr`'s environment weight is a silent fallback for swapped lobes** (`src/style/pbr.ts:36–60`). `lobeAlbedo` is
fitted to `ggx` with `smithGgx`. A replaced `distribution` or `visibility` keeps it, with only a doc comment as
warning. That breaks the furnace test with no error, against the owner's "no silent fallbacks". Fix: compute the
24×24 albedo table at build time from the actual parts by CPU quadrature (this costs milliseconds, the same as the sky
projection already does), or refuse by name.

**C4. The sky projection is frozen at the default sun and time 0** (`src/style/pbr_parts.ts:143–169`, `evaluate(sun)`).
- A day–night cycle, or a sun driven by an input, lights bodies with the wrong ambient. Only the one specular tap
  follows the sun.
- The cache is keyed by the sky function's identity and the default sun only, not by param values.
- The proposed `loft try --set param=v` (step 8) would therefore read a stale projection for a sky that reads a param.

Fix: project on the GPU per frame (9 coefficients × 1024 taps is one small compute pass), or key the cache by the
evaluated sky constants and refuse a time- or param-varying sky by name.

**C5. `pbr` is less swappable than claimed** (`src/style/pbr.ts:27–31, 122–155`). Water colours, the water's
roughness 0.12, the floor albedo, the floor's roughness 0.9 and the fog coefficient 0.008 are constants in the
closure. They are not parts. §4's "Nothing is baked in" is not true of the water and the ground. Fix: either add them
as parts or say in §4 that a different sea means `{ ...pbr(), water }`.

**C6. Serve: a lost device drops the other in-flight answers** (`src/cli/serve.ts:261–265`). Minor, since callers
already treat EOF as a host failure. Also verified: requests take no `params` / `inputs` (`serve.ts:84`), so step 8's
`try` cannot run through serve.

**C7. Not verified:** whether `solve`'s global `solves++` (`solve.ts:560`) leaks a `solve{id}_` name into WGSL or a
hash. If it does, a warm compile worker (D6) would break artefact identity.

## 3. Against the five positions

1. **Programmable motion.** Mostly honoured: equations, `keys` and `solve` all return ordinary expressions, and `solve`
   is explicitly one source among several. The constraints:
   - `recur` cannot be trained or checked (D3);
   - `solve` has one family (uniform cubic B-spline);
   - its holds cannot range over params or inputs;
   - its search is local, so the start picks the side of an obstacle.

   That last point is fine if said. The model must be told that `spline(n, from)` is how to choose a homotopy class.
2. **Arbitrary compute.** At the graph level this is honoured. At the tier level it is not yet:
   - the probe's interpreter refused `normalize(vec3)`;
   - folding is expensive when interpreted (D8);
   - raw WGSL turns checks into `undecided`;
   - "a new kind of pass needs a library change" (§7b).

   The last is the real fence: no model-written compute pass, no custom G-buffer read, no multi-pass simulation. Add a
   `pass` variant that is "a compute graph over named buffers the scene declares". That one name buys the capability.
3. **Own styles, swappable PBR.** Honoured in shape. It is weakened by C3 (silent energy errors), C5 (hard-coded water
   and ground) and D7 (a fixed BRDF in tier I).
4. **Faster, reliable, deterministic training.** This is where the plan is weakest:
   - the speed target is unreachable as designed (D1);
   - promotion breaks determinism (D2);
   - reward and solve costs are unbounded (D4);
   - the cheaper lever is scheduled late (D6).

   It does honour typed errors, the GPU-free compile and byte-identical headless frames (proved in the serve note).
5. **Every name buys a capability.**
   - `equal` / `atLeast` / `When` duplicate ranged formulas (D5).
   - `forAll` is planned but unnecessary, since `always` over a param is already ∀.
   - `measure` / `why` / `try` are three CLI verbs. `why` and `try` are one capability: evaluate checks at a setting
     and explain the witness. Make them one command with flags.
   - `pbr`, the part names and `always` / `eventually` earn their place.

## 4. Missing

- **Who writes the checks in training.** If the model writes both scene and checks, it can write vacuous ones, and
  the margin is easy to game. The plan needs a reward spec: checks are task-authored and held out, the model's own
  checks are advisory, and atoms are normalised (C2).
- **A soundness audit of the interval rules** before any reward depends on them (C1).
- **Linux numbers**: the per-phase CPU split, cache behaviour, and byte-identical repeats across two Linux machines
  with the same driver.
- **A whole-scene tier-I probe.** The step-2 probe measured paint only. Pose, vertex, geometry, `march` and post stages
  are unmeasured, and the probe's own note says credit for whole-scene compile "would require covering the other
  stages too".
- **GPU-side `recur` and state for training** (D3).
- **Twin/GPU verdict disagreement near zero margin.** The note puts f32 against f64 at ~1e−7 relative per operation,
  but the probe's foam mask amplified 9.5e−7 into 1.4e−4. Picture-based rewards need the same care. Until 6.5 lands,
  a check whose |margin| is below a stated epsilon should report `undecided: precision`.
- **Model reachability.** Ranged checks are not in `loft context` or `src/index.ts`; `solve` is marked `~`. No
  experiment asks whether a model, given only `loft context`, writes a working `solve` or ranged check. A 10-brief
  probe is cheap and decides whether these are capabilities in practice.
- **Throughput as well as latency.** Serve draws one request at a time. Batching is planned in step 5, but GPU
  utilisation per server has never been measured.

## 5. Order of work

Change: step 7 (the blind trial) runs now and in parallel, and it **gates step 5**. If Loft loses to three.js, §8
already says Loft becomes a library serving three.js scenes. A Loft-graph interpreter would then be sunk cost. Step 6
(Linux) moves ahead of step 5. Step 8 is cut to one verb plus the canonical cache, and the cache comes first because
`solve` needs it (D4). Step 9 stays later, except that the operator soundness tests (C1) move to now.

Next three things to build:

1. **Linux end to end, with a cost split.** One `loft serve` on a Vulkan GPU, the 10 creative bench programs, cold and
   warm driver cache. Each program's CPU is split into Bun start, import, compile, lower, Tint, driver and frames.
   - **Pass:** all 40 frames are byte-identical across two server runs. The split is recorded. Tier I is funded only
     if Tint plus driver are ≥50% of per-program CPU at the cache-hit rate of a 100-program training stream.
   - **Fail / stop tier I:** below 50%. In that case build item 2 and the canonical cache instead.
2. **A warm confined compile worker, a `solve` cache and budgets.** The compile child is reused under the same
   `--confine`. `solve` is memoised by the canonical hash of its spec. `solve` and ranged checks get budgets ending in
   their existing typed variants.
   - **Pass:** median compile CPU over the 10 programs falls from ~1.1 s to ≤0.5 s. Artefact sha256 is identical to a
     cold `loft build` for all 10. The heron's second build takes <50 ms for `solve`. A 2,000-box ranged check ends
     `undecided: boxes` in under 10 s on the serpent's 3b case.
3. **A whole-scene tier-I probe, with determinism rules.** Crab, wyrm and one folded `march` scene, every stage,
   16 frames, tier recorded in each answer, no promotion.
   - **Pass:** GPU time plus setup ≤0.25 s per 16-frame sample on crab and wyrm. Every pixel within 1/255 of tier C.
     Coverage: ≥90% of the 10 bench programs' graphs run in tier I or hand off with a named reason. The folded scene's
     measured ms per frame is compared with D8's derived 90–410 ms.
   - **Fail:** over 0.25 s on crab or wyrm, which means tier I is not worth building.

## 6. Numbers

| Quantity | Source | Value | Kind |
|---|---|---|---|
| Interpreter price per pixel-op | probe: 1.999 ms / (512² × 63); 5.493 ms / (512² × 323) | 121 ps; 65 ps (note: 123, 65.8) | derived from measured |
| Shader share of per-program CPU, cold Metal | serve round 1: (4.97 + 6.03 − ~1.1) / 11.0 | ~90% | derived |
| Shader share, warm Metal | serve round 2: ≤ (1.86 − 1.1) / 1.86 | ≤41% | derived |
| Per-sample floor after tier I | serve: compile child 0.45–0.85 s wall, 1–1.5 CPU s | 4–8× the 0.1 s target | measured |
| Tier-I paint, 16 frames, whirlpool at full coverage | 16 × 5.49 ms + 0.57 ms setup | ~88 ms (paint only) | derived |
| Same, warm Metal, against compiled | probe: compiled warm paint setup 4.57 ms; interpreted adds 5.49 ms a frame | interpretation loses after 1 frame | derived |
| Folded 128-step march, interpreted | 3.36e7 step-px × 65–123 ps × 40–100 ops | 90–410 ms a frame | derived (field size assumed) |
| Ops per step that fit the plan's 10–30 ms | as above | 2.4–14 | derived |
| Worst-case ranged check at default budget | 20,000 boxes × 24 ms | ~480 s | derived |
| Ranged serpent 3b | note | 3,882 boxes, 91.7 s | measured |
| `solve` per build | solve note | 5.0–5.6 s; 83 s far deck; not cached | measured; "not cached" verified by reading |
| pbr cost over toon | pbr note | +0.07 to +0.52 ms a frame | measured |
| Warm-compile-worker lever | serve note | ~0.44 CPU s per program (Bun start, import, device) | measured as a difference |
