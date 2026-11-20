# Consumer compiler passes: control flow as a library (20 Nov 2026, open question)

Status: **open**. This records the owner's question and the lead's first answer. It is posed to Astra in
`agent_notes/2026-11-20_codex/cp1.brief.md`, and the answer comes back as `docs/proposals/2026-11-20_consumer_passes_astra.md`.

## The question

The owner prefers functions to macros. The one case where macros earn their place is when they extend the
runtime's own dynamics: Clojure's core.async `go` block rewrites sequential code into a resumable state machine
over channels. For a meta engine, that kind of extension looks central, not rare.

Loft's answer so far: no user macros in v1. New constructs (for example a resumable `behaviour { ... }` block for
motion: "walk to the rock, wait until the claw closes, then turn", run as per-tick GPU state) are compiler passes,
registered through `Op::Intrinsic` and the pass registry (spec 06 and PATCH_V1_2 §6). Because they are passes, the
scheduler, the CPU twin, the determinism rule (E504), the bounds and the cost telemetry still see them.

**The owner's follow-up: if passes that change control flow are how Loft grows, consumers should be able to write
them too.** PHILOSOPHY §0.5 says to expose the hard parts and never seal them. Today only the compiler's authors
can add a pass, and that is a seal.

## The lead's first answer (to be challenged)

Yes. Consumer passes should be first-class, with three properties that macros lack:

1. **They run on the typed IR, not on text.** A pass is a function from IR to IR over the same graph that
   `loft ir --sexp` prints. The s-expression form is the consumer's view of the IR; that is where code-as-data
   pays off. Authors still write E+; passes read and write the IR.
2. **The gates re-check everything after every pass.** The checker re-runs typing, spaces, units, ownership
   (`writes`), staging, E504 determinism and the derived bounds on the pass's output. A pass cannot produce a
   program the checker would reject from source, so a consumer pass is safe in the same way consumer source is.
   The CPU twin and the GPU still have to agree.
3. **They are declared and scoped like any other code.** A pass is something a scene or library imports. It is
   applied to the constructs it claims, it appears in `loft show` and `loft profile` (its cost telemetry included),
   and errors in its output point at the author's source span through the pass's provenance.

Open sub-questions:
- **Language.** What are passes written in: `.loft` itself (an IR pattern-and-rewrite subset, staged at build
  time), Rust (a plugin crate, faster but needing a toolchain), or both? WASM plugins would let a Rust pass ship
  inside a `.loftb`.
- **Hygiene and provenance.** How does a rewritten node keep its source span and its name scope?
- **Ordering.** How do two consumer passes compose? Is the order fixed by declaration, or by declared
  phases/claims?
- **Determinism.** A pass must be a pure function of its input IR (a cache key), so builds stay reproducible
  and the pipeline cache stays valid.
- **The first real pass.** `behaviour {}` (resumable per-tick state machines, the `go`-block analogue), `plan`
  lowering, or a schedule transform such as automatic pass fusion. One of these should ship as the worked
  example, written only with the public pass API.

## Not in scope

Text macros, reader macros, and anything that runs before parsing. Build-time code generation stays as staging
(`build` / `emit`).
