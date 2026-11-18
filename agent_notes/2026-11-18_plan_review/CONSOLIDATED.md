# Consolidated review of "Loft next" (18 Nov 2026)

Inputs: `BRIEF.md` and all four reviews, `REVIEW_opus.md`, `REVIEW_fable.md`, `REVIEW_astra.md` and `REVIEW_sol.md`.
None is missing. I checked the plan (`docs/proposals/2026-11-18_next.md`) and main at `52c8d98`, the probe note on
`loft/interp-probe`, creative `RESOURCES.md` and creative `7c9c6963`. I re-ran every correctness counterexample the
reviewers reported with one CPU-only Bun script, kept outside the repository at
`/private/tmp/claude-501/-Users-nicholasbardy-git-wave-sim/e6432553-7bca-4903-b961-fca3d7f05ed4/scratchpad/loftrev/repro.ts`.
I re-derived every number used below. I re-ran no GPU timings and no Linux work.

## 1. Verdict

1. The direction is right. Keep one expression graph with a CPU twin, ranged rewards, PBR from parts and a warm server.
2. Do not use ranged checks or `solve` as training rewards yet. On main both state false things. I reproduced 9 failures.
3. Step 5 is aimed at a target it cannot reach (0.1 s a sample, against a compile child of 0.45–0.85 s wall), and the
   platform it is meant for has not been measured.
4. The model trains through creative's import allowlist, which shuts out `march`, `draw`, `pbr`, `solve`, the ranged
   checks and even `sqrt`, `exp`, `length` and `dot`. Position 2 does not yet hold where it counts.
5. **Fund, with changes.** First a truthful reward path, a Linux baseline and the trial on the real training surface.
   Then a whole-scene tier-I probe with a number to beat. Then tier I.

## 2. Agreement map

O = opus, F = fable, A = astra, S = sol. "Repro" means I reproduced it on `52c8d98`. "Read" means I confirmed it in the
source.

