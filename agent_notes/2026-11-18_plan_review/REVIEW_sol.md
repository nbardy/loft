# Loft next-plan review — sol, 18 November 2026

## Verdict

**CHANGE, then fund the narrowed plan.** Keep Loft, its graph, programmable motion and swappable styles.  
The paint interpreter earns a whole-scene experiment, not a production-tier commitment.  
Fix false proofs and false solver successes before making these results training rewards.  
Move Linux validation and the authoring trial forward; defer promotion and semantic canonicalization.  
Fund measured end-to-end training throughput, with explicit correctness and resource gates.

Scope: reviewed main `52c8d987510f16098ac7feeb8a0dfacf9cca7241`, the proposal, the named implementations and notes, relevant compiler/host/check code, and the interpreter branch's report. I ran in-memory CPU reproductions below. GPU timings are reported measurements from the supplied notes, not measurements I repeated. Linux behavior remains unverified. No implementation files were edited; I did not run the full suite, which would create repository outputs.

## Defects, ranked

Ranked within the two groups below; the reproduced correctness failures take priority over expansion.

### Landed code defects

**1. Ranged checks can prove a condition the actual CPU twin fails.** `src/ranged.ts:222–230` supplies unrounded input defaults/overrides and parameter defaults to the interval evaluator. `src/check.ts:42–54` instead stores host inputs and parameters in `Float32Array`. `src/interval.ts:611–613` treats held values as exact points. This is a boundary mismatch before any difficult transcendental math.

Reproduced with an input registered in a scene, default `16777217`, and `e = sub(input, 16777216)`: `Probe.scalar(e)` returned **0**, while `always([time, 0, 1], ge(e, 0.5))` returned **true**, margin **0.5**, bounds **[0.5, 0.5]**. The input becomes `16777216` in the twin's frame; the enclosure retains `16777217`. Small rounding differences can likewise change a near-zero margin at ordinary scales.

**Fix:** classify and encode held leaves through the same host-value boundary as `Probe.state`, then enclose those encoded values. Add this counterexample as a regression. Separately specify whether proofs concern real arithmetic, the point twin, or rendered arithmetic; the current result has no field identifying that distinction.

**2. `solve` can return success for an undefined constraint.** `src/geom/solve.ts:481–483,695,729–739,751–762`. NaN poisons violation aggregation; `v > MET` is then false. In the final peak search, NaN samples fail the comparisons and disappear. Remaining finite samples can produce `worst: 0`.

Reproduced with one `spline(4)` unknown over `[0,1]`, jerk preference, and:

```ts
equal(select(gt(time, 0.5),
  log(neg(add(mul(c.x, c.x), 1))), 0), 0)
```

The solver returned in one round with reported worst violation **0**. Evaluating the returned hold at **0.75** returned **NaN**. The logarithm is undefined throughout the second half, for every real curve. The initial readability probe at the first sample (`solve.ts:674–688`) does not inspect that branch.

**Fix:** reject nonfinite margins and derivatives whenever evaluated, with hold site and time. A failed numerical evaluation must never enter optimization or certification as an ordinary number. Check every final sampled value before finding extrema. Sampling still cannot establish domain validity between samples; report that limitation separately.

**3. Local search failure is falsely presented as infeasibility.** `src/geom/solve.ts:153–156,189–199,775–792`. Reproduced:

```ts
solve({ over: [0,1], vary: { x: spline(4) },
  hold: c => [atLeast(mul(c.x, c.x), 1)],
  prefer: { jerk: 1 } })
```

It reports “holds that cannot all be met.” Constant `x = 1` is feasible and has zero jerk. Zero is a stationary starting point; a deletion filter using the same local search cannot turn that failure into a proof. The comments acknowledge locality, but the public error and type promise an impossible family and a minimal conflicting set.

**Fix:** return `unsolved` for uncertified search failures, with work and residuals. Reserve `infeasible` for an actual certificate, including an explicitly supported linear case. Describe the deletion result as suspected conflicting holds unless independently established. Add a deterministic work budget covering derivatives, exchange rounds and conflict searches; the note records an **83 s** solve, and there is no caller-controlled total budget.

**4. Proof uncertainty becomes scene failure in the normal check path.** `src/ranged.ts:147–155` maps `undecided` to `pass: false`; `src/cli/check.ts:37–39,83` counts it as failure and exits through `ChecksFailed`. Bounds, budget and the structured status survive only in `decide`, not the standard `Verdict` (`src/check.ts:32`). A training consumer using the normal result can penalize an expensive proof as though it found a collision.

