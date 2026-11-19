# D1c · Authored edit to first usable frame

## Prediction before implementation

The recorded 12-frame p95s are 3.124958 ms editable and 0.682833 ms specialized, a 2.442125 ms gap. The reported 0.063666 ms times queue writes after source compilation. Predict 1.2 ms from receipt to final queue write (2 ms cap), zero foreground GPU pipeline creations and a +0.35 ms GPU p95 difference (+0.5 ms cap). Predict field absolute error at most 1e−5, at least 99.9% of RGB pixels within 1/255, zero clean-kind pipeline creations and zero stale installs. Score at 1280×800 with the same camera and the gate's rounded-rank p95 over 12 frames per tier. Also record paired per-frame stamps and repeated values.

For an authored `sphere(p - shift, radius)`, the tape should be 3, 2, 9, 1, 42, 55 (input, row, subtract, constant, sphere, return), with two live registers. At zero shift, both edits should certify Lipschitz 1 and enclose the sphere at the original proof grid's ±0.625 m then ±0.6875 m, before outward padding. One radius slot, one proxy row and one instance bound change. A shadow-casting kind changes the derived caster camera if it extends the caster union. Generation advances by one only after accepting its writes. A box composition changes the tape and reaches the VM. A rejected edit leaves the latest valid generation available for drawing and promotion.

Shared imports `b92f8c057585a476d9b1d9fec82d94f114f6de0b` contain both scored Phase 4/5 tips. The lead override starts this lane from `d4b7646` through the shared merge with `origin/main`. Primary main at `8fd710db37d4b1d701f6b0d4cd97accf9b84e434` lacks that commit. The m23 Speed miss is recorded at `out/meta/m23/BLOCKED`.

## Delivered source

Implementation `7c57b9a` changes the authored tape and render adapters in `crates/loft-passes/src/tier0.rs`, `tier0_author.rs`, `tier0.wgsl`, `tier0_tests.rs` and `tier0/`. The authoring session retains the checked program, source regions and row layout. It admits the trace and private trace helper bodies, keeps existing helper signatures and rechecks those changed bodies and the trace. A constant-pool or tape edit recomputes the dirty kind's bounds and prepares aligned updates. An incompatible source change or checked error returns a span. In the authored shader, the sphere branch selects operands before marching and reuses them for shading. The fallback shares the 56-op implementation with the compute VM. Authored resource-slot lowering remains open for LUTs and sampled textures.

`crates/loft-cli/src/edit_cmd.rs` and `native/edit.rs` time source receipt through the last queue write, record stage timings and count edit pipeline factory calls on the foreground and worker threads. The native command has a 12-frame p95 per tier, an alternating 12-pair diagnostic, field readback and clean pipeline handle checks. `crates/loft-std/src/promotion.rs` owns quiet-period and generation checks and the authored path's LRU. `crates/loft-cli/tests/d1_edit/` includes byte-identical radius inputs, a structural box, a bounded loop and a rejected trace. The runner's inherited manually registered edit route still has its own pipeline-job and cache plumbing.

The integration seam commit `dd9b825` adds an edit-only hook in `loft-passes/src/emit.rs`, exposes the shared promotion policy through the runner, and preserves the separate manual probe's entry when including the split WGSL. It also records standalone repairs in `loft-runner/src/encode.rs` and `stepping.rs`. These paths belong to d1a/d1b. The commit keeps those hunks separate for i6 to reconcile. The lane cherry-picked d1a's three import repairs as `95d56bd`, `75619c1` and `bd3ed7c`. D1a published the common base and subsequently escalated its Layer 0 size and native verification gates. Its native source render still uses the separate Loaded executor.

## Observations and escalation

**ESCALATE.** The lane reached the two-retry limit before Cargo emitted the lane CLI or std test binary. The two follow-up fixes for the final diagnostics are recorded in `dd9b825` and have no confirming compile.

| Invocation | Observation |
| --- | --- |
| Requested `CARGO_BUILD_JOBS=2 tools/dev.sh build loft-cli -p loft-passes -p loft-std --bins --tests --message-format=json` at `bd3ed7c` plus the lane diff | Inherited double `pub(super)` on `Shader` in the runner encoder. Cargo emitted the passes test executable. |
| First retry, with `-p loft-runner` to emit the separate manual regression | Removing the duplicated visibility let both runner compilers reach the missing `ShaderStages` alias. |
| Second retry, with the alias restored | The inherited `advance_into` passed 15 arguments to the 16-argument encoder and `embed.rs` accessed private `Group::slots`. |
| Emitted `loft_passes` tests filtered by `tier0` | **4 passed**. The full frozen ID dispatch, CPU reference including resource and loop semantics, 32-register limit, and manual sphere tape versus the E+ CPU twin passed. |
| Independent Tier 1 frontend smoke check with the archived d1a binary | Both unchanged literals, the structural box and bounded loop checked. Rejection returned E011 at line 2, column 32. |

The authored residual, generations, row writes, stage timings, new field and pixel deltas, actual foreground pipeline calls, clean-kind identities, stale installs, and both GPU p95 statistics remain unmeasured. The predicted radius residual and bounds above are the static forecast. The new authored CPU test and promotion test had no emitted executable. The public GPU commands are staged as `out/meta/d1c/run.sh` and await the lane binary. No reference, tolerance, golden or manifest changed. No visual change was submitted for approval.

The separate 140,000-sample historical regression measured **0.027792 ms** upload and **0.448499 ms** paired p95 overhead at i23-r2, with **0/140,000** pixels over 1/255. At m23 it measured **0.029083 ms**, **0.544209 ms** and **0/140,000**. Neither observation is a D1c authored run.

The browser target remains `wasm32-unknown-unknown` with `web-sys` on the BrowserWebGpu backend. The time-injected generation policy is portable. The browser route still needs to await `GPUDevice.createRenderPipelineAsync` and `createComputePipelineAsync`, and install the resulting handles only for the matching generation. Pinned wgpu 30.0.1 exposes the underlying device and lacks a constructor for adopting those browser pipeline handles. Browser completion and stale rejection were unverified.

Primary evidence is under `out/meta/d1c`. The exact authored inputs, runner diagnostics, CPU test transcript, source diff and executable digest are preserved there. The merged suite belongs to i6.
