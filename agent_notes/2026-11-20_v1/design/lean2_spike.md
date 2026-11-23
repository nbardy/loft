# lean2 spike: can the redesign land at ≤18,000? (go/no-go)

**Verdict: no-go.** The redesign cannot land at ≤18,000 with any margin unless the feature set or the counting rule changes. Once the code the redesign keeps is counted at its present size, the rest of each subsystem would need to shrink by 72% (compiler) and 83% (hosts). The inventory classifies that code as essential and finds no duplicate path in it. The smallest honest wave-2 target for the present feature set is **≤32,500** (reasoning in §4). Getting under 18,000 needs an owner decision on scope (§5), not a rewrite schedule.

Branch `loft/lean2-spike`, never merged. Counts come from scc 3.7.0 with the `tools/lines.py` rule, run on the lean2 candidate (`loft/lean2` at `77d1b180`, 33,880 counted lines).

## 1. What I priced and how

The review asked me to price a `blobs-1` route and a fixed-tick solver with working code. Every element of both routes already exists as working, tested code in the tree:

| Route element | Working code today | Counted lines |
|---|---|---:|
| Typed program consumed by check/eval/emit/tape | `Ck` (check.rs), `Eval<S>` (eval.rs, already generic over f32/Iv/TraceIv), `Emitter`, tier0 `Lower` | 1,699 + 1,382 + 1,122 + 722 |
| Authored pass → Table/Kernel/Pass plan | `authored_compute.rs` + `bundle::lower_program` | 311 + 782 |
| Compiler-free host reading an old bundle | loft-runner + loft-wgpu + loft-std (no compiler dependency) | 2,159 + 2,597 + 1,871 |
| Reversible Tier 0 edit | loft-tier0 + native `edit.rs` + wgpu `edits.rs` | 1,937 + 335 + 229 |
| Fixed-tick solver, Det trace, raw WGSL twin | `emit/solver.rs`, `motion_source.rs`, std `compute_twin.rs` | 350 + 573 + 747 |

**No new redesign code was written. This is the time-box stop the review allowed for.** I started a typed-IR prototype for the `blobs-1` trace alone and stopped before it compiled. To agree numerically with today's `Eval<f32>`, it has to re-implement map stages (mirror cell, translate, rotate in both directions, point versus direction), rigid transforms, unit-carrying vectors, `.pt`/`.out` frames and per-value Lipschitz tracking. The current `Eval<S>` cannot serve as the shared program either. It runs control flow concretely (an undecided conditional block is E211), and it holds booleans, integers and rows as concrete values. A WGSL or tape instance therefore needs symbolic booleans, rows and control flow first, which means building the redesign's typed program itself. So a single-route prototype is a rewrite of the evaluator, which is the cost the review asked about. I priced it with arithmetic on counted code instead.

## 2. The floor argument (DERIVATION on counted code)

For each subsystem I add up two things: (a) the test, fixture and oracle code that the counting rule keeps in the gate, and (b) the modules whose contract the redesign promises to keep and does not claim to restructure ("What must still work" in lean2.md). Both are taken at today's size. Whatever is left of the ceiling must then hold everything else.

**Compiler** (parse, check, eval, emit, passes: 17,832 today; ceiling 7,520)

| Kept at today's size | Lines |
|---|---:|
| Counted tests/oracles (path_twin, bsdf, cst, plan, sexp, shade_schedule, telemetry, import_table tests) | ≈514 |
| AD/JVP `ad.rs` (the redesign keeps explicit derivative rules) | 512 |
| Interval proofs `lip.rs` + `refine.rs` | 854 |
| CPU BVH references (eval bvh + arena, passes bvh/twin) | 616 |
| Lossless CST (excluding its tests) | 307 |
| Packed formats `quant` ×4, provenance ×2, kind cache, bake textures | 673 |
| Plan asset evaluation (f64 binding for loft-plan) | ≈200 |
| **Floor** | **≈3,676** |

That leaves **3,844** lines for the parser productions (1,830 today, grammar unchanged), the AST (524), elaboration/linking/checking (≈4,484), the evaluator core (1,382), emission (≈2,191) and pass/bundle lowering (≈3,395). Those total **13,806** today, so the required cut is **72%**. The AST is not counted in the floor, which favours the redesign.

**Hosts** (cli, native, runner, std, tier0, wgpu: 12,822 today; ceiling 7,170)

