# Lean 2 — the cost of keeping the Loft v1 contracts

**Lead decision requested. The eight cuts below take the counted tree from 33,881 to a predicted 33,289 Rust lines. That misses G5.1 by 15,289. Stop at design review.** The last column of the crate ledger prices a different architecture at a maximum of 17,916. Its three subsystem ceilings need feasibility evidence before implementation. The 14,500–15,500 wave requires a further 2,416–3,416 lines even at that redesign ceiling.

The design baseline is `b765551`, crates tree `306d8ee65fbac069b95d5c2c4b318e47d0d0a814`. During the audit main advanced to `7a7cc2f` with the same crates tree and six changed Loft library sources. All predictions below use the pinned baseline. The eventual picture and timing control must include the incoming library sources and accepted perf changes in both arms.

## What the gate measures

I ran `tools/check.sh lines` in the clean Cambium checkout. scc 3.7.0 measured 33,881 / 18,000, runner 2,155 / 2,200, CLI native 0 / 450 and zero wgpu dependencies for loft-std across all targets and features. Exit 1 is the expected result. A second file inventory with the tests included reproduces 39,095. The difference is **5,214 code lines in 33 files under directories named `tests`**. The gate includes 165 files, including all examples, private test modules, and shader strings embedded in Rust. The gate excludes standalone WGSL, authored Loft, tools and their implementation files. For example, the CLI gate-capture example charges its two-line entry, while the included capture implementation lives in tools. Every candidate below preserves that counting rule. [Counter](../../../tools/lines.py) · [Prior integration scoreboard](../SPRINT_TO_TRAIN.md)

Lean1a shared parser, solver and plan logic but missed its ceilings and left checked calls open. Lean1b, c and d provided preflights. The ilean audit admitted none of the four. Subsequent v15-exec work delivered the shared executor and wgpu-free std. Wave 16 then split compiler crates, installed the builtin catalog, extracted WGSL and the verifier, and reduced runner glue. Those savings are already in the baseline. In particular `stdlib/materials/bsdf.loft` already owns the GGX lobe and `stdlib/engines/reference.loft` already owns the bounce loop. There is no second CLI renderer to remove. [Wave 1 theory](lean1.md) · [Integration history](../OPEN.md) · [Ownership ruling R2](../../../docs/V1_5_GOALS.md)

## One source-to-frame trace

A user can fork the current realtime schedule and change its lighting while keeping their own material. This existing line selects the callback and supplies the sun direction.

```loft
shade_tiles(Vis64, light: sun_sky, sun: [0.5, 0.75, 0.35])
```

`Graph::parsed` links the selected modules. `parse_all` elaborates, infers and runs AD. `check_modules` records types by expression ID. `schedule::resolve` interprets the pass name, validates resources and records a `Pass::Shade` with kind IDs. `lower_passes::lower` binds the callback to kind shaders. `bundle::lower_program` visits `Pass::Shade` again to choose concrete tables, slots and dispatches. Finally the runner validates a possibly external bundle, links device resources and records commands. The CPU evaluator also executes authored functions when a query is sampled independently. Binding the callback once can remove its repeated selection. The specialized GPU arithmetic, independent CPU observation and load-time checks on externally supplied bytes each have separate callers. [Realtime recipe](../../../stdlib/engines/realtime.loft) · [Schedule resolver](../../../crates/loft-passes/src/pipeline/schedule.rs) · [Bundle lowering](../../../crates/loft-passes/src/bundle.rs) · [Device linker](../../../crates/loft-wgpu/src/schedule.rs)

## Crate ledger

“All Rust” includes the external test directories. “Gate now” reproduces the ratchet. “Gross cut” measures disjoint scc slices of the original functions. “Rust back” budgets the entire replacement, including new fields, dispatch, diagnostics and glue in receiving crates. “Forecast” applies those replacements once each. The ledger assumes the original features, public entry points, oracle coverage and current picture approvals.

| Crate | All Rust | Gate now | Gross cut | Rust back | Forecast | Redesign ceiling¹ |
|---|---:|---:|---:|---:|---:|---:|
| loft-abi | 563 | 372 | 0 | 0 | 372 | 372 |
| loft-builtins | 340 | 293 | 0 | 0 | 293 | 293 |
| loft-check | 5,096 | 5,096 | 672 | 630 | 5,054 | 2,050 |
| loft-cli | 4,736 | 2,777 | 34 | 9 | 2,752 | 1,650 |
| loft-emit | 2,400 | 2,400 | 514 | 275 | 2,161 | 900 |
| loft-eval | 3,391 | 3,391 | 75 | 40 | 3,356 | 1,570 |
| loft-format | 767 | 690 | 0 | 0 | 690 | 690 |
| loft-gltf | 375 | 375 | 0 | 0 | 375 | 375 |
| loft-ir | 680 | 627 | 0 | 15 | 642 | 627 |
| loft-native | 1,438 | 1,438 | 0 | 0 | 1,438 | 620 |
| loft-parse | 2,718 | 2,680 | 0 | 80 | 2,760 | 1,450 |
| loft-passes | 6,243 | 4,273 | 548 | 302 | 4,027 | 1,550 |
| loft-plan | 1,095 | 859 | 0 | 0 | 859 | 859 |
| loft-runner | 2,155 | 2,155 | 0 | 0 | 2,155 | 1,250 |
| loft-std | 1,986 | 1,871 | 0 | 0 | 1,871 | 1,200 |
| loft-tier0 | 2,347 | 1,986 | 161 | 61 | 1,886 | 1,050 |
| loft-wgpu | 2,755 | 2,588 | 0 | 0 | 2,588 | 1,400 |
| shared | 10 | 10 | 0 | 0 | 10 | 10 |
| **Total** | **39,095** | **33,881** | **2,004** | **1,412** | **33,289** | **17,916** |

¹ **Redesign ceiling** is the break-even engineering budget detailed below. The eight-cut forecast and the redesign ceiling are separate alternatives. At the present architecture, deleting the *entire* checker, evaluator, emitter and passes crates would still leave 18,721 lines. That calculation also removes the compiler itself. Moving tests or shader text out of the counter cannot establish a working 18k design.

## Proposed commits and costs

Each numbered row denotes one commit, with the associated source ranges in the inventory. Gross counts are measured. Replacement and authored-source counts are estimates in code lines. The other-source column prices handwritten grammar changes. The plan retains the existing Rust and oracle tests in their original locations. An additional test needed to falsify a new behavior must be charged to the appropriate ledger before scheduling the cut.

| Cut | Removed Rust | Added Rust by crate | Added .loft | Other source | Net counted saving | Primary risk and discriminating check |
|---|---:|---|---:|---:|---:|---|
| 1. Keep the bound motion and plan operands | 163 | check 96, passes 23 | 0 | 0 | 44 | Validation order for open plans and unscheduled solvers. Compare diagnostics, packed prior headers, state and verdict words, and motion Twin. |
| 2. Carry checked call targets to consumers | 536 | check 324, emit 76, eval 28, tier0 51, passes 14 | 0 | 0 | 43 | Shadowed builtin or stale expression ID after AD or editing. Compare diagnostic snapshots, baked texture/scan samples, Tier 0 refusals and promotion, Twin, and Blobs/Capsules Load. |
| 3. Use compiled edit inputs and one shader prelude | 44 | passes 16, tier0 6 | 0 | 0 | 22 | A mesh or World prefix shifts scan offsets. Compare edited/native shader bindings, pixels, pipeline-creation counters, and hot-edit paired timings. |
| 4. Author material composition and energy interpolation | 145 | check 15, emit 32, eval 12, passes 8 | 84 | 0 | 78 | Layer order, emitted radiance and GGX lookup rounding. Run furnace/media oracles, both picture tiers, Hermit-1/20 and matched heavy lighting. |
| 5. Share fixed-tick lowering and expose the XPBD sweep | 129 | emit 41, passes 11 | 86 | 0 | 77 | Tick origin, substeps, sweep order, zero distance and FMA. Compare 120 Hz pose/state/event bytes through checkpoint replay, scoped MSL contraction check and the 1,000-crab dispatch. |
| 6. Author the raster pass declarations | 745 | parse 55, check 69, passes 230, emit 126, ir 15 | 462 | 24 grammar | 250 | Binding range, resource lifetime, tie order, stale shadow or a new pipeline. Compare pass declarations and decoded-bundle replay, all gated scenes, Shrine and matched A/B, plus precision and browser witnesses. |
| 7. Record geometry dependencies on the existing reference walk | 163 | check 126 | 0 | 0 | 37 | Projecting `self.x` as the whole row changes shape counts and cache keys. Check identical geometry field sets, derived bounds, cache hits and Capsules-20/Blobs-20 cold loads. |
| 8. Keep declaration body spans from parsing | 79 | parse 25, cli 9, tier0 4 | 0 | 0 | 41 | A nested query, an expression body, a law writes set or trailing trivia shifts the edit slice. Compare verbatim CST replay, source edit classifications and the bytes printed by `loft context`. |
| **Total** | **2,004** | **1,412** | **632** | **24** | **592** | Across the priced handwritten sources this adds 64 net lines. The payoff is author control and removal of repeated decisions. |

