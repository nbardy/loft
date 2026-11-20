# Open targets for v1.0-alpha

## Wave X2 integration

X2d’s legacy benchmark and X2e’s Three.js demo merged on top of `7e33f09`. The scored merge was `180c708`. The only crate change is in loft-std’s wasm32-only web module. The native renderer, scenes and gate inputs at the starting revision match PF1’s scored `b0fdb44`. [Gate report](../../out/meta/ix2/gate.json) · [18-frame contact sheet](../../out/meta/ix2/sheet.webp)

One locked Apple M4 Metal run at 1280 × 800, with 30 warmups and 300 timed frames per scene, held Twin **36/36**, Truth **34/34**, Speed **12/12** and Load **31/33**, with zero contended verdicts. All six raw p95s increased from PF1. Both Hermit counts created 14 pipelines against 13. X2d and X2e qualified on their own checks under the tools/demo merge rule.

| Scene | Main p95, ms | X2 p95, ms | Cap, ms | Commands | Triangle p50, ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| capsules-1 | 0.451 | 0.579 | 0.800 | 5 | 0.096 |
| capsules-20 | 0.663 | 0.847 | 1.100 | 5 | 0.040 |
| blobs-1 | 0.406 | 0.540 | 1.100 | 5 | 0.098 |
| blobs-20 | 1.079 | 1.565 | 2.400 | 5 | 0.096 |
| hermit-1 | 2.453 | 2.740 | 5.000 | 13 | 0.040 |
| hermit-20 | 3.752 | 4.803 | 5.600 | 13 | 0.098 |

All 18 gate frames matched main’s decoded RGBA exactly. Pictures held **0/18** against the frozen pins for the previous appearance. The layered-material approval at `7e33f09` assigns re-pinning to IX3. All nine bead comparisons passed and matched main. Before and after inventories verified 18 gate pins, nine bead references and 1,510 historical beauty files. [Raw gate](../../out/meta/ix2/five-gates.json) · [Bead results](../../out/meta/ix2/bead/score.json)

All six mutants fired. The wasm32 runner with web-sys, workspace doctests and both grammar recognizers passed. Earley and llguidance each classified 281/281 cases as expected. The single workspace build produced 25 test executables. Native tests passed **153/154**. The existing Tier 0 promotion test measured 0.520375 ms p95 overhead against 0.500 ms, with 0/140,000 pixels over 1/255. Loft-std grew from 4,321 to 4,536 physical Rust/WGSL source lines (+215). The other eight crate counts matched main. [Native log](../../out/meta/ix2/native.log) · [Per-crate counts](../../out/meta/ix2/lines.json)