| Kept at today's size | Lines |
|---|---:|
| Counted fixtures/examples/tests (CLI examples 697, CLI test modules ≈317, runner 818, tier0 diagnostic snapshots 325, wgpu/native/std test modules ≈120) | ≈2,280 |
| Independent CPU execution of raw kernels `compute_twin.rs` | 747 |
| Frozen Tier 0 VM `tier0.rs` (tape semantics and certification) | 448 |
| Device negotiation `ray_query.rs`, browser projection `web.rs`, Three.js handles `interop.rs` | 780 |
| CLI command surface with distinct exit/publication contracts (main, clip, render, profile, diff, edit, context, fork, lint, budget, lib, minus tests) | ≈1,728 |
| **Floor** | **≈5,983** |

That leaves **≈1,190** lines for native loading, recording, timing, promotion, shadow and motion sampling, runner encoding, initialization and stepping, std stepping, promotion, bundle, inputs, params, mips and picking, every wgpu resource module, and Tier 0 lowering plus the edit session. Those total **≈6,839** today, so the required cut is **83%**. Wave 1 (v15-exec) already merged the host projections into one execution path, so the inventory finds no second host path left to delete.

**Frozen** (abi, builtins, format, gltf, ir, plan, shared): 3,226, unchanged by design.

## 3. Why the remaining cut is not credible

- **No duplicate to remove.** The inventory scores 29,369 lines E, 646 D and 1,073 R. Deleting every D and R line still leaves 32,162. Reaching 18,000 therefore means rewriting essential code at 3.6× (compiler) to 5.7× (hosts) its present density with the same contracts. Lean1 and lean2 contain no example of that.
- **Predictions have run high.** Lean2's three approved cuts were forecast to save 122 lines and saved 87 (cut 8: 32 of 41; cut 7: 44 of 37; cut 1: 11 of 44). Lean1 missed at the checked-call and executor boundary. That is the same boundary the 72% compiler cut depends on.
- **The shared-domain idea is real but small.** One typed program with f32, interval, dual and WGSL/tape interpretations would remove the four parallel builtin dispatchers: `Ck::call` ≈290, `Emitter::expression` call arms ≈115, `Eval::builtin` ≈205 and the tier0 `Lower` call arms. Removing the symbolic AD pass in favour of a dual domain could also work if emission becomes a domain. Each of these is worth hundreds of lines. The ceiling needs 10,312 from the compiler.

## 4. Smallest honest alternative target: ≤32,500

Starting from the lean2 total of 33,880:

| Step | Est. counted saving | Basis |
|---|---:|---|
| Deferred cuts 2, 4, 5, 6 (land after the perf branches) | ≈320 | 448 forecast × 0.71, the realisation rate lean2 measured |
| Finish cut 2 into one catalog-driven call dispatch shared by check/eval/emit/tape | ≈700–1,000 | The four builtin dispatchers above, minus a shared operand table |
| Move remaining stock Rust recipes (R rows, 1,073 gross) into stdlib | ≈450 | Lean2 priced Rust replacements at ~55% of gross for cuts 4–6 |
| **Projected** | **≈31,900–32,400** | |

A target of **≤32,500** keeps about 1.5% margin against the projection, given lean2's measured 29% shortfall on its own savings forecast. It keeps every feature, every independent witness and the present counting rule. It is the useful remainder of the redesign: one typed call representation and authored pass recipes, without promising the host or parser rewrites.

## 5. What would actually reach 18,000 (owner decisions, not engineering)

These are choices to make or decline. I have not started any of them.

1. **Change the counting rule** so that test, fixture, example and oracle code (about 2,800 counted lines inside `src/` and `examples/`) moves to `tests/` or `tools/`, where the gate already excludes it. This is honest only if G5.1 is restated as measuring production code.
2. **Retire host or witness features.** Candidates include the CPU raw-kernel twin (747), browser projection (288 + wasm paths), Three.js interop (309), Tier 0 live edit (≈2,500 across crates) and CLI commands that duplicate tools (profile, diff, clip ≈760). Each is a contract that has a caller today.
3. **Redefine the 18,000 as a compiler-plus-runtime figure** that excludes the CLI and Tier 0. That would put today's tree at ≈29,200.

Even all three together put the tree near 24,000–26,000 before any redesign. G5.1 at 18,000 needs the redesign as well, at the densities §2 shows to be uncredible. My recommendation is to restate G5.1 as ≤32,500 at the present scope, keep the 18,000 figure only alongside an explicit scope cut the owner chooses, and record the 23 Nov finding in V1_5_GOALS.

## Claim boundary

The floors are DERIVATION on scc-counted files. The per-module test-line splits marked ≈ come from the lean2 inventory ranges and were not re-sliced. The alternative-target savings are CONJECTURE, scaled by lean2's measured realisation rate. No caller was lost, because no code changed on this branch.