**1.** `motion_source::solver` checks config fields and `scheduled` interprets them again. Keep the parsed strategy/count/defaults in the checked solver, retaining the distinct scheduled capability limit and its diagnostic stage. `check::check` and `motion_module` both bind names in `run_laws`. Keep their table, ownership, order and tier on the checked schedule. `plan_source::check_source` and `lower_source` both flatten objective terms and bind holds. Store those operands with spans by declaration ID. Evaluate defaults and assets only when the plan closes. A template with no bound prior still yields the existing absent result. Preserve all f64 optimization and failure verdicts in loft-plan.

**2.** Extend the existing expression-ID side table with the resolved operand of each call. For example, a baked texture plus optional LOD carries the resolved texture declaration ID and coordinate expression, a user function carries its declaration ID, and a builtin carries its catalog ID. `Ck::call` owns overload resolution and diagnostics. Emit, scalar evaluation and tape lowering dispatch on that value with their own implementations. Keep lazy selection, AD-generated scans, packed atomics, and source-shadowing precedence. Resolve generated expressions after `infer::run` and `ad::run`. An edited module gets its own binding generation. Inference still has incomplete types and the tape still requires a verified intrinsic certificate. The checker’s independent `references::motion_math` name list can consume the existing catalog's `MotionClass`; preserve the current treatment of user calls and missing names.

**3.** `stage0::pack_luts` knows the asset prefix, optional twelve-word World header and scan offsets. Keep the resulting `LutRef`s and selected light binding with the compilation used by the edit session. Delete the offset and shade-pass reconstruction in `tier0/session.rs`. Use one prelude builder for `shader::naga_module` and `session::device_shader`; the ordinary compile retains provenance, while direct edit submission strips it. Preserve target features per path and test the admitted edit subset. A newly detected old offset discrepancy needs its own behavior decision before claiming parity.

**4.** The remaining renderer decisions include the bottom-to-top color/transmittance/emission fold, the two child stacks of `mix_bsdf`, the horizon guard and GGX energy interpolation. `Layout::eval`, `emit_kind_module` and `Eval::eval_layers` currently spell the fold independently. Put the complete layered material calculation and GGX lookup math in the material library. Bind the table resource or constant-table variant at specialization and inline the author's body for the static lobe sequence. Keep the 32 interpolation intervals, square-root view coordinate and 33 roughness rows of 34 words each, clamped endpoints, each backend's numerical tolerance, static layout checks, and the custom-lobe path. The separate analytic furnace and medium references remain independent. Verify the generated straight-line shade path has no interpreter and no extra pipeline.

**5.** `motion_module` and `emit::solver::lower` each emit tick bounds, row window and clock setup. The compiler can specialize one advance skeleton using the existing ABI and distinct row offsets, workgroup sizes and load/store contracts. Author the complete XPBD iteration in `stdlib/motion`, including the forward/reverse or color traversal, persistent lambda for each edge during the substep and deterministic fallback axis. Specialize the authored loop against each checked constraint at compile time. Preserve the single workgroup invocation of the solver and the row-law workgroup of 64. The 120 Hz ABI, access rights, substep ownership, proof of deterministic operations and sparse verdict writes stay in the compiler. Use an authored deterministic block where explicit FMA is necessary.

**6.** Extend the existing stage-0 schedule body so a library can instantiate a render pass with explicit attachments, indirect draw, entry point, logical buffer range and resource effects, as it can already do for an authored compute pass. Each instance produces the existing Table/Kernel/Pass bundle declarations. A raster recipe can select its camera and display transform, expand per-kind visibility and shading, declare shadow-only casters, and select the World callback. Express render-target size and author-selected features symbolically and resolve them when lowering the bundle for a device. Keep format/role precision, binding reflection, initialization and E505 ordering in the linker. Map stock v1 verbs to these source recipes with the old spans and defaults. A user's copied schedule, complete raw compute pass or per-kind material must still override the stock implementation. The 462 Loft lines price the recipes including full-screen sample accumulation, deterministic order and presentation. Budget 24 lines in the canonical grammar; the second path is a symlink. Count any additional adapter or generated input when the grammar is specified.

**7.** `Prog::geometry_fields` implements its own recursive traversal of expressions and statements, beside `References::{expr,statements}`. Have the latter record direct row-member reads and whole-row escape. Keep assignment left sides, emit statements and lobe/material exclusions consistent with the current geometry key. Reference discovery and proof-only traversal still distinguish opaque `unproved` bodies from their step scales. Preserve mutating scope resolution in `source/scope.rs`, which also rewrites names, types and declaration binders.

**8.** Both `cli::context::body_index` and `tier0::session::body_range` discover an author's declaration body from its name and delimiter tokens. Record start and end offsets at parse time in a side table for source-authored items, keyed by their original name span. Read the individual start, end and header ranges in the CLI and edit session, including a nested kind query. Synthetic AD functions have no editable source range. Keep the existing lexer trivia, alias formatting, contract comments and reload error on an unsupported edit.

A complete repeated recipe must replace at least three times the tokens of its call at an actual use site. Before landing cuts 4–6, compare an inlined expanded body with the call using the corpus tokenizer and attach both source snippets. Reuse existing names where they already select the recipe. A one-line renamed builtin earns zero savings. Copy the accepted library file, import the copy, change the fold, sweep order or pass binding, and observe the corresponding shader, state or image change. [Philosophy §4](../../../docs/PHILOSOPHY.md) · [Token counter](../../../tools/corpus_tokens.py)


## What would actually have to change for 18,000

A savings of **15,881** from this baseline requires replacing both source compilation and device preparation. The counterexample is numerical. The five compiler crates (parse, check, eval, emit and passes) total **17,840**. Everything else already costs **16,041**. An attempt confined to the compiler would have **1,959** lines left for the entire language, evaluator, proofs, lowering and bundles. The open design decision is whether to replace the two boundaries below as one v1-compatible architecture. Replacing syntax, changing a look, dropping browser/edit support or losing an independent witness would change this brief.

| Replacement boundary | Current | Maximum | What has to disappear | What must still work |
|---|---:|---:|---|---|
| Source, semantics and lowering across the five compiler crates | 17,840 | 7,520 | Independent semantic dispatch on the source AST in checking, concrete execution, WGSL emission and tape preparation. Rust construction of stock camera, material, visibility, lighting, display and solver programs. | Lossless source and edits, user-defined lobes and escapes, precise diagnostics, AD/JVP and interval proofs, ordered pass/resource effects, device specialization and bundle export. |
| Native, browser, CLI, temporal runtime and live edit (cli, native, runner, std, tier0, wgpu) | 12,815 | 7,170 | Reconstructing layouts, row/dispatch work and render inputs in multiple host projections. Native-only and browser-only descriptions of the common subset. Decoding source expressions again to produce edit code. | Current CLI exits/output, standalone CPU stepping, web device injection, external handles and epochs, readbacks and timing, reversible edits, tape certification, promotion and independent CPU execution of raw kernels. |
| ABI, builtins, format, glTF, IR, plan and shared | 3,226 | 3,226 | No reduction budgeted. Additional retained types must earn their cost elsewhere in the same ledger. | Frozen wire encodings, bounded archive and replay readers, solver convergence, BVH/mesh correctness, catalog and diagnostics. |
| **Total** | **33,881** | **17,916** | **15,965 net reduction required** | **84 lines of headroom against G5.1 before incoming perf deltas.** |

1. **Have the compiler consume one typed program.** Keep source text and the CST for exact rewriting. On elaboration, emit typed values, blocks with ordered effects, declaration operands, and spans. The expression type table already supplies IDs. Extend it through call, member, control flow, units and effect binding. AD must create new typed values with stable source-map relationships. Have an abstract evaluator over those blocks instantiate f32, intervals and dependency tracking while keeping derivative/interval rules explicit. WGSL and the edit tape each lower the supported typed operations; the tape still owns its register allocation, eligibility certificate and refusal. Preserve the current frontend API by deriving its inspection view. Remove each old AST backend with its consumer. The separate Naga interpreter for arbitrary raw compute input still has an independent job.
2. **Make the engine an authored producer of the execution plan.** Extend stage 0 to build typed tables, kernels and render/compute passes from declared resources, with static specialization over kinds and lobe layouts. A user's fork must be able to replace a pass body, its bindings and draw order at source level. Emit one ordered plan with module references, initialization, symbolic extents and capability choices. Static inspection, the bundle writer and the native loader each read it. Keep byte-compatible adapters for older .loftb and logs. Replace the Rust stock expansion and second construction of resource lists. Put the complete domain recipes, including the remaining path/display, camera and motion policies, into the imported engine files. The host can still supply its own attachments and mutable tables after loading.
3. **Have both hosts consume the linked plan.** A compiler-free reader validates the incoming bytes, modules and pass uses, then materializes layout, work, binding extent and attachment records once. The native backend maps them to wgpu handles. The browser projects its supported subset and keeps its host shader callback. Row-count dispatch retains its dynamic semantics on native and the existing browser observation point. Epoch checks, limits and device-dependent reflection still run where their inputs become available. A single host state machine must also own invalidation, checkpoint restoration and promotion. Native presentation/timestamps and Three.js interoperability remain thin callers. Preserve the public runner API and functional execution counts across crates. A transfer of Rust to a tool crate, generated file or JS would still be charged to this design.

