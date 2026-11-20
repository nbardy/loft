# Loft compiler speed, 18 Nov 2026

Keep TypeScript authoring and the TypeScript compiler for training. Keep `rust/loft-loader` as the game's wgpu
executor. Give the Linux baseline in [the plan](2026-11-18_next.md) a compiler breakdown before funding a port.
On this M4, WGSL emitter time was 11–114 CPU ms per build. Graph construction, table work, and an authored
motion solve cost hundreds to thousands of CPU milliseconds. They would still run after a WGSL-only port.

## What was measured

Five existing scenes cover a smaller creature (owl), a large creature (Reefback Titan), a custom PBR part
(thin-film fresnel), a soft medium (storm), and a `solve` run (heron leap). Each `serve` measurement is the
GPU-less `loft build` child of an actual server. Results exclude the server's GPU device, shader/pipeline load,
draw, and the timing wrapper. The four successes produced the same bytes through both CLI routes. Storm runs
through lowering and then exits 4 at `writeBundle` because its dynamic lighting and cluster tables use JavaScript fills.
Its times describe a refused compile, so it contributes no successful artefact or write time.

| Scene | `build` wall / CPU ms | Peak MiB | `serve` child wall / CPU ms | Peak MiB |
|---|---:|---:|---:|---:|
| Owl | 477 / 627 | 153 | 510 / 576 | 154 |
| Titan | 3,523 / 3,872 | 508 | 6,305 / 3,791 | 488 |
| Thin film | 239 / 302 | 117 | 442 / 322 | 114 |
| Storm, exit 4 | 1,661 / 1,298 | 245 | 2,600 / 1,364 | 246 |
| Heron leap | 6,169 / 4,108 | 999 | 22,096 / 6,154 | 999 |

These are medians of three phase-instrumented cold child processes per scene and route. CPU is user plus system
time across threads. One-minute load was 30–57 on a shared 10-core Mac. Use the CPU breakdown for prioritising
work and the wall figures for an observed range only. The compile command and output directory protocol are the
same on both paths for these scenes. The wall gap has strong scheduling and memory-pressure confounds. The
[measurement notes](../../agent_notes/2026-11-18_rustplan/NOTE.md) give attribution, ranges, and the uninstrumented comparison.

At four compile slots, summing these per-child peaks gives a derived sizing figure of about 2 GiB for four Titans
and 3.9 GiB for four heron solves, before the GPU parent. Measure whole-slot memory on Linux to account for shared
pages and peaks that occur at different times.

The two Rust estimates below use the `build` CPU medians. “Lower + emit” includes folding, rate planning, leaf
binding, shader text and host descriptor/shader-sharing work. “Tables” adds table classification, layout,
rest evaluation and level-of-detail measurements. Some of it calls the JavaScript twin and remains on the JS side in
a partial port. The timings partition execution by recorded scope. Medians of individual phases can differ from
the median of their sum.

| Scene | Start + import, CPU ms | Lower + emit, CPU ms | Plus all tables, CPU ms | CPU saving if lower + emit run 3× faster |
|---|---:|---:|---:|---:|
| Owl | 155 | 142 | 295 | 95 ms (15%) |
| Titan | 496 | 1,295 | 2,148 | 863 ms (22%) |
| Thin film | 131 | 69 | 103 | 46 ms (15%) |
| Storm, exit 4 | 249 | 425 | 793 | 283 ms (22%) |
| Heron leap | 135 | 192 | 411 | 128 ms (3%) |

The 3× column is a scenario, calculated as `2/3 × phase CPU`, before transfer, validation or duplicate allocations.
Use it as a budget for a prototype. Even if all lower, emit and table CPU vanished, the
derived floor would be about 332 ms for owl, 1,724 ms for Titan and 3,697 ms for heron. Heron's authored `solve`
alone registered 3,323 CPU ms and raised the process high-water mark by 916 MiB in the build medians. Its sample
profile points to the solver's rows/derivative evaluation and numeric kernel. Treat solver optimisation and its
existing exact in-process cache as a separate lever. The cache can help identical requests and requires evidence
of hits before counting it for generated scenes.

## What Rust would buy

**A compact arena behind the TypeScript graph is plausible.** A data-only node record of 32 bytes would take
about 5.6 MiB for Titan's 182,894 created expression nodes. That is a derived subtotal excluding argument edges,
intern indexes, strings, table data, JS callbacks, the VM, and duplicate representation while sending. The same run
encountered 6.28 million instrumented interner entries (several per constructed expression) and reached hundreds
of MiB of RSS. A dense node layout and reusable traversal buffers could first be tried in TypeScript to isolate
the layout gain. As a scale check, a boundary crossed 6.28 million times at a hypothetical 30 ns CPU per call
would itself cost 188 ms. Batch transfer at a program boundary and measure Rust plus Bun CPU and tree peak RSS.

**A backend port needs a real boundary.** Today styles, drawables, map/scan bodies, and `iterate` steps are
callbacks that create nodes during lowering. `iterate` retains the step so a rewrite can replay it, and `LibFn`
can carry both author-supplied WGSL and a JavaScript CPU twin. Static table fills and rest bounds can execute the
twin. The small candidate is a kernel over already-expanded ordered roots, slots and table identities. It must
carry numeric bits and original IDs, preserve evaluation and declaration order, and return source sites for errors.
Keep JS evaluation where a kernel needs the author's callback. Moving the entire backend requires first staging
those callbacks and their symbolic dependencies as data or providing explicit callbacks across the boundary.