**Fix:** carry a structured three-way result, bounds and stop reason through checks, CLI output and training receipts. Fail-closed CLI behavior may remain an explicit policy, but uncertainty must remain distinguishable from a disproved scene. Test that exhausting the proof budget never manufactures a negative example.

**5. Boolean truth is changed at zero robustness.** `src/ranged.ts:110–114,211` identifies strict and non-strict comparisons and negates robustness without preserving boundary truth. I reproduced **true** for both `always([time,0,1], gt(0,0))` and `always([time,0,1], not(ge(0,0)))`. Both predicates are false. This is documented in part in the ranged note, but incompatible with accepting the existing comparison vocabulary as exact predicates.

**Fix:** either preserve strictness/negation truth alongside the robustness value, or refuse unsupported boundary semantics explicitly. Do not silently redefine `gt` as `ge`. Add boundary cases for comparison and negation, including constant margins and contact.

**6. PBR part replacement leaves hidden default-lobe behavior.** `src/style/pbr.ts:36–59,101–104`: environment weighting remains fitted to default GGX/Smith after either part is replaced. Setting the distribution to zero removes direct specular but leaves environment specular. A single representative cosine is exact for the fitted Schlick decomposition, not for an arbitrary angle-dependent thin-film law. This is verified from the dependency structure, not a new GPU measurement. The note acknowledges the limitation, but “swap one part” needs to expose this dependency.

**Fix:** make the environment response/weight replaceable as a coherent operation, with a reference numerical integral to assess substitutions. Preserve plain `Style` as the unrestricted composition route; do not require authors to rebuild the entire shade function to correct one hidden fitted dependency.

**7. `serve` has a process split, but no complete bounded admission boundary.** `src/cli/serve.ts:195–197` calls `readBundle` before `--check`; `src/host/files.ts:53–61` trusts manifest structure and reads named files before the hook can apply quotas. `serve.ts:275–280` accepts unlimited pending requests despite limiting active compiles. The compile child has no internal deadline (`129–143`). These are concrete exhaustion paths. An environment flag prevents honest Loft GPU use, not hostile direct GPU access (`src/host/headless.ts:7–10`).

**Fix:** validate structure, contained paths and sizes before bulk reads; separate configurable limits from basic parsing correctness. Bound pending work and output bytes. Supply an operational supervisor contract for compile/draw deadlines, device loss and restart. Keep OS confinement mandatory in the training deployment and test it. The hooks are useful policy insertion points, not evidence those policies have run. Linux isolation and failure recovery were not verified here or in the serve note.

### Design defects

**8. “Exact rewards” promises more than the numerical architecture establishes.** Proposal §§1,3,6,6.5; `src/interval.ts:15–18`; `tests/ranged.test.ts:26–29`. The enclosure explicitly excludes GPU f32 and f32-folded subgraphs; the test permits **1e-4** slack. That can validate a useful approximation while missing an incorrect sign. Correctly rounded `sin`, `exp` and `pow` alone cannot make the twin and GPU bit-exact: basic arithmetic precision, contraction, input encoding, reductions, folding and intermediate stores also matter. The interpreter probe itself observes amplification of differing intermediate rounding.

**Fix:** publish the arithmetic contract now. Distinguish real-expression certificates from rendered guarantees. For the latter, propagate conservative numerical error, or use a consistently specified execution arithmetic. Define behavior at seams and undefined operations. Keep a bit-exact backend as a separately costed experiment; stop claiming that replacing three math functions completes it.

**9. Step 5 bundles several unproven projects behind a paint-only gate.** Proposal §3 and step 5. The probe prices two fragment graphs at known coverage, excluding contact search and all pose/vertex work. Its 10 ms threshold establishes an acceptable *stage overhead*. It does not establish a fast scene, let alone batch throughput. The report explicitly says a warm-cache four-frame paint-only switch loses time. The proposal acknowledges the missing stages but nevertheless approves “Tier I for real,” batching and background promotion together.

**Fix:** gate a complete scene first. Include import, graph construction, lowering, upload, pose, geometry, shadows, water/contact, post, readback, checks and solving where applicable. Record fallback frequency on generated scenes. Separate batching from basic coverage, and defer background promotion until observed reuse repays compilation without harming sample latency. Concatenating tapes with offsets does not by itself eliminate GPU work or solve heterogeneous draw scheduling.