**Falsification before a rewrite.** Price one complete `blobs-1` route with an authored pass, an old bundle loaded without a compiler, and a reversible Tier 0 edit. Price one fixed-tick solver with an opted-in Det trace and a raw WGSL twin. Include the slice of parser, source provenance, failure handling and host code each actually needs. Compare IR variants and deletion sites against the three ledgers, then extrapolate only across the inventoried cases sharing those paths. Engineering constraints are tight: e.g. the runner ceiling leaves **432** lines after its 818 fixture/example lines, and the std ceiling leaves **453** after the 747-line arbitrary-compute twin. An allocation can move across crates only with the corresponding functionality and a balanced total. A prototype that exceeds either subsystem budget or loses a caller is evidence to revise the architecture. The historical lean1 prediction missed precisely at the checked-call and executor boundary. There is currently no verified basis for scheduling a 17,916-line implementation or promising the wave-2 range.

## Perf branches and landing order

I compared the committed diffs of all four branches against their common fork point `34bd927`. Their worktrees and refs belong to the perf owner. The scc deltas below are from separately counting the affected Rust blobs on each side. They overlap, so their sum is not an integrated prediction.

| Branch / tip | Independent counted Rust delta | Files held for its owner and behavior to carry forward |
|---|---:|---|
| perf-frame `683bcb3` | +12 | emit.rs, shade_tiles.rs, bvh.wgsl, passes/bundle.rs and stdlib/core classify, classify_halo and present. Preserve the sky-pixel early path, subpixel edge classification/presentation and BVH traversal order. |
| perf-load `33ef326` | +27 | CLI main and phase4_gate example, emit.rs and surface.rs, native lib and shadow, passes/bundle.rs and pipeline/stage0.rs. Preserve index-only surface creation, cross-kind derivation pool and load report. Hermit witness in the branch dropped from 14 pipelines to 13. |
| perf-instrument `fc71e15` | +55 | CLI profile, native lib, verify/suite.rs, timing.py and test_scoring.py. Carry scene-length control brackets, retries and contended/unknown semantics. Record the applicable instrument version on both arms. |
| perf-precision `6339b0e` | +91 | CLI main/profile, IR bundle/schema/precision/lib, native edit/lib/ray_query, passes/bundle.rs and wgpu storage/web. Preserve the f32 opt-out, role-specific HDR format, reported half-storage fallback and old bundle decoding. |

**Landing sequence, one owner.** First commit this design by itself. Await the named review and an explicit disposition of the size miss. If a corresponding cut is authorized, 8, 7 and 1 can be prepared serially on files outside the perf diffs. After the perf owner lands or explicitly holds their own changes, start from that main parent, recount the identical path set and reprice the overlap. Land 2 before its consumers, then 3 and 4, then 5. Land 6 last. For cut 3 wait for perf-load's LUT derivation and perf-precision's edit path, even if a particular patch would apply cleanly. For cut 6 also wait for perf-frame and for the timing instrument decision. Keep each proposed replacement private until all its consumers switch in the same commit. The size gate on main already fails; intermediate revisions can demonstrate monotonic progress and behavioral parity. Main integration still requires the absolute size gate and all requested gates to pass.

**Verification budget needs a lead choice if implementation is reopened.** Eight independently tested commits require at least eight distinct crate checks alone. Clippy, touched tests, deterministic capture instrumentation and `tools/check.sh full` require further compiler invocations. The full script itself runs a wasm check, doctests and an all-target build before the games and timing. Three compiles for the entire lane cannot validate eight distinct source revisions under those rules. Keep the zero-compile design decision in force. The review can choose independent per-cut gates with a revised compile allowance, or one approved atomic revision with a separately agreed verification budget. Do not substitute cumulative tip results for an earlier commit's gate.

## Scorecard for any approved revision

- **Build and language.** Write the complete authorized source change before claiming a slot. Use the checkout's `tools/dev.sh` with `CARGO_BUILD_JOBS=2` through the shared build admission. Use `~/.local/bin/agent-build` where required by the checkout's wrapper. Use check, Clippy and touched tests on the exact SHA. Run both Earley and llguidance via `tools/check.sh static`, and the Rust parser fixture for syntax-affecting changes. Predict identical acceptance/rejection, diagnostic code/span/fix, CLI exit, fork/import behavior and public context output for existing sources. Run the workspace suite once on the integrated candidate.
- **Picture.** Predict all 19 deterministic comparison keys pass, identical visibility IDs and D32, maximum per-channel drift 1/255, strictly fewer than 0.1% changed pixels. Compare owner approvals separately. For reference-sphere, seeds 11/23/37/53 at 64 spp, convergence ≥35 dB, each candidate ≥27 dB, temporal variance ≤0.0025 and the 10% radiance mutant must fail. Include Hermit, Shrine and matched lighting at their declared settings. Preserve original captures and per-frame keys. [Picture manifest](../../../tools/gates/pictures.json) · [Scoring instructions](../../../docs/GATES.md)
- **Twin and truth.** Predict unchanged 36/36 Twin and 34/34 Truth if the control reproduces the last integrated score. Compare check IDs, numeric evidence and unknown reasons. Exact motion opted into Det also compares tick, pose and event bytes over checkpoint/restore and native versus decoded replay. For NonDet compare the declared seeded conservation/error bounds. For raw passes verify actual readback, sparse slots and logical buffer length on both loading routes.
- **Speed and Load.** Use the same prebuilt baseline and candidate configuration. Score six independent baseline/candidate pairs in alternating AB and BA order over the 300-frame 1280×800 orbit for Capsules-, Blobs- and Hermit-1/20. Record M4/Metal adapter, overrides, pipeline/module/command counts, p95 and paired ≥95% intervals plus cold/warm load. Predict identical resources and no admitted regression beyond the configured margin. The gate documentation uses a 0.05 example; record the chosen margin and builds before starting. Use the currently admitted control protocol. Baseline uses the 0.150 ms triangle rule. If perf-instrument lands, preserve its scene-length control, block coverage and legacy-binary scoring. Hold the quiet conductor (which owns `/tmp/loft-gpu.lock` and the build slots) during timing. Release build slots before requesting that window. Contended measurements are Unknown and cannot admit a merge. [Gate manifest](../../../engines/gates.json) · [Quiet conductor](../../../tools/quiet_gpu.sh)
- **Release.** Pin the baseline binary, capture log or picture directory and the allowed timing margin for `tools/check.sh full`. Keep each gate exit, including any inherited failure. The last integrated run reported Picture 19/19, Twin 36/36, Speed 10/12, Load 25/33 and Truth 34/34. Those results belong to that revision. Rerun on the actual perf-integrated parent. Apply the new ratchet to a measured passing total ≤18,000. Check runner ≤2,200, native subtree ≤450 and zero std wgpu dependencies again. Merge/push requires actual all-green evidence and the ≤10% forecast comparison or a documented deviation. [Integration scores](../SPRINT_TO_TRAIN.md)

## Source inventory

The following tables cover all **198 Rust files** in the 39,095-line scan. Each row gives the scc code count. Each function, macro definition or data item spanning at least 100 physical source lines is individually located. Smaller functions are grouped by file and representative entry points are named. Numbered ranges in the ownership column override the file's default for *every* line, including mixed functions. This covers adjacent short handlers, impl blocks and declaration sequences as well as long single functions. The cut number identifies the disjoint slice charged in the gross estimate. An E slice charged to a cut includes essential behavior that the replacement budget must buy back.

| Class | Counted Rust | Meaning |
|---|---:|---|
| E | 29,369 | Compiler-essential, or required to preserve a concrete runtime, format, host or CLI contract. This is a responsibility classification under the current architecture. |
| R | 1,073 | Library-owned rendering, scheduling or motion recipe. Unnumbered R rows are further redesign work with zero forecast credit. |
| D | 646 | Repeated decisions with both paths identified in the numbered cut descriptions. Numeric execution, target-specific emission and independent measurement remain separately costed. |
| X | 9 | Dead by in-tree CLI/test/example call criterion. `Op::Intrinsic`, `IrModule` and `PassRegistry` are also public v1.2 reservations. Keep all nine until the spec changes or a real pass uses them. |
| T | 2,784 | Counted test, measurement/example fixture or independent test-only reference implementation. Includes test suites discovered by attributes, host acceptance probes and the CPU furnace/path oracles. |

To reproduce the inventory, apply `scc --include-ext rs --by-file --format csv crates` and repeat with `--exclude-dir tests`. Use Rust syntax-tree declaration spans, checked against source, including nested functions and explicit test-module inclusions. For mixed files, cut contiguous slices at the listed boundaries and apply the same scc Rust counter to each; assert their counts sum to the intact file. All 165 counted files reconciled. External integration tests have their own tables and contribute zero to the published gate. Cross-check symbol names in callers, macro definitions, entry targets, tests, examples and foreign exports before scoring dead code. For example `web::load_browser` has no Rust call by that name and is used as `loadBundle` by the Three.js host. `Execution::table/read` have real bundle writer/reader callers despite allow attributes. [Browser caller](../../../web/threejs_demo/runner.js) · [Reserved IR](../../../docs/spec/PATCH_V1_2.md)