| # | Claim | Who | My check |
|---|---|---|---|
| 1 | `imix` clips to the hull of its ends, so it excludes the twin's rounded value and proves a false claim | A (false proof), F (ulp misses, "harmless") | **CONFIRMED**, repro: `mix(2^56, 1, time)` at t=1 is 0 on the twin; `always([time,1,1], ge(e,0.5))` returns `holds: true`, margin 511.5 |
| 2 | Strict and non-strict comparisons give the same margin, and `not` loses the boundary truth | A, S | **CONFIRMED**, repro: `gt(time,0)` and `not(ge(time,0))` over [0,0] both return `holds: true` |
| 3 | A range end that depends on a bound leaf is evaluated at time 0, so the wrong domain is proved | A | **CONFIRMED**, repro: ∀t∈[0,1] ∀x∈[0,t]: x≤0.5 returns true with margin +0.5; the true margin is −0.5. Read: `ranged.ts:247` evaluates the ends with `probe.scalar` |
| 4 | Held params and inputs are enclosed as f64, while the twin stores them in `Float32Array` | S | **CONFIRMED**, repro for both an input and a param of 16777217: the twin gives 0, the check proves ≥0.5 |
| 5 | Overflow becomes an internal NaN error, not a typed refusal | F | **CONFIRMED**, repro: `internal: interval: an empty enclosure [NaN, NaN]` |
| 6 | The box budget is not a hard limit | A | **CONFIRMED**, repro: `boxes: 1` spent 25 boxes and reported `finished` |
| 7 | `run()` turns `undecided` into `pass: false`, and the CLI counts it as a failure | F, A, S | **CONFIRMED**, read `ranged.ts:147–155`; repro: `pass:false, measured:"undecided (box budget of 6 spent)…"` |
| 8 | The soundness test is thin: two expressions, 1e-4 relative slack | O, F | **CONFIRMED**, read `tests/ranged.test.ts:19–34` |
| 9 | Ranged checks are not exported, not in `loft context`, and no existing check uses them | F, A, S | **CONFIRMED**: nothing in `src/` imports `ranged.ts`; `src/checks.ts` exports only from `check.ts` |
| 10 | `solve` reports success for a hold that is NaN | A, S | **CONFIRMED**, repro: `atLeast(sqrt(0.5−time), 0)` returns `worst: 0` |
| 11 | `solve` says "infeasible" when only its local search failed | F, A, S | **CONFIRMED**, repro: `atLeast(x², 1)` from 0 gives `infeasible`; from 1 it is solved with a miss of 7e-14. The heron's own `refused.txt` shows the same message |
| 12 | `solve` has no budget and no cache, and runs again on every module load | O, F, S | **CONFIRMED**, read `solve.ts:558` (no memo) and the note's line 137 ("no work budget"); a repeat solve ran again |
| 13 | A curve from `solve` fails the ranged check of its own hold | F | **CONFIRMED**, repro: margin −2.0e-13, `holds: false`; with ge(m, −1e-5) it holds |
| 14 | The heron ships `checks: []` | F, S | **CONFIRMED**, `heron_leap.ts:132` |
| 15 | The 0.1 s per sample target ignores the compile child | O, F, A, S | **CONFIRMED**. The serve note measures the compile child at 0.45–0.85 s wall and 1–1.5 CPU s; 16 × 5.571 ms = 89 ms for the whirlpool paint alone |
| 16 | "~80% shader compile" was measured on Metal with a cold cache | O, F, A, S | **CONFIRMED**: (5.96+10.5)/(9.4+10.5) = 82.7%. With a warm Metal cache it is ≤(1.86−1.1)/1.86 = 41% (serve round 2) |
| 17 | A 128-step folded march at 10–30 ms leaves room for only 5–14 ops a step | O, F, A | **CONFIRMED**: 10 ms and 30 ms divided by (128·512²·65.8 ps) give 4.5 and 13.6 |
| 18 | The step-2 probe priced paint only, so it cannot justify building tier I | O, F, A, S | **CONFIRMED** by the probe note. It also gives a whole-scene **warm** compile of 569 ms for Titan and 313 ms for Wyrm, which no reviewer used (see D4) |
| 19 | Background promotion breaks byte determinism | O, F, A | **CONFIRMED**: the compiled and interpreted paths differ by up to 1.16e-4 before quantising, enough to flip a byte near a 1/255 step |
| 20 | "Fixed BRDF in tier I" contradicts "every part is swappable" | O, S, A | **CONFIRMED**: plan §4 says both |
| 21 | The PBR environment weight is fitted to ggx/smithGgx and is kept, silently, when either is swapped | O, F, A, S | **CONFIRMED**, `pbr.ts:36–49`. The only warning is a doc comment |
| 22 | The sky is projected once, at time 0 and the default sun | O, F | **CONFIRMED**, `pbr_parts.ts:143–169`. Documented under "Cannot", but the plan still says "exact" |
| 23 | Environment light is multiplied by the sun's amount; the clear colour evaluates the sky on the CPU | A | **CONFIRMED**, read `pbr.ts:105,113`. A sun of 0 kills all sky light, and a sky the GPU alone can evaluate throws whatever `environment` is given |
| 24 | A sky the CPU cannot evaluate throws a plain `Error` | F | **CONFIRMED**, `pbr_parts.ts:156`. The exit code it ends with is unverified |
| 25 | Water and ground constants are fixed in the closure | O | **CONFIRMED**, `pbr.ts:27–31,135` |
| 26 | Lamp arrivals come already multiplied by shadow | A | **CONFIRMED**, `light.ts:192–200`; the comment says so |
| 27 | Serve requests carry no `params` or `inputs` | O, F | **CONFIRMED**, `serve.ts:84,208` |
| 28 | Serve reads the bundle before `--check`, has no limit on pending requests and no timeout on the child; a child killed by a signal becomes exit 1; a lost device drops other in-flight answers | A, S, F, O | **CONFIRMED**, `serve.ts:130–143,196–197,275–280` |
| 29 | Two requests with the same `out` share `out/artefact` | A | **CONFIRMED** by reading `serve.ts:233`. I did not run the race |
| 31 | `recur` cannot be built in serve or checked | O, F | **CONFIRMED**, `headless.ts:10` and the ranged note's line 149 |
| 32 | Creative's training allowlist excludes most of Loft | A | **CONFIRMED**, `creative@7c9c6963:training/loft/host/allowed-imports.json`: 28 shared names and 9 vendor names. No `march`, `draw`, `pbr`, `solve`, `sqrt`, `exp`, `length`, `dot` or `normalize` |
| 33 | The plan's text is stale: `planted`/`lands`/`forAll`, "in progress", "without new names" | O, F, S | **CONFIRMED**: `forAll` does not exist, and `staysClear` still samples at 60 Hz (`check.ts:264`) |
| 34 | The "10 of 12" trial gate has low power | F | **CONFIRMED**, recomputed: P(≥10/12 at a true 75%) = 0.391; P(≥17/24) = 0.766, with a false pass of 3.2% at 50% |
| 35 | `and`/`or` margins mix units; a reward needs a scale and a task-owned spec | O, A, F | **CONFIRMED** in design (`ranged.ts:112–113`) |
| 36 | The global `solves++` leaks a name into WGSL or a hash | O (left unverified) | **REFUTED** for returned curves: after a solve the curve contains no `var` node. Read `solve.ts:578`: the jet names exist only while solving |
| 38 | Correctly rounded `sin`/`exp`/`pow` alone cannot make verdicts bit-exact | F, A, S | **CONFIRMED** in principle: the probe traced an observed difference to multiply–add contraction and f32 stores. The WGSL ulp figures are UNVERIFIED |
| 39 | Equality saturation is unsafe as a cache key; reassociation changes values | F, A, S | **CONFIRMED** by arithmetic: in f32, (2^24+1)−2^24 = 0, while 2^24+(1−2^24) = 1 |