**Engineering budget, estimated.** The checked-in source census is about 4.8k TypeScript lines for `expr.ts`,
`compile/`, and host `lower`/`share`/`files`, with `scene`, `artefact`, manifest and author callbacks on the boundary.
Solver, gradients and interval analysis add 2.7k lines before their libraries. Budget 3–5 engineer-days for one
packed-input kernel with a byte/CPU/RSS harness, 3–6 engineer-weeks for a maintained backend through parity on
loops, tables, slots, custom WGSL, checked errors and both OS targets, and at least 2–3 engineer-months for a
replacement that also owns graph construction and CPU semantics. These are planning estimates for implementation
and differential verification, with no replacement authoring language included. Maintain the JS backend until
the entire supported corpus and typed failure cases agree.

wgpu already executes Loft's data format. Compiling with wgpu would move the later shader translation and pipeline
work to a different runtime. Loft WGSL generation and the driver work are separate budget lines. The wgpu WGSL
input still goes through parsing and validation. A game-side translation speedup needs an independent renderer
benchmark on the same adapter, shader inputs, cache state and frame count. [wgpu shader source](https://docs.rs/wgpu/30.0.1/wgpu/enum.ShaderSource.html)

## Alternatives

| Approach | Where it pays | Cost and first limit |
|---|---|---|
| Warm compile worker | Reuses Bun startup, library import/evaluation and possibly code/twin JIT. On this corpus the start + import ceilings are 131–496 CPU ms for `build` and 140–545 ms for the `serve` child. Some imports depend on the job. | 2–3 days for a trusted ceiling and one-job confined prototype. Production reuse takes longer if the runtime cannot supply a pristine job context. Today expression IDs occur in emitted WGSL, and module state, interners, library registrations and folded nodes are process-wide. Warm jobs need clean identities and trust isolation in addition to memory recycling. |
| Graph snapshots | Pay authoring once and replay across backends or on a later run. Good for a fixed scene used for more than just rendering. | 1–2 weeks for a restricted, versioned round trip. A first-time program still executes author code and also pays to encode. Snapshot expanded typed data after callbacks, record leaves/libraries/slots and source metadata, and name opaque CPU callbacks or other graph features that require live authoring. For render retries the existing artefact, measured at 92 KiB to 3.54 MiB here, is already replayable. |
| Exact structural cache of lowered modules | Pays on equal programs across different scenes, potentially shared water/style/geometry. Can skip fold, plan and emit on a hit where binding and stage context agree. Existing `serve` GPU-object caching saves later shader/driver work. | 3–5 days for a trace-only hit census and one module family. A key must account for typed ops, ordered children, literal bit patterns, loop binders, row/table data and identity, slot layout, raw WGSL and dependencies, rates, and compiler/format version. Rematerialise creation-order WGSL names on a hit or use exact names in the key. Admit only dependencies the compiler can describe. |

A structural lookup still pays import and graph build. Derive its break-even from `hit rate × (miss backend cost −
hit cost) > hash + cache bookkeeping`. For example, avoiding 100 CPU ms on a hit and spending 20 ms on lookup
and bookkeeping on every job breaks even above 20% hits. Saving 50 ms/job requires at least 70% hits at those
assumed costs. Measure distinct programs in shuffled order, separating exact repeats and common subgraphs.
Use a bounded, versioned cache and compare full artefact hashes, including after restart. A plain filename or a
hash of the scene text alone cannot account for dependencies and effects of arbitrary TypeScript.

## First experiment and decision

1. Run step 6 on the intended Linux/Vulkan training machine. Use the same pinned Bun/Loft revision and existing
   confinement for the fresh-child and candidate arms. Use 100 distinct generated programs, the plan's existing
   programs, these five diagnostics, and a separate repeat cohort. Randomise pair order, hold compile concurrency
   constant, and record child CPU/wall/peak RSS, whole-slot memory, and shader/driver seconds separately. Record
   medians and p95s, plus cold and warmed driver caches. Score the existing ≥60% steady-state shader CPU gate
   before choosing between the worker and tier-I work in the plan.
2. If compiler work is the lever, measure a warmed TS front end first. Compare a fresh confined subprocess, a
   warmed trusted process for the ceiling, and a fresh confined job using cached trusted library code (the
   transpiler cache and, if dynamic module identity and the CPU twin stay intact, precompiled bytecode). Bun threads share I/O
   resources. The production candidate must have a fresh one-job trust boundary, the same resource and deadline
   semantics, GPU-less compile, and identical bundle SHA-256 and typed refusal results for every program. Use A/B/A
   sequences, changed transitive imports, a typed scene refusal and a time-limited compile. A shared-process success
   alone only estimates headroom. [Bun workers](https://bun.sh/docs/runtime/workers), [Bun bytecode](https://bun.sh/docs/compiler/bytecode)
3. **Pass** if the confined candidate achieves median **≤500 CPU ms/job** on the training set, at least **15%**
   paired median child-wall improvement, p95 whole-slot peak memory at most **1.15×** the fresh-child baseline at
   equal concurrency, and byte/verdict parity throughout. Check memory over 100 jobs after warm-up. These thresholds
   are acceptance targets. If the measured start/import ceiling cannot reach 500 ms, or the worker fails parity,
   isolation, memory or speed, stop worker implementation. Census structural hits and probe a dense TypeScript arena
   on the identified hot family. Fund only a narrow native kernel if a batch-in/batch-out prototype then cuts at least
   **20%** paired median end-to-end compile CPU and **20%** p95 peak RSS without changing bytes or dispositions.

All arms continue to accept the current TypeScript scene surface, including author functions, swappable lights and
styles, bounded loops, raw WGSL and the CPU twin. No alternate scene DSL or new public compiler API is needed to
run the experiment.
