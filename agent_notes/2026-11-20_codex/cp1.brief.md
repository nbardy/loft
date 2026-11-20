You are Loft's language and compiler architect. Repo ~/git/loft. You have no other context. This is a design question: write documents only, no product code.

Read:
- docs/proposals/2026-11-20_consumer_passes.md (the owner's question and the lead's first answer, which you should challenge);
- docs/PHILOSOPHY.md (§0.5);
- docs/spec/README.md, with special attention to 01 (language), 06 and PATCH_V1_2 §3 and §6 (Op::Intrinsic, the pass registry, strict desugaring) and 07;
- the current compiler in crates/loft-passes and crates/loft-ir, to see how a pass is structured today and what `loft ir --sexp` prints.

Write docs/proposals/2026-11-20_consumer_passes_astra.md:
1. **Your verdict.** Should consumers write control-flow-changing compiler passes? Argue against the lead where you disagree.
2. **The design.**
   - the pass API: input and output IR, claims, phases and ordering, provenance and hygiene;
   - the language passes are written in (.loft subset, Rust, WASM plugin, or a mix), with costs;
   - how the checker and gates re-verify pass output;
   - caching and determinism;
   - how a pass ships in a `.loftb`.
3. **The worked example.** A `behaviour { ... }` block (resumable per-tick state machine, the core.async go-block analogue) for motion: the author's source, the pass, the IR it emits, the per-tick GPU state it needs, and the CPU twin.
4. **The other two candidate first passes** (`plan` lowering and automatic pass fusion), each with a one-paragraph sketch.
5. **What changes in the spec:** the exact grammar, ABI and IR additions, as a patch section in the style of PATCH_V1_2, plus what must be frozen now versus later.
6. **Predicted line cost.** Mark every number as predicted.

Commit by path to main and push. Messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Final message ≤150 words.