### loft-abi

The frozen wire records and tick, Philox and deterministic-math functions have GPU and CPU consumers. Reimplementing the test vectors with the production function would defeat the ABI check.

| File | scc code | Function or declaration | Ownership by source line |
|---|---:|---|---|
| [src/advance.rs](../../../crates/loft-abi/src/advance.rs) | 20 | `advance` | E |
| [src/det.rs](../../../crates/loft-abi/src/det.rs) | 70 | `horner`, `det_cos`, `exp_poly`, `det_exp` | E |
| [src/lib.rs](../../../crates/loft-abi/src/lib.rs) | 201 | `BodyRow, ChainRow, wire records and layout assertions`, `VisRec::pack`, `VisRec::unpack` | E |
| [src/rand.rs](../../../crates/loft-abi/src/rand.rs) | 36 | `philox`, `cell`, `hash2`, `rng_key` | E |
| [src/tier0.rs](../../../crates/loft-abi/src/tier0.rs) | 45 | `Tier0Opcode::pack` | E |

Integration tests outside the ratchet. All rows are T.

| File | scc code | Function or fixture |
|---|---:|---|
| [tests/golden.rs](../../../crates/loft-abi/tests/golden.rs) | 191 | `storage_layouts`, `verdict_codes`, `visibility_lanes`, `philox_vectors` |

### loft-builtins

One macro invocation supplies the public builtin IDs, spellings and metadata. Candidate 2 consumes that table in another caller. The catalog was introduced in Wave 16.

| File | scc code | Function or declaration | Ownership by source line |
|---|---:|---|---|
| [src/lib.rs](../../../crates/loft-builtins/src/lib.rs) | 293 | `catalog! (builtin spellings, signatures, lowering and determinism)`, `catalog! 174–362` | E |

Integration tests outside the ratchet. All rows are T.

| File | scc code | Function or fixture |
|---|---:|---|
| [tests/table.rs](../../../crates/loft-builtins/tests/table.rs) | 47 | `each_variant_and_spelling_has_one_row`, `names_round_trip`, `source_fits_line_budget`, `count_lines` |

### loft-check

`check` and `Prog::index` are the typed frontend. `source/scope` links private names, binds raw references and rewrites types as well as expressions. `plan_source` separates binding from evaluation of closed assets. `path_twin` and the CPU medium integral are referenced by the independent engines and media tests.

| File | scc code | Function or declaration | Ownership by source line |
|---|---:|---|---|
| [src/bsdf.rs](../../../crates/loft-check/src/bsdf.rs) | 100 | `ggx_energy`, `energy_words`, `std::ops::Add for Coefficients::add`, `inscatter_joint` | E elsewhere. 9–21 R4, 27–55 R4, 57–115 T |
| [src/check.rs](../../../crates/loft-check/src/check.rs) | 1,670 | `Ck::kind 484–629`, `Ck::stmt 640–811`, `Ck::expr_inner 902–1048`, `Ck::call 1110–1400` | E elsewhere. 91–109 D1, 1110–1400 E2 |
| [src/defaults.rs](../../../crates/loft-check/src/defaults.rs) | 76 | `schedule` | R6 |
| [src/infer.rs](../../../crates/loft-check/src/infer.rs) | 110 | `syntax`, `run` | E |
| [src/kernel_units.rs](../../../crates/loft-check/src/kernel_units.rs) | 92 | `Ck::declared_unit`, `Ck::expect_unit`, `Ck::kernel_unit` | E |
| [src/lib.rs](../../../crates/loft-check/src/lib.rs) | 15 | Module declarations and exports | E |
| [src/motion_source.rs](../../../crates/loft-check/src/motion_source.rs) | 579 | `distance 490–598`, `check`, `unique`, `solver_statement`, `small_count` | E elsewhere. 243–271 D1, 424–450 D1 |
| [src/path_twin.rs](../../../crates/loft-check/src/path_twin.rs) | 126 | `add`, `random`, `Reservoir::scale`, `photon_fixed` | T |
| [src/paths.rs](../../../crates/loft-check/src/paths.rs) | 24 | `Integrator`, `WorldResources`, `PathTrace` | E |
| [src/plan_source.rs](../../../crates/loft-check/src/plan_source.rs) | 388 | `lower_source 284–410`, `invalid`, `weight`, `asset`, `vector` | E elsewhere. 149–160 D1, 315–319 D1, 339–340 D1 |
| [src/prog.rs](../../../crates/loft-check/src/prog.rs) | 282 | `Prog::index 32–161`, `Prog::linked`, `Prog::param_word`, `geometry_fields`, `finite_diff_parameter` | E elsewhere. 226–294 D7 |
| [src/program.rs](../../../crates/loft-check/src/program.rs) | 101 | `Draw::vertices`, `DispatchRows::view`, `words::serialize`, `words::deserialize` | E |
| [src/quant.rs](../../../crates/loft-check/src/quant.rs) | 60 | `Quant::name`, `Quant::resolve`, `Fp8::layout` | E |
| [src/reason.rs](../../../crates/loft-check/src/reason.rs) | 11 | `reason` | E |
| [src/references.rs](../../../crates/loft-check/src/references.rs) | 180 | `motion_math`, `References::from_body`, `References::statements`, `reachable_impl` | E elsewhere. 6–15 D2, 48–150 E7 |
| [src/source.rs](../../../crates/loft-check/src/source.rs) | 364 | `Provider for Disk::read`, `Provider for Overlay::read`, `complete`, `Graph::parsed` | E elsewhere. 225–232 R, 348–374 R |
| [src/source/scope.rs](../../../crates/loft-check/src/source/scope.rs) | 869 | `Linker::scope 166–270`, `Scope::item 654–799` | E |
| [src/world.rs](../../../crates/loft-check/src/world.rs) | 49 | `WorldCall::resolve` | E |

### loft-cli

`command`, `still`, `clip`, `edited_still` and `profile` have distinct exit, publication and latency contracts. `diff` scores author images. The `examples` targets include the ordered picture and load instruments. The Tier 0 edit/body-span duplicate is in cut 8.

| File | scc code | Function or declaration | Ownership by source line |
|---|---:|---|---|
| [examples/corpus_pictures.rs](../../../crates/loft-cli/examples/corpus_pictures.rs) | 120 | `compile`, `score`, `main` | T |
| [examples/gate_capture.rs](../../../crates/loft-cli/examples/gate_capture.rs) | 2 | `include! of tools/gates/capture.rs` | T |
| [examples/phase4_gate.rs](../../../crates/loft-cli/examples/phase4_gate.rs) | 343 | `measure 247–347`, `compile`, `bake`, `observe`, `main` | T |
| [examples/phase5_corpus.rs](../../../crates/loft-cli/examples/phase5_corpus.rs) | 86 | `time_clip`, `write_rgba`, `observe`, `main` | T |
| [examples/phase5_cpu.rs](../../../crates/loft-cli/examples/phase5_cpu.rs) | 60 | `main` | T |
| [examples/phase5_ray.rs](../../../crates/loft-cli/examples/phase5_ray.rs) | 86 | `main` | T |
| [src/budget.rs](../../../crates/loft-cli/src/budget.rs) | 52 | `Failure::exit`, `timestamps`, `measured`, `tests::missing_timestamps_are_an_environment_failure` | E elsewhere. 54–60 T |
| [src/build.rs](../../../crates/loft-cli/src/build.rs) | 21 | `run` | E |
| [src/build/artifact.rs](../../../crates/loft-cli/src/build/artifact.rs) | 10 | `lower` | E |
| [src/clip.rs](../../../crates/loft-cli/src/clip.rs) | 299 | `run 112–215`, `number`, `limit`, `observe_frame`, `tests::missing_frame_is_three_and_never_publishes_partial_output` | E elsewhere. 282–317 T |
| [src/context.rs](../../../crates/loft-cli/src/context.rs) | 232 | `Section::parse`, `catalog`, `type_end`, `last` | E elsewhere. 181–195 D8, 232–252 D8 |
| [src/diff.rs](../../../crates/loft-cli/src/diff.rs) | 256 | `Rule::parse`, `read`, `tests::step`, `tests::scattered_edge_flips_pass_but_an_off_edge_pixel_fails` | E elsewhere. 230–288 T |
| [src/edit_cmd.rs](../../../crates/loft-cli/src/edit_cmd.rs) | 250 | `edited_still 14–154`, `timed_frames`, `field_inputs`, `pixel_count`, `tests::d1_edit_checks_the_source_dependency_slice_and_residual` | E elsewhere. 202–260 T |
| [src/fmt.rs](../../../crates/loft-cli/src/fmt.rs) | 4 | `run` | E |
| [src/fork.rs](../../../crates/loft-cli/src/fork.rs) | 104 | `run`, `module_path`, `file_error`, `tests::sibling_imports_keep_their_bindings_after_relocation` | E elsewhere. 106–118 T |
| [src/lib.rs](../../../crates/loft-cli/src/lib.rs) | 121 | `sources`, `shader_hashes`, `authored_checks`, `tests::authored_verdicts_require_a_record_and_finite_evidence` | E elsewhere. 127–139 T |
| [src/lint.rs](../../../crates/loft-cli/src/lint.rs) | 56 | `run`, `lint` | E |
| [src/main.rs](../../../crates/loft-cli/src/main.rs) | 215 | `command 28–157`, `exec_sibling`, `dispatch`, `main`, `tests::fake_foot_20mm` | E elsewhere. 214–235 T |
| [src/profile.rs](../../../crates/loft-cli/src/profile.rs) | 304 | `run 28–127`, `show`, `show_tests::shown_fixed_steps_execute_solver_and_prior` | E elsewhere. 212–310 T |
| [src/render.rs](../../../crates/loft-cli/src/render.rs) | 156 | `source_still`, `run`, `create_parent`, `tests::still_accepts_diagnostic_aov` | E elsewhere. 158–166 T |