## 3. Ranked defects to act on

Each defect is marked **[code]** (landed on main) or **[design]** (the plan).

**D1 [code]. Ranged checks prove false statements.** These are claims 1–5 above, five separate mechanisms. A model
trained against this reward will find them.
*Fix:*
- Enclose held leaves through the same f32 encoding that `Probe.state` uses.
- Pad `imix` and `imod` outward instead of clipping them to the hull.
- Keep a boolean result beside the robustness: a margin of exactly 0 under `gt`/`lt`/`not` is `false` or `undecided`,
  never `true`.
- Refuse a range end that reads a bound leaf, as a typed error.
- Make non-finite bounds throw `Unbounded("overflow")`.
- Put fable's per-operator fuzz into the suite with zero slack.

*Check:* each of the 9 repro cases gives the right truth or a named refusal. The fuzz passes at least 10⁵ random trees
with zero slack.

**D2 [code]. An undecided check is scored as a failure, and the budget is not a limit.** These are claims 6 and 7.
*Fix:*
- `Verdict` carries `holds: true | false | undecided`, the bounds, the witness and the reason the search stopped,
  through `loft check` and on to the receipt.
- Every enclosure is charged against the budget before it is evaluated.
- The budget is wall-clock milliseconds as well as boxes.

*Check:* with `boxes: N`, no run counts more than N. An exhausted check never appears as a counterexample in any
output. The serpent 3b case with `{ ms: 5000 }` ends `undecided: ms` in at most 5.5 s.

**D3 [design]. The authoring promise stops at the trainer's import boundary.** This is claim 32, which only astra
found. It matters more than any interpreter number. Today the model in training cannot write a field for `march`, a
PBR style or a `solve`, or even take `sqrt`. Position 2 fails in production, whatever the library allows.
*Fix:*
- Generate creative's allowlist from Loft's public exports, minus a named denylist with a reason for each entry.
- Bump the creative pin.
- Run the trial (step 7) through that exact surface.

*Check:* a generated brief scene that uses `march` over its own fold, a custom `Style`, `pbr` and a hand-written
polynomial compiles through creative's confined path on the new pin.

