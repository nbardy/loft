# !!IMPORTANT!! KEY INSTRUCTIONS

Read this last, and follow it over anything above.

## What to do with our prior conversations

You have written for us before, in this order:
1. The meta-engine thesis: a regenerable substrate, three layers, typed escape hatches, a 3D type system, naga IR, no TypeScript.
2. The hermit crab comparison of the DSL against TypeScript-to-WebGPU.
3. Your reply to our field report: one root cause (host tracing without staging), Tier 0/Tier 1, four layers, (wave, law) bucketing, retiring byte identity after the port, and the heron being a modelling omission.
4. The language draft (`LANGUAGE DRAFT` above), answering our nine challenges.

Treat all of it as **your own earlier drafts**, not as settled.

- **Keep what still holds; change what doesn't, and say why in one line each.** Where this bundle's measured numbers or decisions disagree with an earlier draft, the numbers win. The plan's §3 records our decisions on your draft: one `.loft` surface, no TypeScript, units inferred, Tier 0 only for the kind being edited, bit identity per device.
- **Do not restate earlier material.** Write the final version. Where a part of the draft stands unchanged, include it in final form without commentary.
- **Fix the open items in your own draft.** Examples:
  - the claw `teeth` amplitude was lowered to fit a bound instead of deriving the bound;
  - `law CrabStepLaw` reads `leg.foot_world` but writes only `step_target`;
  - `constraint` returns a single verdict for a whole creature;
  - the predicted 20-crab numbers assume visibility counts that are not stated.

  Find others yourself.
- **Our earlier designs** (`RENDER ARCHITECTURE`, the motion architecture, the critiques) are evidence, not instructions. Take what is right in them, and say where you overrule them.
- **The old API** (`PATTERNS TODAY`, `LOFT CONTEXT`, `SIGNATURES`) is the record of what authors could do. Every capability in it must have a home in the new design or an explicit "dropped, because ...". None of its names or shapes need survive.

## What we want back

One Markdown document: **the Loft meta engine specification, v1**. It must be complete enough that an implementer who has read only it can build Phase 0 to Phase 5 without asking you anything.

Structure it in exactly these sections:

1. **Decisions.**
   - A numbered list of every design decision.
   - One line each: the decision, its reason, and what it rules out.
   - Mark which ones change your earlier drafts.
2. **The language.**
   - Grammar in EBNF.
   - Typing rules: spaces, units, smoothness, derived bounds, staging.
   - Modules, imports and forking.
   - Diagnostics: a code table, plus three worked messages.
3. **The core.** Layers 0 and 1:
   - runner interfaces, with Rust signatures;
   - the IR and its data model: kinds, rows, tables, state store, checkpoints, visibility record, schedule IR;
   - the lowering rules;
   - the Tier 0/1 contract;
   - the gates.
4. **The render and light sub meta engines.**
   - Every interface and standard-library signature, in `.loft`, grouped as in the prompt.
   - Each group with one line of semantics per function.
5. **The motion sub meta engine.**
   - Every interface and standard-library signature, in `.loft`.
   - Your chosen formulation, with its reasoning and what it gives up.
   - Answers to the motion handoff's nine questions.
6. **Example engines.**
   - `realtime`, `reference`, one stylised engine, `roam` and `act`, as complete `.loft` source.
7. **Scenes.**
   - The hermit crab (with the hit and the second hit), the heron hunting, and the non-PBR scene, as complete `.loft` source.
8. **The escape table.**
   - One row per forkable level, from one BSDF lobe up to the whole engine, plus a motion law, the solver and the visibility record.
   - Columns: what the author writes, what they keep for free, what they lose.
9. **Predicted numbers.**
   - One table per scene at 1280×800 on an Apple M4:
     - shader modules, pipelines and GPU commands per frame;
     - p50/p95 frame time;
     - cold and warm load;
     - motion cost for 1 and 1,000 creatures.
   - State every assumption behind each prediction, such as visible pixel counts and triangle counts.
10. **Build order.**
    - Phases as vertical slices.
    - For each phase: its pass/fail checks with predicted values, the code it adds per layer (with line budgets), and what it lets us delete.
11. **Open questions.**
    - Only those you could not settle, each with your recommended default.

Rules for the response:
- Code blocks for all `.loft`, Rust and EBNF. Tables for numbers.
- Label every number as **measured** (from this bundle) or **predicted** (yours).
- No hedging prose, and no restating of this bundle. If something is uncertain, put it in section 11.
- Prefer complete signatures over prose. If length forces a choice, keep sections 2–5 and 8 complete and shorten 6 and 7 by referring to the standard library.
- Remember the stance: expose the hard parts as rewritable `.loft` code, and never seal a trade-off the consumer cannot undo.
