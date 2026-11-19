# Loft v1.0, "the polished syntax": the ask (20 Nov 2026)

Everything referred to by a file path below is included further down in this same paste, under a header naming that path.

You helped design the Loft meta engine (`docs/PHILOSOPHY.md`, `docs/proposals/2026-11-20_meta_engine_plan.md`, the
language draft `agent_notes/dump_manual_nov_19th_alpha_v1.md`, Astra's `docs/handoff/2026-11-20_spec/astra_spec_v1.md`).
The **semantics are settled** and two phases are built on them: kinds as the only unit of code identity, rows as data,
explicit staging, derived bounds, typed spaces with domain-map spaces, smoothness for `diff`, the six queries, an open
Bsdf, the visibility record, laws, solvers, constraints with honest verdicts, schedules owned by the compiler, Tier 0
for the dirty kind only. Phase 1 drew 1 and 20 hermit crabs with 8 shader modules either way.

What is **not** settled is the **surface**: what an author (mostly an LLM) types. Both earlier specs assumed "a WGSL
superset" without testing anything else. We are now testing, and we want your judgment before we freeze v1.0.

## What we measured (blind test, round 1)

Four one-page language cards with identical semantics; Sonnet and Haiku each wrote three tasks (a hermit crab with
mirrored eyes tracking a world target; 200 reeds in an ink look; a heron hop law with gaze lock, neck limit and body
turn) from the card alone; an Opus judge scored blind. Files: `agent_notes/2026-11-20_syntax_bakeoff/`.

| Syntax | Violations Sonnet / Haiku | Semantic checks | Readability | Avg tokens |
|---|---|---|---|---|
| A: WGSL superset, declarations (`kind { row {} }`, `comptime for`) | 4 / 4 | all pass (Haiku partial on gaze) | 4 | 6,320 |
| D: Python-like (`@kind`, `@comptime`) | 1 / 6 | all pass | 4 | 6,527 |
| B: WGSL superset, builder calls (Astra) | 3 / 6 | Haiku gaze fails | 3 | 6,751 |
| C: Slang/HLSL | 4 / 7 | all pass | 3 | 6,422 |

- Tokens were within 7%: all four still spell types and ceremony.
- A won on **forgiveness**: Haiku made no more errors than Sonnet, because `comptime for` marks the stage on the loop.
- Familiar syntax invites familiar mistakes: in the Python-like surface Haiku wrote ordinary module-level Python loops
  outside any build step (unstaged host loops, the root cause of the old engine's blowups).
- Errors common to every syntax: two laws writing the same kind (5 of 8 outputs); points from the wrong space passed
  to a shared material helper (both Sonnets).
- The judge's grafts: domain maps as values (`sym.p`); typed verdicts (`Verdict[Rad]`); field-level write ownership
  (`writes Heron.{neck, gaze_err}`); generic material helpers over spaces (`fn chitin[S](p: Pt[S])`).

Round 2, running now: **E** a terse expression language (inferred types, numeric promotion, implicit return, `[x,y,z]`
literals, pipes, unit suffixes, rows as constructor arguments); **F** a Lua-like lean surface with infix maths; **G** a
Clojure-like Lisp with macros as staging. Early sign: E/F/G outputs are about 40% smaller in bytes than A–D. The judge
of all seven has not reported yet; its file will be `VERDICT_SEVEN.md`.

## Our own reflection (the owner's ideas and the lead engineer's view)

- The owner leans away from a strict WGSL superset toward something as lean as Lua or Clojure, with TypeScript-style
  inference so types are rarely written.
- Agreed: a strict superset forces WGSL's verbosity (`vec3<f32>`, `1.0` vs `1`, `f32(...)`, `return`, `@` attributes)
  on every author. A raw WGSL block keeps hardware access without making the whole surface WGSL.
- Agreed: local inference, types only at module boundaries.
- Lisp has strong bones (macros as staging, code as data for agents that rewrite code, a tiny parser) but prefix
  maths (`(- (length (- p c)) r)`) is the "maths as nested calls" problem that cost old Loft 2–3× tokens and errors.
- The owner floated a chain: a Clojure/Lua-style Lisp → TypeScript → a TS-to-Rust step → wgpu. The lead engineer
  argued against: TypeScript returns (host tracing was the root cause of the old blowups), errors stop pointing at the
  author's line, two type systems must agree, every edit crosses more tools. Proposed instead: one surface parsed by
  the Rust compiler straight into the typed graph (a 1–2k-line parser), optionally plus a second, machine-facing
  s-expression form of the same IR for agents to transform.
- Forgiveness matters as much as tokens: the cheapest model that writes the scene must not be able to write the
  root-cause bugs (unstaged loops, shared writers, wrong-space points) without an error.

## The questions

1. **Superset or not.** Should v1.0 be a strict WGSL superset, or a lean surface with WGSL only in escape blocks? What
   do we lose either way?
2. **Which family.** A (WGSL declarations), E (terse expressions), F (Lua-like), G (Clojure-like), or something better
   you would design now. Use the cards and outputs; argue from the measured errors, not taste.
3. **Maths.** How should shader maths read? If a Lisp, how do you keep infix maths without two languages?
4. **Inference.** How far can types be inferred (spaces, units, smoothness, rows) and where must the author still
   write them so errors stay local and readable?
5. **Staging on the surface.** What syntax makes the build stage unmistakable even to a weak model (A's `comptime for`
   worked; D's decorators did not)?
6. **Ownership.** How does the syntax make two writers of the same field impossible to write (5 of 8 outputs erred)?
7. **Spaces in helpers.** How does a shared helper (a material) stay correct for points from any space?
8. **The transpile chain.** Do you agree it should be one surface parsed directly by Rust? Is there any case for
   compiling through TypeScript or another host?
9. **A second form.** Is a canonical s-expression (or other) form of the IR worth having for agents that rewrite code?
10. **The floor.** What is the token floor for the hermit crab task, and what does your v1.0 cost against A's 6,320?

## What we would like back

One Markdown document: **Loft v1.0 surface syntax**:
1. Decisions, one line each with the reason and what it rules out.
2. The grammar in EBNF, and the 10 rules that keep it unambiguous for a small parser.
3. The typing and inference rules that the surface relies on (what is inferred, what is written).
4. The three bakeoff tasks written in it, complete, with token counts.
5. Five diagnostics for the errors above (unstaged loop, shared writer, wrong-space helper, mirrored gaze, authored
   bound), each quoting the line and naming the fix in one sentence.
6. Which grafts you accept and why; whether to ship a second machine-facing form.
7. The parser size you predict.

Code in code blocks, numbers in tables, every number labelled measured (from the files) or predicted (yours). Be
ambitious; the semantics stay, the surface may change completely.
