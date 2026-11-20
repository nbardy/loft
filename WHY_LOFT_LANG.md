# Why Loft has a language

Loft could have been a library over raw WGSL and wgpu. It is a language instead, because most of what goes wrong
in a GPU scene is not the shader maths. The maths itself compiles to the same code you would write by hand.

## The maths is a thin layer over WGSL

Per-pixel and per-thread maths lowers almost one to one to WGSL. Hermit's `rib` function, compiled by Loft and
written by hand in WGSL, produced identical naga output: nine multiplies, two adds, two cosines, zero loads
(measured, [backends and escape hatches](docs/proposals/2026-11-20_backends_and_escape_hatches.md)). Loft emits a
naga module, naga emits MSL or SPIR-V, and the compiler runs only at build time (20–37 ms measured; budget 2 s).
Nothing sits between your code and frame time.

Every level can be replaced with raw code: a function (`wgsl(...) -> T { }`), a compute kernel
(`kernel ... allow(...)`), a schedule. Whole raw WGSL passes from `.loft`, and native Metal shaders through wgpu
passthrough, are the next escape hatches (OPEN). A 5% whole-pass parity gate holds Loft to hand-written speed: each
engine keeps a hand-written pass as a control.

## What the language buys

**1. It writes the host side and the batching.** Most of a raw wgpu scene is buffers, bind groups, pipelines,
passes, instance management and per-frame encoding. Batching is where hand-written engines go wrong. In Loft you
declare a `kind` and its rows. The compiler makes one shader per kind and one indirect draw, so 1 crab and 20 crabs
both cost 5 GPU commands and 8 shader modules (measured). The old TypeScript engine, where author code drove this,
reached 305 modules for 5 crabs.

**2. It rejects the bugs models actually write.** These are from the blind syntax test, and each error points at the
author's line:
- two laws writing the same field (5 of 8 outputs): `writes` ownership makes it an error;
- a point from the wrong space passed to a shared material: `@Space` types;
- unstaged loops, the root cause of the old engine's blow-ups: `build` / `emit` staging;
- wrong units;
- a ray-march step that is not safe: bounds are derived, never guessed;
- motion that differs between devices or between the CPU and the GPU: the determinism rule (E504).

WGSL checks none of these. You find them as wrong pictures or flaky motion.

**3. It can analyse the program, because it sees maths as maths:**
- automatic derivatives (`spine'(u)`, normals);
- interval and Lipschitz bounds;
- a CPU twin of GPU code, so motion is testable bit for bit and runnable for training;
- constraints with honest verdicts (Held, Missed or Unknown);
- `solve`.

None of this is possible on opaque shader text.

**4. The tools come with it:**
- Tier 0 hot edits;
- naga cost telemetry and AOV heatmaps;
- bit-exact `.loftlog` replay;
- `.loftb` bundles;
- the training contract;
- picture, speed and truth gates on every scene.

**5. Fewer tokens, fewer errors.** The hermit crab is 3,021 tokens in E+, against about 6,320 for the same scene in a
WGSL-style surface (measured in the syntax bake-off). Haiku-class models got the crab, reeds and heron compiling in
3–4 rounds.

## What it costs

- A compiler to maintain.
- A language models have not seen before. That is why there is a one-page [agent card](AGENT_CARD.md).
- Decisions above the maths that need escape hatches: march policy, tile size, memory layout and pass boundaries.
  Those are where a compiler could become a ceiling. The parity gate and whole-pass escape are the answer.

## When not to use it

For one bespoke renderer written once by an expert, raw WGSL and wgpu are fine. Loft pays off when many scenes are
written and rewritten by models and scored by a reward, where safety, composition and cheap rewrites matter more
than one hand-tuned pass.

## Why this syntax

E+ is a terse expression language: infix maths, inferred types, `@Space` annotations, implicit returns. It is not a
WGSL superset (too verbose: types and ceremony on every line) and not a Lisp (prefix maths cost the old engine 2–3×
the tokens and errors). It won a seven-way blind test at about 1,570 tokens per task with the fewest violations.
Tools that transform code use the same program as an s-expression (`loft ir --sexp`). See the
[syntax decision](docs/handoff/2026-11-20_syntax/) and the [language spec](docs/spec/01_LANGUAGE_V1.md).
