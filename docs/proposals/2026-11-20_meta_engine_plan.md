# Loft as a meta engine: the plan from first principles (20 Nov 2026)

Read first: `docs/PHILOSOPHY.md` (the stance), `CORE.md` (priorities), `docs/history/2026-11-19_catalogue.md`
(what exists and what we measured). The language specification draft is
`agent_notes/dump_manual_nov_19th_alpha_v1.md` (the chief scientist's answer to our questions); this plan adopts it
with the decisions in §3.

Everything old is left behind: the TypeScript authoring API, the TS compiler and host, the lean Rust port and its wire
formats, and the earlier syntax. Mining old code is allowed (naga emit, interval arithmetic, solve, parity tooling);
porting it is not a goal. Back-compat is not a goal: every example and seed is regenerated in the new language.

## 1. What we are building

A meta engine: a small, slow-changing core that compiles and runs compositions, sub meta engines for render, light
and motion, and example engines written in one language that any person or model can rewrite.

| Layer | Contents | Size budget | Changes |
|---|---|---|---|
| **0. Runner** (Rust, wgpu; native and WASM) | Device and surface, buffer/texture arenas, async pipeline cache on disk, command-list and RenderBundle executor, indirect draw/dispatch, Tier 0 tape VM host, state store with checkpoints, frame clock and input log | ≤ 1,000 lines | Rarely |
| **1. Core IR and gates** | Typed SSA graph (hash-consed), kind/row/table manifest, schedule IR, verdicts; the gate suite | ≤ 1,500 lines | Slowly |
| **2. Compiler passes** (regenerable behind the gates) | Parser (`.loft` v1.0, E+), checker (spaces, units, smoothness), staging, forward AD, intervals and derived Lipschitz, dependency-wave batching, (wave, law) bucketing, kind dedup, cull and indirect args, naga emit, tape emit, CPU twin | ~10k lines, one module per pass | Often |
| **3. Library and example engines** (written in the language) | Analytic and SDF primitives with Lipschitz axioms, noise, BSDF lobes, light estimators, schedules (vis-buffer deferred, forward, path-traced reference, toon and print), motion laws and solvers, body plans | Unbounded | Constantly |

## 2. The invariants (what the measurements forced)

1. **A kind is the only unit of code identity.** Continuous values live in rows; kind parameters are discrete and
   budgeted. Module count is O(kinds) by construction; a scene's module count does not change with its instance
   count. Gate: modules and pipelines for 1 and 20 crabs are identical.
2. **The compiler owns dispatch shape.** Dependency-wave batching, (wave, law kind) bucketing, per-kind indirect
   draws inside RenderBundles. Schedules may override; the gates catch regressions. Gate: GPU commands per frame are
   O(kinds + waves), budgeted per scene.
3. **Staging is explicit.** `comptime` loops emit rows; runtime loops stay loops; nothing unrolls silently.
4. **Bounds are derived, never authored.** Lipschitz and smoothness come from the compiler; `Unbounded` is a named
   result that forces a conservative step.
5. **Nothing reports false.** Verdicts are Held(margin), Missed(err) or Unknown(reason); gates pass only on Held.
6. **Vertical slices end to end, from the first week**, with every gate live. No layer is built ahead of a picture.

## 3. Decisions on the language draft

Adopted as specified: spaces and generative domain-map spaces (`map`, `.cell`, `.to_parent`), units, the smoothness
lattice and `diff` rule, the derived Lipschitz calculus, three-stage staging with E401/E402, `kind`/`row`/`query`,
`scan` with schedules, `solver` with colored Gauss-Seidel and Jacobi, `law`, `constraint` returning `Verdict`,
`schedule`, Vis64 plus tile classification, RenderBundle plus one `drawIndirect` per kind, the Tier 0/1 contract.

Decided here:
- **One surface, `.loft` v1.0 (E+), not a WGSL superset** (decided 20 Nov after a seven-syntax blind test and the
  research scientist's review: `docs/spec/LOFT_V1_SYNTAX.md`, `docs/handoff/2026-11-20_syntax/`). No TypeScript
  anywhere in the product. The CLI and the WASM build are Rust.
- **Units ship first-class but inferred**; literals need suffixes only where a unit is ambiguous.
- **Tier 0 is edit-time only**, one dirty kind at a time, kind-bucketed; motion and solver laws never run in Tier 0.
- **Exactness is not a goal** (owner, 20 Nov 2026). Pictures and the twin are judged by perceptual and statistical tolerances. Exact determinism is an opt-in contract for scenes that need exact replay or training reproducibility.

Open, for the chief scientist (motion) and the first slice (render): see §6 and
`docs/handoff/2026-11-19_motion_chief_scientist.md`.

## 4. Gates (live from phase 0)

| Gate | Pass condition |
|---|---|
| Pictures | Every scene in the new corpus renders every expected frame; perceptual diff against its approved reference within tolerance; a missing frame fails |
| Twin | CPU twin agrees with GPU within stated tolerances; exact agreement only where a scene opts into determinism (replay, training) |
| Speed | p95 frame time and GPU command count within the scene's budget |
| Load | Shader modules, pipelines, cold and warm load within budget; module count independent of instance count |
| Truth | Every constraint check is Held, or the scene declares the Missed/Unknown it expects |

## 5. Phases

Each phase is a vertical slice with predicted numbers written before building and scored once.

- **Phase 0, the spine (days 1 to 2).** Tag `legacy/2026-11-19`. New workspace (`crates/`: runner, ir, compiler
  passes, cli; `lib/` and `engines/` in the language). Parser and checker for a minimal subset (spaces, `kind`, `row`,
  `query visible|trace`, `schedule`), naga emit, runner, one analytic kind and one SDF kind drawn into a vis-buffer
  and shaded. All five gates running on these two scenes.
- **Phase 1, the hermit crab (render).** The draft's crab without motion: shell via `scan` (baked parallel transport),
  claw SDF with derived L, legs as one kind with 12 rows, face with mirror spaces, Bsdf algebra, tile shading.
  Predicted (draft §7): 11 modules, 12 pipelines, 13 GPU commands, 2.1/2.4 ms for 1 crab and 4.0/4.6 ms for 20 at
  1280×800 on the M4, cold load about 1.1 s.
- **Phase 2, editing.** Tier 0 tape for the dirty kind, async promotion, diagnostics E104/E211/E307 with fixes,
  `loft show`. Hot reload under 2 ms for a kind edit.
- **Phase 3, motion** (after the chief scientist's review): state store, laws, solvers, verdicts; the crab scramble,
  the heron gaze hop, a boss hit; 1,000 creatures under 0.5 ms.
- **Phase 4, light.** Reference path tracer as an example engine; real-time estimators judged against it.
- **Phase 5, the corpus.** Regenerate examples and seeds in the language (wide Codex Sol fan-out, each gated by the
  picture and speed gates); creative training re-pins; Keel and room-runners embed the runner.

## 6. How we work this time

- Opus for design, integration and judgement; Codex gpt-6.2-sol for bulk implementation lanes; Astra for audits.
- One integrator merges; at most four GPU-heavy lanes; one full gate per wave.
- Lanes are cut by vertical slice, not by layer; every lane's gate is end to end.
- Runners are resumable and branches are pushed after every milestone (two machine restarts on 19 Nov).
- The owner sees a picture and the numbers at the end of every phase.
