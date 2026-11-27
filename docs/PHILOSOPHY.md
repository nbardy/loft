# Loft: philosophy and direction

The aim is to move 3D engines forward, in rendering and in motion, by building tools that agents and people keep
rewriting. Loft should support a loop of recursive self-improvement: every consumer, human or model, can replace any
part, from a single shading term to the whole engine, and is helped to do so. The best version of each part wins
because replacing a part is cheap, local and safe.

`CORE.md` turns this into priorities and a definition of done. `docs/design/render.md` (render and light) and
`docs/design/motion.md` are the design goals for the sub meta engines. This page is the stance they share.

## 0. A meta engine

Loft is a **meta engine**: an engine for writing engines.

- **The meta engine** is the small core: one graph of maths and GPU programs, the compiler that turns a composition
  into fast GPU work, the runtime that runs it, and the analysis (CPU twin, checks, solve, scheduling) it offers
  every part. It is the only part that must stay tiny and stable.
- **Sub meta engines** specialise it for a domain: **render** (how things are intersected, rasterised, marched and
  shaded), **light** (how light is transported: direct, shadows, global illumination, media), and **motion** (state,
  laws and constraints over bodies). Each provides interfaces, orchestration and library functions for writing
  engines in that domain.
- **Engines** are programs written in the sub meta engines: a PBR path tracer, a toon renderer, a demoscene raymarcher,
  a Spore-like locomotion engine, a Disney-style performance engine. The ones Loft ships are example engines: useful
  defaults, written only with public pieces, that any person or model can read, copy, mix and rewrite.
- **Lines live in the library and the example engines**, which may be large. The meta engine and the sub meta
  engines' interfaces stay small. Size is judged per layer.

## 0.5 Agentic development inverts the old trade

Engines and frameworks were built for a world where hard code was expensive: hide the difficult parts (intersection,
light transport, solvers, schedulers, animation systems) behind a framework, and accept its trade-offs in return.
That advice is in most of what models have read: use the framework, let it do the heavy lifting, write as little
code as possible.

For us the trade has flipped. Highly technical code is now cheap: an agent writes a ray-quadric intersector, a
ReSTIR estimator, a capture-point stepper or a sparse solver in minutes, and rewrites it when it is wrong. What is
expensive is **lock-in**: the trade-offs a framework made for everyone, which we cannot remove because the hard part
is sealed inside it. Those trade-offs are what hold the picture, the speed and the motion back.

So Loft does the opposite of a classic engine:

- **Expose the hard parts; never seal them.** Every intersector, estimator, solver, scheduler and controller is
  ordinary, readable code in the library, written to be rewritten. "This is hard, leave it to the engine" is not a
  reason to hide anything.
- **Prefer writing the specific thing over configuring the general thing.** A purpose-built function for this scene
  or this body beats a flag on a general system. Options and settings are a smell when a few lines of code would say
  it exactly.
- **Judge a design by what it lets the consumer change**, not by how much code it saves them from writing. Code
  volume in the library and in consumer code is cheap; a trade-off the consumer cannot undo is not.
- **The meta engine provides leverage, not walls**: shared representation, compilation to fast GPU work, analysis
  (CPU twin, checks, solve), orchestration and good interfaces. It never decides on the consumer's behalf something
  the consumer cannot override.

## 1. No baked engine

An engine here is a default composition of small functions, not a framework that owns the program.

- **The Rust core is tiny (~10k LOC target, compiler + runner + CLI + ABI combined).** The runtime loads a set of GPU
  programs, resources, and an ordered pass graph and runs them on a device someone else may own (~1.5k–2k LOC). The
  compiler turns typed `.loft` maths, `kind` row tables, and schedules into validated WGSL and bundles (~5.5k LOC).
  **Everything else is `stdlib/`.**
- **`stdlib/` is uncapped and does NOT count against core size.** Grow `stdlib/` freely as long as modules are clean,
  well-organized, composable, and forkable. Putting an engine, shader, material, shape, or motion solver in `stdlib/`
  is always preferred over putting 10 lines of special-case code in Rust.
- **Every framework is a recipe.** Any "renderer", "animation system", "physics" or "scene" that Loft ships is a
  composition of the same public pieces a consumer has. A consumer can copy it, edit one line, or throw it away.
  Nothing in a default is privileged.
- **The escape hatch is the normal path.** Every level can be written directly: a maths expression, a WGSL function,
  a draw with its own shader, a compute pass, a whole pass list. Higher levels call lower ones and lower ones can be
  called from higher ones. There is no point where the consumer must stop and ask the engine for a feature.

## 2. Modularity that costs nothing

Composition must not cost speed. Parts are small and many; the program that runs is compiled from the composition.

- **Compile the composition, not the components.** Inline, specialise, fuse passes, fold constants and share
  shaders by kind where the frame is faster that way; keep calls, loops and tables where they are faster. The
  author's structure is for people and models; the GPU sees what is fastest.
- **Load time is precompute, frame time is sacred.** We will spend load-time compute (bakes, solves, pipeline
  compilation, acceleration structures) to buy frame time. Load time must still not grow with the amount of
  content: shaders are shared per kind, instances are data.
