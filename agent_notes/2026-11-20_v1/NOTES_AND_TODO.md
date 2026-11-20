# Notes and follow-ups, 20 Nov 2026 (owner ↔ lead ↔ chief scientist)

The decisions and open questions from today's conversation. `OPEN.md` holds the measured engineering targets;
this file holds the reasoning behind them and the follow-ups that are not yet lanes.

## 1. Syntax and the second form
- v1.0 is **E+**: a terse expression language with infix maths, inferred types, `@Space` annotations, `build`/`emit`
  staging and `writes` ownership. Not a WGSL superset, not a Lisp. It is closest to Rust/Swift syntax with ML-style
  expression bodies.
- `loft ir --sexp` is the machine-facing form for tools that transform code. It is predicted to cost 1.5–3× more
  tokens than E+ (types, spaces and units written out; prefix forms). That is unmeasured: the lab binary was stale.
  TODO: measure it on hermit, capsules and Shrine.
- Version naming: the tag `v1.0-alpha` is language v1.0 with spec v1.2 (PATCH_V1_2) and 07 implemented. TODO
  (owner's choice): retag it as `v1.2-alpha` and make the docs say "language v1.0, spec v1.2".

## 2. Macros vs consumer compiler passes
- Functions over macros; build-time generation is staging (`build`/`emit`), the Zig-comptime equivalent.
- The real macro case is extending runtime dynamics (core.async `go`). In Loft that is a compiler pass over the
  typed IR, re-checked by every gate. Consumers should be able to write passes too (owner).
- Lead: docs/proposals/2026-11-20_consumer_passes.md. Astra: docs/proposals/2026-11-20_consumer_passes_astra.md,
  which proposes build-time Wasm passes over typed source regions, with a `behaviour {}` worked example and
  ~1,520 lines predicted. Astra's key finding: the CPU and GPU can agree and both resume on the wrong tick, so
  resumable constructs also need an independent tick trace.
- TODO: the chief scientist's verdict (asked in the review packet), then decide whether `behaviour {}` is the first
  pass.

## 3. GPU commands and frame cost
- A command is a draw, dispatch, pass begin/end, copy/clear, or a pipeline/binding switch. The old engine
  measured ~1.3 ms + ~15 µs per command on M4, mostly GPU-side cost: dependent-dispatch barriers, small dispatches
  that leave cores idle, tile flushes at pass boundaries. CPU encoding is a smaller share.
- Commands hold handles, not data. Tables, state and poses stay on the GPU; per frame the CPU uploads a few hundred
  bytes. Work counts already live on the GPU (indirect draw/dispatch), and draw lists are pre-recorded
  (RenderBundles). A fully GPU-generated command list (Metal ICBs, Vulkan DGC, D3D12 ExecuteIndirect) is not
  available through wgpu. It needs a native backend.
- Users write their own command lists as `schedule`s, plus `kernel`, `PassLoop` and clocks. The compiler may batch
  them, and the gates budget commands per scene against measured frame time. There is no fixed "N per frame" rule.
- These are general GPU hygiene rules (few large commands, few pipelines, data resident, less memory traffic, low
  divergence, few tile flushes, big dispatches). Loft applies them in the compiler and gates.
- Now: ~5 commands per frame (~75 µs). Hermit-20 at 5.46 ms and Shrine at 3.90 ms are per-pixel shading. The
  frame-cost directions are in OPEN.md: shading cost, shading pass fusion, Apple tile memory, async compute,
  megakernels.

## 4. Backends, escape hatches, and "light vs heavy compile" (owner's worry)
- The worry: a compiler layer can cap performance where straight WGSL/Metal would not. Answer so far:
  - **Per-pixel maths** lowers near 1:1 to WGSL. Loft emits a naga::Module, and naga produces MSL for Metal.
    The compiler runs at build time only (20–37 ms measured, budget 2 s).
  - **Heavy parts:**
    - analysis, which emits no code;
    - work shape (batching, merges, indirect), which is where a ceiling could exist; schedules are the override;
    - codegen choices: inlining, specialisation, and FMA policy.
  - The d2 twin fix forbids FMA contraction for E504 motion state. It must never reach shading. Leaking would cost
    an estimated 10–15% on hermit-20 and Shrine (scientist's estimate).
- Escape hatches: `wgsl(...) -> T {}` functions and `kernel ... allow(...)` compute (in the spec). A whole raw WGSL
  pass with custom bindings is unverified in code (bk1/bk1b check it).
- Native shaders: `msl(...) {}` via wgpu passthrough (`create_shader_module_passthrough`, MSL text or `.metallib`;
  Burn/CubeCL use this hybrid). Loft must auto-emit Metal resource slots from `PassDecl.bindings`. With no CPU twin,
  Truth is Unknown unless a twin is supplied.
- A pure Metal backend (ExecutionBackend over objc2-metal): defer until a measurement shows tile memory or ICBs win
  real milliseconds on hermit or Shrine. The lead and the chief scientist agree.
- **New gate, "never slower than hand-written":** each shipped engine keeps a hand-written WGSL control for one hot
  pass, and Loft's output must stay within a few percent. Per the scientist, it needs a whole-pass control too, so
  it catches bad pass shape as well as inner-loop codegen.
- Design docs: docs/proposals/2026-11-20_backends_and_escape_hatches.md (Astra, bk1), plus the "Scientist checks"
  section (bk1b). The chief scientist's checks are verbatim in scientist_bk1_checks.md.

## 5. Process lessons today
- The all-or-nothing phase gate kept Phases 2–5 off main for ~4 h. Now: merge what holds the core gates, and list
  the rest as targets.
- "Compile once, at most 2 reruns" made lanes give up on trivial compile errors. Now: iterate `cargo check` until
  it compiles, and cap only the test and GPU reruns.
- Speed and Load are contention-sensitive on the shared M4 (12/12, then 6/12 on the same code). Score them with no
  concurrent builds.
- Cambium checkpoint collision: fixed in fb755e9 on release/v0.10 and installed locally. TODO (owner): push and
  release it.

## TODO (not yet lanes)
- [ ] bk1, then bk1b (the scientist's three checks), then send bk1 to the chief scientist for a second look at the
      emitted shaders.
- [ ] Implement the "never slower than hand-written" gate (inner-loop control plus whole-pass control) per bk1.
- [ ] Measure s-expression vs E+ tokens.
- [ ] Owner: picture approvals (out/meta/i6/approval_needed.md, out/meta/d1d/approval/, out/meta/i5-r2/approval_needed.md).
- [ ] Owner: retag v1.2-alpha? Push the cambium fix? Merge the Room Runners first-light branch (Keel) via PR?
- [ ] Chief scientist review: the packet is at docs/handoff/2026-11-20_review/ (rebuild with tools/review_dump.sh).
