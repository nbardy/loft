# Loft v1.0 (spec v1.2): review request (20 Nov 2026)

Everything referred to below is included further down in this same paste, under a header naming its file.

You wrote the Loft v1.0 spec: the E+ language, PATCH_V1_2 and the platform extensions (now `07`). Over the last
24 hours we built it with Codex workers (gpt-6.2-sol as engineers, gpt-6.2-astra for freezes, reviews and
integration), deleted both old engines, and released `v1.0-alpha` on main. We want your review before the next
round of work.

## Where it stands (measured on Apple M4, Metal)

- **Main contains only the new workspace.** Rust crates (lines in `src/`): runner 1,630 (983 in the lead's L0 count),
  ir 548, abi 418, format 815, parse 2,723, plan 949, passes 15,620, std 4,321, cli 5,357. The `.loft` library is in
  `lib/`, the example engines in `engines/`, and the motion scenes in `scenes/`. Both legacy engines (TypeScript and
  the old Rust port) were deleted; `examples/` is kept read-only as Phase 5 source.
- **Gates on main:** Twin 36/36, Truth 34/34, Speed 12/12, Load 31/33. Pictures is 0/18 against the old frozen
  references, because all 18 frames carry the layered-material look change, which is queued for owner approval
  (the references themselves are unchanged). Mutants 6/6, wasm32 and grammar (280 cases) pass. Native tests pass
  152/153.
- **Speed (p95 over 300 frames, about 5 GPU commands per frame):** capsules 1/20 at 0.46/0.72 ms, blobs 1/20 at
  0.51/1.49 ms, hermit-20 at 5.46 ms (budget 5.6), Coral Shrine at 3.90 ms (budget 4.0) at 1280×800.
- **Built:**
  - Phase 1.5: E+ parser over a lossless CST, struct/type/kernel, the lark grammar, `loft fmt`, and an `ir --sexp`
    round trip that is byte-exact with identical pixels. Hermit is 3,021 tokens.
  - The PATCH_V1_2 and 07 ABI, frozen.
  - Phase 2: Tier 0 primitive edits (source→upload 0.48 ms, +0.13 ms GPU), live params, zero-copy interop, naga
    telemetry.
  - Phase 3: motion core, outbox and picking (0 drops, 32,768 correct picks), and crab reference playback with a
    0/481-tick twin difference.
  - Phase 4: BVH and tracing, layered BSDF, media, quant, AOVs, bake with mips (1.7 ms load), and the integrator shelf
    including spectral_caustic.
  - Phase 5: `.loftb`, `.loftlog` replay (0-bit difference over 481 ticks), glTF import, and the training contract
    (48/48 frames, builds 20–37 ms).
  - Room Runners first light in Keel's gallery, on a branch.
- **Open** (`OPEN.md` and the designs `d1.md` and `d2.md` below):
  1. **Authored solvers vs the CPU twin.** They differed on 371/481 crab ticks. d2 proved the cause: implicit FMA
     contraction (first divergence at crab tick 2, heron tick 19, boss tick 8). The fix designed is compiler-verified
     shared binary32 operations; it is not yet built.
  2. **General Tier 0 tapes.** GPU p95 runs +0.1 to +2.5 ms against 0.5 ms. Repeated interpretation dominates;
     fusing one case brought it to +0.17–0.25 ms.
  3. **One execution path.** Native and portable preparation still differ, and execution code is about 5,200–5,500
     lines against a 2,500 target.
  4. **The corpus:** 0/295 certified. It waits on owner approvals and on import speed (100k-triangle import 332–558 ms
     against 100 ms).
  5. **Performance carry-overs:** pipeline cache with warm-up during loading, a solve cache, a volume budget.

## What went poorly (so you can correct the process, not just the code)

- The all-or-nothing phase gate kept Phases 2–5 off main for about 4 hours of rounds. We switched to merging what
  holds the core gates and listing the rest as targets.
- **The runner ceiling.** Lanes kept adding to the runner (2,779 → 3,758 lines) until a dedicated cut lane brought it
  to 983. The lead's broader L0+L1 accounting still says about 5,400 lines against 1,500, and type unification across
  ir/abi/format barely moved it.
- **"Write first, compile once"** made lanes give up on trivial compile errors. We now iterate `cargo check` and cap
  only the test and GPU reruns.
- **Speed and load measurements are contention-sensitive on one shared M4.** The same code scored Speed 12/12 and
  then 6/12. Timestamp readback was a cause.

## The new open question: consumer compiler passes

The owner prefers functions to macros, except where a macro extends the runtime's own dynamics (Clojure's core.async
`go`). Our position: no user macros. New constructs are compiler passes over the typed IR, re-checked by the
gates. The owner asked whether consumers should be able to write those passes too. See
`2026-11-20_consumer_passes.md` (the lead's first answer) and `2026-11-20_consumer_passes_astra.md` (Astra:
build-time Wasm passes over typed source regions, a `behaviour {}` worked example, 1,520 lines predicted).

## What we would like back

One Markdown document:
1. **Verdict on the build so far,** against your spec. Where did we drift? What did we get wrong or overbuild?
2. **The three open engineering classes** (twin/FMA, Tier 0 tapes, one execution path). Do you agree with d2's proved
   causes and designs? If not, give a better design, with predicted numbers.
3. **Size.** What should L0/L1 actually be, given what the runner now carries (interop, outbox, pick, replay, bake)?
   Either give the cut, or argue for new ceilings.
4. **Consumer passes.** Your verdict and the API, if any, including how it composes with E504, ownership, staging and
   the CPU twin. Should `behaviour {}` be the first one?
5. **The next 3 phases of work, ranked,** each with an acceptance test. Include what you would cut.
6. **Process.** What would you change in how we run Sol/Astra lanes and gates?

Mark every number as measured (from the files) or predicted. Keep it to what an implementer needs.