**D4 [design]. Tier I is sized against the wrong unit, the wrong platform and an unreachable target.** This is claims
15–18.
- The probe's own whole-scene numbers set the ceiling. Module and pipeline setup is 5.4 s and 3.3 s cold, but **569 ms
  and 313 ms warm**.
- So on a warm driver cache, tier I can save at most about 0.3–0.6 s per program. Against that it pays interpreter GPU
  time: 16 frames × several stages × 2–6 ms each.
- It never touches the compile child (1–1.5 CPU s).
- This agrees with fable's 1.3× warm against 8× cold. The decisive quantity is the share of pipelines that are cold in
  a real training stream on Vulkan, and nobody has measured it.

*Fix:*
- Step 6 comes before step 5.
- Measure the per-phase CPU split and the steady-state cold-pipeline share over 100 distinct programs.
- Restate the target as "end-to-end samples per GPU-hour and per CPU-second".

*Check:* see build 2 in section 5.

**D5 [code]. `solve` reports false successes and false infeasibility, with no budget and no cache.** These are claims
10–12.
*Fix:*
- Reject any non-finite margin or derivative wherever it is evaluated, naming the hold and the time.
- A failed local search returns `unsolved`, with the fix "start on the other side"; `infeasible` is kept for holds that
  are linear in the unknowns, where it is certified.
- Add `budget: { ms }`, ending in `unsolved: budget`.
- Memoise on a structural hash of (holds, family, prefer, over). The interned graph already gives that hash.

*Check:*
- The NaN case raises `unreadable`.
- `x² ≥ 1` from 0 gives `unsolved`, never `infeasible`.
- The heron's second build in one process spends under 50 ms in `solve`.
- The 83 s far deck under `{ ms: 2000 }` ends `unsolved: budget` within 2.5 s.

**D6 [design+code]. `solve` and the checks do not compose.** This is claims 13 and 14. The plan's central story is that
a hold is "any expression ≥ 0" and the ranged machinery works on it. It fails on the one real example.
*Fix:* `solve` aims `atLeast` at +δ, with a default of 1e-6 in the margin's units, and reports δ. A `HoldLine` can be
handed to `checks` directly. `equal` becomes a band of width 2·TOLERANCE.
*Check:* the heron ships `checks: leap.holds`. All holds are `true` with no `undecided`, and the check CPU is ≤5 s
after D1, D2 and the interval-to-JS compile.

**D7 [design]. Background promotion breaks determinism.** This is claim 19.
*Fix:* the tier is part of a frame's identity and of every answer. In training the tier is fixed per sample and there
is no promotion; promotion is for the game only.
*Check:* the same request sent 100 times in shuffled order, with a server restart in between, gives identical sha256
values and identical `tier` fields.

**D8 [design]. Reward cost has no bound, and the reward spec is missing.** This is claims 12 and 35, plus the measured
91.7 s serpent check, which is 917 times the 0.1 s target.
*Fix:* the plan states:
- who owns the checks: they are task-authored and held out; the model's own checks are advisory;
- that the reward reads `bounds[0]` per atom, normalised by a declared scale;
- that `undecided` is never a negative label;
- the budget for each check.

*Check:* every reward in the receipt schema has a scale, a budget and a three-way result.

**D9 [code+design]. PBR parts hide fixed couplings.** This is claims 20–26.
*Fix:*
- Make the environment weight a slot. By default it integrates the given `distribution` and `visibility` on the twin at
  build time, which is milliseconds.
- Refuse, by name, a sky that reads `time` or a param, or re-project it.
- Let the clear colour be given.
- Type the CPU-sky error.
- Decide whether sky radiance scales with the sun, and test it.
- Expose `visibility` beside the arrival.
- State in §4 that water and ground are overridden by spreading `{ ...pbr(), water, ground }`.
- Tier I interprets the author's parts. Fixed code is an optimisation only for the default parts, matched by hash.

*Check:* with `distribution: () => 0`, the environment specular is 0 in a furnace test. `pbr({ visibility: myVis })`
passes the furnace test within 0.02. A sky that reads `time` is refused by name.