**10. The proposed fixed BRDF conflicts with interchangeable arbitrary parts.** Proposal §4 says tier I fixes BRDF/term computation while interpreting the style graph. That works only for recognized built-in terms. An author's distribution, Fresnel, light response or environment is code too; it cannot silently inherit fixed lighting. In addition, `pbr_parts.ts:191–194` acknowledges a frozen, approximate sky projection; the proposal's “exact environment” wording is misleading.

**Fix:** interpret author code wherever it appears, or give a named compiled fallback. State approximation limits and require a custom lighting-part case in the tier-I gate.

**11. Semantic canonicalization is an unjustified speed dependency.** Proposal §7/step 8 claims a content hash “by meaning,” program deduplication and detection of no visible change. Equality saturation supplies selected rewrite equivalences, not a universal semantic identity. Floating-point reassociation alone changes results: in f32, `(16777216 + 1) - 16777216` is **0**, while `16777216 + (1 - 16777216)` is **1**. Expression equality also does not identify a scene's resources, passes or observable picture.

**Fix:** first cache exact serialized programs/resources with versioned arithmetic, compiler, library and execution keys. Measure actual training hit rates and memory. Add only proven rewrites under explicit arithmetic assumptions. Treat visual equivalence as a separate, view/time-dependent question. If the interpreter fails, do not assume a semantic cache rescues throughput without those measurements.

## Against the five binding positions

1. **Programmable motion: substantially honored.** Equations and keys remain independent; solved curves are ordinary expressions. The replacement of `planted/lands/clear` wrappers with general `equal/atLeast` constraints is sensible subtraction. However, the solver handles only cubic spline unknowns, own-time reads and default params/inputs. Those are solver limits, not limits that should spread into motion authoring. The heron starts from an explicit rough arc; do not sell it as discovering the full motion from constraints alone.

2. **Arbitrary computation: honored as an escape route, unproven as the fast path.** `iterate`, custom fields, `march`, tracing and raw WGSL remain. Named fallback preserves expressiveness. It does not preserve training economics: if difficult or inventive programs routinely fall back, the reward budget favors the interpreter's vocabulary. Track fallback reasons and cost by feature. Keep generic algorithms and folded marching in the acceptance corpus.

3. **Author-owned styles: mostly honored in landed structure.** Plain expression functions and `Style` overrides are good. Fixed tier-I lighting and hidden environment fits weaken the promise. Separated lighting terms remain proposed work: `ShadeCtx` currently exposes a light fold, channels and context, not the proposal's full ready-made term set (`src/style.ts:13–50`). Mark that gap explicitly.

4. **Much faster, reliable, deterministic training: not established.** Paint setup is cheap; end-to-end samples are not yet. Solver/proof CPU costs can dominate rendering. The serve order-independence evidence is useful, but cannot establish every graph, driver or raw-WGSL program as byte-identical. Record execution tier, backend versions and numerical contract in receipts; test repeated frames across request orders and fresh processes on Linux.

5. **Names must buy capability: generally honored, with unfinished interfaces.** `solve` and ranged quantifiers buy clear capabilities. `measure/why/try` should be tested as one structured query/result interaction before adding three overlapping CLI interfaces. Ranged checks are still absent from the public entry/context, quantified conjunction/disjunction is missing, and the proposal's upgraded `staysClear` semantics have not landed. Do not present the illustrative API as the shipped API.

## Missing

- **Solve-to-proof integration.** Heron `design.build` returns `checks: []` (`examples/second/heron_leap.ts:132`). Returned holds need reusable tolerance-aware ranged certification and witness-driven exchange. Certification must also distinguish clearance of sampled stations from clearance of the entire skin; the heron checks five torso/neck stations, not continuous surface coverage (`62–69`).
- **A realistic reward budget.** Measure sign-only proof separately from a precision-polished dense reward. The search continues refining extrema after a sign can be known. Specify normalized margin units, uncertainty policy and scheduling so certificate cost cannot consume the sample budget unnoticed.
- **A complete observation identity.** Raw WGSL can read host-supplied data and arbitrary scenes may use external resources. Define asset hashes, seeded randomness and allowed host dependencies, rather than assuming `(scene, camera, time, size, commit, adapter)` captures every input.
- **A fair authoring trial protocol.** Fix briefs, model/tool budgets, seeds, repair opportunities and blind criteria before running. Score usable motion, geometry correctness, visual quality, time, tokens and compute separately. Include custom styles, equations, folded rays and input-driven behavior. The three.js fallback also needs an explicit expression/rig adapter; the twin cannot inspect arbitrary three.js JavaScript for free.