- **The engine helps, it never owns.** Because parts are explicit functions in one graph, the engine can analyse
  them: a CPU twin, interval checks, solving for curves, scheduling work to the right stage, choosing analytic vs
  marched vs rasterised. Consumers get that help for their own parts too, and may opt out.

## 3. Consumers define the laws

Rendering and motion are both "state, laws, and constraints, evaluated somewhere".

- **State** is explicit and typed: what persists between frames, and where it lives (a closed form of time, a baked
  table, a per-frame GPU buffer).
- **Laws** are functions the consumer can write or replace: a BRDF, a light transport estimator, a spring, a gait
  controller, a contact model.
- **Constraints** are conditions the consumer can state, that the engine can solve for, enforce, or check: a foot
  stays planted, a neck has a length, a gaze holds a point, a material conserves energy.
- **Truth.** Whatever the engine reports about a constraint is true: proved, measured, or named as unknown.

## 4. Few tokens, no hidden code

The consumer is often a model. The shortest correct program wins.

- Write maths as maths (infix, as a shader writer writes it), not as nested calls. The surface is lean and typed by
  inference; WGSL is what Loft compiles to and the escape hatch for what only the hardware can say.
- A new name must buy a capability the consumer did not have. This rule is about the **core**: keep the meta
  engine's own surface small and lean. The **standard library** follows a different test (owner, 20 Nov 2026):
  when a sufficiently complicated block of code recurs and costs **3× or more the tokens** of calling a reusable
  function, that function belongs in std. A std function is ordinary forkable `.loft` code, so adding one hides
  nothing and locks nothing in.
- Every generated program can be printed and read; errors point at the consumer's own line.
- The documentation a model reads is generated from the code, so it cannot drift.

## 5. Ambition

Set new state of the art in three things at once, without inheriting the limits of current engines:

1. **Speed**: frame time and load time, by choosing the fastest exact method per object and compiling compositions.
2. **Developer experience**: adaptability, flexibility and customisation. Any part rewritten in minutes, by a
   person or an agent, without forking an engine.
3. **Capability**: rendering that mixes analytic surfaces, distance fields, triangles, volumes and particles in one
   frame with physically based light transport; motion that mixes authored performance with physical, adaptive,
   constraint-driven movement on any body.

Prior art is a launchpad. Understand why a technique won, keep it if nothing better exists, replace it if we can do
better, and say which ideas are inherited and which are ours.

## 6. Architectural Red Lines (for Future Agents and Authors)

Previous agent waves let the Rust core bloat to ~29.4k LOC by bolting demo-specific renderers, string hacks, and
benchmark workarounds into `crates/`. These rules are hard boundaries going forward:

1. **Keep the compiler — delete the baked engines.** Loft has a compiler specifically for: (a) 3×–4× token density and
   O(kinds) GPU batching (`kind` row tables + stage-0 `build`/`emit`), (b) static analysis over maths (interval/Lipschitz
   bounds, forward-mode AD, `@Space` affine checks, `writes` ownership, deterministic `det_*` motion), and (c) generic
   pass-graph lowering to `wgpu`. Everything above that — visibility buffers, shadows, tiled shading, woodblock outlines,
   path tracers, caustics, volumes, creature anatomy — belongs in `stdlib/`, NEVER hardcoded in Rust.
2. **Zero scene, style, or creature names in Rust.** Never branch in the compiler on literal library/scene names
   (`"woodblock_pigment"`, `"sun_sky"`, `"spectral_dielectric"`, `"bead_observation"`), never put game/creature structs
   (`CreatureHeader`, `FootPin`, `ManipState`, `TrailNode`) in `loft-abi`, and never maintain hardcoded stdlib file
   whitelists in the module loader.
3. **Zero WGSL string surgery.** Never stitch shaders with `.replace(...)`, comment-line splits, or byte-offset header
   slicing across WGSL text. Compose from structured AST/IR or explicit template bindings.
4. **One implementation per contract.** One generic `wgpu` runner (native + WebGPU — no separate `loft-native` host layer
   and no parallel JS metadata exporter), one CPU expression evaluator (keep verification twins in `tools/loft-verify/`,
   not in core runtime crates), one ray-AABB / BVH implementation.
5. **Never weaken soundness or gates to pass a benchmark.** Never punch holes in `@Space`, module privacy, Lipschitz
   bounds, or determinism checks to salvage an eval run, and never filter contended runs out of gate exit codes. If a
   check is partial or fake (e.g. kernel-only units), either make it real or remove the claim.
6. **Core Rust shrinks; `stdlib/` grows cleanly.** Judge Rust core size strictly under `crates/` (target ~10k LOC).
   Keep test suites and examples in `tests/` and `tools/`, not inline in `crates/*/src/`. `stdlib/` is uncapped — prefer
   clean, composable `.loft`/`.wgsl` library code over adding special cases to Rust.
7. **Keep existing consumer rollouts working.** When moving hardcoded schedule verbs out of Rust into `stdlib/`, keep
   the same verb names and call signatures, and make new language ergonomics (`v3(...)@Space`, `match`, `if` over `emit`,
   fixed arrays, `struct` returns, `target`) strictly additive so existing rollouts (`out/spark_pairs/`, `corpus/`,
   `scenes/`, `engines/gates.json`) keep compiling and rendering.