Integration tests outside the ratchet. All rows are T.

| File | scc code | Function or fixture |
|---|---:|---|
| [tests/bakeoff.rs](../../../crates/loft-cli/tests/bakeoff.rs) | 328 | `scene`, `both_pupils_use_their_own_world_origin`, `loop_binder_shadows_only_inside_the_gpu_and_cpu_loop`, `Provider for Overlay::read` |
| [tests/clip_contract.rs](../../../crates/loft-cli/tests/clip_contract.rs) | 73 | `loft`, `corpus_build_and_clip_input_failures`, `training_command_writes_sixteen_pngs_and_measured_metadata` |
| [tests/context_fork.rs](../../../crates/loft-cli/tests/context_fork.rs) | 81 | `run`, `stdout`, `context_is_compact_and_every_loft_fence_checks`, `fork_is_editable_and_compiles_from_outside_the_workspace` |
| [tests/d2_execution.rs](../../../crates/loft-cli/tests/d2_execution.rs) | 278 | `device`, `shader`, `packed_bake_matches_direct_at_every_mip`, `sampled_study_ink_draws_from_the_portable_bake` |
| [tests/escape_gpu.rs](../../../crates/loft-cli/tests/escape_gpu.rs) | 302 | `source_compute_bundle_runs_with_gpu_readback 12–121`, `sources`, `opted_out_trace_renders_beauty_and_normals`, `scatter_reference`, `nondet_law_matches_a_statistical_twin_and_refuses_cpu_stepping` |
| [tests/exit_status.rs](../../../crates/loft-cli/tests/exit_status.rs) | 22 | `cli_distinguishes_missing_input_and_failed_truth` |
| [tests/motion_lane.rs](../../../crates/loft-cli/tests/motion_lane.rs) | 875 | `motion_gpu_washer_picks_sample_zero 227–397`, `crab_gpu_thousand_row_motion_budget 660–785`, `authored_gpu_heron_and_boss_verdicts 788–907` |

### loft-emit

Type-directed WGSL and provenance are compiler work. The render policy inside `emit_kind_module`, full-screen integration in `paths` and solver strategy inside `lower` have source-library owners. The existing external WGSL templates contributed zero Rust lines to this baseline.

| File | scc code | Function or declaration | Ownership by source line |
|---|---:|---|---|
| [src/aov.rs](../../../crates/loft-emit/src/aov.rs) | 49 | `std::str::FromStr for Aov::from_str` | E |
| [src/bake_tex.rs](../../../crates/loft-emit/src/bake_tex.rs) | 97 | `size`, `workgroup`, `sample_expression`, `sampled_declarations` | E |
| [src/emit.rs](../../../crates/loft-emit/src/emit.rs) | 1,122 | `Emitter::expression 295–560`, `emit_kind_module 727–945` | E elsewhere. 444–558 D2, 765–807 R4, 826–945 R6, 973–995 R6 |
| [src/emit/column.rs](../../../crates/loft-emit/src/emit/column.rs) | 77 | `axes`, `expression`, `emit` | E |
| [src/emit/lighting.rs](../../../crates/loft-emit/src/emit/lighting.rs) | 74 | `invariant_lets`, `emit_prepared` | E |
| [src/emit/paths.rs](../../../crates/loft-emit/src/emit/paths.rs) | 243 | `emit_world 43–182`, `namespace`, `query_source`, `module`, `emit_deposit` | E elsewhere. 203–245 R6 |
| [src/emit/shade_tiles.rs](../../../crates/loft-emit/src/emit/shade_tiles.rs) | 100 | `world_source`, `module`, `classify_module`, `present_module` | E elsewhere. 61–110 R6 |
| [src/emit/solver.rs](../../../crates/loft-emit/src/emit/solver.rs) | 350 | `lower 7–295`, `sweep`, `refuse`, `nondeterministic` | E elsewhere. 202–230 R5, 231–256 D5, 297–351 R5 |
| [src/emit/surface.rs](../../../crates/loft-emit/src/emit/surface.rs) | 43 | `cached_grid` | E |
| [src/events.rs](../../../crates/loft-emit/src/events.rs) | 25 | `event_tag`, `event_wgsl` | E |
| [src/lib.rs](../../../crates/loft-emit/src/lib.rs) | 8 | `BVH_WGSL` | E |
| [src/material_shape.rs](../../../crates/loft-emit/src/material_shape.rs) | 74 | `Layout::add`, `Layout::same_as`, `Layout::eval` | E elsewhere. 48–79 R4 |
| [src/provenance.rs](../../../crates/loft-emit/src/provenance.rs) | 12 | `expression`, `raw` | E |
| [src/quant.rs](../../../crates/loft-emit/src/quant.rs) | 100 | `wgsl`, `wgsl_decode`, `dot_wgsl`, `wgsl_prelude` | E |
| [src/world/mesh.rs](../../../crates/loft-emit/src/world/mesh.rs) | 26 | `draw` | E |

### loft-eval

Concrete row building, AD, Lipschitz and refinement have different numeric contracts. `Eval<S>` already shares concrete and interval traversal. `TraceIv` tracks affine provenance for vector bounds. Keep the interval and sign/cell tests independent of generated WGSL.

| File | scc code | Function or declaration | Ownership by source line |
|---|---:|---|---|
| [src/ad.rs](../../../crates/loft-eval/src/ad.rs) | 512 | `Ad::d 91–191`, `Ad::d_call 199–360` | E |
| [src/bvh.rs](../../../crates/loft-eval/src/bvh.rs) | 277 | `Ray::new`, `Bvh2::build`, `Bvh2::trace`, `bounds` | E |
| [src/bvh/arena.rs](../../../crates/loft-eval/src/bvh/arena.rs) | 62 | `pack_world` | E |
| [src/eval.rs](../../../crates/loft-eval/src/eval.rs) | 1,382 | `Eval::stmt 321–460`, `Eval::expr_inner 554–751`, `Eval::binary 753–887`, `Eval::builtin 917–1121` | E elsewhere. 705–749 D2, 1125–1155 R4 |
| [src/import_table.rs](../../../crates/loft-eval/src/import_table.rs) | 131 | `error`, `Column::tag`, `take`, `tests::rejects_corrupt_sample_tables_and_preserves_sparse_defaults` | E elsewhere. 126–142 T |
| [src/kernel.rs](../../../crates/loft-eval/src/kernel.rs) | 21 | `kernel_argument` | E |
| [src/lib.rs](../../../crates/loft-eval/src/lib.rs) | 46 | `stage0`, `as_f32`, `pack` | E |
| [src/lip.rs](../../../crates/loft-eval/src/lip.rs) | 545 | `derive_trace 432–581`, `ml`, `Scalar for Iv::with_lip`, `Scalar for TraceIv::neg`, `derive_surface` | E |
| [src/quant.rs](../../../crates/loft-eval/src/quant.rs) | 97 | `twin`, `unpack_nibbles`, `round_bf16`, `unpack` | E |
| [src/refine.rs](../../../crates/loft-eval/src/refine.rs) | 309 | `Proof::expr 45–179`, `term`, `unknown`, `Proof::block`, `check` | E |
| [src/solver.rs](../../../crates/loft-eval/src/solver.rs) | 9 | `initial_words` | E |

### loft-format

Encoding executable sections, bounds-checked decoding, raw WGSL validation, replay ordering and identity verification are public boundaries. Writer and reader checks have different failure modes.

| File | scc code | Function or declaration | Ownership by source line |
|---|---:|---|---|
| [src/bundle.rs](../../../crates/loft-format/src/bundle.rs) | 281 | `fmt::Display for BundleError::fmt`, `bulk`, `validate_wgsl`, `get_metadata` | E |
| [src/bundle/wire.rs](../../../crates/loft-format/src/bundle/wire.rs) | 207 | `Reader::new`, `Wire for bool::get`, `Wire for Box::get`, `get_bytes` | E |
| [src/lib.rs](../../../crates/loft-format/src/lib.rs) | 3 | Module declarations and exports | E |
| [src/replay.rs](../../../crates/loft-format/src/replay.rs) | 172 | `fmt::Display for ReplayError::fmt`, `LoftLog::encode`, `LoftLog::verify_identity`, `finite` | E |
| [src/wgsl.rs](../../../crates/loft-format/src/wgsl.rs) | 27 | `Error::emit_to_string`, `std::fmt::Display for Error::fmt`, `validate` | E |