**D10 [code]. Serve's admission boundary is open.** This is claims 27–30.
*Fix:*
- Accept `params` and `inputs` in requests.
- Validate manifest paths and sizes before reading.
- Give each request its own build directory.
- Limit pending requests.
- Put a deadline on the compile child, and answer a child killed by the deadline with a typed `budget` exit, not 1.
- On device loss, answer the in-flight requests before exiting.

*Check:* two requests with the same `out` cannot corrupt each other. A child killed by the deadline is answered as
`budget`. Every in-flight request is answered before exit 6.

**D11 [design]. `recur` is outside both training and checks.** This is claim 31.
*Fix (staged):* now, ranged checks and serve refuse `recur` as a named reason, `undecided: recur`, and the plan
admits it in §7b. Later, the serve process bakes `recur` from data the GPU-less child emits.
*Check:* a `recur` scene through `loft check` returns `undecided` with the reason, not `false`.

**D12 [design]. The arithmetic promise is too strong.** This is claims 38 and 39.
*Fix:*
- Replace 6.5 with a written arithmetic contract (what a proof is about: reals, the f64 twin, or f32 rendering).
- Add an f32-padded enclosure mode.
- Drop equality saturation in favour of the exact structural hash.

*Check:* the f32 mode encloses the GPU readback of the claw and serpent expressions at 10⁴ points.

**D13 [design]. Stale plan text and a wrong march estimate** (claims 17, 33). *Fix:* section 6.

## 4. Real disagreements and my call

1. **One vocabulary or two for "a margin over a window".** Opus would cut `equal`/`atLeast`/`When` in favour of
   `always`+`ge`. Astra and sol would keep them. Fable wants a hold and a ranged condition to be the same value.
   *Call: fable's version.*
   - Keep `equal` and `atLeast`. `equal` buys something the ranged layer refuses (`eq`), and they read well inside
     `hold`.
   - Make a `HoldLine` a ranged condition with its window, accepted by `checks` as it is (D6).
   - Do not add `forAll`.

2. **What a reward does with `undecided`.**
   - Fable: fall back to the sampled check, as a named variant.
   - Astra and sol: three-way result, never a negative label.
   - Opus: `undecided: precision` near zero.

   *Call: a three-way result with reasons, including opus's precision band.* A fallback to the sampled check may be
   reported as a separate field, but must never stand in for the verdict. Doing so would be a silent fallback and
   would reintroduce the tunnelling the ranged checks exist to catch.

3. **The tier-I funding gate.**
   - Opus: Tint plus driver ≥50% of CPU.
   - Fable: ≥60%.
   - Astra and sol: ≥2× completed samples per GPU-hour, end to end.

   They agree more than it looks. Removing a share s perfectly gives at most 1/(1−s), so 2× requires s ≥ 0.5.
   *Call: s ≥ 0.6 on Linux in steady state as the cheap screen,* which leaves headroom for the interpreter's cost, and
   2× end to end on a whole scene as the funding gate.

4. **The trial gate.** Fable: 24 briefs, pass at 17, against three.js and raw shaders only. Astra: keep 10 of 12 per
   comparator, with a fixed protocol. *Call: fable's.* The power numbers are verified (0.39 against 0.77 at a true
   75%). Keep astra's protocol rules: the same model, budget, cameras and revisions on every arm, and ties and failed
   builds decided in advance. Run it on the creative training surface after D3.

5. **Does the trial gate tier I?** Opus says yes, since the Loft interpreter is sunk cost if Loft becomes a library for
   three.js scenes. Sol says it is independent. *Call: opus.* The §8 fail branch makes Loft's own graph the thing at
   stake, and tier I interprets that graph.

6. **Do the interval rule bugs matter?** Fable found ulp-level `mix` misses and called them harmless. Astra built a
   false proof from the same rule. *Call: astra.* Under RL a hole you can reach is a hole that gets used. A bound must
   enclose, with no slack.