## Order of work: change steps 5–9

Start the authoring trial's protocol and baseline collection now. Move the Linux vertical slice ahead of full tier-I engineering. Move structured proof results and basic query tooling ahead of canonicalization. Keep player-input quantification and picture facts later; move the arithmetic specification out of step 9 into present correctness work.

The next three builds should be:

1. **A truthful numerical result path.** Fix defects 1–5; add finite-value rejection, structured uncertainty and deterministic solver/proof budgets. Wire at least one solved hold through ranged certification. **Pass:** all reproduced counterexamples give correct truth or explicit uncertainty/error; a feasible stationary-start example is never certified infeasible; certified solved margins meet stated tolerance. **Fail:** any undefined hold succeeds, any budget exhaustion becomes a scene counterexample, or any reported proof contradicts its declared execution semantics.

2. **A supervised Linux training slice using compiled scenes first.** Carry one actual training configuration through confinement, build, validation, serve, reward and persisted receipts. Exercise malformed artefacts, compile timeout and server death, then restart. **Pass:** valid samples complete, repeated/reordered frames agree on the pinned stack, every injected host failure remains a host failure, and queues/resources remain bounded. Record cold/warm CPU, GPU, wall p50/p95 and samples/min. **Fail:** a killed worker yields scene reward zero or another request cannot recover.

3. **A complete tier-I scene experiment on that stack.** Start with one representative creature, then the whirlpool with actual contact, a folded march, a custom PBR replacement and a raw-WGSL fallback. Preserve a compiled reference. **Pass:** every case runs or hands off with a reason; finite outputs and predefined tolerances hold; representative training throughput improves at least **2×** under equal CPU/GPU limits, including frontend and reward costs. This is my proposed funding gate, not a measured result. Publish achieved latency instead of retaining 0.1 s by assumption. **Fail:** speed comes from omitted stages, restricted author code or shifted host failures.

Only after that add batch scheduling. Add promotion only with measured reuse; add canonical rewrites only with measured cache value. Let the blind trial decide whether to expand the authoring layer, independently of backend engineering success.

## Numbers: recomputed, measured versus derived

These calculations use the supplied measurement reports; I did not rerun their GPU benchmarks.

| Claim | Recalculation and implication |
|---|---|
| Shader compilation is about 80% of CPU | Creative `RESOURCES.md`: `(5.96 + 10.5) / (9.4 + 10.5) = 82.7%`. **Derived from older measured phases.** Perfectly removing only that work caps that workload's CPU speedup at `19.9 / 3.44 = 5.78×`; it does not imply the same wall speedup on current main or Vulkan. |
| Paint-only four-frame saving | Probe: crab cold `34.42 − 0.413 − 4×1.999 = 26.01 ms`; water cold `51.52 − 0.569 − 4×5.493 = 28.98 ms`. Warm: **−6.71 / −17.97 ms**. **Derived**, resident interpreter assumed. Whole-scene compilation savings cannot be credited to replacing paint alone. |
| A 16-frame sample is about 0.1 s | Water paint alone: best `16×5.571 = 89.14 ms`; reported median `16×6.160 = 98.56 ms`. **Derived**, no frontend, pose, contact, geometry or readback. The proposal's estimated 10–30 ms folded-march frames imply **160–480 ms** for sixteen frames before other costs. |
| Rewards remain cheap | Ranged note reports **91.7 s** for the two-leaf serpent proof: **917×** the proposed 0.1 s sample. Solve note reports **5.0 s** for the heron: **50×**. **Reported measurements, ratios derived**; they are different scenes, not an additive benchmark. They establish why rendering alone is the wrong budget. |

The inherited techniques are useful: interval/affine arithmetic, branch-and-bound, augmented Lagrangians, GGX/Smith/Schlick and bytecode interpretation. Loft's combination and affine moving witnesses deserve testing. The next investment should establish that those components compose into truthful, bounded, faster training—not just separately successful demonstrations.