Integration tests outside the ratchet. All rows are T.

| File | scc code | Function or fixture |
|---|---:|---|
| [tests/bundle.rs](../../../crates/loft-format/tests/bundle.rs) | 77 | `container_rejects_corruption`, `pass_tree_preserves_the_wire_depth_limit` |

### loft-gltf

A source import can produce packed mesh rows and a CPU ray hit. Preserve relative path resolution, range checks, transformed geometry and triangle order.

| File | scc code | Function or declaration | Ownership by source line |
|---|---:|---|---|
| [src/lib.rs](../../../crates/loft-gltf/src/lib.rs) | 375 | `GltfScene::mesh`, `cross`, `lower`, `block` | E |

### loft-ir

The shared schema drives declarations and their wire codec. The execution descriptor carries additional ranges and initialization metadata for old/current bundles. Nine lines scored X are the specified future pass registry and its types.

| File | scc code | Function or declaration | Ownership by source line |
|---|---:|---|---|
| [src/bundle.rs](../../../crates/loft-ir/src/bundle.rs) | 128 | `bundle_schema!, Bundle, PassRegistry`, `BindAccess::group`, `Resources::is_empty` | E elsewhere. 130–138 X |
| [src/execution.rs](../../../crates/loft-ir/src/execution.rs) | 162 | `ImageStorage::encode`, `Execution::table`, `Reader::word`, `Reader::list` | E |
| [src/lib.rs](../../../crates/loft-ir/src/lib.rs) | 141 | `Span::to`, `Code::is_warning`, `Diag::note`, `fmt::Display for Verdict::fmt` | E |
| [src/schema.rs](../../../crates/loft-ir/src/schema.rs) | 196 | `bundle_schema 7–225` | E |

Integration tests outside the ratchet. All rows are T.

| File | scc code | Function or fixture |
|---|---:|---|
| [tests/platform.rs](../../../crates/loft-ir/tests/platform.rs) | 53 | `platform_abi` |

### loft-native

`Loaded::load` prepares native resources using the shared runner. Timing, texture readback, device negotiation and edit promotion still have paying callers. Shadow cache eligibility uses live shader-use analysis. Its stock reuse policy and the sun-camera framing policy have library owners in a larger rewrite.

| File | scc code | Function or declaration | Ownership by source line |
|---|---:|---|---|
| [src/edit.rs](../../../crates/loft-native/src/edit.rs) | 335 | `TraceSession::edit 157–262`, `PipelineCreates::new`, `changed_ranges`, `TraceSession::poll_promotion`, `TraceSession::promotion_stats` | E |
| [src/lib.rs](../../../crates/loft-native/src/lib.rs) | 602 | `Loaded::load 294–404`, `gpu`, `Gpu::triangle_control_ms`, `Loaded::record`, `invert` | E elsewhere. 611–621 R |
| [src/motion.rs](../../../crates/loft-native/src/motion.rs) | 118 | `sample 18–123`, `sample_motion`, `sample_motion_timed` | E |
| [src/ray_query.rs](../../../crates/loft-native/src/ray_query.rs) | 183 | `Requirements::limits`, `std::fmt::Display for RayError::fmt`, `request`, `attach` | E |
| [src/shadow.rs](../../../crates/loft-native/src/shadow.rs) | 200 | `Reuse::prepare`, `Reuse::needs_draw`, `static_frame`, `tests::only_caster_stages_must_ignore_frame_time_and_samples` | E elsewhere. 19–44 R, 185–216 T |

### loft-parse

`Cst::parse` owns lossless tokens, groups and raw islands. `Parser::item`, `kind` and `primary` apply grammar productions and diagnose source errors. This frontend already shares the CST token stream. Cut 8 preserves the source positions those callers need.

| File | scc code | Function or declaration | Ownership by source line |
|---|---:|---|---|
| [src/ast.rs](../../../crates/loft-parse/src/ast.rs) | 522 | `Item::declarations`, `BinOp::wgsl`, `Geo::short`, `Ty::words` | E |
| [src/cst.rs](../../../crates/loft-parse/src/cst.rs) | 352 | `Cst::parse 116–262`, `raw_end`, `Cst::write`, `corpus::all_authored_files_replay_verbatim`, `corpus::unit_powers_and_reciprocals_are_accepted_in_type_position` | E elsewhere. 306–321 T, 323–359 T |
| [src/lib.rs](../../../crates/loft-parse/src/lib.rs) | 4 | Module declarations and exports | E |
| [src/parser.rs](../../../crates/loft-parse/src/parser.rs) | 1,802 | `Parser::item 523–855`, `Parser::kind 996–1203`, `Parser::primary 1541–1691` | E |

Integration tests outside the ratchet. All rows are T.

| File | scc code | Function or fixture |
|---|---:|---|
| [tests/grammar.rs](../../../crates/loft-parse/tests/grammar.rs) | 38 | `parser_accepts_the_decoder_corpus_and_rejects_the_same_fixture`, `authoring_guide_uses_the_native_parser` |

### loft-passes

`pipeline::compile` orders typed stages. `derive_kind` builds proofs and packs rows. `lower_passes`, `motion_module` and `bundle::lower_program` consume schedules. CPU ray traversal and the frozen shader tests are independent references. Execution through a decoded bundle is a required entry point.

| File | scc code | Function or declaration | Ownership by source line |
|---|---:|---|---|
| [src/authored_compute.rs](../../../crates/loft-passes/src/authored_compute.rs) | 311 | `AuthoredCompute::lower 76–186`, `schedule_name`, `AuthoredCompute::declare_table`, `AuthoredCompute::record_pass`, `link_raw_compute` | E |
| [src/bake_tex.rs](../../../crates/loft-passes/src/bake_tex.rs) | 78 | `one_row`, `bake_module`, `Bake::mip_count`, `Bake::passes` | E |
| [src/bundle.rs](../../../crates/loft-passes/src/bundle.rs) | 782 | `lower_program 415–711`, `shader`, `Builder::transient`, `parameters`, `lower_twin` | E elsewhere. 504–541 R6, 548–567 R6, 579–693 R6 |
| [src/bvh.rs](../../../crates/loft-passes/src/bvh.rs) | 4 | `Bvh2 re-export, twin module` | E |
| [src/bvh/twin.rs](../../../crates/loft-passes/src/bvh/twin.rs) | 277 | `Kind::pose`, `Kind::unpack`, `CpuWorld::new`, `CpuWorld::visible` | E |
| [src/cache.rs](../../../crates/loft-passes/src/cache.rs) | 141 | `error`, `fingerprint`, `kind_key`, `derive` | E |
| [src/fusion.rs](../../../crates/loft-passes/src/fusion.rs) | 109 | `motion_params`, `tick_run`, `motion_bundle` | E |
| [src/lib.rs](../../../crates/loft-passes/src/lib.rs) | 407 | `parse_all`, `Compiled::pose`, `CompileOptions::with_cache`, `bundle::lower` | E |
| [src/motion.rs](../../../crates/loft-passes/src/motion.rs) | 270 | `motion_module 14–275`, `unsupported` | E elsewhere. 21–89 D1, 164–185 D5 |
| [src/pipeline/assemble.rs](../../../crates/loft-passes/src/pipeline/assemble.rs) | 53 | `assemble`, `Assembled::finish` | E |
| [src/pipeline/lower_passes.rs](../../../crates/loft-passes/src/pipeline/lower_passes.rs) | 149 | `lower`, `lower_world`, `lower_dispatches` | E elsewhere. 37–48 R6 |
| [src/pipeline/mod.rs](../../../crates/loft-passes/src/pipeline/mod.rs) | 75 | `Timings::lap`, `compile`, `apply_overrides`, `check_scene` | E |
| [src/pipeline/schedule.rs](../../../crates/loft-passes/src/pipeline/schedule.rs) | 362 | `resolve`, `renderer_effects`, `screen_pass`, `clone_name` | E elsewhere. 125–225 R6, 227–312 R6 |
| [src/pipeline/stage0.rs](../../../crates/loft-passes/src/pipeline/stage0.rs) | 367 | `derive_kind 182–283`, `evaluate`, `evaluate_rows`, `add_media`, `pack_luts` | E elsewhere. 143–180 R6 |
| [src/plan.rs](../../../crates/loft-passes/src/plan.rs) | 294 | `lower_source`, `sample_prior_cpu`, `tests::prior_reader_validates_in_naga`, `source_tests::authored_hop_binds_assets_and_retains_the_crouch` | E elsewhere. 171–262 T, 264–315 T |
| [src/program.rs](../../../crates/loft-passes/src/program.rs) | 75 | `Orbit::eye` | E elsewhere. 35–46 R |
| [src/provenance.rs](../../../crates/loft-passes/src/provenance.rs) | 80 | `Map::new`, `Map::origin`, `Map::expressions` | E |
| [src/quant.rs](../../../crates/loft-passes/src/quant.rs) | 8 | `compile` | E |
| [src/sexp.rs](../../../crates/loft-passes/src/sexp.rs) | 127 | `write`, `read`, `tests::typed_shader_round_trip` | E elsewhere. 88–132 T |
| [src/shade_schedule.rs](../../../crates/loft-passes/src/shade_schedule.rs) | 76 | `lighting`, `tests::tiled_world_callback_keeps_raster_passes` | E elsewhere. 1–48 R6, 50–80 T |
| [src/shader.rs](../../../crates/loft-passes/src/shader.rs) | 22 | `naga_module` | E elsewhere. 6–13 D3 |
| [src/telemetry.rs](../../../crates/loft-passes/src/telemetry.rs) | 206 | `storage_entries`, `ModuleTelemetry::function`, `tests::counts`, `tests::l23a_detects_five_inline_bsdf_copies` | E elsewhere. 174–220 T |