## 5. Revised order of work

| # | Step | Gate |
|---|---|---|
| 5a | **Truthful reward path** (D1, D2, D5, D6). CPU only. Also compile the interval evaluator to JS | Build 1 below |
| 5b | **Training surface** (D3): a generated allowlist, the creative pin bumped | A scene that uses march, pbr and solve passes creative's confined compile |
| 6 | **Linux tier-C baseline**, moved ahead of tier I (D4, D7, D10) | Build 2 below |
| 7 | **Blind trial**, starting now: 24 briefs, pass at 17, on the 5b surface | Fail means stop growing the authoring layer and do not build tier I |
| 5c | **Whole-scene tier-I probe** (D4, D9) | Build 3 below |
| 5d | Tier I for real, only if 5c and 7 pass. Batching after the queue cost has been measured. Promotion for the game only | — |
| 8 | One verb, `loft check --at t=…,param=… --explain`, plus the exact structural cache. No `measure`/`why`/`try` trio, no equality saturation | `why` and `try` are answered from `Decision` fields |
| 9 | 6.5 becomes an arithmetic contract plus f32-padded enclosures (moved up, into 5a's spec). 6.3 after budgets exist. 6.4 later. A `recur` bake in serve (D11) | — |

**The next three builds:**

1. **A truthful reward path.**
   - *Pass:*
     - all 9 counterexamples in the repro script give the right truth, `undecided` with a reason, or a typed refusal;
     - the zero-slack operator fuzz passes 10⁵ trees;
     - `loft check` output and the receipts carry `holds`, the bounds and the stop reason;
     - the heron ships `checks: leap.holds`, all true, with check CPU ≤5 s and `solve` ≤2 s;
     - a repeat `solve` costs under 50 ms.
   - *Fail:* any reproduced false proof survives, or a budget-ended check prints as a failure.
2. **Linux tier-C baseline.** Run creative's 10 bench programs, plus 100 distinct generated programs, through a
   confined `loft serve` on the farm's Vulkan GPU, cold and warm, with a restart and shuffled order.
   - *Pass:*
     - every frame is byte-identical across two runs and a restart;
     - every injected host fault (a child killed, malformed input) stays a host fault;
     - a per-phase CPU table is recorded: Bun start, import, graph, lower, Tint, driver, frames.
   - *Gate for 5c:* Tint plus driver ≥60% of per-program CPU in steady state. Below that, build a warm confined
     compile worker instead. Its pass mark: median compile CPU ≤0.5 s, with artefact sha256 equal to a cold build.
3. **Whole-scene tier-I probe on that Linux worker.** Titan, Wyrm, one folded `march` scene, one custom PBR part and one
   raw-WGSL scene that hands off. All stages, 16 frames, the tier recorded, no promotion.
   - *Pass:*
     - ≥2× completed samples per GPU-hour at equal CPU, with compile and reward cost included;
     - every pixel within 1/255 of tier C;
     - ≥90% of the generated graphs run in tier I or hand off with a named reason;
     - the folded march measured against the derived 33–410 ms a frame.
   - *Fail:* below 2×. Then keep tier C, and the cache plus the warm compile worker are the speed levers.

## 6. Proposed edits to `docs/proposals/2026-11-18_next.md` (not applied)

**§3, "Why two tiers", replace the paragraph from "Measured" to "about 0.1 s for a 16-frame sample." with:**

> Measured on macOS/Metal (`agent_notes/2026-11-18_loft_serve.md`, creative `RESOURCES.md`, probe be17739): shader and
> pipeline work was 83% of per-program CPU with a cold Metal cache and at most 41% with a warm one. A whole creature's
> modules and pipelines cost 3.3–5.4 s cold and 0.31–0.57 s warm. The compile child (Bun start, import, graph, lower)
> costs 0.45–0.85 s wall and 1–1.5 CPU s, and tier I does not remove it. Interpreted paint costs 65–123 ps per
> pixel-op (2–5.5 ms per 512² stage). A 128-step march at that price costs 2.2 ms per field op per frame. Nothing
> has been measured on Vulkan. Tier I is funded only if the Linux baseline (step 6) shows shader work at least 60% of
> per-program CPU in steady state, and a whole-scene probe shows at least 2× completed samples per GPU-hour.

**§3, delete:** "`loft serve` may compile a scene it sees often in the background and swap it in (partial evaluation
of the interpreter, done by the driver)." **Replace with:**