| Lane | Disposition and evidence |
| --- | --- |
| X2a · `68400b0` | Held. The shared recorder, packed/direct R16 checks, numeric ranges and concurrent cold pipeline preparation worked. All 18 frames matched and Load reached 31/33. Speed fell from 12/12 to 11/12 (Capsules-1 at 0.835 ms against 0.800). Execution used 6,717 lines against 2,500. Subresource views, sampler, index subranges and attachment policies still use defaults. Std still depends directly on wgpu and wgpu-naga-bridge. Frame uploads allocate in runner’s encode.rs and the event header allocates in std’s outbox.rs. The branch does derive vertex usage and export bake/path bundles. [Lane audit](https://github.com/nbardy/loft/blob/68400b0/agent_notes/2026-11-20_v1/lanes/x2a.md) |
| X2b · `4a364e9` | Held. Crab, Heron and Boss matched 481/481 ticks per scene and the E504 refusal probes passed. Ordinary shading multiply-add matched explicit FMA on 4,096/4,096 pixels and differed from split rounding on 1,804. The integration MSL check found zero strict helpers or contraction bans in 16 Shade entries and detected all three strict motion controls. The branch lacks a committed automated leak guard. The final focused 1,000-Crab dispatch measured 0.179500 ms p95 against 0.150 ms, up from 0.024042 ms on main. [MSL checks](../../out/meta/ix2/x2b-msl.json) · [Motion budget](../../out/meta/x2b/thousand-crab-3.log) |
| X2c · `b314298` | Held. The branch contains the diagnosis only. Ground’s Shade layout lacks group 2 binding 5 for the sampled cube. The bake-only probe reproduced all three main stills and moved Shade p50 from 2.248 to 2.257 ms against a required 1.000 ms saving. There is no integrated candidate frame for owner approval. [Binding diagnostic](../../out/meta/x2c/binding.log) |
| X2d · `8461865` | Merged. Both 300-frame comparisons reproduced all four stills and both legacy bundles. At 20 groups the two legacy/native GPU p95 pairs were 9.389/2.798 and 9.326/1.019 ms. Each engine issued five commands. The report documents lighting, sampling and load-scope differences. [A/B report](AB_OLD_VS_NEW.md) |
| X2e · `d297bc3` | Merged. Headless Chrome shared one device and queue and matched all 151 attachment pairs with zero readbacks. In 18,925 overlapping pixels the front cube won all, the rear cube won none, and the intersecting cube won 6,958. Relighting changed 57,475 crab pixels. Composed queue completion measured 17.215 ms p95. [Browser measurements](../../out/meta/x2e/browser.json) · [Demo](../../web/threejs_demo/README.md) |

## PF1 integration

PF1a’s native sun-map reuse passed at `b0fdb44`. Hermit-20 used 13 commands on every timed frame and improved from 4.802 to 3.752 ms p95. All six scenes improved on the published del2 p95 measurements at `33cdcbf`. That revision and integration base `bca209d` have identical renderer, engine and fixture sources. [Gate report](../../out/meta/ipf/gate.json) · [18-frame contact sheet](../../out/meta/ipf/sheet.webp)

| Scene | Main p95, ms | PF1a p95, ms | Cap, ms | Commands | Triangle p50, ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Capsules-1 | 0.528 | 0.451 | 0.800 | 5 | 0.100 |
| Capsules-20 | 0.961 | 0.663 | 1.100 | 5 | 0.100 |
| Blobs-1 | 0.678 | 0.406 | 1.100 | 5 | 0.099 |
| Blobs-20 | 1.629 | 1.079 | 2.400 | 5 | 0.099 |
| Hermit-1 | 3.225 | 2.453 | 5.000 | 13 | 0.062 |
| Hermit-20 | 4.802 | 3.752 | 5.600 | 13 | 0.053 |

One locked Apple M4 Metal run at 1280 × 800, with 30 warmups and 300 timed frames per scene, held Twin **36/36**, Truth **34/34**, Speed **12/12** and Load **31/33**, with zero contended verdicts. Both Hermit counts created 14 pipelines against 13. Pictures held **0/18** against the frozen appearances already queued for owner review. All 18 rendered RGBA hashes matched main exactly. All nine bead frames passed their frozen comparison and matched main exactly. The before and after inventories verified 18 gate references, nine bead references and 1,510 historical beauty files. [Five-gate measurements](../../out/meta/ipf/five-gates.json) · [Bead comparisons](../../out/meta/ipf/bead/score.json)

All six mutants fired. The wasm32 runner with `web-sys`, workspace doctests and both grammar recognizers passed. Earley and llguidance each classified 281/281 fixtures as expected, including the negative sentinel. The workspace build produced 25 native test executables. **153/154** tests passed. The existing runner Tier 0 promotion test measured **0.932 ms** p95 overhead against **0.500 ms**, with 0/140,000 pixels over 1/255. CLI grew from 5,357 to 5,616 physical Rust/WGSL source lines (+259). The other eight crates had zero line delta. [Native test log](../../out/meta/ipf/native.log) · [Per-crate counts](../../out/meta/ipf/lines.json)

| Lane | Audit and disposition |
| --- | --- |
| PF1a · `e00e634` | Merged. The final paired run saved 0.709 ms median on Hermit and 0.442 ms on close Shrine. The slower candidate p95 was 4.425 and 3.211 ms respectively. Changing the sun redrew 219,448 shadow texels and produced the expected 13 → 17 → 13 command sequence. All 36 paired beauty, depth, ID and shadow comparisons matched at 0/4/9 s, including both fixed Shrine row orders. [Lane evidence](lanes/pf1a.md) |
| PF1b · `3606613` | Held out. Two Shrine runs met both targets. The final allowed replay saved 0.236 ms Vis p50 against the 0.250 ms minimum and measured 3.826 ms frame p95 against the 3.800 ms cap. Hermit passed the absolute cap in every run and was 2.08–4.89% faster than control, outside the brief’s literal ±2% band. All 63 decoded comparisons and ten boundary cases matched. [Lane evidence](https://github.com/nbardy/loft/blob/360661374ee02b5045bff30e96cfbb8c87763c8f/agent_notes/2026-11-20_v1/lanes/pf1b.md) |
| PF1c | Undelivered. The PF1 manifest lists only PF1a and PF1b. Origin supplied no PF1c ref, and the repository supplied no PF1c brief or lane note. [Lane manifest](../2026-11-20_codex/pf1.lanes) |

## Cook cycle 1 integration

Cook cycle 1 retained the del2 engine. C1b and C1d passed their focused promotion and replay measurements. The combined candidate `bb7e43c` missed the publication rule. Cargo discovered C1c’s integration-test target before the integrator removed that lane for a legal-loop regression. The production compiler built after removal. The stale test path stopped the single workspace build with exit 101. Its 11 emitted native test executables passed **81 tests** of 156 expected. 75 tests lack a combined result. [Build diagnostics](../../out/meta/i1/full.stderr) · [Cargo timeline](../../out/meta/i1/build-timeline.json) · [Excluded heads and metrics](../../out/meta/i1/BLOCKED)

The single Apple M4 Metal gate returned 3. Pictures held **0/18** against the approved frames. **18/18** actual RGBA hashes matched del2. Twin held **36/36**, Speed **9/12**, Load **31/33** and Truth **34/34**. All nine bead renders matched main, and the 18 gate pins, nine bead pins and 1,510 historical beauty hashes verified before and after. Both grammar recognizers passed all 280 original cases including the negative sentinel and all three additions. Doctests and wasm32 with web-sys passed. Five mutants fired in the combined binaries. The sixth, five inlined BSDF copies, fired separately in C1b’s compiler test binary with matching compiler sources. [Gate evidence](../../out/meta/i1/gate.json) · [Frame comparisons](../../out/meta/i1/pixels.json) · [Contact sheet](../../out/meta/i1/sheet.webp)

| Lane | Delivery and remaining work |
| --- | --- |
| C1a · index-only Hermit · [f9bd52c](https://github.com/nbardy/loft/blob/f9bd52cb89f2195314cb25ccbf496447ac53890c/agent_notes/2026-11-20_v1/lanes/c1a.md) | The actual bake pipeline disappeared at counts 1 and 20. Creates fell 14→13, load-bake dispatches fell to zero, eight modules and 17 frame commands remained, and all 18 beauty/depth/ID frames matched. Both final cold loads missed 1.500 s at 3.781/4.371 s. The slowest existing pipeline took 3.413/4.091 s. Matched main also missed its ordinary CLI load caps. The branch needs a qualifying cold-load and Speed measurement. |
| C1b · native Tier 0 promotion · [b656186](https://github.com/nbardy/loft/blob/b656186b7cc6b00f2a3df35ed3f821be6805c040/agent_notes/2026-11-20_v1/lanes/c1b.md) | The three planned paired p95 increases were +0.244/+0.257/+0.230 ms against +0.500 ms. The combined runner measured +0.264 ms with 0/140,000 pixels over 1/255. All seven register, loop and edit controls matched the CPU. A full combined suite remains due. Authored full-screen tapes, LUT/texture paths and browser promotion remain open. |
| C1c · raw motion verification · [f01386f](https://github.com/nbardy/loft/blob/f01386f81d638fa2d9a0acdb29409bcd13016cc3/agent_notes/2026-11-20_v1/lanes/c1c.md) | All 12 planned law and solver cases passed. The literal and declared three-trip loops reproduced law 0/4/12 and solver 1/4/10 at ticks 0/1/3. Main accepts both `i < 3u` and `3u > i`. The lane emits E504 for the second spelling on both routes. Normalize the comparison operands in its induction proof and retain the data-dependent loop refusals and author locations. [Guard controls](../../out/meta/i1/loop-controls/results.json) |
| C1d · persistent native replay · [009fca1](https://github.com/nbardy/loft/blob/009fca1a898cbf451f0125aa999d053e0933ab14/agent_notes/2026-11-20_v1/lanes/c1d.md) | Render and check matched the independent four-byte identities for 8.0 and 7.0 after four ticks. Check returned 2 for the recovering tick-2 miss. The fresh 481-tick Scramble image matched. A full combined suite remains due. Event/PosePacket input, cross-device tolerances and checkpoints remain open. |

### Frame and load observations

Each scene used 1280 × 800, 30 warmups and 300 measured frames. Commands are the observed count on every timed frame. Cold/warm times include derivation and device preparation. Del2 at `33cdcbf` scored Speed 12/12 and Load 31/33. The intervening PF1 diagnostic on the same engine scored 8/12 and 24/33.

| Scene | Del2 p95, ms | Candidate p95, ms | Delta, ms | Cap, ms | Commands | Triangle, ms | Cold / warm, s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| capsules-1 | 0.528 | 0.691 | +0.163 | 0.800 | 5 | 0.096 | 0.327 / 0.005 |
| capsules-20 | 0.961 | 0.990 | +0.029 | 1.100 | 5 | 0.031 | 0.380 / 0.006 |
| blobs-1 | 0.677 | 2.082 | +1.404 | 1.100 | 5 | 0.096 | 0.343 / 0.006 |
| blobs-20 | 1.629 | 1.974 | +0.346 | 2.400 | 5 | 0.032 | 0.426 / 0.007 |
| hermit-1 | 3.225 | 5.130 | +1.905 | 5.000 | 17 | 0.100 | 0.916 / 0.021 |
| hermit-20 | 4.802 | 6.533 | +1.731 | 5.600 | 17 | 0.099 | 0.893 / 0.022 |

Frame-time misses were blobs-1 2.082/1.100 ms, hermit-1 5.130/5.000 ms, hermit-20 6.533/5.600 ms. Hermit created 14 and 14 pipelines against 13 at each count. The gate classified 0 Speed verdicts as contended. [All verdicts](../../out/meta/i1/gate.stdout)

Matched main/candidate profiles reversed the order for the second pair. The candidate-minus-main p95 deltas were capsules-1 +0.076/-0.548 ms, capsules-20 -0.021/-0.027 ms, blobs-1 -0.031/+0.142 ms, blobs-20 -0.188/+0.218 ms, hermit-1 +0.648/+0.005 ms, hermit-20 -0.021/+0.269 ms. Both orders measured a positive delta for hermit-1. Hermit-1 varied by 1.320 ms between the two candidate profiles. Its Shade median changed by +0.130 ms and −0.027 ms against adjacent main. All paired shader digests matched. Those measurements leave the cause of the hard Speed misses unresolved. [Profile controls and pass medians](../../out/meta/i1/controls/profiles.json)

The candidate adds 144 physical Rust/WGSL source lines in CLI, 165 in Passes and 98 in Runner. The expanded functional L0 count is 5,814→5,953 using the same source ranges on both revisions where the files match. It charges motion and replay execution in CLI and the shipped VM in Passes, with a single physical opcode body. [Per-crate and L0 accounting](../../out/meta/i1/lines.json)

### Owner decisions

The 18 layered frames, Shrine woodblock look, archived per-hit-shadow proposal and 187 corpus beauty pairs in 17 batches, including revised bead, remain queued. The 107 scratch geometry/depth observations and pinned trace identities await owner re-pins. [Appearance queue](../../out/meta/i6/approval_needed.md) · [Scratch provenance](lanes/d1d.md) · [Trace bundle identities](../../out/meta/c1d/bundle-controls.json)

The frozen texture-bundle format, external Keel and Room Runners dependency re-pins, release retag and downstream first-light merge remain owner decisions. The first-light candidate is `472a78f` on Room Runners, using Loft `b1a4c26` against Keel base `b4cf34f`. [Frozen format request](../../out/meta/i4-r2/approval_needed.md) · [Host pin history](room_runner/STUDY.md) · [First-light candidate](https://github.com/nbardy/room-runners-arena-lib/commit/472a78f341389118e7b845660f654e5bcdda3aba) · [Release decisions](NOTES_AND_TODO.md)

## Legacy retirement

One locked Apple M4 Metal run on `33cdcbf` scored Pictures **0/18**, Twin **36/36**,
Speed **12/12**, Load **31/33** and Truth **34/34**, with zero contended verdicts.
All 18 gate RGBA hashes and all nine bead renders matched archived main bit for bit. The two
Load misses were Hermit's 14 pipelines against a budget of 13. All 1,510 historical beauty
hashes verified. The reference approval queue is unchanged. [Gate measurements](../../out/meta/del2/five-gates.json) · [Exact frame comparisons](../../out/meta/del2/pixels.json)

The single native build produced 25 test executables. **152/153** tests passed, including all
six mutants and the three transferred image-comparison tests. The Tier 0 promotion test measured
**0.581 ms** p95 against **0.500 ms**, with zero of 140,000 pixels over 1/255. Isolated checks
measured **0.667 ms** and **0.796 ms**. The same test executable (identical SHA-256) measured
**0.787 ms** on concurrent unchanged main. This timing miss remains open. The wasm32 runner
checked with web-sys, doctests passed, and Earley and llguidance each returned all 280 expected
outcomes. [Native and main evidence](../../out/meta/del2/gate.json)

The root Cargo workspace owns the native and WebAssembly builds. The 18 approved images in
`tests/fixtures/gate-refs/` carry the same RGBA hashes pinned in `engines/gates.json`.
`loft gate` records each rendered RGBA hash in `out/meta/gate.json` for exact comparisons with
a previously scored revision. Picture approval remains a separate owner decision.

| Retired check or tool | Disposition |
| --- | --- |
| Bun suite, typecheck and loader-import scan | Their compiler, renderer and loader were deleted. The Phase 5 manifest partition check survives in `tools/check_corpus.py`. Native Cargo tests cover the current compiler and runner, and the dependency check keeps the runner independent of compiler passes. |
| Nested Cargo workspace checks, lab binaries and single-test-binary layout | The Bun serve tests launched the retired `loft-node` and `loft-draw` binaries and exercised their child-process protocol. Those binaries and the old bundle loader were deleted. `tools/dev.sh` builds the root members. Per-crate lint is available through `tools/dev.sh lint`. |
| R0 regeneration, live cross-engine replay and TypeScript seed sampling | They require executing the retired engine. The original inputs, hashes and observations remain historical evidence. New comparisons use the checked-in corpus and frozen images. |
| Standalone bead comparator | The same pixel, edge and cluster rules and their three tests now run in the root CLI as `loft diff`. The bead gate renders all nine observations with the workspace’s `phase5_corpus` example. |

The [Phase 5 corpus](../../corpus/README.md) feeds the grammar and geometry checks.
[Examples](../../examples/README.md) remain read-only migration sources. The browser viewer depended on
the retired engine. The unused training directory had no tracked files. The [historical design](../../docs/legacy/DESIGN.md)
and manuals are archived.

## D2 integration on 20 November

D2a, D2b and D2c remain on their branches. Each lane’s build exited 101. The single locked Apple M4 Metal gate scored retained main at `47ac462`. Its executable matched the archived main binary byte for byte. Twin held 36/36 and Truth 34/34. Speed fell from 12/12 to 10/12, missing the merge rule. The gate recorded zero contended verdicts. [Gate report](../../out/meta/i8/gate.json) · [Contact sheet](../../out/meta/i8/sheet.webp)

| Lane | Evidence and reason for exclusion |
| --- | --- |
| D2a · authored motion · `71b0be0` | The verifier hit E0308 and E0502. Crab, Heron and Boss still call `run_laws`. The lane compared zero of 481 required candidate ticks per table and zero of three half-second pictures. [Lane note](https://github.com/nbardy/loft/blob/71b0be06235d4b1fc68e9782b75245a27009ca5a/agent_notes/2026-11-20_v1/lanes/d2a.md) |
| D2b · Tier 0 edits · `8177a09` | The pushed head includes a type annotation for E0282 and has **0/3** required timing runs. Its authored trace shader has no bindings for LUT or texture samples. Archived 120-pair A/A controls measured **+0.109, +0.147 and +0.170 ms** against **+0.100 ms**. Upload, frame cost and parity on the pushed head remain unmeasured. [Lane note](https://github.com/nbardy/loft/blob/8177a09ea58bf6ffc6bfaec59b90acf975bc19d9/agent_notes/2026-11-20_v1/lanes/d2b.md) |
| D2c · portable execution · `1d89966` | The pushed head corrects E0369. Native and archive parity, baked mip bytes and browser execution remain unverified. The execution ledger grew from **5,209 to 5,473 lines** against **2,500**. Runner accounts for 1,977, Std for 2,419 and native CLI for 1,077. [Lane note](https://github.com/nbardy/loft/blob/1d89966247a099dc2df656e54417e3b2dbb1a803/agent_notes/2026-11-20_v1/lanes/d2c.md) |

All 18 beauty frames matched archived main RGBA. Pictures held 0/18 against frozen references. All 18 differences are in the existing layered-material owner queue, with at most 8 pixels per frame more than two codes from the proposal. Load held 25/33. All six scenes missed their cold-load caps, and both Hermit scenes created 14 pipelines against 13. The raw five-gate command exited 3. [Pixel comparisons](../../out/meta/i8/pixels.json) · [Owner queue](../../out/meta/i6/approval_needed.md)

Frozen reference hashes matched before and after for 18 gate frames, 9 bead frames and 1,510 legacy beauty images. Bead passed 9/9 geometry checks and reproduced 9/9 archived RGBA frames. All 6 mutants fired. The wasm32 runner checked with web-sys. Earley and llguidance each returned 280 expected outcomes, including the negative fixture. The native suite passed 149/150. The Tier 0 promotion test measured **0.787 ms** p95 overhead against **0.500 ms**, with zero of 140,000 pixels over 1/255. Bun passed 315/315. [Run log](../../out/meta/i8/runs.jsonl)

### Speed and cold load on retained main

Each scene ran for 300 timed frames at 1280 × 800.

| Scene | Prior main p95, ms | D2 integration p95, ms | Cap, ms | Commands | Cold load, s | Cold cap, s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| capsules-1 | 0.443 | 0.522 | 0.8 | 5 | 0.629 | 0.5 |
| capsules-20 | 0.749 | 1.322 | 1.1 | 5 | 0.793 | 0.6 |
| blobs-1 | 0.520 | 1.451 | 1.1 | 5 | 0.617 | 0.5 |
| blobs-20 | 1.498 | 1.716 | 2.4 | 5 | 0.823 | 0.5 |
| hermit-1 | 3.814 | 3.880 | 5.0 | 17 | 1.964 | 1.5 |
| hermit-20 | 5.271 | 5.531 | 5.6 | 17 | 2.038 | 1.5 |

### Physical lines per crate

Counts include tracked Rust and WGSL under each crate’s `src/`, including blank lines and inline tests.

| Crate | Retained main | D2a | D2b | D2c |
| --- | ---: | ---: | ---: | ---: |
| loft-abi | 498 | 498 | 498 | 498 |
| loft-cli | 5,066 | 5,066 | 5,315 | 4,183 |
| loft-format | 815 | 815 | 815 | 815 |
| loft-ir | 548 | 548 | 548 | 720 |
| loft-parse | 2,723 | 2,723 | 2,723 | 2,723 |
| loft-passes | 16,456 | 17,423 | 17,310 | 16,627 |
| loft-plan | 949 | 949 | 949 | 949 |
| loft-runner | 1,630 | 1,630 | 1,630 | 2,387 |
| loft-std | 4,321 | 4,322 | 4,321 | 4,241 |

## v1–v3 integration on 20 November

All three lane heads stay on their branches. The single locked Apple M4 Metal run scored the retained engine at `843adfe`. Its executable matched the SHA-256 of the archived main build at `9878c6e`. Twin held 36/36, Truth 34/34 and Speed 12/12, with zero contended verdicts. All six mutants fired, the wasm32 runner checked with web-sys, and Earley and llguidance each returned 280/280 expected results. Native passed 150/150 and Bun passed 315/315. [Gate report](../../out/meta/iv/gate.json) · [Contact sheet](../../out/meta/iv/sheet.webp)

All 18 fresh 1280×800 frames matched the archived main RGBA byte for byte. Pictures held 0/18 against the frozen references. All 18 misses belong to the existing owner queue, with at most eight pixels per frame more than two codes from the queued proposal. Load held 31/33. Hermit created 14 pipelines against a budget of 13 at both instance counts. Before and after hashes matched for 18 frozen gate images, nine bead geometry frames and 1,510 legacy beauty images. All nine bead geometry checks passed, and their renders matched archived main RGBA. [Pixel counts](../../out/meta/iv/pixels.json) · [Owner queue](../../out/meta/i6/approval_needed.md)

| Blocked lane | Measurement and reason |
| --- | --- |
| v1 · Tier 0 edit speed · `f5bea03` | Staged fields removed most interpreter overhead. The general clipped sphere took **0.486 ms** to upload and **+0.110 ms** paired GPU p95 on the first run. Confirmation measured **0.448 ms / +2.478 ms** against the **2.000 / +0.500 ms** caps. The two-sided clip missed both at **2.280 / +3.622 ms**. Literal GPU overhead was +0.531 and +2.327 ms. The marker probe was repaired, and every recorded positive edit matched the independently compiled image. [v1 lane note](https://github.com/nbardy/loft/blob/f5bea03ee6d983a4aa103a464a4435d083578a56/agent_notes/2026-11-20_v1/lanes/v1.md) · [General confirmation](../../out/meta/v1/confirm/general.log) |
| v2 · Authored solvers · `6776d49` | Packed CPU/Metal rows differed on **371/481** Crab ticks, **177/481** Heron ticks and **447/481** Boss ticks. Verdict words differed on **481/481** for each. The three final 0.5 s frames matched main. The branch adds a Heron plan and solver event support, but playback still executes the laws. Full solver execution, bit-equal twins and the E201/E308 emitters and snapshots remain open. [v2 lane note](https://github.com/nbardy/loft/blob/6776d494ee727d3440cb9f5550cbf7915e9abb2e/agent_notes/2026-11-20_v1/lanes/v2.md) · [Tick readbacks](../../out/meta/v2/twin-final.log) |
| v3 · Shared pass preparation · `d0e6a79` | All **18/18** beauty frames changed. The owner queue allows 1,024 pixels per frame to differ from the proposal by more than two codes. Every v3 frame had at least **35,063** such pixels. Load fell from **31/33 to 28/33**. The archived Classify shader failed resource binding, and the bake and spectral hosts failed storage-texture validation. Functional L0 fell **5,447 → 5,209** against **2,500**. [v3 lane note](https://github.com/nbardy/loft/blob/d0e6a7962b21c4a572759eea06e8a559207b067e/agent_notes/2026-11-20_v1/lanes/v3.md) · [Compatibility and frame results](../../out/meta/v3/verification.json) |

### Retained Speed measurements

Each case has 300 timed frames. The prior main measurements came from the byte-identical executable.

| Scene | Prior main p95, ms | This run p95, ms | Cap, ms | GPU commands |
| --- | ---: | ---: | ---: | ---: |
| capsules-1 | 0.460 | 0.443 | 0.8 | 5 |
| capsules-20 | 0.720 | 0.749 | 1.1 | 5 |
| blobs-1 | 0.512 | 0.520 | 1.1 | 5 |
| blobs-20 | 1.489 | 1.498 | 2.4 | 5 |
| hermit-1 | 3.820 | 3.814 | 5.0 | 17 |
| hermit-20 | 5.330 | 5.271 | 5.6 | 17 |

### Source lines by audited head

The counts include tracked Rust and WGSL under each crate’s `src/`, with blanks and inline tests. Functional L0 counts executor code across crate boundaries.

| Crate | Retained main | v1 | v2 | v3 |
| --- | ---: | ---: | ---: | ---: |
| loft-abi | 498 | 498 | 498 | 498 |
| loft-cli | 5,066 | 5,182 | 5,066 | 4,307 |
| loft-format | 815 | 815 | 815 | 815 |
| loft-ir | 548 | 548 | 548 | 548 |
| loft-parse | 2,723 | 2,723 | 2,723 | 2,723 |
| loft-passes | 16,456 | 16,670 | 16,519 | 16,734 |
| loft-plan | 949 | 949 | 949 | 949 |
| loft-runner | 1,630 | 1,630 | 1,630 | 1,998 |
| loft-std | 4,321 | 4,321 | 4,321 | 4,321 |

## Wave integration on 20 November

The Apple M4 Metal gate on `9878c6e` passed the merge rule for w1–w4. Twin held 36/36, Truth 34/34 and Speed 12/12, with zero contention. All six mutants fired. The runner passed wasm32 with web-sys, and Earley and llguidance each returned 280/280 expected outcomes, including the negative sentinel. Frozen hashes matched before and after for 18 gate frames, nine bead frames and 1,510 legacy beauty images. All nine bead geometry checks passed and their RGBA matched the archived release. [Gate evidence](../../out/meta/iw/gate.json) · [Contact sheet](../../out/meta/iw/sheet.webp)

Pictures held 0/18 against the frozen references. All 18 frames belong to the existing layered-material review queue. At most 8 pixels per frame differed from its queued proposal by more than two codes. The separate comparison against freshly rendered main frames found exact RGBA on 15/18. Capsules-20 accounted for all remaining changes. Load held 31/33, with Hermit at 14 pipelines against 13 at both counts. [Pixel comparisons and hashes](../../out/meta/iw/pixels.json) · [Owner review queue](../../out/meta/i6/approval_needed.md)

| Lane | Measured delivery and work still open |
| --- | --- |
| w1 · Authored edits and motion | All four positive edit probes matched independently compiled images at every pixel and had zero foreground pipeline creations. The rejected name returned E011. The general tape and literal edit exceeded the paired GPU budget in this run. The table below gives the individual measurements. The 18 emitted diagnostics in E101–E503 have snapshots. E201 and E308 still need emitters and fixtures. The bound HeronHop crouch reached 0.59348 m in the lane probe. The three shipped characters still execute laws. Their 0.5 s images matched main, and full authored solver/plan execution and bit-equal twins for all three remain open. [w1 audit](lanes/w1.md) |
| w2 · Shrine and capsule ties | Shrine measured **3.904 ms p95** against 4.000 ms at 1280×800, after 30 warmups over 100 frames. Median Shade was 0.313 ms. The unmodified main and candidate beauty renders differed at 65 inherited variable pixels. Every candidate value occurred at that coordinate in a baseline replay. The separate full-size comparisons at identical ascending and descending cull orders were byte-identical. Depth matched exactly. Capsules-20 selected the lower row at every witnessed tie and reproduced the independently recorded beauty and ID hashes at 0, 4 and 9 s. [w2 audit](lanes/w2.md) |
| w3 · Native tests and executor | All 38 inherited failures now pass. The combined workspace ran 150 native tests across 25 executables, with zero failures or ignores. Bun passed 315/315. Native and embedded paths share raster preparation and command recording. `Loaded::prepare_passes` still selects commands from its source schedule. Full shared preparation requires portable per-draw bindings, dirty bakes and path resources. The embedded host drew two frames and passed both depth conventions. [w3 audit](lanes/w3.md) |
| w4 · Core declarations | Four archived bundles round-tripped byte-for-byte. Removing the manifest and table schema mirrors reduced the four owned crates by 17 lines, from 2,827 to 2,810. Functional L0 alone measures **5,447** on the combined tree. Including IR gives at least **5,995**, against the 1,500-line L0+L1 target. [w4 audit](lanes/w4.md) · [Functional line accounting](../../out/meta/iw/l0.json) |

| Authored edit | Source to final upload, ms · cap 2 | Paired GPU overhead p95, ms · cap 0.5 |
| --- | ---: | ---: |
| Literal | 0.480 | +0.592 |
| Structural | 0.479 | +0.252 |
| Loop | 0.481 | -0.021 |
| General | 2.133 | +4.554 |

| Speed case | Main p95, ms | Integrated p95, ms | Cap, ms | GPU commands |
| --- | ---: | ---: | ---: | ---: |
| capsules-1 | 0.526 | 0.460 | 0.8 | 5 |
| capsules-20 | 0.643 | 0.720 | 1.1 | 5 |
| blobs-1 | 0.501 | 0.512 | 1.1 | 5 |
| blobs-20 | 1.506 | 1.489 | 2.4 | 5 |
| hermit-1 | 3.921 | 3.820 | 5.0 | 17 |
| hermit-20 | 5.451 | 5.330 | 5.6 | 17 |

Physical tracked Rust and WGSL lines under each workspace crate’s `src/` include blanks and inline tests. The separate functional L0 count also charges execution in Std and CLI.

| Crate | Main | Integrated | Change |
| --- | ---: | ---: | ---: |
| loft-abi | 498 | 498 | +0 |
| loft-cli | 4,948 | 5,066 | +118 |
| loft-format | 867 | 815 | -52 |
| loft-ir | 513 | 548 | +35 |
| loft-parse | 2,723 | 2,723 | +0 |
| loft-passes | 15,185 | 16,456 | +1,271 |
| loft-plan | 949 | 949 | +0 |
| loft-runner | 1,585 | 1,630 | +45 |
| loft-std | 4,321 | 4,321 | +0 |

## Earlier alpha and runner-cut measurements

The alpha uses the lead’s release rule. The locked Apple M4 Metal run on `07ac562` held Twin 36/36, Truth 34/34 and Speed 12/12, with zero contention. All six mutants fired. The runner typechecked for wasm32 with web-sys. Both grammar recognizers gave 280/280 expected outcomes, including the negative sentinel. The 18 gate hashes, nine bead pins and 1,510 legacy beauty hashes matched. The six scenes used 5/5, 5/5 and 17/17 commands at counts 1 and 20. Their 300-frame p95 times were 0.5005/0.653 ms for capsules, 0.456/1.506 ms for blobs and 3.911/5.453 ms for hermits. [Release measurements](../../out/meta/rel/gate.json) · [Five-gate log](../../out/meta/rel/baseline.log)

### Runner and edits

| Target | Alpha measurement and work remaining |
| --- | --- |
| Runner size and shared execution | 3,758 physical lines against 1,000 before counting the separate native executor. The 16 runner tests passed and an external host drew two frames. Native source and embed still need shared bake/path resources, cross-caller pixel and state parity, and browser frames. Three- and four-section persisted layouts both report ABI 1. |
| Tier 0 authored edits | Literal, structural, bounded-loop and rejected edits all exited 6 at WGSL line 140 on the U+FDD0 source marker. The three independently compiled controls rendered. The manual GPU fixture uploaded in 0.021 ms, added 0.490 ms p95 and had 0/140,000 pixels above 1/255. The public path still needs source-to-upload ≤2 ms, zero foreground pipeline creations, overhead ≤0.5 ms, field and image parity, typed rejection and browser promotion. |

#### Runner cut

The workspace build and external host passed. All five locked gate scores and individual verdicts matched alpha. The
strict release-frame comparison held **16/18**. Capsules-20 differed at two pixels at 0 s and one at 9 s. The 4 s frame
matched in the gate. Both archived binaries reproduced both variants at 4 s and 9 s in four fresh renders per time.
The alpha binary also reproduced the exact two-pixel variant at 0 s in three of 16 replays. The alpha ID AOV
alternated between rows 63 and 64 at 4 s, and between rows 63 and 69 at 9 s. The observed row switches are consistent
with the unchanged cull shader's atomic survivor append and the strict Less depth test, which keeps the first hit
when stored depths tie. W2 subsequently established the deterministic lower-row policy and the hashes reported above.
[Strict comparison](../../out/meta/fxcut/comparison.json) · [Replay evidence](../../out/meta/fxcut/probe/comparison.json)

`Loaded::prepare_passes` still translates source-program passes while the embedded runner prepares persisted
`PassDecl` records. Both record through `command::Pass::record`. A single preparation path must carry native
load and dirty bakes, the path/photon resources and edit-time bundle refresh. Portable lowering still refuses programs
with bakes or path resources. The native load and pass preparation span over 400 lines and the shared route requires
changes to the resource contract and edit path, beyond the 150-line cut.

D1 charges device setup, binding and attachment validation, pipeline preparation, and command recording and transfer
where they live. Count physical tracked Rust/WGSL lines including blank lines and inline tests, following runner
source inclusions and excluding its two standalone test modules. The Std subtotal includes the device-facing parts
of mixed modules, with CPU-only pick decoding and the bake dirty clock excluded. Native adds device execution
including the separate source renderer, bakes, path, edits, motion readback and hardware ray setup. The camera
matrix helpers, instance packing and CPU motion decoding are outside that count.

| L0 source | Physical lines |
| --- | ---: |
| Runner | 983 |
| Execution and preparation moved into Std | 2,429 |
| Pre-existing Std pipeline worker | 67 |
| Native CLI device execution | 1,938 |
| **Total** | **5,417** |

Runner plus displaced Std work alone is **3,412**, or **2,412 over** the 1,000-line ceiling. Including the worker
and native execution, L0 is **4,417 over**. Std grew from 1,772 to 4,321 lines and the CLI shrank from 5,246 to
4,948 lines under the crate-wide count. [Per-file source spans](../../out/meta/fxcut/l0.json)

### Motion

| Target | Alpha measurement and work remaining |
| --- | --- |
| Authored solvers and plans | Scheduled solver and open HeronHop both exited 1 with E405. The closed prior clip ran at 0.553 ms p95. Stage 0 asset binding for `near`, `clear` and `land_on`, colored constraints, solver events, world-space pinning and support hulls remain open. |
| Full motion model | Scalar Crab replay matched all 481 ticks. Its 1,000-row kernel measured 0.011 ms p95 against 0.150 ms, with a 0.047 ms control. Full BODY/STATE/POSE execution remains open. The earlier Boss measurement was 76 bytes of active state against a 2,928-byte state and 3,072-byte pose requirement. |

### Rendering

| Target | Alpha measurement and work remaining |
| --- | --- |
| Coral Shrine close view | Restoring the shadow map reduced p95 from 19.240 to **4.210 ms** against **4.000 ms**, at 1280×800 after 30 warmups over 100 frames. Median Shade fell from 16.019 to 0.544 ms. Control was 0.039 ms. The woodblock comparison had 73 pixels above 2/255 out of 1,024,000. The per-hit shadow proposal is excluded from this alpha. |
| Hermit load pipelines | Load held 31/33. Hermit at both counts created 14 pipelines against 13. Its indexed shell uses a load-only bake. All 6,369 UV vertices and 36,864 corners produced matching procedural and indexed images for beauty, depth and ID. |
| Bake completeness | This run completed the 20 load dispatches in 1.371 ms against 2.000 ms. The scored integration took 3.748 ms. Dirty dispatches were 5/0/0/5/0. Every-mip texel parity, vector parameter invalidation and portable bake descriptors still need verification. |
| Transport and viewer | The four-sample spectral study rendered at 128² on the eight-slot device. Dielectric transmission, dipole/random-walk SSS, Gaussian media, prior-frame reservoir reuse, reprojection and scheduled bloom mips remain open. |

### Corpus and interoperability

| Target | Alpha measurement and work remaining |
| --- | --- |
| Corpus certification | **0/295 certified**, 281 required. All 187 candidate hashes matched and all 187 clips met 50 ms, with a maximum p95 of 14.100 ms. There are 108 missing candidates and 186 candidates without pinned geometry. The 107 scratch observations cover 15 identities. Source checks, 14 reduced previews and between-key seed motion still need evidence. |
| Bead export | Bead passed 9/9 frozen geometry frames with at most eight isolated pixels above one code, zero off-edge pixels and zero depth failures. Bundle export exited 1 because the screen-path resource needs a portable descriptor. |
| glTF import time and coverage | The optimized 100,000-triangle import took **22.135 ms** against **100 ms**. D1d’s debug runs took 331.740 and 557.989 ms. Two-primitive packing succeeded, but D1d’s primitive-color probe failed. Textures, skinning and animation remain open. |
| Retargeting | The positive walk exited 1 with E601. Planted-toe drift remains unmeasured against 0.1 mm. |
| Hardware rays | The portable probe returned 1 m and −1 for hit and miss. The hardware probe exited 1 with one acceleration structure requested against a per-stage limit of zero. Compiled World traversal still refuses mesh geometry. |
| Embedding and replay | The standalone host drew two frames and passed both depth conventions. Fresh replay reproduced the recorded image. Pinned replay exited 5 on bundle identity. Live slots, post-start input, cross-device replay, Keel/browser frames, dynamic host viewport and portable ASSETS remain open. |

### Workspace tests and appearance review

Bun passed all 315 tests. Native tests recorded 108 passes, 38 failures and zero ignored. Of those failures, 36 read `lib/migration/` as a file. The Shrine test prelude lacks `loft_user_paper_grain` (E701). The custom Radiance medium fixture returns E405. [Named failures](../../out/meta/rel/gate.json) · [Native logs](../../out/meta/rel/tests/)

The full TypeScript suite passed **315/315** after provisioning the legacy `loft-node` and `loft-draw` lab binaries.
The runner-cut recheck ran all 25 native test executables and reproduced **108 passed, 38 failed, 0 ignored**, with
the same failing test names. The 36 directory-read failures comprise ten tests in `bakeoff`, 25 in `diagnostics`
and `tier0::tests::l23a_sphere_tape_matches_the_e_plus_twin`. The two other failures are
`coral_shrine_has_analytic_branches_and_two_lantern_snapshots` (E701) and
`checked_contract_reaches_lowering` (E405). W3 subsequently repaired all 38 cases.
[Named cut failures](../../out/meta/fxcut/comparison.json) · [Cut test logs](../../out/meta/fxcut/tests/)

All 18 layered frames exceeded frozen picture tolerance. Each independent still was within one pixel above 2/255 of its queued Phase 4 proposal, against an allowance of 1,024. The woodblock look and 187 corpus beauty pairs across 17 batches, including the revised bead proposal, remain queued for owner review. Texture bundle serialization still needs a format decision. [Release image comparisons](../../out/meta/rel/picture-diffs.json) · [Original appearance queue](../../out/meta/i6/approval_needed.md) · [Format request](../../out/meta/i4-r2/approval_needed.md)

## Performance carry-overs from the 18 Nov cook (lead, 20 Nov)

- **Pipeline cache plus warm-up during loading** (spec flag `PIPELINE_CACHE`). Metal compiles about 70 ms per pipeline. The old lane (agent_notes/2026-11-18_cook/pipecache.final.md) activated retained pipelines in 0.08–1.28 ms with byte-identical frames. Target: cold load of hermit-20 dominated by the cache import, and warm load unchanged.
- **Solve cache:** keep the 128 most recently used verified `solve` results (agent_notes/2026-11-18_cook/small.final.md).
- **Volume march budget** before real-time media: the old dust volume cost 20–61 ms with shadow marches (volume.final.md). Bound the step and shadow counts, and add them to the Speed budget.
- **Not carried:** f16 gave no gain on M4 (f16.final.md); f32 stays the default.

## Frame-cost directions (owner → Astra, 20 Nov)

Command count is not the bottleneck: about 5 commands per frame, roughly 75 µs. The initial hypothesis attributed Hermit-20 at 5.46 ms and Shrine at 3.90 ms to per-pixel shading. PF1 below measures Shrine’s primary visibility pass as the main cost. In order:
1. **Shading cost:** march steps, BSDF cost and memory traffic per pixel, from naga telemetry plus per-pass timings.
2. **Pass fusion for shading:** remove barriers and memory round trips between passes, as already done for motion ticks.
3. **Apple tile memory:** keep the vis-buffer on-chip between raster and shade. wgpu cannot do this; it needs a native Metal backend. See the design question bk1.
4. **Overlap:** QueueClass::AsyncCompute (the seam exists; wgpu support is limited).
5. **Persistent megakernel dispatches,** only for chains of many tiny steps.

## Performance playbook (chief scientist, 20 Nov), to be measured first

The scientist's ranked playbook is in agent_notes/2026-11-20_v1/scientist_perf_playbook.md. Its impact figures are estimates, not Loft measurements. Measure before building (pf1):
- **Tier 1 (portable):**
  - trusted shader modules without runtime bounds checks (`ShaderRuntimeChecks::unchecked()`) on compiler-proved passes;
  - per-kind tile lists with indirect dispatch;
  - coarse-to-fine raymarch with over-relaxation and split Lipschitz;
  - fuse shade with tonemap and post;
  - hot/cold SoA table split and f16 shading. Note that f16 gave no gain on M4 in the 18 Nov test (f16.final.md), so re-measure per pass.
  - bake noise, masks and IBL.
- **Tier 2:** hardware RAY_QUERY, half-resolution secondary effects, a non-inlining policy for large helpers, FMA kept on in shading.
- **Tier 3 (native Metal, after Tiers 1–2 are measured):** tile memory, async compute, ICBs and binary archives.

## PF1 · M4 frame cost measured on 20 November

At 1280 × 800, the 300-frame Hermit-20 orbit measured **5.585 ms p95** and the 100-frame fixed close Shrine measured **3.958 ms p95**. Both triangle controls were .096 ms. Pass medians in Cull / Vis / Shadow / Classify / Shade / Present order were **.004 / 1.417 / .649 / .000 / 2.188 / .173 ms** for Hermit and **.009 / 2.150 / .461 / .000 / .397 / .189 ms** for Shrine. Classify overlaps other work.

- **Hermit is lighting-bound.** Omitting the twelve sky samples saved 1.542 ms of Shade. Expanding the same fourteen lighting iterations saved .356 ms of Shade but changed 13–29 pixels. The covered traced pixels at t=0 took 9 steps at the median, 23 at p95.
- **Shrine is visibility-bound.** The folded lantern and analytic branches independently accounted for 1.066 and .957 ms of Vis in omission probes. Lantern hits took 129 steps at the median and 182 at p95. Halving L=22.871492 saved .264 ms Vis but changed 11,705 pixels in the close view.
- **Two playbook premises are resolved.** Shade already uses per-kind indirect tile lists (1,238,848 / 759,168 Vis reads at t=0). Shading MSL permits FMA contraction, confirmed numerically. HDR is materialized. Physical DRAM bytes and hardware divergence remain unknown. Only timestamps are exposed here.
- **Independent gate check.** Hermit-20 reached 6.775 ms p95 in the later five-gate run with the same renderer and a .096 ms triangle. All 18 decoded frame hashes matched previously scored main. The timing spread is material to the lane p95 targets.
- **Two Sol lanes.** Reusing the static sun map saved .564/.459 ms of frame p50 on Hermit/Shrine. Four checked march steps per outer iteration saved .384 ms of Shrine Vis with exact fixed-order images, but the all-ray treatment regressed Hermit frame p95 by 2.046 ms. The second lane targets the long tail after 24 scalar steps, predicting .31 ms on Shrine and .01 ms on Hermit. [PF1 report, raw timings and AOVs](design/pf1.md) · [Lane IDs](../2026-11-20_codex/pf1.lanes)

## Source-authored WGSL compute passes

A scene author should be able to schedule one complete WGSL compute entry against a table they declare. On `fc7b5ed`, `loft render` rejects a valid custom schedule pass with E405 before loading the shader. The same module runs through `Runner::load_bundle_bytes`. Give the existing source `pass` a module path, entry, dispatch and binding-to-table map, then lower it in schedule order to the existing `ModuleBlob` and `PassRun::Compute`. Use the linked pass and explicit logical binding ranges for both CLI execution and .loftb replay. [CLI and runner probe](../../docs/proposals/2026-11-20_backends_and_escape_hatches.md#scientist-checks)

**Acceptance.** From a clean checkout, `loft render` dispatches the authored `stamp` entry once with two zeroed u32s at group 2, binding 5. The GPU returns `[42, 0]` for `answer[0] = 40u + arrayLength(&answer)`. An exported and reloaded bundle returns the same words using a Loft-owned table. The current M4 runner returns `[43, 0]` on that allocation. A second control may replace Classify and all per-kind Shade dispatches with one full-screen launch. Verify the output and charge all dispatches, conversions and intermediate traffic to the whole-pass Speed comparison.

## Integration and hand-off (20 Nov)

Seven lanes, each with an acceptance test, in agent_notes/2026-11-20_v1/INTEGRATION_TODO.md:
1. Three.js demo
2. Unity Metal spike
3. Lighting adapter
4. glTF export with a loss report
5. Material export as surface functions
6. Motion CPU twin as a library
7. Room Runners merge (owner)

Items 1 and 2 come first.

The X2f size audit and four proposed cuts are recorded in [the lean-down plan](design/lean1.md).

## Chief scientist's handoff (20 Nov, scientist_handoff_2.md): routing
- loft/s1-lip: Lipschitz fix, Shrine Vis 0.94 ms faster. **Owner approval needed** for out/meta/s1_lip/shrine-review.webp (the old picture was a truncated march). Merged by ix3 once approved.
- loft/s1-controls: parity gate prototype; polished by s1_controls_fix (wave x3).
- s1_escapes: the four front-end v1.2 stubs (bake_tex sampling, kernel allow, whole-pass `pass`, unproved). Runs in wave x3.
- Lean audit (x2f) and lean lanes must also cover:
  - remove the wgpu dependency from loft-std (~2,429 lines of device/encoder code; a duplicate 1,938-line renderer in loft-cli/src/native/);
  - per-frame create_buffer_init in encode.rs:228–235 and outbox.rs:66–70;
  - U::VERTEX on LUTs (lib.rs:47);
  - .loftb export refused for bakes and paths (bundle.rs:325).
- x2b: keep FMA ON in shading (Fast mode). A shading contraction ban costs +0.18 ms p50 and +0.86 ms p95; global Safe mode costs +4.68 ms. Prefer a scoped motion-only barrier over a soft-float emulator. Add an automated MSL leak check.
- X2e now runs the Hermit demo schedule on the Three.js device. The browser adapter is scoped to that schedule.
- Deferred: the native MetalBackend (predicted 0.09–0.19 ms for 2.3–3.8k lines), HDR+Present fusion, f16 shading, ground fBM bake, and consumer Wasm passes until check.rs (86.5 KB) is split.
- Superseded briefs: s1_exec, s1_motion_fma, s1_web, is1 and sci_chain.sh, which are covered by x2a, x2b and x2e.

## Std library rule and anti-patterns (owner, 20 Nov)
- A std function earns its place when it replaces a recurring, sufficiently complicated block costing ≥3× the tokens of its call (PHILOSOPHY §4).
- **Anti-pattern (seen in Buddies expeditions):** re-implementing standard functional patterns (`map`, `await`, filter, fold) as tiny renamed wrappers that cover one or two calls under a new name. That adds names and saves nothing. Std functions are LARGE, real blocks: a leg family, a shell generator, a layered material, a gait law, a camera rig. Never a renamed one-liner.
- Std today: lib/*.loft, 607 lines and 9.5k tokens (the scene-side std); crates/loft-std (Rust host side, 4.3k lines). Every build re-parses all of lib/ (cost within the 20–37 ms build). AOT plan: stdaot1.

## Flagship scenes not yet ported (owner, 20 Nov)
- The 5-crab idol lineup dance (examples/bosses/crab_lineup.ts, lineup_shots.ts; song score crab_song.ts), the crab_live motion (examples/motion/crab_live.ts, stage.ts), the bosses (wreckers_lamp, reefback_titan, maelstrom_wyrm, thundermaw, tithe_fleet), and the first lanterns. engines/hermit.loft exists; the "nice hermit" look must match the best old render. Lane: port1.