Integration tests outside the ratchet. All rows are T.

| File | scc code | Function or fixture |
|---|---:|---|
| [tests/bvh.rs](../../../crates/loft-passes/tests/bvh.rs) | 297 | `compile_scene`, `vacuum`, `deterministic_topology_and_atomic_refit`, `coral_shrine_has_analytic_branches_and_two_lantern_snapshots` |
| [tests/diagnostics.rs](../../../crates/loft-passes/tests/diagnostics.rs) | 258 | `compile`, `e402_kind_variant_budget`, `row_declarations_resolve_outside_the_emit_scope`, `diff_preserves_inactive_helper_parameters` |
| [tests/engines_cpu.rs](../../../crates/loft-passes/tests/engines_cpu.rs) | 88 | `sources`, `all_aovs_and_engines_validate`, `bake_declarations_lower_to_dispatch_then_mips`, `spectral_study_lowers_the_blend_and_nested_boundaries` |
| [tests/escapes.rs](../../../crates/loft-passes/tests/escapes.rs) | 180 | `sources`, `subgroup_operation_reaches_a_valid_compute_entry`, `scatter_selects_float_atomics_or_fixed_point_at_link`, `nondet_law_keeps_gpu_math_and_refuses_mixed_tiers` |
| [tests/furnace.rs](../../../crates/loft-passes/tests/furnace.rs) | 39 | `check_rgb`, `closed`, `coincident`, `analytic_reference_vectors` |
| [tests/lipschitz_rotations.rs](../../../crates/loft-passes/tests/lipschitz_rotations.rs) | 103 | `source`, `measure`, `rotations_projections_and_scales_compose`, `coupled_rows_and_distinct_folds_keep_sound_bounds` |
| [tests/modules.rs](../../../crates/loft-passes/tests/modules.rs) | 118 | `Provider for Files::read`, `graph`, `one_leaf_and_two_lexical_helpers`, `import_failures_retain_the_offending_source` |
| [tests/motion_std.rs](../../../crates/loft-passes/tests/motion_std.rs) | 357 | `build`, `Scene::set`, `Scene::toe_y`, `boss_slips_then_takes_the_second_hit` |
| [tests/phase4.rs](../../../crates/loft-passes/tests/phase4.rs) | 80 | `sources`, `shared_function_still_requires_proof_at_an_unwrapped_call`, `world_operands_are_ordered_and_light_sampling_shares_a_hook`, `negative_fixtures_fail_at_the_author_site` |
| [tests/phase5.rs](../../../crates/loft-passes/tests/phase5.rs) | 41 | `scene_names_resolve_to_bundle_indices`, `mesh_constructor_uses_asset_and_optional_lod`, `imported_instances_share_one_mesh_module` |
| [tests/reference/bsdf_media.rs](../../../crates/loft-passes/tests/reference/bsdf_media.rs) | 278 | `coincident_media_twin`, `validate`, `quant_sources`, `joint_media_surface_types` |
| [tests/reference/engines.rs](../../../crates/loft-passes/tests/reference/engines.rs) | 131 | `World for BoxFurnace::trace`, `World for WhiteFurnace::trace`, `mean`, `spectral_stack_and_integer_photons` |

### loft-plan

`prepare_and_solve` constructs the 24-knot problem. `optimize` and spline/flight layout implement the f64 prior. The anticipatory initial seed is a candidate source policy once deterministic stage-0 f64 evaluation can reproduce it.

| File | scc code | Function or declaration | Ownership by source line |
|---|---:|---|---|
| [src/lib.rs](../../../crates/loft-plan/src/lib.rs) | 318 | `prepare_and_solve 117–321`, `Curve::at`, `failure`, `finite`, `anticipate` | E elsewhere. 323–348 R |
| [src/solve.rs](../../../crates/loft-plan/src/solve.rs) | 287 | `dot`, `within`, `Multiplier::augmented`, `optimize` | E |
| [src/spline.rs](../../../crates/loft-plan/src/spline.rs) | 254 | `Form::control`, `basis`, `Flight::at`, `Layout::seed` | E |

Integration tests outside the ratchet. All rows are T.

| File | scc code | Function or fixture |
|---|---:|---|
| [tests/plan.rs](../../../crates/loft-plan/tests/plan.rs) | 236 | `y`, `boss_windup_from_arrival_only`, `active_clearance_and_tolerance`, `invalid_and_infeasible_requests_fail_closed` |

### loft-runner

`ComputeCache::prepare` links device handles, layouts and cached pipelines. `Runner` owns live mutations, rebinding, host attachment and edits. `encode/edit_gate.rs` is compiled only for native tests. Its timed edit-versus-promotion control and the other runner fixtures remain charged.

| File | scc code | Function or declaration | Ownership by source line |
|---|---:|---|---|
| [examples/loftb.rs](../../../crates/loft-runner/examples/loftb.rs) | 39 | `main` | T |
| [examples/phase5_host.rs](../../../crates/loft-runner/examples/phase5_host.rs) | 195 | `main 62–199`, `bundle` | T |
| [src/backend.rs](../../../crates/loft-runner/src/backend.rs) | 78 | `ExecutionBackend for WgpuBackend::attach`, `ExecutionBackend for WgpuBackend::resize_table`, `ExecutionBackend for WgpuBackend::step_pass_cpu`, `ExecutionBackend for WgpuBackend::export_texture` | E |
| [src/bundle.rs](../../../crates/loft-runner/src/bundle.rs) | 7 | `crate::Runner::load_bundle_bytes` | E |
| [src/embed.rs](../../../crates/loft-runner/src/embed.rs) | 83 | `Render::prepare`, `Render::pass`, `Render::misses`, `Render::recipe` | E |
| [src/encode.rs](../../../crates/loft-runner/src/encode.rs) | 561 | `ComputeCache::prepare 101–263`, `Group::bound`, `ComputeCache::kernel`, `Instance::record`, `ComputeCache::install` | E elsewhere. 286–289 T, 531–532 T |
| [src/encode/edit_gate.rs](../../../crates/loft-runner/src/encode/edit_gate.rs) | 203 | `time_edit_and_compare_native_promotion 95–210`, `words`, `fixture`, `frame` | T |
| [src/events.rs](../../../crates/loft-runner/src/events.rs) | 11 | `Runner::drain_events` | E |
| [src/frame.rs](../../../crates/loft-runner/src/frame.rs) | 45 | `Runner::load_bundle_for_frame`, `Runner::encode_load`, `Runner::encode_frame`, `Runner::install_module_pipelines` | E |
| [src/initialize.rs](../../../crates/loft-runner/src/initialize.rs) | 156 | `Initializers::prepare`, `Initializers::check_params`, `bake_shader`, `image_desc` | E |
| [src/interop.rs](../../../crates/loft-runner/src/interop.rs) | 63 | `Runner::bind_external_buffer`, `Runner::allocate_texture`, `Runner::export_buffer`, `Runner::export_texture` | E |
| [src/lib.rs](../../../crates/loft-runner/src/lib.rs) | 216 | `std::ops::Deref for Table::deref`, `Runner::with_pipeline_clock`, `Runner::set_row_count`, `Runner::encode_into` | E elsewhere. 226–227 T |
| [src/params.rs](../../../crates/loft-runner/src/params.rs) | 12 | `Runner::params`, `Runner::set_param` | E |
| [src/pick.rs](../../../crates/loft-runner/src/pick.rs) | 30 | `Runner::set_pick_source`, `Runner::pick_async`, `Runner::pick_sync` | E |
| [src/stepping.rs](../../../crates/loft-runner/src/stepping.rs) | 70 | `Runner::advance_into`, `Runner::advance`, `Runner::step_pass_cpu`, `check_fused` | E |
| [src/tests.rs](../../../crates/loft-runner/src/tests.rs) | 373 | `host_edits_parameters_and_external_handles_at_frame_boundary 255–379`, `bundle`, `device`, `vertex_or_index_replacement_invalidates_recorded_draws`, `reflected_texture_group_accepts_sparse_slots` | T |
| [src/tier0.rs](../../../crates/loft-runner/src/tier0.rs) | 13 | `Runner::register_tier0`, `Runner::hot_swap_tape` | E |