> In training the tier is chosen once per sample and recorded. Promotion to tier C happens only in the game.

**§3, "Determinism", replace the tuple with:** "(scene, camera, time, size, params, inputs, tier, library commit,
adapter, driver)".

**§4, replace** "In tier I the BRDF and term computation are fixed code; the style's own graph is interpreted." **with:**

> In tier I every part is graph and is interpreted. Fixed code is only a faster path for the default parts, matched by
> their structural hash.

**§4, replace** "the closed-form sky as an exact environment" **with:**

> the closed-form sky projected once per scene, at its default sun and time 0. A sky that reads time or a param is
> refused by name.

**§4, add:**

> The environment weight is a slot. By default it integrates the given distribution and visibility. Water and ground
> are overridden by spreading the Style record.

**§5, replace the code block with the landed API:**

```ts
const leap = solve({
  over: [0, 1.4], vary: { x: spline(8, roughX), y: spline(8, roughY) },
  hold: ({ x, y }) => [equal(vec2(x, y), [0, 0], { from: 0, to: 0.3 }), equal(vec2(x, y), DECK, { at: 1.2 }), atLeast(clearance(x, y), 0.05)],
  prefer: { jerk: 1 },
});
export const checks = leap.holds;   // the same holds, proved over time
```

**§5, add:**

> `solve` is local. It starts from `spline(n, from)`, which picks the side of each obstacle. A failed search is
> `unsolved`. `infeasible` is reserved for a certified contradiction. Solves take a `budget` and are cached by their
> structural hash.

**§6, replace** "They gain ranged semantics without new names where the meaning is the same:" **and the block with:**

> Ranged checks are `always` and `eventually` over a leaf range (`src/ranged.ts`). The sampled checks
> (`staysClear` and the rest) are unchanged and still sample at 60 Hz.

Then show `always([time, 0, 4], ge(gap, 0.1))` and `always([speed, 4, 6], eventually([time, 0, 2], …))`, and delete
`forAll`.

**§6, result, replace** "`undecided` appears only when a stated budget runs out" **with:**

> `holds` is true, false or undecided. Undecided carries a reason: a budget (in ms or boxes), a refused operator,
> `recur`, or a margin within precision of 0. Undecided is never a failure in a reward. Margins combined by
> `and`/`or` are normalised by a declared scale.

**§6, "Method", delete** "**In progress** on branch `loft/ranged-checks`." **and add:**

> Proofs are about the f64 twin with f32-encoded held values. A range end may not depend on a bound leaf.

**§6.5, replace with:**

> **Arithmetic contract.** State what each proof is about. Add an f32-padded enclosure mode that bounds the GPU's
> value. Correctly rounded transcendentals are an experiment, not the route to bit-exact checks: contraction and f32
> stores also differ.

**§7, replace the last two bullets with:**

> One command, `loft check --at … --explain`: the margin at a setting, the witness, and the graph nodes driving it.
> The cache is the exact structural hash. Equality saturation is not planned.

**§7b, add two rows:**

| Layer | The author's own | Today |
|---|---|---|
| State | `recur` | renders through `loft build` only; checks report `undecided: recur` |
| Training surface | everything public | creative's allowlist admits 37 names; generated from Loft's exports from step 5b |

**§8 table, replace steps 5–9 with the table in section 5 above.**

**§8, "If step 7 fails", add:**

> 24 briefs, pass at 17 against three.js and against raw shaders (false pass 3.2% at parity, power 0.77 at a true
> 75%). Run on the creative training surface. A failure also cancels step 5d.

**§9, add:** "Background promotion in training; equality saturation as a cache key."