### loft-std

No wgpu dependency. `compute_twin::dispatch` interprets externally supplied compute IR; it reaches raw programs unavailable to the source evaluator. `History` owns seeking, checkpoints and Det refusal. CPU validation, input/output layouts and parameter state are used by runner.

| File | scc code | Function or declaration | Ownership by source line |
|---|---:|---|---|
| [src/bundle.rs](../../../crates/loft-std/src/bundle.rs) | 163 | `Resources::new`, `assign`, `passes`, `children` | E |
| [src/compute_twin.rs](../../../crates/loft-std/src/compute_twin.rs) | 747 | `binary 214–322`, `Value::word`, `read`, `Eval::reference`, `dispatch` | E |
| [src/error.rs](../../../crates/loft-std/src/error.rs) | 27 | `From for RunnerError::from` | E |
| [src/events.rs](../../../crates/loft-std/src/events.rs) | 33 | `capacity`, `decode` | E |
| [src/frame.rs](../../../crates/loft-std/src/frame.rs) | 63 | `Templates::new`, `Templates::change` | E |
| [src/inputs.rs](../../../crates/loft-std/src/inputs.rs) | 97 | `Inputs::new`, `Inputs::size`, `Inputs::advance`, `range` | E |
| [src/lib.rs](../../../crates/loft-std/src/lib.rs) | 11 | Module declarations and exports | E |
| [src/mips.rs](../../../crates/loft-std/src/mips.rs) | 98 | `Clock::due`, `Clock::complete`, `Dimension::group`, `source` | E |
| [src/params.rs](../../../crates/loft-std/src/params.rs) | 89 | `width`, `Params::bytes`, `Params::copy_from`, `Params::take_upload` | E |
| [src/pick.rs](../../../crates/loft-std/src/pick.rs) | 69 | `miss`, `decode`, `shader` | E |
| [src/promotion.rs](../../../crates/loft-std/src/promotion.rs) | 133 | `Default for Pending::default`, `Pending::generation`, `Pending::complete`, `tests::l23a_debounces_and_discards_stale_pipeline` | E elsewhere. 124–150 T |
| [src/stepping.rs](../../../crates/loft-std/src/stepping.rs) | 341 | `Fused::reflect`, `History::seek`, `cpu_tick_module`, `History::restored` | E |

Integration tests outside the ratchet. All rows are T.

| File | scc code | Function or fixture |
|---|---:|---|
| [tests/mips.rs](../../../crates/loft-std/tests/mips.rs) | 41 | `every_dimension_and_format_validates`, `initial_and_dirty_clocks_are_edge_triggered` |
| [tests/params.rs](../../../crates/loft-std/tests/params.rs) | 41 | `l23a_clamps_and_uploads_once` |
| [tests/pick.rs](../../../crates/loft-std/tests/pick.rs) | 24 | `picking_shader_and_unprojection` |
| [tests/stepping.rs](../../../crates/loft-std/tests/stepping.rs) | 9 | `checkpoint_windows` |

### loft-tier0

The certified tape, fixed opcode semantics and allocator support live geometry edits. Direct residuals of spheres/boxes and native promotion are distinct modes. `diagnostics.rs` lives under `src` but enters through the test-only module declaration.

| File | scc code | Function or declaration | Ownership by source line |
|---|---:|---|---|
| [src/lib.rs](../../../crates/loft-tier0/src/lib.rs) | 2 | Module declarations and exports | E |
| [src/tier0.rs](../../../crates/loft-tier0/src/tier0.rs) | 448 | `Tape::run 181–351`, `operands`, `dot`, `smin`, `voronoi` | E elsewhere. 468–470 T |
| [src/tier0/diagnostics.rs](../../../crates/loft-tier0/src/tier0/diagnostics.rs) | 325 | `CASES 19–328`, `authored_diagnostic_snapshots` | T |
| [src/tier0/session.rs](../../../crates/loft-tier0/src/tier0/session.rs) | 489 | `TraceSource::prepare 160–296`, `PreparedEdit::class`, `TraceSource::bounds`, `render_shader`, `token` | E elsewhere. 412–439 D3, 461–470 D3, 472–516 D8 |
| [src/tier0_author.rs](../../../crates/loft-tier0/src/tier0_author.rs) | 722 | `Lower::statements 447–556`, `Lower::expr 558–755` | E elsewhere. 673–752 D2 |

Integration tests outside the ratchet. All rows are T.

| File | scc code | Function or fixture |
|---|---:|---|
| [tests/tier0.rs](../../../crates/loft-tier0/tests/tier0.rs) | 361 | `l23a_every_opcode_against_cpu_reference 18–173`, `word`, `assert_close`, `l23a_vm_validates_and_dispatches_all_frozen_ids`, `l23a_sphere_tape_matches_the_e_plus_twin` |

### loft-wgpu

`Schedule::new` validates input bundles, `bindings::reflect` checks pass uses, and interop validates live handles and epochs. Native commands and the direct browser host both consume the ABI. The browser accepts a caller-owned device and an adapter for host shader conventions.

| File | scc code | Function or declaration | Ownership by source line |
|---|---:|---|---|
| [src/attachments.rs](../../../crates/loft-wgpu/src/attachments.rs) | 219 | `Canvas::depth_state`, `Canvas::depth_ops`, `Attachments::canvas`, `tests::load` | E elsewhere. 49–56 T, 59–65 T, 183–235 T |
| [src/bindings.rs](../../../crates/loft-wgpu/src/bindings.rs) | 185 | `reflect 8–147`, `slot`, `merge_bindings`, `group`, `validator` | E |
| [src/command.rs](../../../crates/loft-wgpu/src/command.rs) | 163 | `compute_pipeline`, `Pass::append`, `record_render`, `RasterRecipe::name` | E |
| [src/edits.rs](../../../crates/loft-wgpu/src/edits.rs) | 229 | `Edits::request`, `Edits::update`, `Edits::edit`, `tests::l23a_upload_validates_before_replacing_tape` | E elsewhere. 227–242 T |
| [src/frame.rs](../../../crates/loft-wgpu/src/frame.rs) | 108 | `Work::resolve`, `Clock::validate`, `Clock::advance` | E |
| [src/interop.rs](../../../crates/loft-wgpu/src/interop.rs) | 309 | `stale`, `storage_access`, `Resource::buffer`, `Resource::bind` | E |
| [src/lib.rs](../../../crates/loft-wgpu/src/lib.rs) | 31 | `allocate`, `timed` | E |
| [src/mips.rs](../../../crates/loft-wgpu/src/mips.rs) | 211 | `storage_view`, `bindings`, `Packed::record`, `Mips::encode` | E |
| [src/outbox.rs](../../../crates/loft-wgpu/src/outbox.rs) | 107 | `Outbox::new`, `Outbox::prepare`, `Outbox::end`, `Outbox::drain` | E |
| [src/pick.rs](../../../crates/loft-wgpu/src/pick.rs) | 128 | `Source::publish`, `Source::pick_async` | E |
| [src/promotion.rs](../../../crates/loft-wgpu/src/promotion.rs) | 41 | `start`, `compile` | E |
| [src/ray_query.rs](../../../crates/loft-wgpu/src/ray_query.rs) | 54 | `limits`, `has_aabb_capacity`, `build_aabbs` | E |
| [src/readback.rs](../../../crates/loft-wgpu/src/readback.rs) | 55 | `read_buffer`, `read_texture`, `read_copy` | E |
| [src/schedule.rs](../../../crates/loft-wgpu/src/schedule.rs) | 302 | `Order::visit`, `Linker::dispatch`, `Linker::kernel`, `groups` | E |
| [src/storage.rs](../../../crates/loft-wgpu/src/storage.rs) | 158 | `allocations`, `rows`, `compatible_state`, `create` | E |
| [src/web.rs](../../../crates/loft-wgpu/src/web.rs) | 288 | `load_browser 99–290`, `WebGpu::attach`, `WebGpu::prepare`, `array`, `method` | E |

Integration tests outside the ratchet. All rows are T.

| File | scc code | Function or fixture |
|---|---:|---|
| [tests/attachments.rs](../../../crates/loft-wgpu/tests/attachments.rs) | 96 | `check_targets`, `pass`, `distinct_logical_targets_cannot_alias_the_host_views`, `render_shader_validates_and_reflects_storage_in_both_stages` |
| [tests/interop.rs](../../../crates/loft-wgpu/tests/interop.rs) | 55 | `l23a_interop_checks_and_affected_groups_only` |
| [tests/outbox.rs](../../../crates/loft-wgpu/tests/outbox.rs) | 16 | `outbox_record_prefix` |

### shared

The visibility-ID helper is charged once under `crates/shared`.

| File | scc code | Function or declaration | Ownership by source line |
|---|---:|---|---|
| [visibility_ids.rs](../../../crates/shared/visibility_ids.rs) | 10 | `visibility_ids` | E |
